//! Closed source mapping for authenticated gfx942 integer instruction markers.
//! Source terminal codes 138..=143 are disjoint from saturation and Wave64 codes.
//! Historical V30 type names do not denote the standalone MIR34 wire version.

use crate::trusted_device_items::TrustedAmdGpuInlineOperation;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGfx942InlineInstructionV30, SemanticGfx942InlineU32V30, SemanticMirErrorV1,
};

pub(crate) const fn input_count(operation: TrustedAmdGpuInlineOperation) -> usize {
    match operation {
        TrustedAmdGpuInlineOperation::VMovB32 => 1,
        TrustedAmdGpuInlineOperation::VAddU32
        | TrustedAmdGpuInlineOperation::VSubU32
        | TrustedAmdGpuInlineOperation::VAndB32
        | TrustedAmdGpuInlineOperation::VOrB32
        | TrustedAmdGpuInlineOperation::VXorB32 => 2,
    }
}

pub(crate) const fn source_terminal_tag(operation: TrustedAmdGpuInlineOperation) -> u8 {
    match operation {
        TrustedAmdGpuInlineOperation::VMovB32 => 138,
        TrustedAmdGpuInlineOperation::VAddU32 => 139,
        TrustedAmdGpuInlineOperation::VSubU32 => 140,
        TrustedAmdGpuInlineOperation::VAndB32 => 141,
        TrustedAmdGpuInlineOperation::VOrB32 => 142,
        TrustedAmdGpuInlineOperation::VXorB32 => 143,
    }
}

pub(crate) fn semantic_operation(
    operation: TrustedAmdGpuInlineOperation,
) -> Result<SemanticGfx942InlineU32V30, SemanticMirErrorV1> {
    use SemanticGfx942InlineInstructionV30 as I;
    SemanticGfx942InlineU32V30::new(
        match operation {
            TrustedAmdGpuInlineOperation::VMovB32 => I::VMovB32,
            TrustedAmdGpuInlineOperation::VAddU32 => I::VAddU32,
            TrustedAmdGpuInlineOperation::VSubU32 => I::VSubU32,
            TrustedAmdGpuInlineOperation::VAndB32 => I::VAndB32,
            TrustedAmdGpuInlineOperation::VOrB32 => I::VOrB32,
            TrustedAmdGpuInlineOperation::VXorB32 => I::VXorB32,
        },
        SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_source_terminal_codes_arity_and_options_match_semantic_catalog() {
        use TrustedAmdGpuInlineOperation as T;
        let cases = [
            (T::VMovB32, 138, "v_mov_b32", 1),
            (T::VAddU32, 139, "v_add_u32", 2),
            (T::VSubU32, 140, "v_sub_u32", 2),
            (T::VAndB32, 141, "v_and_b32", 2),
            (T::VOrB32, 142, "v_or_b32", 2),
            (T::VXorB32, 143, "v_xor_b32", 2),
        ];
        for (trusted, tag, mnemonic, arity) in cases {
            let semantic = semantic_operation(trusted).unwrap();
            assert_eq!(source_terminal_tag(trusted), tag);
            assert_eq!(input_count(trusted), arity);
            assert_eq!(semantic.input_count(), arity);
            assert_eq!(semantic.instruction().mnemonic(), mnemonic);
            assert_eq!(
                semantic.option_bits(),
                SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
            );
        }
    }
}
