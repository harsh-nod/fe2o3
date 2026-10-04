#[test]
fn complete_body_v36_preflight_domain_has_only_terminal144() {
    let body = ProductionTerminalExpansionV1::Gfx942CompleteBodyE32;
    assert_eq!(
        terminal_identity_schema_v1(body),
        TerminalIdentitySchemaV1::CombinedV6
    );
    assert_eq!(
        terminal_expansion_tag_for_schema_v1(body, TerminalIdentitySchemaV1::CombinedV6),
        144
    );
    for schema in [
        TerminalIdentitySchemaV1::IndependentV1,
        TerminalIdentitySchemaV1::CombinedV2,
        TerminalIdentitySchemaV1::CombinedV3,
        TerminalIdentitySchemaV1::CombinedV4,
        TerminalIdentitySchemaV1::CombinedV5,
    ] {
        assert_eq!(terminal_expansion_tag_for_schema_v1(body, schema), u8::MAX);
    }
    for old in [
        ProductionTerminalExpansionV1::Gfx942OrderedProgramE32,
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
            terminal_expansion_tag_for_schema_v1(old, TerminalIdentitySchemaV1::CombinedV6),
            u8::MAX
        );
        let expected = if old == ProductionTerminalExpansionV1::Gfx942OrderedProgramE32 {
            TerminalIdentitySchemaV1::CombinedV5
        } else {
            TerminalIdentitySchemaV1::CombinedV4
        };
        assert_eq!(terminal_identity_schema_v1(old), expected);
    }
}
