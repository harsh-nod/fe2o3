#[test]
fn physical_lds_exchange_v39_importer_identity_is_exclusively_combined_v9() {
    use ProductionTerminalExpansionV1 as E;
    let rows = [
        (E::Gfx942PhysicalLdsExchangeBegin, 151),
        (E::Gfx942PhysicalLdsExchangeLabel, 152),
        (E::Gfx942PhysicalLdsExchangeStep, 153),
    ];
    for (expansion, tag) in rows {
        assert_eq!(
            compiler_intrinsic_identity_schema_v1(expansion),
            (
                b"fe2o3/semantic-mir/production-compiler-intrinsic/v9".as_slice(),
                TerminalIdentitySchemaV1::CombinedV9
            )
        );
        assert_eq!(
            terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::CombinedV9),
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
            TerminalIdentitySchemaV1::CombinedV8,
        ] {
            assert_eq!(
                terminal_operation_tag_for_schema_v1(expansion, schema),
                u8::MAX
            );
        }
    }
    for expansion in [
        E::Gfx942PhysicalGlobalCopyBegin,
        E::Gfx942PhysicalGlobalCopyLabel,
        E::Gfx942PhysicalGlobalCopyStep,
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
            terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::CombinedV9),
            u8::MAX
        );
        assert_ne!(
            compiler_intrinsic_identity_schema_v1(expansion).1,
            TerminalIdentitySchemaV1::CombinedV9
        );
    }
}
