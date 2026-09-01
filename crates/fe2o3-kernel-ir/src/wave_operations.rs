use std::collections::BTreeSet;

use crate::{
    AddressSpace, Convergence, MemoryEffect, SynchronizationScope, TargetCapability, ValueId,
    WaveWidth, gfx950_xnack_minus_target_capability,
};

/// Low-precision format staged through a gfx950 LDS transpose tile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx950LdsTransposeFormatV1 {
    Fp4E2M1,
    Fp8E4M3,
}

impl Gfx950LdsTransposeFormatV1 {
    pub const fn lds_bytes(self) -> u32 {
        match self {
            Self::Fp4E2M1 => 1024,
            Self::Fp8E4M3 => 2048,
        }
    }

    pub const fn transpose_read_parts(self) -> u32 {
        match self {
            Self::Fp4E2M1 => 2,
            Self::Fp8E4M3 => 4,
        }
    }

    pub const fn lane_byte_stride(self) -> u32 {
        match self {
            Self::Fp4E2M1 => 16,
            Self::Fp8E4M3 => 32,
        }
    }
}

/// One state transition in the exact gfx950 LDS transpose target extension.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx950LdsTransposeOperationKindV1 {
    Current {
        format: Gfx950LdsTransposeFormatV1,
    },
    Stage {
        format: Gfx950LdsTransposeFormatV1,
        storage: ValueId,
        source_slice: ValueId,
        offset: ValueId,
        rows: ValueId,
        columns: ValueId,
        stride: ValueId,
        token_base: ValueId,
        reduction_base: ValueId,
    },
    Publish {
        format: Gfx950LdsTransposeFormatV1,
        storage: ValueId,
    },
    Read {
        format: Gfx950LdsTransposeFormatV1,
        storage: ValueId,
    },
}

impl Gfx950LdsTransposeOperationKindV1 {
    pub fn operands(self) -> Vec<ValueId> {
        match self {
            Self::Current { .. } => Vec::new(),
            Self::Stage {
                storage,
                source_slice,
                offset,
                rows,
                columns,
                stride,
                token_base,
                reduction_base,
                ..
            } => vec![
                storage,
                source_slice,
                offset,
                rows,
                columns,
                stride,
                token_base,
                reduction_base,
            ],
            Self::Publish { storage, .. } | Self::Read { storage, .. } => vec![storage],
        }
    }

    pub const fn format(self) -> Gfx950LdsTransposeFormatV1 {
        match self {
            Self::Current { format }
            | Self::Stage { format, .. }
            | Self::Publish { format, .. }
            | Self::Read { format, .. } => format,
        }
    }
}

/// A typed gfx950 LDS transpose operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx950LdsTransposeOperationV1 {
    pub kind: Gfx950LdsTransposeOperationKindV1,
    pub width: WaveWidth,
    pub active_lanes: u32,
    pub convergence: Convergence,
}

impl Gfx950LdsTransposeOperationV1 {
    pub fn full(kind: Gfx950LdsTransposeOperationKindV1) -> Self {
        Self {
            kind,
            width: WaveWidth::Wave64,
            active_lanes: 64,
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }
    }

    pub fn operands(self) -> Vec<ValueId> {
        self.kind.operands()
    }

    pub fn required_capabilities(self) -> BTreeSet<TargetCapability> {
        BTreeSet::from([
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(64),
            TargetCapability::WaveWidth(WaveWidth::Wave64),
            TargetCapability::WorkgroupMemory,
            gfx950_xnack_minus_target_capability(),
        ])
    }

    pub fn memory_effects(self) -> Vec<MemoryEffect> {
        match self.kind {
            Gfx950LdsTransposeOperationKindV1::Current { .. } => {
                vec![MemoryEffect::Allocate(AddressSpace::Workgroup)]
            }
            Gfx950LdsTransposeOperationKindV1::Stage { .. } => vec![
                MemoryEffect::Read(AddressSpace::Global),
                MemoryEffect::Write(AddressSpace::Workgroup),
            ],
            Gfx950LdsTransposeOperationKindV1::Publish { .. } => {
                vec![MemoryEffect::Synchronize {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
                }]
            }
            Gfx950LdsTransposeOperationKindV1::Read { .. } => {
                vec![MemoryEffect::Read(AddressSpace::Workgroup)]
            }
        }
    }
}

/// Floating-point reduction retained by a V9 wave operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaveF32ReductionKindV1 {
    Sum,
    Maximum,
}
