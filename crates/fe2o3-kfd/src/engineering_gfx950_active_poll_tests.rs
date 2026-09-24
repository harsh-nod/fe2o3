use super::*;
use crate::engineering_gfx950::wait_policy::{ActivePollWait, PauseAction};

struct ActiveFake {
    inner: Fake,
    wait: Option<ActivePollWait>,
    now: Instant,
    actions: Vec<PauseAction>,
}

impl ActiveFake {
    fn new(fail_at: Option<usize>) -> Self {
        Self {
            inner: Fake {
                fail_at,
                pending_polls: 2,
                ..Fake::default()
            },
            wait: None,
            now: Instant::now(),
            actions: Vec::new(),
        }
    }
}

impl OrderedBackend for ActiveFake {
    type Prepared = usize;
    type Staged = usize;
    type Pending = usize;

    fn dispatch_fence(&mut self) -> Result<()> {
        self.inner.dispatch_fence()
    }
    fn prepare_all(&mut self, count: usize) -> Result<Vec<usize>> {
        self.inner.prepare_all(count)
    }
    fn preparation_fence(&mut self) -> Result<()> {
        self.inner.preparation_fence()
    }
    fn stage(&mut self, prepared: Vec<usize>) -> Result<usize> {
        self.inner.stage(prepared)
    }
    fn publish(&mut self, count: usize, deadline: Instant) -> Result<usize> {
        let pending = self.inner.publish(count, deadline)?;
        self.wait = Some(ActivePollWait::new(self.now, deadline)?);
        Ok(pending)
    }
    fn poll_final(&mut self, pending: &mut usize) -> Result<bool> {
        self.inner.poll_final(pending)
    }
    fn validate_all(&mut self, pending: &usize) -> Result<()> {
        self.inner.validate_all(pending)
    }
    fn complete(&mut self, pending: usize) -> Result<()> {
        self.inner.complete(pending)
    }
    fn pause(&mut self) -> Result<()> {
        self.inner.event("pause".into())?;
        let action = self.wait.as_mut().unwrap().next_action(self.now)?;
        self.actions.push(action);
        // Advance an injected clock without sleeping or weakening the real loop.
        self.now += Duration::from_millis(10);
        Ok(())
    }
    fn poison(&mut self) {
        self.inner.poison()
    }
}

#[test]
fn active_and_fallback_pauses_preserve_the_complete_ordered64_lifecycle() {
    for count in [1, 16, 17, 63, 64] {
        let mut fake = ActiveFake::new(None);
        run_ordered_batch_mode(&mut fake, count, 600_000, OrderedMode::Batch64).unwrap();
        assert_eq!(
            fake.actions,
            [
                PauseAction::Spin,
                PauseAction::Sleep(Duration::from_micros(50))
            ]
        );
        assert!(!fake.inner.poisoned);
        assert!(fake.inner.completed);
        assert_eq!(fake.inner.retained, count);
        assert_eq!(fake.inner.events.first().unwrap(), "dispatch_fence");
        assert_eq!(fake.inner.events.last().unwrap(), "dispatch_fence");
        assert_eq!(
            fake.inner
                .events
                .iter()
                .filter(|event| event.starts_with("poll_final"))
                .count(),
            3
        );
        assert_eq!(
            fake.inner
                .events
                .iter()
                .filter(|event| event.starts_with("validate_signal:"))
                .count(),
            count
        );
        assert_eq!(
            fake.inner
                .events
                .iter()
                .filter(|event| *event == "doorbell")
                .count(),
            1
        );
    }
}

#[test]
fn active_policy_failure_at_every_lifecycle_step_preserves_poison_and_custody() {
    for count in [1, 64] {
        let mut success = ActiveFake::new(None);
        run_ordered_batch_mode(&mut success, count, 600_000, OrderedMode::Batch64).unwrap();
        for fail_at in 0..success.inner.events.len() {
            let mut fake = ActiveFake::new(Some(fail_at));
            assert!(
                run_ordered_batch_mode(&mut fake, count, 600_000, OrderedMode::Batch64).is_err()
            );
            assert!(fake.inner.poisoned);
            assert_eq!(fake.inner.events, success.inner.events[..=fail_at]);
            if fake
                .inner
                .events
                .iter()
                .any(|event| event == "write_reservation")
            {
                assert_eq!(fake.inner.retained, count);
            }
            let before = fake.inner.events.clone();
            assert!(
                run_ordered_batch_mode(&mut fake, count, 600_000, OrderedMode::Batch64).is_err()
            );
            assert_eq!(fake.inner.events, before);
        }
    }
}
