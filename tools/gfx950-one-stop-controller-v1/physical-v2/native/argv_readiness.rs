//! Bounded initial cmdline readiness only. No process/PID constructor or authority.
use super::setup_diagnostic::{SetupStage, SetupTrace};
use fe2o3_private_one_stop_protocol::Refusal;

pub(super) const ATTEMPTS: u8 = 16;
pub(super) const MAX_CMDLINE: usize = 2048;
pub(super) const CMDLINE_REQUESTED_BYTES: usize = ATTEMPTS as usize * (MAX_CMDLINE + 1);

pub(super) trait Source {
    fn clock(&mut self) -> Result<(), Refusal>;
    fn identity(&mut self) -> Result<(), Refusal>;
    fn read_cmdline(&mut self) -> Result<Vec<u8>, Refusal>;
    fn yield_once(&mut self);
}

fn guard(
    source: &mut impl Source,
    trace: &mut SetupTrace,
    stage: SetupStage,
) -> Result<(), Refusal> {
    trace.step(stage, || source.clock())?;
    trace.readiness_identity();
    trace.step(stage, || source.identity())?;
    trace.step(stage, || source.clock())
}

pub(super) fn initial(
    source: &mut impl Source,
    expected: &[u8],
    trace: &mut SetupTrace,
) -> Result<(), Refusal> {
    if expected.len() > MAX_CMDLINE {
        return Err(Refusal::Bound);
    }
    if expected.is_empty() || expected.last() != Some(&0) {
        return Err(Refusal::Shape);
    }
    let mut requested = 0usize;
    for attempt in 0..ATTEMPTS {
        guard(source, trace, SetupStage::CmdlineReadyBefore)?;
        requested = requested
            .checked_add(MAX_CMDLINE + 1)
            .ok_or(Refusal::Bound)?;
        if requested > CMDLINE_REQUESTED_BYTES {
            return Err(Refusal::Bound);
        }
        trace.readiness_attempt();
        let raw = trace.step(SetupStage::Cmdline, || source.read_cmdline())?;
        let same = raw == expected;
        trace.cmdline(raw.len(), same);
        if raw.is_empty() {
            trace.readiness_empty();
        }
        guard(source, trace, SetupStage::CmdlineReadyAfter)?;
        if raw.len() > MAX_CMDLINE {
            return trace.step(SetupStage::Cmdline, || Err(Refusal::Bound));
        }
        if same {
            return Ok(());
        }
        if !raw.is_empty() {
            return trace.step(SetupStage::Cmdline, || Err(Refusal::Changed));
        }
        drop(raw);
        if attempt + 1 == ATTEMPTS {
            return trace.step(SetupStage::CmdlineReadyLimit, || Err(Refusal::Incomplete));
        }
        trace.step(SetupStage::CmdlineReadyYield, || source.clock())?;
        trace.readiness_yield();
        source.yield_once();
        trace.step(SetupStage::CmdlineReadyYield, || source.clock())?;
    }
    Err(Refusal::Incomplete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    const EXACT: &[u8] = b"fixed-debugger\0--interpreter=mi2\0";
    struct Fake {
        reads: VecDeque<Result<Vec<u8>, Refusal>>,
        events: Vec<char>,
        clocks: usize,
        identities: usize,
        read_calls: usize,
        yields: usize,
        fail_clock: Option<usize>,
        fail_identity: Option<(usize, Refusal)>,
    }
    impl Fake {
        fn new(values: impl IntoIterator<Item = Result<Vec<u8>, Refusal>>) -> Self {
            Self {
                reads: values.into_iter().collect(),
                events: Vec::new(),
                clocks: 0,
                identities: 0,
                read_calls: 0,
                yields: 0,
                fail_clock: None,
                fail_identity: None,
            }
        }
    }
    impl Source for Fake {
        fn clock(&mut self) -> Result<(), Refusal> {
            self.events.push('C');
            self.clocks += 1;
            if self.fail_clock == Some(self.clocks) {
                Err(Refusal::Deadline)
            } else {
                Ok(())
            }
        }
        fn identity(&mut self) -> Result<(), Refusal> {
            self.events.push('I');
            self.identities += 1;
            match self.fail_identity {
                Some((at, e)) if at == self.identities => Err(e),
                _ => Ok(()),
            }
        }
        fn read_cmdline(&mut self) -> Result<Vec<u8>, Refusal> {
            self.events.push('R');
            self.read_calls += 1;
            self.reads.pop_front().unwrap_or(Err(Refusal::Incomplete))
        }
        fn yield_once(&mut self) {
            self.events.push('Y');
            self.yields += 1;
        }
    }
    fn run(f: &mut Fake) -> (Result<(), Refusal>, String) {
        let mut trace = SetupTrace::new();
        let result = initial(f, EXACT, &mut trace);
        (result, trace.freeze(0).to_string())
    }
    fn empty() -> Result<Vec<u8>, Refusal> {
        Ok(Vec::new())
    }
    fn exact() -> Result<Vec<u8>, Refusal> {
        Ok(EXACT.to_vec())
    }

    #[test]
    fn initial_exact_has_two_complete_guards_and_no_yield() {
        let mut f = Fake::new([exact()]);
        let (r, t) = run(&mut f);
        assert_eq!(r, Ok(()));
        assert_eq!(f.events, ['C', 'I', 'C', 'R', 'C', 'I', 'C']);
        assert_eq!(
            (f.read_calls, f.identities, f.clocks, f.yields),
            (1, 2, 4, 0)
        );
        assert!(t.contains(
            "cmdline_attempts=1,cmdline_empty=0,cmdline_identity_checks=2,cmdline_yields=0"
        ));
    }
    #[test]
    fn empty_then_exact_is_same_source_with_no_relaunch_interface() {
        let mut f = Fake::new([empty(), exact()]);
        let (r, t) = run(&mut f);
        assert_eq!(r, Ok(()));
        assert_eq!(
            (f.read_calls, f.identities, f.clocks, f.yields),
            (2, 4, 10, 1)
        );
        assert_eq!(
            f.events,
            [
                'C', 'I', 'C', 'R', 'C', 'I', 'C', 'C', 'Y', 'C', 'C', 'I', 'C', 'R', 'C', 'I', 'C'
            ]
        );
        assert!(t.contains(
            "cmdline_attempts=2,cmdline_empty=1,cmdline_identity_checks=4,cmdline_yields=1"
        ));
        assert!(t.contains("sent_commands=0"));
    }
    #[test]
    fn all_empty_exhausts_sixteen_without_seventeenth_read_or_last_yield() {
        let mut f = Fake::new((0..17).map(|_| empty()));
        let (r, t) = run(&mut f);
        assert_eq!(r, Err(Refusal::Incomplete));
        assert_eq!(
            (f.read_calls, f.identities, f.clocks, f.yields),
            (16, 32, 94, 15)
        );
        assert_eq!(f.reads.len(), 1);
        assert!(t.contains("stage=CmdlineReadyLimit"));
        assert!(t.contains(
            "cmdline_attempts=16,cmdline_empty=16,cmdline_identity_checks=32,cmdline_yields=15"
        ));
        assert!(t.contains("cmdline_equal=Some(false)"));
    }
    #[test]
    fn exact_on_last_permitted_read_is_checked_afterward() {
        let mut f = Fake::new((0..15).map(|_| empty()).chain([exact(), exact()]));
        assert_eq!(run(&mut f).0, Ok(()));
        assert_eq!(
            (f.read_calls, f.identities, f.clocks, f.yields),
            (16, 32, 94, 15)
        );
        assert_eq!(f.reads.len(), 1);
        assert_eq!(f.events.last(), Some(&'C'));
    }
    #[test]
    fn prefix_or_any_nonempty_mismatch_is_immediate_refusal() {
        for raw in [
            EXACT[..EXACT.len() - 1].to_vec(),
            b"wrong\0".to_vec(),
            [EXACT, b"extra\0"].concat(),
        ] {
            let mut f = Fake::new([Ok(raw), exact()]);
            let (r, t) = run(&mut f);
            assert_eq!(r, Err(Refusal::Changed));
            assert!(t.starts_with("stage=Cmdline,"));
            assert_eq!((f.read_calls, f.identities, f.yields), (1, 2, 0));
            assert_eq!(f.reads.len(), 1);
        }
    }
    #[test]
    fn nonempty_mismatch_after_empty_does_not_sample_again() {
        let mut f = Fake::new([empty(), Ok(b"changed\0".to_vec()), exact()]);
        assert_eq!(run(&mut f).0, Err(Refusal::Changed));
        assert_eq!((f.read_calls, f.yields), (2, 1));
        assert_eq!(f.reads.len(), 1);
    }
    #[test]
    fn all_read_refusals_preserve_error_and_skip_yield_and_next_read() {
        for error in [
            Refusal::Process,
            Refusal::Exit,
            Refusal::Changed,
            Refusal::Bound,
            Refusal::Incomplete,
            Refusal::Deadline,
        ] {
            let mut f = Fake::new([Err(error), exact()]);
            let (r, t) = run(&mut f);
            assert_eq!(r, Err(error));
            assert_eq!((f.read_calls, f.identities, f.yields), (1, 1, 0));
            assert!(t.contains("cmdline_attempts=1"));
            assert!(t.contains("cmdline_bytes=None"));
        }
    }
    #[test]
    fn identity_failure_before_read_never_reads_or_yields() {
        for error in [
            Refusal::Changed,
            Refusal::Exit,
            Refusal::Process,
            Refusal::Deadline,
        ] {
            let mut f = Fake::new([exact()]);
            f.fail_identity = Some((1, error));
            assert_eq!(run(&mut f).0, Err(error));
            assert_eq!((f.read_calls, f.yields), (0, 0));
        }
    }
    #[test]
    fn identity_failure_after_exact_never_accepts() {
        for error in [
            Refusal::Changed,
            Refusal::Exit,
            Refusal::Process,
            Refusal::Deadline,
        ] {
            let mut f = Fake::new([exact()]);
            f.fail_identity = Some((2, error));
            assert_eq!(run(&mut f).0, Err(error));
            assert_eq!((f.read_calls, f.yields), (1, 0));
        }
    }
    #[test]
    fn identity_failure_after_empty_never_yields_or_reads_again() {
        let mut f = Fake::new([empty(), exact()]);
        f.fail_identity = Some((2, Refusal::Changed));
        assert_eq!(run(&mut f).0, Err(Refusal::Changed));
        assert_eq!((f.read_calls, f.yields), (1, 0));
    }
    #[test]
    fn every_clock_position_in_empty_exact_sequence_refuses_without_reset() {
        for at in 1..=10 {
            let mut f = Fake::new([empty(), exact()]);
            f.fail_clock = Some(at);
            assert_eq!(run(&mut f).0, Err(Refusal::Deadline));
            assert_eq!(f.clocks, at);
            assert!(f.read_calls <= 2 && f.yields <= 1);
        }
    }
    #[test]
    fn exact_after_original_clock_expires_is_not_success() {
        for at in [3, 4] {
            let mut f = Fake::new([exact()]);
            f.fail_clock = Some(at);
            assert_eq!(run(&mut f).0, Err(Refusal::Deadline));
            assert_eq!(f.read_calls, 1);
            assert_eq!(f.yields, 0);
        }
    }
    #[test]
    fn expired_clock_before_initial_guard_does_not_observe() {
        let mut f = Fake::new([exact()]);
        f.fail_clock = Some(1);
        assert_eq!(run(&mut f).0, Err(Refusal::Deadline));
        assert_eq!((f.identities, f.read_calls, f.yields), (0, 0, 0));
    }
    #[test]
    fn every_identity_guard_position_can_refuse_without_fallback() {
        for at in 1..=4 {
            let mut f = Fake::new([empty(), exact()]);
            f.fail_identity = Some((at, Refusal::Changed));
            assert_eq!(run(&mut f).0, Err(Refusal::Changed));
            assert_eq!(f.identities, at);
        }
    }
    #[test]
    fn oversized_observation_is_bound_not_a_prefix_match() {
        let mut f = Fake::new([Ok(vec![0; MAX_CMDLINE + 1]), exact()]);
        let (r, t) = run(&mut f);
        assert_eq!(r, Err(Refusal::Bound));
        assert!(t.starts_with("stage=Cmdline,"));
        assert_eq!((f.read_calls, f.identities, f.yields), (1, 2, 0));
    }
    #[test]
    fn exact_boundary_bytes_can_match_only_in_full() {
        let expected = vec![0; MAX_CMDLINE];
        let mut f = Fake::new([Ok(expected.clone())]);
        assert_eq!(initial(&mut f, &expected, &mut SetupTrace::new()), Ok(()));
        assert_eq!((f.read_calls, f.identities), (1, 2));
        assert_eq!(CMDLINE_REQUESTED_BYTES, 32784);
    }
    #[test]
    fn invalid_expected_shape_or_size_fails_before_observations() {
        for (expected, error) in [
            (Vec::new(), Refusal::Shape),
            (b"not-nul".to_vec(), Refusal::Shape),
            (vec![0; MAX_CMDLINE + 1], Refusal::Bound),
        ] {
            let mut f = Fake::new([exact()]);
            assert_eq!(
                initial(&mut f, &expected, &mut SetupTrace::new()),
                Err(error)
            );
            assert!(f.events.is_empty());
        }
    }
}
