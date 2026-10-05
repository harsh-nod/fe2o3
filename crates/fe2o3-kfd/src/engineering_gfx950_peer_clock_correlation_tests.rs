use super::*;
use crate::currentness::admit_clock_correlation;
use fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs;

struct Fake {
    identities: Vec<Identity>,
    calls: Vec<String>,
    checks: usize,
    fail_at: Option<usize>,
    wrong_reply: Option<usize>,
    changed_after: Option<(usize, Identity)>,
}

impl Fake {
    fn new(count: usize) -> Self {
        Self {
            identities: (0..count)
                .map(|rank| Identity {
                    unique_id: 100 + rank as u64,
                    gpu_id: 7 + rank as u32,
                    queue_epoch: rank as u64,
                })
                .collect(),
            calls: vec![],
            checks: 0,
            fail_at: None,
            wrong_reply: None,
            changed_after: None,
        }
    }
    fn step(&mut self, name: String) -> Result<()> {
        self.calls.push(name);
        if self.fail_at == Some(self.calls.len() - 1) {
            Err("injected group clock failure".into())
        } else {
            Ok(())
        }
    }
}

impl ClockBackend for Fake {
    fn participants(&self) -> usize {
        self.identities.len()
    }
    fn check(&mut self) -> Result<()> {
        self.step("check".into())?;
        self.checks += 1;
        Ok(())
    }
    fn identity(&self, rank: usize) -> Identity {
        if self.checks == 2
            && let Some((changed_rank, identity)) = self.changed_after
        {
            if changed_rank == rank {
                return identity;
            }
        }
        self.identities[rank]
    }
    fn sample(&mut self, rank: usize) -> Result<KfdClockCorrelationObservationV1> {
        self.step(format!("sample:{rank}"))?;
        let gpu_id = self.identities[rank].gpu_id + u32::from(self.wrong_reply == Some(rank));
        Ok(admit_clock_correlation(
            KfdIoctlGetClockCountersArgs {
                gpu_clock_counter: 1000 - rank as u64,
                cpu_clock_counter: 2000 + rank as u64,
                system_clock_counter: 3000 + rank as u64,
                system_clock_freq: 1_000_000_000,
                gpu_id,
                pad: 0,
            },
            gpu_id,
        )
        .unwrap())
    }
}

fn owner() -> Gfx950EngineeringPeerGroupV1 {
    Gfx950EngineeringPeerGroupV1 {
        incarnation: 9,
        contexts: vec![],
        buffers: Default::default(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    }
}

fn quarantined(group: &mut Gfx950EngineeringPeerGroupV1) {
    assert!(group.poisoned);
    assert_eq!(
        group.observe_clock_correlation_v1().unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert_eq!(
        group.close().unwrap_err(),
        "peer group is closed or quarantined"
    );
}

#[test]
fn clocks_two_and_eight_ranks_preserve_raw_identity_and_full_fence_order() {
    for count in [2, 8] {
        let mut fake = Fake::new(count);
        let values = sample_group(&mut fake, 9).unwrap();
        assert_eq!(values.len(), count);
        let mut expected = vec!["check".to_owned()];
        expected.extend((0..count).map(|rank| format!("sample:{rank}")));
        expected.push("check".into());
        assert_eq!(fake.calls, expected);
        for (rank, value) in values.iter().copied().enumerate() {
            assert_eq!(value.group_incarnation(), 9);
            assert_eq!(value.rank(), rank);
            assert_eq!(value.unique_id(), 100 + rank as u64);
            assert_eq!(value.queue_epoch(), rank as u64);
            assert_eq!(value.counters().gpu_id(), 7 + rank as u32);
            assert_eq!(value.counters().gpu_clock_counter(), 1000 - rank as u64);
            assert_eq!(value.counters().system_clock_frequency_hz(), 1_000_000_000);
            assert!(
                value
                    .sample_finished()
                    .checked_duration_since(value.sample_started())
                    .is_some()
            );
        }
    }
}

#[test]
fn clocks_invalid_world_or_group_refuses_before_backend_and_poisons() {
    for (count, incarnation) in [(0, 9), (1, 9), (3, 9), (7, 9), (9, 9), (2, 0)] {
        let mut fake = Fake::new(count);
        let mut group = owner();
        assert_eq!(
            group
                .finish(sample_group(&mut fake, incarnation))
                .unwrap_err(),
            "invalid clock observation group identity or participant count"
        );
        assert!(fake.calls.is_empty());
        quarantined(&mut group);
    }
}

#[test]
fn clocks_invalid_or_duplicate_whole_roster_refuses_before_first_sample() {
    for kind in 0..3 {
        let mut fake = Fake::new(8);
        match kind {
            0 => fake.identities[7].unique_id = 0,
            1 => fake.identities[7].unique_id = fake.identities[0].unique_id,
            _ => fake.identities[7].gpu_id = fake.identities[0].gpu_id,
        }
        let mut group = owner();
        assert_eq!(
            group.finish(sample_group(&mut fake, 9)).unwrap_err(),
            "invalid or duplicate clock observation device identity"
        );
        assert_eq!(fake.calls, ["check"]);
        quarantined(&mut group);
    }
}

#[test]
fn clocks_every_sample_and_both_fence_failures_poison_without_partial_return() {
    let mut success = Fake::new(8);
    sample_group(&mut success, 9).unwrap();
    for at in 0..10 {
        let mut fake = Fake::new(8);
        fake.fail_at = Some(at);
        let mut group = owner();
        assert_eq!(
            group.finish(sample_group(&mut fake, 9)).unwrap_err(),
            "injected group clock failure"
        );
        assert_eq!(fake.calls, success.calls[..=at]);
        quarantined(&mut group);
    }
}

#[test]
fn clocks_wrong_counter_gpu_refuses_first_middle_last_reply() {
    for rank in [0, 4, 7] {
        let mut fake = Fake::new(8);
        fake.wrong_reply = Some(rank);
        let mut group = owner();
        assert_eq!(
            group.finish(sample_group(&mut fake, 9)).unwrap_err(),
            "clock observation counter identity or system frequency"
        );
        assert_eq!(fake.calls.len(), rank + 2);
        quarantined(&mut group);
    }
}

#[test]
fn clocks_changed_uid_gpu_or_epoch_refuses_after_exit_fence() {
    for kind in 0..3 {
        let mut fake = Fake::new(2);
        let mut changed = fake.identities[1];
        match kind {
            0 => changed.unique_id += 1,
            1 => changed.gpu_id += 1,
            _ => changed.queue_epoch += 1,
        }
        fake.changed_after = Some((1, changed));
        let mut group = owner();
        assert_eq!(
            group.finish(sample_group(&mut fake, 9)).unwrap_err(),
            "clock observation device identity or queue epoch changed"
        );
        assert_eq!(fake.calls, ["check", "sample:0", "sample:1", "check"]);
        quarantined(&mut group);
    }
}

#[test]
fn clocks_public_empty_group_refuses_and_closed_or_poisoned_group_stays_closed() {
    let mut group = owner();
    assert_eq!(
        group.observe_clock_correlation_v1().unwrap_err(),
        "invalid clock observation group identity or participant count"
    );
    quarantined(&mut group);
    let mut closed = owner();
    closed.closed = true;
    assert_eq!(
        closed.observe_clock_correlation_v1().unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert!(closed.closed);
    assert!(!closed.poisoned);
}

#[test]
fn clocks_each_successful_call_takes_new_fences_and_samples() {
    let mut fake = Fake::new(2);
    let first = sample_group(&mut fake, 9).unwrap();
    let second = sample_group(&mut fake, 9).unwrap();
    assert_eq!(
        fake.calls,
        [
            "check", "sample:0", "sample:1", "check", "check", "sample:0", "sample:1", "check"
        ]
    );
    assert_eq!(first[0].counters(), second[0].counters());
    assert!(
        second[0]
            .sample_started()
            .checked_duration_since(first[1].sample_finished())
            .is_some()
    );
}
