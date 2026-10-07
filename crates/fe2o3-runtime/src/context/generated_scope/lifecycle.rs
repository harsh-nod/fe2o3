//! Single-attempt lifecycle shared by the lexical driver and CPU controls.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Adopting,
    Issuing,
    Completing,
    RetainedProducer,
    Settled,
    Cancelled,
    CancelledUnpublished,
    FailedUnpublished,
    Unknown,
}

pub(super) struct Lifecycle<T, O> {
    pub value: Option<T>,
    pub phase: Phase,
    pub outcome: Option<O>,
}

impl<T, O> Lifecycle<T, O> {
    pub fn new(value: T) -> Self {
        Self {
            value: Some(value),
            phase: Phase::Adopting,
            outcome: None,
        }
    }

    pub fn unsettled(&self) -> bool {
        !matches!(
            self.phase,
            Phase::Settled
                | Phase::Cancelled
                | Phase::CancelledUnpublished
                | Phase::FailedUnpublished
        )
    }

    pub fn cancel_before_adoption<E>(
        &mut self,
        release_hold: impl FnOnce() -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.phase != Phase::Adopting {
            return Ok(false);
        }
        // Disposal and exact hold release must both complete before any
        // cancellation observation. Unwind/refusal is permanently ambiguous.
        self.phase = Phase::Unknown;
        drop(self.value.take().expect("original never-adopted carrier"));
        release_hold()?;
        self.phase = Phase::Cancelled;
        Ok(true)
    }

    pub fn advance<E>(
        &mut self,
        perform: impl FnOnce(Phase, &mut T) -> Result<bool, E>,
        decode: impl FnOnce(T) -> O,
    ) -> Result<bool, E> {
        let phase = self.phase;
        if matches!(
            phase,
            Phase::Settled
                | Phase::Cancelled
                | Phase::CancelledUnpublished
                | Phase::FailedUnpublished
                | Phase::Unknown
        ) {
            return Ok(false);
        }
        // A failed or unwinding native call is never eligible for a second try.
        self.phase = Phase::Unknown;
        let advanced = perform(phase, self.value.as_mut().expect("retained scoped owner"))?;
        self.phase = match (phase, advanced) {
            (Phase::Adopting, false) => Phase::Adopting,
            (Phase::Adopting, true) => Phase::Issuing,
            (Phase::Issuing, false) => Phase::Issuing,
            (Phase::Issuing, true) => Phase::Completing,
            (Phase::Completing, true) => Phase::Settled,
            // Only the private completion hook may return false here: it has
            // retained the original producer but has not disposed DATA yet.
            (Phase::Completing, false) => Phase::RetainedProducer,
            (Phase::RetainedProducer, true) => Phase::Settled,
            _ => Phase::Unknown,
        };
        if self.phase == Phase::Settled {
            // Only host decoding remains after exact native and Context settlement.
            self.outcome = Some(decode(self.value.take().expect("settled scoped owner")));
        }
        Ok(advanced || self.phase == Phase::RetainedProducer)
    }

    pub fn cancel_unpublished<C, E>(
        &mut self,
        context: &mut C,
        retire_native: impl FnOnce(&mut C) -> Result<(), E>,
        release_hold: impl FnOnce(&mut C) -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.phase != Phase::Issuing {
            return Ok(false);
        }
        // The owning caller first checks exact original nonpublication. Neither
        // native disposal nor carrier destruction is retryable after entry.
        self.phase = Phase::Unknown;
        retire_native(context)?;
        drop(self.value.take().expect("original unpublished carrier"));
        release_hold(context)?;
        self.phase = Phase::CancelledUnpublished;
        Ok(true)
    }

    pub fn settle_rejected<C, E>(
        &mut self,
        context: &mut C,
        settle_native_and_context: impl FnOnce(&mut C, &mut T) -> Result<(), E>,
        release_hold: impl FnOnce(&mut C) -> Result<(), E>,
    ) -> Result<bool, E> {
        if self.phase != Phase::Issuing {
            return Ok(false);
        }
        // Classification alone is not settlement. Keep ambiguity until the
        // original native abort, source bracket, owner disposal and hold release
        // all return; no decoder or successful writer version is produced.
        self.phase = Phase::Unknown;
        let Some(value) = self.value.as_mut() else {
            std::process::abort()
        };
        settle_native_and_context(context, value)?;
        let Some(value) = self.value.take() else {
            std::process::abort()
        };
        drop(value);
        release_hold(context)?;
        self.phase = Phase::FailedUnpublished;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Disposal<'a> {
        dropped: &'a Cell<usize>,
        panic: bool,
    }
    impl Drop for Disposal<'_> {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
            assert!(!self.panic, "injected never-issued disposal failure");
        }
    }

    #[test]
    fn cancellation_disposes_original_before_hold_release_without_decoding() {
        let dropped = Cell::new(0);
        let mut slot = Lifecycle::<_, ()>::new(Disposal {
            dropped: &dropped,
            panic: false,
        });
        assert_eq!(
            slot.cancel_before_adoption(|| {
                assert_eq!(dropped.get(), 1);
                Ok::<_, ()>(())
            }),
            Ok(true)
        );
        assert_eq!(slot.phase, Phase::Cancelled);
        assert!(!slot.unsettled());
        assert!(slot.value.is_none() && slot.outcome.is_none());
        assert_eq!(
            slot.cancel_before_adoption(|| -> Result<(), ()> {
                panic!("cannot release twice");
            }),
            Ok(false)
        );
        assert_eq!(
            slot.advance::<()>(
                |_, _| panic!("cancelled owner cannot issue"),
                |_| panic!("cancelled owner cannot decode")
            ),
            Ok(false)
        );
    }

    #[test]
    fn cancellation_refusal_or_disposal_unwind_cannot_publish_cancelled_state() {
        for panic in [false, true] {
            let dropped = Cell::new(0);
            let released = Cell::new(false);
            let mut slot = Lifecycle::<_, ()>::new(Disposal {
                dropped: &dropped,
                panic,
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                slot.cancel_before_adoption(|| {
                    released.set(true);
                    Err(7)
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(7));
            }
            assert_eq!(dropped.get(), 1);
            assert_eq!(released.get(), !panic);
            assert_eq!(slot.phase, Phase::Unknown);
            assert!(slot.unsettled());
            assert!(slot.value.is_none() && slot.outcome.is_none());
            assert_eq!(
                slot.cancel_before_adoption(|| -> Result<(), i32> {
                    panic!("ambiguous release cannot retry");
                }),
                Ok(false)
            );
        }
    }

    #[test]
    fn borrowed_owners_settle_in_mixed_duration_round_robin_order() {
        let decoded = Cell::new(0);
        let mut slots: Vec<_> = [3, 0, 2]
            .into_iter()
            .map(|ticks| Lifecycle::new((ticks, &decoded)))
            .collect();
        let mut settled = Vec::new();
        for _ in 0..7 {
            for (index, slot) in slots.iter_mut().enumerate() {
                let was_live = slot.unsettled();
                slot.advance::<()>(
                    |phase, (ticks, _)| {
                        if phase == Phase::Issuing && *ticks != 0 {
                            *ticks -= 1;
                            return Ok(false);
                        }
                        Ok(true)
                    },
                    |(_, decoded)| decoded.set(decoded.get() + 1),
                )
                .unwrap();
                if was_live && !slot.unsettled() {
                    settled.push(index);
                }
            }
        }
        assert_eq!(settled, [1, 2, 0]);
        assert_eq!(decoded.get(), 3);
        assert!(slots.iter().all(|s| s.value.is_none()));
    }

    #[test]
    fn rejected_failure_observation_follows_original_settlement_and_drop() {
        let dropped = Cell::new(0);
        let mut slot = Lifecycle::<_, ()>::new(Disposal {
            dropped: &dropped,
            panic: false,
        });
        slot.phase = Phase::Issuing;
        let mut settled = false;
        assert_eq!(
            slot.settle_rejected(
                &mut settled,
                |settled, _| {
                    assert_eq!(dropped.get(), 0);
                    *settled = true;
                    Ok::<_, ()>(())
                },
                |settled| {
                    assert!(*settled);
                    assert_eq!(dropped.get(), 1);
                    Ok(())
                }
            ),
            Ok(true)
        );
        assert_eq!(slot.phase, Phase::FailedUnpublished);
        assert!(!slot.unsettled());
        assert!(slot.value.is_none() && slot.outcome.is_none());
        assert_eq!(
            slot.advance::<()>(
                |_, _| panic!("failed owner cannot issue"),
                |_| panic!("failed owner cannot decode")
            ),
            Ok(false)
        );
    }

    #[test]
    fn rejected_settlement_failure_retains_unknown_without_local_result() {
        for stage in 0..3 {
            let dropped = Cell::new(0);
            let mut slot = Lifecycle::<_, ()>::new(Disposal {
                dropped: &dropped,
                panic: stage == 1,
            });
            slot.phase = Phase::Issuing;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                slot.settle_rejected(
                    &mut (),
                    |_, _| {
                        if stage == 0 { Err(7) } else { Ok(()) }
                    },
                    |_| Err(8),
                )
            }));
            match stage {
                0 => assert_eq!(result.unwrap(), Err(7)),
                1 => assert!(result.is_err()),
                _ => assert_eq!(result.unwrap(), Err(8)),
            }
            assert_eq!(slot.phase, Phase::Unknown);
            assert!(slot.unsettled() && slot.outcome.is_none());
            assert_eq!(slot.value.is_some(), stage == 0);
            assert_eq!(dropped.get(), usize::from(stage != 0));
        }
    }

    #[test]
    fn refusal_and_unwind_retain_owner_and_never_retry_or_decode() {
        for phase in [
            Phase::Adopting,
            Phase::Issuing,
            Phase::Completing,
            Phase::RetainedProducer,
        ] {
            let owner = Cell::new(7);
            let mut slot = Lifecycle::<_, ()>::new(&owner);
            slot.phase = phase;
            assert_eq!(
                slot.advance(|_, _| Err(5), |_| panic!("must not decode")),
                Err(5)
            );
            assert_eq!(slot.phase, Phase::Unknown);
            assert!(slot.unsettled());
            assert!(std::ptr::eq(*slot.value.as_ref().unwrap(), &owner));
            assert_eq!(
                slot.advance::<()>(|_, _| panic!("must not retry"), |_| ()),
                Ok(false)
            );
            let mut slot = Lifecycle::<_, ()>::new(&owner);
            slot.phase = phase;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = slot.advance::<()>(|_, _| panic!("injected"), |_| ());
            }));
            assert!(result.is_err());
            assert_eq!(slot.phase, Phase::Unknown);
            assert!(slot.value.is_some());
        }
    }
}
