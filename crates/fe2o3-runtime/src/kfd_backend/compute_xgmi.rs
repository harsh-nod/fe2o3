//! Native peer transfers attached to the router's existing submission ledger.

use super::*;
use fe2o3_kfd::{
    Gfx942ComputeXgmiQueueCreationRootV1, Gfx942ComputeXgmiQueueV1,
    Gfx942DirectionalQueuePersistentAllocationV1,
};

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

#[derive(Clone, Copy, Debug)]
pub(super) enum Route {
    Native(Gfx942XgmiRouteV1),
    #[cfg(test)]
    Scripted {
        failure: Option<Stage>,
        unwind: bool,
    },
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    Create,
    Copy,
    Retire,
    Restore,
}

pub(super) struct Root {
    route: Route,
    creation: Gfx942ComputeXgmiQueueCreationRootV1,
    queue: Option<Gfx942ComputeXgmiQueueV1>,
    owners: [Option<Gfx942DirectionalQueuePersistentAllocationV1>; 2],
    shells: [Option<Box<MaybeUninit<DirectionalSdmaDeviceOwnerV1>>>; 2],
    #[cfg(test)]
    scripted_owners: [Option<DirectionalSdmaDeviceOwnerV1>; 2],
    #[cfg(test)]
    trace: Vec<Stage>,
}

impl fmt::Debug for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComputeXgmiRoot")
            .field("route", &self.route)
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
    fn prepare(route: Route) -> Result<Box<Self>, Failure> {
        let capacity = || KfdRuntimeBackendV1::capacity("compute-XGMI owner allocation failed");
        let root = try_uninit_box_v1().map_err(|()| capacity())?;
        let shells = [
            Some(try_uninit_box_v1().map_err(|()| capacity())?),
            Some(try_uninit_box_v1().map_err(|()| capacity())?),
        ];
        #[cfg(test)]
        let trace = {
            let mut trace = Vec::new();
            trace.try_reserve_exact(4).map_err(|_| capacity())?;
            trace
        };
        Ok(Box::write(
            root,
            Self {
                route,
                creation: Gfx942ComputeXgmiQueueCreationRootV1::new(),
                queue: None,
                owners: [None, None],
                shells,
                #[cfg(test)]
                scripted_owners: [None, None],
                #[cfg(test)]
                trace,
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

    fn execute(
        &mut self,
        source: &mut KfdRuntimeBackendV1,
        destination: &mut KfdRuntimeBackendV1,
        _byte_len: usize,
    ) -> Result<(), Failure> {
        #[cfg(not(test))]
        let Route::Native(route) = self.route;
        #[cfg(test)]
        let route = match self.route {
            Route::Native(route) => route,
            Route::Scripted { .. } => {
                self.script_step(Stage::Create)?;
                self.script_step(Stage::Copy)?;
                let [source, destination] = &mut self.scripted_owners;
                destination.as_mut().unwrap().scripted_bytes_mut().unwrap()[.._byte_len]
                    .copy_from_slice(
                        &source.as_ref().unwrap().scripted_bytes().unwrap()[.._byte_len],
                    );
                self.script_step(Stage::Retire)?;
                return self.script_step(Stage::Restore);
            }
        };
        let (source, destination) = match (&mut source.queue, &mut destination.queue) {
            (Some(source), Some(destination)) => (source, destination),
            _ => return Err(terminal("compute-XGMI endpoint queue disappeared")),
        };
        self.queue = Some(
            source
                .create_native_xgmi_queue_with_peer_v1(destination, route, &mut self.creation)
                .map_err(|error| terminal(format!("compute-XGMI queue creation: {error}")))?,
        );
        let queue = self.queue.as_mut().unwrap_or_else(|| std::process::abort());
        let [source_owner, destination_owner] = &mut self.owners;
        queue
            .copy_persistent_data_full_extent_with_peer_v1(
                source,
                destination,
                source_owner,
                destination_owner,
                Duration::from_secs(30),
            )
            .map_err(|error| terminal(format!("compute-XGMI transfer: {error}")))?;
        source
            .destroy_native_xgmi_queue_with_peer_v1(destination, queue)
            .map_err(|error| terminal(format!("compute-XGMI queue retirement: {error}")))?;
        self.queue = None;
        Ok(())
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

fn full_extent(record: &AllocationRecordV1, region: BackendMemoryRegionV1) -> bool {
    record.kind == RuntimeMemoryKindV1::DeviceLocal
        && record.sdma_backed
        && region.byte_offset == 0
        && region.byte_len != 0
        && region.byte_len <= u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
        && region.byte_len == record.bytes.len() as u64
        && record.native_dirty.is_empty()
}

impl KfdMultiDeviceRuntimeBackendV1 {
    /// Native compute-owned peer copies whose queue and both buffers were retired
    /// or restored before successful completion. This is an observation, not authority.
    #[cfg(feature = "hardware-qualification")]
    pub fn completed_compute_xgmi_copies_v1(&self) -> u64 {
        self.completed_compute_xgmi_copies
    }

    #[cfg(feature = "hardware-qualification")]
    pub(super) fn admit_compute_xgmi_routes_v1(&mut self) -> Result<(), KfdRuntimeBackendErrorV1> {
        let mut routes = HashMap::new();
        routes
            .try_reserve(
                self.children
                    .len()
                    .saturating_mul(self.children.len().saturating_sub(1)),
            )
            .map_err(|_| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "compute-XGMI route roster allocation failed",
                )
            })?;
        for (source_index, source) in self.children.iter().enumerate() {
            for (destination_index, destination) in self.children.iter().enumerate() {
                if source_index == destination_index {
                    continue;
                }
                let source = source.admitted_device.as_ref().ok_or_else(|| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "compute-XGMI admission requires unopened compute sessions",
                    )
                })?;
                let destination = destination.admitted_device.as_ref().ok_or_else(|| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "compute-XGMI admission requires unopened compute sessions",
                    )
                })?;
                let route = source
                    .topology_snapshot()
                    .topology()
                    .admit_gfx942_xgmi_route(
                        source.observation().kfd_gpu_id(),
                        destination.observation().kfd_gpu_id(),
                    )
                    .map_err(|error| {
                        KfdRuntimeBackendErrorV1::new(
                            KfdRuntimeBackendErrorKindV1::Unsupported,
                            format!("compute-XGMI route admission: {error}"),
                        )
                    })?;
                routes.insert((source_index, destination_index), Route::Native(route));
            }
        }
        self.compute_xgmi_routes = routes;
        Ok(())
    }

    pub(super) fn prepare_compute_xgmi_v1(
        &self,
        source: RoutedHandleV1,
        source_region: BackendMemoryRegionV1,
        destination: RoutedHandleV1,
        destination_region: BackendMemoryRegionV1,
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
            if !child.peer_visible_device_allocations
                || !child
                    .allocations
                    .get(&endpoint.local)
                    .is_some_and(|record| {
                        full_extent(record, region)
                            && record.sdma_initialized
                            && matches!(
                                record.sdma_storage,
                                KfdRuntimeSdmaStorageV1::Device(_)
                                    | KfdRuntimeSdmaStorageV1::H2dReady(_)
                                    | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                                    | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
                            )
                    })
            {
                return Ok(None);
            }
        }
        Root::prepare(route).map(Some)
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
        for (endpoint, region) in [(source, regions[0]), (destination, regions[1])] {
            let child = &self.children[endpoint.child];
            child.require_live()?;
            if child.allocation_is_active(endpoint.local)
                || child.any_compute_active_v1()
                || !child.active_sdma.is_empty()
                || child.native_reconciliations.iter().any(Option::is_some)
            {
                return Ok(BackendPollV1::Pending);
            }
            let record = &child.allocations[&endpoint.local];
            if record.persistent_storage_restore.is_some() {
                return Ok(BackendPollV1::Pending);
            }
            if !full_extent(record, region) || !record.sdma_initialized {
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
        ].into_iter().all(|(child, endpoint, region)| {
            let record = &child.allocations[&endpoint.local];
            full_extent(record, region) && record.sdma_initialized
                && matches!(&record.sdma_storage, KfdRuntimeSdmaStorageV1::Device(owner) if root.accepts(owner))
        });
        if !slots_ready {
            return Ok(self.fail_cooperative_copy(submission));
        }
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
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
            ) else {
                std::process::abort();
            };
            root.take_owner(index, *owner);
        }
        root.execute(
            source_child,
            destination_child,
            regions[0].byte_len as usize,
        )?;
        for (index, child, endpoint) in [
            (0, &*source_child, source),
            (1, &*destination_child, destination),
        ] {
            if !root.output_ready(index)
                || !matches!(
                    child.allocations[&endpoint.local].sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous)
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
