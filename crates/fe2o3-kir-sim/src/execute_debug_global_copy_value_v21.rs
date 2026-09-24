//! Opaque V21 CPU observations; no physical register or device-address bits.
use super::*;
use physical_entry_state_v20::{Half, Value};

#[cfg(test)]
#[path = "execute_debug_global_copy_value_v21_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalGlobalCopyDebugSymbolicKindV21 {
    KernargLow,
    KernargHigh,
    PendingPointerLow,
    PendingPointerHigh,
    PendingLengthLow,
    PendingLengthHigh,
    PendingGlobalRead,
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
pub struct PhysicalGlobalCopyDebugSymbolicV21 {
    value: Value,
    kind: PhysicalGlobalCopyDebugSymbolicKindV21,
}
impl PhysicalGlobalCopyDebugSymbolicV21 {
    pub(super) fn into_parts(self) -> (Value, PhysicalGlobalCopyDebugSymbolicKindV21) {
        (self.value, self.kind)
    }
    pub(super) fn from_runtime(value: &Value) -> Option<Self> {
        use PhysicalGlobalCopyDebugSymbolicKindV21 as K;
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
            // A V20 source-parameter family must not be relabelled V21.
            Value::Kernarg { .. } => return None,
        };
        Some(Self {
            value: value.clone(),
            kind,
        })
    }
    pub const fn kind(&self) -> PhysicalGlobalCopyDebugSymbolicKindV21 {
        self.kind
    }
    pub const fn scalar_type(&self) -> ScalarType {
        self.value.scalar_type()
    }
}
impl fmt::Debug for PhysicalGlobalCopyDebugSymbolicV21 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicalGlobalCopyDebugSymbolicV21")
            .field("kind", &self.kind)
            .field("numeric_bits", &"unavailable")
            .finish()
    }
}

/// A borrowed immutable checkpoint, never a cloneable raw snapshot or source map.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{PhysicalGlobalCopyDebugCaptureV21, SimulationDebugRecordV1};
/// fn escape(capture: &PhysicalGlobalCopyDebugCaptureV21) {
///     let _: SimulationDebugRecordV1 = capture.record(0).unwrap().clone();
/// }
/// ```
#[derive(Clone, Copy)]
pub struct PhysicalGlobalCopyDebugRecordRefV21<'a> {
    pub(super) record: PhysicalEntryDebugRecordRefV20<'a>,
}
impl<'a> PhysicalGlobalCopyDebugRecordRefV21<'a> {
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
    ) -> Option<PhysicalGlobalCopyDebugBindingRefV21<'a>> {
        Some(PhysicalGlobalCopyDebugBindingRefV21 {
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
    pub fn is_committed_store(self) -> bool {
        self.record.is_committed_store()
    }
}

#[derive(Clone, Copy)]
pub struct PhysicalGlobalCopyDebugBindingRefV21<'a> {
    binding: PhysicalEntryDebugBindingRefV20<'a>,
}
impl PhysicalGlobalCopyDebugBindingRefV21<'_> {
    pub fn value(self) -> ValueId {
        self.binding.value()
    }
    /// Only genuinely ready scalar values from the existing Engine are projected.
    pub fn scalar(self) -> Option<ScalarBitsV1> {
        self.binding.scalar()
    }
    pub fn symbolic_kind(self) -> Option<PhysicalGlobalCopyDebugSymbolicKindV21> {
        match &self.binding.binding.observed {
            SimulationDebugValueV1::PhysicalSymbolicV1(value) => value.global_copy_kind(),
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
