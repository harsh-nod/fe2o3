//! Borrowed exact-pair transitions retaining native progress in caller custody.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) fn with_compute_transition<B: MemoryBackend, R>(
    engine: &mut SharedMemoryEngine<B>,
    operation: impl FnOnce(&mut SharedMemoryEngine<B>) -> R,
) -> R {
    match catch_unwind(AssertUnwindSafe(|| operation(engine))) {
        Ok(result) => result,
        Err(payload) => {
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            resume_unwind(payload)
        }
    }
}

pub(crate) struct ComputeXgmiBufferV1 {
    pub(super) local: Option<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>>,
    pub(super) unmapped: Option<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>>,
    pub(crate) peer: Option<Gfx942XgmiMappedDeviceMemoryV1>,
    pub(super) roster: Option<Box<[u32]>>,
    pub(super) progress: [transitions::NativeTransitionProgressV1; 4],
}

impl ComputeXgmiBufferV1 {
    pub(super) fn require_exact_roster(
        &self,
        expected: [u32; 2],
        mapping: bool,
    ) -> Result<(), MemorySessionError> {
        let retained = if mapping {
            self.roster.as_deref()
        } else {
            self.peer.as_ref().map(|peer| peer.gpu_ids())
        };
        if retained != Some(expected.as_slice()) {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        Ok(())
    }

    pub(crate) fn new(
        local: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        roster: Box<[u32]>,
    ) -> Self {
        Self {
            local: Some(local),
            unmapped: None,
            peer: None,
            roster: Some(roster),
            progress: Default::default(),
        }
    }

    pub(crate) fn take_local(
        &mut self,
    ) -> Option<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>> {
        self.local.take()
    }

    pub(super) fn local_unmap<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) -> Result<(), MemorySessionError> {
        let local = self
            .local
            .as_ref()
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        engine.unmap_device_memory_borrowed(local, &mut self.progress[0])?;
        self.unmapped = self.local.take().map(Gfx942DeviceMemoryLeaseV1::retag);
        Ok(())
    }

    pub(super) fn peer_map<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) -> Result<(), MemorySessionError> {
        let lease = self
            .unmapped
            .as_ref()
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        let index = engine.device_memory_index(lease, DeviceMemoryPhaseV1::Unmapped)?;
        let roster = self
            .roster
            .as_ref()
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        if lease.layout.uapi_flags != KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
            || roster.len() != 2
            || roster[0] >= roster[1]
            || !roster.contains(&engine.backend.gpu_id())
        {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        engine.check_currentness()?;
        let handle = engine.device_memory[index]
            .handle
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        self.peer = Some(Gfx942XgmiMappedDeviceMemoryV1 {
            lease: self
                .unmapped
                .take()
                .unwrap_or_else(|| std::process::abort())
                .retag(),
            gpu_ids: self.roster.take().unwrap_or_else(|| std::process::abort()),
            mapped_prefix: 0,
            unmapped_prefix: 0,
            map_succeeded: false,
            unmap_indeterminate: false,
        });
        let mapping = self.peer.as_mut().unwrap_or_else(|| std::process::abort());
        engine.device_memory[index].phase = DeviceMemoryPhaseV1::Ambiguous;
        self.progress[1].attempted = true;
        let outcome = engine.backend.map_gpu_ids(handle, &mapping.gpu_ids, 0);
        self.progress[1].returned_map_prefix = Some(outcome.value);
        self.progress[1].returned_success = Some(outcome.result.is_ok());
        if outcome.value <= 2 {
            mapping.mapped_prefix = outcome.value;
        }
        if let Err(error) = outcome.result {
            return engine.quarantine(error);
        }
        if outcome.value != 2 {
            return engine.quarantine(MemorySessionError::KernelResultMalformed(
                "compute-XGMI exact map prefix",
            ));
        }
        engine.check_currentness()?;
        engine.device_memory[index].phase = DeviceMemoryPhaseV1::Mapped;
        mapping.map_succeeded = true;
        Ok(())
    }

    pub(super) fn peer_unmap<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) -> Result<(), MemorySessionError> {
        let mapping = self
            .peer
            .as_mut()
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        if !mapping.is_fully_mapped() || mapping.gpu_ids.len() != 2 {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        let index = engine.device_memory_index(&mapping.lease, DeviceMemoryPhaseV1::Mapped)?;
        engine.check_currentness()?;
        let handle = engine.device_memory[index]
            .handle
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        engine.device_memory[index].phase = DeviceMemoryPhaseV1::Ambiguous;
        mapping.unmap_indeterminate = true;
        self.progress[2].attempted = true;
        let outcome = engine.backend.unmap_gpu_ids(handle, &mapping.gpu_ids, 0);
        self.progress[2].returned_map_prefix = Some(outcome.value);
        self.progress[2].returned_success = Some(outcome.result.is_ok());
        if outcome.value <= 2 {
            mapping.unmapped_prefix = outcome.value;
        }
        if let Err(error) = outcome.result {
            return engine.quarantine(error);
        }
        if outcome.value != 2 {
            return engine.quarantine(MemorySessionError::KernelResultMalformed(
                "compute-XGMI exact unmap prefix",
            ));
        }
        engine.device_memory[index].phase = DeviceMemoryPhaseV1::Unmapped;
        self.unmapped = self.peer.take().map(|mapping| mapping.lease.retag());
        // The already-unmapped authority is rooted before closing currentness.
        engine.check_currentness()
    }

    pub(super) fn local_map<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) -> Result<(), MemorySessionError> {
        let lease = self
            .unmapped
            .as_ref()
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        engine.map_device_memory_borrowed(lease, &mut self.progress[3])?;
        self.local = self.unmapped.take().map(Gfx942DeviceMemoryLeaseV1::retag);
        Ok(())
    }
}

impl SharedGttMemorySessionV1 {
    pub(crate) fn unmap_compute_xgmi_local_v1(
        &mut self,
        root: &mut ComputeXgmiBufferV1,
    ) -> Result<(), MemorySessionError> {
        with_compute_transition(&mut self.engine, |engine| root.local_unmap(engine))
    }

    pub(crate) fn map_compute_xgmi_local_v1(
        &mut self,
        root: &mut ComputeXgmiBufferV1,
    ) -> Result<(), MemorySessionError> {
        with_compute_transition(&mut self.engine, |engine| root.local_map(engine))
    }

    pub(crate) fn transition_compute_xgmi_peer_v1(
        &mut self,
        peer: &mut Self,
        route: crate::topology::Gfx942XgmiRouteV1,
        root: &mut ComputeXgmiBufferV1,
        mapping: bool,
    ) -> Result<(), MemorySessionError> {
        root.require_exact_roster(route.canonical_mapping_gpu_ids(), mapping)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_device_backing_pair_unwind_quarantine(self, peer, |session, peer| {
                let roster = route.canonical_mapping_gpu_ids();
                let a = session.engine.backend.gpu_id_value();
                let b = peer.engine.backend.gpu_id_value();
                if a == b || roster != [a.min(b), a.max(b)] {
                    return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
                }
                for memory in [&mut *session, &mut *peer] {
                    if memory
                        .engine
                        .backend
                        .retained_xgmi_route(route.source_gpu_id(), route.destination_gpu_id())?
                        != route
                    {
                        return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
                    }
                    memory.engine.backend.check_xgmi_route_currentness(route)?;
                }
                peer.engine.check_currentness()?;
                if mapping {
                    root.peer_map(&mut session.engine)?;
                } else {
                    root.peer_unmap(&mut session.engine)?;
                }
                peer.engine.check_currentness()
            })
        }));
        if !matches!(result, Ok(Ok(()))) {
            self.engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            peer.engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            if let Some(mapping) = root.peer.as_mut() {
                mapping.map_succeeded = false;
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}
