use super::*;

#[test]
fn gfx942_inline_v30_markers_keep_exact_six_kind_mapping_and_reserved_source_ids() {
    use crate::production_inline_assembly_v30::{
        input_count, semantic_operation, source_terminal_tag,
    };
    use crate::production_semantic_terminal_v1::{
        ProductionSemanticTerminalRuleV1, ProductionTerminalExpansionV1,
        is_traversed_reviewed_helper_v1,
    };
    use crate::trusted_device_items::TrustedDeviceItem;
    use fe2o3_mir_model::semantic_mir_v1::SemanticGfx942InlineInstructionV30 as Kind;
    let cases = [
        (TrustedAmdGpuInlineOperation::VMovB32, Kind::VMovB32, 127, 1),
        (TrustedAmdGpuInlineOperation::VAddU32, Kind::VAddU32, 128, 2),
        (TrustedAmdGpuInlineOperation::VSubU32, Kind::VSubU32, 129, 2),
        (TrustedAmdGpuInlineOperation::VAndB32, Kind::VAndB32, 130, 2),
        (TrustedAmdGpuInlineOperation::VOrB32, Kind::VOrB32, 131, 2),
        (TrustedAmdGpuInlineOperation::VXorB32, Kind::VXorB32, 132, 2),
    ];
    for (operation, kind, tag, arity) in cases {
        let item = TrustedDeviceItem::AmdGpuInline(operation);
        let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
        assert_eq!(
            rule,
            ProductionSemanticTerminalRuleV1::Expand(
                ProductionTerminalExpansionV1::Gfx942InlineU32(operation)
            )
        );
        assert_eq!(rule.trusted_device_item(), item);
        assert!(!is_traversed_reviewed_helper_v1(item));
        assert_eq!(source_terminal_tag(operation), tag);
        assert!(!(122..=126).contains(&tag));
        assert_eq!(input_count(operation), arity);
        let semantic = semantic_operation(operation).unwrap();
        assert_eq!(semantic.instruction(), kind);
        assert_eq!(semantic.input_count(), arity);
        assert_eq!(semantic.option_bits(), 1);
    }
}
