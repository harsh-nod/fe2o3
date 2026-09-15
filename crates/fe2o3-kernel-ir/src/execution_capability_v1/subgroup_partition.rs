//! Bounded, target-neutral contiguous subgroup partition contracts.

use super::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "subgroup_partition/ordered_max_v1/tests.rs"]
mod ordered_max_tests;

pub const MAX_SUBGROUP_PARTITION_PHYSICAL_WIDTH_V1: u32 = 64;

pub const fn valid_subgroup_partition_widths_v1(width: u32, partition: u32) -> bool {
    matches!(width, 32 | 64) && partition != 0 && partition.is_power_of_two() && partition <= width
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SubgroupPartitionOperationV1 {
    Derive {
        subgroup_reference: ExecutionTypeIdentityV1,
        subgroup: ExecutionTypeIdentityV1,
        epoch: ExecutionTypeIdentityV1,
        partition: ExecutionTypeIdentityV1,
        width: u32,
        partition_width: u32,
    },
    ReduceSumF32 {
        partition_reference: ExecutionTypeIdentityV1,
        partition: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        width: u32,
        partition_width: u32,
    },
    BroadcastF32 {
        partition_reference: ExecutionTypeIdentityV1,
        partition: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        source_lane: ExecutionTypeIdentityV1,
        width: u32,
        partition_width: u32,
    },
    /// Ascending-XOR, stage-synchronous ordered comparison; equal/unordered keeps lhs bits.
    ReduceMaxF32 {
        partition_reference: ExecutionTypeIdentityV1,
        partition: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        width: u32,
        partition_width: u32,
    },
}

impl SubgroupPartitionOperationV1 {
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
        let (width, partition_width) = self.widths();
        valid_subgroup_partition_widths_v1(width, partition_width)
    }

    pub fn type_references(self) -> Vec<ExecutionTypeIdentityV1> {
        match self {
            Self::Derive {
                subgroup_reference,
                subgroup,
                epoch,
                partition,
                ..
            } => vec![subgroup_reference, subgroup, epoch, partition],
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
            } => vec![partition_reference, partition, element],
            Self::BroadcastF32 {
                partition_reference,
                partition,
                element,
                source_lane,
                ..
            } => vec![partition_reference, partition, element, source_lane],
        }
    }

    pub fn signature_matches(self, signature: ExecutionCapabilitySignatureV1) -> bool {
        let matches = |arguments: &[ExecutionTypeIdentityV1], output| {
            signature.arguments().eq(arguments.iter().copied()) && signature.output() == output
        };
        match self {
            Self::Derive {
                subgroup_reference,
                epoch,
                partition,
                ..
            } => matches(&[subgroup_reference, epoch], partition),
            Self::ReduceSumF32 {
                partition_reference,
                element,
                ..
            }
            | Self::ReduceMaxF32 {
                partition_reference,
                element,
                ..
            } => matches(&[partition_reference, element], element),
            Self::BroadcastF32 {
                partition_reference,
                element,
                source_lane,
                ..
            } => matches(&[partition_reference, element, source_lane], element),
        }
    }

    pub const fn operand_count(self) -> usize {
        match self {
            Self::Derive { .. } | Self::ReduceSumF32 { .. } | Self::ReduceMaxF32 { .. } => 2,
            Self::BroadcastF32 { .. } => 3,
        }
    }

    pub const fn obligations(self) -> u32 {
        use ExecutionSafetyObligationsV1 as O;
        let base = O::TARGET_SUPPORT | O::DYNAMIC_WORKGROUP_IDENTITY | O::LIFETIME_VALIDITY;
        match self {
            Self::Derive { .. } => base,
            Self::ReduceSumF32 { .. } | Self::ReduceMaxF32 { .. } => base | O::SUBGROUP_CONVERGENCE | O::EXACT_PARTICIPATION,
            Self::BroadcastF32 { .. } => {
                base | O::SUBGROUP_CONVERGENCE | O::EXACT_PARTICIPATION | O::BOUNDS
            }
        }
    }

    pub fn required_capabilities(self) -> BTreeSet<TargetCapability> {
        let (width, _) = self.widths();
        let mut required = BTreeSet::from([
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(width),
        ]);
        if !matches!(self, Self::Derive { .. }) {
            if let Some(wave) = match width {
                32 => Some(crate::WaveWidth::Wave32),
                64 => Some(crate::WaveWidth::Wave64),
                _ => None,
            } {
                required.insert(TargetCapability::WaveWidth(wave));
            }
        }
        required
    }

    pub(super) fn encode(self, writer: &mut ContractWriter) {
        writer.u8(match self {
            Self::Derive { .. } => 0,
            Self::ReduceSumF32 { .. } => 1,
            Self::BroadcastF32 { .. } => 2,
            Self::ReduceMaxF32 { .. } => 3,
        });
        for identity in self.type_references() {
            writer.identity(identity);
        }
        let (width, partition_width) = self.widths();
        writer.u32(width);
        writer.u32(partition_width);
    }

    pub(super) fn decode(reader: &mut ContractReader<'_>) -> Option<Self> {
        let tag = reader.u8()?;
        (tag <= 3).then_some(())?;
        let receiver = reader.identity()?;
        let receiver_type = reader.identity()?;
        let third = reader.identity()?;
        let fourth = if matches!(tag, 1 | 3) {
            None
        } else {
            Some(reader.identity()?)
        };
        let width = reader.u32()?;
        let partition_width = reader.u32()?;
        Some(match tag {
            0 => Self::Derive {
                subgroup_reference: receiver,
                subgroup: receiver_type,
                epoch: third,
                partition: fourth?,
                width,
                partition_width,
            },
            1 => Self::ReduceSumF32 {
                partition_reference: receiver,
                partition: receiver_type,
                element: third,
                width,
                partition_width,
            },
            3 => Self::ReduceMaxF32 {
                partition_reference: receiver,
                partition: receiver_type,
                element: third,
                width,
                partition_width,
            },
            2 => Self::BroadcastF32 {
                partition_reference: receiver,
                partition: receiver_type,
                element: third,
                source_lane: fourth?,
                width,
                partition_width,
            },
            _ => return None,
        })
    }
}
