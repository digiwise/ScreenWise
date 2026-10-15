//! Provider-independent UIA admission and traversal. Tests never touch COM or the desktop.

use crate::events::AccessibilityNode;
use std::cell::Cell;
use std::time::{Duration, Instant};

pub(crate) const PROVIDER_TIMEOUT_MS: u32 = 50;
const MAX_NODES: usize = 5_000;
const MAX_DEPTH: usize = 35;
const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_PROPERTY_BYTES: usize = 16 * 1024;
const MAX_WALK_TIME: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
pub(crate) struct UiaLimits {
    pub timeout: Duration,
    pub nodes: usize,
    pub depth: usize,
    pub text_bytes: usize,
}

impl UiaLimits {
    pub fn clamped(self) -> Self {
        Self {
            timeout: self.timeout.min(MAX_WALK_TIME),
            nodes: self.nodes.min(MAX_NODES),
            depth: self.depth.min(MAX_DEPTH),
            text_bytes: self.text_bytes.min(MAX_TEXT_BYTES),
        }
    }
    /// The focused-state probe has its own total budget for three provider
    /// requests; native per-request waits do not define evidence freshness.
    pub fn keyboard_privacy() -> Self {
        Self::tree(1)
    }

    pub fn tree(nodes: usize) -> Self {
        Self {
            timeout: MAX_WALK_TIME,
            nodes,
            depth: MAX_DEPTH,
            text_bytes: MAX_TEXT_BYTES,
        }
    }
}

pub(crate) use screenpipe_config::capture_diagnostics::AccessibilityStopReason as UiaStop;

#[derive(Default)]
pub(crate) struct ProviderBackoff(Cell<Option<Instant>>);

impl ProviderBackoff {
    pub fn ready(&self) -> bool {
        self.ready_at(Instant::now())
    }
    fn ready_at(&self, now: Instant) -> bool {
        !self.0.get().is_some_and(|until| now < until)
    }
    pub fn failed(&self) {
        self.0.set(Some(Instant::now() + Duration::from_secs(1)));
    }
}

pub(crate) struct WalkBudget {
    start: Instant,
    limits: UiaLimits,
    nodes: usize,
    text_bytes: usize,
    stop: Option<UiaStop>,
    deepest_attempted: Cell<usize>,
    #[cfg(test)]
    clock: Option<std::rc::Rc<Cell<Instant>>>,
}

impl WalkBudget {
    pub fn new(limits: UiaLimits) -> Self {
        Self {
            start: Instant::now(),
            limits: limits.clamped(),
            nodes: 0,
            text_bytes: 0,
            stop: None,
            deepest_attempted: Cell::new(0),
            #[cfg(test)]
            clock: None,
        }
    }

    fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(clock) = &self.clock {
            return clock.get();
        }
        Instant::now()
    }

    /// Local decoding and completion need no reservation for another request.
    pub fn check_complete(&self) -> Result<(), UiaStop> {
        self.check_complete_at(self.now())
    }

    fn check_complete_at(&self, now: Instant) -> Result<(), UiaStop> {
        if let Some(stop) = self.stop {
            return Err(stop);
        }
        if now.duration_since(self.start) >= self.limits.timeout {
            return Err(UiaStop::Timeout);
        }
        Ok(())
    }

    /// Start requests only before the actual deadline. Native timeout setters
    /// require at least 50 ms, so a final in-flight request may return late.
    /// Every response is deadline-checked and late trees are discarded.
    pub fn request_timeout(&self) -> Result<u32, UiaStop> {
        self.request_timeout_at(self.now())
    }

    fn request_timeout_at(&self, now: Instant) -> Result<u32, UiaStop> {
        self.check_complete_at(now)?;
        // A valid native wait is not a reservation: fast providers can finish
        // within the remaining budget even when it is less than 50 ms.
        Ok(PROVIDER_TIMEOUT_MS)
    }

    fn admit_node(&self, depth: usize) -> Result<(), UiaStop> {
        self.deepest_attempted
            .set(self.deepest_attempted.get().max(depth));
        self.check_complete()?;
        if self.nodes >= self.limits.nodes {
            return Err(UiaStop::Nodes);
        }
        if depth > self.limits.depth {
            return Err(UiaStop::Depth);
        }
        Ok(())
    }

    /// Decode only a bounded prefix of borrowed UTF-16, without duplicating the
    /// entire BSTR. Provider marshalling itself is outside this allocation limit.
    pub fn string(&mut self, utf16: &[u16]) -> Option<String> {
        if utf16.is_empty() || self.stop.is_some() {
            return None;
        }
        let remaining = self.limits.text_bytes.saturating_sub(self.text_bytes);
        let available = remaining.min(MAX_PROPERTY_BYTES);
        let mut text = String::with_capacity(available.min(utf16.len()));
        for decoded in char::decode_utf16(utf16.iter().copied()) {
            let ch = decoded.unwrap_or(char::REPLACEMENT_CHARACTER);
            if text.len() + ch.len_utf8() > available {
                self.stop = Some(if remaining < MAX_PROPERTY_BYTES {
                    UiaStop::AggregateText
                } else {
                    UiaStop::PropertyText
                });
                break;
            }
            text.push(ch);
        }
        self.text_bytes += text.len();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

/// Each navigation method fetches exactly one element and its properties.
pub(crate) trait TreeProvider {
    type Element;
    fn root(&self, timeout_ms: u32) -> Result<Self::Element, UiaStop>;
    fn node(&self, element: &Self::Element, budget: &mut WalkBudget) -> AccessibilityNode;
    fn first_child(
        &self,
        element: &Self::Element,
        timeout_ms: u32,
    ) -> Result<Option<Self::Element>, UiaStop>;
    fn next_sibling(
        &self,
        element: &Self::Element,
        timeout_ms: u32,
    ) -> Result<Option<Self::Element>, UiaStop>;
}

pub(crate) fn walk(
    provider: &impl TreeProvider,
    limits: UiaLimits,
) -> Result<AccessibilityNode, UiaStop> {
    walk_with_budget(provider, WalkBudget::new(limits))
}

fn walk_with_budget(
    provider: &impl TreeProvider,
    mut budget: WalkBudget,
) -> Result<AccessibilityNode, UiaStop> {
    let result = (|| {
        budget.admit_node(0)?;
        let root = provider.root(budget.request_timeout()?)?;
        budget.check_complete()?;
        visit(provider, &root, 0, &mut budget)
    })();
    if let Err(reason) = result {
        screenpipe_config::sensitive_debug::record("uia_walk_progress", || {
            serde_json::json!({"reason":reason.code(),"visited_nodes":budget.nodes,
                "decoded_text_bytes":budget.text_bytes,"deepest_attempted":budget.deepest_attempted.get(),
                "elapsed_ms":budget.now().duration_since(budget.start).as_millis()})
        });
    }
    result
}

fn visit<P: TreeProvider>(
    provider: &P,
    element: &P::Element,
    depth: usize,
    budget: &mut WalkBudget,
) -> Result<AccessibilityNode, UiaStop> {
    budget.admit_node(depth)?;
    budget.nodes += 1;
    let mut node = provider.node(element, budget);
    // A limit is conservative: don't make one more provider call just to prove
    // that a node at the boundary has no children/siblings. Incomplete trees
    // must not authorize a frame whose private subtree/URL wasn't inspected.
    budget.admit_node(depth + 1)?;
    let mut child = provider.first_child(element, budget.request_timeout()?)?;
    budget.check_complete()?;
    while let Some(current) = child {
        node.children
            .push(visit(provider, &current, depth + 1, budget)?);
        budget.admit_node(depth + 1)?;
        child = provider.next_sibling(&current, budget.request_timeout()?)?;
        budget.check_complete()?;
    }
    budget.check_complete()?;
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fake {
        calls: RefCell<Vec<u32>>,
        // An infinite cyclic provider catches missing depth/node limits.
        child: bool,
        sibling: bool,
        fail: bool,
        expire: Cell<bool>,
        text: Vec<u16>,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                child: false,
                sibling: false,
                fail: false,
                expire: Cell::new(false),
                text: Vec::new(),
            }
        }
        fn request(&self, ms: u32) -> Result<(), UiaStop> {
            self.calls.borrow_mut().push(ms);
            if self.fail {
                Err(UiaStop::Provider)
            } else {
                Ok(())
            }
        }
    }
    impl TreeProvider for Fake {
        type Element = ();
        fn root(&self, ms: u32) -> Result<(), UiaStop> {
            self.request(ms)
        }
        fn node(&self, _: &(), budget: &mut WalkBudget) -> AccessibilityNode {
            let mut node = AccessibilityNode {
                control_type: "Edit".into(),
                ..Default::default()
            };
            node.name = budget.string(&self.text);
            if self.expire.get() {
                budget.start = Instant::now() - Duration::from_secs(1);
            }
            node
        }
        fn first_child(&self, _: &(), ms: u32) -> Result<Option<()>, UiaStop> {
            self.request(ms)?;
            Ok(self.child.then_some(()))
        }
        fn next_sibling(&self, _: &(), ms: u32) -> Result<Option<()>, UiaStop> {
            self.request(ms)?;
            Ok(self.sibling.then_some(()))
        }
    }
    #[test]
    fn complete_small_tree_and_short_provider_waits() {
        let provider = Fake::new();
        assert_eq!(
            walk(&provider, UiaLimits::tree(10)).unwrap().node_count(),
            1
        );
        assert_eq!(*provider.calls.borrow(), vec![50, 50]);
    }
    #[test]
    fn zero_budgets_do_not_contact_provider() {
        for limits in [
            UiaLimits::tree(0),
            UiaLimits {
                timeout: Duration::ZERO,
                ..UiaLimits::tree(10)
            },
        ] {
            let provider = Fake::new();
            assert!(walk(&provider, limits).is_err());
            assert!(provider.calls.borrow().is_empty());
        }
    }
    #[test]
    fn depth_stops_cyclic_provider_before_another_request() {
        let provider = Fake {
            child: true,
            ..Fake::new()
        };
        assert_eq!(
            walk(
                &provider,
                UiaLimits {
                    depth: 3,
                    ..UiaLimits::tree(100)
                }
            )
            .unwrap_err(),
            UiaStop::Depth
        );
        assert_eq!(provider.calls.borrow().len(), 4);
    }
    #[test]
    fn nodes_stop_cyclic_provider_before_another_request() {
        let provider = Fake {
            child: true,
            ..Fake::new()
        };
        assert_eq!(
            walk(&provider, UiaLimits::tree(3)).unwrap_err(),
            UiaStop::Nodes
        );
        assert_eq!(provider.calls.borrow().len(), 3);
    }
    #[test]
    fn deadline_stops_navigation_after_a_request_returns() {
        let provider = Fake {
            expire: Cell::new(true),
            ..Fake::new()
        };
        assert_eq!(
            walk(&provider, UiaLimits::tree(10)).unwrap_err(),
            UiaStop::Timeout
        );
        assert_eq!(provider.calls.borrow().len(), 1);
    }
    #[test]
    fn provider_failure_is_not_retried_or_returned_as_complete() {
        let provider = Fake {
            fail: true,
            ..Fake::new()
        };
        assert_eq!(
            walk(&provider, UiaLimits::tree(10)).unwrap_err(),
            UiaStop::Provider
        );
        assert_eq!(provider.calls.borrow().len(), 1);
    }
    #[test]
    fn large_multiline_json_stops_without_more_provider_requests() {
        let provider = Fake {
            text: ("{\n  \"value\": \"synthetic\"\n}\n".repeat(10_000))
                .encode_utf16()
                .collect(),
            ..Fake::new()
        };
        assert_eq!(
            walk(&provider, UiaLimits::tree(100)).unwrap_err(),
            UiaStop::PropertyText
        );
        assert_eq!(provider.calls.borrow().len(), 1);
    }
    #[test]
    fn unicode_and_aggregate_text_memory_are_bounded() {
        let mut budget = WalkBudget::new(UiaLimits {
            text_bytes: 9,
            ..UiaLimits::tree(10)
        });
        assert_eq!(
            budget
                .string(&"😀é".encode_utf16().collect::<Vec<_>>())
                .unwrap(),
            "😀é"
        );
        assert_eq!(
            budget
                .string(&"éé".encode_utf16().collect::<Vec<_>>())
                .unwrap(),
            "é"
        );
        assert_eq!(budget.text_bytes, 8);
        assert_eq!(budget.request_timeout(), Err(UiaStop::AggregateText));
    }
    #[test]
    fn completion_uses_deadline_without_reserving_a_new_provider_request() {
        let mut budget = WalkBudget::new(UiaLimits::tree(10));
        let now = Instant::now();
        budget.start = now - Duration::from_millis(225);
        assert_eq!(budget.request_timeout_at(now), Ok(PROVIDER_TIMEOUT_MS));
        assert_eq!(budget.check_complete_at(now), Ok(()));
        assert_eq!(
            budget.check_complete_at(budget.start + Duration::from_millis(250)),
            Err(UiaStop::Timeout)
        );
        budget.stop = Some(UiaStop::PropertyText);
        assert_eq!(budget.check_complete_at(now), Err(UiaStop::PropertyText));
    }

    #[test]
    fn focused_privacy_budget_admits_three_minimum_native_waits() {
        let mut budget = WalkBudget::new(UiaLimits::keyboard_privacy());
        let now = Instant::now();
        for elapsed in [1, 51, 101] {
            budget.start = now - Duration::from_millis(elapsed);
            assert_eq!(budget.request_timeout_at(now), Ok(PROVIDER_TIMEOUT_MS));
        }
        budget.start = now - Duration::from_millis(151);
        assert_eq!(budget.check_complete_at(now), Ok(()));
    }

    #[test]
    fn positive_remaining_time_allows_valid_native_wait_until_actual_deadline() {
        let budget = WalkBudget::new(UiaLimits::tree(10));
        for millis in [1, 5, 25, 49, 249] {
            assert_eq!(
                budget.request_timeout_at(budget.start + Duration::from_millis(millis)),
                Ok(PROVIDER_TIMEOUT_MS)
            );
        }
        for millis in [250, 251] {
            assert_eq!(
                budget.request_timeout_at(budget.start + Duration::from_millis(millis)),
                Err(UiaStop::Timeout)
            );
        }
    }

    struct Timed {
        clock: std::rc::Rc<Cell<Instant>>,
        requests: RefCell<std::collections::VecDeque<(&'static str, u64)>>,
        decoded: Cell<usize>,
        one_child: bool,
    }

    impl Timed {
        fn request(&self, stage: &'static str, timeout_ms: u32) {
            assert_eq!(timeout_ms, PROVIDER_TIMEOUT_MS);
            let (expected, millis) = self.requests.borrow_mut().pop_front().unwrap();
            assert_eq!(stage, expected);
            self.clock
                .set(self.clock.get() + Duration::from_millis(millis));
        }

        fn run(
            steps: &[(&'static str, u64)],
            one_child: bool,
        ) -> (Result<AccessibilityNode, UiaStop>, usize) {
            let start = Instant::now();
            let clock = std::rc::Rc::new(Cell::new(start));
            let provider = Self {
                clock: clock.clone(),
                requests: RefCell::new(steps.iter().copied().collect()),
                decoded: Cell::new(0),
                one_child,
            };
            let budget = WalkBudget {
                start,
                clock: Some(clock),
                ..WalkBudget::new(UiaLimits::tree(10))
            };
            let result = walk_with_budget(&provider, budget);
            assert!(provider.requests.borrow().is_empty());
            (result, provider.decoded.get())
        }
    }

    impl TreeProvider for Timed {
        type Element = usize;
        fn root(&self, ms: u32) -> Result<usize, UiaStop> {
            self.request("root", ms);
            Ok(0)
        }
        fn node(&self, _: &usize, _: &mut WalkBudget) -> AccessibilityNode {
            self.decoded.set(self.decoded.get() + 1);
            AccessibilityNode::default()
        }
        fn first_child(&self, element: &usize, ms: u32) -> Result<Option<usize>, UiaStop> {
            self.request("child", ms);
            Ok((self.one_child && *element == 0).then_some(1))
        }
        fn next_sibling(&self, _: &usize, ms: u32) -> Result<Option<usize>, UiaStop> {
            self.request("sibling", ms);
            Ok(None)
        }
    }

    #[test]
    fn complete_provider_response_at_225_ms_is_admitted() {
        let (result, decoded) = Timed::run(&[("root", 0), ("child", 225)], false);
        assert_eq!(result.unwrap().node_count(), 1);
        assert_eq!(decoded, 1);
    }

    #[test]
    fn request_at_240_ms_can_complete_inside_actual_deadline() {
        let (result, decoded) = Timed::run(&[("root", 240), ("child", 5)], false);
        assert_eq!(result.unwrap().node_count(), 1);
        assert_eq!(decoded, 1);
    }

    #[test]
    fn late_root_response_is_never_decoded() {
        for millis in [250, 260] {
            let (result, decoded) = Timed::run(&[("root", millis)], false);
            assert_eq!(result.unwrap_err(), UiaStop::Timeout);
            assert_eq!(decoded, 0);
        }
    }

    #[test]
    fn late_null_child_response_discards_partial_tree() {
        let (result, decoded) = Timed::run(&[("root", 240), ("child", 20)], false);
        assert_eq!(result.unwrap_err(), UiaStop::Timeout);
        assert_eq!(decoded, 1);
    }

    #[test]
    fn late_null_sibling_response_discards_partial_tree() {
        let (result, decoded) = Timed::run(
            &[("root", 0), ("child", 0), ("child", 240), ("sibling", 20)],
            true,
        );
        assert_eq!(result.unwrap_err(), UiaStop::Timeout);
        assert_eq!(decoded, 2);
    }
    #[test]
    fn failed_provider_is_deferred_without_waiting_or_spawning_work() {
        let gate = ProviderBackoff::default();
        assert!(gate.ready());
        gate.failed();
        assert!(!gate.ready());
        let deadline = gate.0.get().unwrap();
        assert!(!gate.ready_at(deadline - Duration::from_nanos(1)));
        assert!(gate.ready_at(deadline));
    }

    // A flat, endlessly expanding editor tree must obey the same node budget
    // as a deep/cyclic provider, without relying on recursion to stop it.
    struct Wide {
        requests: Cell<usize>,
        fail_at: Option<usize>,
    }
    impl Wide {
        fn request(&self) -> Result<(), UiaStop> {
            let count = self.requests.get() + 1;
            self.requests.set(count);
            if self.fail_at == Some(count) {
                Err(UiaStop::Provider)
            } else {
                Ok(())
            }
        }
    }
    impl TreeProvider for Wide {
        type Element = usize;
        fn root(&self, _: u32) -> Result<usize, UiaStop> {
            self.request()?;
            Ok(0)
        }
        fn node(&self, _: &usize, _: &mut WalkBudget) -> AccessibilityNode {
            AccessibilityNode::default()
        }
        fn first_child(&self, element: &usize, _: u32) -> Result<Option<usize>, UiaStop> {
            self.request()?;
            Ok((*element == 0).then_some(1))
        }
        fn next_sibling(&self, element: &usize, _: u32) -> Result<Option<usize>, UiaStop> {
            self.request()?;
            Ok(Some(element + 1))
        }
    }
    #[test]
    fn wide_editor_tree_stops_before_requesting_more_nodes() {
        let provider = Wide {
            requests: Cell::new(0),
            fail_at: None,
        };
        assert_eq!(
            walk(&provider, UiaLimits::tree(5)).unwrap_err(),
            UiaStop::Nodes
        );
        assert_eq!(provider.requests.get(), 8);
    }
    #[test]
    fn late_provider_failure_discards_the_partial_tree() {
        let provider = Wide {
            requests: Cell::new(0),
            fail_at: Some(4),
        };
        assert_eq!(
            walk(&provider, UiaLimits::tree(100)).unwrap_err(),
            UiaStop::Provider
        );
        assert_eq!(provider.requests.get(), 4);
    }
}
