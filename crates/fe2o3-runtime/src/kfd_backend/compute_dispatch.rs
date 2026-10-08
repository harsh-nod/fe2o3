use super::*;

#[cfg(test)]
pub(super) struct UnpublishedComputeFailureForTestV1 {
    pub(super) submission: u64,
    pub(super) stream: u64,
    pub(super) stage: &'static str,
    pub(super) kind: KfdRuntimeBackendErrorKindV1,
    pub(super) quiescent: bool,
    pub(super) detail_truncated: bool,
    detail: [u8; 512],
    detail_len: usize,
}

#[cfg(test)]
impl UnpublishedComputeFailureForTestV1 {
    pub(super) fn detail(&self) -> &str {
        std::str::from_utf8(&self.detail[..self.detail_len]).unwrap()
    }
}

#[cfg(test)]
std::thread_local! {
    // Explicit opt-in, first failure only, no allocation or native authority.
    static UNPUBLISHED_COMPUTE_FAILURE_FOR_TEST_V1:
        std::cell::RefCell<(bool, Option<UnpublishedComputeFailureForTestV1>)> =
        const { std::cell::RefCell::new((false, None)) };
}

#[cfg(test)]
pub(super) fn reset_unpublished_compute_failure_for_test_v1() {
    UNPUBLISHED_COMPUTE_FAILURE_FOR_TEST_V1.with(|cell| *cell.borrow_mut() = (true, None));
}

#[cfg(test)]
pub(super) fn take_unpublished_compute_failure_for_test_v1()
-> Option<UnpublishedComputeFailureForTestV1> {
    UNPUBLISHED_COMPUTE_FAILURE_FOR_TEST_V1.with(|cell| {
        let mut state = cell.borrow_mut();
        state.0 = false;
        state.1.take()
    })
}

#[cfg(test)]
fn record_unpublished_compute_failure_for_test_v1(
    submission: u64,
    stream: u64,
    stage: &'static str,
    failure: &RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
) {
    UNPUBLISHED_COMPUTE_FAILURE_FOR_TEST_V1.with(|cell| {
        let mut state = cell.borrow_mut();
        if !state.0 || state.1.is_some() {
            return;
        }
        let (error, quiescent) = match failure {
            RuntimeBackendFailureV1::Rejected(error) => (error, false),
            RuntimeBackendFailureV1::Quiescent(error) => (error, true),
            RuntimeBackendFailureV1::Terminal(_) => return,
        };
        let mut detail = [0; 512];
        let mut detail_len = error.detail().len().min(detail.len());
        while !error.detail().is_char_boundary(detail_len) {
            detail_len -= 1;
        }
        detail[..detail_len].copy_from_slice(&error.detail().as_bytes()[..detail_len]);
        state.1 = Some(UnpublishedComputeFailureForTestV1 {
            submission,
            stream,
            stage,
            kind: error.kind(),
            quiescent,
            detail_truncated: detail_len != error.detail().len(),
            detail,
            detail_len,
        });
    });
}

#[cfg(test)]
#[test]
fn unpublished_failure_capture_is_opt_in_first_only_bounded_and_utf8_safe() {
    let rejected = RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Unsupported,
        "must not replace the first failure",
    ));
    assert!(take_unpublished_compute_failure_for_test_v1().is_none());
    record_unpublished_compute_failure_for_test_v1(1, 2, "disabled", &rejected);
    assert!(take_unpublished_compute_failure_for_test_v1().is_none());

    reset_unpublished_compute_failure_for_test_v1();
    let terminal = RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Terminal,
        "terminal failures must not masquerade as unpublished settlement",
    ));
    record_unpublished_compute_failure_for_test_v1(3, 4, "terminal", &terminal);
    UNPUBLISHED_COMPUTE_FAILURE_FOR_TEST_V1.with(|cell| {
        assert!(cell.borrow().1.is_none());
    });
    let prefix = "a".repeat(511);
    let quiescent = RuntimeBackendFailureV1::Quiescent(KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Native,
        format!("{prefix}\u{00e9}tail"),
    ));
    record_unpublished_compute_failure_for_test_v1(5, 6, "staging", &quiescent);
    record_unpublished_compute_failure_for_test_v1(7, 8, "later", &rejected);
    let captured = take_unpublished_compute_failure_for_test_v1().unwrap();
    assert_eq!((captured.submission, captured.stream), (5, 6));
    assert_eq!(captured.stage, "staging");
    assert_eq!(captured.kind, KfdRuntimeBackendErrorKindV1::Native);
    assert!(captured.quiescent && captured.detail_truncated);
    assert_eq!(captured.detail(), prefix);
    assert!(captured.detail().len() <= 512);
    record_unpublished_compute_failure_for_test_v1(9, 10, "disabled-after-take", &rejected);
    assert!(take_unpublished_compute_failure_for_test_v1().is_none());

    reset_unpublished_compute_failure_for_test_v1();
    let exact_detail = "\u{00e9}".repeat(256);
    let exact = RuntimeBackendFailureV1::Rejected(KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Capacity,
        &exact_detail,
    ));
    record_unpublished_compute_failure_for_test_v1(11, 12, "exact-bound", &exact);
    let captured = take_unpublished_compute_failure_for_test_v1().unwrap();
    assert_eq!(captured.detail().len(), 512);
    assert_eq!(captured.detail(), exact_detail);
    assert!(!captured.quiescent && !captured.detail_truncated);
}

impl KfdRuntimeBackendV1 {
    pub(super) fn compute_stream_head_publication_blocker_v1(
        &self,
        pending: &PendingComputeSubmissionV1,
    ) -> Option<&'static str> {
        let ordered_lane = pending.ordered_predecessor.and_then(|predecessor| {
            let lane = self.active_compute_lane_v1(predecessor)?;
            ordered_successor_lane_matches_v1(
                self.stream_compute_lanes
                    .get(&pending.launch.stream)
                    .copied(),
                lane,
            )
            .then_some(lane)
        });
        if self.free_compute_lane_v1().is_none() && ordered_lane.is_none() {
            return Some("KFD compute stream head has no mutation-free publication slot");
        }
        let overlaps_compute = (0..self.native_compute_lanes.len()).any(|lane| {
            if ordered_lane == Some(lane) {
                return false;
            }
            if lane == 0 {
                launch_overlaps_active_compute_v1(
                    &pending.launch.bindings,
                    self.active.iter().chain(self.compute_pipeline.iter()),
                )
            } else {
                let state = &self.auxiliary_compute_lanes[lane - 1];
                launch_overlaps_active_compute_v1(
                    &pending.launch.bindings,
                    state.active.iter().chain(state.pipeline.iter()),
                )
            }
        });
        if overlaps_compute
            || self
                .published_sdma_conflict_v1(
                    pending.id,
                    pending.launch.stream,
                    &pending.launch.bindings,
                )
                .is_some()
        {
            return Some("KFD compute stream head conflicts with published native work");
        }
        None
    }
}

mod materialization;
use materialization::{
    materialize_in_retained_session_v1, materialize_with_custody_v1, overwrite_with_custody_v1,
};

pub(super) fn three_binding_persistent_compute_access_shape_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
) -> bool {
    let [a, b, c] = bindings else {
        return false;
    };
    semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && [a.region.access, b.region.access, c.region.access]
            == [
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Read,
                RuntimeAccessV1::Write,
            ]
}

pub(super) fn three_binding_requires_persistent_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    allocations: &AllocationTableV1,
) -> bool {
    // Only fully resolved host-visible rosters may use ordinary materialization.
    // Mixed or device-local candidates still require authenticated persistence.
    three_binding_persistent_compute_access_shape_v1(semantic_launch, bindings)
        && !bindings.iter().all(|binding| {
            allocations
                .get(&binding.region.allocation)
                .is_some_and(|allocation| allocation.kind == RuntimeMemoryKindV1::HostVisible)
        })
}

pub(super) fn three_binding_persistent_compute_shape_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocations: &AllocationTableV1,
) -> bool {
    let [a, b, c] = bindings else {
        return false;
    };
    if a.region.byte_len != b.region.byte_len || a.region.byte_len != c.region.byte_len {
        return false;
    }
    if [a.region.access, b.region.access, c.region.access]
        != [
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Read,
            RuntimeAccessV1::Write,
        ]
        || a.region.allocation == b.region.allocation
        || a.region.allocation == c.region.allocation
        || b.region.allocation == c.region.allocation
        || semantic_launch != KfdRuntimeSemanticLaunchV1::Ordinary
    {
        return false;
    }
    bindings.iter().all(|binding| {
        let Some(allocation) = allocations.get(&binding.region.allocation) else {
            return false;
        };
        let Ok(logical_bytes) = u64::try_from(allocation.bytes.len()) else {
            return false;
        };
        allocation.device == stream_device
            && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
            && allocation.sdma_backed
            && allocation.sdma_initialized
            && allocation.native_dirty.is_empty()
            && logical_bytes != 0
            && binding.region.byte_offset == 0
            && binding.region.byte_len == logical_bytes
    })
}

pub(super) fn three_binding_persistent_ready_source_v1(
    allocation: &AllocationRecordV1,
) -> Option<PersistentFullRangeComputeSourceV1> {
    let logical_bytes = u64::try_from(allocation.bytes.len()).ok()?;
    match &allocation.sdma_storage {
        KfdRuntimeSdmaStorageV1::InitializedStorage(ready)
            if ready.extents() == (logical_bytes, logical_bytes) =>
        {
            Some(PersistentFullRangeComputeSourceV1::InitializedStorage)
        }
        KfdRuntimeSdmaStorageV1::H2dReady(ready)
            if !allocation.sdma_shadow_dirty
                && allocation.content_sha256 == Some(ready.owner.authenticated_sha256())
                && ready.owner.byte_len() == logical_bytes
                && ready.owner.physical_byte_len() == logical_bytes =>
        {
            Some(PersistentFullRangeComputeSourceV1::AuthenticatedH2d)
        }
        KfdRuntimeSdmaStorageV1::PersistentReplay(input) if input.is_fully_initialized() => {
            Some(PersistentFullRangeComputeSourceV1::RetainedControlReplay)
        }
        #[cfg(test)]
        KfdRuntimeSdmaStorageV1::Device(_) if allocation.scripted_three_binding_replay => {
            Some(PersistentFullRangeComputeSourceV1::RetainedControlReplay)
        }
        _ => None,
    }
}

pub(super) fn three_binding_persistent_compute_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocations: &AllocationTableV1,
) -> Option<ThreeBindingPersistentComputeAdmissionV1> {
    if !three_binding_persistent_compute_shape_v1(
        semantic_launch,
        bindings,
        stream_device,
        allocations,
    ) {
        return None;
    }
    let admit = |index: usize| {
        let binding = &bindings[index];
        let source =
            three_binding_persistent_ready_source_v1(allocations.get(&binding.region.allocation)?)?;
        Some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source,
        })
    };
    Some(ThreeBindingPersistentComputeAdmissionV1 {
        bindings: [admit(0)?, admit(1)?, admit(2)?],
    })
}
pub(super) fn persistent_control_is_reused_v1(
    retained: Option<RetainedPersistentDispatchV1>,
    admission: Option<PersistentFullRangeComputeAdmissionV1>,
    dispatch_shape_sha256: [u8; 32],
) -> bool {
    retained
        .zip(admission)
        .is_some_and(|(retained, admission)| {
            retained.allocation == admission.allocation
                && retained.dispatch_shape_sha256 == dispatch_shape_sha256
        })
}

pub(super) fn persistent_full_range_compute_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocation: Option<&AllocationRecordV1>,
    ready: Option<PersistentComputeReadyFactsV1>,
) -> Option<PersistentFullRangeComputeAdmissionV1> {
    let [binding] = bindings else {
        return None;
    };
    let allocation = allocation?;
    let ready = ready?;
    let logical_bytes = u64::try_from(allocation.bytes.len()).ok()?;
    let max_window_bytes = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
        .checked_mul(u64::try_from(KFD_RUNTIME_MAX_SDMA_WINDOW_PACKETS_V1).ok()?)?;
    (semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && allocation.device == stream_device
        && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.sdma_backed
        && allocation.sdma_initialized
        && allocation.native_dirty.is_empty()
        && !allocation.sdma_shadow_dirty
        && allocation.content_sha256 == Some(ready.authenticated_sha256)
        && logical_bytes != 0
        && logical_bytes <= max_window_bytes
        && logical_bytes.is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
        && ready.logical_bytes == logical_bytes
        && ready.physical_bytes == logical_bytes
        && binding.region.byte_offset == 0
        && binding.region.byte_len == logical_bytes)
        .then_some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source: PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
        })
}

pub(super) fn retained_persistent_full_range_compute_admission_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    allocation: Option<&AllocationRecordV1>,
    retained: RetainedPersistentDispatchV1,
    dispatch_shape_sha256: [u8; 32],
) -> Option<PersistentFullRangeComputeAdmissionV1> {
    let [binding] = bindings else {
        return None;
    };
    let allocation = allocation?;
    let logical_bytes = u64::try_from(allocation.bytes.len()).ok()?;
    let initialized = match &allocation.sdma_storage {
        KfdRuntimeSdmaStorageV1::PersistentReplay(input) => input.is_fully_initialized(),
        #[cfg(test)]
        KfdRuntimeSdmaStorageV1::Device(_) => true,
        _ => return None,
    };
    (semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && retained.allocation == binding.region.allocation
        && retained.dispatch_shape_sha256 == dispatch_shape_sha256
        && allocation.device == stream_device
        && allocation.kind == RuntimeMemoryKindV1::DeviceLocal
        && allocation.sdma_backed
        && allocation.sdma_initialized
        && allocation.native_dirty.is_empty()
        && logical_bytes != 0
        && binding.region.byte_offset == 0
        && binding.region.byte_len == logical_bytes
        && (binding.region.access == RuntimeAccessV1::Write || initialized))
        .then_some(PersistentFullRangeComputeAdmissionV1 {
            allocation: binding.region.allocation,
            access: binding.region.access,
            source: PersistentFullRangeComputeSourceV1::RetainedControlReplay,
        })
}

pub(super) fn admitted_compute_lane_v1(
    available_lane: Option<usize>,
    persistent_selected: bool,
) -> Option<usize> {
    if persistent_selected {
        available_lane.filter(|lane| *lane == 0)
    } else {
        available_lane
    }
}

pub(super) fn early_pipeline_access_is_admitted_v1(bindings: &[BackendBindingV1]) -> bool {
    bindings.iter().all(|binding| {
        binding.region.access != RuntimeAccessV1::ReadWrite
            && !bindings.iter().any(|other| {
                other.region.allocation == binding.region.allocation
                    && other.region.access != binding.region.access
            })
    })
}

pub(super) fn early_pipeline_launch_is_admitted_v1(
    semantic_launch: KfdRuntimeSemanticLaunchV1,
    bindings: &[BackendBindingV1],
) -> bool {
    semantic_launch == KfdRuntimeSemanticLaunchV1::Ordinary
        && early_pipeline_access_is_admitted_v1(bindings)
}

#[cfg(test)]
pub(super) const fn explicit_dependency_succeeded_v1(status: BackendPollV1) -> bool {
    matches!(status, BackendPollV1::Succeeded)
}

pub(super) const fn ordered_predecessor_completed_v1(status: BackendPollV1) -> bool {
    !matches!(status, BackendPollV1::Pending)
}

pub(super) const fn ordered_successor_lane_matches_v1(
    stream_lane: Option<usize>,
    predecessor_lane: usize,
) -> bool {
    matches!(stream_lane, Some(lane) if lane == predecessor_lane)
}

pub(super) fn ordinary_compute_recipes_match_v1(
    left: &OwnedComputeLaunchV1,
    right: &OwnedComputeLaunchV1,
) -> bool {
    left == right
}

pub(super) fn retain_unique_native_dirty_extent_v1(
    dirty: &mut Vec<NativeDirtyExtentV1>,
    extent: NativeDirtyExtentV1,
) -> bool {
    if dirty.contains(&extent) {
        false
    } else {
        dirty.push(extent);
        true
    }
}

pub(super) fn apply_persistent_compute_effect_v1(
    record: &mut AllocationRecordV1,
    effect: Gfx942PersistentComputeEffectV1,
) {
    if effect.writes() {
        record.content_sha256 = None;
        record.last_full_host_write = None;
        record.sdma_shadow_dirty = true;
    }
}

pub(super) const fn persistent_compute_effect_v1(
    access: RuntimeAccessV1,
) -> Gfx942PersistentComputeEffectV1 {
    match access {
        RuntimeAccessV1::Read => Gfx942PersistentComputeEffectV1::Read,
        RuntimeAccessV1::Write => Gfx942PersistentComputeEffectV1::Write,
        RuntimeAccessV1::ReadWrite => Gfx942PersistentComputeEffectV1::ReadWrite,
    }
}
pub(super) fn recycled_dispatch_reuse_is_admitted_v1(
    recycled: &RecycledDispatchV1,
    dispatch_shape_sha256: [u8; 32],
    resident_descriptors: &[ResidentDataDescriptorV1],
    data: &[DataSpecV1],
) -> bool {
    recycled.dispatch_shape_sha256 == dispatch_shape_sha256
        && same_resident_storage_shape_v1(&recycled.descriptors, resident_descriptors)
        && data
            .iter()
            .all(|spec| spec.kind == RuntimeMemoryKindV1::HostVisible)
}

pub(super) fn host_visible_resident_roster_is_reusable_v1(
    descriptors: &[ResidentDataDescriptorV1],
    native_data_count: usize,
) -> bool {
    descriptors.len() == native_data_count
        && host_visible_resident_descriptors_are_reusable_v1(descriptors)
}

pub(super) fn host_visible_resident_descriptors_are_reusable_v1(
    descriptors: &[ResidentDataDescriptorV1],
) -> bool {
    !descriptors.is_empty()
        && descriptors.iter().all(|descriptor| {
            descriptor.kind == RuntimeMemoryKindV1::HostVisible && descriptor.byte_len != 0
        })
}

pub(super) fn resident_data_needs_host_overwrite_v1(
    prior: &ResidentDataDescriptorV1,
    current_host_sha256: Option<[u8; 32]>,
) -> bool {
    prior.device_may_have_modified
        || prior.host_content_sha256.is_none()
        || prior.host_content_sha256 != current_host_sha256
}

impl KfdRuntimeBackendV1 {
    #[cfg(test)]
    pub(super) fn finish_scripted_materialized_compute_v1(
        &mut self,
        mut active: ActiveSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        debug_assert!(active.writebacks.is_empty());
        debug_assert_eq!(
            active.performance.data_path,
            KfdRuntimeLaunchDataPathV1::Materialized
        );
        active.performance.completed_readback = Duration::ZERO;
        active.performance.completion_signal_recycle = Duration::ZERO;
        active.performance.completion_detach_restore = Duration::ZERO;
        let compute_lane = self.selected_compute_lane;
        let module = self
            .kernels
            .get(&active.kernel)
            .expect("active compute retains its kernel")
            .module;
        self.release_compute_custody_v1(active.id, module, active.allocations.iter().copied());
        let status = BackendPollV1::Succeeded;
        self.submissions.insert(
            active.id,
            SubmissionRecordV1 {
                origin: SubmissionOriginV1::Ordinary,
                stream: active.stream,
                status,
                dependency_depth: active.dependency_depth,
                profile_dispatch_published: true,
            },
        );
        self.compute_completion_reservations = self
            .compute_completion_reservations
            .checked_sub(1)
            .expect("published compute reserves one completion slot");
        self.release_compute_lane_lease_v1(active.stream, compute_lane);
        self.last_launch_performance = Some(active.performance);
        let profile_dispatch =
            self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, active.id);
        self.observe_profile_v1(profile_dispatch.map(|dispatch| {
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: profile_host_timing_v1(active.performance),
            }
        }));
        active.execution = None;
        Ok(status)
    }
}

pub(super) fn map_access_v1(access: RuntimeAccessV1) -> ArgumentAccess {
    match access {
        RuntimeAccessV1::Read => ArgumentAccess::ReadOnly,
        RuntimeAccessV1::Write => ArgumentAccess::WriteOnly,
        RuntimeAccessV1::ReadWrite => ArgumentAccess::ReadWrite,
    }
}
pub(super) fn snapshot_three_binding_persistent_data_v1(
    allocations: &AllocationTableV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
    admission: ThreeBindingPersistentComputeAdmissionV1,
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let current = three_binding_persistent_compute_admission_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        bindings,
        stream_device,
        allocations,
    );
    if current != Some(admission) {
        return Err(KfdRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::Busy,
            "three-binding persistent admission changed while snapshotting",
        ));
    }
    let mut data = Vec::with_capacity(3);
    let mut placements = HashMap::with_capacity(3);
    for (index, binding) in bindings.iter().enumerate() {
        let allocation = allocations
            .get(&binding.region.allocation)
            .expect("revalidated three-binding allocation");
        data.push(DataSpecV1 {
            allocation: binding.region.allocation,
            kind: allocation.kind,
            alignment: allocation.alignment,
            allocation_offset: 0,
            bytes: Arc::clone(&allocation.bytes),
            byte_range: 0..allocation.bytes.len(),
            content_sha256: if admission.bindings[index].source
                == PersistentFullRangeComputeSourceV1::InitializedStorage
            {
                None
            } else {
                allocation.content_sha256
            },
        });
        placements.insert(
            binding.region.allocation,
            StagedPlacementV1 {
                data_index: index,
                allocation_offset: 0,
            },
        );
    }
    Ok(StagedDataRosterV1 { data, placements })
}

pub(super) fn snapshot_persistent_full_range_data_v1(
    allocations: &AllocationTableV1,
    binding: &BackendBindingV1,
    stream_device: u64,
    admission: PersistentFullRangeComputeAdmissionV1,
    retained: Option<RetainedPersistentDispatchV1>,
    dispatch_shape_sha256: [u8; 32],
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    if admission.allocation != binding.region.allocation
        || admission.access != binding.region.access
    {
        return Err(KfdRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            "persistent-compute admission no longer matches its binding",
        ));
    }
    let allocation = allocations.get(&admission.allocation).ok_or_else(|| {
        KfdRuntimeBackendV1::rejected(
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
            "persistent-compute allocation disappeared",
        )
    })?;
    let current = match admission.source {
        PersistentFullRangeComputeSourceV1::InitializedStorage => {
            initialized_storage::initialized_storage_full_range_admission_v1(
                KfdRuntimeSemanticLaunchV1::Ordinary,
                core::slice::from_ref(binding),
                stream_device,
                Some(allocation),
            )
        }
        PersistentFullRangeComputeSourceV1::AuthenticatedH2d => allocation
            .sdma_storage
            .persistent_compute_ready_facts_v1()
            .and_then(|ready| {
                persistent_full_range_compute_admission_v1(
                    KfdRuntimeSemanticLaunchV1::Ordinary,
                    core::slice::from_ref(binding),
                    stream_device,
                    Some(allocation),
                    Some(ready),
                )
            }),
        PersistentFullRangeComputeSourceV1::RetainedControlReplay => {
            retained.and_then(|retained| {
                retained_persistent_full_range_compute_admission_v1(
                    KfdRuntimeSemanticLaunchV1::Ordinary,
                    core::slice::from_ref(binding),
                    stream_device,
                    Some(allocation),
                    retained,
                    dispatch_shape_sha256,
                )
            })
        }
    };
    current
        .filter(|actual| *actual == admission)
        .ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "persistent-compute admission changed while snapshotting",
            )
        })?;
    let mut data = Vec::new();
    data.try_reserve_exact(1)
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD persistent snapshot allocation failed"))?;
    data.push(DataSpecV1 {
        allocation: admission.allocation,
        kind: allocation.kind,
        alignment: allocation.alignment,
        allocation_offset: 0,
        bytes: Arc::clone(&allocation.bytes),
        byte_range: 0..allocation.bytes.len(),
        content_sha256: if admission.source
            == PersistentFullRangeComputeSourceV1::InitializedStorage
        {
            None
        } else {
            allocation.content_sha256
        },
    });
    let mut placements = HashMap::new();
    placements
        .try_reserve(1)
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD persistent placement allocation failed"))?;
    placements.insert(
        admission.allocation,
        StagedPlacementV1 {
            data_index: 0,
            allocation_offset: 0,
        },
    );
    Ok(StagedDataRosterV1 { data, placements })
}

pub(super) fn snapshot_bound_data_v1(
    allocations: &AllocationTableV1,
    bindings: &[BackendBindingV1],
    stream_device: u64,
) -> Result<StagedDataRosterV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut ranges = HashMap::<u64, (u64, u64)>::new();
    let mut order = Vec::<u64>::new();
    ranges
        .try_reserve(bindings.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-range map allocation failed"))?;
    order
        .try_reserve_exact(bindings.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-range order allocation failed"))?;

    for binding in bindings {
        let region = binding.region;
        let allocation = allocations.get(&region.allocation).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD allocation",
            )
        })?;
        if allocation.device != stream_device {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "allocation and stream belong to different devices",
            ));
        }
        if allocation.kind == RuntimeMemoryKindV1::DeviceLocal
            && region.access != RuntimeAccessV1::Read
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "device-local writeback is unavailable without an admitted copy path",
            ));
        }
        let range_end = region
            .byte_offset
            .checked_add(region.byte_len)
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "binding range overflow",
                )
            })?;
        if region.byte_len == 0 || range_end > allocation.bytes.len() as u64 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "binding lies outside its allocation",
            ));
        }
        let aligned_start = region.byte_offset & !(allocation.alignment - 1);
        if let Some((start, end)) = ranges.get_mut(&region.allocation) {
            *start = (*start).min(aligned_start);
            *end = (*end).max(range_end);
        } else {
            if order.len() == GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "fixed KFD dispatch data roster is full",
                ));
            }
            ranges.insert(region.allocation, (aligned_start, range_end));
            order.push(region.allocation);
        }
    }

    let mut data = Vec::new();
    let mut placements = HashMap::new();
    data.try_reserve_exact(order.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-data roster allocation failed"))?;
    placements
        .try_reserve(order.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD staged-placement map allocation failed"))?;
    for allocation_id in order {
        let allocation = &allocations[&allocation_id];
        let (start, end) = ranges[&allocation_id];
        let start_index = usize::try_from(start).map_err(|_| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "staged allocation offset does not fit host address space",
            )
        })?;
        let end_index = usize::try_from(end).map_err(|_| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "staged allocation end does not fit host address space",
            )
        })?;
        let data_index = data.len();
        data.push(DataSpecV1 {
            allocation: allocation_id,
            kind: allocation.kind,
            alignment: allocation.alignment,
            allocation_offset: start,
            bytes: Arc::clone(&allocation.bytes),
            byte_range: start_index..end_index,
            content_sha256: (start_index == 0 && end_index == allocation.bytes.len())
                .then_some(allocation.content_sha256)
                .flatten(),
        });
        placements.insert(
            allocation_id,
            StagedPlacementV1 {
                data_index,
                allocation_offset: start,
            },
        );
    }
    Ok(StagedDataRosterV1 { data, placements })
}

pub(super) fn build_program_v1<'a>(
    program: &'a ResidentKernelImageV1,
    signature: [u8; 32],
    owned_rows: &[OwnedAbiRowV1],
) -> Result<ValidatedKernelEnvelope<'a>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let arguments = program.selected_kernel().explicit_arguments();
    let mut rows = Vec::new();
    rows.try_reserve_exact(owned_rows.len()).map_err(|_| {
        KfdRuntimeBackendV1::capacity("KFD reconciled ABI roster allocation failed")
    })?;
    for row in owned_rows {
        let name = arguments[row.explicit_argument_index]
            .name()
            .expect("prepared global-buffer ABI row retains a source name");
        rows.push(KernelGlobalBufferAbiV1::new(
            row.explicit_argument_index,
            name,
            row.offset,
            row.pointee_alignment,
            row.access,
        ));
    }
    program
        .validated()
        .reconcile_dispatch_abi(signature, &rows)
        .map_err(|error| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                format!("typed AMDHSA dispatch ABI: {error:?}"),
            )
        })
}

pub(super) fn materialize_initial_data_v1(
    memory: &mut SharedGttMemorySessionV1,
    specs: Vec<DataSpecV1>,
    role_identity: [u8; 32],
) -> Result<Vec<Gfx942FixedDispatchDataV1>, String> {
    materialize_with_custody_v1(
        specs,
        "KFD native-data roster allocation failed",
        |index, spec| {
            let result = materialize_initial_data_item_v1(memory, spec, index, role_identity);
            #[cfg(test)]
            if result.is_err() {
                super::retained_release_tests::observe_materialization_failure_usage(
                    memory.host_visible_backing_usage_v1(),
                );
            }
            result
        },
        |custody| {
            // Test observation must not release owners even if the observer unwinds.
            let _retained = core::mem::ManuallyDrop::new(custody);
            #[cfg(test)]
            super::retained_release_tests::observe_materialization_retention(
                &_retained.specs,
                &_retained.data,
            );
        },
    )
}

fn materialize_initial_data_item_v1(
    memory: &mut SharedGttMemorySessionV1,
    spec: &DataSpecV1,
    index: usize,
    role_identity: [u8; 32],
) -> Result<Gfx942FixedDispatchDataV1, String> {
    match spec.kind {
        RuntimeMemoryKindV1::HostVisible => memory
            .initialize_host_visible_coherent_from_slice_v1(spec.bytes())
            .map(Gfx942FixedDispatchDataV1::host_visible_initialized)
            .map_err(|error| format!("KFD host-visible initialization: {error}")),
        RuntimeMemoryKindV1::DeviceLocal => {
            let owned_bytes = spec.try_owned_bytes()?;
            let ordinal = u32::try_from(index)
                .map_err(|_| "KFD device-content ordinal does not fit u32".to_owned())?;
            let role = Gfx942DeviceContentRoleV1::new(role_identity, ordinal)
                .map_err(|error| format!("KFD device-content role: {error}"))?;
            let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &owned_bytes)
                .map_err(|error| format!("KFD device-content descriptor: {error}"))?;
            memory
                .initialize_gfx942_device_memory(owned_bytes, spec.alignment, content)
                .map(Gfx942FixedDispatchDataV1::initialized)
                .map_err(|error| format!("KFD device-local initialization: {error}"))
        }
    }
}

pub(super) fn resident_descriptors_v1(
    specs: &[DataSpecV1],
) -> Result<Vec<ResidentDataDescriptorV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut descriptors = Vec::new();
    descriptors
        .try_reserve_exact(specs.len())
        .map_err(|_| KfdRuntimeBackendV1::capacity("KFD resident-data roster allocation failed"))?;
    for spec in specs {
        descriptors.push(ResidentDataDescriptorV1 {
            allocation: spec.allocation,
            kind: spec.kind,
            alignment: spec.alignment,
            allocation_offset: spec.allocation_offset,
            byte_len: u64::try_from(spec.bytes().len()).map_err(|_| {
                KfdRuntimeBackendV1::capacity("KFD resident-data extent does not fit u64")
            })?,
            host_content_sha256: spec.content_sha256,
            device_may_have_modified: false,
        });
    }
    Ok(descriptors)
}

pub(super) fn same_resident_storage_shape_v1(
    left: &[ResidentDataDescriptorV1],
    right: &[ResidentDataDescriptorV1],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.allocation == right.allocation
                && left.kind == right.kind
                && left.alignment == right.alignment
                && left.allocation_offset == right.allocation_offset
                && left.byte_len == right.byte_len
        })
}

struct ResidentReleaseCustodyV1<T> {
    _descriptors: Vec<ResidentDataDescriptorV1>,
    active: Option<T>,
    remaining: std::vec::IntoIter<T>,
    completed: usize,
}

fn with_resident_release_custody_v1<T>(
    descriptors: Vec<ResidentDataDescriptorV1>,
    data: Vec<T>,
    operation: impl FnOnce(&mut ResidentReleaseCustodyV1<T>) -> Result<(), String>,
    retain: impl FnOnce(ResidentReleaseCustodyV1<T>),
) -> Result<(), String> {
    let mut remaining = data.into_iter();
    let mut custody = ResidentReleaseCustodyV1 {
        _descriptors: descriptors,
        active: remaining.next(),
        remaining,
        completed: 0,
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(&mut custody))) {
        Ok(Ok(())) if custody.active.is_none() && custody.remaining.len() == 0 => Ok(()),
        Ok(Ok(())) => {
            retain(custody);
            Err("KFD resident-data release callback left owned data".to_owned())
        }
        Ok(Err(error)) => {
            retain(custody);
            Err(error)
        }
        Err(payload) => {
            retain(custody);
            std::panic::resume_unwind(payload)
        }
    }
}

fn release_resident_items_v1<T>(
    custody: &mut ResidentReleaseCustodyV1<T>,
    mut release: impl FnMut(T) -> Result<(), String>,
) -> Result<(), String> {
    while let Some(data) = custody.active.take() {
        // The consuming lower boundary roots this item before any fallible work.
        release(data)?;
        custody.completed += 1;
        custody.active = custody.remaining.next();
    }
    Ok(())
}

fn release_resident_data_v1(
    queue: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    custody: &mut ResidentReleaseCustodyV1<Gfx942FixedDispatchDataV1>,
) -> Result<(), String> {
    release_resident_items_v1(custody, |data| {
        queue
            .release_detached_fixed_dispatch_data(data)
            .map_err(|error| format!("KFD resident-data release: {error}"))
    })
}

pub(super) fn materialize_rebound_data_v1(
    queue: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    specs: Vec<DataSpecV1>,
    role_identity: [u8; 32],
) -> Result<Vec<Gfx942FixedDispatchDataV1>, String> {
    materialize_with_custody_v1(
        specs,
        "KFD rebound-data roster allocation failed",
        |index, spec| {
            queue
                .preflight_fixed_dispatch_data_insertion(index)
                .map_err(|error| format!("KFD dispatch-data insertion preflight: {error}"))?;
            let item = match spec.kind {
                RuntimeMemoryKindV1::HostVisible => queue
                    .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
                        index,
                        spec.bytes(),
                    )
                    .map_err(|error| format!("KFD host-visible insertion: {error}"))?,
                RuntimeMemoryKindV1::DeviceLocal => {
                    let owned_bytes = spec.try_owned_bytes()?;
                    let ordinal = u32::try_from(index)
                        .map_err(|_| "KFD device-content ordinal does not fit u32".to_owned())?;
                    let role = Gfx942DeviceContentRoleV1::new(role_identity, ordinal)
                        .map_err(|error| format!("KFD device-content role: {error}"))?;
                    let content =
                        Gfx942DeviceContentDescriptorV1::from_bytes(role, &owned_bytes)
                            .map_err(|error| format!("KFD device-content descriptor: {error}"))?;
                    queue
                        .insert_initialized_fixed_dispatch_data(
                            index,
                            owned_bytes,
                            spec.alignment,
                            content,
                        )
                        .map_err(|error| format!("KFD device-local insertion: {error}"))?
                }
            };
            Ok(item)
        },
        core::mem::forget,
    )
}

#[cfg(test)]
mod resident_release_tests;

#[cfg(test)]
mod borrowed_initialization_tests {
    use super::*;

    #[test]
    fn borrowed_initialization_runtime_preserves_both_materializers_and_device_owned_path() {
        let source = include_str!("compute_dispatch.rs");
        let initial = source
            .split_once("pub(super) fn materialize_initial_data_v1(")
            .unwrap()
            .1
            .split_once("fn materialize_initial_data_item_v1(")
            .unwrap()
            .0;
        assert!(initial.contains("materialize_with_custody_v1("));
        assert!(
            initial
                .contains("materialize_initial_data_item_v1(memory, spec, index, role_identity)")
        );
        for (start, end, borrowed) in [
            (
                "fn materialize_initial_data_item_v1(",
                "pub(super) fn resident_descriptors_v1(",
                "initialize_host_visible_coherent_from_slice_v1(spec.bytes())",
            ),
            (
                "pub(super) fn materialize_rebound_data_v1(",
                "#[cfg(test)]\nmod resident_release_tests;",
                "insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(index,spec.bytes()",
            ),
        ] {
            let body = source
                .split_once(start)
                .unwrap()
                .1
                .split_once(end)
                .unwrap()
                .0;
            let (before_device, device) = body
                .split_once("RuntimeMemoryKindV1::DeviceLocal =>")
                .unwrap();
            let host = before_device
                .split_once("RuntimeMemoryKindV1::HostVisible =>")
                .unwrap()
                .1;
            let normalized: String = host.chars().filter(|ch| !ch.is_whitespace()).collect();
            assert!(normalized.contains(borrowed));
            for forbidden in ["try_owned_bytes", "to_vec(", "to_owned(", "Box::from("] {
                assert!(!host.contains(forbidden));
            }
            assert!(device.contains("let owned_bytes = spec.try_owned_bytes()?"));
            assert!(device.contains("Gfx942DeviceContentDescriptorV1::from_bytes"));
            assert!(!body.contains("submit_"));
        }
    }

    #[test]
    fn borrowed_initialization_runtime_views_need_no_encoded_host_allocation() {
        let bytes: Arc<[u8]> = Arc::from([0x5a; 97]);
        for range in [0..97, 3..91] {
            let spec = DataSpecV1 {
                allocation: 7,
                kind: RuntimeMemoryKindV1::HostVisible,
                alignment: 4,
                allocation_offset: range.start as u64,
                bytes: Arc::clone(&bytes),
                byte_range: range.clone(),
                content_sha256: None,
            };
            let (view, allocations) = super::super::drain_capture::tests::counted(|| spec.bytes());
            assert_eq!(allocations, 0);
            assert!(std::ptr::eq(view.as_ptr(), bytes[range.clone()].as_ptr()));
            assert_eq!(view, &bytes[range]);
            let (owned, allocations) =
                super::super::drain_capture::tests::counted(|| spec.try_owned_bytes().unwrap());
            assert!(allocations >= 1);
            assert_eq!(&*owned, view);
            assert_ne!(owned.as_ptr(), view.as_ptr());
        }
    }
}

mod validation;

mod polling;

mod pending;

mod preparation;

mod publication;

mod persistent_publication;

mod shutdown;

mod resident_release;

mod semantic_contract;
pub(super) use semantic_contract::*;

#[cfg(test)]
mod test_source;
#[cfg(test)]
pub(super) use test_source::TEST_SOURCE_V1;
