use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn counts(local: u64) -> Counts {
    Counts {
        full_discoveries: 2,
        local_checkpoints: local,
        before_calls: local + 16,
        after_calls: local + 16,
        generation_probes: 2 * local + 3,
    }
}
struct Recording {
    events: Vec<String>,
    fail: Option<String>,
    unwind: Option<String>,
    quarantined: [bool; 4],
    pending_polls: [usize; 3],
    periodic_checks: [usize; 3],
    observed: [u64; 5],
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            events: vec![],
            fail: None,
            unwind: None,
            quarantined: [false; 4],
            pending_polls: [1; 3],
            periodic_checks: [1; 3],
            observed: [0; 5],
        }
    }
}
impl Recording {
    fn checkpoint(&mut self, participants: u64) {
        self.observed[1] += 1;
        self.observed[2] += participants;
        self.observed[3] += participants;
        self.observed[4] += 2;
    }
    fn full(&mut self) {
        self.observed[0] += 1;
        self.observed[2] += 2;
        self.observed[3] += 2;
        self.observed[4] += 1;
    }
    fn event(&mut self, label: impl Into<String>) -> Result<()> {
        let label = label.into();
        self.events.push(label.clone());
        assert!(self.unwind.as_ref() != Some(&label), "injected tail unwind");
        if self.fail.as_ref() == Some(&label) {
            return Err("injected tail failure".into());
        }
        Ok(())
    }
}
impl ClosedBackend for Recording {
    type Prepared = Kind;
    type Pending = (Kind, usize);
    fn enter(&mut self) -> Result<()> {
        self.event("entry")?;
        self.full();
        Ok(())
    }
    fn prepare(&mut self, kind: Kind) -> Result<Kind> {
        self.event(format!("{kind:?}.prepare"))?;
        self.checkpoint(2);
        self.checkpoint(1);
        Ok(kind)
    }
    fn publish(&mut self, kind: Kind) -> Result<(Kind, usize)> {
        self.event(format!("{kind:?}.publish"))?;
        self.checkpoint(1);
        Ok((kind, 0))
    }
    fn poll(&mut self, pending: &mut (Kind, usize)) -> Result<Option<u64>> {
        let (kind, observed) = pending;
        let index = *kind as usize;
        let completed = *observed == self.pending_polls[index];
        self.event(format!(
            "{kind:?}.{}",
            if completed { "completed" } else { "pending" }
        ))?;
        if completed {
            self.checkpoint(1);
            Ok(Some(*kind as u64 + 11))
        } else {
            if *observed < self.periodic_checks[index] {
                self.checkpoint(1);
            }
            *observed += 1;
            Ok(None)
        }
    }
    fn pause(&mut self, kind: Kind) {
        self.event(format!("{kind:?}.pause")).unwrap();
    }
    fn retired(&mut self, kind: Kind) -> Result<()> {
        self.event(format!("{kind:?}.retired_group_fence"))?;
        self.checkpoint(2);
        Ok(())
    }
    fn read(&mut self, root: Root) -> Result<Vec<u8>> {
        self.event(format!("{root:?}.read"))?;
        for participants in [2, 1, 1, 2] {
            self.checkpoint(participants);
        }
        Ok(match root {
            Root::Choice => vec![0, 0, 0, 0],
            Root::Normalized => vec![0; 8192],
            Root::Logits => vec![0; 303872],
            _ => panic!("unexpected read role"),
        })
    }
    fn exit(&mut self) -> Result<Counts> {
        self.event("full_exit")?;
        self.observed[4] += 1;
        self.full();
        let [
            full_discoveries,
            local_checkpoints,
            before_calls,
            after_calls,
            generation_probes,
        ] = self.observed;
        Ok(Counts {
            full_discoveries,
            local_checkpoints,
            before_calls,
            after_calls,
            generation_probes,
        })
    }
    fn build(
        &mut self,
        choice: Vec<u8>,
        normalized: Vec<u8>,
        logits: Vec<u8>,
        host_ns: [u64; 3],
        counts: Counts,
    ) -> Result<Gfx950EngineeringPeerScopedTailObservationV1> {
        self.event("build")?;
        observation(choice, normalized, logits, host_ns, counts)
    }
    fn final_deadline(&mut self) -> Result<()> {
        self.event("final_deadline")
    }
    fn quarantine(&mut self) {
        self.events.push("quarantine".into());
        self.quarantined.fill(true);
    }
}
fn trace() -> Vec<String> {
    [
        "entry",
        "FinalNorm.prepare",
        "FinalNorm.publish",
        "FinalNorm.pending",
        "FinalNorm.pause",
        "FinalNorm.completed",
        "FinalNorm.retired_group_fence",
        "Head.prepare",
        "Head.publish",
        "Head.pending",
        "Head.pause",
        "Head.completed",
        "Head.retired_group_fence",
        "Argmax.prepare",
        "Argmax.publish",
        "Argmax.pending",
        "Argmax.pause",
        "Argmax.completed",
        "Argmax.retired_group_fence",
        "Choice.read",
        "Normalized.read",
        "Logits.read",
        "full_exit",
        "build",
        "final_deadline",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn scoped_tail_closed_engine_preserves_serial_retirement_and_original_read_order() {
    let mut fake = Recording::default();
    let output = closed_tail(&mut fake).unwrap();
    assert_eq!(fake.events, trace());
    assert_eq!(fake.quarantined, [false; 4]);
    assert_eq!(output.host_ns, [11, 12, 13]);
    assert_eq!(output.choice, [0; 4]);
    assert_eq!(output.normalized.len(), 8192);
    assert_eq!(output.logits.len(), 303872);
    assert_eq!(output.currentness, counts(30));
}
#[test]
fn scoped_tail_each_fallible_boundary_quarantines_without_output_or_later_dispatch() {
    let expected = trace();
    for label in expected.iter().filter(|label| !label.ends_with(".pause")) {
        let index = expected.iter().position(|v| v == label).unwrap();
        let mut fake = Recording {
            fail: Some(label.clone()),
            ..Recording::default()
        };
        assert!(closed_tail(&mut fake).is_err(), "{label}");
        assert_eq!(&fake.events[..=index], &expected[..=index], "{label}");
        assert_eq!(fake.events.len(), index + 2, "{label}");
        assert_eq!(fake.events.last().unwrap(), "quarantine");
        assert_eq!(fake.quarantined, [true; 4]);
    }
}
#[test]
fn scoped_tail_each_unwind_boundary_quarantines_including_after_full_exit() {
    let expected = trace();
    for label in &expected {
        let index = expected.iter().position(|v| v == label).unwrap();
        let mut fake = Recording {
            unwind: Some(label.clone()),
            ..Recording::default()
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| closed_tail(&mut fake))).is_err(),
            "{label}"
        );
        assert_eq!(&fake.events[..=index], &expected[..=index], "{label}");
        assert_eq!(fake.events.len(), index + 2, "{label}");
        assert_eq!(fake.events.last().unwrap(), "quarantine");
        assert_eq!(fake.quarantined, [true; 4]);
    }
}
fn roots() -> [Buffer; 7] {
    std::array::from_fn(|i| Buffer {
        group: 11,
        id: i as u64 + 1,
        owner: 0,
        bytes: Root::ALL[i].bytes(),
    })
}
#[test]
fn scoped_tail_roots_refuse_each_group_rank_extent_and_alias_substitution() {
    let valid = roots();
    validate_roots(11, &valid).unwrap();
    for index in 0..7 {
        let mut wrong = valid;
        wrong[index].group += 1;
        assert!(validate_roots(11, &wrong).is_err());
        let mut wrong = valid;
        wrong[index].owner = 1;
        assert!(validate_roots(11, &wrong).is_err());
        for extent in [0, valid[index].bytes - 1, valid[index].bytes + 1] {
            let mut wrong = valid;
            wrong[index].bytes = extent;
            assert!(validate_roots(11, &wrong).is_err());
        }
        for prior in 0..index {
            let mut wrong = valid;
            wrong[index].id = wrong[prior].id;
            assert!(validate_roots(11, &wrong).is_err());
        }
    }
}
#[test]
fn scoped_tail_kernels_refuse_foreign_rank_alias_and_permuted_roles() {
    let valid = [(11, 0, 1), (11, 0, 2), (11, 0, 3)];
    validate_kernels(11, valid).unwrap();
    for index in 0..3 {
        let mut wrong = valid;
        wrong[index].0 += 1;
        assert!(validate_kernels(11, wrong).is_err());
        let mut wrong = valid;
        wrong[index].1 = 1;
        assert!(validate_kernels(11, wrong).is_err());
        let mut wrong = valid;
        wrong[index].2 = valid[(index + 1) % 3].2;
        assert!(validate_kernels(11, wrong).is_err());
    }
    for kind in Kind::ALL {
        validate_program(kind, kind.symbol(), kind.explicit_bytes() + 256).unwrap();
        assert!(validate_program(kind, kind.symbol(), kind.explicit_bytes()).is_err());
        assert!(validate_program(kind, "", kind.explicit_bytes() + 256).is_err());
        for other in Kind::ALL {
            if other != kind {
                assert!(
                    validate_program(kind, other.symbol(), kind.explicit_bytes() + 256).is_err()
                );
            }
        }
    }
}
#[test]
fn scoped_tail_keeps_per_dispatch_cap_and_uses_inherited_absolute_deadline() {
    for timeout in [1, 10_000] {
        validate_timeout(timeout).unwrap();
    }
    for timeout in [0, 10_001, 600_000, u32::MAX] {
        assert!(validate_timeout(timeout).is_err());
    }
    let now = Instant::now();
    let future = now.checked_add(Duration::from_millis(10)).unwrap();
    check_deadline(now, future).unwrap();
    assert!(check_deadline(now, now).is_err());
    assert!(check_deadline(future, now).is_err());
    assert!(check_deadline(future, future).is_err());
}
#[test]
fn scoped_tail_counts_close_every_original_boundary_and_checked_poll_addition() {
    for local in [27, 28, 30, 100, 1_000_000] {
        validate_counts(&counts(local)).unwrap();
    }
    assert!(validate_counts(&counts(26)).is_err());
    let base = counts(27);
    for field in 0..5 {
        let mut wrong = base;
        match field {
            0 => wrong.full_discoveries += 1,
            1 => wrong.local_checkpoints += 1,
            2 => wrong.before_calls += 1,
            3 => wrong.after_calls += 1,
            _ => wrong.generation_probes += 1,
        }
        assert!(validate_counts(&wrong).is_err());
    }
    let mut overflow = base;
    overflow.local_checkpoints = u64::MAX;
    assert!(validate_counts(&overflow).is_err());
    overflow.local_checkpoints = u64::MAX / 2;
    assert!(validate_counts(&overflow).is_err());
}
#[test]
fn scoped_tail_closed_route_counts_zero_and_extra_periodic_checks_not_raw_polls() {
    for (pending, periodic) in [
        ([0; 3], [0; 3]),
        ([1, 2, 4], [0; 3]),
        ([1, 2, 4], [1, 0, 3]),
        ([4; 3], [4; 3]),
    ] {
        let mut fake = Recording {
            pending_polls: pending,
            periodic_checks: periodic,
            ..Recording::default()
        };
        let output = closed_tail(&mut fake).unwrap();
        let extra = periodic.iter().sum::<usize>() as u64;
        assert_eq!(output.currentness, counts(27 + extra));
        assert_eq!(
            fake.events
                .iter()
                .filter(|s| s.ends_with(".pending"))
                .count(),
            pending.iter().sum::<usize>()
        );
        assert_eq!(
            fake.events
                .iter()
                .filter(|s| s.ends_with(".completed"))
                .count(),
            3
        );
        assert_eq!(fake.quarantined, [false; 4]);
    }
}
fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap()
        .1
        .split_once(end)
        .unwrap()
        .0
}
#[test]
fn scoped_tail_production_routes_keep_the_audited_group_and_rank_boundaries() {
    // Source-bound callsite guards complement the closed-engine fault fixture.
    // They do not simulate Context hardware or qualify machine-code behavior.
    let tail = include_str!("engineering_gfx950_peer_scoped_tail_v1.rs");
    let native = tail
        .split_once("impl ClosedBackend for NativeTail")
        .unwrap()
        .1;
    let prepare = section(native, "fn prepare(", "fn publish(");
    assert_eq!(prepare.matches("route.idle_group(self.group)?").count(), 1);
    assert_eq!(
        prepare
            .matches("prepare_peer_dispatch_currentness(")
            .count(),
        1
    );
    assert!(
        prepare.find("route.idle_group").unwrap()
            < prepare.find("prepare_peer_dispatch_currentness").unwrap()
    );
    assert_eq!(
        section(native, "fn retired(", "fn read(")
            .matches(".idle_group(self.group)")
            .count(),
        1
    );
    assert_eq!(
        section(native, "fn publish(", "fn poll(")
            .matches("publish_prepared_dispatch_with_currentness(")
            .count(),
        1
    );
    assert_eq!(
        section(native, "fn poll(", "fn pause(")
            .matches("poll_pending_dispatch_with_currentness(")
            .count(),
        1
    );
    assert_eq!(
        section(native, "fn read(", "fn exit(")
            .matches(".read_currentness(")
            .count(),
        1
    );
    assert!(!native.contains("RankCurrentness::Full"));
    let peer = include_str!("engineering_gfx950_peer.rs");
    let read = section(peer, "fn read_currentness(", "pub fn load_kernel(");
    assert_eq!(read.matches("currentness.idle_group(self)?").count(), 2);
    assert_eq!(
        read.matches(".read_currentness(local, offset, bytes, &mut route)?")
            .count(),
        1
    );
    let prepare = section(
        peer,
        "fn prepare_peer_dispatch_currentness(",
        "pub fn release(",
    );
    assert_eq!(
        prepare
            .matches("prepare_dispatch_with_peer_bindings_currentness(")
            .count(),
        1
    );
    assert!(!prepare.contains("idle_group"));
    let context = include_str!("engineering_gfx950.rs");
    let read = section(context, "fn read_currentness(", "fn load(");
    assert_eq!(read.matches("currentness.idle(self)?").count(), 1);
    assert_eq!(read.matches("currentness.check(self, false)?").count(), 1);
    assert!(read.find("currentness.idle").unwrap() < read.find("Backend::with_bytes").unwrap());
    assert!(read.find("Backend::with_bytes").unwrap() < read.find("currentness.check").unwrap());
    let prepare = section(
        context,
        "fn prepare_dispatch_with_peer_bindings_currentness(",
        "unsafe fn dispatch(",
    );
    assert_eq!(prepare.matches("currentness.idle(self)?").count(), 1);
    let publish = section(
        context,
        "unsafe fn publish_prepared_dispatch_with_currentness(",
        "fn poll_pending_dispatch(",
    );
    assert_eq!(
        publish.matches("currentness.check(self, false)?").count(),
        1
    );
    let poll = section(
        context,
        "fn poll_pending_dispatch_with_currentness(",
        "pending.completed = true;",
    );
    assert_eq!(poll.matches("currentness.check(self, false)?").count(), 1);
    assert_eq!(poll.matches("currentness.idle(self)?").count(), 1);
    assert!(poll.contains("if now >= pending.next_currentness"));
    assert!(poll.contains("pending.next_currentness = now + Duration::from_millis(100)"));
    assert!(poll.find("currentness.check").unwrap() < poll.find("return Ok(None)").unwrap());
    assert!(poll.find("return Ok(None)").unwrap() < poll.find("currentness.idle").unwrap());
    let scope = include_str!("engineering_gfx950_peer_scoped_currentness_v1.rs");
    assert!(
        section(
            scope,
            "pub(super) fn idle_group(",
            "pub(super) fn publication("
        )
        .contains("Self::Scoped(window) => window.checkpoint(group, true)")
    );
}
#[test]
fn scoped_tail_return_preserves_original_bytes_without_replacing_worker_argmax() {
    let mut normalized = vec![0; 8192];
    let mut logits = vec![0; 303872];
    normalized[..2].copy_from_slice(&0x7f80_u16.to_le_bytes());
    logits[..2].copy_from_slice(&0x7fc1_u16.to_le_bytes());
    let choice = u32::MAX.to_le_bytes();
    let output = observation(
        choice.to_vec(),
        normalized.clone(),
        logits.clone(),
        [7, 8, 9],
        counts(27),
    )
    .unwrap();
    assert_eq!(output.choice, choice);
    assert_eq!(output.normalized, normalized);
    assert_eq!(output.logits, logits);
    assert_eq!(output.host_ns, [7, 8, 9]);
}
#[test]
fn scoped_tail_return_refuses_wrong_extents_before_guard_disarm() {
    for size in [0, 3, 5] {
        assert!(
            observation(
                vec![0; size],
                vec![0; 8192],
                vec![0; 303872],
                [0; 3],
                counts(27)
            )
            .is_err()
        );
    }
    for size in [0, 8191, 8193] {
        assert!(
            observation(
                vec![0; 4],
                vec![0; size],
                vec![0; 303872],
                [0; 3],
                counts(27)
            )
            .is_err()
        );
    }
    for size in [0, 303871, 303873] {
        assert!(observation(vec![0; 4], vec![0; 8192], vec![0; size], [0; 3], counts(27)).is_err());
    }
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
#[test]
fn scoped_tail_finalnorm_plan_equals_pinned_original_worker_bytes_and_roles() {
    let actual = plan(Kind::FinalNorm);
    let mut expected = vec![0; 352];
    for offset in [8, 40, 72] {
        put64(&mut expected, offset, 4096);
    }
    for (offset, word) in [(80, 1), (84, 4096), (88, 897_988_541)] {
        put32(&mut expected, offset, word);
    }
    assert_eq!(actual.bytes, expected);
    use BufferAccessV1::{Read, Write};
    assert_eq!(
        actual.slices,
        vec![
            (Root::Hidden, 0, 8192, Read),
            (Root::Empty, 16, 0, Read),
            (Root::FinalNorm, 32, 8192, Read),
            (Root::Empty, 48, 0, Write),
            (Root::Normalized, 64, 8192, Write)
        ]
    );
    assert_eq!(Kind::FinalNorm.grid(), [64, 1, 1]);
}
#[test]
fn scoped_tail_head_plan_equals_pinned_original_worker_bytes_and_roles() {
    let actual = plan(Kind::Head);
    let mut expected = vec![0; 328];
    for (offset, count) in [(8, 4096), (24, 622_329_856), (40, 151_936)] {
        put64(&mut expected, offset, count);
    }
    for (offset, word) in [(48, 1), (52, 151_936), (56, 4096), (60, 2), (64, 6)] {
        put32(&mut expected, offset, word);
    }
    assert_eq!(actual.bytes, expected);
    use BufferAccessV1::{Read, Write};
    assert_eq!(
        actual.slices,
        vec![
            (Root::Normalized, 0, 8192, Read),
            (Root::Head, 16, 1_244_659_712, Read),
            (Root::Logits, 32, 303872, Write)
        ]
    );
    assert_eq!(Kind::Head.grid(), [607744, 1, 1]);
}
#[test]
fn scoped_tail_argmax_plan_equals_pinned_original_worker_bytes_and_roles() {
    let actual = plan(Kind::Argmax);
    let mut expected = vec![0; 296];
    put64(&mut expected, 8, 151_936);
    put64(&mut expected, 24, 1);
    put32(&mut expected, 32, 1);
    assert_eq!(actual.bytes, expected);
    use BufferAccessV1::{Read, Write};
    assert_eq!(
        actual.slices,
        vec![
            (Root::Logits, 0, 303872, Read),
            (Root::Choice, 16, 4, Write)
        ]
    );
    assert_eq!(Kind::Argmax.grid(), [64, 1, 1]);
}
#[test]
fn scoped_tail_public_signature_has_fixed_inputs_and_no_callback_or_borrowed_result() {
    let _: for<'owner, 'input, 'kernel> unsafe fn(
        &'owner mut Gfx950EngineeringPeerGroupV1,
        &'input Gfx950EngineeringPeerScopedTailInputsV1<'kernel>,
        u32,
        Instant,
    ) -> Result<
        Gfx950EngineeringPeerScopedTailObservationV1,
    > = Gfx950EngineeringPeerGroupV1::dispatch_tail_scoped_currentness_unchecked_v1;
    fn data_only(_: Gfx950EngineeringPeerScopedTailObservationV1) {}
    let _ = data_only;
}
