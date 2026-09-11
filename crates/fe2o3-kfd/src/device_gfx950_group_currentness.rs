//! One fresh topology observation bracketed by every participant's full checks.

use super::*;

trait GroupFence {
    type Snapshot;
    fn participants(&self) -> usize;
    fn before_topology(&mut self, rank: usize) -> Result<(), DeviceBindingError>;
    fn discover(&mut self) -> Result<Self::Snapshot, DeviceBindingError>;
    fn compare(&mut self, rank: usize, snapshot: &Self::Snapshot)
    -> Result<(), DeviceBindingError>;
    fn after_topology(&mut self, rank: usize) -> Result<(), DeviceBindingError>;
    fn finish_snapshot(&mut self, snapshot: &Self::Snapshot) -> Result<(), DeviceBindingError>;
    fn poison_all(&mut self);
}

fn run_fence(backend: &mut impl GroupFence) -> Result<(), DeviceBindingError> {
    let result = (|| {
        let count = backend.participants();
        if !matches!(count, 2 | 8) {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "engineering group currentness participant count",
            ));
        }
        for rank in 0..count {
            backend.before_topology(rank)?;
        }
        // The snapshot is fresh for this fence and cannot escape it. All ranks'
        // mutable checks bracket its generation-consistent full observation.
        let snapshot = backend.discover()?;
        for rank in 0..count {
            backend.compare(rank, &snapshot)?;
        }
        for rank in 0..count {
            backend.after_topology(rank)?;
        }
        backend.finish_snapshot(&snapshot)?;
        Ok(())
    })();
    if result.is_err() {
        backend.poison_all();
    }
    result
}

struct NativeFence<'a, 'device> {
    devices: &'a mut [&'device mut CheckedGfx950XnackMinusDevice],
}

impl GroupFence for NativeFence<'_, '_> {
    type Snapshot = HostTopologySnapshot;

    fn participants(&self) -> usize {
        self.devices.len()
    }

    fn before_topology(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        let device = &mut self.devices[rank];
        if device.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        device.check_currentness_before_topology()
    }

    fn discover(&mut self) -> Result<Self::Snapshot, DeviceBindingError> {
        topology::discover_default_topology_for_target(topology::GfxTarget::Gfx950)
            .map_err(Into::into)
    }

    fn compare(
        &mut self,
        rank: usize,
        snapshot: &Self::Snapshot,
    ) -> Result<(), DeviceBindingError> {
        if *snapshot != self.devices[rank].topology {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        Ok(())
    }

    fn after_topology(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
        self.devices[rank].check_currentness_after_topology()
    }

    fn finish_snapshot(&mut self, snapshot: &Self::Snapshot) -> Result<(), DeviceBindingError> {
        topology::recheck_default_topology_generation(snapshot).map_err(Into::into)
    }

    fn poison_all(&mut self) {
        for device in &mut *self.devices {
            device.currentness_poisoned = true;
        }
    }
}

/// Internal, opt-in peer-group fence only. No cached snapshot, public token, or
/// new authority is returned. Any failure invalidates every retained device.
pub(crate) fn check_engineering_group_currentness(
    devices: &mut [&mut CheckedGfx950XnackMinusDevice],
) -> Result<(), DeviceBindingError> {
    run_fence(&mut NativeFence { devices })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recording {
        retained: Vec<u64>,
        observed: u64,
        events: Vec<String>,
        fail: Option<String>,
        change_at: Option<String>,
        poisoned: Vec<bool>,
    }

    impl Recording {
        fn new(count: usize) -> Self {
            Self {
                retained: vec![7; count],
                observed: 7,
                events: Vec::new(),
                fail: None,
                change_at: None,
                poisoned: vec![false; count],
            }
        }

        fn event(&mut self, event: String) -> Result<(), DeviceBindingError> {
            self.events.push(event.clone());
            if self.change_at.as_ref() == Some(&event) {
                self.observed = 8;
            }
            if self.fail.as_ref() == Some(&event) {
                Err(DeviceBindingError::CurrentnessFencePoisoned)
            } else {
                Ok(())
            }
        }
    }

    impl GroupFence for Recording {
        type Snapshot = u64;
        fn participants(&self) -> usize {
            self.retained.len()
        }
        fn before_topology(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
            self.event(format!("before:{rank}"))?;
            if self.poisoned[rank] {
                return Err(DeviceBindingError::CurrentnessFencePoisoned);
            }
            Ok(())
        }
        fn discover(&mut self) -> Result<Self::Snapshot, DeviceBindingError> {
            self.event("discover".into())?;
            Ok(self.observed)
        }
        fn compare(
            &mut self,
            rank: usize,
            snapshot: &Self::Snapshot,
        ) -> Result<(), DeviceBindingError> {
            self.event(format!("compare:{rank}"))?;
            if *snapshot != self.retained[rank] {
                return Err(DeviceBindingError::TopologySnapshotChanged);
            }
            Ok(())
        }
        fn after_topology(&mut self, rank: usize) -> Result<(), DeviceBindingError> {
            self.event(format!("after:{rank}"))
        }
        fn finish_snapshot(&mut self, snapshot: &Self::Snapshot) -> Result<(), DeviceBindingError> {
            self.event("finish".into())?;
            if *snapshot != self.observed {
                return Err(DeviceBindingError::TopologySnapshotChanged);
            }
            Ok(())
        }
        fn poison_all(&mut self) {
            self.events.push("poison".into());
            self.poisoned.fill(true);
        }
    }

    #[test]
    fn one_fresh_discovery_is_bracketed_by_all_participants() {
        for count in [2, 8] {
            let mut backend = Recording::new(count);
            run_fence(&mut backend).unwrap();
            let mut expected = (0..count)
                .map(|rank| format!("before:{rank}"))
                .collect::<Vec<_>>();
            expected.push("discover".into());
            expected.extend((0..count).map(|rank| format!("compare:{rank}")));
            expected.extend((0..count).map(|rank| format!("after:{rank}")));
            expected.push("finish".into());
            assert_eq!(backend.events, expected);
            run_fence(&mut backend).unwrap();
            assert_eq!(
                backend
                    .events
                    .iter()
                    .filter(|event| *event == "discover")
                    .count(),
                2
            );
        }
    }

    #[test]
    fn every_before_discovery_compare_and_after_failure_poison_all_without_later_work() {
        for count in [2, 8] {
            let mut control = Recording::new(count);
            run_fence(&mut control).unwrap();
            for (index, failure) in control.events.iter().enumerate() {
                let mut backend = Recording::new(count);
                backend.fail = Some(failure.clone());
                assert!(run_fence(&mut backend).is_err());
                assert_eq!(&backend.events[..=index], &control.events[..=index]);
                assert_eq!(backend.events.last().unwrap(), "poison");
                assert_eq!(backend.events.len(), index + 2);
                assert!(backend.poisoned.iter().all(|poisoned| *poisoned));
                backend.fail = None;
                assert!(run_fence(&mut backend).is_err());
            }
        }
    }

    #[test]
    fn changed_topology_or_any_retained_rank_rejects_without_exit_checks() {
        for count in [2, 8] {
            for changed in 0..=count {
                let mut backend = Recording::new(count);
                if changed == count {
                    backend.observed = 8;
                } else {
                    backend.retained[changed] = 8;
                }
                assert!(run_fence(&mut backend).is_err());
                assert!(backend.poisoned.iter().all(|poisoned| *poisoned));
                assert!(
                    !backend
                        .events
                        .iter()
                        .any(|event| event.starts_with("after:"))
                );
            }
        }
    }

    #[test]
    fn invalid_or_partial_roster_never_starts_observation() {
        for count in [0, 1, 3, 7, 9] {
            let mut backend = Recording::new(count);
            assert!(run_fence(&mut backend).is_err());
            assert_eq!(backend.events, ["poison"]);
        }
    }

    #[test]
    fn topology_change_between_ranks_or_after_discovery_cannot_pass() {
        for count in [2, 8] {
            for rank in 0..count {
                for phase in ["before", "after"] {
                    let mut backend = Recording::new(count);
                    backend.change_at = Some(format!("{phase}:{rank}"));
                    assert!(run_fence(&mut backend).is_err());
                    assert!(backend.poisoned.iter().all(|poisoned| *poisoned));
                }
            }
        }
    }
}
