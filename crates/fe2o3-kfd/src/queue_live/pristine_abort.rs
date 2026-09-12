//! Separate unpublished provenance and retained abort custody for each lane.

use super::super::dispatch_binding::pristine_abort::PristineAbortBuffersV1;
use super::*;

type AbortCleanupV1 = Option<std::thread::Result<Result<(), Gfx942DispatchBindingErrorV1>>>;
type AbortEnvelopeV1 =
    Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>;

#[derive(Default)]
pub(super) struct UnpublishedDispatchStateV1 {
    pub(super) continuation: Option<PristineDispatchContinuationV1>,
    terminal_abort: Option<PristineDispatchAbortV1>,
}

impl UnpublishedDispatchStateV1 {
    pub(super) fn is_clear(&self) -> bool {
        self.continuation.is_none() && self.terminal_abort.is_none()
    }

    pub(super) fn is_detached(&self) -> bool {
        self.continuation.is_some() && self.terminal_abort.is_none()
    }

    pub(super) fn quiescent(
        &self,
        completion_releasable: bool,
        attached: bool,
        recycled: Option<u64>,
        count: usize,
        identities: usize,
        insertion: Option<usize>,
    ) -> bool {
        self.is_detached()
            && completion_releasable
            && !attached
            && recycled.is_none()
            && count <= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1
            && count == identities
            && insertion.is_none_or(|index| index <= count)
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Releases only the code and kernarg of a strictly pristine ordinary recipe.
    ///
    /// Returns every mapped data input in its original order and initialization
    /// state, including unused inputs. The queue privately retains its exact next
    /// generation for `bind_fixed_dispatch`; no completion receipt is produced.
    /// Any reservation history, persistent attachment or completion pin rejects
    /// before control disposal. Once disposal starts, failure is terminal and the
    /// complete data roster remains in queue custody until process teardown.
    pub fn abort_unpublished_fixed_dispatch_v1(
        &mut self,
    ) -> Result<Vec<Gfx942FixedDispatchDataV1>, ComputeAqlQueueSessionErrorV1> {
        self.abort_unpublished_with_v1(
            |session, buffers, dispatch, abort, cleanup| {
                session.with_live_queue_memory_model_custody(|memory| {
                    *abort = Some(
                        dispatch
                            .take()
                            .expect("one pristine abort owner")
                            .begin_pristine_abort_v1(buffers),
                    );
                    // Keep the cleanup panic outside retake, including a second panic there.
                    *cleanup = Some(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || {
                            abort
                                .as_mut()
                                .expect("abort rooted before disposal")
                                .release_controls(memory)
                        },
                    )));
                })
            },
            permanently_poison_process_global_kfd_runtime_gate_v1,
        )
    }

    fn abort_unpublished_with_v1(
        &mut self,
        run: impl FnOnce(
            &mut Self,
            PristineAbortBuffersV1,
            &mut Option<DispatchResourceOwnerV1>,
            &mut Option<PristineDispatchAbortV1>,
            &mut AbortCleanupV1,
        ) -> AbortEnvelopeV1,
        poison_process: impl FnOnce(),
    ) -> Result<Vec<Gfx942FixedDispatchDataV1>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1()
            || !self.unpublished_dispatch.is_clear()
            || self.detached_data_count != 0
            || self.detached_dispatch_generation.is_some()
            || !self.detached_data_identities.is_empty()
            || self.detached_next_insertion_index.is_some()
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        self.completion_owner.ensure_releasable()?;
        let buffers = self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .prepare_pristine_abort_v1()?;
        let mut dispatch = self.dispatch.take();
        let mut abort = None;
        let mut cleanup = None;
        let envelope = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(self, buffers, &mut dispatch, &mut abort, &mut cleanup)
        }));
        self.dispatch = dispatch;
        self.unpublished_dispatch.terminal_abort = abort;
        let cleanup = match cleanup {
            Some(Err(payload)) => {
                self.poison_terminal();
                poison_process();
                std::panic::resume_unwind(payload)
            }
            Some(Ok(result)) => Some(result),
            None => None,
        };
        let closing = match envelope {
            Err(payload) => {
                self.poison_terminal();
                poison_process();
                std::panic::resume_unwind(payload)
            }
            Ok(Err(error)) => return Err(error), // Loan rejected; attached owner was restored.
            Ok(Ok(((), closing))) => closing,
        };
        if let Err(error) = closing {
            self.poison_terminal();
            poison_process();
            return Err(error);
        }
        if let Err(error) = cleanup.expect("opened loan executes cleanup") {
            self.poison_terminal();
            poison_process();
            return Err(error.into());
        }
        let (continuation, data, identities) = self
            .unpublished_dispatch
            .terminal_abort
            .take()
            .expect("successful abort retains data")
            .into_detached();
        self.unpublished_dispatch.continuation = Some(continuation);
        self.detached_data_count = data.len();
        self.detached_data_identities = identities;
        self.detached_next_insertion_index = None;
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        compute_lane_state_for_multi_inflight_test, persistent_compute_cancellation_test_session,
    };
    use super::*;
    use crate::queue::dispatch_binding::pristine_abort::pristine_dispatch_fixture_v1;
    use crate::shared_memory::PristineAbortMemoryFixtureV1;

    fn fixture() -> (PristineAbortMemoryFixtureV1, ComputeAqlQueueSessionV1) {
        let (memory, owner) = pristine_dispatch_fixture_v1(8);
        let queue = fe2o3_runtime_model::QueueKeyV1 {
            vm: fe2o3_runtime_model::VmKeyV1 {
                device: fe2o3_runtime_model::DeviceKeyV1 {
                    physical: fe2o3_runtime_model::PhysicalDeviceIdV1(1),
                    generation: fe2o3_runtime_model::DeviceGenerationV1(1),
                },
                id: fe2o3_runtime_model::VmIdV1(1),
            },
            id: fe2o3_runtime_model::QueueInstanceIdV1(1),
            generation: fe2o3_runtime_model::QueueGenerationV1(1),
        };
        let mut session = persistent_compute_cancellation_test_session(queue, None, None);
        session.dispatch = Some(owner);
        (memory, session)
    }

    fn abort_with(
        memory: &mut PristineAbortMemoryFixtureV1,
        session: &mut ComputeAqlQueueSessionV1,
        closing: u8,
    ) -> Result<Vec<Gfx942FixedDispatchDataV1>, ComputeAqlQueueSessionErrorV1> {
        let gated = std::cell::Cell::new(false);
        let result = session.abort_unpublished_with_v1(
            |_, buffers, dispatch, abort, cleanup| {
                if closing == 3 {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract("loan rejected"));
                }
                *abort = Some(dispatch.take().unwrap().begin_pristine_abort_v1(buffers));
                *cleanup = Some(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || abort.as_mut().unwrap().release_controls(memory),
                )));
                match closing {
                    0 => Ok(((), Ok(()))),
                    1 => Ok((
                        (),
                        Err(ComputeAqlQueueSessionErrorV1::Contract(
                            "closing retake failed",
                        )),
                    )),
                    _ => std::panic::panic_any("closing retake panic"),
                }
            },
            || gated.set(true),
        );
        if result.is_err() && session.unpublished_dispatch.terminal_abort.is_some() {
            assert!(gated.get(), "post-effect error must gate the process");
        }
        result
    }

    #[test]
    fn pristine_abort_commits_complete_ledger_only_after_closing_success() {
        let (mut memory, mut session) = fixture();
        let data = abort_with(&mut memory, &mut session, 0).unwrap();
        assert_eq!(data.len(), 5);
        assert!(session.dispatch.is_none());
        assert!(session.unpublished_dispatch.is_detached());
        assert!(session.detached_dispatch_generation.is_none());
        assert_eq!(session.detached_data_count, 5);
        assert_eq!(
            session.detached_data_identities,
            fixed_dispatch_storage_identities(&data)
        );
        assert!(!session.terminal_poisoned);
        session.require_unbound_fixed_dispatch().unwrap();
        assert!(session.detach_recycled_fixed_dispatch().is_err());
        assert!(
            session
                .release_retained_persistent_fixed_dispatch_control_v1()
                .is_err()
        );
        assert!(!session.terminal_poisoned);
    }

    #[test]
    fn pristine_abort_loan_rejection_restores_attached_owner_without_effects() {
        let (mut memory, mut session) = fixture();
        let calls = memory.native_calls();
        assert!(abort_with(&mut memory, &mut session, 3).is_err());
        assert!(session.dispatch.is_some());
        assert!(session.unpublished_dispatch.is_clear());
        assert!(!session.terminal_poisoned);
        assert_eq!(memory.native_calls(), calls);
        assert_eq!(abort_with(&mut memory, &mut session, 0).unwrap().len(), 5);
    }

    #[test]
    fn pristine_abort_closing_failure_keeps_complete_terminal_custody() {
        for closing in [1, 2] {
            let (mut memory, mut session) = fixture();
            let usage = memory.usage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                abort_with(&mut memory, &mut session, closing)
            }));
            if closing == 1 {
                assert!(result.unwrap().is_err());
            } else {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"closing retake panic")
                );
            }
            assert_eq!(memory.freed(), 3);
            assert_eq!(memory.usage(), usage);
            assert!(session.terminal_poisoned);
            assert!(session.dispatch.is_none());
            assert!(session.unpublished_dispatch.terminal_abort.is_some());
            assert!(!session.unpublished_dispatch.is_detached());
            assert!(session.detached_dispatch_generation.is_none());
            assert!(session.detached_data_identities.is_empty());
            let calls = memory.native_calls();
            assert!(session.abort_unpublished_fixed_dispatch_v1().is_err());
            assert_eq!(memory.native_calls(), calls);
        }
    }

    #[test]
    fn pristine_abort_cleanup_error_terminalizes_queue_and_process() {
        let (mut memory, mut session) = fixture();
        memory.fail("unmap_gpu", false);
        assert!(abort_with(&mut memory, &mut session, 0).is_err());
        assert!(session.terminal_poisoned);
        assert!(session.unpublished_dispatch.terminal_abort.is_some());
        assert!(memory.data_is_retained());
    }

    #[test]
    fn pristine_abort_cleanup_panic_survives_second_panic_and_keeps_custody() {
        let (mut memory, mut session) = fixture();
        memory.fail("free", true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            abort_with(&mut memory, &mut session, 2)
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "free"))
        );
        assert!(session.terminal_poisoned);
        assert!(session.unpublished_dispatch.terminal_abort.is_some());
        assert!(memory.data_is_retained());
    }

    #[test]
    fn pristine_abort_auxiliary_state_restores_on_success_and_panic() {
        for panic in [false, true] {
            let (mut memory, mut session) = fixture();
            let primary = session.key;
            let mut auxiliary = primary;
            auxiliary.id = fe2o3_runtime_model::QueueInstanceIdV1(2);
            let mut state = compute_lane_state_for_multi_inflight_test(auxiliary);
            state.dispatch = session.dispatch.take();
            session
                .auxiliary_compute_lanes
                .push(AuxiliaryComputeLaneSlotV1 {
                    generation: 7,
                    state: Some(state),
                });
            let lane = ComputeAqlQueueLaneV1 {
                session: primary,
                ordinal: 1,
                generation: 7,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session.with_compute_lane_v1(lane, |lane| {
                    abort_with(&mut memory, lane.session, if panic { 2 } else { 0 })
                })
            }));
            assert_eq!(result.is_err(), panic);
            if panic {
                assert!(take_lane_unwind_process_gate_record_v1());
            }
            assert_eq!(session.key, primary);
            assert!(session.unpublished_dispatch.is_clear());
            let state = session.auxiliary_compute_lanes[0].state.as_ref().unwrap();
            assert_eq!(state.key, auxiliary);
            if panic {
                assert!(state.unpublished_dispatch.terminal_abort.is_some());
            } else {
                assert!(state.unpublished_dispatch.is_detached());
                assert_eq!(state.detached_data_count, 5);
                assert!(auxiliary_compute_lanes_are_quiescent_v1(
                    &session.auxiliary_compute_lanes
                ));
                assert!(preflight_auxiliary_compute_lane_destroy_v1(state).is_err());
            }
        }
    }

    #[test]
    fn pristine_detached_quiescence_rejects_mixed_or_incomplete_state() {
        let (mut memory, mut session) = fixture();
        let data = abort_with(&mut memory, &mut session, 0).unwrap();
        let state = &session.unpublished_dispatch;
        assert!(state.quiescent(true, false, None, 5, 5, None));
        assert!(!state.quiescent(false, false, None, 5, 5, None));
        assert!(!state.quiescent(true, true, None, 5, 5, None));
        assert!(!state.quiescent(true, false, Some(0), 5, 5, None));
        assert!(!state.quiescent(true, false, Some(7), 5, 5, None));
        assert!(!state.quiescent(true, false, None, 5, 4, None));
        assert!(!state.quiescent(true, false, None, 5, 5, Some(6)));
        assert!(state.quiescent(true, false, None, 0, 0, Some(0)));
        assert!(
            session
                .destroy_queue_and_event(QueueDestroyModeV1::ReturnDetached(data))
                .is_err()
        );
        assert!(!session.terminal_poisoned);
    }

    #[test]
    fn pristine_abort_rejects_bad_ledger_and_unreleasable_completion_before_cleanup() {
        for mutation in 0..3 {
            let (mut memory, mut session) = fixture();
            let calls = memory.native_calls();
            match mutation {
                0 => session.detached_dispatch_generation = Some(0),
                1 => session.detached_data_count = 1,
                _ => session.completion_owner.poison_owner(),
            }
            assert!(abort_with(&mut memory, &mut session, 0).is_err());
            assert!(session.dispatch.is_some());
            assert!(session.unpublished_dispatch.is_clear());
            assert_eq!(memory.native_calls(), calls);
        }
    }

    #[test]
    fn pristine_consuming_rebind_preflight_failure_is_terminal_without_native_calls() {
        let (mut memory, mut session) = fixture();
        let data = abort_with(&mut memory, &mut session, 0).unwrap();
        let calls = memory.native_calls();
        assert!(session.bind_fixed_dispatch(Vec::new(), [], data).is_err());
        assert!(session.terminal_poisoned);
        assert!(session.unpublished_dispatch.continuation.is_none());
        assert!(session.dispatch.is_none());
        assert!(memory.data_is_retained());
        assert_eq!(memory.native_calls(), calls);
    }

    #[test]
    fn pristine_rebind_settlement_roots_prepared_owner_before_closing_or_validation_failure() {
        super::super::rebind_tests::preparation::pristine_settlement_regression_v1();
    }

    #[test]
    fn pristine_rebind_constructor_rejection_is_terminal_without_retry_authority() {
        super::super::rebind_tests::preparation::pristine_constructor_regression_v1();
    }
}
