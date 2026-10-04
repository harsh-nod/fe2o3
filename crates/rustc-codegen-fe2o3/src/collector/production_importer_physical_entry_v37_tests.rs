#[test]
fn physical_entry_v37_importer_identity_is_exclusively_combined_v7() {
    use ProductionTerminalExpansionV1 as E;
    let rows = [
        (E::Gfx942PhysicalEntryBegin, 145),
        (E::Gfx942PhysicalEntryLabel, 146),
        (E::Gfx942PhysicalEntryStep, 147),
    ];
    for (expansion, tag) in rows {
        assert_eq!(
            compiler_intrinsic_identity_schema_v1(expansion),
            (
                b"fe2o3/semantic-mir/production-compiler-intrinsic/v7".as_slice(),
                TerminalIdentitySchemaV1::CombinedV7
            )
        );
        assert_eq!(
            terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::CombinedV7),
            tag
        );
        for schema in [
            TerminalIdentitySchemaV1::IndependentV1,
            TerminalIdentitySchemaV1::CombinedV2,
            TerminalIdentitySchemaV1::CombinedV3,
            TerminalIdentitySchemaV1::CombinedV4,
            TerminalIdentitySchemaV1::CombinedV5,
            TerminalIdentitySchemaV1::CombinedV6,
        ] {
            assert_eq!(
                terminal_operation_tag_for_schema_v1(expansion, schema),
                u8::MAX
            );
        }
    }
    for expansion in [
        E::Gfx942CompleteBodyE32,
        E::Gfx942OrderedProgramE32,
        E::Gfx942OrderedXorAddE32,
        E::ThreadIndex1d,
        E::WorkgroupCollectiveContextCurrent,
        E::NeutralWorkgroupInclusiveScanSum,
        E::Gfx942InlineU32(crate::trusted_device_items::TrustedAmdGpuInlineOperation::VMovB32),
    ] {
        assert_eq!(
            terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::CombinedV7),
            u8::MAX
        );
        assert_ne!(
            compiler_intrinsic_identity_schema_v1(expansion).1,
            TerminalIdentitySchemaV1::CombinedV7
        );
    }
}
