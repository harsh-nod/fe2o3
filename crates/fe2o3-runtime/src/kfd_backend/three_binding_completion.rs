//! Exact-three completion keeps data and logical custody indexed through control release.

use super::*;

pub(super) type ThreeCompletionReceiptV1 = IndexedPersistentCompletionReceiptV1<
    Gfx942ThreeBindingPersistentComputeDispatchV1,
    fe2o3_kfd::Gfx942RecycledThreeBindingPersistentComputeDispatchV1,
    fe2o3_kfd::Gfx942ThreeBindingPersistentComputeCompletedV1,
    [Option<KfdRuntimePersistentComputeInputV1>; 3],
    [Gfx942PersistentComputeEffectV1; 3],
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreeCompletionControlV1 {
    Pending,
    Releasing,
    Released,
}

pub(super) struct ThreeBindingPersistentCompletionV1 {
    pub(super) admissions: [PersistentFullRangeComputeAdmissionV1; 3],
    pub(super) shells: [Option<ThreeBindingPersistentRestoreShellV1>; 3],
    pub(super) receipt: ThreeCompletionReceiptV1,
    pub(super) control: ThreeCompletionControlV1,
    observed_at: Option<Instant>,
    signal_recycle: Duration,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedThreeCompletionFaultV1 {
    Reserve,
    Poll,
    Detach,
    Retire,
    AfterRetirement,
    Slot(usize),
    Shell(usize),
    Effect(usize),
    AfterRestore(usize),
    CommitRoster,
    ControlMissing,
    ControlFailure,
    ControlUnwind,
    ProfileUnwind,
}

struct ThreeCompletionCommitV1 {
    id: u64,
    stream: u64,
    module: u64,
    depth: usize,
    allocations: [u64; 3],
    performance: KfdRuntimeLaunchPerformanceV1,
}

impl KfdRuntimeBackendV1 {
    pub(super) fn reserve_three_binding_completion_v1(
        &mut self,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
    ) -> Result<
        Box<ThreeBindingPersistentCompletionV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        #[cfg(test)]
        if self.scripted_three_completion_fault == Some(ScriptedThreeCompletionFaultV1::Reserve) {
            self.scripted_three_completion_fault = None;
            return Err(Self::capacity(
                "scripted three-binding completion reservation failure",
            ));
        }
        let shell = try_uninit_box_v1()
            .map_err(|_| Self::capacity("KFD three-binding completion root allocation failed"))?;
        Ok(fill_restore_shell_v1(
            shell,
            ThreeBindingPersistentCompletionV1 {
                admissions,
                shells: [None, None, None],
                receipt: ThreeCompletionReceiptV1::Reserved,
                control: ThreeCompletionControlV1::Pending,
                observed_at: None,
                signal_recycle: Duration::ZERO,
            },
        ))
    }

    pub(super) fn three_completion_reservation_intact_v1(
        &self,
        root: &ThreeBindingPersistentCompletionV1,
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
    ) -> bool {
        root.admissions == admissions
            && matches!(root.receipt, ThreeCompletionReceiptV1::Reserved)
            && root.shells.iter().all(Option::is_none)
            && root.control == ThreeCompletionControlV1::Pending
            && admissions.map(|admission| admission.access)
                == [
                    RuntimeAccessV1::Read,
                    RuntimeAccessV1::Read,
                    RuntimeAccessV1::Write,
                ]
    }

    pub(super) fn three_completion_selected_v1(&self) -> bool {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(
                ActiveComputeExecutionV1::ThreeBindingPersistent { .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(_),
            ) => true,
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. }) => true,
            _ => false,
        }
    }

    pub(super) fn three_completion_root_v1(&self) -> &ThreeBindingPersistentCompletionV1 {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(root)) => root,
            _ => unreachable!("indexed three-binding completion root"),
        }
    }

    fn three_completion_root_mut_v1(&mut self) -> &mut ThreeBindingPersistentCompletionV1 {
        match self
            .active
            .as_mut()
            .and_then(|active| active.execution.as_mut())
        {
            Some(ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(root)) => root,
            _ => unreachable!("indexed three-binding completion root"),
        }
    }

    fn three_completion_preflight_v1(&self) -> bool {
        let Some(active) = &self.active else {
            return false;
        };
        let (admissions, shells, root) = match active.execution.as_ref() {
            Some(ActiveComputeExecutionV1::ThreeBindingPersistent {
                admissions,
                restore_shells,
                completion,
                ..
            }) => (*admissions, restore_shells, completion),
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                admissions,
                restore_shells,
                completion,
                ..
            }) => (*admissions, restore_shells, completion),
            _ => return false,
        };
        self.selected_compute_lane == 0
            && self.terminal_sdma_custody.is_none()
            && self.three_completion_reservation_intact_v1(root, admissions)
            && self.persistent_compute_custody_intact_v1(active.id, false)
            && admissions.iter().zip(shells).all(|(admission, shell)| {
                shell.supports_origin_v1(*admission)
                    && self.allocations[&admission.allocation]
                        .persistent_storage_restore
                        .is_none()
            })
    }

    fn take_three_completion_receipt_v1(&mut self) -> ThreeCompletionReceiptV1 {
        core::mem::replace(
            &mut self.three_completion_root_mut_v1().receipt,
            ThreeCompletionReceiptV1::NativeOwned,
        )
    }

    pub(super) fn advance_three_completion_v1(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.three_completion_preflight_v1() {
            return Err(self
                .terminal_error("three-binding completion custody changed before native effects"));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let active = self.active.as_mut().unwrap();
            let root = match active.execution.take().unwrap() {
                ActiveComputeExecutionV1::ThreeBindingPersistent {
                    dispatch,
                    restore_shells,
                    mut completion,
                    ..
                } => {
                    completion.shells = restore_shells.map(Some);
                    completion.receipt = ThreeCompletionReceiptV1::Published(dispatch);
                    completion
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    devices,
                    restore_shells,
                    mut completion,
                    ..
                } => {
                    completion.shells = restore_shells.map(Some);
                    completion.receipt = ThreeCompletionReceiptV1::Scripted(
                        ScriptedCompletionOwnersV1::Three(devices),
                    );
                    completion
                }
                _ => unreachable!("preflighted three-binding published receipt"),
            };
            active.execution = Some(ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(
                root,
            ));
            self.advance_indexed_three_completion_v1(deadline)
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1())
            }
        }
    }

    fn three_completion_pending_v1(&mut self) -> BackendPollV1 {
        let active = self.active.as_mut().unwrap();
        let Some(ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(mut completion)) =
            active.execution.take()
        else {
            unreachable!()
        };
        let admissions = completion.admissions;
        let restore_shells = std::array::from_fn(|index| completion.shells[index].take().unwrap());
        let receipt =
            core::mem::replace(&mut completion.receipt, ThreeCompletionReceiptV1::Reserved);
        active.execution = Some(match receipt {
            ThreeCompletionReceiptV1::Published(dispatch) => {
                ActiveComputeExecutionV1::ThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    dispatch,
                    completion,
                }
            }
            #[cfg(test)]
            ThreeCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Three(devices)) => {
                ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    devices,
                    completion,
                }
            }
            _ => unreachable!("pending three-binding owner"),
        });
        BackendPollV1::Pending
    }

    fn retain_three_completion_failure_v1(
        &mut self,
        custody: Option<KfdRuntimeTerminalSdmaCustodyV1>,
    ) {
        let receipt = if let Some(custody) = custody {
            self.retain_terminal_sdma_custody_v1(custody);
            ThreeCompletionReceiptV1::TerminalRooted
        } else {
            ThreeCompletionReceiptV1::LowerTerminalOwned
        };
        self.three_completion_root_mut_v1().receipt = receipt;
    }

    #[allow(clippy::result_large_err)]
    fn advance_indexed_three_completion_v1(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        if matches!(
            self.three_completion_root_v1().receipt,
            ThreeCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Three(_))
        ) {
            return self.advance_scripted_three_completion_v1(deadline);
        }
        if self.queue.is_none() {
            return Err(self.terminal_error("published three-binding completion lost its queue"));
        }
        let ThreeCompletionReceiptV1::Published(dispatch) = self.take_three_completion_receipt_v1()
        else {
            unreachable!()
        };
        let queue = self.queue.as_mut().unwrap();
        let poll = match deadline {
            None => queue
                .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1(dispatch),
            Some(deadline) => queue
                .wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1(
                    dispatch, deadline,
                )
                .map(|wait| match wait {
                    Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Timeout {
                        dispatch,
                        ..
                    } => Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(dispatch),
                    Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Recycled {
                        recycled,
                        completion_observed_at,
                        ..
                    } => Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                        recycled,
                        completion_observed_at,
                    },
                }),
        };
        let observed_at =
            match poll {
                Ok(Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Pending(dispatch)) => {
                    self.three_completion_root_mut_v1().receipt =
                        ThreeCompletionReceiptV1::Published(dispatch);
                    return Ok(self.three_completion_pending_v1());
                }
                Ok(Gfx942ThreeBindingPersistentComputePollAndRecycleV1::Recycled {
                    recycled,
                    completion_observed_at,
                }) => {
                    self.three_completion_root_mut_v1().receipt =
                        ThreeCompletionReceiptV1::Recycled(recycled);
                    completion_observed_at
                }
                Err(failure) => {
                    let (error, recovered) = failure.into_parts();
                    self.retain_three_completion_failure_v1(recovered.map(
                        KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputePublished,
                    ));
                    return Err(self
                        .terminal_error(format!("KFD three-binding completion/recycle: {error}")));
                }
            };
        let active = self.active.as_mut().unwrap();
        active.performance.publish_to_completion =
            observed_at.saturating_duration_since(active.published_at);
        let signal_recycle = observed_at.elapsed();
        active.performance.completion_signal_recycle += signal_recycle;
        let root = self.three_completion_root_mut_v1();
        root.observed_at = Some(observed_at);
        root.signal_recycle = signal_recycle;
        let ThreeCompletionReceiptV1::Recycled(recycled) = self.take_three_completion_receipt_v1()
        else {
            unreachable!()
        };
        match self
            .queue
            .as_mut()
            .unwrap()
            .detach_recycled_three_binding_directional_persistent_fixed_dispatch_v1(recycled)
        {
            Ok(completed) => {
                self.three_completion_root_mut_v1().receipt =
                    ThreeCompletionReceiptV1::Detached(completed)
            }
            Err(failure) => {
                let (error, recovered) = failure.into_parts();
                self.retain_three_completion_failure_v1(
                    recovered.map(
                        KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentComputeRecycled,
                    ),
                );
                return Err(
                    self.terminal_error(format!("KFD three-binding completion detach: {error}"))
                );
            }
        }
        let ThreeCompletionReceiptV1::Detached(completed) = self.take_three_completion_receipt_v1()
        else {
            unreachable!()
        };
        match completed.retire_settled_frontiers_for_replay_v1() {
            Ok(completed) => {
                let effects = std::array::from_fn(|index| completed[index].1);
                self.three_completion_root_mut_v1().receipt = ThreeCompletionReceiptV1::Retired(
                    completed
                        .map(|(input, _)| Some(KfdRuntimePersistentComputeInputV1::Native(input))),
                    effects,
                );
            }
            Err(completed) => {
                self.three_completion_root_mut_v1().receipt =
                    ThreeCompletionReceiptV1::Detached(completed);
                return Err(self.terminal_error(
                    "KFD three-binding frontier retirement rejected its exact completed roster",
                ));
            }
        }
        self.finish_indexed_three_completion_v1()
    }

    fn finish_indexed_three_completion_v1(
        &mut self,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        self.inject_three_retired_fault_v1();
        self.restore_indexed_three_completion_v1()?;
        let plan = self.preflight_three_completion_commit_v1()?;
        self.three_completion_root_mut_v1().control = ThreeCompletionControlV1::Releasing;
        self.release_primary_detached_persistent_control_v1(
            "three-binding completion lost its detached queue control",
        )?;
        self.three_completion_root_mut_v1().control = ThreeCompletionControlV1::Released;
        self.commit_three_completion_v1(plan);
        Ok(BackendPollV1::Succeeded)
    }

    fn restore_indexed_three_completion_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.three_completion_root_v1();
        let ThreeCompletionReceiptV1::Retired(inputs, effects) = &root.receipt else {
            unreachable!("retired exact-three inputs")
        };
        let active = self.active.as_ref().unwrap();
        let valid = self.terminal_sdma_custody.is_none()
            && root.control == ThreeCompletionControlV1::Pending
            && self.persistent_compute_custody_intact_v1(active.id, false)
            && *effects
                == root
                    .admissions
                    .map(|admission| persistent_compute_effect_v1(admission.access))
            && root.admissions.iter().zip(&root.shells).zip(inputs).all(
                |((admission, shell), input)| {
                    self.allocations[&admission.allocation]
                        .persistent_storage_restore
                        .is_none()
                        && shell
                            .as_ref()
                            .zip(input.as_ref())
                            .is_some_and(|(shell, input)| shell.accepts_v1(*admission, input))
                },
            );
        if !valid {
            return Err(
                self.terminal_error("three-binding completion slot/input/effect/shell mismatch")
            );
        }
        // Preflight all slots before committing any. A completed prefix is
        // record-owned; every remaining input and shell stays in the root.
        for index in 0..3 {
            let active = self.active.as_mut().unwrap();
            let Some(ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(root)) =
                active.execution.as_mut()
            else {
                unreachable!()
            };
            let ThreeCompletionReceiptV1::Retired(inputs, effects) = &mut root.receipt else {
                unreachable!()
            };
            let record = self
                .allocations
                .get_mut(&root.admissions[index].allocation)
                .unwrap();
            let input = inputs[index].take().unwrap();
            #[cfg(test)]
            let replay = matches!(
                &input,
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(_)
            );
            record.sdma_storage = root.shells[index].take().unwrap().restore_v1(input, None);
            #[cfg(test)]
            {
                record.scripted_three_binding_replay = replay;
            }
            apply_persistent_compute_effect_v1(record, effects[index]);
            #[cfg(test)]
            if self.scripted_three_completion_fault
                == Some(ScriptedThreeCompletionFaultV1::AfterRestore(index))
            {
                self.scripted_three_completion_fault = None;
                panic!("scripted three-binding restoration-prefix unwind");
            }
        }
        self.three_completion_root_mut_v1().receipt = ThreeCompletionReceiptV1::Restored;
        Ok(())
    }

    fn preflight_three_completion_commit_v1(
        &mut self,
    ) -> Result<ThreeCompletionCommitV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        if self.scripted_three_completion_fault
            == Some(ScriptedThreeCompletionFaultV1::CommitRoster)
        {
            self.scripted_three_completion_fault = None;
            self.active.as_mut().unwrap().allocations.clear();
        }
        let root = self.three_completion_root_v1();
        let active = self.active.as_ref().unwrap();
        let native_queue_present = self.queue.is_some()
            && self
                .native_compute_lanes
                .first()
                .is_some_and(Option::is_some);
        #[cfg(test)]
        let native_queue_present = native_queue_present || self.scripted_sdma.is_some();
        let valid = self.selected_compute_lane == 0
            && native_queue_present
            && matches!(root.receipt, ThreeCompletionReceiptV1::Restored)
            && root.control == ThreeCompletionControlV1::Pending
            && root.shells.iter().all(Option::is_none)
            && self.terminal_sdma_custody.is_none()
            && self.retained_persistent_dispatch.is_none()
            && self.recycled_dispatch.is_none()
            && self.resident_data.is_none()
            && self.persistent_compute_custody_intact_v1(active.id, true)
            && self
                .submissions
                .capacity()
                .saturating_sub(self.submissions.len())
                >= self.compute_completion_reservations
            && root.admissions.iter().all(|admission| {
                self.allocations
                    .get(&admission.allocation)
                    .is_some_and(|record| {
                        record.persistent_storage_restore.is_none()
                            && matches!(
                                record.sdma_storage,
                                KfdRuntimeSdmaStorageV1::H2dReady(_)
                                    | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                                    | KfdRuntimeSdmaStorageV1::Device(_)
                            )
                    })
            });
        if !valid {
            return Err(self.terminal_error(
                "three-binding completion lost settlement custody before control release",
            ));
        }
        let detach_restore = root.observed_at.map_or(Duration::ZERO, |start| {
            completion_detach_restore_duration_v1(start.elapsed(), root.signal_recycle)
        });
        let allocations = root.admissions.map(|admission| admission.allocation);
        let active = self.active.as_mut().unwrap();
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_detach_restore += detach_restore;
        Ok(ThreeCompletionCommitV1 {
            id: active.id,
            stream: active.stream,
            module: self.kernels[&active.kernel].module,
            depth: active.dependency_depth,
            allocations,
            performance: active.performance,
        })
    }

    fn commit_three_completion_v1(&mut self, plan: ThreeCompletionCommitV1) {
        self.release_compute_custody_v1(plan.id, plan.module, plan.allocations);
        self.submissions.insert(
            plan.id,
            SubmissionRecordV1 {
                stream: plan.stream,
                status: BackendPollV1::Succeeded,
                dependency_depth: plan.depth,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations -= 1;
        self.release_compute_lane_lease_v1(plan.stream, self.selected_compute_lane);
        self.last_launch_performance = Some(plan.performance);
        #[cfg(test)]
        if self.scripted_three_completion_fault
            == Some(ScriptedThreeCompletionFaultV1::ProfileUnwind)
        {
            self.scripted_three_completion_fault = None;
            panic!("scripted three-binding completion-report unwind");
        }
        let profile = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, plan.id);
        self.observe_profile_v1(profile.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(plan.performance),
            }
        }));
        self.active = None;
    }

    #[cfg(test)]
    fn advance_scripted_three_completion_v1(
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
                    return Ok(self.three_completion_pending_v1());
                }
                attempts = attempts.saturating_add(1);
            }
        }
        if matches!(
            self.scripted_three_completion_fault,
            Some(
                ScriptedThreeCompletionFaultV1::Poll
                    | ScriptedThreeCompletionFaultV1::Detach
                    | ScriptedThreeCompletionFaultV1::Retire
            )
        ) {
            self.scripted_three_completion_fault = None;
            return Err(
                self.terminal_error("scripted three-binding completion observation failure")
            );
        }
        let admissions = self.three_completion_root_v1().admissions;
        let digests =
            admissions.map(|admission| self.allocations[&admission.allocation].content_sha256);
        if admissions.iter().zip(digests).any(|(admission, digest)| {
            admission.source == PersistentFullRangeComputeSourceV1::AuthenticatedH2d
                && admission.access == RuntimeAccessV1::Read
                && digest.is_none()
        }) {
            return Err(
                self.terminal_error("scripted three-binding read lost its authenticated digest")
            );
        }
        let ThreeCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Three(devices)) =
            &self.three_completion_root_v1().receipt
        else {
            unreachable!()
        };
        if devices
            .iter()
            .any(|device| device.scripted_owner_id().is_none())
        {
            return Err(self
                .terminal_error("scripted three-binding completion lost its exact device roster"));
        }
        let ThreeCompletionReceiptV1::Scripted(ScriptedCompletionOwnersV1::Three([a, b, c])) =
            self.take_three_completion_receipt_v1()
        else {
            unreachable!()
        };
        let restore = |index: usize, device| {
            if admissions[index].source == PersistentFullRangeComputeSourceV1::AuthenticatedH2d
                && admissions[index].access == RuntimeAccessV1::Read
            {
                let DirectionalSdmaDeviceOwnerV1::Scripted(device) = device else {
                    unreachable!()
                };
                KfdRuntimePersistentComputeInputV1::ScriptedReady(PersistentComputeReadyStorageV1 {
                    owner: PersistentComputeReadyOwnerV1::Scripted {
                        device,
                        authenticated_sha256: digests[index].unwrap(),
                    },
                    promotion: None,
                })
            } else {
                KfdRuntimePersistentComputeInputV1::ScriptedReplay(device)
            }
        };
        self.three_completion_root_mut_v1().receipt = ThreeCompletionReceiptV1::Retired(
            [
                Some(restore(0, a)),
                Some(restore(1, b)),
                Some(restore(2, c)),
            ],
            admissions.map(|admission| persistent_compute_effect_v1(admission.access)),
        );
        let active = self.active.as_mut().unwrap();
        active.performance.publish_to_completion = active.published_at.elapsed();
        self.finish_indexed_three_completion_v1()
    }

    #[cfg(test)]
    fn inject_three_retired_fault_v1(&mut self) {
        match self.scripted_three_completion_fault.take() {
            Some(ScriptedThreeCompletionFaultV1::AfterRetirement) => {
                panic!("scripted three-binding retired-input unwind")
            }
            Some(ScriptedThreeCompletionFaultV1::Slot(index)) => {
                let allocation = self.three_completion_root_v1().admissions[index].allocation;
                self.allocations.get_mut(&allocation).unwrap().sdma_storage =
                    KfdRuntimeSdmaStorageV1::ComputeInFlight(u64::MAX);
            }
            Some(ScriptedThreeCompletionFaultV1::Shell(index)) => {
                self.three_completion_root_mut_v1().shells[index] = None
            }
            Some(ScriptedThreeCompletionFaultV1::Effect(index)) => {
                let ThreeCompletionReceiptV1::Retired(_, effects) =
                    &mut self.three_completion_root_mut_v1().receipt
                else {
                    unreachable!()
                };
                effects[index] = persistent_compute_effect_v1(if index == 2 {
                    RuntimeAccessV1::Read
                } else {
                    RuntimeAccessV1::Write
                });
            }
            fault => self.scripted_three_completion_fault = fault,
        }
    }
}
