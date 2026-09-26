//! LLVM AMDGPU pointer encoding, not hardware segment-address arithmetic.
//!
//! See https://llvm.org/docs/AMDGPUUsage.html#address-spaces. In particular,
//! LLVM private and local pointers use all-ones null, including when a hardware
//! memory table elsewhere uses a different convention for segment addresses.

use crate::ProductionAmdTargetProfileV1;

/// Representation facts conditional on the exact production AMDGPU profile.
/// This value supplies no source identity, pointer provenance or dereference right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmdPointerEncodingV1 {
    address_space: u32,
    bits: u16,
    null: u128,
}

impl AmdPointerEncodingV1 {
    /// The LLVM IR address space whose stored representation this describes.
    pub const fn address_space(self) -> u32 { self.address_space }

    /// Width of the stored LLVM pointer representation.
    pub const fn bits(self) -> u16 { self.bits }

    /// Integer bit pattern of an LLVM null pointer in this representation.
    pub const fn null_bits(self) -> u128 { self.null }

    /// Conservative bit domain of a separately authenticated non-null pointer.
    /// True means possible, not that the caller has established pointer validity.
    pub const fn nonnull_domain_contains(self, bits: u128) -> bool {
        bits < (1_u128 << self.bits) && bits != self.null
    }
}

impl ProductionAmdTargetProfileV1 {
    /// LLVM representation for the five address spaces admitted by typed KIR
    /// storage. Buffer resources and other non-integral representations are absent.
    pub const fn pointer_encoding(self, address_space: u32) -> Option<AmdPointerEncodingV1> {
        match self {
            Self::Gfx942 | Self::Gfx950 => match address_space {
                0 | 1 | 4 => Some(AmdPointerEncodingV1 { address_space, bits: 64, null: 0 }),
                3 | 5 => Some(AmdPointerEncodingV1 { address_space, bits: 32, null: u32::MAX as u128 }),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llvm_pointer_encoding_is_exact_for_both_profiles_and_all_admitted_spaces() {
        for profile in [ProductionAmdTargetProfileV1::Gfx942, ProductionAmdTargetProfileV1::Gfx950] {
            for (space, width, null) in [(0, 64, 0), (1, 64, 0), (3, 32, u32::MAX as u128),
                (4, 64, 0), (5, 32, u32::MAX as u128)] {
                let recipe = profile.pointer_encoding(space).unwrap();
                assert_eq!((recipe.address_space(), recipe.bits(), recipe.null_bits()), (space, width, null));
                assert!(!recipe.nonnull_domain_contains(null));
                assert!(!recipe.nonnull_domain_contains(1_u128 << width));
                assert!(recipe.nonnull_domain_contains(if null == 0 { 1 } else { 0 }));
            }
            for space in [2, 6, 7, 8, 9, 10, 13, 15, 128, u32::MAX] {
                assert_eq!(profile.pointer_encoding(space), None);
            }
        }
    }
}
