#[test]
fn ordered_program_preflight_allocates_only_combined_v5_without_old_selector_changes() {
    let program = ProductionTerminalExpansionV1::Gfx942OrderedProgramE32;
    assert_eq!(
        terminal_identity_schema_v1(program),
        TerminalIdentitySchemaV1::CombinedV5
    );
    assert_eq!(
        terminal_expansion_tag_for_schema_v1(program, TerminalIdentitySchemaV1::CombinedV5),
        134
    );
    for schema in [
        TerminalIdentitySchemaV1::IndependentV1,
        TerminalIdentitySchemaV1::CombinedV2,
        TerminalIdentitySchemaV1::CombinedV3,
        TerminalIdentitySchemaV1::CombinedV4,
    ] {
        assert_eq!(
            terminal_expansion_tag_for_schema_v1(program, schema),
            u8::MAX
        );
    }
    for old in [
        ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32,
        ProductionTerminalExpansionV1::Gfx942InlineU32(
            crate::trusted_device_items::TrustedAmdGpuInlineOperation::VMovB32,
        ),
        ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
        ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
        ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
        ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
        ProductionTerminalExpansionV1::Bf16Conversion(
            crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromBits,
        ),
        ProductionTerminalExpansionV1::ThreadIndex1d,
    ] {
        assert_eq!(
            terminal_identity_schema_v1(old),
            TerminalIdentitySchemaV1::CombinedV4
        );
        assert_eq!(
            terminal_expansion_tag_for_schema_v1(old, TerminalIdentitySchemaV1::CombinedV4),
            terminal_expansion_tag_for_schema_v1(old, TerminalIdentitySchemaV1::CombinedV5)
        );
    }
}
