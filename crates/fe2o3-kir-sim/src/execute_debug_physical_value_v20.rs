//! Opaque CPU-only snapshots of real V20 symbolic SSA values.
//! None of these observations are hardware registers, native addresses or source custody.
use super::*;
use physical_entry_state_v20::{Half, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalEntryDebugSymbolicKindV20 {
    KernargLow,
    KernargHigh,
    OutputLow,
    OutputHigh,
    ScaledOffsetLow,
    ScaledOffsetHigh,
    AddressLow,
    AddressHigh,
    CarryLow,
    CarryHigh,
}

/// An immutable exact engine value, with no public constructor or bit projection.
/// Kept inline in the ordinary checkpoint binding; the enclosing typed capture
/// does not expose cloneable record references.
#[derive(Clone, Eq, PartialEq)]
pub struct PhysicalEntryDebugSymbolicV20 {
    value: Value,
    kind: PhysicalEntryDebugSymbolicKindV20,
}
impl PhysicalEntryDebugSymbolicV20 {
    pub(super) fn from_runtime(value: &Value) -> Option<Self> {
        use PhysicalEntryDebugSymbolicKindV20 as K;
        // A closed classification, intentionally not a catch-all over future
        // symbolic families. V21 pending values cannot become V20 observations.
        let kind = if matches!(
            value,
            Value::Kernarg {
                half: Half::Low,
                ..
            }
        ) {
            K::KernargLow
        } else if matches!(
            value,
            Value::Kernarg {
                half: Half::High,
                ..
            }
        ) {
            K::KernargHigh
        } else if matches!(
            value,
            Value::Output {
                half: Half::Low,
                ..
            }
        ) {
            K::OutputLow
        } else if matches!(
            value,
            Value::Output {
                half: Half::High,
                ..
            }
        ) {
            K::OutputHigh
        } else if matches!(
            value,
            Value::ScaledOffset {
                half: Half::Low,
                ..
            }
        ) {
            K::ScaledOffsetLow
        } else if matches!(
            value,
            Value::ScaledOffset {
                half: Half::High,
                ..
            }
        ) {
            K::ScaledOffsetHigh
        } else if matches!(value, Value::AddressLow(_)) {
            K::AddressLow
        } else if matches!(value, Value::AddressHigh { .. }) {
            K::AddressHigh
        } else if matches!(value, Value::CarryLow(_)) {
            K::CarryLow
        } else if matches!(value, Value::CarryHigh { .. }) {
            K::CarryHigh
        } else {
            return None;
        };
        Some(Self {
            value: value.clone(),
            kind,
        })
    }
    pub const fn kind(&self) -> PhysicalEntryDebugSymbolicKindV20 {
        self.kind
    }
    pub const fn scalar_type(&self) -> ScalarType {
        self.value.scalar_type()
    }
}
impl fmt::Debug for PhysicalEntryDebugSymbolicV20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicalEntryDebugSymbolicV20")
            .field("kind", &self.kind())
            .field("numeric_bits", &"unavailable")
            .finish()
    }
}

/// Borrowed observation only. No raw record, snapshot clone, or mutable access.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{PhysicalEntryDebugCaptureV20,SimulationDebugRecordV1};
/// fn escape(capture: &PhysicalEntryDebugCaptureV20) {
///     let _: SimulationDebugRecordV1 = capture.record(0).unwrap().clone();
/// }
/// ```
#[derive(Clone, Copy)]
pub struct PhysicalEntryDebugRecordRefV20<'a> {
    pub(super) record: &'a SimulationDebugRecordV1,
}
impl<'a> PhysicalEntryDebugRecordRefV20<'a> {
    pub fn ordinal(self) -> u64 {
        self.record.ordinal
    }
    pub fn invocation(self) -> SimulationInvocationV1 {
        self.record.invocation
    }
    pub fn site(self) -> SimulationDebugSiteV1 {
        self.record.site
    }
    pub fn phase(self) -> Option<SimulationDebugCheckpointPhaseV1> {
        match self.record.kind {
            SimulationDebugRecordKindV1::Checkpoint { phase, .. } => Some(phase),
            _ => None,
        }
    }
    pub fn frame_count(self) -> usize {
        match &self.record.kind {
            SimulationDebugRecordKindV1::Checkpoint {
                stack: SimulationDebugCollectionV1::Captured(frames),
                ..
            } => frames.len(),
            _ => 0,
        }
    }
    pub fn binding_count(self, depth: usize) -> Option<usize> {
        self.bindings(depth).map(<[_]>::len)
    }
    fn bindings(self, depth: usize) -> Option<&'a [SimulationDebugBindingV1]> {
        let SimulationDebugRecordKindV1::Checkpoint {
            stack: SimulationDebugCollectionV1::Captured(frames),
            ..
        } = &self.record.kind
        else {
            return None;
        };
        let SimulationDebugCollectionV1::Captured(values) = &frames.get(depth)?.values else {
            return None;
        };
        Some(values)
    }
    pub fn binding(
        self,
        depth: usize,
        index: usize,
    ) -> Option<PhysicalEntryDebugBindingRefV20<'a>> {
        Some(PhysicalEntryDebugBindingRefV20 {
            binding: self.bindings(depth)?.get(index)?,
        })
    }
    /// Allocation-relative CPU observation. Never a GPU address or target-memory read.
    pub fn memory_byte_at(self, allocation_index: usize, offset: usize) -> Option<(u8, bool)> {
        let SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Captured(memory),
            ..
        } = &self.record.kind
        else {
            return None;
        };
        let row = memory.get(allocation_index)?;
        Some((*row.bytes.get(offset)?, *row.initialized.get(offset)?))
    }
    pub fn memory_allocation(self, index: usize) -> Option<(u64, usize)> {
        let SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Captured(memory),
            ..
        } = &self.record.kind
        else {
            return None;
        };
        let row = memory.get(index)?;
        Some((row.allocation, row.bytes.len()))
    }
    pub fn memory_address_space(self, index: usize) -> Option<fe2o3_kernel_ir::AddressSpace> {
        let SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Captured(memory),
            ..
        } = &self.record.kind
        else {
            return None;
        };
        Some(memory.get(index)?.address_space)
    }
    pub fn is_committed_store(self) -> bool {
        matches!(
            self.record.kind,
            SimulationDebugRecordKindV1::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                ..
            }
        )
    }
}

/// Present symbolic bindings remain distinct from absent/out-of-scope bindings.
#[derive(Clone, Copy)]
pub struct PhysicalEntryDebugBindingRefV20<'a> {
    binding: &'a SimulationDebugBindingV1,
}
impl PhysicalEntryDebugBindingRefV20<'_> {
    pub fn value(self) -> ValueId {
        self.binding.value
    }
    pub fn scalar(self) -> Option<ScalarBitsV1> {
        match self.binding.observed {
            SimulationDebugValueV1::Scalar(value) => Some(value),
            _ => None,
        }
    }
    pub fn symbolic_kind(self) -> Option<PhysicalEntryDebugSymbolicKindV20> {
        match &self.binding.observed {
            SimulationDebugValueV1::PhysicalEntrySymbolicV20(value) => Some(value.kind()),
            _ => None,
        }
    }
    /// A CPU allocation-relative pointer/slice; symbolic halves never enter this projection.
    pub fn logical_pointer_address_space(self) -> Option<fe2o3_kernel_ir::AddressSpace> {
        match self.binding.observed {
            SimulationDebugValueV1::Pointer { address_space, .. }
            | SimulationDebugValueV1::Slice { address_space, .. } => Some(address_space),
            _ => None,
        }
    }
    pub fn logical_pointer(self) -> Option<(u64, usize)> {
        match self.binding.observed {
            SimulationDebugValueV1::Pointer {
                allocation,
                byte_offset,
                ..
            }
            | SimulationDebugValueV1::Slice {
                allocation,
                byte_offset,
                ..
            } => Some((allocation, byte_offset)),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "execute_debug_physical_value_v20_tests.rs"]
mod tests;
