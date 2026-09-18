//! Closed source mapping for authenticated gfx942 integer instruction markers.
//! Source terminal codes 127..=132 are disjoint from reserved capability codes.

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
        TrustedAmdGpuInlineOperation::VMovB32 => 127,
        TrustedAmdGpuInlineOperation::VAddU32 => 128,
        TrustedAmdGpuInlineOperation::VSubU32 => 129,
        TrustedAmdGpuInlineOperation::VAndB32 => 130,
        TrustedAmdGpuInlineOperation::VOrB32 => 131,
        TrustedAmdGpuInlineOperation::VXorB32 => 132,
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
            (T::VMovB32, 127, "v_mov_b32", 1),
            (T::VAddU32, 128, "v_add_u32", 2),
            (T::VSubU32, 129, "v_sub_u32", 2),
            (T::VAndB32, 130, "v_and_b32", 2),
            (T::VOrB32, 131, "v_or_b32", 2),
            (T::VXorB32, 132, "v_xor_b32", 2),
        ];
        for (trusted, tag, mnemonic, arity) in cases {
            let semantic = semantic_operation(trusted).unwrap();
            assert_eq!(source_terminal_tag(trusted), tag);
            assert_eq!(input_count(trusted), arity);
            assert_eq!(semantic.input_count() as usize, arity);
            assert_eq!(semantic.instruction().mnemonic(), mnemonic);
            assert_eq!(
                semantic.option_bits(),
                SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS
            );
        }
    }
}
