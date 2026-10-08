//! Original destination restoration surrounds the separately rooted DATA copy.
//!
//! Generic backend polling observes only recorded status. The owning generated
//! Context caller alone advances this root inside its original source bracket.

use super::*;

// The enclosing native-call finisher poisons the original backend before this
// diagnostic can escape. Owners stay in the copy root throughout formatting.
fn retained_failure(detail: &'static str) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
    RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Terminal,
        detail,
    ))
}

pub(in crate::kfd_backend::generated_adoption) struct BackendCopyV1 {
    submission: u64,
    destination: u64,
    root: NativeCopyV1,
    shell: Option<Box<MaybeUninit<DirectionalSdmaDeviceOwnerV1>>>,
    released: Option<ReleasedCopyV1>,
    source_released: Option<ReleasedSourceV1>,
}

impl BackendCopyV1 {
    pub(in crate::kfd_backend::generated_adoption) fn physically_released_count(
        &self,
    ) -> Option<usize> {
        (self.root.is_empty()
            && self.shell.is_none()
            && self.released.is_none()
            && self.source_released.is_some())
        .then_some(1)
    }
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn begin_retained_generated_copy_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        producer: u64,
        stream: u64,
        destination: u64,
        bytes: u32,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_submission_capacity_v1()?;
        self.require_no_generated_stream_v1(stream)?;
        if !self.generated_submission_owner_matches_v1(producer, plan)
            || plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
            || plan.count != 1
            || plan.members[0].is_none_or(|m| m.description.byte_len != u64::from(bytes))
            || self.streams.get(&stream) != Some(&plan.binding.backend_device)
            || self.stream_submission_tails.contains_key(&stream)
            || self.active_sdma_streams.contains_key(&stream)
            || self.pending_compute_streams.contains_key(&stream)
            || self.stream_compute_lanes.contains_key(&stream)
            || self.allocation_custody.contains_key(&destination)
            || self.queue.is_none()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "retained generated copy identity or stream refusal",
            ));
        }
        let native = self
            .generated_shells
            .get(&plan.key)
            .and_then(|r| r.native.as_ref());
        if !native.is_some_and(|n| {
            n.phase == PhaseV1::Detached
                && n.lane == 0
                && n.copy.is_none()
                && n.sdma.is_bound()
                && n.detached.is_held()
        }) || !self.generated_lease_matches_v1(plan)
            || !self.allocations.get(&destination).is_some_and(|r| {
                r.device == plan.binding.backend_device
                    && r.kind == RuntimeMemoryKindV1::DeviceLocal
                    && r.bytes.len() == bytes as usize
                    && r.native_dirty.is_empty()
                    && matches!(&r.sdma_storage, KfdRuntimeSdmaStorageV1::Device(owner)
                        if matches!(&**owner, DirectionalSdmaDeviceOwnerV1::Native(_)))
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "retained generated copy requires original detached DATA and native whole destination",
            ));
        }
        let root = NativeCopyV1::empty(0, bytes)
            .ok_or_else(|| Self::capacity("generated copy byte bound"))?;
        self.submissions
            .try_reserve(1)
            .map_err(|_| Self::capacity("generated copy submission storage"))?;
        self.stream_submission_tails
            .try_reserve(1)
            .map_err(|_| Self::capacity("generated copy stream storage"))?;
        let custody = self.reserve_allocation_custody_v1(&[destination])?;
        let submission = self.next_id()?;
        if submission == 0
            || self.submissions.contains_key(&submission)
            || self.generated_submissions.contains_key(&submission)
        {
            self.poison_terminal_v1();
            return Err(retained_failure(
                "generated copy submission identity collision",
            ));
        }
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .unwrap()
            .native
            .as_mut()
            .unwrap();
        native.copy = Some(BackendCopyV1 {
            submission,
            destination,
            root,
            shell: None,
            released: None,
            source_released: None,
        });
        let storage = &mut self.allocations.get_mut(&destination).unwrap().sdma_storage;
        let KfdRuntimeSdmaStorageV1::Device(owner) = core::mem::replace(
            storage,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(submission)),
        ) else {
            std::process::abort()
        };
        let (owner, shell) = take_restore_shell_v1(owner);
        #[cfg(not(test))]
        let DirectionalSdmaDeviceOwnerV1::Native(owner) = owner;
        #[cfg(test)]
        let owner = match owner {
            DirectionalSdmaDeviceOwnerV1::Native(owner) => owner,
            DirectionalSdmaDeviceOwnerV1::Scripted(_) => std::process::abort(),
        };
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .unwrap()
            .native
            .as_mut()
            .unwrap();
        let copy = native.copy.as_mut().unwrap();
        copy.shell = Some(shell);
        let original = native.submission.as_ref().unwrap();
        if let Err(owner) = copy.root.install(
            plan,
            original,
            &mut native.detached,
            &mut native.sdma,
            owner,
        ) {
            let shell = copy.shell.take().unwrap();
            self.allocations.get_mut(&destination).unwrap().sdma_storage =
                KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(
                    shell,
                    DirectionalSdmaDeviceOwnerV1::Native(owner),
                ));
            native.copy = None;
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "original generated copy lineage refused before transfer",
            ));
        }
        native.phase = PhaseV1::Copying;
        self.retain_allocation_custody_v1(
            &[destination],
            RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            },
            custody,
        );
        let std::collections::hash_map::Entry::Vacant(entry) = self.submissions.entry(submission)
        else {
            // Original copy custody is already rooted; never replace an owner.
            std::process::abort();
        };
        entry.insert(SubmissionRecordV1 {
            origin: SubmissionOriginV1::GeneratedDataCopy {
                shell: plan.key,
                producer,
                destination,
            },
            stream,
            status: BackendPollV1::Pending,
            dependency_depth: 0,
            profile_dispatch_published: false,
        });
        let std::collections::hash_map::Entry::Vacant(entry) =
            self.stream_submission_tails.entry(stream)
        else {
            std::process::abort();
        };
        entry.insert(submission);
        Ok(submission)
    }

    pub(crate) fn advance_retained_generated_copy_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        producer: u64,
        submission: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.generated_submission_owner_matches_v1(producer, plan)
            || !self.generated_lease_matches_v1(plan)
            || !self
                .generated_shells
                .get(&plan.key)
                .and_then(|r| r.native.as_ref())
                .is_some_and(|n| {
                    n.phase == PhaseV1::Copying
                        && n.copy.as_ref().is_some_and(|c| c.submission == submission)
                })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "retained generated copy continuation mismatch",
            ));
        }
        let mut complete = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .unwrap()
                .native
                .as_mut()
                .unwrap();
            let copy = native.copy.as_mut().unwrap();
            if copy.physically_released_count().is_some() {
                complete = true;
                return Ok(());
            }
            let progress = copy
                .root
                .advance(self.queue.as_mut().unwrap())
                .map_err(|_| {
                    retained_failure("generated DATA copy retains failed original custody")
                })?;
            if progress != Progress::Released {
                return Ok(());
            }
            copy.released = copy
                .root
                .take_released(plan, native.submission.as_ref().unwrap());
            if copy.released.is_none() {
                return Err(retained_failure(
                    "generated copy release lost original lineage",
                ));
            }
            let current = self
                .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                    device.model_admission()
                })?;
            if current != plan.binding.native_device || !self.generated_lease_matches_v1(plan) {
                return Err(retained_failure("generated copy closing device mismatch"));
            }
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .unwrap()
                .native
                .as_mut()
                .unwrap();
            let copy = native.copy.as_mut().unwrap();
            let original = native.submission.as_ref().unwrap();
            if !copy.released.as_ref().is_some_and(|r| r.matches(plan, original))
                || copy.shell.is_none()
                || !self.allocations.get(&copy.destination).is_some_and(|r|
                    matches!(r.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) if id == submission))
            {
                return Err(retained_failure("generated copy destination restoration mismatch"));
            }
            let (destination, source_released) = copy.released.take().unwrap().into_parts();
            copy.source_released = Some(source_released);
            let shell = copy.shell.take().unwrap();
            let record = self.allocations.get_mut(&copy.destination).unwrap();
            record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(fill_restore_shell_v1(
                shell,
                DirectionalSdmaDeviceOwnerV1::Native(destination),
            ));
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = true;
            record.content_sha256 = None;
            record.last_full_host_write = None;
            let destination_id = copy.destination;
            self.release_allocation_custody_v1(destination_id, submission);
            let Some(record) = self.submissions.get_mut(&submission) else {
                std::process::abort()
            };
            record.status = BackendPollV1::Succeeded;
            complete = true;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)?;
        Ok(complete)
    }

    pub(crate) fn finish_retained_generated_copy_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        producer: u64,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.generated_submission_owner_matches_v1(producer, plan)
            || !self.generated_lease_matches_v1(plan)
            || self.submissions.contains_key(&submission)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated copy requires exact original Context copy release",
            ));
        }
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .unwrap()
            .native
            .as_mut()
            .unwrap();
        let Some(copy) = native.copy.as_ref() else {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "missing original copy",
            ));
        };
        if native.phase != PhaseV1::Copying
            || copy.submission != submission
            || copy.physically_released_count() != Some(1)
            || !copy
                .source_released
                .as_ref()
                .is_some_and(|r| r.lineage.matches(plan, native.submission.as_ref().unwrap()))
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "source release is incomplete",
            ));
        }
        native.phase = PhaseV1::CopyRetired;
        let lane = native.lane;
        self.release_compute_lane_lease_v1(plan.binding.backend_stream, lane);
        Ok(())
    }

    pub(in crate::kfd_backend) fn generated_copy_release_blocked_v1(
        &mut self,
        submission: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(record) = self.submissions.get(&submission).copied() else {
            // Pending ordinary owners keep their existing checks; a missing
            // settled record still fails at the sole release/removal tail.
            return Ok(false);
        };
        let SubmissionOriginV1::GeneratedDataCopy {
            shell,
            producer,
            destination,
        } = record.origin
        else {
            return Ok(false);
        };
        let state =
            self.generated_copy_release_state_v1(submission, record, shell, producer, destination);
        match state {
            Some(blocked) => Ok(blocked),
            None => {
                self.poison_terminal_v1();
                Err(retained_failure(
                    "generated copy release index or original owner mismatch",
                ))
            }
        }
    }

    // Fixed keyed lookups only. Full live-set validation stays at original
    // admission/progress/finish; this adds no table-wide scan to ordinary release.
    fn generated_copy_release_state_v1(
        &self,
        submission: u64,
        record: SubmissionRecordV1,
        shell: u64,
        producer: u64,
        destination: u64,
    ) -> Option<bool> {
        if submission == 0
            || shell == 0
            || producer == 0
            || destination == 0
            || self.generated_submissions.get(&producer) != Some(&shell)
        {
            return None;
        }
        let stored = self.generated_shells.get(&shell)?;
        let plan = &stored.plan;
        let native = stored.native.as_ref()?;
        let original = native.submission.as_ref()?;
        let copy = native.copy.as_ref()?;
        if plan.key != shell
            || plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
            || plan.count != 1
            || native.phase != PhaseV1::Copying
            || original.id != producer
            || original.receipt.profile() != plan.profile
            || !stored
                .source_identity
                .matches(&original.roster.source_identity)
            || !readback::roster_matches_plan_v1(plan, &original.roster)
            || copy.submission != submission
            || copy.destination != destination
            || record.stream == plan.binding.backend_stream
            || self.streams.get(&record.stream) != Some(&plan.binding.backend_device)
        {
            return None;
        }
        if copy.physically_released_count() != Some(1) {
            return Some(true);
        }
        if record.status != BackendPollV1::Succeeded
            || !copy
                .source_released
                .as_ref()?
                .lineage
                .matches(plan, original)
        {
            return None;
        }
        Some(false)
    }
}

#[cfg(test)]
mod release_tests {
    use super::*;

    #[test]
    fn backend_success_without_original_physical_release_cannot_release_copy() {
        let (mut backend, plan, roster) =
            super::release_index_tests::single_buffer_shells_with_roster();
        let producer = backend.install_generated_receipt_metadata_for_test_v1(&plan, &roster);
        let stream = backend
            .create_stream_v1(plan.binding.backend_device)
            .unwrap();
        let id = backend.next_id().unwrap();
        let native = backend
            .generated_shells
            .get_mut(&plan.key)
            .unwrap()
            .native
            .as_mut()
            .unwrap();
        // No DATA/queue/physical receipt exists in this metadata-only refusal.
        native.phase = PhaseV1::Copying;
        native.copy = Some(BackendCopyV1 {
            submission: id,
            destination: 999,
            root: NativeCopyV1::empty(0, 16).unwrap(),
            shell: None,
            released: None,
            source_released: None,
        });
        assert_eq!(native.disposed_count(), None);
        assert!(!native.is_retired());
        backend.submissions.insert(
            id,
            SubmissionRecordV1 {
                origin: SubmissionOriginV1::GeneratedDataCopy {
                    shell: plan.key,
                    producer,
                    destination: 999,
                },
                stream,
                status: BackendPollV1::Succeeded,
                dependency_depth: 0,
                profile_dispatch_published: false,
            },
        );
        assert!(backend.generated_copy_release_blocked_v1(id).unwrap());
        assert!(matches!(
            backend.release_submission_v1(id),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(backend.submissions.contains_key(&id));
        let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
        assert!(native.copy.as_ref().unwrap().source_released.is_none());
        assert_eq!(native.disposed_count(), None);
        assert!(!native.is_retired());
        // There is no legitimate physical release to fabricate for cleanup.
        core::mem::forget(backend);
    }
}

#[cfg(test)]
#[path = "release_index_tests.rs"]
mod release_index_tests;
