use fe2o3_hsaco::KernelBindingError;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, PlironScalarAddV1ElfField, PlironScalarAddV1InspectionError,
    inspect_pliron_scalar_add_v1_hsaco,
};

#[path = "fixtures/worker_v2_hsaco_test_support.rs"]
mod fixture_support;

use fixture_support::{ScalarAddFixtureMutation as Mutation, scalar_add_fixture_with};

#[test]
fn exact_fixture_produces_inert_complete_observations() {
    let fixture = scalar_add_fixture_with(Mutation::None);
    let inspected = inspect_pliron_scalar_add_v1_hsaco(&fixture.bytes).expect("exact inspection");

    assert_eq!(
        inspected.output_identity(),
        ContentIdentityV1::calculate(&fixture.bytes)
    );
    assert_ne!(inspected.descriptor_identity().as_bytes(), &[0; 32]);
    assert_ne!(inspected.machine_identity().as_bytes(), &[0; 32]);
    assert!(!inspected.grants_publication_authority());
    assert!(!inspected.grants_load_authority());
    assert!(!inspected.grants_launch_authority());
}

#[test]
fn relocation_sections_fail_with_the_exact_relocation_category() {
    for mutation in [
        Mutation::RelSection,
        Mutation::RelaSection,
        Mutation::RelrSection,
        Mutation::CrelSection,
        Mutation::AndroidRelSection,
        Mutation::AndroidRelaSection,
        Mutation::AndroidRelrSection,
    ] {
        assert_rejected(mutation, PlironScalarAddV1ElfField::Relocations);
    }
}

#[test]
fn malformed_headers_and_tables_fail_without_unwinding() {
    for mutation in [
        Mutation::TruncatedHeader,
        Mutation::ElfClass32,
        Mutation::ElfBigEndian,
        Mutation::OverflowingSectionTable,
    ] {
        let fixture = scalar_add_fixture_with(mutation);
        let result =
            std::panic::catch_unwind(|| inspect_pliron_scalar_add_v1_hsaco(&fixture.bytes));
        assert!(result.is_ok(), "malformed {mutation:?} unwound");
        assert_eq!(
            result.unwrap(),
            Err(PlironScalarAddV1InspectionError::ElfProfile(
                PlironScalarAddV1ElfField::Object
            )),
            "unexpected category for {mutation:?}"
        );
    }

    let exact = scalar_add_fixture_with(Mutation::None);
    for length in 0..exact.bytes.len() {
        let truncated = &exact.bytes[..length];
        let result = std::panic::catch_unwind(|| inspect_pliron_scalar_add_v1_hsaco(truncated));
        assert!(result.is_ok(), "length {length} unwound");
        assert_eq!(
            result.unwrap(),
            Err(PlironScalarAddV1InspectionError::ElfProfile(
                PlironScalarAddV1ElfField::Object
            )),
            "length {length} did not fail at the object boundary"
        );
    }
}

#[test]
fn every_file_backed_section_body_is_bounded_without_unwinding() {
    for section in [1_usize, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14] {
        let mutation = Mutation::FileBackedSectionOutOfBounds(section);
        let fixture = scalar_add_fixture_with(mutation);
        let result =
            std::panic::catch_unwind(|| inspect_pliron_scalar_add_v1_hsaco(&fixture.bytes));
        assert!(result.is_ok(), "section {section} bounds check unwound");
        assert_eq!(
            result.unwrap(),
            Err(PlironScalarAddV1InspectionError::ElfProfile(
                PlironScalarAddV1ElfField::Object
            )),
            "section {section} did not fail at the object boundary"
        );
    }
}

#[test]
fn exact_section_and_loader_closure_rejects_substitutions() {
    for mutation in [
        Mutation::DuplicateSymtab,
        Mutation::DuplicateDynsym,
        Mutation::DuplicateDynamic,
        Mutation::LoaderMapping,
        Mutation::DynamicPointer,
        Mutation::DynamicStringPointer,
        Mutation::DynamicHashPointer,
        Mutation::DynamicGnuHashPointer,
        Mutation::DynamicStringSize,
        Mutation::DynamicSymbolEntrySize,
        Mutation::DynamicFlags,
        Mutation::HashGeometry,
        Mutation::GnuHashGeometry,
    ] {
        assert_rejected(mutation, PlironScalarAddV1ElfField::DynamicLoader);
    }
    for section in [2_usize, 3, 4, 8, 12] {
        assert_rejected(
            Mutation::SectionLink(section),
            PlironScalarAddV1ElfField::DynamicLoader,
        );
    }
    for section in [2_usize, 4, 8, 11, 12] {
        assert_rejected(
            Mutation::SectionEntrySize(section),
            PlironScalarAddV1ElfField::DynamicLoader,
        );
    }
}

#[test]
fn null_local_common_absolute_and_unnamed_symbol_hostiles_are_closed() {
    for mutation in [
        Mutation::StaticCommonSymbol,
        Mutation::StaticUnexpectedAbsolute,
        Mutation::MalformedStaticNull,
        Mutation::MalformedDynamicNull,
    ] {
        assert_rejected(mutation, PlironScalarAddV1ElfField::DefinedSymbols);
    }
    assert_rejected(
        Mutation::UndefinedStaticSymbol,
        PlironScalarAddV1ElfField::UndefinedSymbols,
    );
}

#[test]
fn executable_entry_and_padding_are_exact() {
    assert_rejected(
        Mutation::ExecutablePadding,
        PlironScalarAddV1ElfField::ExecutableRange,
    );
    assert_rejected(
        Mutation::EntrySize,
        PlironScalarAddV1ElfField::DefinedSymbols,
    );
}

#[test]
fn dynamic_dependency_and_table_hostiles_fail_with_exact_categories() {
    let cases = [
        (
            Mutation::DynamicNeeded,
            PlironScalarAddV1ElfField::UndefinedSymbols,
        ),
        (
            Mutation::DynamicMissingNull,
            PlironScalarAddV1ElfField::DynamicLoader,
        ),
        (
            Mutation::DynamicForbiddenTag,
            PlironScalarAddV1ElfField::Relocations,
        ),
        (
            Mutation::DynamicDuplicateTag,
            PlironScalarAddV1ElfField::DynamicLoader,
        ),
        (
            Mutation::DynamicMissingRequiredTags,
            PlironScalarAddV1ElfField::DynamicLoader,
        ),
    ];

    for (mutation, field) in cases {
        assert_rejected(mutation, field);
    }

    for tag in [
        2_i64,
        7,
        8,
        9,
        17,
        18,
        19,
        20,
        22,
        23,
        35,
        36,
        37,
        0x6000_000f,
        0x6000_0010,
        0x6000_0011,
        0x6000_0012,
        0x6fff_e000,
        0x6fff_e001,
        0x6fff_e003,
        0x6fff_fff9,
        0x6fff_fffa,
    ] {
        assert_rejected(
            Mutation::DynamicRelocationTag(tag),
            PlironScalarAddV1ElfField::Relocations,
        );
    }
}

#[test]
fn static_and_dynamic_symbol_hostiles_fail_with_exact_categories() {
    let cases = [
        (
            Mutation::ExtraDefinedSymbol,
            PlironScalarAddV1ElfField::DefinedSymbols,
        ),
        (
            Mutation::UndefinedStaticSymbol,
            PlironScalarAddV1ElfField::UndefinedSymbols,
        ),
        (
            Mutation::ExtraDynamicSymbol,
            PlironScalarAddV1ElfField::DefinedSymbols,
        ),
        (
            Mutation::UndefinedDynamicSymbol,
            PlironScalarAddV1ElfField::UndefinedSymbols,
        ),
    ];

    for (mutation, field) in cases {
        assert_rejected(mutation, field);
    }

    assert_rejected(
        Mutation::ExtraLocalSymbol,
        PlironScalarAddV1ElfField::DefinedSymbols,
    );
}

#[test]
fn descriptor_substitutions_change_only_observation_not_approval() {
    let exact = scalar_add_fixture_with(Mutation::None);
    let expected = inspect_pliron_scalar_add_v1_hsaco(&exact.bytes).expect("exact inspection");
    let changed = scalar_add_fixture_with(Mutation::DescriptorComputePgmRsrc3);
    let observed = inspect_pliron_scalar_add_v1_hsaco(&changed.bytes).expect("bounded observation");
    assert_ne!(observed.output_identity(), expected.output_identity());
    assert_ne!(
        observed.descriptor_identity(),
        expected.descriptor_identity()
    );
    assert_eq!(observed.machine_identity(), expected.machine_identity());
}

#[test]
fn invalid_descriptor_substitutions_fail_with_exact_binding_categories() {
    let cases = [
        (
            Mutation::DescriptorComputePgmRsrc1,
            "reserved or unsupported COMPUTE_PGM_RSRC1 bits are nonzero",
        ),
        (
            Mutation::DescriptorComputePgmRsrc2,
            "HSA-fixed or reserved COMPUTE_PGM_RSRC2 bits are nonzero",
        ),
        (
            Mutation::DescriptorKernelCodeProperties,
            "reserved kernel-code-property bits are nonzero",
        ),
        (
            Mutation::DescriptorReservedByte,
            "reserved descriptor bytes are nonzero",
        ),
    ];

    for (mutation, reason) in cases {
        let changed = scalar_add_fixture_with(mutation);
        assert_eq!(
            inspect_pliron_scalar_add_v1_hsaco(&changed.bytes),
            Err(PlironScalarAddV1InspectionError::HsacoBinding(
                KernelBindingError::InvalidKernelDescriptor(reason)
            )),
            "unexpected category for {mutation:?}"
        );
    }
}

#[test]
fn machine_substitution_changes_only_observation_not_approval() {
    let exact = scalar_add_fixture_with(Mutation::None);
    let changed = scalar_add_fixture_with(Mutation::MachineBytes);
    let expected = inspect_pliron_scalar_add_v1_hsaco(&exact.bytes).expect("exact inspection");
    let observed =
        inspect_pliron_scalar_add_v1_hsaco(&changed.bytes).expect("structural observation");

    assert_ne!(observed.output_identity(), expected.output_identity());
    assert_eq!(
        observed.descriptor_identity(),
        expected.descriptor_identity()
    );
    assert_ne!(observed.machine_identity(), expected.machine_identity());
}

fn assert_rejected(mutation: Mutation, expected: PlironScalarAddV1ElfField) {
    let fixture = scalar_add_fixture_with(mutation);
    assert_eq!(
        inspect_pliron_scalar_add_v1_hsaco(&fixture.bytes),
        Err(PlironScalarAddV1InspectionError::ElfProfile(expected)),
        "unexpected category for {mutation:?}"
    );
}
