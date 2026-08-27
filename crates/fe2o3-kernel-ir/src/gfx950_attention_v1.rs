use std::collections::BTreeSet;

use crate::{
    AddressSpace, Convergence, MemoryEffect, SynchronizationScope, TargetCapability, ValueId,
    WaveWidth, gfx950_xnack_minus_target_capability,
};

/// Exact low-precision format staged through one gfx950 LDS transpose tile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx950LdsTransposeFormatV1 {
    Fp4E2M1,
    Fp8E4M3,
}

impl Gfx950LdsTransposeFormatV1 {
    /// Static byte extent of one 16x128 tile in its packed LDS representation.
    pub const fn lds_bytes(self) -> u32 {
        match self {
            Self::Fp4E2M1 => 1024,
            Self::Fp8E4M3 => 2048,
        }
    }

    /// Number of 64-bit transpose reads needed to produce eight operand dwords.
    pub const fn transpose_read_parts(self) -> u32 {
        match self {
            Self::Fp4E2M1 => 2,
            Self::Fp8E4M3 => 4,
        }
    }

    /// Per-lane packed byte stride used by the gfx950 transpose-read contract.
    pub const fn lane_byte_stride(self) -> u32 {
        match self {
            Self::Fp4E2M1 => 16,
            Self::Fp8E4M3 => 32,
        }
    }
}

/// One state transition in the exact gfx950 low-precision LDS transpose path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx950LdsTransposeOperationKindV1 {
    /// Declares one private static LDS tile for the current kernel entry.
    Current { format: Gfx950LdsTransposeFormatV1 },
    /// Stages a checked row-major token tile using the inverse transpose mapping.
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
    /// Publishes every staged byte through one uniform workgroup barrier.
    Publish {
        format: Gfx950LdsTransposeFormatV1,
        storage: ValueId,
    },
    /// Reads one published B fragment with gfx950 transpose instructions.
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

/// A typed, executable gfx950 LDS transpose operation.
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

/// Exact floating-point reduction retained by a V9 wave operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaveF32ReductionKindV1 {
    /// Fixed XOR-tree addition order.
    Sum,
    /// Ordered `less-than` plus select; this is not `fmax` or `maxnum`.
    Maximum,
}
