use super::*;
use crate::production_semantic_terminal_v1::{
    ProductionExecutionTerminalV1 as T, ProductionSemanticTerminalRuleV1 as Rule,
    ProductionTerminalExpansionV1 as Expansion,
};

#[test]
fn transpose_source_terminals_have_exact_noncolliding_current_identity_tags() {
    for (terminal, id, arity) in [
        (T::Gfx950TransposeIssue, 47, 1),
        (T::Gfx950TransposeStageB4, 48, 4),
        (T::Gfx950TransposeStageB8, 49, 4),
        (T::Gfx950TransposePublish, 50, 2),
        (T::Gfx950TransposeReadB4, 51, 2),
        (T::Gfx950TransposeReadB8, 52, 2),
    ] {
        assert_eq!(terminal.identity_tag(), id);
        assert_eq!(terminal.source_argument_count(), arity);
        assert_eq!(
            Rule::from_trusted_device_item(terminal.trusted_device_item()),
            Rule::Expand(Expansion::Execution(terminal))
        );
        assert_eq!(
            terminal_operation_tag_for_schema_v1(
                Expansion::Execution(terminal),
                TerminalIdentitySchemaV1::CombinedV5
            ),
            126 + id
        );
        assert!((173..=178).contains(&(126 + id)));
        assert!(
            !crate::production_semantic_terminal_v1::is_traversed_reviewed_helper_v1(
                terminal.trusted_device_item()
            )
        );
    }
    for (operation, id) in [
        (Expansion::Gfx950LdsTransposeTileCurrent, 80),
        (Expansion::Gfx950LdsTransposeStageB4, 81),
        (Expansion::Gfx950LdsTransposeStageB8, 82),
        (Expansion::Gfx950LdsTransposePublish, 83),
        (Expansion::Gfx950LdsTransposeReadB4, 84),
        (Expansion::Gfx950LdsTransposeReadB8, 85),
    ] {
        assert_eq!(
            terminal_operation_tag_for_schema_v1(operation, TerminalIdentitySchemaV1::CombinedV5),
            id
        );
    }
}

#[test]
fn transpose_type_and_format_markers_do_not_issue_source_authority() {
    for item in [
        TrustedDeviceItem::Gfx950LdsTransposeTile,
        TrustedDeviceItem::Gfx950Fp4E2M1Format,
        TrustedDeviceItem::Gfx950Fp8E4M3Format,
    ] {
        assert_eq!(Rule::from_trusted_device_item(item), Rule::Reject(item));
    }
}
