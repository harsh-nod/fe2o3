#[test]
fn complete_body_v36_importer_domain_and_tag_are_closed() {
    let body = ProductionTerminalExpansionV1::Gfx942CompleteBodyE32;
    assert_eq!(
        compiler_intrinsic_identity_schema_v1(body),
        (
            b"fe2o3/semantic-mir/production-compiler-intrinsic/v6".as_slice(),
            TerminalIdentitySchemaV1::CombinedV6
        )
    );
    assert_eq!(
        terminal_operation_tag_for_schema_v1(body, TerminalIdentitySchemaV1::CombinedV6),
        144
    );
    for schema in [
        TerminalIdentitySchemaV1::IndependentV1,
        TerminalIdentitySchemaV1::CombinedV2,
        TerminalIdentitySchemaV1::CombinedV3,
        TerminalIdentitySchemaV1::CombinedV4,
        TerminalIdentitySchemaV1::CombinedV5,
    ] {
        assert_eq!(terminal_operation_tag_for_schema_v1(body, schema), u8::MAX);
    }
}

#[test]
fn complete_body_v36_does_not_relabel_old_terminal_identities() {
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
        let (domain, schema) = compiler_intrinsic_identity_schema_v1(old);
        let expected = if old == ProductionTerminalExpansionV1::Gfx942OrderedProgramE32 {
            (
                PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V5,
                TerminalIdentitySchemaV1::CombinedV5,
            )
        } else {
            (
                PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4,
                TerminalIdentitySchemaV1::CombinedV4,
            )
        };
        assert_eq!((domain, schema), expected);
        assert_eq!(
            terminal_operation_tag_for_schema_v1(old, TerminalIdentitySchemaV1::CombinedV6),
            u8::MAX
        );
        // Existing identity field order is retained, not migrated to the new domain.
        let digest = |selected_domain: &[u8]| {
            let mut value = SemanticIdentityDigestV1::new(selected_domain);
            value.field(&[1; 32]);
            value.field(&[2; 32]);
            value.field(&[terminal_operation_tag_for_schema_v1(old, schema)]);
            value.field(&7_u32.to_le_bytes());
            value.finish()
        };
        assert_eq!(digest(domain), digest(expected.0));
        assert_ne!(
            digest(domain),
            digest(PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V6)
        );
    }
}
