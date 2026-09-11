//! Borrowed construction handoffs; the caller owns terminal settlement.

use super::*;
use crate::shared_memory::SharedGttAllocationIdentityV1;

#[cfg(test)]
#[path = "construction/tests.rs"]
mod tests;

pub(super) enum MappedRingV1 {
    AqlSpecial(SharedGttAllocationV1<AqlQueueGttV1, GttGpuAccessibleMutableV1>),
    ExecutableProbe(SharedGttAllocationV1<ExecutableAqlQueueProbeGttV1, GttGpuAccessibleMutableV1>),
    UserptrProbe(SharedGttAllocationV1<UserptrAqlQueueProbeGttV1, GttGpuAccessibleMutableV1>),
}

// Implementations only forward the existing memory primitives, never plan or allocate.
pub(super) trait RingMemoryV1 {
    fn preflight_cpu(&self, ring: &CpuRingAuthorityV1) -> Result<(), MemorySessionError>;
    fn preflight_mapped(&self, ring: &MappedRingV1) -> Result<(), MemorySessionError>;
    fn map(&mut self, ring: CpuRingAuthorityV1) -> Result<MappedRingV1, MemorySessionError>;
    fn retain(&mut self, ring: MappedRingV1) -> Result<RingAuthority, MemorySessionError>;
}

impl RingMemoryV1 for SharedGttMemorySessionV1 {
    fn preflight_cpu(&self, ring: &CpuRingAuthorityV1) -> Result<(), MemorySessionError> {
        match ring {
            CpuRingAuthorityV1::AqlSpecial(token) => self.preflight_cpu_queue_token_v1(token),
            CpuRingAuthorityV1::ExecutableProbe(token) => self.preflight_cpu_queue_token_v1(token),
            CpuRingAuthorityV1::UserptrProbe(token) => self.preflight_cpu_queue_token_v1(token),
        }
    }

    fn preflight_mapped(&self, ring: &MappedRingV1) -> Result<(), MemorySessionError> {
        match ring {
            MappedRingV1::AqlSpecial(token) => self.preflight_mapped_queue_token_v1(token),
            MappedRingV1::ExecutableProbe(token) => self.preflight_mapped_queue_token_v1(token),
            MappedRingV1::UserptrProbe(token) => self.preflight_mapped_queue_token_v1(token),
        }
    }

    fn map(&mut self, ring: CpuRingAuthorityV1) -> Result<MappedRingV1, MemorySessionError> {
        match ring {
            CpuRingAuthorityV1::AqlSpecial(token) => {
                self.map_to_gpu(token).map(MappedRingV1::AqlSpecial)
            }
            CpuRingAuthorityV1::ExecutableProbe(token) => {
                self.map_to_gpu(token).map(MappedRingV1::ExecutableProbe)
            }
            CpuRingAuthorityV1::UserptrProbe(token) => {
                self.map_to_gpu(token).map(MappedRingV1::UserptrProbe)
            }
        }
    }

    fn retain(&mut self, ring: MappedRingV1) -> Result<RingAuthority, MemorySessionError> {
        match ring {
            MappedRingV1::AqlSpecial(token) => self
                .retain_aql_ring_resource(token)
                .map(RingAuthority::AqlSpecial),
            MappedRingV1::ExecutableProbe(token) => self
                .retain_executable_aql_probe_ring_resource(token)
                .map(RingAuthority::ExecutableProbe),
            MappedRingV1::UserptrProbe(token) => self
                .retain_userptr_aql_probe_ring_resource(token)
                .map(RingAuthority::UserptrProbe),
        }
    }
}

pub(super) enum RingConstructionV1 {
    Cpu(CpuRingAuthorityV1),
    Mapped(MappedRingV1),
    Retained(RingAuthority),
    // This identifies custody in the same retained R88 session, not a usable token.
    #[allow(dead_code)]
    InSession(SharedGttAllocationIdentityV1),
    Transferred,
}

impl RingConstructionV1 {
    pub(super) fn map_in_place(
        &mut self,
        memory: &mut impl RingMemoryV1,
    ) -> Result<(), MemorySessionError> {
        let Self::Cpu(ring) = self else {
            return Err(MemorySessionError::Model("queue ring construction phase"));
        };
        // Rejection before R88 admission must leave the actual token here.
        memory.preflight_cpu(ring)?;
        let identity = match ring {
            CpuRingAuthorityV1::AqlSpecial(token) => token.storage_identity(),
            CpuRingAuthorityV1::ExecutableProbe(token) => token.storage_identity(),
            CpuRingAuthorityV1::UserptrProbe(token) => token.storage_identity(),
        };
        let Self::Cpu(ring) = core::mem::replace(self, Self::InSession(identity)) else {
            unreachable!("preflighted CPU ring")
        };
        *self = Self::Mapped(memory.map(ring)?);
        Ok(())
    }

    pub(super) fn retain_in_place(
        &mut self,
        memory: &mut impl RingMemoryV1,
    ) -> Result<(), MemorySessionError> {
        let Self::Mapped(ring) = self else {
            return Err(MemorySessionError::Model("queue ring construction phase"));
        };
        memory.preflight_mapped(ring)?;
        let identity = match ring {
            MappedRingV1::AqlSpecial(token) => token.storage_identity(),
            MappedRingV1::ExecutableProbe(token) => token.storage_identity(),
            MappedRingV1::UserptrProbe(token) => token.storage_identity(),
        };
        let Self::Mapped(ring) = core::mem::replace(self, Self::InSession(identity)) else {
            unreachable!("preflighted mapped ring")
        };
        *self = Self::Retained(memory.retain(ring)?);
        Ok(())
    }

    pub(super) fn take_retained(&mut self) -> Result<RingAuthority, MemorySessionError> {
        if !matches!(self, Self::Retained(_)) {
            return Err(MemorySessionError::Model("queue ring construction phase"));
        }
        let Self::Retained(ring) = core::mem::replace(self, Self::Transferred) else {
            unreachable!("checked retained ring")
        };
        Ok(ring)
    }
}

pub(super) struct QueueResourcePrefixV1 {
    ring: Option<RingAuthority>,
    control: Option<ControlAuthority>,
    eop: Option<EopAuthority>,
    context_save: Option<ContextSaveAuthority>,
    complete: Option<QueueResourceAuthorityV1>,
    started: bool,
}

impl QueueResourcePrefixV1 {
    pub(super) fn new(
        ring: RingAuthority,
        control: ControlAuthority,
        eop: EopAuthority,
        context_save: ContextSaveAuthority,
    ) -> Self {
        Self {
            ring: Some(ring),
            control: Some(control),
            eop: Some(eop),
            context_save: Some(context_save),
            complete: None,
            started: false,
        }
    }

    pub(super) fn build_in_place(
        &mut self,
        device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.build_with(device, geometry, &NEXT_QUEUE_INSTANCE)
    }

    fn build_with(
        &mut self,
        device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
        geometry: Gfx942AqlQueueResourcePlanV1,
        next_queue: &AtomicU64,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.started || self.complete.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "queue resource construction phase",
            ));
        }
        self.started = true;
        let missing = || ComputeAqlQueueSessionErrorV1::Contract("missing queue resource prefix");
        let view = build_resource_view(
            device,
            geometry,
            self.ring.as_ref().ok_or_else(missing)?,
            self.control.as_ref().ok_or_else(missing)?,
            self.eop.as_ref().ok_or_else(missing)?,
            self.context_save.as_ref().ok_or_else(missing)?,
            next_queue,
        )?;
        // Every check precedes extraction. These moves neither allocate nor call native code.
        self.complete = Some(QueueResourceAuthorityV1 {
            ring: self.ring.take().expect("validated ring"),
            control: self.control.take().expect("validated control"),
            eop: self.eop.take().expect("validated EOP"),
            context_save: self.context_save.take().expect("validated context-save"),
            view,
        });
        Ok(())
    }

    pub(super) fn take_complete(
        &mut self,
    ) -> Result<QueueResourceAuthorityV1, ComputeAqlQueueSessionErrorV1> {
        self.complete
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "queue resources are not complete",
            ))
    }
}
