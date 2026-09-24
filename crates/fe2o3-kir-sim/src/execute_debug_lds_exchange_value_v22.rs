//! Opaque V22 CPU observations; no physical register or device-address bits.
use super::*;
use physical_entry_state_v20::{Half, Value};

#[cfg(test)]
#[path = "execute_debug_lds_exchange_value_v22_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalLdsExchangeDebugSymbolicKindV22 {
    KernargLow,
    KernargHigh,
    PendingPointerLow,
    PendingPointerHigh,
    PendingLengthLow,
    PendingLengthHigh,
    PendingGlobalRead,
    PendingLdsRead,
    PointerLow,
    PointerHigh,
    ScaledOffsetLow,
    ScaledOffsetHigh,
    AddressLow,
    AddressHigh,
    CarryLow,
    CarryHigh,
}

/// Retains the exact inline Engine value/generation, with no public constructor
/// or numerical projection. Pending input bits are never observable through it.
#[derive(Clone, Eq, PartialEq)]
pub struct PhysicalLdsExchangeDebugSymbolicV22 {
    value: Value,
    kind: PhysicalLdsExchangeDebugSymbolicKindV22,
}
impl PhysicalLdsExchangeDebugSymbolicV22 {
    pub(super) fn into_parts(self) -> (Value, PhysicalLdsExchangeDebugSymbolicKindV22) {
        (self.value, self.kind)
    }
    pub(super) fn from_runtime(value: &Value) -> Option<Self> {
        use PhysicalLdsExchangeDebugSymbolicKindV22 as K;
        let kind = match value {
            Value::GlobalCopyKernarg {
                half: Half::Low, ..
            } => K::KernargLow,
            Value::GlobalCopyKernarg {
                half: Half::High, ..
            } => K::KernargHigh,
            Value::GlobalCopyPendingPointer {
                half: Half::Low, ..
            } => K::PendingPointerLow,
            Value::GlobalCopyPendingPointer {
                half: Half::High, ..
            } => K::PendingPointerHigh,
            Value::GlobalCopyPendingLength {
                half: Half::Low, ..
            } => K::PendingLengthLow,
            Value::GlobalCopyPendingLength {
                half: Half::High, ..
            } => K::PendingLengthHigh,
            Value::GlobalCopyPendingRead { .. } => K::PendingGlobalRead,
            Value::Output {
                half: Half::Low, ..
            } => K::PointerLow,
            Value::Output {
                half: Half::High, ..
            } => K::PointerHigh,
            Value::ScaledOffset {
                half: Half::Low, ..
            } => K::ScaledOffsetLow,
            Value::ScaledOffset {
                half: Half::High, ..
            } => K::ScaledOffsetHigh,
            Value::AddressLow(_) => K::AddressLow,
            Value::AddressHigh { .. } => K::AddressHigh,
            Value::CarryLow(_) => K::CarryLow,
            Value::CarryHigh { .. } => K::CarryHigh,
            Value::LdsPendingRead { .. } => K::PendingLdsRead,
            // A V20 source-parameter family must not be relabelled V22.
            Value::Kernarg { .. } => return None,
        };
        Some(Self {
            value: value.clone(),
            kind,
        })
    }
    pub const fn kind(&self) -> PhysicalLdsExchangeDebugSymbolicKindV22 {
        self.kind
    }
    pub const fn scalar_type(&self) -> ScalarType {
        self.value.scalar_type()
    }
}
impl fmt::Debug for PhysicalLdsExchangeDebugSymbolicV22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicalLdsExchangeDebugSymbolicV22")
            .field("kind", &self.kind)
            .field("numeric_bits", &"unavailable")
            .finish()
    }
}

/// A borrowed immutable checkpoint, never a cloneable raw snapshot or source map.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{PhysicalLdsExchangeDebugCaptureV22, SimulationDebugRecordV1};
/// fn escape(capture: &PhysicalLdsExchangeDebugCaptureV22) {
///     let _: SimulationDebugRecordV1 = capture.record(0).unwrap().clone();
/// }
/// ```
#[derive(Clone, Copy)]
pub struct PhysicalLdsExchangeDebugRecordRefV22<'a> {
    pub(super) record: PhysicalEntryDebugRecordRefV20<'a>,
}
impl<'a> PhysicalLdsExchangeDebugRecordRefV22<'a> {
    pub fn ordinal(self) -> u64 {
        self.record.ordinal()
    }
    pub fn invocation(self) -> SimulationInvocationV1 {
        self.record.invocation()
    }
    pub fn site(self) -> SimulationDebugSiteV1 {
        self.record.site()
    }
    pub fn phase(self) -> Option<SimulationDebugCheckpointPhaseV1> {
        self.record.phase()
    }
    pub fn frame_count(self) -> usize {
        self.record.frame_count()
    }
    pub fn binding_count(self, depth: usize) -> Option<usize> {
        self.record.binding_count(depth)
    }
    pub fn binding(
        self,
        depth: usize,
        index: usize,
    ) -> Option<PhysicalLdsExchangeDebugBindingRefV22<'a>> {
        Some(PhysicalLdsExchangeDebugBindingRefV22 {
            binding: self.record.binding(depth, index)?,
        })
    }
    pub fn memory_byte_at(self, allocation_index: usize, offset: usize) -> Option<(u8, bool)> {
        self.record.memory_byte_at(allocation_index, offset)
    }
    pub fn memory_allocation(self, index: usize) -> Option<(u64, usize)> {
        self.record.memory_allocation(index)
    }
    pub fn memory_address_space(self, index: usize) -> Option<fe2o3_kernel_ir::AddressSpace> {
        self.record.memory_address_space(index)
    }
    /// Existing Engine event only. Arrival is per invocation; Release records the
    /// full 128-participant group. It does not complete pending memory operations.
    pub fn barrier(self) -> Option<(SimulationDebugBarrierActionV1, u64, u32)> {
        match &self.record.record.kind {
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action,
                phase,
                participants,
            } => Some((*action, *phase, *participants)),
            _ => None,
        }
    }
    /// Actual recorded logical memory access; no device address or hidden queue.
    pub fn memory_access(
        self,
    ) -> Option<(
        SimulationDebugMemoryAccessV1,
        u64,
        usize,
        usize,
        AddressSpace,
    )> {
        match &self.record.record.kind {
            SimulationDebugRecordKindV1::Memory {
                access,
                allocation,
                byte_offset,
                byte_len,
                address_space,
                ..
            } => Some((
                *access,
                *allocation,
                *byte_offset,
                *byte_len,
                *address_space,
            )),
            _ => None,
        }
    }
    pub fn is_committed_store(self) -> bool {
        self.record.is_committed_store()
    }
}

#[derive(Clone, Copy)]
pub struct PhysicalLdsExchangeDebugBindingRefV22<'a> {
    binding: PhysicalEntryDebugBindingRefV20<'a>,
}
impl PhysicalLdsExchangeDebugBindingRefV22<'_> {
    pub fn value(self) -> ValueId {
        self.binding.value()
    }
    /// Only genuinely ready scalar values from the existing Engine are projected.
    pub fn scalar(self) -> Option<ScalarBitsV1> {
        self.binding.scalar()
    }
    pub fn symbolic_kind(self) -> Option<PhysicalLdsExchangeDebugSymbolicKindV22> {
        match &self.binding.binding.observed {
            SimulationDebugValueV1::PhysicalSymbolicV1(value) => value.lds_exchange_kind(),
            _ => None,
        }
    }
    pub fn logical_pointer(self) -> Option<(u64, usize)> {
        self.binding.logical_pointer()
    }
    pub fn logical_pointer_address_space(self) -> Option<fe2o3_kernel_ir::AddressSpace> {
        self.binding.logical_pointer_address_space()
    }
}
