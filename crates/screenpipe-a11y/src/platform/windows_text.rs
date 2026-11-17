//! Entered-text reads: the focused field's text at the caret, through UI
//! Automation (see `docs/ENTERED_TEXT_CAPTURE.md`).
//!
//! The caller (`UiaContext::read_entered_text`) decides whether a read may
//! happen and confirms it afterwards; this module only fetches text from an
//! element the keyboard privacy probe has already verified, under a budget.

use std::cell::Cell;
use std::time::{Duration, Instant};

use windows::Win32::UI::Accessibility::{
    IUIAutomationElement, IUIAutomationTextPattern, TextPatternRangeEndpoint_End,
    TextPatternRangeEndpoint_Start, TextUnit_Paragraph, UIA_TextPatternId,
    UIA_ValueValuePropertyId, UIA_E_TIMEOUT,
};

use super::budget::{UiaLimits, UiaStop, WalkBudget};
use crate::entered_text::{ReadFailure, TextSource, MAX_READ_BYTES};

/// A read's total budget (up to twelve provider requests, then the two of the
/// focus recheck): under the probe's, because the keyboard privacy worker
/// probes nothing while it runs.
pub(super) const READ_BUDGET: Duration = Duration::from_millis(150);

/// Characters requested from a text range: one more than a read keeps, so a
/// longer text is known to be truncated.
const MAX_READ_CHARS: i32 = MAX_READ_BYTES as i32 + 1;

/// The text one read returned, before the tracker's bounds.
pub(super) struct FieldText {
    pub text: String,
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
/// range), and the range read is clamped to it. Some providers (Chromium,
/// WebView2, Electron) move range endpoints through the whole page, so the
/// clamp is what keeps a chat box's read out of the conversation around it.
///
/// The text runs from the start of the paragraph before the caret's (or the
/// selection's) to the end of the caret's (or the selection's last)
/// paragraph, within the element, at most [`MAX_READ_BYTES`] characters.
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
        // Compare one endpoint of `range` with the same endpoint of the
        // element's text: negative before it, positive after it.
        let compare =
            |endpoint| calls.call(|| range.CompareEndpoints(endpoint, &document, endpoint));
        if compare(TextPatternRangeEndpoint_Start)? < 0
            || compare(TextPatternRangeEndpoint_End)? > 0
        {
            return Err(ReadFailure::NoTextSource);
        }
        calls.call(|| {
            range.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Paragraph, -2)
        })?;
        calls.call(|| {
            range.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Paragraph, 1)
        })?;
        for (endpoint, outside) in [
            (TextPatternRangeEndpoint_Start, -1),
            (TextPatternRangeEndpoint_End, 1),
        ] {
            if compare(endpoint)?.signum() == outside {
                calls.call(|| range.MoveEndpointByRange(endpoint, &document, endpoint))?;
            }
        }
        let text = calls.call(|| range.GetText(MAX_READ_CHARS))?;
        let truncated = text.len() >= MAX_READ_CHARS as usize;
        return Ok(FieldText {
            text: text.to_string(),
            truncated,
            source: TextSource::TextPattern,
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
            truncated,
            source: TextSource::Value,
        });
    }
    Err(ReadFailure::NoTextSource)
}
