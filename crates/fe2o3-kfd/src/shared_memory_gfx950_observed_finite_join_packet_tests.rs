use super::*;
use std::collections::VecDeque;

fn prime() -> Snapshot {
    Snapshot {
        write: 0,
        read: 0,
        slot: 0,
        header: 1,
        setup: 0,
        kind: 1,
        value: 1,
    }
}
fn done() -> Snapshot {
    Snapshot {
        write: 1,
        read: 0,
        slot: 0,
        header: AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1,
        setup: 1,
        kind: 1,
        value: 0,
    }
}
fn packet() -> AqlPreparedKernelDispatchV1 {
    AqlKernelDispatchPacketV1::new_unpublished(
        geometry().unwrap(),
        0,
        0,
        ObservedGpuAddressV1::new(0x10000).unwrap(),
        ObservedGpuAddressV1::new(0x20000).unwrap(),
        8,
        ObservedGpuAddressV1::new(0x30000).unwrap(),
    )
    .unwrap()
}
struct Fake {
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
    snapshots: VecDeque<Snapshot>,
    write: u64,
    expired: bool,
    pause_expires: bool,
    body: Option<[u8; 64]>,
    header: Option<u16>,
    doorbells: usize,
}
impl Fake {
    fn new() -> Self {
        Self {
            calls: Vec::new(),
            fail: None,
            snapshots: VecDeque::from([prime(), done()]),
            write: 0,
            expired: false,
            pause_expires: false,
            body: None,
            header: None,
            doorbells: 0,
        }
    }
    fn tick(&mut self, name: &'static str) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.calls.push(name);
        if self.fail == Some(name) {
            Err(NativeAqlSubmissionErrorV1::Currentness)
        } else {
            Ok(())
        }
    }
}
impl NativeAqlSubmissionBackendV1 for Fake {
    fn check_currentness(&mut self) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.tick("currentness")
    }
    fn observe_counters_acquire(&mut self) -> Result<(u64, u64), NativeAqlSubmissionErrorV1> {
        self.tick("counters")?;
        Ok((self.write, 0))
    }
    fn fetch_add_write_acq_rel(&mut self, n: u64) -> Result<u64, NativeAqlSubmissionErrorV1> {
        self.tick("reserve")?;
        assert_eq!(n, 1);
        let old = self.write;
        self.write += n;
        Ok(old)
    }
    fn write_unpublished(
        &mut self,
        slot: u32,
        b: &[u8; 64],
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.tick("body")?;
        assert_eq!(slot, 0);
        assert!(fixed_body(b));
        self.body = Some(*b);
        Ok(())
    }
    fn publish_release_header(
        &mut self,
        slot: u32,
        h: u16,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.tick("header")?;
        assert_eq!(slot, 0);
        assert_eq!(h, AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1);
        assert!(self.body.is_some());
        self.header = Some(h);
        Ok(())
    }
    fn ring_doorbell_release(&mut self, id: u64) -> Result<(), NativeAqlSubmissionErrorV1> {
        self.tick("doorbell")?;
        assert_eq!(id, 0);
        assert!(self.header.is_some());
        self.doorbells += 1;
        Ok(())
    }
}
impl KernelBackend for Fake {
    fn snapshot(&mut self) -> QueueResult<Snapshot> {
        self.tick("snapshot").map_err(describe)?;
        self.snapshots
            .pop_front()
            .ok_or("missing fake snapshot".into())
    }
    fn expired(&self) -> bool {
        self.expired
    }
    fn pause(&mut self) {
        self.calls.push("pause");
        if self.pause_expires {
            self.expired = true;
        }
    }
}

#[test]
fn actual_single_packet_publisher_preserves_body_header_doorbell_order() {
    let mut f = Fake::new();
    assert_eq!(complete_one_kernel(&mut f, packet()).unwrap(), done());
    assert_eq!(f.write, 1);
    assert_eq!(f.doorbells, 1);
    let names: Vec<_> = f
        .calls
        .iter()
        .copied()
        .filter(|s| matches!(*s, "reserve" | "body" | "header" | "doorbell"))
        .collect();
    assert_eq!(names, ["reserve", "body", "header", "doorbell"]);
    let b = f.body.unwrap();
    assert_eq!(u64::from_le_bytes(b[32..40].try_into().unwrap()), 0x10000);
    for i in [0, 2, 4, 12, 24, 28, 48] {
        let mut bad = b;
        bad[i] ^= 1;
        assert!(!fixed_body(&bad));
    }
}

#[test]
fn publisher_error_never_continues_to_later_side_effects() {
    for fail in [
        "currentness",
        "snapshot",
        "counters",
        "reserve",
        "body",
        "header",
        "doorbell",
    ] {
        let mut f = Fake::new();
        f.fail = Some(fail);
        assert!(complete_one_kernel(&mut f, packet()).is_err());
        assert_eq!(f.calls.last(), Some(&fail));
        assert_eq!(f.doorbells, 0);
    }
}

#[test]
fn malformed_primed_or_completed_snapshots_refuse() {
    for phase in 0..2 {
        for field in 0..7 {
            let mut f = Fake::new();
            let s = &mut f.snapshots[phase];
            match field {
                0 => s.write = 2,
                1 => s.read = 2,
                2 => s.slot = 1,
                3 => s.header = 0x1403,
                4 => s.setup = 3,
                5 => s.kind = 2,
                6 => s.value = -1,
                _ => unreachable!(),
            }
            assert!(complete_one_kernel(&mut f, packet()).is_err());
            if phase == 0 {
                assert_eq!(f.write, 0);
            }
        }
    }
}

#[test]
fn timeout_and_read_regression_cannot_become_completion() {
    let mut f = Fake::new();
    f.expired = true;
    assert!(complete_one_kernel(&mut f, packet()).is_err());
    assert!(f.calls.is_empty());
    let mut f = Fake::new();
    let mut pending = done();
    pending.value = 1;
    f.snapshots = VecDeque::from([prime(), pending]);
    f.pause_expires = true;
    assert!(
        complete_one_kernel(&mut f, packet())
            .unwrap_err()
            .contains("deadline")
    );
    assert_eq!(f.doorbells, 1);
    let mut f = Fake::new();
    pending.read = 1;
    f.snapshots = VecDeque::from([prime(), pending, done()]);
    assert!(complete_one_kernel(&mut f, packet()).is_err());
}

#[test]
fn progress_requires_completion_then_destroy_and_fresh_snapshot() {
    let mut p = Progress::default();
    assert!(p.destroy().is_err());
    p.counters((0, 0)).unwrap();
    assert!(p.counters((1, 0)).is_err());
    p.complete(done()).unwrap();
    assert!(p.complete(done()).is_err());
    p.observe_completed(done()).unwrap();
    assert!(p.after_destroy.is_none());
    p.destroy().unwrap();
    assert!(p.destroy().is_err());
    let mut s = done();
    s.read = 1;
    p.observe_completed(s).unwrap();
    assert_eq!(p.after_destroy, Some(s));
    assert!(p.observe_completed(done()).is_err());
    s.value = 1;
    assert!(p.observe_completed(s).is_err());
    p.released = true;
    assert!(p.observe_completed(done()).is_err());
}

#[test]
fn retired_header_word_with_acquired_completion_survives_the_full_lifecycle() {
    for setup in [0, 1] {
        let retired = Snapshot {
            header: AQL_INVALID_PACKET_HEADER_V1,
            setup,
            ..done()
        };
        let mut f = Fake::new();
        f.snapshots = VecDeque::from([prime(), retired]);
        let observed = complete_one_kernel(&mut f, packet()).unwrap();
        assert_eq!(f.doorbells, 1);
        let mut p = Progress::default();
        p.complete(observed).unwrap();
        p.observe_completed(observed).unwrap();
        assert!(p.after_destroy.is_none());
        p.destroy().unwrap();
        p.observe_completed(Snapshot {
            read: 1,
            ..observed
        })
        .unwrap();
        assert_eq!(p.after_destroy.unwrap().read, 1);
        assert!(!p.released);
    }
}

#[test]
fn cleared_setup_never_substitutes_for_signal_completion_or_a_valid_packet_word() {
    for (header, setup, kind, value) in [
        (AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1, 0, 1, 0),
        (AQL_INVALID_PACKET_HEADER_V1, 0, 1, 1),
        (AQL_INVALID_PACKET_HEADER_V1, 2, 1, 0),
        (AQL_INVALID_PACKET_HEADER_V1, 0, 0, 0),
        (AQL_INVALID_PACKET_HEADER_V1, 0, 1, -1),
    ] {
        let mut f = Fake::new();
        f.snapshots = VecDeque::from([
            prime(),
            Snapshot {
                header,
                setup,
                kind,
                value,
                ..done()
            },
        ]);
        assert!(complete_one_kernel(&mut f, packet()).is_err());
        assert_eq!(f.doorbells, 1);
    }
}

struct Lifecycle {
    packet: Fake,
    steps: Vec<Step>,
    fail: Option<Step>,
    currentness: usize,
    fail_currentness: Option<usize>,
    quarantined: bool,
    completed: bool,
    destroyed: bool,
    finished: bool,
}
impl LifecycleBackend for Lifecycle {
    fn preflight(&mut self) -> QueueResult<()> {
        Ok(())
    }
    fn currentness(&mut self) -> QueueResult<()> {
        self.currentness += 1;
        if self.fail_currentness == Some(self.currentness) {
            Err("currentness".into())
        } else {
            Ok(())
        }
    }
    fn quarantine(&mut self) {
        self.quarantined = true;
    }
    fn step(&mut self, step: Step) -> QueueResult<()> {
        self.steps.push(step);
        if self.fail == Some(step) {
            return Err("injected lifecycle failure".into());
        }
        match step {
            Step::CheckUnpublished => {
                complete_one_kernel(&mut self.packet, packet())?;
                self.completed = true;
            }
            Step::Destroy => {
                assert!(self.completed);
                self.destroyed = true;
            }
            Step::ReleaseMemory => assert!(self.destroyed),
            Step::Finish => {
                assert!(self.destroyed);
                self.finished = true;
            }
            _ => {}
        }
        Ok(())
    }
}
fn lifecycle() -> Lifecycle {
    Lifecycle {
        packet: Fake::new(),
        steps: Vec::new(),
        fail: None,
        currentness: 0,
        fail_currentness: None,
        quarantined: false,
        completed: false,
        destroyed: false,
        finished: false,
    }
}

#[test]
fn actual_lifecycle_orders_kernel_before_destroy_and_teardown() {
    let mut f = lifecycle();
    run_lifecycle(&mut f).unwrap();
    assert_eq!(f.steps, STEPS);
    assert!(f.completed && f.destroyed && f.finished);
    assert!(!f.quarantined);
}

#[test]
fn every_lifecycle_error_or_currentness_loss_quarantines_without_cleanup_retry() {
    for (i, step) in STEPS.into_iter().enumerate() {
        let mut f = lifecycle();
        f.fail = Some(step);
        assert!(run_lifecycle(&mut f).is_err());
        assert_eq!(f.steps, &STEPS[..=i]);
        assert!(f.quarantined);
        assert!(!f.finished);
    }
    for n in 1..=STEPS.len() * 2 {
        let mut f = lifecycle();
        f.fail_currentness = Some(n);
        assert!(run_lifecycle(&mut f).is_err());
        assert!(f.quarantined);
        assert_eq!(f.currentness, n);
    }
    let mut f = lifecycle();
    f.packet.pause_expires = true;
    let mut s = done();
    s.value = 1;
    f.packet.snapshots = VecDeque::from([prime(), s]);
    assert!(run_lifecycle(&mut f).is_err());
    assert!(!f.destroyed && !f.finished && f.quarantined);
    assert_eq!(f.steps.last(), Some(&Step::CheckUnpublished));
}
