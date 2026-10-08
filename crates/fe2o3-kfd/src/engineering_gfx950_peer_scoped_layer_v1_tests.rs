use super::*;
#[cfg(feature = "engineering-currentness-duration-diagnostics")]
#[path = "engineering_gfx950_peer_scoped_layer_duration_v1_tests.rs"]
mod duration;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Default)]
struct Recording {
    events: Vec<&'static str>,
    fail: Option<&'static str>,
    unwind: Option<&'static str>,
    poisoned: [bool; 5],
    published: bool,
}
impl Recording {
    fn event(&mut self, name: &'static str) -> Result<()> {
        self.events.push(name);
        assert_ne!(self.unwind, Some(name), "injected closed-layer unwind");
        if self.fail == Some(name) {
            return Err("injected closed-layer failure".into());
        }
        Ok(())
    }
}
impl ClosedBackend for Recording {
    type Prefix = u8;
    type Pending = u8;
    type Hidden = u8;
    type Counts = u8;
    type Output = [u8; 4];
    fn enter(&mut self) -> Result<()> {
        self.event("entry")
    }
    fn prefix(&mut self) -> Result<u8> {
        self.event("prefix")?;
        Ok(1)
    }
    fn mlp(&mut self) -> Result<u8> {
        self.event("mlp_and_private_retirement")?;
        Ok(2)
    }
    fn hidden(&mut self) -> Result<u8> {
        self.event("two_hidden_reads_and_validation")?;
        Ok(3)
    }
    fn exit(&mut self) -> Result<u8> {
        self.event("full_exit")?;
        Ok(4)
    }
    fn commit(&mut self, prefix: u8, pending: u8, hidden: u8, counts: u8) -> Result<[u8; 4]> {
        self.event("commit")?;
        self.published = true;
        Ok([prefix, pending, hidden, counts])
    }
    fn quarantine(&mut self) {
        self.events.push("quarantine");
        self.poisoned.fill(true);
    }
}
const TRACE: [&str; 6] = [
    "entry",
    "prefix",
    "mlp_and_private_retirement",
    "two_hidden_reads_and_validation",
    "full_exit",
    "commit",
];

#[test]
fn scoped_closed_layer_uses_one_native_and_fake_ordering_engine() {
    let mut backend = Recording::default();
    assert_eq!(closed_layer(&mut backend).unwrap(), [1, 2, 3, 4]);
    assert_eq!(backend.events, TRACE);
    assert_eq!(backend.poisoned, [false; 5]);
    assert!(backend.published);
}
#[test]
fn scoped_closed_layer_every_boundary_failure_quarantines_before_publication() {
    for (index, name) in TRACE.iter().enumerate() {
        let mut backend = Recording {
            fail: Some(name),
            ..Recording::default()
        };
        assert!(closed_layer(&mut backend).is_err());
        assert_eq!(&backend.events[..=index], &TRACE[..=index]);
        assert_eq!(backend.events.len(), index + 2);
        assert_eq!(backend.events.last(), Some(&"quarantine"));
        assert_eq!(backend.poisoned, [true; 5]);
        assert!(!backend.published);
    }
}
#[test]
fn scoped_closed_layer_every_boundary_unwind_quarantines_before_publication() {
    for (index, name) in TRACE.iter().enumerate() {
        let mut backend = Recording {
            unwind: Some(name),
            ..Recording::default()
        };
        assert!(catch_unwind(AssertUnwindSafe(|| closed_layer(&mut backend))).is_err());
        assert_eq!(&backend.events[..=index], &TRACE[..=index]);
        assert_eq!(backend.events.len(), index + 2);
        assert_eq!(backend.poisoned, [true; 5]);
        assert!(!backend.published);
    }
}
#[test]
fn scoped_warm_admission_requires_ready_consumed_reuse_without_completed_proof() {
    assert!(warm_admission(Phase::Ready, ArenaPolicy::ReuseRetired, 2, true, false).is_ok());
    assert!(
        warm_admission(
            Phase::Ready,
            ArenaPolicy::ReuseRetired,
            u64::MAX,
            true,
            false
        )
        .is_ok()
    );
    for phase in [Phase::Busy, Phase::Completed, Phase::Poisoned] {
        assert!(warm_admission(phase, ArenaPolicy::ReuseRetired, 2, true, false).is_err());
    }
    for generation in [0, 1] {
        assert!(
            warm_admission(
                Phase::Ready,
                ArenaPolicy::ReuseRetired,
                generation,
                true,
                false
            )
            .is_err()
        );
    }
    assert!(warm_admission(Phase::Ready, ArenaPolicy::Fresh, 2, true, false).is_err());
    assert!(warm_admission(Phase::Ready, ArenaPolicy::ReuseRetired, 2, false, false).is_err());
    assert!(warm_admission(Phase::Ready, ArenaPolicy::ReuseRetired, 2, true, true).is_err());
}
#[test]
fn scoped_hidden_validation_requires_exact_finite_rank_pair() {
    let valid = [vec![0_u8; 8192], vec![0_u8; 8192]];
    hidden_pair(&valid).unwrap();
    for size in [0, 8191, 8193] {
        assert!(hidden_pair(&[vec![0; size], vec![0; size]]).is_err());
    }
    let mut mismatch = valid.clone();
    mismatch[1][0] = 1;
    assert!(hidden_pair(&mismatch).is_err());
    for word in [0x7f80_u16, 0xff80, 0x7fc1, 0xffff] {
        let mut bad = valid.clone();
        for rank in &mut bad {
            rank[..2].copy_from_slice(&word.to_le_bytes());
        }
        assert!(hidden_pair(&bad).is_err());
    }
}
#[test]
fn scoped_public_counts_are_data_only_and_preserve_rank_call_totals() {
    let source = crate::device::ScopedCountsV1 {
        full_discoveries: 2,
        local_checkpoints: 5,
        before_calls: 11,
        after_calls: 11,
        generation_probes: 13,
    };
    let public = Gfx950EngineeringPeerScopedCurrentnessCountsV1::from(source);
    assert_eq!(
        (
            public.full_discoveries,
            public.local_checkpoints,
            public.before_calls,
            public.after_calls,
            public.generation_probes
        ),
        (2, 5, 11, 11, 13)
    );
}
#[test]
fn scoped_closed_call_keeps_original_paired_timeout_cap() {
    let now = Instant::now();
    for value in [0, 10_001, 600_000, u32::MAX] {
        assert!(deadline(now, value).is_err());
    }
    assert!(deadline(now, 1).is_ok());
    assert!(deadline(now, 10_000).is_ok());
}
