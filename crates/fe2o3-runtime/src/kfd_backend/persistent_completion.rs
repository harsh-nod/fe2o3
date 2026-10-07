//! Scalar completion keeps logical custody indexed while native calls consume receipts.

use super::*;

// Keep scripted owners inline in the preallocated root, just like native receipts.
#[cfg_attr(test, allow(clippy::large_enum_variant))]
pub(super) enum IndexedPersistentCompletionReceiptV1<P, R, D, I, E> {
    Reserved,
    Published(P),
    Recycled(R),
    Detached(D),
    Retired(I, E),
    NativeOwned,
    // Explicit lower failure returned no receipt: the queue quarantined it.
    LowerTerminalOwned,
    TerminalRooted,
    Restored,
    #[cfg(test)]
    Scripted(ScriptedCompletionOwnersV1),
}

// Scripted owners stay inline in the already boxed completion root.
#[cfg(test)]
#[allow(clippy::large_enum_variant)]
pub(super) enum ScriptedCompletionOwnersV1 {
    Single(Box<DirectionalSdmaDeviceOwnerV1>),
    Three([DirectionalSdmaDeviceOwnerV1; 3]),
}

pub(super) type PersistentCompletionReceiptV1 = IndexedPersistentCompletionReceiptV1<
    Gfx942PersistentComputeDispatchV1,
    Gfx942RecycledPersistentComputeDispatchV1,
    fe2o3_kfd::Gfx942PersistentComputeCompletedV1,
    KfdRuntimePersistentComputeInputV1,
    Gfx942PersistentComputeEffectV1,
>;

pub(super) struct PersistentComputeCompletionV1 {
    pub(super) admission: PersistentFullRangeComputeAdmissionV1,
    // After binding, None denotes InitializedStorage's record-owned shell.
    pub(super) shell: Option<ThreeBindingPersistentRestoreShellV1>,
    pub(super) receipt: PersistentCompletionReceiptV1,
    observed_at: Option<Instant>,
    signal_recycle: Duration,
}

impl KfdRuntimeBackendV1 {
    pub(super) fn reserve_persistent_completion_v1(
        &mut self,
        admission: PersistentFullRangeComputeAdmissionV1,
    ) -> Result<Box<PersistentComputeCompletionV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        #[cfg(test)]
        if self.scripted_persistent_transition_failure
            == Some(ScriptedPersistentTransitionFailureV1::ReserveCompletion)
        {
            self.scripted_persistent_transition_failure = None;
            return Err(Self::capacity("scripted completion reservation failure"));
        }
        // Successful read/replay binding donates its original empty Box. Only
        // writes from authenticated input change to a different native type.
        let shell = if admission.source == PersistentFullRangeComputeSourceV1::AuthenticatedH2d
            && admission.access != RuntimeAccessV1::Read
        {
            Some(ThreeBindingPersistentRestoreShellV1 {
                initialized: None,
                ready: None,
                device: None,
                replay: Some(try_uninit_box_v1().map_err(|_| {
                    Self::capacity("KFD scalar completion replay shell allocation failed")
                })?),
            })
        } else {
            None
        };
        let root = try_uninit_box_v1()
            .map_err(|_| Self::capacity("KFD persistent completion root allocation failed"))?;
        Ok(fill_restore_shell_v1(
            root,
            PersistentComputeCompletionV1 {
                admission,
                shell,
                receipt: PersistentCompletionReceiptV1::Reserved,
                observed_at: None,
                signal_recycle: Duration::ZERO,
            },
        ))
    }

    pub(super) fn install_scalar_completion_shell_v1(
        &mut self,
        restoration: PersistentComputeBindRestoreV1,
    ) {
        let root = match self
            .active
            .as_mut()
            .and_then(|active| active.execution.as_mut())
        {
            Some(ActiveComputeExecutionV1::PersistentPrepared { completion, .. }) => completion,
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { completion, .. }) => {
                completion
            }
            _ => unreachable!("successful scalar bind is indexed before its empty box handoff"),
        };
        if let Some(mut original) = restoration.restore_shell {
            if let Some(alternate) = root.shell.take() {
                original.replay = alternate.replay;
            }
            root.shell = Some(original);
        }
    }

    pub(super) fn scalar_completion_selected_v1(&self) -> bool {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(
                ActiveComputeExecutionV1::Persistent { .. }
                | ActiveComputeExecutionV1::PersistentCompleting(_),
            ) => true,
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedPersistent { .. }) => true,
            _ => false,
        }
    }

    pub(super) fn persistent_completion_root_v1(&self) -> &PersistentComputeCompletionV1 {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(ActiveComputeExecutionV1::PersistentCompleting(root)) => root,
            _ => unreachable!("indexed scalar completion root"),
        }
    }

    fn persistent_completion_root_mut_v1(&mut self) -> &mut PersistentComputeCompletionV1 {
        match self
            .active
            .as_mut()
            .and_then(|active| active.execution.as_mut())
        {
            Some(ActiveComputeExecutionV1::PersistentCompleting(root)) => root,
            _ => unreachable!("indexed scalar completion root"),
        }
    }

    fn take_completion_receipt_v1(&mut self) -> PersistentCompletionReceiptV1 {
        core::mem::replace(
            &mut self.persistent_completion_root_mut_v1().receipt,
            PersistentCompletionReceiptV1::NativeOwned,
        )
    }

    pub(super) fn advance_scalar_completion_v1(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.scalar_completion_preflight_v1() {
            return Err(
                self.terminal_error("scalar completion custody changed before native effects")
            );
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let active = self.active.as_mut().expect("selected scalar completion");
            let root = match active.execution.take().unwrap() {
                ActiveComputeExecutionV1::Persistent {
                    dispatch,
                    mut completion,
                    ..
                } => {
                    completion.receipt = PersistentCompletionReceiptV1::Published(dispatch);
                    completion
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedPersistent {
                    device,
                    mut completion,
                    ..
                } => {
                    completion.receipt = PersistentCompletionReceiptV1::Scripted(
                        ScriptedCompletionOwnersV1::Single(device),
                    );
                    completion
                }
                execution => {
                    active.execution = Some(execution);
                    return Err(self.terminal_error(
                        "scalar completion cannot resume an interrupted transition",
                    ));
                }
            };
            active.execution = Some(ActiveComputeExecutionV1::PersistentCompleting(root));
            self.advance_indexed_scalar_completion_v1(deadline)
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1())
            }
        }
    }

    fn scalar_completion_preflight_v1(&self) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        let (allocation, access, root) = match active.execution.as_ref() {
            Some(ActiveComputeExecutionV1::Persistent {
                allocation,
                access,
                completion,
                ..
            }) => (*allocation, *access, completion),
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedPersistent {
                allocation,
                access,
                completion,
                ..
            }) => (*allocation, *access, completion),
            _ => return false,
        };
        if self.selected_compute_lane != 0
            || !self.persistent_compute_custody_intact_v1(active.id, false)
            || self.terminal_sdma_custody.is_some()
        {
            return false;
        }
        self.scalar_completion_reservation_intact_v1(
            root,
            PersistentFullRangeComputeAdmissionV1 {
                allocation,
                access,
                source: root.admission.source,
            },
        )
    }

    pub(super) fn scalar_completion_reservation_intact_v1(
        &self,
        root: &PersistentComputeCompletionV1,
        expected: PersistentFullRangeComputeAdmissionV1,
    ) -> bool {
        if !matches!(root.receipt, PersistentCompletionReceiptV1::Reserved)
            || root.admission.allocation != expected.allocation
            || root.admission.access != expected.access
            || root.admission.source != expected.source
        {
            return false;
        }
        let Some(record) = self.allocations.get(&expected.allocation) else {
            return false;
        };
        let shell =
            if root.admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage {
                if root.shell.is_some() {
                    return false;
                }
                record.persistent_storage_restore.as_ref()
            } else {
                if record.persistent_storage_restore.is_some() {
                    return false;
                }
                root.shell.as_ref()
            };
        shell.is_some_and(|shell| shell.supports_origin_v1(root.admission))
    }

    fn scalar_completion_pending_v1(&mut self) -> BackendPollV1 {
        let active = self.active.as_mut().unwrap();
        let Some(ActiveComputeExecutionV1::PersistentCompleting(mut completion)) =
            active.execution.take()
        else {
            unreachable!("selected scalar completion");
        };
        let admission = completion.admission;
        let receipt = core::mem::replace(
            &mut completion.receipt,
            PersistentCompletionReceiptV1::Reserved,
        );
        active.execution = Some(match receipt {
            PersistentCompletionReceiptV1::Published(dispatch) => {
                ActiveComputeExecutionV1::Persistent {
                    allocation: admission.allocation,
                    access: admission.access,
                    dispatch,
                    completion,
                }
            }
            #[cfg(test)]
            PersistentCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Single(device)) => {
                ActiveComputeExecutionV1::ScriptedPersistent {
                    allocation: admission.allocation,
                    access: admission.access,
                    device,
                    completion,
                }
            }
            _ => unreachable!("pending completion preserves published receipt"),
        });
        BackendPollV1::Pending
    }

    fn retain_completion_failure_v1(&mut self, custody: KfdRuntimeTerminalSdmaCustodyV1) {
        self.retain_terminal_sdma_custody_v1(custody);
        self.persistent_completion_root_mut_v1().receipt =
            PersistentCompletionReceiptV1::TerminalRooted;
    }

    #[allow(clippy::result_large_err)]
    fn advance_indexed_scalar_completion_v1(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        if matches!(
            self.persistent_completion_root_v1().receipt,
            PersistentCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Single(_))
        ) {
            return self.advance_scripted_scalar_completion_v1(deadline);
        }
        if self.queue.is_none() {
            return Err(self.terminal_error("published persistent submission lost its KFD queue"));
        }
        let PersistentCompletionReceiptV1::Published(dispatch) = self.take_completion_receipt_v1()
        else {
            unreachable!("new scalar completion owns its published receipt");
        };
        let queue = self.queue.as_mut().unwrap();
        let poll = match deadline {
            None => queue.poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch),
            Some(deadline) => queue
                .wait_and_recycle_directional_persistent_fixed_dispatch_until_v1(dispatch, deadline)
                .map(|wait| match wait {
                    Gfx942PersistentComputeWaitAndRecycleV1::Timeout { dispatch, .. } => {
                        Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch)
                    }
                    Gfx942PersistentComputeWaitAndRecycleV1::Recycled {
                        recycled,
                        completion_observed_at,
                        ..
                    } => Gfx942PersistentComputePollAndRecycleV1::Recycled {
                        recycled,
                        completion_observed_at,
                    },
                }),
        };
        self.finish_indexed_persistent_poll_v1(poll)
    }

    #[allow(clippy::result_large_err)]
    fn finish_indexed_persistent_poll_v1(
        &mut self,
        poll: Result<
            Gfx942PersistentComputePollAndRecycleV1,
            Gfx942PersistentComputePollAndRecycleFailureV1,
        >,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let observed_at = match poll {
            Ok(Gfx942PersistentComputePollAndRecycleV1::Pending(dispatch)) => {
                self.persistent_completion_root_mut_v1().receipt =
                    PersistentCompletionReceiptV1::Published(dispatch);
                return Ok(self.scalar_completion_pending_v1());
            }
            Ok(Gfx942PersistentComputePollAndRecycleV1::Recycled {
                recycled,
                completion_observed_at,
            }) => {
                self.persistent_completion_root_mut_v1().receipt =
                    PersistentCompletionReceiptV1::Recycled(recycled);
                completion_observed_at
            }
            Err(Gfx942PersistentComputePollAndRecycleFailureV1::Poll(failure)) => {
                let (error, custody) = failure.into_parts();
                self.retain_completion_failure_v1(match custody {
                    Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(dispatch) => {
                        KfdRuntimeTerminalSdmaCustodyV1::PersistentComputePublished(dispatch)
                    }
                    Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) => {
                        KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody)
                    }
                });
                return Err(self.terminal_error(format!(
                    "KFD persistent-compute completion observation: {error}"
                )));
            }
            Err(Gfx942PersistentComputePollAndRecycleFailureV1::Recycle(failure)) => {
                let (error, custody) = failure.into_parts();
                self.retain_completion_failure_v1(match custody {
                    Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(completed) => {
                        KfdRuntimeTerminalSdmaCustodyV1::PersistentComputeCompleted(completed)
                    }
                    Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) => {
                        KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody)
                    }
                });
                return Err(self.terminal_error(format!(
                    "KFD persistent-compute completion recycle: {error}"
                )));
            }
        };
        let active = self.active.as_mut().unwrap();
        active.performance.publish_to_completion =
            observed_at.saturating_duration_since(active.published_at);
        let signal_recycle = observed_at.elapsed();
        active.performance.completion_signal_recycle += signal_recycle;
        let root = self.persistent_completion_root_mut_v1();
        root.observed_at = Some(observed_at);
        root.signal_recycle = signal_recycle;
        let PersistentCompletionReceiptV1::Recycled(recycled) = self.take_completion_receipt_v1()
        else {
            unreachable!("indexed recycled receipt")
        };
        let detach = self
            .queue
            .as_mut()
            .unwrap()
            .detach_recycled_directional_persistent_fixed_dispatch_v1(recycled);
        match detach {
            Ok(completed) => {
                self.persistent_completion_root_mut_v1().receipt =
                    PersistentCompletionReceiptV1::Detached(completed)
            }
            Err(failure) => {
                let (error, custody) = failure.into_parts();
                self.retain_completion_failure_v1(match custody {
                    Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(recycled) => {
                        KfdRuntimeTerminalSdmaCustodyV1::PersistentComputeRecycled(recycled)
                    }
                    Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) => {
                        KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody)
                    }
                });
                return Err(self
                    .terminal_error(format!("KFD persistent-compute completion detach: {error}")));
            }
        }
        let PersistentCompletionReceiptV1::Detached(completed) = self.take_completion_receipt_v1()
        else {
            unreachable!("indexed detached receipt")
        };
        match completed.retire_settled_frontier_for_replay_v1() {
            Ok((input, effect)) => {
                self.persistent_completion_root_mut_v1().receipt =
                    PersistentCompletionReceiptV1::Retired(
                        KfdRuntimePersistentComputeInputV1::Native(input),
                        effect,
                    )
            }
            Err(failure) => {
                self.retain_completion_failure_v1(
                    KfdRuntimeTerminalSdmaCustodyV1::ComputeRetirement(failure),
                );
                return Err(self.terminal_error(
                    "KFD persistent-compute completion frontier retirement failed",
                ));
            }
        }
        self.restore_indexed_scalar_completion_v1()?;
        self.commit_indexed_scalar_completion_v1()
    }

    fn restore_indexed_scalar_completion_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.persistent_completion_root_v1();
        let admission = root.admission;
        let PersistentCompletionReceiptV1::Retired(input, effect) = &root.receipt else {
            unreachable!("indexed retired input")
        };
        if *effect != persistent_compute_effect_v1(admission.access) {
            return Err(self
                .terminal_error("KFD persistent-compute effect changed after metadata admission"));
        }
        let id = self.active.as_ref().unwrap().id;
        let valid = self.allocations.get(&admission.allocation).is_some_and(|record| {
            if !matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == id) {
                return false;
            }
            let shell = match admission.source {
                PersistentFullRangeComputeSourceV1::InitializedStorage if root.shell.is_none() => record.persistent_storage_restore.as_ref(),
                PersistentFullRangeComputeSourceV1::InitializedStorage => None,
                _ if record.persistent_storage_restore.is_none() => root.shell.as_ref(),
                _ => None,
            };
            shell.is_some_and(|shell| {
                #[cfg(test)]
                if matches!(input, KfdRuntimePersistentComputeInputV1::ScriptedReplay(_)) {
                    // The scalar scripted dispatcher normalizes even read-only inputs.
                    return shell.device.is_some();
                }
                shell.accepts_v1(admission, input)
            })
        });
        if !valid {
            return Err(
                self.terminal_error("persistent-compute completion slot/input/shell mismatch")
            );
        }
        // Everything below is an authenticated move into an already allocated box.
        let active = self.active.as_mut().unwrap();
        let Some(ActiveComputeExecutionV1::PersistentCompleting(root)) = active.execution.as_mut()
        else {
            unreachable!()
        };
        let record = self.allocations.get_mut(&admission.allocation).unwrap();
        let shell = if admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage {
            record.persistent_storage_restore.take().unwrap()
        } else {
            root.shell.take().unwrap()
        };
        let PersistentCompletionReceiptV1::Retired(input, effect) = core::mem::replace(
            &mut root.receipt,
            PersistentCompletionReceiptV1::NativeOwned,
        ) else {
            unreachable!()
        };
        record.sdma_storage = shell.restore_v1(input, None);
        root.receipt = PersistentCompletionReceiptV1::Restored;
        apply_persistent_compute_effect_v1(record, effect);
        Ok(())
    }

    fn commit_indexed_scalar_completion_v1(
        &mut self,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        if self.scripted_persistent_transition_failure
            == Some(ScriptedPersistentTransitionFailureV1::CompletionCommitRosterMismatch)
        {
            self.scripted_persistent_transition_failure = None;
            self.active.as_mut().unwrap().allocations.clear();
        }
        let root = self.persistent_completion_root_v1();
        let active = self.active.as_ref().unwrap();
        if !matches!(root.receipt, PersistentCompletionReceiptV1::Restored)
            || root.shell.is_some()
            || !self.persistent_compute_custody_intact_v1(active.id, true)
            || self
                .submissions
                .capacity()
                .saturating_sub(self.submissions.len())
                < self.compute_completion_reservations
            || self.terminal_sdma_custody.is_some()
            || !self
                .allocations
                .get(&root.admission.allocation)
                .is_some_and(|record| {
                    record.persistent_storage_restore.is_none()
                        && matches!(
                            record.sdma_storage,
                            KfdRuntimeSdmaStorageV1::H2dReady(_)
                                | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                                | KfdRuntimeSdmaStorageV1::Device(_)
                        )
                })
        {
            return Err(self.terminal_error("scalar completion lost logical settlement custody"));
        }
        let allocation = root.admission.allocation;
        let detach_restore = root.observed_at.map_or(Duration::ZERO, |start| {
            completion_detach_restore_duration_v1(start.elapsed(), root.signal_recycle)
        });
        let active = self.active.as_mut().unwrap();
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_detach_restore += detach_restore;
        debug_assert_eq!(active.performance.user_data_materializations, 0);
        let (id, stream, kernel, dependency_depth, shape, performance) = (
            active.id,
            active.stream,
            active.kernel,
            active.dependency_depth,
            active.dispatch_shape_sha256,
            active.performance,
        );
        self.retained_persistent_dispatch = Some(RetainedPersistentDispatchV1 {
            allocation,
            dispatch_shape_sha256: shape,
        });
        let module = self
            .kernels
            .get(&kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(id, module, core::iter::once(allocation));
        self.submissions.insert(
            id,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Succeeded,
                dependency_depth,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.release_compute_lane_lease_v1(stream, self.selected_compute_lane);
        self.last_launch_performance = Some(performance);
        let dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
        self.observe_profile_v1(dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(performance),
            }
        }));
        self.active = None;
        Ok(BackendPollV1::Succeeded)
    }

    #[cfg(test)]
    fn advance_scripted_scalar_completion_v1(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if let Some(deadline) = deadline {
            let mut attempts = 0_u32;
            let mut sleep = WAIT_INITIAL_SLEEP_V1;
            loop {
                self.scripted_persistent_wait_observations += 1;
                if self.scripted_persistent_wait_pending_observations == 0 {
                    break;
                }
                self.scripted_persistent_wait_pending_observations -= 1;
                if !apply_wait_backoff_v1(attempts, &mut sleep, deadline) {
                    return Ok(self.scalar_completion_pending_v1());
                }
                attempts = attempts.saturating_add(1);
            }
        }
        if matches!(
            self.scripted_persistent_transition_failure,
            Some(
                ScriptedPersistentTransitionFailureV1::Poll
                    | ScriptedPersistentTransitionFailureV1::Recycle
                    | ScriptedPersistentTransitionFailureV1::Detach
            )
        ) {
            self.scripted_persistent_transition_failure = None;
            let PersistentCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Single(device)) =
                self.take_completion_receipt_v1()
            else {
                unreachable!()
            };
            self.retain_completion_failure_v1(KfdRuntimeTerminalSdmaCustodyV1::Device(*device));
            return Err(self.terminal_error(
                "scripted persistent-compute completion returned foreign retryable custody",
            ));
        }
        let PersistentCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Single(device)) =
            self.take_completion_receipt_v1()
        else {
            unreachable!()
        };
        let (device, box_shell) = take_restore_shell_v1(device);
        let root = self.persistent_completion_root_mut_v1();
        let admission = root.admission;
        root.receipt = PersistentCompletionReceiptV1::Retired(
            KfdRuntimePersistentComputeInputV1::ScriptedReplay(device),
            persistent_compute_effect_v1(admission.access),
        );
        let shell = if admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage {
            self.allocations
                .get_mut(&admission.allocation)
                .unwrap()
                .persistent_storage_restore
                .as_mut()
                .unwrap()
        } else {
            root.shell.as_mut().unwrap()
        };
        shell.device = Some(box_shell);
        let active = self.active.as_mut().unwrap();
        active.performance.publish_to_completion = active.published_at.elapsed();
        match self.scripted_persistent_transition_failure.take() {
            Some(ScriptedPersistentTransitionFailureV1::UnwindAfterRetire) => {
                panic!("scripted scalar completion unwind after retirement")
            }
            Some(ScriptedPersistentTransitionFailureV1::CompletionSlotMismatch) => {
                self.allocations
                    .get_mut(&admission.allocation)
                    .unwrap()
                    .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(u64::MAX)
            }
            Some(ScriptedPersistentTransitionFailureV1::CompletionShellMismatch) => {
                if admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage {
                    self.allocations
                        .get_mut(&admission.allocation)
                        .unwrap()
                        .persistent_storage_restore = None;
                } else {
                    self.persistent_completion_root_mut_v1().shell = None;
                }
            }
            Some(ScriptedPersistentTransitionFailureV1::CompletionEffectMismatch) => {
                let PersistentCompletionReceiptV1::Retired(_, effect) =
                    &mut self.persistent_completion_root_mut_v1().receipt
                else {
                    unreachable!()
                };
                *effect =
                    persistent_compute_effect_v1(if admission.access == RuntimeAccessV1::Read {
                        RuntimeAccessV1::Write
                    } else {
                        RuntimeAccessV1::Read
                    });
            }
            fault => self.scripted_persistent_transition_failure = fault,
        }
        self.restore_indexed_scalar_completion_v1()?;
        self.commit_indexed_scalar_completion_v1()
    }
}
