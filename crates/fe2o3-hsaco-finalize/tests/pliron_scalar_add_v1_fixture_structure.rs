use std::collections::BTreeSet;

use object::{Object, ObjectSection, ObjectSymbol};

#[path = "fixtures/worker_v2_hsaco_test_support.rs"]
mod fixture_support;

use fixture_support::{ScalarAddFixtureMutation as Mutation, scalar_add_fixture_with};

const DYNSYM_SECTION_INDEX: usize = 2;
const DYNAMIC_SECTION_INDEX: usize = 8;
const COMMENT_SECTION_INDEX: usize = 11;
const SYMTAB_SECTION_INDEX: usize = 12;
const SECTION_HEADER_BYTES: usize = 64;

#[test]
fn exact_fixture_matches_the_measured_section_and_program_inventory() {
    let fixture = scalar_add_fixture_with(Mutation::None);
    let file = object::File::parse(fixture.bytes.as_slice()).expect("parse exact fixture");
    let names = file
        .sections()
        .map(|section| section.name().expect("valid section name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            ".note",
            ".dynsym",
            ".gnu.hash",
            ".hash",
            ".dynstr",
            ".rodata",
            ".text",
            ".dynamic",
            ".relro_padding",
            ".AMDGPU.gpr_maximums",
            ".comment",
            ".symtab",
            ".shstrtab",
            ".strtab",
        ]
    );
    assert_eq!(
        u16::from_le_bytes(fixture.bytes[56..58].try_into().unwrap()),
        8
    );
    assert_eq!(
        u16::from_le_bytes(fixture.bytes[60..62].try_into().unwrap()),
        15
    );
    assert_eq!(section_u64(&fixture.bytes, 7, 32), 0x440);
    assert_eq!(
        (0..15)
            .map(|index| {
                (
                    section_u64(&fixture.bytes, index, 16),
                    section_u64(&fixture.bytes, index, 24),
                    section_u64(&fixture.bytes, index, 32),
                )
            })
            .collect::<Vec<_>>(),
        [
            (0, 0, 0),
            (0x200, 0x200, 0x4e0),
            (0x6e0, 0x6e0, 0x48),
            (0x728, 0x728, 0x24),
            (0x74c, 0x74c, 0x20),
            (0x76c, 0x76c, 0x1a),
            (0x7c0, 0x7c0, 0x40),
            (0x1800, 0x800, 0x440),
            (0x2c40, 0xc40, 0x80),
            (0x2cc0, 0xcc0, 0x340),
            (0, 0xcc0, 0),
            (0, 0xcc0, 0x67),
            (0, 0xd28, 0x120),
            (0, 0xe48, 0x85),
            (0, 0xecd, 0xe9),
        ]
    );
    assert_eq!(section_table_offset(&fixture.bytes), 0xfb8);
    assert_eq!(
        program_geometry(&fixture.bytes, 0),
        (6, 4, 0x40, 0x40, 0x1c0, 0x1c0, 8)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 1),
        (1, 4, 0, 0, 0x800, 0x800, 0x1000)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 2),
        (1, 5, 0x800, 0x1800, 0x440, 0x440, 0x1000)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 3),
        (1, 6, 0xc40, 0x2c40, 0x80, 0x3c0, 0x1000)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 4),
        (2, 6, 0xc40, 0x2c40, 0x80, 0x80, 8)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 5),
        (0x6474_e552, 4, 0xc40, 0x2c40, 0x80, 0x3c0, 1)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 6),
        (0x6474_e551, 6, 0, 0, 0, 0, 0)
    );
    assert_eq!(
        program_geometry(&fixture.bytes, 7),
        (4, 4, 0x200, 0x200, 0x4e0, 0x4e0, 4)
    );
    let entry = file
        .symbols()
        .find(|symbol| symbol.name() == Ok("scalar_add"))
        .expect("exact static entry");
    assert_eq!(entry.size(), 56);
}

#[test]
fn exact_fixture_matches_the_measured_static_and_dynamic_symbol_closure() {
    let fixture = scalar_add_fixture_with(Mutation::None);
    let file = object::File::parse(fixture.bytes.as_slice()).expect("parse exact fixture");
    assert_eq!(section_u32(&fixture.bytes, DYNSYM_SECTION_INDEX, 4), 11);
    assert_eq!(section_u32(&fixture.bytes, SYMTAB_SECTION_INDEX, 4), 2);
    let static_names = file
        .symbols()
        .filter(|symbol| !symbol.name().unwrap_or("").is_empty())
        .map(|symbol| symbol.name().expect("valid symbol name"))
        .collect::<BTreeSet<_>>();
    let dynamic_names = file
        .dynamic_symbols()
        .filter(|symbol| !symbol.name().unwrap_or("").is_empty())
        .map(|symbol| symbol.name().expect("valid symbol name"))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        static_names,
        BTreeSet::from([
            "_DYNAMIC",
            "scalar_add",
            "scalar_add.has_dyn_sized_stack",
            "scalar_add.has_recursion",
            "scalar_add.kd",
            "scalar_add.num_agpr",
            "scalar_add.num_vgpr",
            "scalar_add.numbered_sgpr",
            "scalar_add.private_seg_size",
            "scalar_add.uses_flat_scratch",
            "scalar_add.uses_vcc",
        ])
    );
    assert_eq!(
        dynamic_names,
        BTreeSet::from(["scalar_add", "scalar_add.kd"])
    );
    assert!(
        file.sections()
            .all(|section| section.relocations().next().is_none())
    );
}

#[test]
fn relocation_fixtures_are_real_relocation_bearing_sections() {
    for (mutation, expected_type, expected_entry_size) in [
        (Mutation::RelSection, 9, 16),
        (Mutation::RelaSection, 4, 24),
    ] {
        let fixture = scalar_add_fixture_with(mutation);
        assert_eq!(
            section_u32(&fixture.bytes, COMMENT_SECTION_INDEX, 4),
            expected_type
        );
        assert_eq!(
            section_u64(&fixture.bytes, COMMENT_SECTION_INDEX, 56),
            expected_entry_size
        );
        assert_eq!(
            section_u64(&fixture.bytes, COMMENT_SECTION_INDEX, 32),
            expected_entry_size
        );

        let file = object::File::parse(fixture.bytes.as_slice()).expect("parse relocation fixture");
        let relocation_section = file
            .sections()
            .find(|section| section.name() == Ok(".comment"))
            .expect("typed auxiliary relocation section");
        assert_eq!(
            relocation_section.data().expect("relocation bytes").len(),
            expected_entry_size as usize
        );
        let text = file
            .section_by_name(".text")
            .expect("relocated text section");
        assert_eq!(text.relocations().count(), 1);
    }
}

#[test]
fn dynamic_fixtures_encode_the_requested_real_tags() {
    let cases: &[(Mutation, &[(i64, u64)])] = &[
        (Mutation::DynamicNeeded, &[(1, 0), (0, 0)]),
        (Mutation::DynamicForbiddenTag, &[(7, 0x1000), (0, 0)]),
        (
            Mutation::DynamicDuplicateTag,
            &[(5, 0x1000), (5, 0x1000), (0, 0)],
        ),
        (Mutation::DynamicMissingNull, &[(5, 0x1000)]),
        (Mutation::DynamicMissingRequiredTags, &[(0, 0)]),
    ];

    for (mutation, expected) in cases {
        let fixture = scalar_add_fixture_with(*mutation);
        assert_eq!(section_u32(&fixture.bytes, DYNAMIC_SECTION_INDEX, 4), 6);
        assert_eq!(section_u64(&fixture.bytes, DYNAMIC_SECTION_INDEX, 56), 16);
        let entries = dynamic_entries(&fixture.bytes);
        assert_eq!(&entries[..expected.len()], *expected);
        assert_eq!(entries.len(), 8);
    }
}

#[test]
fn static_symbol_fixtures_cover_local_and_undefined_entries() {
    let local = scalar_add_fixture_with(Mutation::ExtraLocalSymbol);
    let local_file = object::File::parse(local.bytes.as_slice()).expect("parse local fixture");
    assert!(local_file.symbols().any(|symbol| {
        !symbol.is_undefined()
            && !symbol.is_global()
            && !symbol.is_weak()
            && symbol
                .name()
                .is_ok_and(|name| !name.is_empty() && name.bytes().all(|byte| byte == b'x'))
    }));

    let undefined = scalar_add_fixture_with(Mutation::UndefinedStaticSymbol);
    let symbol = section_u64(&undefined.bytes, SYMTAB_SECTION_INDEX, 24) as usize + 24;
    assert_eq!(
        u32::from_le_bytes(undefined.bytes[symbol..symbol + 4].try_into().unwrap()),
        0
    );
    assert_eq!(undefined.bytes[symbol + 4] >> 4, 1);
    assert_eq!(
        u16::from_le_bytes(undefined.bytes[symbol + 6..symbol + 8].try_into().unwrap()),
        0
    );
}

#[test]
fn dynamic_symbol_fixtures_cover_defined_and_undefined_entries() {
    let defined = scalar_add_fixture_with(Mutation::ExtraDynamicSymbol);
    let defined_file = object::File::parse(defined.bytes.as_slice()).expect("parse dynsym fixture");
    assert!(defined_file.dynamic_symbols().any(|symbol| {
        symbol.is_definition() && symbol.is_global() && symbol.name() == Ok("scalar_add")
    }));

    let undefined = scalar_add_fixture_with(Mutation::UndefinedDynamicSymbol);
    let undefined_file =
        object::File::parse(undefined.bytes.as_slice()).expect("parse undefined dynsym fixture");
    assert!(undefined_file.dynamic_symbols().any(|symbol| {
        symbol.is_undefined() && symbol.is_global() && symbol.name() == Ok("scalar_add.kd")
    }));
}

#[test]
fn descriptor_fixtures_change_only_the_selected_descriptor_field() {
    let exact = scalar_add_fixture_with(Mutation::None);
    let cases = [
        (Mutation::DescriptorComputePgmRsrc3, 44_usize, 4_usize),
        (Mutation::DescriptorComputePgmRsrc1, 48, 4),
        (Mutation::DescriptorComputePgmRsrc2, 52, 4),
        (Mutation::DescriptorKernelCodeProperties, 56, 2),
        (Mutation::DescriptorReservedByte, 63, 1),
    ];

    for (mutation, relative_offset, width) in cases {
        let changed = scalar_add_fixture_with(mutation);
        let exact_descriptor = &exact.bytes[exact.descriptor_offset..exact.descriptor_offset + 64];
        let changed_descriptor =
            &changed.bytes[changed.descriptor_offset..changed.descriptor_offset + 64];
        assert_ne!(changed_descriptor, exact_descriptor);
        assert_eq!(
            &changed_descriptor[..relative_offset],
            &exact_descriptor[..relative_offset]
        );
        assert_eq!(
            &changed_descriptor[relative_offset + width..],
            &exact_descriptor[relative_offset + width..]
        );
    }
}

#[test]
fn machine_fixture_changes_the_bound_entry_bytes_only() {
    let exact = scalar_add_fixture_with(Mutation::None);
    let changed = scalar_add_fixture_with(Mutation::MachineBytes);

    assert_ne!(changed.bytes, exact.bytes);
    assert_eq!(changed.bytes.len(), exact.bytes.len());
    assert_eq!(
        changed.bytes[changed.text_offset],
        exact.bytes[exact.text_offset] ^ 1
    );
    assert_eq!(
        &changed.bytes[..changed.text_offset],
        &exact.bytes[..exact.text_offset]
    );
    assert_eq!(
        &changed.bytes[changed.text_offset + 1..],
        &exact.bytes[exact.text_offset + 1..]
    );
}

fn section_header_offset(bytes: &[u8], index: usize) -> usize {
    read_u64(bytes, 40) as usize + index * SECTION_HEADER_BYTES
}

fn section_u32(bytes: &[u8], index: usize, field: usize) -> u32 {
    let offset = section_header_offset(bytes, index) + field;
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn section_u64(bytes: &[u8], index: usize, field: usize) -> u64 {
    read_u64(bytes, section_header_offset(bytes, index) + field)
}

fn section_table_offset(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes[40..48].try_into().unwrap())
}

fn program_geometry(bytes: &[u8], index: usize) -> (u32, u32, u64, u64, u64, u64, u64) {
    let offset = 64 + index * 56;
    (
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
        u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
        u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap()),
        u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().unwrap()),
        u64::from_le_bytes(bytes[offset + 32..offset + 40].try_into().unwrap()),
        u64::from_le_bytes(bytes[offset + 40..offset + 48].try_into().unwrap()),
        u64::from_le_bytes(bytes[offset + 48..offset + 56].try_into().unwrap()),
    )
}

fn dynamic_entries(bytes: &[u8]) -> Vec<(i64, u64)> {
    let offset = section_u64(bytes, DYNAMIC_SECTION_INDEX, 24) as usize;
    let size = section_u64(bytes, DYNAMIC_SECTION_INDEX, 32) as usize;
    bytes[offset..offset + size]
        .chunks_exact(16)
        .map(|entry| {
            (
                i64::from_le_bytes(entry[..8].try_into().unwrap()),
                u64::from_le_bytes(entry[8..].try_into().unwrap()),
            )
        })
        .collect()
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
