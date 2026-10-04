use std::{borrow::Borrow, cmp::Ordering, collections::BTreeSet};

use super::{TargetCapabilityNameRefV1, TargetCapabilityRefV1, lower_hex_digit};
use crate::TargetCapability;

// BTreeSet requires borrowed keys to have exactly the owned key's ordering.
// This private key uses TargetCapability's declaration order and field order;
// it deliberately excludes atomic coverage and diagnostic-alias support policy.
trait CapabilityBorrowedKeyV1 {
    fn capability_key_v1(&self) -> TargetCapabilityRefV1<'_>;
}

impl CapabilityBorrowedKeyV1 for TargetCapability {
    fn capability_key_v1(&self) -> TargetCapabilityRefV1<'_> {
        TargetCapabilityRefV1::from_owned(self)
    }
}

impl CapabilityBorrowedKeyV1 for TargetCapabilityRefV1<'_> {
    fn capability_key_v1(&self) -> TargetCapabilityRefV1<'_> {
        *self
    }
}

impl<'key> Borrow<dyn CapabilityBorrowedKeyV1 + 'key> for TargetCapability {
    fn borrow(&self) -> &(dyn CapabilityBorrowedKeyV1 + 'key) {
        self
    }
}

impl PartialEq for dyn CapabilityBorrowedKeyV1 + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for dyn CapabilityBorrowedKeyV1 + '_ {}

impl PartialOrd for dyn CapabilityBorrowedKeyV1 + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for dyn CapabilityBorrowedKeyV1 + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        self.capability_key_v1()
            .compare_key_v1(other.capability_key_v1())
    }
}

impl<'a> TargetCapabilityNameRefV1<'a> {
    fn lexical_bytes_v1(self) -> impl Iterator<Item = u8> + 'a {
        let (text, hex) = match self {
            Self::Text(text) => (Some(text), None),
            Self::LowerHex(bytes) => (None, Some(bytes)),
        };
        text.into_iter()
            .flat_map(str::bytes)
            .chain(hex.into_iter().flat_map(|bytes| {
                bytes
                    .iter()
                    .flat_map(|byte| [lower_hex_digit(byte >> 4), lower_hex_digit(byte & 0x0f)])
            }))
    }

    fn compare_name_v1(self, other: Self) -> Ordering {
        match (self, other) {
            (Self::Text(left), Self::Text(right)) => left.cmp(right),
            _ => self.lexical_bytes_v1().cmp(other.lexical_bytes_v1()),
        }
    }
}

impl TargetCapabilityRefV1<'_> {
    fn order_tag_v1(self) -> u8 {
        // Keep exhaustive and in TargetCapability's derived-Ord declaration order.
        match self {
            Self::Float16 => 0,
            Self::BFloat16 => 1,
            Self::Float64 => 2,
            Self::Int64 => 3,
            Self::Subgroups => 4,
            Self::SubgroupSize(_) => 5,
            Self::WorkgroupMemory => 6,
            Self::WorkgroupBarrier => 7,
            Self::Atomic { .. } => 8,
            Self::DynamicWorkgroupMemory => 9,
            Self::Extension { .. } => 10,
            Self::WaveWidth(_) => 11,
        }
    }

    fn compare_key_v1(self, other: Self) -> Ordering {
        let tag_order = self.order_tag_v1().cmp(&other.order_tag_v1());
        if !tag_order.is_eq() {
            return tag_order;
        }
        match self {
            Self::Float16
            | Self::BFloat16
            | Self::Float64
            | Self::Int64
            | Self::Subgroups
            | Self::WorkgroupMemory
            | Self::WorkgroupBarrier
            | Self::DynamicWorkgroupMemory => Ordering::Equal,
            Self::SubgroupSize(left) => {
                let Self::SubgroupSize(right) = other else {
                    unreachable!("equal exhaustive capability tags have equal variants")
                };
                left.cmp(&right)
            }
            Self::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => {
                let Self::Atomic {
                    width_bits: right_width,
                    address_space: right_space,
                    max_scope: right_scope,
                } = other
                else {
                    unreachable!("equal exhaustive capability tags have equal variants")
                };
                (width_bits, address_space, max_scope).cmp(&(right_width, right_space, right_scope))
            }
            Self::Extension { namespace, name } => {
                let Self::Extension {
                    namespace: right_namespace,
                    name: right_name,
                } = other
                else {
                    unreachable!("equal exhaustive capability tags have equal variants")
                };
                namespace
                    .cmp(right_namespace)
                    .then_with(|| name.compare_name_v1(right_name))
            }
            Self::WaveWidth(left) => {
                let Self::WaveWidth(right) = other else {
                    unreachable!("equal exhaustive capability tags have equal variants")
                };
                left.cmp(&right)
            }
        }
    }
}

pub(super) fn contains_v1(
    required: TargetCapabilityRefV1<'_>,
    supported: &BTreeSet<TargetCapability>,
) -> bool {
    // Both the query and the lazy hex encoder are borrowed stack values.
    let key: &dyn CapabilityBorrowedKeyV1 = &required;
    supported.contains(key)
}

#[cfg(test)]
#[path = "capability_borrowed_key_v1_tests.rs"]
mod tests;
