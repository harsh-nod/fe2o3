#[test]
fn ordered_program_identity_domain_is_new_only_for_the_new_terminal() {
    let program = ProductionTerminalExpansionV1::Gfx942OrderedProgramE32;
    assert_eq!(
        compiler_intrinsic_identity_schema_v1(program),
        (
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V5,
            TerminalIdentitySchemaV1::CombinedV5
        )
    );
    assert_ne!(
        PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4,
        PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V5
    );
    assert_eq!(
        terminal_operation_tag_for_schema_v1(program, TerminalIdentitySchemaV1::CombinedV5),
        134
    );
    for schema in [
        TerminalIdentitySchemaV1::IndependentV1,
        TerminalIdentitySchemaV1::CombinedV2,
        TerminalIdentitySchemaV1::CombinedV3,
        TerminalIdentitySchemaV1::CombinedV4,
    ] {
        assert_eq!(
            terminal_operation_tag_for_schema_v1(program, schema),
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
        let (domain, schema) = compiler_intrinsic_identity_schema_v1(old);
        assert_eq!(domain, PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4);
        assert_eq!(schema, TerminalIdentitySchemaV1::CombinedV4);
        assert_eq!(
            terminal_operation_tag_for_schema_v1(old, schema),
            terminal_operation_tag_for_schema_v1(old, TerminalIdentitySchemaV1::CombinedV5)
        );
        // Exact old field sequence remains unchanged even in a new-profile module.
        let old_digest = |domain| {
            let mut digest = SemanticIdentityDigestV1::new(domain);
            digest.field(&[1; 32]);
            digest.field(&[2; 32]);
            digest.field(&[terminal_operation_tag_for_schema_v1(
                old,
                TerminalIdentitySchemaV1::CombinedV4,
            )]);
            digest.field(&7_u32.to_le_bytes());
            digest.finish()
        };
        assert_eq!(
            old_digest(domain),
            old_digest(PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4)
        );
    }
}
