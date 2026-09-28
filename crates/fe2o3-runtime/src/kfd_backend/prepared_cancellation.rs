//! Indexed outer custody for cancellation of an unpublished persistent dispatch.

use super::*;

pub(super) enum PreparedCancellationReceiptV1 {
    Single(Gfx942PreparedPersistentComputeDispatchV1),
    Three(Gfx942PreparedThreeBindingPersistentComputeDispatchV1),
    // The lower queue owns cancellation custody until it returns typed inputs.
    NativeOwned,
    InputsReturned,
}

pub(super) struct PreparedCancellationSlotV1 {
    pub(super) admission: PersistentFullRangeComputeAdmissionV1,
    pub(super) promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
    pub(super) shell: Option<ThreeBindingPersistentRestoreShellV1>,
    pub(super) input: Option<KfdRuntimePersistentComputeInputV1>,
    pub(super) restored: bool,
}

pub(super) struct PreparedComputeCancellationV1 {
    pub(super) receipt: PreparedCancellationReceiptV1,
    pub(super) slots: [Option<PreparedCancellationSlotV1>; 3],
    // Publication provenance remains owned even when no native receipt returns.
    pub(super) _profile: PersistentPublicationProfileV1,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedPreparedCancelFaultV1 {
    Terminal,
    Unwind,
    ReturnedSlotMismatch(usize),
    AfterRestore(usize),
    BeforeCommit,
}

impl ThreeBindingPersistentRestoreShellV1 {
    pub(super) fn supports_origin_v1(
        &self,
        admission: PersistentFullRangeComputeAdmissionV1,
    ) -> bool {
        match admission.source {
            PersistentFullRangeComputeSourceV1::InitializedStorage => {
                self.initialized.is_some() && self.replay.is_some()
            }
            PersistentFullRangeComputeSourceV1::AuthenticatedH2d => {
                self.ready.is_some()
                    && (admission.access == RuntimeAccessV1::Read || self.replay.is_some())
            }
            PersistentFullRangeComputeSourceV1::RetainedControlReplay => {
                #[cfg(test)]
                if self.device.is_some() {
                    return true;
                }
                self.replay.is_some()
            }
        }
    }

    pub(super) fn accepts_v1(
        &self,
        admission: PersistentFullRangeComputeAdmissionV1,
        input: &KfdRuntimePersistentComputeInputV1,
    ) -> bool {
        use PersistentFullRangeComputeSourceV1 as Source;
        match (admission.source, input) {
            (
                Source::InitializedStorage,
                KfdRuntimePersistentComputeInputV1::Native(
                    Gfx942PersistentComputeInputV1::InitializedStorage(_),
                ),
            ) => self.initialized.is_some(),
            (
                Source::AuthenticatedH2d,
                KfdRuntimePersistentComputeInputV1::Native(
                    Gfx942PersistentComputeInputV1::Initialized(_),
                ),
            ) => self.ready.is_some(),
            (
                Source::InitializedStorage | Source::RetainedControlReplay,
                KfdRuntimePersistentComputeInputV1::Native(
                    Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_),
                ),
            ) => self.replay.is_some(),
            (
                Source::AuthenticatedH2d,
                KfdRuntimePersistentComputeInputV1::Native(
                    Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_),
                ),
            ) if admission.access != RuntimeAccessV1::Read => self.replay.is_some(),
            #[cfg(test)]
            (
                Source::InitializedStorage,
                KfdRuntimePersistentComputeInputV1::ScriptedStorage(_),
            ) => self.initialized.is_some(),
            #[cfg(test)]
            (Source::AuthenticatedH2d, KfdRuntimePersistentComputeInputV1::ScriptedReady(_)) => {
                self.ready.is_some()
            }
            #[cfg(test)]
            (
                Source::InitializedStorage | Source::RetainedControlReplay,
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(_),
            ) => self.device.is_some(),
            #[cfg(test)]
            (Source::AuthenticatedH2d, KfdRuntimePersistentComputeInputV1::ScriptedReplay(_))
                if admission.access != RuntimeAccessV1::Read =>
            {
                self.device.is_some()
            }
            _ => false,
        }
    }

    // Caller authenticates input/shell compatibility before extracting custody.
    pub(super) fn restore_v1(
        self,
        input: KfdRuntimePersistentComputeInputV1,
        promotion: Option<KfdRuntimeReadyPromotionPerformanceV1>,
    ) -> KfdRuntimeSdmaStorageV1 {
        match input {
            KfdRuntimePersistentComputeInputV1::Native(
                Gfx942PersistentComputeInputV1::Initialized(ready),
            ) => KfdRuntimeSdmaStorageV1::H2dReady(fill_restore_shell_v1(
                self.ready.expect("preflighted ready shell"),
                PersistentComputeReadyStorageV1 {
                    owner: PersistentComputeReadyOwnerV1::from_native(ready),
                    promotion,
                },
            )),
            KfdRuntimePersistentComputeInputV1::Native(
                input @ Gfx942PersistentComputeInputV1::InitializedAfterDispatch(_),
            ) => KfdRuntimeSdmaStorageV1::PersistentReplay(fill_restore_shell_v1(
                self.replay.expect("preflighted replay shell"),
                input,
            )),
            KfdRuntimePersistentComputeInputV1::Native(
                Gfx942PersistentComputeInputV1::InitializedStorage(ready),
            ) => KfdRuntimeSdmaStorageV1::InitializedStorage(fill_restore_shell_v1(
                self.initialized.expect("preflighted storage shell"),
                InitializedStorageOwnerV1::Native(ready),
            )),
            KfdRuntimePersistentComputeInputV1::Native(
                Gfx942PersistentComputeInputV1::Uninitialized(device),
            ) => KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(
                self.device.expect("preflighted device shell"),
                DirectionalSdmaDeviceOwnerV1::Native(device),
            )),
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => {
                KfdRuntimeSdmaStorageV1::H2dReady(fill_restore_shell_v1(
                    self.ready.expect("preflighted scripted ready shell"),
                    ready,
                ))
            }
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedStorage(device) => {
                KfdRuntimeSdmaStorageV1::InitializedStorage(fill_restore_shell_v1(
                    self.initialized
                        .expect("preflighted scripted storage shell"),
                    InitializedStorageOwnerV1::Scripted(device),
                ))
            }
            #[cfg(test)]
            KfdRuntimePersistentComputeInputV1::ScriptedReplay(device) => {
                KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(
                    self.device.expect("preflighted scripted device shell"),
                    device,
                ))
            }
        }
    }
}

impl KfdRuntimeBackendV1 {
    fn prepared_cancel_root_v1(&self) -> &PreparedComputeCancellationV1 {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(ActiveComputeExecutionV1::PersistentCancelling(root)) => root,
            _ => unreachable!("indexed prepared cancellation root"),
        }
    }

    fn prepared_cancel_root_mut_v1(&mut self) -> &mut PreparedComputeCancellationV1 {
        match self
            .active
            .as_mut()
            .and_then(|active| active.execution.as_mut())
        {
            Some(ActiveComputeExecutionV1::PersistentCancelling(root)) => root,
            _ => unreachable!("indexed prepared cancellation root"),
        }
    }

    fn persistent_compute_admissions_v1(
        &self,
    ) -> [Option<PersistentFullRangeComputeAdmissionV1>; 3] {
        let execution = self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref());
        match execution {
            Some(
                ActiveComputeExecutionV1::Persistent { completion, .. }
                | ActiveComputeExecutionV1::PersistentCompleting(completion),
            ) => [Some(completion.admission), None, None],
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedPersistent { completion, .. }) => {
                [Some(completion.admission), None, None]
            }
            Some(ActiveComputeExecutionV1::PersistentPrepared {
                allocation,
                access,
                source,
                ..
            }) => [
                Some(PersistentFullRangeComputeAdmissionV1 {
                    allocation: *allocation,
                    access: *access,
                    source: *source,
                }),
                None,
                None,
            ],
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared {
                allocation,
                access,
                source,
                ..
            }) => [
                Some(PersistentFullRangeComputeAdmissionV1 {
                    allocation: *allocation,
                    access: *access,
                    source: *source,
                }),
                None,
                None,
            ],
            Some(ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                admissions, ..
            }) => admissions.map(Some),
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                admissions,
                ..
            }) => admissions.map(Some),
            Some(ActiveComputeExecutionV1::PersistentCancelling(root)) => {
                std::array::from_fn(|index| root.slots[index].as_ref().map(|slot| slot.admission))
            }
            _ => [None; 3],
        }
    }

    pub(super) fn persistent_compute_custody_intact_v1(
        &self,
        submission: u64,
        restored: bool,
    ) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        let Some(kernel) = self.kernels.get(&active.kernel) else {
            return false;
        };
        let admissions = self.persistent_compute_admissions_v1();
        let count = admissions.iter().flatten().count();
        if active.id != submission
            || submission == 0
            || !matches!(count, 1 | 3)
            || admissions[0].is_none()
            || (count == 1 && admissions[1..].iter().any(Option::is_some))
            || active.allocations.len() != count
            || active.deferred_ordered_predecessor_retain
            || !self.compute_pipeline.is_empty()
            || self.stream_compute_lanes.get(&active.stream) != Some(&0)
            || self
                .stream_submission_tails
                .get(&active.stream)
                .is_none_or(|tail| *tail < submission)
            || self.compute_completion_reservations == 0
            || self.submissions.contains_key(&submission)
            || self.pending_compute.contains_key(&submission)
            || self.active_sdma.contains_key(&submission)
            || self
                .compute_module_retain_counts
                .get(&kernel.module)
                .is_none_or(|count| *count == 0)
            || self
                .modules
                .get(&kernel.module)
                .is_none_or(|module| self.streams.get(&active.stream) != Some(&module.device))
        {
            return false;
        }
        if count == 1
            && admissions[0].unwrap().source
                == PersistentFullRangeComputeSourceV1::RetainedControlReplay
            && !active.performance.persistent_control_reused()
        {
            return false;
        }
        let expected_control = if active.performance.persistent_control_reused() {
            Some(RetainedPersistentDispatchV1 {
                allocation: admissions[0].unwrap().allocation,
                dispatch_shape_sha256: active.dispatch_shape_sha256,
            })
        } else {
            None
        };
        if self.retained_persistent_dispatch != expected_control {
            return false;
        }
        for (index, admission) in admissions
            .iter()
            .enumerate()
            .filter_map(|(index, value)| value.as_ref().map(|value| (index, value)))
        {
            if admissions[..index]
                .iter()
                .flatten()
                .any(|prior| prior.allocation == admission.allocation)
                || !active.allocations.contains(&admission.allocation)
            {
                return false;
            }
            let Some(record) = self.allocations.get(&admission.allocation) else {
                return false;
            };
            if !restored
                && !matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == submission)
            {
                return false;
            }
            let expected = RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream: active.stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            };
            if !self
                .allocation_custody
                .get(&admission.allocation)
                .is_some_and(|custody| {
                    custody.owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] != 0
                        && custody
                            .owners
                            .binary_search_by_key(&submission, |owner| owner.submission)
                            .is_ok_and(|index| {
                                custody.owners[index] == expected
                                    && index
                                        .checked_sub(1)
                                        .and_then(|prior| custody.owners.get(prior))
                                        .is_none_or(|owner| owner.submission != submission)
                                    && custody
                                        .owners
                                        .get(index + 1)
                                        .is_none_or(|owner| owner.submission != submission)
                            })
                })
            {
                return false;
            }
        }
        true
    }

    pub(super) fn prepared_persistent_storage_intact_v1(&self) -> bool {
        let Some(execution) = self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        else {
            return false;
        };
        let single = |allocation, access, source, completion| {
            self.scalar_completion_reservation_intact_v1(
                completion,
                PersistentFullRangeComputeAdmissionV1 {
                    allocation,
                    access,
                    source,
                },
            )
        };
        match execution {
            ActiveComputeExecutionV1::PersistentPrepared {
                allocation,
                access,
                source,
                completion,
                ..
            } => single(*allocation, *access, *source, completion),
            ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                admissions,
                restore_shells,
                ..
            } => admissions
                .iter()
                .zip(restore_shells)
                .all(|(admission, shell)| shell.supports_origin_v1(*admission)),
            #[cfg(test)]
            ActiveComputeExecutionV1::ScriptedPersistentPrepared {
                allocation,
                access,
                source,
                completion,
                input,
                ..
            } => {
                single(*allocation, *access, *source, completion)
                    && input.armed().is_some_and(|input| {
                        matches!(
                            (source, input.as_ref()),
                            (
                                PersistentFullRangeComputeSourceV1::InitializedStorage,
                                KfdRuntimePersistentComputeInputV1::ScriptedStorage(_)
                            ) | (
                                PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
                                KfdRuntimePersistentComputeInputV1::ScriptedReady(_)
                            ) | (
                                PersistentFullRangeComputeSourceV1::RetainedControlReplay,
                                KfdRuntimePersistentComputeInputV1::ScriptedReplay(_)
                            )
                        )
                    })
            }
            #[cfg(test)]
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                admissions,
                restore_shells,
                inputs,
                ..
            } => inputs.armed().is_some_and(|inputs| {
                admissions.iter().zip(restore_shells).zip(inputs).all(
                    |((admission, shell), input)| {
                        !matches!(input, KfdRuntimePersistentComputeInputV1::Native(_))
                            && shell.supports_origin_v1(*admission)
                            && shell.accepts_v1(*admission, input)
                    },
                )
            }),
            _ => false,
        }
    }

    fn single_cancel_shell_v1(
        &self,
        admission: PersistentFullRangeComputeAdmissionV1,
    ) -> Result<
        Option<ThreeBindingPersistentRestoreShellV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        if admission.source == PersistentFullRangeComputeSourceV1::InitializedStorage {
            return Ok(None); // Transfer its original shell only after preflight succeeds.
        }
        let ready = admission.source == PersistentFullRangeComputeSourceV1::AuthenticatedH2d;
        let allocate_error =
            |_| Self::capacity("prepared cancellation restore-shell allocation failed");
        Ok(Some(ThreeBindingPersistentRestoreShellV1 {
            initialized: None,
            ready: ready
                .then(try_uninit_box_v1)
                .transpose()
                .map_err(allocate_error)?,
            replay: (!ready || admission.access == RuntimeAccessV1::Write)
                .then(try_uninit_box_v1)
                .transpose()
                .map_err(allocate_error)?,
            #[cfg(test)]
            device: (!ready || admission.access == RuntimeAccessV1::Write)
                .then(try_uninit_box_v1)
                .transpose()
                .map_err(allocate_error)?,
            #[cfg(not(test))]
            device: None,
        }))
    }

    pub(super) fn cancel_persistent_prepared_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        if !self.persistent_prepared_is_armed_v1()
            || !self.persistent_compute_custody_intact_v1(submission, false)
            || !self.prepared_persistent_storage_intact_v1()
            || self.terminal_sdma_custody.is_some()
        {
            return Err(
                self.terminal_error("prepared cancellation custody changed before native effects")
            );
        }
        let admissions = self.persistent_compute_admissions_v1();
        let single = admissions[1].is_none();
        let first = admissions[0].unwrap();
        let native = matches!(
            self.active.as_ref().unwrap().execution,
            Some(
                ActiveComputeExecutionV1::PersistentPrepared { .. }
                    | ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. }
            )
        );
        if native && self.queue.is_none() {
            return Err(self.terminal_error("prepared cancellation lost its native queue"));
        }
        if self
            .submissions
            .try_reserve(self.compute_completion_reservations)
            .is_err()
        {
            return Err(Self::capacity(
                "prepared cancellation cannot reserve completion records",
            ));
        }
        let mut single_shell = if single {
            self.single_cancel_shell_v1(first)?
        } else {
            None
        };
        // Cancellation-only storage must not inflate every published pipeline slot.
        // Reserve it while the original Prepared owner is still indexed.
        let root_shell = try_uninit_box_v1()
            .map_err(|_| Self::capacity("prepared cancellation root allocation failed"))?;
        if single && single_shell.is_none() {
            single_shell = self
                .allocations
                .get_mut(&first.allocation)
                .unwrap()
                .persistent_storage_restore
                .take();
        }
        let active = self.active.as_mut().unwrap();
        let promotion = active.performance.ready_promotion;
        let slot = |admission, promotion, shell, input| {
            Some(PreparedCancellationSlotV1 {
                admission,
                promotion,
                shell: Some(shell),
                input,
                restored: false,
            })
        };
        // Only infallible moves occur between the old and new indexed variants.
        let root = match active.execution.take().unwrap() {
            ActiveComputeExecutionV1::PersistentPrepared {
                prepared, profile, ..
            } => PreparedComputeCancellationV1 {
                receipt: PreparedCancellationReceiptV1::Single(
                    prepared.into_armed().expect("preflighted armed receipt"),
                ),
                slots: [
                    slot(first, promotion, single_shell.unwrap(), None),
                    None,
                    None,
                ],
                _profile: profile,
            },
            ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                admissions,
                promotions,
                restore_shells,
                prepared,
                profile,
            } => {
                let mut shells = restore_shells.into_iter();
                PreparedComputeCancellationV1 {
                    receipt: PreparedCancellationReceiptV1::Three(
                        prepared.into_armed().expect("preflighted armed receipt"),
                    ),
                    slots: std::array::from_fn(|index| {
                        slot(
                            admissions[index],
                            promotions[index],
                            shells.next().unwrap(),
                            None,
                        )
                    }),
                    _profile: profile,
                }
            }
            #[cfg(test)]
            ActiveComputeExecutionV1::ScriptedPersistentPrepared { input, profile, .. } => {
                PreparedComputeCancellationV1 {
                    receipt: PreparedCancellationReceiptV1::InputsReturned,
                    slots: [
                        slot(
                            first,
                            promotion,
                            single_shell.unwrap(),
                            Some(*input.into_armed().expect("preflighted scripted input")),
                        ),
                        None,
                        None,
                    ],
                    _profile: profile,
                }
            }
            #[cfg(test)]
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                admissions,
                promotions,
                restore_shells,
                inputs,
                profile,
            } => {
                let mut shells = restore_shells.into_iter();
                let mut inputs = inputs
                    .into_armed()
                    .expect("preflighted scripted inputs")
                    .into_iter();
                PreparedComputeCancellationV1 {
                    receipt: PreparedCancellationReceiptV1::InputsReturned,
                    slots: std::array::from_fn(|index| {
                        slot(
                            admissions[index],
                            promotions[index],
                            shells.next().unwrap(),
                            inputs.next(),
                        )
                    }),
                    _profile: profile,
                }
            }
            _ => unreachable!("preflighted prepared cancellation"),
        };
        active.execution = Some(ActiveComputeExecutionV1::PersistentCancelling(
            fill_restore_shell_v1(root_shell, root),
        ));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.cancel_prepared_native_v1()?;
            self.restore_cancelled_inputs_v1(submission)?;
            self.commit_prepared_cancellation_v1(submission)
        }));
        match result {
            Ok(result) => result,
            Err(payload) => super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                self.poison_terminal_v1()
            }),
        }
    }

    fn cancel_prepared_native_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let receipt = core::mem::replace(
            &mut self.prepared_cancel_root_mut_v1().receipt,
            PreparedCancellationReceiptV1::NativeOwned,
        );
        match receipt {
            PreparedCancellationReceiptV1::Single(prepared) => {
                match self
                    .queue
                    .as_mut()
                    .unwrap()
                    .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
                {
                    Ok(input) => {
                        self.prepared_cancel_root_mut_v1().slots[0]
                            .as_mut()
                            .unwrap()
                            .input = Some(KfdRuntimePersistentComputeInputV1::Native(input))
                    }
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        match custody {
                            Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(
                                prepared,
                            ) => {
                                self.prepared_cancel_root_mut_v1().receipt =
                                    PreparedCancellationReceiptV1::Single(prepared)
                            }
                            Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(
                                custody,
                            ) => self.retain_terminal_sdma_custody_v1(
                                KfdRuntimeTerminalSdmaCustodyV1::PersistentCompute(custody),
                            ),
                        }
                        return Err(
                            self.terminal_error(format!("KFD prepared cancellation: {error}"))
                        );
                    }
                }
            }
            PreparedCancellationReceiptV1::Three(prepared) => {
                match self
                    .queue
                    .as_mut()
                    .unwrap()
                    .cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(
                        prepared,
                    ) {
                    Ok(inputs) => {
                        for (slot, input) in self
                            .prepared_cancel_root_mut_v1()
                            .slots
                            .iter_mut()
                            .zip(inputs.into_inputs())
                        {
                            slot.as_mut().unwrap().input =
                                Some(KfdRuntimePersistentComputeInputV1::Native(input));
                        }
                    }
                    Err(failure) => {
                        let (error, recovered) = failure.into_parts();
                        if let Some(prepared) = recovered {
                            self.prepared_cancel_root_mut_v1().receipt =
                                PreparedCancellationReceiptV1::Three(prepared);
                        }
                        return Err(self.terminal_error(format!(
                            "KFD three-binding prepared cancellation: {error}"
                        )));
                    }
                }
            }
            PreparedCancellationReceiptV1::InputsReturned => {}
            PreparedCancellationReceiptV1::NativeOwned => {
                unreachable!("native cancellation cannot be repeated")
            }
        }
        self.prepared_cancel_root_mut_v1().receipt = PreparedCancellationReceiptV1::InputsReturned;
        #[cfg(test)]
        match self.scripted_prepared_cancel_fault {
            Some(ScriptedPreparedCancelFaultV1::Terminal) => {
                self.scripted_prepared_cancel_fault = None;
                return Err(self.terminal_error("scripted consuming cancellation failure"));
            }
            Some(ScriptedPreparedCancelFaultV1::Unwind) => {
                self.scripted_prepared_cancel_fault = None;
                std::panic::panic_any("scripted consuming cancellation unwind");
            }
            Some(ScriptedPreparedCancelFaultV1::ReturnedSlotMismatch(index)) => {
                self.scripted_prepared_cancel_fault = None;
                let allocation = self.prepared_cancel_root_v1().slots[index]
                    .as_ref()
                    .unwrap()
                    .admission
                    .allocation;
                self.allocations.get_mut(&allocation).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::ComputeInFlight(0);
            }
            _ => {}
        }
        Ok(())
    }

    fn restore_cancelled_inputs_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.prepared_cancel_root_v1();
        let valid = matches!(root.receipt, PreparedCancellationReceiptV1::InputsReturned)
            && root.slots.iter().flatten().all(|slot| {
                !slot.restored && slot.shell.as_ref().zip(slot.input.as_ref()).is_some_and(|(shell, input)| shell.accepts_v1(slot.admission, input))
                    && self.allocations.get(&slot.admission.allocation).is_some_and(|record| matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == submission))
            });
        if !valid {
            return Err(self.terminal_error(
                "prepared cancellation returned incompatible restoration custody",
            ));
        }
        for index in 0..3 {
            let Some(slot) = self.prepared_cancel_root_mut_v1().slots[index].as_mut() else {
                continue;
            };
            let allocation = slot.admission.allocation;
            #[cfg(test)]
            let replay = matches!(
                slot.input,
                Some(KfdRuntimePersistentComputeInputV1::ScriptedReplay(_))
            );
            let storage = slot
                .shell
                .take()
                .unwrap()
                .restore_v1(slot.input.take().unwrap(), slot.promotion);
            self.allocations.get_mut(&allocation).unwrap().sdma_storage = storage;
            #[cfg(test)]
            {
                self.allocations
                    .get_mut(&allocation)
                    .unwrap()
                    .scripted_three_binding_replay = replay;
            }
            self.prepared_cancel_root_mut_v1().slots[index]
                .as_mut()
                .unwrap()
                .restored = true;
            #[cfg(test)]
            if self.scripted_prepared_cancel_fault
                == Some(ScriptedPreparedCancelFaultV1::AfterRestore(index))
            {
                self.scripted_prepared_cancel_fault = None;
                std::panic::panic_any("scripted cancellation restoration unwind");
            }
        }
        Ok(())
    }

    fn commit_prepared_cancellation_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        #[cfg(test)]
        if self.scripted_prepared_cancel_fault == Some(ScriptedPreparedCancelFaultV1::BeforeCommit)
        {
            self.scripted_prepared_cancel_fault = None;
            std::panic::panic_any("scripted cancellation commit unwind");
        }
        if !self.persistent_compute_custody_intact_v1(submission, true)
            || self
                .submissions
                .capacity()
                .saturating_sub(self.submissions.len())
                < self.compute_completion_reservations
            || !self
                .prepared_cancel_root_v1()
                .slots
                .iter()
                .flatten()
                .all(|slot| slot.restored && slot.input.is_none() && slot.shell.is_none())
        {
            return Err(
                self.terminal_error("prepared cancellation lost logical settlement custody")
            );
        }
        let active = self.active.as_ref().unwrap();
        let (stream, module, depth) = (
            active.stream,
            self.kernels[&active.kernel].module,
            active.dependency_depth,
        );
        let allocations = self.persistent_compute_admissions_v1();
        self.retained_persistent_dispatch = None;
        for admission in allocations.into_iter().flatten() {
            self.release_allocation_custody_v1(admission.allocation, submission);
        }
        self.release_compute_module_retain_v1(module);
        self.submissions.insert(
            submission,
            SubmissionRecordV1 {
                stream,
                status: BackendPollV1::Failed { code: -2 },
                dependency_depth: depth,
                profile_dispatch_published: false,
            },
        );
        self.compute_completion_reservations -= 1;
        self.release_compute_lane_lease_v1(stream, 0);
        self.restore_unfinished_stream_tail_v1(stream, submission);
        self.active = None;
        Ok(crate::BackendCancellationV1::Cancelled)
    }
}
