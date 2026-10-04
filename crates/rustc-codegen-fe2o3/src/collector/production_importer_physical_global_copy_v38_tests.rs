#[test]
fn physical_global_copy_v38_importer_identity_is_exclusively_combined_v8() {
    use ProductionTerminalExpansionV1 as E;
    let rows = [
        (E::Gfx942PhysicalGlobalCopyBegin, 148),
        (E::Gfx942PhysicalGlobalCopyLabel, 149),
        (E::Gfx942PhysicalGlobalCopyStep, 150),
    ];
    for (expansion, tag) in rows {
        assert_eq!(
            compiler_intrinsic_identity_schema_v1(expansion),
            (
                b"fe2o3/semantic-mir/production-compiler-intrinsic/v8".as_slice(),
                TerminalIdentitySchemaV1::CombinedV8
            )
        );
        assert_eq!(
            terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::CombinedV8),
            tag
        );
        for schema in [
            TerminalIdentitySchemaV1::IndependentV1,
            TerminalIdentitySchemaV1::CombinedV2,
            TerminalIdentitySchemaV1::CombinedV3,
            TerminalIdentitySchemaV1::CombinedV4,
            TerminalIdentitySchemaV1::CombinedV5,
            TerminalIdentitySchemaV1::CombinedV6,
            TerminalIdentitySchemaV1::CombinedV7,
        ] {
            assert_eq!(
                terminal_operation_tag_for_schema_v1(expansion, schema),
                u8::MAX
            );
        }
    }
    for expansion in [
        E::Gfx942PhysicalEntryBegin,
        E::Gfx942PhysicalEntryLabel,
        E::Gfx942PhysicalEntryStep,
        E::Gfx942CompleteBodyE32,
        E::Gfx942OrderedProgramE32,
        E::Gfx942OrderedXorAddE32,
        E::ThreadIndex1d,
        E::WorkgroupCollectiveContextCurrent,
        E::NeutralWorkgroupInclusiveScanSum,
        E::Gfx942InlineU32(crate::trusted_device_items::TrustedAmdGpuInlineOperation::VMovB32),
    ] {
        assert_eq!(
            terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::CombinedV8),
            u8::MAX
        );
        assert_ne!(
            compiler_intrinsic_identity_schema_v1(expansion).1,
            TerminalIdentitySchemaV1::CombinedV8
        );
    }
}
