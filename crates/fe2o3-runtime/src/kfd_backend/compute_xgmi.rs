//! Native peer transfers attached to the router's existing submission ledger.

use super::*;
use fe2o3_kfd::{
    Gfx942ComputeXgmiCopyWindowV1, Gfx942ComputeXgmiPacketPlanV1, Gfx942ComputeXgmiProgressV1,
    Gfx942ComputeXgmiQueueCreationRootV1, Gfx942ComputeXgmiQueueV1,
    Gfx942ComputeXgmiSegmentsPlanV1, Gfx942DirectionalQueuePersistentAllocationV1,
};

mod directed;
mod segments;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

#[derive(Clone, Copy, Debug)]
pub(super) enum Route {
    Native(Gfx942XgmiRouteV1),
    #[cfg(test)]
    Scripted {
        failure: Option<Stage>,
        unwind: bool,
        pending_samples: usize,
    },
}

pub(super) fn admit_native_route_v1(
    source: &KfdRuntimeBackendV1,
    destination: &KfdRuntimeBackendV1,
) -> Result<Route, KfdRuntimeBackendErrorV1> {
    let unopened = || {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            "compute-XGMI admission requires unopened compute sessions",
        )
    };
    let source = source.admitted_device.as_ref().ok_or_else(unopened)?;
    let destination = destination.admitted_device.as_ref().ok_or_else(unopened)?;
    source
        .topology_snapshot()
        .topology()
        .admit_gfx942_xgmi_route(
            source.observation().kfd_gpu_id(),
            destination.observation().kfd_gpu_id(),
        )
        .map(Route::Native)
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                format!("compute-XGMI route admission: {error}"),
            )
        })
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    Create,
    Copy,
    NextPacket,
    NextSegment,
    Poll,
    Finish,
    Retire,
    Restore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Prepared,
    Published,
    Ready,
    Retired,
}

enum Progress {
    Pending,
    Changed,
    Complete,
}

pub(super) struct Root {
    route: Route,
    window: Gfx942ComputeXgmiCopyWindowV1,
    segments: Option<Arc<Gfx942ComputeXgmiSegmentsPlanV1>>,
    phase: Phase,
    creation: Gfx942ComputeXgmiQueueCreationRootV1,
    queue: Option<Gfx942ComputeXgmiQueueV1>,
    owners: [Option<Gfx942DirectionalQueuePersistentAllocationV1>; 2],
    shells: [Option<Box<MaybeUninit<DirectionalSdmaDeviceOwnerV1>>>; 2],
    #[cfg(test)]
    scripted_owners: [Option<DirectionalSdmaDeviceOwnerV1>; 2],
    #[cfg(test)]
    trace: Vec<Stage>,
    #[cfg(test)]
    pending_samples: usize,
    #[cfg(test)]
    packet_plan: Gfx942ComputeXgmiPacketPlanV1,
    #[cfg(test)]
    packet_index: usize,
    #[cfg(test)]
    between_packets: bool,
    #[cfg(test)]
    segment_index: usize,
    #[cfg(test)]
    between_segments: bool,
}

impl fmt::Debug for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComputeXgmiRoot")
            .field("route", &self.route)
            .field("window", &self.window)
            .field(
                "segments",
                &self.segments.as_ref().map(|plan| {
                    (
                        plan.windows().len(),
                        plan.packet_count(),
                        plan.total_bytes(),
                    )
                }),
            )
            .field("phase", &self.phase)
            .field("creation_vacant", &self.creation.is_vacant())
            .field("queue", &self.queue.is_some())
            .field("owners", &self.owners.each_ref().map(Option::is_some))
            .finish()
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if !self.is_quiescent() {
            std::process::abort();
        }
    }
}

fn terminal(detail: impl Into<String>) -> Failure {
    RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Terminal,
        detail,
    ))
}

impl Root {
    fn prepare(route: Route, window: Gfx942ComputeXgmiCopyWindowV1) -> Result<Box<Self>, Failure> {
        Self::prepare_profile(route, window, None)
    }

    fn prepare_segments(
        route: Route,
        segments: Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    ) -> Result<Box<Self>, Failure> {
        Self::prepare_profile(route, segments.windows()[0], Some(segments))
    }

    fn prepare_profile(
        route: Route,
        window: Gfx942ComputeXgmiCopyWindowV1,
        segments: Option<Arc<Gfx942ComputeXgmiSegmentsPlanV1>>,
    ) -> Result<Box<Self>, Failure> {
        let capacity = || KfdRuntimeBackendV1::capacity("compute-XGMI owner allocation failed");
        let root = try_uninit_box_v1().map_err(|()| capacity())?;
        let shells = [
            Some(try_uninit_box_v1().map_err(|()| capacity())?),
            Some(try_uninit_box_v1().map_err(|()| capacity())?),
        ];
        #[cfg(test)]
        let trace = {
            let mut trace = Vec::new();
            let samples = match route {
                Route::Scripted {
                    pending_samples, ..
                } => pending_samples,
                Route::Native(_) => 0,
            };
            let stages = samples
                .checked_add(2)
                .and_then(|per_packet| {
                    per_packet.checked_mul(
                        segments
                            .as_ref()
                            .map_or(window.plan().count(), |plan| plan.packet_count()),
                    )
                })
                .and_then(|stages| {
                    stages.checked_add(segments.as_ref().map_or(0, |plan| plan.windows().len()))
                })
                .and_then(|stages| stages.checked_add(4))
                .ok_or_else(capacity)?;
            trace.try_reserve_exact(stages).map_err(|_| capacity())?;
            trace
        };
        Ok(Box::write(
            root,
            Self {
                route,
                window,
                segments,
                phase: Phase::Prepared,
                creation: Gfx942ComputeXgmiQueueCreationRootV1::new(),
                queue: None,
                owners: [None, None],
                shells,
                #[cfg(test)]
                scripted_owners: [None, None],
                #[cfg(test)]
                trace,
                #[cfg(test)]
                pending_samples: match route {
                    Route::Scripted {
                        pending_samples, ..
                    } => pending_samples,
                    Route::Native(_) => 0,
                },
                #[cfg(test)]
                packet_plan: window.plan(),
                #[cfg(test)]
                packet_index: 0,
                #[cfg(test)]
                between_packets: false,
                #[cfg(test)]
                segment_index: 0,
                #[cfg(test)]
                between_segments: false,
            },
        ))
    }

    pub(super) fn is_quiescent(&self) -> bool {
        #[cfg(test)]
        if self.scripted_owners.iter().any(Option::is_some) {
            return false;
        }
        self.creation.is_vacant() && self.queue.is_none() && self.owners.iter().all(Option::is_none)
    }

    pub(super) fn matches_window(&self, window: Gfx942ComputeXgmiCopyWindowV1) -> bool {
        self.segments.is_none() && self.window == window
    }

    pub(super) fn is_segmented(&self) -> bool {
        self.segments.is_some()
    }

    fn matches_regions(
        &self,
        source: &AllocationRecordV1,
        source_region: BackendMemoryRegionV1,
        destination: &AllocationRecordV1,
        destination_region: BackendMemoryRegionV1,
    ) -> bool {
        if let Some(plan) = &self.segments {
            return plan.source_logical_bytes() == source.bytes.len() as u64
                && plan.destination_logical_bytes() == destination.bytes.len() as u64
                && plan.source_offset() == source_region.byte_offset
                && plan.source_len() == source_region.byte_len
                && plan.destination_offset() == destination_region.byte_offset
                && plan.destination_len() == destination_region.byte_len;
        }
        source_region.byte_len == destination_region.byte_len
            && Gfx942ComputeXgmiCopyWindowV1::new(
                source.bytes.len() as u64,
                destination.bytes.len() as u64,
                source_region.byte_offset,
                destination_region.byte_offset,
                source_region.byte_len,
            ) == Some(self.window)
    }

    fn accepts_region(&self, record: &AllocationRecordV1, region: BackendMemoryRegionV1) -> bool {
        if self.segments.is_some() {
            checked_envelope(record, region)
        } else {
            checked_region(record, region)
        }
    }

    fn accepts(&self, owner: &DirectionalSdmaDeviceOwnerV1) -> bool {
        match (self.route, owner) {
            (Route::Native(_), DirectionalSdmaDeviceOwnerV1::Native(_)) => true,
            #[cfg(test)]
            (Route::Scripted { .. }, DirectionalSdmaDeviceOwnerV1::Scripted(_)) => true,
            #[cfg(test)]
            _ => false,
        }
    }

    fn take_owner(&mut self, index: usize, owner: DirectionalSdmaDeviceOwnerV1) {
        match owner {
            DirectionalSdmaDeviceOwnerV1::Native(owner) => self.owners[index] = Some(owner),
            #[cfg(test)]
            owner @ DirectionalSdmaDeviceOwnerV1::Scripted(_) => {
                self.scripted_owners[index] = Some(owner)
            }
        }
    }

    fn output_ready(&self, index: usize) -> bool {
        self.shells[index].is_some()
            && match self.route {
                Route::Native(_) => self.owners[index].is_some(),
                #[cfg(test)]
                Route::Scripted { .. } => self.scripted_owners[index].is_some(),
            }
    }

    fn take_output(&mut self, index: usize) -> Box<DirectionalSdmaDeviceOwnerV1> {
        let owner = match self.route {
            Route::Native(_) => DirectionalSdmaDeviceOwnerV1::Native(
                self.owners[index]
                    .take()
                    .unwrap_or_else(|| std::process::abort()),
            ),
            #[cfg(test)]
            Route::Scripted { .. } => self.scripted_owners[index]
                .take()
                .unwrap_or_else(|| std::process::abort()),
        };
        Box::write(
            self.shells[index]
                .take()
                .unwrap_or_else(|| std::process::abort()),
            owner,
        )
    }

    #[cfg(test)]
    fn script_step(&mut self, stage: Stage) -> Result<(), Failure> {
        self.trace.push(stage);
        if let Route::Scripted {
            failure: Some(failure),
            unwind,
            ..
        } = self.route
            && failure == stage
        {
            assert!(!unwind, "scripted compute-XGMI unwind at {stage:?}");
            return Err(terminal(format!(
                "scripted compute-XGMI failure at {stage:?}"
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    fn script_copy_window(&mut self, window: Gfx942ComputeXgmiCopyWindowV1) {
        let [source, destination] = &mut self.scripted_owners;
        let source_offset = window.source_offset() as usize;
        let destination_offset = window.destination_offset() as usize;
        let bytes = window.bytes() as usize;
        destination.as_mut().unwrap().scripted_bytes_mut().unwrap()
            [destination_offset..destination_offset + bytes]
            .copy_from_slice(
                &source.as_ref().unwrap().scripted_bytes().unwrap()
                    [source_offset..source_offset + bytes],
            );
    }

    fn progress(
        &mut self,
        source: &mut KfdRuntimeBackendV1,
        destination: &mut KfdRuntimeBackendV1,
    ) -> Result<Progress, Failure> {
        #[cfg(not(test))]
        let Route::Native(route) = self.route;
        #[cfg(test)]
        let route = match self.route {
            Route::Native(route) => route,
            Route::Scripted { .. } => {
                return match self.phase {
                    Phase::Prepared => {
                        self.script_step(Stage::Create)?;
                        self.script_step(Stage::Copy)?;
                        self.phase = Phase::Published;
                        Ok(Progress::Changed)
                    }
                    Phase::Published => {
                        if self.between_segments {
                            self.script_step(Stage::NextSegment)?;
                            self.segment_index += 1;
                            self.packet_index = 0;
                            self.packet_plan = self.segments.as_ref().unwrap().windows()
                                [self.segment_index]
                                .plan();
                            self.pending_samples = match self.route {
                                Route::Scripted {
                                    pending_samples, ..
                                } => pending_samples,
                                Route::Native(_) => unreachable!(),
                            };
                            self.between_segments = false;
                            return Ok(Progress::Changed);
                        }
                        if self.between_packets {
                            self.script_step(Stage::NextPacket)?;
                            self.packet_index += 1;
                            self.pending_samples = match self.route {
                                Route::Scripted {
                                    pending_samples, ..
                                } => pending_samples,
                                Route::Native(_) => unreachable!(),
                            };
                            self.between_packets = false;
                            return Ok(Progress::Changed);
                        }
                        self.script_step(Stage::Poll)?;
                        if self.pending_samples != 0 {
                            self.pending_samples -= 1;
                            return Ok(Progress::Pending);
                        }
                        if self.packet_index + 1 < self.packet_plan.count() {
                            self.between_packets = true;
                        } else {
                            if let Some(plan) = &self.segments {
                                let window = plan.windows()[self.segment_index];
                                self.between_segments =
                                    self.segment_index + 1 < plan.windows().len();
                                self.script_copy_window(window);
                            }
                            if !self.between_segments {
                                self.phase = Phase::Ready;
                            }
                        }
                        Ok(Progress::Changed)
                    }
                    Phase::Ready => {
                        self.script_step(Stage::Finish)?;
                        if self.segments.is_none() {
                            self.script_copy_window(self.window);
                        }
                        self.script_step(Stage::Retire)?;
                        self.script_step(Stage::Restore)?;
                        self.phase = Phase::Retired;
                        Ok(Progress::Complete)
                    }
                    Phase::Retired => {
                        Err(terminal("compute-XGMI root progressed after retirement"))
                    }
                };
            }
        };
        let (source, destination) = match (&mut source.queue, &mut destination.queue) {
            (Some(source), Some(destination)) => (source, destination),
            _ => return Err(terminal("compute-XGMI endpoint queue disappeared")),
        };
        match self.phase {
            Phase::Prepared => {
                self.queue = Some(
                    source
                        .create_native_xgmi_queue_with_peer_v1(
                            destination,
                            route,
                            &mut self.creation,
                        )
                        .map_err(|error| {
                            terminal(format!("compute-XGMI queue creation: {error}"))
                        })?,
                );
                let [source_owner, destination_owner] = &mut self.owners;
                let queue = self.queue.as_mut().unwrap_or_else(|| std::process::abort());
                let begin = if let Some(plan) = &self.segments {
                    queue.begin_persistent_data_segments_with_peer_v1(
                        source,
                        destination,
                        source_owner,
                        destination_owner,
                        Arc::clone(plan),
                    )
                } else {
                    queue.begin_persistent_data_range_with_peer_v1(
                        source,
                        destination,
                        source_owner,
                        destination_owner,
                        self.window,
                    )
                };
                begin.map_err(|error| terminal(format!("compute-XGMI publication: {error}")))?;
                self.phase = Phase::Published;
                Ok(Progress::Changed)
            }
            Phase::Published => {
                let progress = self
                    .queue
                    .as_mut()
                    .unwrap_or_else(|| std::process::abort())
                    .progress_persistent_data_full_extent_with_peer_v1(source, destination)
                    .map_err(|error| terminal(format!("compute-XGMI progress: {error}")))?;
                match progress {
                    Gfx942ComputeXgmiProgressV1::Pending => Ok(Progress::Pending),
                    Gfx942ComputeXgmiProgressV1::Changed => Ok(Progress::Changed),
                    Gfx942ComputeXgmiProgressV1::Ready => {
                        self.phase = Phase::Ready;
                        Ok(Progress::Changed)
                    }
                }
            }
            Phase::Ready => {
                let queue = self.queue.as_mut().unwrap_or_else(|| std::process::abort());
                let [source_owner, destination_owner] = &mut self.owners;
                queue
                    .finish_persistent_data_full_extent_with_peer_v1(
                        source,
                        destination,
                        source_owner,
                        destination_owner,
                    )
                    .map_err(|error| terminal(format!("compute-XGMI restoration: {error}")))?;
                source
                    .destroy_native_xgmi_queue_with_peer_v1(destination, queue)
                    .map_err(|error| terminal(format!("compute-XGMI queue retirement: {error}")))?;
                self.queue = None;
                self.phase = Phase::Retired;
                Ok(Progress::Complete)
            }
            Phase::Retired => Err(terminal("compute-XGMI root progressed after retirement")),
        }
    }
}

fn pair_mut<T>(values: &mut [T], source: usize, destination: usize) -> (&mut T, &mut T) {
    assert_ne!(source, destination);
    if source < destination {
        let (first, second) = values.split_at_mut(destination);
        (&mut first[source], &mut second[0])
    } else {
        let (first, second) = values.split_at_mut(source);
        (&mut second[0], &mut first[destination])
    }
}

pub(super) fn full_extent(record: &AllocationRecordV1, region: BackendMemoryRegionV1) -> bool {
    record.kind == RuntimeMemoryKindV1::DeviceLocal
        && record.sdma_backed
        && region.byte_offset == 0
        && region.byte_len != 0
        && Gfx942ComputeXgmiPacketPlanV1::new(region.byte_len).is_some()
        && region.byte_len == record.bytes.len() as u64
        && record.native_dirty.is_empty()
}

pub(super) fn checked_region(record: &AllocationRecordV1, region: BackendMemoryRegionV1) -> bool {
    checked_envelope(record, region)
        && Gfx942ComputeXgmiPacketPlanV1::new(region.byte_len).is_some()
}

fn checked_envelope(record: &AllocationRecordV1, region: BackendMemoryRegionV1) -> bool {
    record.kind == RuntimeMemoryKindV1::DeviceLocal
        && record.sdma_backed
        && region.byte_len != 0
        && region
            .byte_offset
            .checked_add(region.byte_len)
            .is_some_and(|end| end <= record.bytes.len() as u64)
        && record.native_dirty.is_empty()
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn prepare_compute_xgmi_plan_v1(
        &self,
        source: RoutedHandleV1,
        destination: RoutedHandleV1,
        window: Gfx942ComputeXgmiCopyWindowV1,
    ) -> Result<Box<Root>, Failure> {
        let route = self
            .compute_xgmi_routes
            .get(&(source.child, destination.child))
            .copied()
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "pending compute peer has no native route",
                )
            })?;
        Root::prepare(route, window)
    }

    pub(super) fn compute_xgmi_child_occupied_v1(&self, child: usize) -> bool {
        self.compute_xgmi_children[child].is_some()
    }

    pub(super) fn require_compute_xgmi_child_available_v1(
        &self,
        child: usize,
    ) -> Result<(), Failure> {
        if self.compute_xgmi_child_occupied_v1(child) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "KFD child is retained by a pending compute-XGMI transfer",
            ));
        }
        Ok(())
    }

    pub(super) fn compute_xgmi_stored_observation_v1(
        &self,
        route: RoutedHandleV1,
    ) -> Option<Result<BackendPollV1, Failure>> {
        if !self.compute_xgmi_child_occupied_v1(route.child) {
            return None;
        }
        let child = &self.children[route.child];
        if let Err(error) = child.require_live() {
            return Some(Err(error));
        }
        if child.quiescent_sdma_submissions.contains(&route.local) {
            return Some(Err(KfdRuntimeBackendV1::quiescent_error(
                KfdRuntimeBackendErrorKindV1::Native,
                "KFD SDMA submission is quiescent without a complete result",
            )));
        }
        Some(Ok(child
            .submissions
            .get(&route.local)
            .map_or(BackendPollV1::Pending, |record| record.status)))
    }

    /// Native compute-owned peer copies whose queue and both buffers were retired
    /// or restored before successful completion. This is an observation, not authority.
    pub fn completed_compute_xgmi_copies_v1(&self) -> u64 {
        self.completed_compute_xgmi_copies
    }

    /// Native peer copies retaining their queue and owners after publication.
    ///
    /// This stored observation does not sample a fence or drive progress. The
    /// hardware transfer may already be complete; the count is neither an
    /// authority to access its allocations nor evidence of execution overlap.
    pub fn retained_compute_xgmi_copies_v1(&self) -> usize {
        self.submissions
            .values()
            .filter(|submission| {
                matches!(submission, RoutedSubmissionV1::CooperativeCopy(copy)
                if copy.compute_xgmi.as_ref().is_some_and(|root| {
                    matches!(root.route, Route::Native(_))
                        && matches!(root.phase, Phase::Published | Phase::Ready)
                        && !root.is_quiescent()
                }))
            })
            .count()
    }

    pub(super) fn prepare_compute_xgmi_v1(
        &mut self,
        source: RoutedHandleV1,
        source_region: BackendMemoryRegionV1,
        destination: RoutedHandleV1,
        destination_region: BackendMemoryRegionV1,
        directed: bool,
    ) -> Result<Option<Box<Root>>, Failure> {
        let Some(route) = self
            .compute_xgmi_routes
            .get(&(source.child, destination.child))
            .copied()
        else {
            return Ok(None);
        };
        for (endpoint, region) in [(source, source_region), (destination, destination_region)] {
            let child = &self.children[endpoint.child];
            let Some(record) = child.allocations.get(&endpoint.local) else {
                return Ok(None);
            };
            if !child.peer_visible_device_allocations
                || !checked_region(record, region)
                || !record.sdma_initialized
            {
                return Ok(None);
            }
            match record.sdma_storage {
                KfdRuntimeSdmaStorageV1::Device(_)
                | KfdRuntimeSdmaStorageV1::H2dReady(_)
                | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                | KfdRuntimeSdmaStorageV1::InitializedStorage(_) => {}
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner))
                    if directed =>
                {
                    // The caller has already checked overlap ordering. Only the
                    // exact started directed owner can stand in for a local slot;
                    // its physical allocations remain exclusively in that root.
                    self.require_retained_directed_compute_xgmi_endpoint_v1(endpoint, owner)?;
                }
                _ => return Ok(None),
            }
        }
        if source_region.byte_len != destination_region.byte_len {
            return Ok(None);
        }
        let Some(window) = Gfx942ComputeXgmiCopyWindowV1::new(
            self.children[source.child].allocations[&source.local]
                .bytes
                .len() as u64,
            self.children[destination.child].allocations[&destination.local]
                .bytes
                .len() as u64,
            source_region.byte_offset,
            destination_region.byte_offset,
            source_region.byte_len,
        ) else {
            return Ok(None);
        };
        Root::prepare(route, window).map(Some)
    }

    fn require_retained_directed_compute_xgmi_endpoint_v1(
        &mut self,
        endpoint: RoutedHandleV1,
        owner: u64,
    ) -> Result<(), Failure> {
        let intact = matches!(self.submissions.get(&owner),
        Some(RoutedSubmissionV1::CooperativeCopy(copy))
            if copy.directed.is_some()
                && [copy.source, copy.destination].contains(&endpoint)
                && copy.phase == CooperativeCopyPhaseV1::Read
                && copy.compute_xgmi.as_ref().is_some_and(|root| {
                    matches!(root.phase, Phase::Published | Phase::Ready)
                        && !root.is_quiescent()
                }))
            && self.compute_xgmi_children.get(endpoint.child) == Some(&Some(owner))
            && self.directed_identity_is_intact_v1(owner);
        if !intact {
            return Err(self.directed_corruption_v1());
        }
        Ok(())
    }

    pub(super) fn compute_xgmi_endpoints_v1(&self, submission: u64) -> Option<[usize; 2]> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = self.submissions.get(&submission)? else {
            return None;
        };
        copy.compute_xgmi.as_ref()?;
        Some([copy.source.child, copy.destination.child])
    }

    pub(super) fn poison_compute_xgmi_children_v1(&mut self, endpoints: [usize; 2]) {
        self.terminal = true;
        for index in endpoints {
            self.children[index].terminal = true;
        }
        for index in endpoints {
            if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.children[index].poison_terminal_v1();
            })) {
                // Preserve first failure custody and still quarantine the other VM.
                core::mem::forget(payload);
            }
        }
    }

    pub(super) fn progress_compute_xgmi_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, Failure> {
        let (source, destination, regions) = match &self.submissions[&submission] {
            RoutedSubmissionV1::CooperativeCopy(copy) => (
                copy.source,
                copy.destination,
                [copy.source_region, copy.destination_region],
            ),
            _ => unreachable!("compute-XGMI submission remains indexed"),
        };
        let window_intact = matches!(&self.submissions[&submission],
        RoutedSubmissionV1::CooperativeCopy(copy) if copy.compute_xgmi.as_ref().is_some_and(|root| {
            root.matches_regions(
                &self.children[source.child].allocations[&source.local], regions[0],
                &self.children[destination.child].allocations[&destination.local], regions[1],
            )
        }));
        if !window_intact {
            return Err(terminal("compute-XGMI retained copy window changed"));
        }
        let fresh = match &self.submissions[&submission] {
            RoutedSubmissionV1::CooperativeCopy(copy) => {
                copy.compute_xgmi.as_ref().unwrap().is_quiescent()
            }
            _ => unreachable!(),
        };
        if fresh {
            if [source.child, destination.child]
                .into_iter()
                .any(|child| self.compute_xgmi_child_occupied_v1(child))
            {
                return Ok(BackendPollV1::Pending);
            }
            let origins = [
                self.peer_copy_origin_for_leg_v1(submission, PeerCopyLegV1::Read)?,
                self.peer_copy_origin_for_leg_v1(submission, PeerCopyLegV1::Write)?,
            ];
            for ((endpoint, region), origin) in [(source, regions[0]), (destination, regions[1])]
                .into_iter()
                .zip(origins)
            {
                let child = &self.children[endpoint.child];
                child.require_live()?;
                if child.peer_access_has_conflict_v1(
                    endpoint.local,
                    origin,
                    PeerAccessPurposeV1::Copy,
                ) || child.peer_access_has_conflict_v1(
                    endpoint.local,
                    origin,
                    PeerAccessPurposeV1::Reconcile,
                ) || child.any_compute_active_v1()
                    || !child.active_sdma.is_empty()
                    || child.native_reconciliations.iter().any(Option::is_some)
                {
                    return Ok(BackendPollV1::Pending);
                }
                let record = &child.allocations[&endpoint.local];
                if record.persistent_storage_restore.is_some() {
                    return Ok(BackendPollV1::Pending);
                }
                let RoutedSubmissionV1::CooperativeCopy(copy) = &self.submissions[&submission]
                else {
                    unreachable!()
                };
                if !copy
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .accepts_region(record, region)
                    || !record.sdma_initialized
                {
                    return Ok(self.fail_cooperative_copy(submission));
                }
            }
            for endpoint in [source, destination] {
                let child = &mut self.children[endpoint.child];
                let prepared = (|| {
                    // The peer queue requires VM-wide compute quiescence. The normal
                    // release path also reconciles unrelated cached materialized data.
                    child.release_retained_persistent_control_v1()?;
                    for lane in 0..child.native_compute_lanes.len() {
                        child.release_compute_lane_cache_v1(lane)?;
                    }
                    child.normalize_h2d_ready_v1(endpoint.local)
                })();
                match prepared {
                    Ok(()) => {}
                    Err(RuntimeBackendFailureV1::Rejected(error))
                        if error.kind() == KfdRuntimeBackendErrorKindV1::Busy =>
                    {
                        return Ok(BackendPollV1::Pending);
                    }
                    Err(RuntimeBackendFailureV1::Rejected(_))
                    | Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                        return Ok(self.fail_cooperative_copy(submission));
                    }
                    Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => return Err(failure),
                }
            }
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                self.submissions.get_mut(&submission).unwrap()
            else {
                unreachable!()
            };
            let root = copy.compute_xgmi.as_mut().unwrap();
            let (source_child, destination_child) =
                pair_mut(&mut self.children, source.child, destination.child);
            // Check both slots before extracting either owner. From extraction through
            // restoration, the submission itself owns every native failure prefix.
            let slots_ready = [
                (&*source_child, source, regions[0]),
                (&*destination_child, destination, regions[1]),
            ]
            .into_iter()
            .all(|(child, endpoint, region)| {
                let record = &child.allocations[&endpoint.local];
                let accepted = matches!(&record.sdma_storage,
                    KfdRuntimeSdmaStorageV1::Device(owner) if root.accepts(owner));
                root.accepts_region(record, region) && record.sdma_initialized && accepted
            });
            if !slots_ready {
                return Ok(self.fail_cooperative_copy(submission));
            }
            self.compute_xgmi_children[source.child] = Some(submission);
            self.compute_xgmi_children[destination.child] = Some(submission);
            for (index, child, endpoint) in [
                (0, &mut *source_child, source),
                (1, &mut *destination_child, destination),
            ] {
                let slot = &mut child
                    .allocations
                    .get_mut(&endpoint.local)
                    .unwrap()
                    .sdma_storage;
                let KfdRuntimeSdmaStorageV1::Device(owner) = core::mem::replace(
                    slot,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(
                        submission,
                    )),
                ) else {
                    std::process::abort();
                };
                root.take_owner(index, *owner);
            }
        }
        if [source.child, destination.child]
            .into_iter()
            .any(|child| self.compute_xgmi_children[child] != Some(submission))
        {
            return Err(terminal("compute-XGMI child custody changed"));
        }
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            self.submissions.get_mut(&submission).unwrap()
        else {
            unreachable!()
        };
        let root = copy.compute_xgmi.as_mut().unwrap();
        let (source_child, destination_child) =
            pair_mut(&mut self.children, source.child, destination.child);
        for (child, endpoint) in [(&*source_child, source), (&*destination_child, destination)] {
            child.require_live()?;
            if !matches!(child.allocations[&endpoint.local].sdma_storage,
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) if owner == submission)
            {
                return Err(terminal("compute-XGMI retained allocation slot changed"));
            }
        }
        match root.progress(source_child, destination_child)? {
            Progress::Pending => return Ok(BackendPollV1::Pending),
            Progress::Changed => {
                self.note_cooperative_progress();
                return Ok(BackendPollV1::Pending);
            }
            Progress::Complete => {}
        }
        for (index, child, endpoint) in [
            (0, &*source_child, source),
            (1, &*destination_child, destination),
        ] {
            if !root.output_ready(index)
                || !matches!(
                    child.allocations[&endpoint.local].sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) if owner == submission
                )
            {
                return Err(terminal("compute-XGMI restoration owner or slot changed"));
            }
        }
        source_child
            .allocations
            .get_mut(&source.local)
            .unwrap()
            .sdma_storage = KfdRuntimeSdmaStorageV1::Device(root.take_output(0));
        let record = destination_child
            .allocations
            .get_mut(&destination.local)
            .unwrap();
        record.sdma_storage = KfdRuntimeSdmaStorageV1::Device(root.take_output(1));
        record.sdma_initialized = true;
        record.sdma_shadow_dirty = true;
        record.content_sha256 = None;
        record.last_full_host_write = None;
        let native = matches!(root.route, Route::Native(_));
        self.compute_xgmi_children[source.child] = None;
        self.compute_xgmi_children[destination.child] = None;
        let status = self.finish_cooperative_copy(submission, CooperativeCopyPhaseV1::Succeeded);
        if native {
            self.completed_compute_xgmi_copies =
                self.completed_compute_xgmi_copies.saturating_add(1);
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests;
