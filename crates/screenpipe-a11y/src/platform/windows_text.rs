//! Entered-text reads: the focused field's text at the caret, through UI
//! Automation (see `docs/ENTERED_TEXT_CAPTURE.md`).
//!
//! The caller (`UiaContext::read_entered_text`) decides whether a read may
//! happen and confirms it afterwards; this module only fetches text from an
//! element the keyboard privacy probe has already verified, under a budget.

use std::cell::Cell;
use std::time::{Duration, Instant};

use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationTextPattern, IUIAutomationTextRange,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Line,
    TextUnit_Paragraph, UIA_TextPatternId, UIA_ValueValuePropertyId, UIA_E_TIMEOUT,
};

use super::budget::{UiaLimits, UiaStop, WalkBudget};
use crate::entered_text::{
    needs_line_window, CaretAt, ReadFailure, TextSource, MAX_READ_BYTES, MAX_READ_LINES,
};

/// A read's total budget (up to eighteen provider requests, six more for
/// each selection end moved onto the element's edge and seven more for a
/// line window, then the two of the focus recheck): under the probe's,
/// because the keyboard privacy worker probes nothing while it runs.
pub(super) const READ_BUDGET: Duration = Duration::from_millis(150);

/// UTF-16 code units requested from a text range: one more than a read keeps,
/// so a longer text is known to be truncated.
const MAX_READ_CHARS: i32 = MAX_READ_BYTES as i32 + 1;

/// UTF-16 code units requested for a line window: room for twice
/// [`MAX_READ_LINES`] wide terminal lines. The window is cut to its input
/// before the read's bounds apply; a longer one is discarded (its caret's
/// line could be cut off).
const MAX_WINDOW_CHARS: i32 = 64 * 1024;

/// The text one read returned, before the tracker's bounds.
pub(super) struct FieldText {
    pub text: String,
    /// Where the caret is in `text`, when known.
    pub caret: Option<CaretAt>,
    pub truncated: bool,
    pub source: TextSource,
}

/// Provider calls of one read: each waits at most for what remains of the
/// read budget, and their cost is recorded for the read breaker.
pub(super) struct ProviderCalls<'a> {
    budget: WalkBudget,
    set_timeout: &'a dyn Fn(u32) -> Result<(), UiaStop>,
    work: Cell<Duration>,
    called: Cell<bool>,
    failed: Cell<bool>,
    timed_out: Cell<bool>,
    budget_stopped: Cell<bool>,
}

impl<'a> ProviderCalls<'a> {
    pub fn new(set_timeout: &'a dyn Fn(u32) -> Result<(), UiaStop>) -> Self {
        Self {
            budget: WalkBudget::new(
                UiaLimits {
                    timeout: READ_BUDGET,
                    ..UiaLimits::keyboard_privacy()
                }
                .clamped(),
            ),
            set_timeout,
            work: Cell::new(Duration::ZERO),
            called: Cell::new(false),
            failed: Cell::new(false),
            timed_out: Cell::new(false),
            budget_stopped: Cell::new(false),
        }
    }

    /// One provider call (a request served on the target's UI thread).
    pub fn call<T>(
        &self,
        request: impl FnOnce() -> windows::core::Result<T>,
    ) -> Result<T, ReadFailure> {
        self.budget
            .remaining_request_timeout()
            .and_then(|ms| (self.set_timeout)(ms))
            .map_err(|_| {
                self.budget_stopped.set(true);
                ReadFailure::Timeout
            })?;
        self.called.set(true);
        let started = Instant::now();
        let result = request();
        self.work.set(self.work.get() + started.elapsed());
        result.map_err(|error| {
            self.failed.set(true);
            if error.code().0 as u32 == UIA_E_TIMEOUT {
                self.timed_out.set(true);
                ReadFailure::Timeout
            } else {
                ReadFailure::Failed
            }
        })
    }

    /// Whether the read finished within its budget.
    pub fn within_budget(&self) -> bool {
        self.budget.check_complete().is_ok()
    }

    /// What the read cost its provider, if it called it at all.
    pub fn cost(&self) -> Option<crate::uia_breaker::ProbeCost> {
        self.called.get().then(|| crate::uia_breaker::ProbeCost {
            provider_work: self.work.get(),
            timed_out: self.timed_out.get(),
            answered: !self.failed.get() && !self.budget_stopped.get(),
            budget_stopped: self.budget_stopped.get() || !self.within_budget(),
        })
    }
}

/// Read the text around the caret of `element`, the verified focused
/// element: through its own `TextPattern` when it has one, otherwise its
/// `Value` when `value_allowed` (an `Edit` control under an unrelaxed
/// decision). Never an ancestor's, descendant's or another element's text:
/// the caret must lie inside the element's own text (its pattern's document
/// range, or just outside its edge with no text between), and the range read
/// is clamped to it. Some providers (Chromium,
/// WebView2, Electron) move range endpoints through the whole page, so the
/// clamp is what keeps a chat box's read out of the conversation around it.
///
/// The text runs from the start of the paragraph before the caret's (or the
/// selection's) to the end of the caret's (or the selection's last)
/// paragraph, within the element. When that is no paragraph read
/// ([`needs_line_window`]), a line window of up to [`MAX_READ_LINES`] lines
/// before the caret's, the caret's and as many after it is read instead, for
/// the tracker to cut to the input at its prompt
/// (`crate::entered_text::CaretText::new`). The caret's place in the text is
/// returned with it.
///
/// # Safety
/// COM must be initialized on this thread and `element` must be a live
/// element obtained on it.
pub(super) unsafe fn read_field_text(
    element: &IUIAutomationElement,
    text_pattern: bool,
    value_allowed: bool,
    calls: &ProviderCalls<'_>,
) -> Result<FieldText, ReadFailure> {
    if text_pattern {
        let pattern: IUIAutomationTextPattern =
            calls.call(|| element.GetCurrentPatternAs(UIA_TextPatternId))?;
        let document = calls.call(|| pattern.DocumentRange())?;
        let selection = calls.call(|| pattern.GetSelection())?;
        // The range array is a client-side copy: no provider call.
        if selection.Length().map_err(|_| ReadFailure::Failed)? < 1 {
            return Err(ReadFailure::NoTextSource);
        }
        let range = selection.GetElement(0).map_err(|_| ReadFailure::Failed)?;
        // Each endpoint of a range and the side beyond which it lies outside
        // the element's text: compared with the same endpoint of the
        // element's text, negative is before its start, positive after its end.
        let edges = [
            (TextPatternRangeEndpoint_Start, -1),
            (TextPatternRangeEndpoint_End, 1),
        ];
        let side_of = |range: &IUIAutomationTextRange, endpoint| {
            calls.call(|| range.CompareEndpoints(endpoint, &document, endpoint))
        };
        let onto_edge = |range: &IUIAutomationTextRange, endpoint| {
            calls.call(|| range.MoveEndpointByRange(endpoint, &document, endpoint))
        };
        // The selection must lie inside the element's own text. Chromium may
        // place a caret at the edge of a field just outside its text (the same
        // visible place, in a neighbouring node): such an endpoint is moved
        // onto the edge, but only when no text lies between them.
        for (endpoint, outside) in edges {
            if side_of(&range, endpoint)?.signum() != outside {
                continue;
            }
            let other = if outside < 0 {
                TextPatternRangeEndpoint_End
            } else {
                TextPatternRangeEndpoint_Start
            };
            // From the endpoint to the element's edge.
            let gap = calls.call(|| range.Clone())?;
            calls.call(|| gap.MoveEndpointByRange(other, &document, endpoint))?;
            if !calls.call(|| gap.GetText(1))?.is_empty() {
                return Err(ReadFailure::NoTextSource);
            }
            onto_edge(&range, endpoint)?;
            // A caret outside leaves its other endpoint beyond the moved one:
            // collapse the range there rather than rely on the provider.
            if calls
                .call(|| range.CompareEndpoints(other, &range, endpoint))?
                .signum()
                == outside
            {
                calls.call(|| range.MoveEndpointByRange(other, &range, endpoint))?;
            }
        }
        // The caret (the selection's start), kept for its place in the text
        // read, and whether a selection replaces it.
        let caret = calls.call(|| range.Clone())?;
        let selection = calls.call(|| {
            range.CompareEndpoints(
                TextPatternRangeEndpoint_Start,
                &range,
                TextPatternRangeEndpoint_End,
            )
        })? != 0;
        // Kept at the caret for a line window.
        let lines = calls.call(|| range.Clone())?;
        // Move the start of `range` by `before` units and its end by `after`
        // (from the end of the caret's unit), clamped to the element's text,
        // and read at most `max` UTF-16 code units. Also says whether it spans
        // the element's whole text.
        let read_around = |range: &IUIAutomationTextRange, unit, before, after, max| {
            calls
                .call(|| range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, unit, before))?;
            calls.call(|| range.MoveEndpointByUnit(TextPatternRangeEndpoint_End, unit, after))?;
            let mut whole_field = true;
            for (endpoint, outside) in edges {
                let side = side_of(range, endpoint)?;
                if side.signum() == outside {
                    onto_edge(range, endpoint)?;
                }
                whole_field &= side == 0 || side.signum() == outside;
            }
            let text = calls.call(|| range.GetText(max))?;
            let too_long = text.len() >= max as usize;
            Ok::<_, ReadFailure>((too_long, whole_field, text.to_string()))
        };
        // Where the caret is in `text`, read from `range`: by the text from
        // the caret to the range's end, which `text` must end with.
        let caret_in = |range: &IUIAutomationTextRange, text: &str, max| {
            let tail = calls.call(|| range.Clone())?;
            calls.call(|| {
                tail.MoveEndpointByRange(
                    TextPatternRangeEndpoint_Start,
                    &caret,
                    TextPatternRangeEndpoint_Start,
                )
            })?;
            let tail = calls.call(|| tail.GetText(max))?.to_string();
            Ok::<_, ReadFailure>(text.ends_with(&tail).then(|| CaretAt {
                offset: text.len() - tail.len(),
                selection,
            }))
        };
        let (too_long, whole_field, text) =
            read_around(&range, TextUnit_Paragraph, -2, 1, MAX_READ_CHARS)?;
        if !needs_line_window(&text, too_long, whole_field) {
            let caret = caret_in(&range, &text, MAX_READ_CHARS)?;
            return Ok(FieldText {
                text,
                caret,
                truncated: false,
                source: TextSource::TextPattern,
            });
        }
        // The provider's paragraphs are not paragraphs (a terminal's whole
        // buffer) or hold more than a read keeps: read the lines around the
        // caret's, for the tracker to cut to the input at its prompt. Kept
        // only around a known caret.
        let (too_long, _, text) = read_around(
            &lines,
            TextUnit_Line,
            -(MAX_READ_LINES as i32 - 1),
            MAX_READ_LINES as i32 + 1,
            MAX_WINDOW_CHARS,
        )?;
        if too_long {
            return Err(ReadFailure::TooLarge);
        }
        let Some(caret) = caret_in(&lines, &text, MAX_WINDOW_CHARS)? else {
            return Err(ReadFailure::NoTextSource);
        };
        return Ok(FieldText {
            text,
            caret: Some(caret),
            truncated: false,
            source: TextSource::TextPatternLines,
        });
    }
    if value_allowed {
        let value = calls.call(|| element.GetCurrentPropertyValue(UIA_ValueValuePropertyId))?;
        let value = windows::core::BSTR::try_from(&value).map_err(|_| ReadFailure::NoTextSource)?;
        let truncated = value.len() >= MAX_READ_CHARS as usize;
        let value =
            String::from_utf16_lossy(&value.as_wide()[..value.len().min(MAX_READ_CHARS as usize)]);
        return Ok(FieldText {
            text: value,
            caret: None,
            truncated,
            source: TextSource::Value,
        });
    }
    Err(ReadFailure::NoTextSource)
}
