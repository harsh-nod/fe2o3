//! Closed contiguous partitions of one physical subgroup and workgroup epoch.

use super::*;

/// The derivation borrows an existing subgroup and its workgroup epoch. It
/// neither issues independent authority nor asserts dynamic convergence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticSubgroupPartitionOperationV1 {
    Derive {
        subgroup_reference: SemanticTypeIdV1,
        subgroup: SemanticTypeIdV1,
        epoch: SemanticTypeIdV1,
        partition: SemanticTypeIdV1,
        width: u32,
        partition_width: u32,
    },
    ReduceSumF32 {
        partition_reference: SemanticTypeIdV1,
        partition: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
        width: u32,
        partition_width: u32,
    },
    BroadcastF32 {
        partition_reference: SemanticTypeIdV1,
        partition: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
        source_lane: SemanticTypeIdV1,
        width: u32,
        partition_width: u32,
    },
    /// Ascending-XOR, stage-synchronous ordered comparison; equal/unordered keeps lhs bits.
    ReduceMaxF32 {
        partition_reference: SemanticTypeIdV1,
        partition: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
        width: u32,
        partition_width: u32,
    },
}

impl SemanticSubgroupPartitionOperationV1 {
    pub const fn widths(self) -> (u32, u32) {
        match self {
            Self::Derive {
                width,
                partition_width,
                ..
            }
            | Self::ReduceSumF32 {
                width,
                partition_width,
                ..
            }
            | Self::ReduceMaxF32 {
                width,
                partition_width,
                ..
            }
            | Self::BroadcastF32 {
                width,
                partition_width,
                ..
            } => (width, partition_width),
        }
    }

    pub fn is_well_formed(self) -> bool {
        let (width, partition) = self.widths();
        matches!(width, 32 | 64)
            && partition != 0
            && partition.is_power_of_two()
            && partition <= width
    }

    pub(super) fn signature_matches(
        self,
        signature: SemanticExecutionCapabilitySignatureV1,
    ) -> bool {
        match self {
            Self::Derive {
                subgroup_reference,
                epoch,
                partition,
                ..
            } => signature.matches(&[subgroup_reference, epoch], partition),
            Self::ReduceSumF32 {
                partition_reference,
                element,
                ..
            }
            | Self::ReduceMaxF32 {
                partition_reference,
                element,
                ..
            } => signature.matches(&[partition_reference, element], element),
            Self::BroadcastF32 {
                partition_reference,
                element,
                source_lane,
                ..
            } => signature.matches(&[partition_reference, element, source_lane], element),
        }
    }

    pub(super) fn type_references(self) -> [Option<SemanticTypeIdV1>; 8] {
        let mut references = [None; 8];
        let ids = match self {
            Self::Derive {
                subgroup_reference,
                subgroup,
                epoch,
                partition,
                ..
            } => [
                Some(subgroup_reference),
                Some(subgroup),
                Some(epoch),
                Some(partition),
            ],
            Self::ReduceSumF32 {
                partition_reference,
                partition,
                element,
                ..
            }
            | Self::ReduceMaxF32 {
                partition_reference,
                partition,
                element,
                ..
            } => [
                Some(partition_reference),
                Some(partition),
                Some(element),
                None,
            ],
            Self::BroadcastF32 {
                partition_reference,
                partition,
                element,
                source_lane,
                ..
            } => [
                Some(partition_reference),
                Some(partition),
                Some(element),
                Some(source_lane),
            ],
        };
        references[..4].copy_from_slice(&ids);
        references
    }

    pub const fn obligations(self) -> u32 {
        use SemanticExecutionSafetyObligationsV1 as O;
        let base = O::TARGET_SUPPORT | O::DYNAMIC_WORKGROUP_IDENTITY | O::LIFETIME_VALIDITY;
        match self {
            Self::Derive { .. } => base,
            Self::ReduceSumF32 { .. } | Self::ReduceMaxF32 { .. } => base | O::SUBGROUP_CONVERGENCE | O::EXACT_PARTICIPATION,
            Self::BroadcastF32 { .. } => {
                base | O::SUBGROUP_CONVERGENCE | O::EXACT_PARTICIPATION | O::BOUNDS
            }
        }
    }

    pub(super) fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        let tag = match self {
            Self::Derive { .. } => 0,
            Self::ReduceSumF32 { .. } => 1,
            Self::BroadcastF32 { .. } => 2,
            Self::ReduceMaxF32 { .. } => 3,
        };
        writer.u8(tag)?;
        for ty in self.type_references().into_iter().flatten() {
            writer.u32(ty.0)?;
        }
        let (width, partition_width) = self.widths();
        writer.u32(width)?;
        writer.u32(partition_width)
    }
}
