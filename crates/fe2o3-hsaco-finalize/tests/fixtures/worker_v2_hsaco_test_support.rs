// Shared synthetic gfx942 HSACO builder reused by typed publication tests.

use fe2o3_hsaco_finalize::DEVICE_DESCRIPTOR_SECTION_NAME;

use self::msgpack::{Value, write_value};

mod msgpack {
    #[derive(Clone, Debug)]
    pub(super) enum Value {
        String(String),
        Unsigned(u64),
        Boolean(bool),
        Array(Vec<Self>),
        Map(Vec<(Self, Self)>),
    }

    impl From<&str> for Value {
        fn from(value: &str) -> Self {
            Self::String(value.to_owned())
        }
    }

    impl From<String> for Value {
        fn from(value: String) -> Self {
            Self::String(value)
        }
    }

    impl From<bool> for Value {
        fn from(value: bool) -> Self {
            Self::Boolean(value)
        }
    }

    macro_rules! unsigned_value {
        ($($ty:ty),+ $(,)?) => {
            $(
                impl From<$ty> for Value {
                    fn from(value: $ty) -> Self {
                        Self::Unsigned(u64::from(value))
                    }
                }
            )+
        };
    }

    unsigned_value!(u8, u16, u32, u64);

    impl From<i32> for Value {
        fn from(value: i32) -> Self {
            Self::Unsigned(u64::try_from(value).expect("fixture integer is nonnegative"))
        }
    }

    pub(super) fn write_value(output: &mut Vec<u8>, value: &Value) -> Result<(), ()> {
        match value {
            Value::String(value) => write_string(output, value),
            Value::Unsigned(value) => write_unsigned(output, *value),
            Value::Boolean(value) => output.push(if *value { 0xc3 } else { 0xc2 }),
            Value::Array(values) => {
                write_array_len(output, values.len())?;
                for value in values {
                    write_value(output, value)?;
                }
            }
            Value::Map(fields) => {
                write_map_len(output, fields.len())?;
                for (key, value) in fields {
                    write_value(output, key)?;
                    write_value(output, value)?;
                }
            }
        }
        Ok(())
    }

    fn write_string(output: &mut Vec<u8>, value: &str) {
        let len = value.len();
        if len < 32 {
            output.push(0xa0 | u8::try_from(len).expect("fixstr length"));
        } else if let Ok(len) = u8::try_from(len) {
            output.extend_from_slice(&[0xd9, len]);
        } else if let Ok(len) = u16::try_from(len) {
            output.push(0xda);
            output.extend_from_slice(&len.to_be_bytes());
        } else {
            let len = u32::try_from(len).expect("fixture string length fits u32");
            output.push(0xdb);
            output.extend_from_slice(&len.to_be_bytes());
        }
        output.extend_from_slice(value.as_bytes());
    }

    fn write_unsigned(output: &mut Vec<u8>, value: u64) {
        if let Ok(value) = u8::try_from(value) {
            if value < 128 {
                output.push(value);
            } else {
                output.extend_from_slice(&[0xcc, value]);
            }
        } else if let Ok(value) = u16::try_from(value) {
            output.push(0xcd);
            output.extend_from_slice(&value.to_be_bytes());
        } else if let Ok(value) = u32::try_from(value) {
            output.push(0xce);
            output.extend_from_slice(&value.to_be_bytes());
        } else {
            output.push(0xcf);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }

    fn write_array_len(output: &mut Vec<u8>, len: usize) -> Result<(), ()> {
        if len < 16 {
            output.push(0x90 | u8::try_from(len).map_err(|_| ())?);
        } else if let Ok(len) = u16::try_from(len) {
            output.push(0xdc);
            output.extend_from_slice(&len.to_be_bytes());
        } else {
            output.push(0xdd);
            output.extend_from_slice(&u32::try_from(len).map_err(|_| ())?.to_be_bytes());
        }
        Ok(())
    }

    fn write_map_len(output: &mut Vec<u8>, len: usize) -> Result<(), ()> {
        if len < 16 {
            output.push(0x80 | u8::try_from(len).map_err(|_| ())?);
        } else if let Ok(len) = u16::try_from(len) {
            output.push(0xde);
            output.extend_from_slice(&len.to_be_bytes());
        } else {
            output.push(0xdf);
            output.extend_from_slice(&u32::try_from(len).map_err(|_| ())?.to_be_bytes());
        }
        Ok(())
    }
}

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const SECTION_HEADER_BYTES: usize = 64;
const NOTE_SECTION_INDEX: usize = 1;
const RODATA_SECTION_INDEX: usize = 2;
const TEXT_SECTION_INDEX: usize = 3;
const STRTAB_SECTION_INDEX: usize = 4;
const SYMTAB_SECTION_INDEX: usize = 5;
const CANONICAL_DESCRIPTOR_SECTION_INDEX: usize = 6;
const SHSTRTAB_SECTION_INDEX: usize = 7;
const DYNSTR_SECTION_INDEX: usize = 8;
const DYNSYM_SECTION_INDEX: usize = 9;
const HASH_SECTION_INDEX: usize = 10;
const GNU_HASH_SECTION_INDEX: usize = 11;
const DYNAMIC_SECTION_INDEX: usize = 12;
const SCALAR_SECTION_COUNT: usize = 13;
const SCALAR_PROGRAM_COUNT: usize = 5;
const SCALAR_READ_LOAD_PROGRAM_INDEX: usize = 0;
const SCALAR_EXEC_LOAD_PROGRAM_INDEX: usize = 1;
const SCALAR_WRITE_LOAD_PROGRAM_INDEX: usize = 2;
const SCALAR_NOTE_PROGRAM_INDEX: usize = 3;
const SCALAR_DYNAMIC_PROGRAM_INDEX: usize = 4;

const SCALAR_V1_DYNSYM_SECTION_INDEX: usize = 2;
const SCALAR_V1_GNU_HASH_SECTION_INDEX: usize = 3;
const SCALAR_V1_HASH_SECTION_INDEX: usize = 4;
const SCALAR_V1_RODATA_SECTION_INDEX: usize = 6;
const SCALAR_V1_TEXT_SECTION_INDEX: usize = 7;
const SCALAR_V1_DYNAMIC_SECTION_INDEX: usize = 8;
const SCALAR_V1_GPR_SECTION_INDEX: usize = 10;
const SCALAR_V1_COMMENT_SECTION_INDEX: usize = 11;
const SCALAR_V1_SYMTAB_SECTION_INDEX: usize = 12;
const SCALAR_V1_SHSTRTAB_SECTION_INDEX: usize = 13;
const SCALAR_V1_STRTAB_SECTION_INDEX: usize = 14;
const SCALAR_V1_SECTION_COUNT: usize = 15;
const SCALAR_V1_PROGRAM_COUNT: usize = 8;

#[derive(Clone, Copy, Eq, PartialEq)]
#[allow(dead_code)]
enum FixtureAbi {
    SliceF32,
    ScalarAddV1,
    TiledGemmV1,
    RowSoftmaxV1,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum FixtureMetadataValue<'a> {
    String(&'a str),
    Unsigned(u64),
    Boolean(bool),
}

#[derive(Clone, Copy)]
struct FixtureOptions<'a> {
    target: &'a str,
    code_object_version: u8,
    entry: &'a str,
    descriptor: &'a str,
    required_workgroup_size: [u32; 3],
    max_flat_workgroup_size: u32,
    wavefront_size: u32,
    descriptor_wavefront_size: u32,
    include_export: bool,
    include_canonical_descriptor_section_name: bool,
    include_explicit_argument_alignments: bool,
    include_pointee_alignment: bool,
    pointee_alignment: u64,
    optional_hidden_argument: Option<(u64, u64, &'a str)>,
    second_optional_hidden_argument: Option<(u64, u64, &'a str)>,
    include_exact_row_llvm22_hidden_arguments: bool,
    omitted_hidden_argument: Option<usize>,
    hidden_argument_override: Option<(usize, u64, u64, &'a str)>,
    argument_extra: Option<(usize, &'a str, FixtureMetadataValue<'a>)>,
    include_required_workgroup_size: bool,
    max_workgroups: [Option<u32>; 3],
    cluster_dims: Option<[u32; 3]>,
    kernel_kind: Option<&'a str>,
    uses_dynamic_stack: Option<bool>,
    uniform_work_group_size: Option<u64>,
    workgroup_processor_mode: Option<bool>,
    gfx1250_revision: Option<&'a str>,
    device_enqueue_symbol: Option<&'a str>,
    source_language: Option<&'a str>,
    source_language_version: Option<[u32; 2]>,
    include_workgroup_size_hint: bool,
    include_vector_type_hint: bool,
    include_printf_metadata: bool,
    sgpr_count: u16,
    vgpr_count: u16,
    agpr_count: Option<u32>,
    sgpr_spill_count: Option<u32>,
    vgpr_spill_count: Option<u32>,
    include_dynamic_lds_size: bool,
    duplicate_max_workgroups_x: bool,
    malformed_max_workgroups_x: bool,
    abi: FixtureAbi,
    tiled_first_argument_offset: u64,
    row_softmax_first_argument_offset: u64,
    kernarg_segment_size_override: Option<u64>,
    group_segment_fixed_size: u64,
}

impl FixtureOptions<'static> {
    fn valid() -> Self {
        Self {
            target: "gfx942",
            code_object_version: 4,
            entry: "vecadd",
            descriptor: "vecadd.kd",
            required_workgroup_size: [256, 1, 1],
            max_flat_workgroup_size: 256,
            wavefront_size: 64,
            descriptor_wavefront_size: 64,
            include_export: true,
            include_canonical_descriptor_section_name: false,
            include_explicit_argument_alignments: false,
            include_pointee_alignment: false,
            pointee_alignment: 4,
            optional_hidden_argument: None,
            second_optional_hidden_argument: None,
            include_exact_row_llvm22_hidden_arguments: false,
            omitted_hidden_argument: None,
            hidden_argument_override: None,
            argument_extra: None,
            include_required_workgroup_size: true,
            max_workgroups: [None; 3],
            cluster_dims: None,
            kernel_kind: None,
            uses_dynamic_stack: None,
            uniform_work_group_size: None,
            workgroup_processor_mode: None,
            gfx1250_revision: None,
            device_enqueue_symbol: None,
            source_language: None,
            source_language_version: None,
            include_workgroup_size_hint: false,
            include_vector_type_hint: false,
            include_printf_metadata: false,
            sgpr_count: 14,
            vgpr_count: 11,
            agpr_count: Some(3),
            sgpr_spill_count: Some(2),
            vgpr_spill_count: Some(4),
            include_dynamic_lds_size: false,
            duplicate_max_workgroups_x: false,
            malformed_max_workgroups_x: false,
            abi: FixtureAbi::SliceF32,
            tiled_first_argument_offset: 0,
            row_softmax_first_argument_offset: 0,
            kernarg_segment_size_override: None,
            group_segment_fixed_size: 0,
        }
    }
}

pub(crate) struct Fixture {
    pub(crate) bytes: Vec<u8>,
    pub(crate) text_offset: usize,
    pub(crate) descriptor_offset: usize,
}

#[allow(dead_code)]
pub(crate) fn scalar_add_fixture() -> Fixture {
    scalar_add_fixture_with(ScalarAddFixtureMutation::None)
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) enum ScalarAddFixtureMutation {
    None,
    Target,
    CodeObjectVersion,
    EntrySymbol,
    DescriptorSymbol,
    RequiredWorkgroup,
    MaxFlatWorkgroup,
    Wave32,
    KernargSize,
    KernargAlignment,
    ExplicitArgumentOffset,
    HiddenArgument,
    GroupSegment,
    PrivateSegment,
    SpillCount,
    DynamicStack,
    CanonicalDescriptorSection,
    ExtraDefinedSymbol,
    ExtraLocalSymbol,
    UndefinedStaticSymbol,
    ExtraDynamicSymbol,
    UndefinedDynamicSymbol,
    RelSection,
    RelaSection,
    RelrSection,
    CrelSection,
    AndroidRelSection,
    AndroidRelaSection,
    AndroidRelrSection,
    DynamicNeeded,
    DynamicForbiddenTag,
    DynamicRelocationTag(i64),
    DynamicDuplicateTag,
    DynamicMissingNull,
    DynamicMissingRequiredTags,
    DynamicPointer,
    DynamicStringPointer,
    DynamicHashPointer,
    DynamicGnuHashPointer,
    DynamicStringSize,
    DynamicSymbolEntrySize,
    DynamicFlags,
    HashGeometry,
    GnuHashGeometry,
    SectionLink(usize),
    SectionEntrySize(usize),
    LoaderMapping,
    DuplicateSymtab,
    DuplicateDynsym,
    DuplicateDynamic,
    StaticCommonSymbol,
    StaticUnexpectedAbsolute,
    MalformedStaticNull,
    MalformedDynamicNull,
    ElfClass32,
    ElfBigEndian,
    OverflowingSectionTable,
    TruncatedHeader,
    FileBackedSectionOutOfBounds(usize),
    ExecutablePadding,
    EntrySize,
    DescriptorComputePgmRsrc3,
    DescriptorComputePgmRsrc1,
    DescriptorComputePgmRsrc2,
    DescriptorKernelCodeProperties,
    DescriptorReservedByte,
    MachineBytes,
}

#[allow(dead_code)]
pub(crate) fn scalar_add_fixture_with(mutation: ScalarAddFixtureMutation) -> Fixture {
    let mut options = FixtureOptions::valid();
    options.target = "gfx942:xnack-";
    options.code_object_version = 4;
    options.entry = "scalar_add";
    options.descriptor = "scalar_add.kd";
    options.include_export = false;
    options.include_required_workgroup_size = false;
    options.max_flat_workgroup_size = 64;
    options.wavefront_size = 64;
    options.descriptor_wavefront_size = 64;
    options.uses_dynamic_stack = Some(false);
    options.sgpr_spill_count = Some(0);
    options.vgpr_spill_count = Some(0);
    options.abi = FixtureAbi::ScalarAddV1;
    match mutation {
        ScalarAddFixtureMutation::Target => options.target = "gfx950",
        ScalarAddFixtureMutation::CodeObjectVersion => options.code_object_version = 3,
        ScalarAddFixtureMutation::EntrySymbol => options.entry = "scalar_mul",
        ScalarAddFixtureMutation::DescriptorSymbol => options.descriptor = "scalar_add.xx",
        ScalarAddFixtureMutation::RequiredWorkgroup => {
            options.include_required_workgroup_size = true;
            options.required_workgroup_size = [64, 1, 1];
        }
        ScalarAddFixtureMutation::MaxFlatWorkgroup => options.max_flat_workgroup_size = 32,
        ScalarAddFixtureMutation::Wave32 => {
            options.wavefront_size = 32;
            options.descriptor_wavefront_size = 32;
        }
        ScalarAddFixtureMutation::KernargSize => {
            options.kernarg_segment_size_override = Some(281);
        }
        ScalarAddFixtureMutation::ExplicitArgumentOffset => {
            options.argument_extra = Some((0, ".offset", FixtureMetadataValue::Unsigned(8)));
        }
        ScalarAddFixtureMutation::HiddenArgument => options.omitted_hidden_argument = Some(12),
        ScalarAddFixtureMutation::GroupSegment => options.group_segment_fixed_size = 1,
        ScalarAddFixtureMutation::SpillCount => options.sgpr_spill_count = Some(1),
        ScalarAddFixtureMutation::DynamicStack => options.uses_dynamic_stack = Some(true),
        ScalarAddFixtureMutation::CanonicalDescriptorSection => {
            options.include_canonical_descriptor_section_name = true;
        }
        _ => {}
    }
    let mut result = fixture(options);
    match mutation {
        ScalarAddFixtureMutation::KernargAlignment => {
            replace_metadata_fixint(&mut result.bytes, b".kernarg_segment_align", 8, 16);
        }
        ScalarAddFixtureMutation::PrivateSegment => {
            replace_metadata_fixint(&mut result.bytes, b".private_segment_fixed_size", 0, 1);
        }
        ScalarAddFixtureMutation::ExtraDefinedSymbol => {
            add_static_symbol(&mut result, false, true);
        }
        ScalarAddFixtureMutation::ExtraLocalSymbol => add_static_symbol(&mut result, true, true),
        ScalarAddFixtureMutation::UndefinedStaticSymbol => {
            add_static_symbol(&mut result, false, false);
        }
        ScalarAddFixtureMutation::ExtraDynamicSymbol => {
            add_dynamic_symbol(&mut result, true);
        }
        ScalarAddFixtureMutation::UndefinedDynamicSymbol => {
            add_dynamic_symbol(&mut result, false);
        }
        ScalarAddFixtureMutation::RelSection => add_relocation_section(&mut result, false),
        ScalarAddFixtureMutation::RelaSection => add_relocation_section(&mut result, true),
        ScalarAddFixtureMutation::RelrSection => set_relocation_section(&mut result, 19),
        ScalarAddFixtureMutation::CrelSection => {
            set_relocation_section(&mut result, 0x4000_0014);
        }
        ScalarAddFixtureMutation::AndroidRelSection => {
            set_relocation_section(&mut result, 0x6000_0001);
        }
        ScalarAddFixtureMutation::AndroidRelaSection => {
            set_relocation_section(&mut result, 0x6000_0002);
        }
        ScalarAddFixtureMutation::AndroidRelrSection => {
            set_relocation_section(&mut result, 0x6fff_ff00);
        }
        ScalarAddFixtureMutation::DynamicNeeded => {
            add_dynamic_section(&mut result, &[(1, 0), (0, 0)]);
        }
        ScalarAddFixtureMutation::DynamicForbiddenTag => {
            add_dynamic_section(&mut result, &[(7, 0x1000), (0, 0)]);
        }
        ScalarAddFixtureMutation::DynamicRelocationTag(tag) => {
            add_dynamic_section(&mut result, &[(tag, 0x1000), (0, 0)]);
        }
        ScalarAddFixtureMutation::DynamicDuplicateTag => {
            add_dynamic_section(&mut result, &[(5, 0x1000), (5, 0x1000), (0, 0)]);
        }
        ScalarAddFixtureMutation::DynamicMissingNull => {
            add_dynamic_section(&mut result, &[(5, 0x1000)]);
        }
        ScalarAddFixtureMutation::DynamicMissingRequiredTags => {
            add_dynamic_section(&mut result, &[(0, 0)]);
        }
        ScalarAddFixtureMutation::DynamicPointer => {
            mutate_dynamic_value(&mut result, 6, 1);
        }
        ScalarAddFixtureMutation::DynamicStringPointer => {
            mutate_dynamic_value(&mut result, 5, 1);
        }
        ScalarAddFixtureMutation::DynamicHashPointer => {
            mutate_dynamic_value(&mut result, 4, 1);
        }
        ScalarAddFixtureMutation::DynamicGnuHashPointer => {
            mutate_dynamic_value(&mut result, 0x6fff_fef5, 1);
        }
        ScalarAddFixtureMutation::DynamicStringSize => {
            mutate_dynamic_value(&mut result, 10, 1);
        }
        ScalarAddFixtureMutation::DynamicSymbolEntrySize => {
            mutate_dynamic_value(&mut result, 11, 1);
        }
        ScalarAddFixtureMutation::DynamicFlags => {
            mutate_dynamic_value(&mut result, 30, 1);
        }
        ScalarAddFixtureMutation::HashGeometry => {
            mutate_section_word(&mut result, SCALAR_V1_HASH_SECTION_INDEX, 0);
        }
        ScalarAddFixtureMutation::GnuHashGeometry => {
            mutate_section_word(&mut result, SCALAR_V1_GNU_HASH_SECTION_INDEX, 8);
        }
        ScalarAddFixtureMutation::SectionLink(section) => {
            mutate_section_field_u32(&mut result, section, 40, 1);
        }
        ScalarAddFixtureMutation::SectionEntrySize(section) => {
            mutate_section_field_u64(&mut result, section, 56, 1);
        }
        ScalarAddFixtureMutation::LoaderMapping => mutate_program_offset(&mut result),
        ScalarAddFixtureMutation::DuplicateSymtab => {
            duplicate_section_header_type(
                &mut result,
                SCALAR_V1_GPR_SECTION_INDEX,
                SCALAR_V1_SYMTAB_SECTION_INDEX,
            );
        }
        ScalarAddFixtureMutation::DuplicateDynsym => {
            duplicate_section_header_type(
                &mut result,
                SCALAR_V1_GPR_SECTION_INDEX,
                SCALAR_V1_DYNSYM_SECTION_INDEX,
            );
        }
        ScalarAddFixtureMutation::DuplicateDynamic => {
            duplicate_section_header_type(
                &mut result,
                SCALAR_V1_GPR_SECTION_INDEX,
                SCALAR_V1_DYNAMIC_SECTION_INDEX,
            );
        }
        ScalarAddFixtureMutation::StaticCommonSymbol => {
            mutate_static_symbol_section(&mut result, 1, 0xfff2);
        }
        ScalarAddFixtureMutation::StaticUnexpectedAbsolute => {
            add_static_symbol(&mut result, true, true);
        }
        ScalarAddFixtureMutation::MalformedStaticNull => {
            mutate_symbol_null(&mut result, SCALAR_V1_SYMTAB_SECTION_INDEX);
        }
        ScalarAddFixtureMutation::MalformedDynamicNull => {
            mutate_symbol_null(&mut result, SCALAR_V1_DYNSYM_SECTION_INDEX);
        }
        ScalarAddFixtureMutation::ElfClass32 => result.bytes[4] = 1,
        ScalarAddFixtureMutation::ElfBigEndian => result.bytes[5] = 2,
        ScalarAddFixtureMutation::OverflowingSectionTable => {
            write_u64(&mut result.bytes, 40, u64::MAX - 31);
        }
        ScalarAddFixtureMutation::TruncatedHeader => result.bytes.truncate(40),
        ScalarAddFixtureMutation::FileBackedSectionOutOfBounds(index) => {
            mutate_section_out_of_bounds(&mut result, index);
        }
        ScalarAddFixtureMutation::ExecutablePadding => {
            result.bytes[result.text_offset + 56] ^= 1;
        }
        ScalarAddFixtureMutation::EntrySize => mutate_entry_sizes(&mut result),
        ScalarAddFixtureMutation::DescriptorComputePgmRsrc3 => {
            xor_u32(&mut result.bytes, result.descriptor_offset + 44, 1 << 16);
        }
        ScalarAddFixtureMutation::DescriptorComputePgmRsrc1 => {
            xor_u32(&mut result.bytes, result.descriptor_offset + 48, 1 << 28);
        }
        ScalarAddFixtureMutation::DescriptorComputePgmRsrc2 => {
            xor_u32(&mut result.bytes, result.descriptor_offset + 52, 1 << 20);
        }
        ScalarAddFixtureMutation::DescriptorKernelCodeProperties => {
            let offset = result.descriptor_offset + 56;
            let value = u16::from_le_bytes(result.bytes[offset..offset + 2].try_into().unwrap());
            write_u16(&mut result.bytes, offset, value ^ (1 << 15));
        }
        ScalarAddFixtureMutation::DescriptorReservedByte => {
            result.bytes[result.descriptor_offset + 63] ^= 0x80;
        }
        ScalarAddFixtureMutation::MachineBytes => result.bytes[result.text_offset] ^= 0x01,
        _ => {}
    }
    result
}

#[allow(dead_code)]
fn replace_metadata_fixint(bytes: &mut [u8], key: &[u8], expected: u8, replacement: u8) {
    let position = bytes
        .windows(key.len())
        .position(|window| window == key)
        .expect("fixture metadata contains the requested key");
    let value = position + key.len();
    assert_eq!(bytes[value], expected);
    bytes[value] = replacement;
}

#[allow(dead_code)]
fn add_static_symbol(fixture: &mut Fixture, local: bool, defined: bool) {
    let header = section_header_offset(&fixture.bytes, SCALAR_V1_SYMTAB_SECTION_INDEX);
    let table = read_u64(&fixture.bytes, header + 24) as usize;
    let strings_header = section_header_offset(&fixture.bytes, SCALAR_V1_STRTAB_SECTION_INDEX);
    let strings = read_u64(&fixture.bytes, strings_header + 24) as usize;
    let name_offset = read_u32(&fixture.bytes, table + 24) as usize;
    let name_start = strings + name_offset;
    let name_len = fixture.bytes[name_start..]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap();
    fixture.bytes[name_start..name_start + name_len].fill(b'x');

    let symbol = table + 24;
    if !defined {
        write_u32(&mut fixture.bytes, symbol, 0);
    }
    fixture.bytes[symbol + 4] = if local { 0 } else { 0x12 };
    fixture.bytes[symbol + 5] = 0;
    write_u16(
        &mut fixture.bytes,
        symbol + 6,
        if defined {
            if local {
                0xfff1
            } else {
                SCALAR_V1_TEXT_SECTION_INDEX as u16
            }
        } else {
            0
        },
    );
    write_u64(
        &mut fixture.bytes,
        symbol + 8,
        if defined && !local {
            (fixture.text_offset + 0x1000) as u64
        } else {
            0
        },
    );
    write_u64(
        &mut fixture.bytes,
        symbol + 16,
        u64::from(defined && !local),
    );
}

#[allow(dead_code)]
fn add_dynamic_symbol(fixture: &mut Fixture, defined: bool) {
    let dynsym_header = section_header_offset(&fixture.bytes, SCALAR_V1_DYNSYM_SECTION_INDEX);
    let dynsym_offset = read_u64(&fixture.bytes, dynsym_header + 24) as usize;
    let entry_name = read_u32(&fixture.bytes, dynsym_offset + 24);
    let symbol = dynsym_offset + 48;
    if defined {
        write_u32(&mut fixture.bytes, symbol, entry_name);
    }
    write_u16(
        &mut fixture.bytes,
        symbol + 6,
        if defined {
            SCALAR_V1_RODATA_SECTION_INDEX as u16
        } else {
            0
        },
    );
    if !defined {
        write_u64(&mut fixture.bytes, symbol + 8, 0);
        write_u64(&mut fixture.bytes, symbol + 16, 0);
    }
}

#[allow(dead_code)]
fn add_relocation_section(fixture: &mut Fixture, with_addend: bool) {
    let entry_size = if with_addend { 24 } else { 16 };
    let mut relocation = vec![0_u8; entry_size];
    write_u64(&mut relocation, 0, (fixture.text_offset + 0x1000) as u64);
    write_u64(&mut relocation, 8, (1_u64 << 32) | 1);
    let header = section_header_offset(&fixture.bytes, SCALAR_V1_COMMENT_SECTION_INDEX);
    let offset = read_u64(&fixture.bytes, header + 24) as usize;
    fixture.bytes[offset..offset + entry_size].copy_from_slice(&relocation);
    write_u32(
        &mut fixture.bytes,
        header + 4,
        if with_addend { 4 } else { 9 },
    );
    write_u64(&mut fixture.bytes, header + 8, 0);
    write_u64(&mut fixture.bytes, header + 32, entry_size as u64);
    write_u32(
        &mut fixture.bytes,
        header + 40,
        SCALAR_V1_SYMTAB_SECTION_INDEX as u32,
    );
    write_u32(
        &mut fixture.bytes,
        header + 44,
        SCALAR_V1_TEXT_SECTION_INDEX as u32,
    );
    write_u64(&mut fixture.bytes, header + 48, 8);
    write_u64(&mut fixture.bytes, header + 56, entry_size as u64);
}

#[allow(dead_code)]
fn add_dynamic_section(fixture: &mut Fixture, entries: &[(i64, u64)]) {
    let header = section_header_offset(&fixture.bytes, SCALAR_V1_DYNAMIC_SECTION_INDEX);
    let offset = read_u64(&fixture.bytes, header + 24) as usize;
    let old_size = read_u64(&fixture.bytes, header + 32) as usize;
    let new_size = entries.len() * 16;
    assert!(new_size <= old_size);
    for (index, (tag, value)) in entries.iter().enumerate() {
        let entry = offset + index * 16;
        fixture.bytes[entry..entry + 8].copy_from_slice(&tag.to_le_bytes());
        fixture.bytes[entry + 8..entry + 16].copy_from_slice(&value.to_le_bytes());
    }
}

#[allow(dead_code)]
fn set_relocation_section(fixture: &mut Fixture, section_type: u32) {
    let header = section_header_offset(&fixture.bytes, SCALAR_V1_COMMENT_SECTION_INDEX);
    write_u32(&mut fixture.bytes, header + 4, section_type);
}

#[allow(dead_code)]
fn mutate_dynamic_value(fixture: &mut Fixture, tag: i64, mask: u64) {
    let header = section_header_offset(&fixture.bytes, SCALAR_V1_DYNAMIC_SECTION_INDEX);
    let offset = read_u64(&fixture.bytes, header + 24) as usize;
    let size = read_u64(&fixture.bytes, header + 32) as usize;
    let entry = (0..size / 16)
        .map(|index| offset + index * 16)
        .find(|entry| read_u64(&fixture.bytes, *entry) == tag as u64)
        .unwrap();
    let value = read_u64(&fixture.bytes, entry + 8);
    write_u64(&mut fixture.bytes, entry + 8, value ^ mask);
}

#[allow(dead_code)]
fn mutate_section_word(fixture: &mut Fixture, section: usize, relative_offset: usize) {
    let header = section_header_offset(&fixture.bytes, section);
    let offset = read_u64(&fixture.bytes, header + 24) as usize + relative_offset;
    xor_u32(&mut fixture.bytes, offset, 1);
}

#[allow(dead_code)]
fn mutate_section_field_u32(fixture: &mut Fixture, section: usize, field: usize, mask: u32) {
    let offset = section_header_offset(&fixture.bytes, section) + field;
    xor_u32(&mut fixture.bytes, offset, mask);
}

#[allow(dead_code)]
fn mutate_section_field_u64(fixture: &mut Fixture, section: usize, field: usize, mask: u64) {
    let offset = section_header_offset(&fixture.bytes, section) + field;
    let value = read_u64(&fixture.bytes, offset);
    write_u64(&mut fixture.bytes, offset, value ^ mask);
}

#[allow(dead_code)]
fn mutate_program_offset(fixture: &mut Fixture) {
    let program = ELF_HEADER_BYTES + 2 * PROGRAM_HEADER_BYTES;
    let offset = read_u64(&fixture.bytes, program + 8);
    write_u64(&mut fixture.bytes, program + 8, offset + 1);
}

#[allow(dead_code)]
fn duplicate_section_header_type(fixture: &mut Fixture, destination: usize, source: usize) {
    let destination = section_header_offset(&fixture.bytes, destination);
    let source = section_header_offset(&fixture.bytes, source);
    let name = read_u32(&fixture.bytes, source);
    let section_type = read_u32(&fixture.bytes, source + 4);
    write_u32(&mut fixture.bytes, destination, name);
    write_u32(&mut fixture.bytes, destination + 4, section_type);
}

#[allow(dead_code)]
fn mutate_static_symbol_section(fixture: &mut Fixture, symbol_index: usize, section: u16) {
    let header = section_header_offset(&fixture.bytes, SCALAR_V1_SYMTAB_SECTION_INDEX);
    let table = read_u64(&fixture.bytes, header + 24) as usize;
    write_u16(&mut fixture.bytes, table + symbol_index * 24 + 6, section);
}

#[allow(dead_code)]
fn mutate_symbol_null(fixture: &mut Fixture, section: usize) {
    let header = section_header_offset(&fixture.bytes, section);
    let table = read_u64(&fixture.bytes, header + 24) as usize;
    fixture.bytes[table + 4] = 1;
}

#[allow(dead_code)]
fn mutate_section_out_of_bounds(fixture: &mut Fixture, section: usize) {
    let header = section_header_offset(&fixture.bytes, section);
    write_u64(&mut fixture.bytes, header + 24, u64::MAX - 7);
    write_u64(&mut fixture.bytes, header + 32, 16);
}

#[allow(dead_code)]
fn mutate_entry_sizes(fixture: &mut Fixture) {
    for section in [
        SCALAR_V1_SYMTAB_SECTION_INDEX,
        SCALAR_V1_DYNSYM_SECTION_INDEX,
    ] {
        let header = section_header_offset(&fixture.bytes, section);
        let table = read_u64(&fixture.bytes, header + 24) as usize;
        let symbol_index = if section == SCALAR_V1_SYMTAB_SECTION_INDEX {
            10
        } else {
            1
        };
        write_u64(&mut fixture.bytes, table + symbol_index * 24 + 16, 55);
    }
}

fn section_header_offset(bytes: &[u8], index: usize) -> usize {
    read_u64(bytes, 40) as usize + index * SECTION_HEADER_BYTES
}

fn xor_u32(bytes: &mut [u8], offset: usize, mask: u32) {
    let value = read_u32(bytes, offset);
    write_u32(bytes, offset, value ^ mask);
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn fixture(options: FixtureOptions<'_>) -> Fixture {
    fixture_with_descriptor_table(options, None)
}

fn fixture_with_descriptor_table(
    options: FixtureOptions<'_>,
    descriptor_table: Option<&[u8]>,
) -> Fixture {
    if options.abi == FixtureAbi::ScalarAddV1 {
        return measured_scalar_loader_fixture(options, descriptor_table);
    }
    legacy_fixture_with_descriptor_table(options, descriptor_table)
}

fn measured_scalar_loader_fixture(
    options: FixtureOptions<'_>,
    descriptor_table: Option<&[u8]>,
) -> Fixture {
    const TEXT_BYTES: usize = 0x440;
    const ENTRY_BYTES: usize = 56;
    const DYNAMIC_BYTES: usize = 8 * 16;
    const COMMENT_BYTES: &[u8] = b"Linker: LLD 22.1.8 (https://github.com/llvm/llvm-project.git ca7933e47d3a3451d81e72ac174dcb5aa28b59d1)\0";
    const RESOURCE_SYMBOLS: [(&str, u64); 8] = [
        ("scalar_add.private_seg_size", 0),
        ("scalar_add.num_vgpr", 2),
        ("scalar_add.num_agpr", 0),
        ("scalar_add.numbered_sgpr", 8),
        ("scalar_add.uses_vcc", 0),
        ("scalar_add.uses_flat_scratch", 0),
        ("scalar_add.has_dyn_sized_stack", 0),
        ("scalar_add.has_recursion", 0),
    ];

    let metadata = metadata(options);
    let note = metadata_note(&metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + SCALAR_V1_PROGRAM_COUNT * PROGRAM_HEADER_BYTES];

    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    let mut dynstr = vec![0];
    let dynamic_entry_name = push_name(&mut dynstr, options.entry);
    let dynamic_descriptor_name = push_name(&mut dynstr, options.descriptor);

    align(&mut bytes, 8);
    let dynsym_offset = bytes.len();
    bytes.resize(dynsym_offset + 3 * 24, 0);

    align(&mut bytes, 8);
    let gnu_hash_offset = bytes.len();
    bytes.resize(gnu_hash_offset + 36, 0);
    let entry_hash = gnu_symbol_hash(options.entry.as_bytes());
    let descriptor_hash = gnu_symbol_hash(options.descriptor.as_bytes());
    write_u32(&mut bytes, gnu_hash_offset, 1);
    write_u32(&mut bytes, gnu_hash_offset + 4, 1);
    write_u32(&mut bytes, gnu_hash_offset + 8, 1);
    write_u32(&mut bytes, gnu_hash_offset + 12, 26);
    let bloom = (1_u64 << (entry_hash % 64))
        | (1_u64 << ((entry_hash >> 26) % 64))
        | (1_u64 << (descriptor_hash % 64))
        | (1_u64 << ((descriptor_hash >> 26) % 64));
    write_u64(&mut bytes, gnu_hash_offset + 16, bloom);
    write_u32(&mut bytes, gnu_hash_offset + 24, 1);
    write_u32(&mut bytes, gnu_hash_offset + 28, entry_hash & !1);
    write_u32(&mut bytes, gnu_hash_offset + 32, descriptor_hash | 1);

    align(&mut bytes, 4);
    let hash_offset = bytes.len();
    for word in [3_u32, 3, 0, 1, 2, 0, 0, 0] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }

    let dynstr_offset = bytes.len();
    bytes.extend_from_slice(&dynstr);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let descriptor_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0);

    align(&mut bytes, 256);
    let text_offset = bytes.len();
    bytes.resize(text_offset + ENTRY_BYTES, 0xbf);
    while bytes.len() < text_offset + TEXT_BYTES {
        bytes.extend_from_slice(&[0, 0, 0x80, 0xbf]);
    }
    let entry_address = (text_offset + 0x1000) as u64;

    align(&mut bytes, 64);
    let dynamic_offset = bytes.len();
    let dynamic_address = (dynamic_offset + 0x2000) as u64;
    let dynamic_entries = [
        (30_i64, 2_u64),
        (6, dynsym_offset as u64),
        (11, 24),
        (5, dynstr_offset as u64),
        (10, dynstr.len() as u64),
        (0x6fff_fef5, gnu_hash_offset as u64),
        (4, hash_offset as u64),
        (0, 0),
    ];
    for (tag, value) in dynamic_entries {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(bytes.len() - dynamic_offset, DYNAMIC_BYTES);
    let dynamic_end = dynamic_address + DYNAMIC_BYTES as u64;
    let relro_end = (dynamic_end + 0xfff) & !0xfff;
    let relro_padding_size = relro_end - dynamic_end;

    let gpr_offset = bytes.len();
    if let Some(table) = descriptor_table {
        bytes.extend_from_slice(table);
    }
    let gpr_size = bytes.len() - gpr_offset;
    let comment_offset = bytes.len();
    bytes.extend_from_slice(COMMENT_BYTES);

    let mut strtab = vec![0];
    let resource_names = RESOURCE_SYMBOLS
        .iter()
        .map(|(name, _)| push_name(&mut strtab, name))
        .collect::<Vec<_>>();
    let dynamic_name = push_name(&mut strtab, "_DYNAMIC");
    let entry_name = push_name(&mut strtab, options.entry);
    let descriptor_name = push_name(&mut strtab, options.descriptor);

    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    bytes.resize(symtab_offset + 12 * 24, 0);
    for (index, ((_, value), name)) in RESOURCE_SYMBOLS.into_iter().zip(resource_names).enumerate()
    {
        write_fixture_symbol(
            &mut bytes,
            symtab_offset + (index + 1) * 24,
            name,
            0,
            0,
            0xfff1,
            value,
            0,
        );
    }
    write_fixture_symbol(
        &mut bytes,
        symtab_offset + 9 * 24,
        dynamic_name,
        0,
        2,
        SCALAR_V1_DYNAMIC_SECTION_INDEX as u16,
        dynamic_address,
        0,
    );
    write_fixture_symbol(
        &mut bytes,
        symtab_offset + 10 * 24,
        entry_name,
        0x12,
        3,
        SCALAR_V1_TEXT_SECTION_INDEX as u16,
        entry_address,
        ENTRY_BYTES as u64,
    );
    write_fixture_symbol(
        &mut bytes,
        symtab_offset + 11 * 24,
        descriptor_name,
        0x11,
        0,
        SCALAR_V1_RODATA_SECTION_INDEX as u16,
        rodata_offset as u64,
        64,
    );

    let mut shstr = vec![0];
    let section_names = [
        "",
        ".note",
        ".dynsym",
        ".gnu.hash",
        ".hash",
        ".dynstr",
        ".rodata",
        ".text",
        ".dynamic",
        ".relro_padding",
        if options.include_canonical_descriptor_section_name || descriptor_table.is_some() {
            DEVICE_DESCRIPTOR_SECTION_NAME
        } else {
            ".AMDGPU.gpr_maximums"
        },
        ".comment",
        ".symtab",
        ".shstrtab",
        ".strtab",
    ];
    let section_name_offsets = section_names
        .into_iter()
        .map(|name| {
            if name.is_empty() {
                0
            } else {
                push_name(&mut shstr, name)
            }
        })
        .collect::<Vec<_>>();
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);

    write_fixture_symbol(
        &mut bytes,
        dynsym_offset + 24,
        dynamic_entry_name,
        0x12,
        3,
        SCALAR_V1_TEXT_SECTION_INDEX as u16,
        entry_address,
        ENTRY_BYTES as u64,
    );
    write_fixture_symbol(
        &mut bytes,
        dynsym_offset + 48,
        dynamic_descriptor_name,
        0x11,
        0,
        SCALAR_V1_RODATA_SECTION_INDEX as u16,
        rodata_offset as u64,
        64,
    );

    write_u32(
        &mut bytes,
        descriptor_offset,
        u32::try_from(options.group_segment_fixed_size).unwrap(),
    );
    write_u32(
        &mut bytes,
        descriptor_offset + 8,
        u32::try_from(kernarg_segment_size(options)).unwrap(),
    );
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - rodata_offset as u64).unwrap(),
    );
    write_u32(&mut bytes, descriptor_offset + 44, 1);
    write_u32(&mut bytes, descriptor_offset + 48, 0x00af_0081);
    write_u32(
        &mut bytes,
        descriptor_offset + 52,
        0x1390 | u32::from(options.uses_dynamic_stack.unwrap_or(false)),
    );
    let mut kernel_code_properties = 0x001e;
    if options.descriptor_wavefront_size == 32 {
        kernel_code_properties |= 1 << 10;
    }
    if options.uses_dynamic_stack.unwrap_or(false) {
        kernel_code_properties |= 1 << 11;
    }
    write_u16(&mut bytes, descriptor_offset + 56, kernel_code_properties);

    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    bytes.resize(
        section_table_offset + SCALAR_V1_SECTION_COUNT * SECTION_HEADER_BYTES,
        0,
    );
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = options.code_object_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, target_flags(options.target));
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, SCALAR_V1_PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, SCALAR_V1_SECTION_COUNT as u16);
    write_u16(&mut bytes, 62, SCALAR_V1_SHSTRTAB_SECTION_INDEX as u16);

    write_fixture_program(&mut bytes, 0, 6, 4, 64, 64, 448, 448, 8);
    write_fixture_program(
        &mut bytes,
        1,
        1,
        4,
        0,
        0,
        text_offset as u64,
        text_offset as u64,
        0x1000,
    );
    write_fixture_program(
        &mut bytes,
        2,
        1,
        5,
        text_offset as u64,
        entry_address,
        TEXT_BYTES as u64,
        TEXT_BYTES as u64,
        0x1000,
    );
    write_fixture_program(
        &mut bytes,
        3,
        1,
        6,
        dynamic_offset as u64,
        dynamic_address,
        DYNAMIC_BYTES as u64,
        DYNAMIC_BYTES as u64 + relro_padding_size,
        0x1000,
    );
    write_fixture_program(
        &mut bytes,
        4,
        2,
        6,
        dynamic_offset as u64,
        dynamic_address,
        DYNAMIC_BYTES as u64,
        DYNAMIC_BYTES as u64,
        8,
    );
    write_fixture_program(
        &mut bytes,
        5,
        0x6474_e552,
        4,
        dynamic_offset as u64,
        dynamic_address,
        DYNAMIC_BYTES as u64,
        DYNAMIC_BYTES as u64 + relro_padding_size,
        1,
    );
    write_fixture_program(&mut bytes, 6, 0x6474_e551, 6, 0, 0, 0, 0, 0);
    write_fixture_program(
        &mut bytes,
        7,
        4,
        4,
        note_offset as u64,
        note_offset as u64,
        note.len() as u64,
        note.len() as u64,
        4,
    );

    let sections = [
        (0, 0, 0, 0, 0, 0, 0, 0, 0),
        (
            7,
            2,
            note_offset as u64,
            note_offset as u64,
            note.len() as u64,
            0,
            0,
            4,
            0,
        ),
        (
            11,
            2,
            dynsym_offset as u64,
            dynsym_offset as u64,
            72,
            5,
            1,
            8,
            24,
        ),
        (
            0x6fff_fff6,
            2,
            gnu_hash_offset as u64,
            gnu_hash_offset as u64,
            36,
            2,
            0,
            8,
            0,
        ),
        (5, 2, hash_offset as u64, hash_offset as u64, 32, 2, 0, 4, 4),
        (
            3,
            2,
            dynstr_offset as u64,
            dynstr_offset as u64,
            dynstr.len() as u64,
            0,
            0,
            1,
            0,
        ),
        (
            1,
            2,
            rodata_offset as u64,
            rodata_offset as u64,
            64,
            0,
            0,
            64,
            0,
        ),
        (
            1,
            6,
            entry_address,
            text_offset as u64,
            TEXT_BYTES as u64,
            0,
            0,
            256,
            0,
        ),
        (
            6,
            3,
            dynamic_address,
            dynamic_offset as u64,
            DYNAMIC_BYTES as u64,
            5,
            0,
            8,
            16,
        ),
        (
            8,
            3,
            dynamic_end,
            (dynamic_offset + DYNAMIC_BYTES) as u64,
            relro_padding_size,
            0,
            0,
            1,
            0,
        ),
        (1, 0, 0, gpr_offset as u64, gpr_size as u64, 0, 0, 1, 0),
        (
            1,
            0x30,
            0,
            comment_offset as u64,
            COMMENT_BYTES.len() as u64,
            0,
            0,
            1,
            1,
        ),
        (2, 0, 0, symtab_offset as u64, 12 * 24, 14, 10, 8, 24),
        (
            3,
            0,
            0,
            shstrtab_offset as u64,
            shstr.len() as u64,
            0,
            0,
            1,
            0,
        ),
        (
            3,
            0,
            0,
            strtab_offset as u64,
            strtab.len() as u64,
            0,
            0,
            1,
            0,
        ),
    ];
    for (index, (section_type, flags, address, offset, size, link, info, alignment, entry_size)) in
        sections.into_iter().enumerate()
    {
        write_fixture_section(
            &mut bytes,
            section_table_offset,
            index,
            section_name_offsets[index],
            section_type,
            flags,
            address,
            offset,
            size,
            link,
            info,
            alignment,
            entry_size,
        );
    }

    Fixture {
        bytes,
        text_offset,
        descriptor_offset,
    }
}

#[allow(dead_code)]
fn scalar_loader_fixture(options: FixtureOptions<'_>, descriptor_table: Option<&[u8]>) -> Fixture {
    let metadata = metadata(options);
    let note = metadata_note(&metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + SCALAR_PROGRAM_COUNT * PROGRAM_HEADER_BYTES];

    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let descriptor_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0);

    let mut dynstr = vec![0];
    let dynamic_entry_name = push_name(&mut dynstr, options.entry);
    let dynamic_descriptor_name = push_name(&mut dynstr, options.descriptor);
    let dynstr_offset = bytes.len();
    bytes.extend_from_slice(&dynstr);

    align(&mut bytes, 8);
    let dynsym_offset = bytes.len();
    // Keep one spare entry in the payload so hostile inventory fixtures can
    // remain inside the same production-shaped read-only load mapping.
    bytes.resize(dynsym_offset + 4 * 24, 0);

    align(&mut bytes, 4);
    let hash_offset = bytes.len();
    bytes.resize(hash_offset + 7 * 4, 0);
    write_u32(&mut bytes, hash_offset, 1);
    write_u32(&mut bytes, hash_offset + 4, 3);
    write_u32(&mut bytes, hash_offset + 8, 1);
    write_u32(&mut bytes, hash_offset + 16, 2);

    align(&mut bytes, 8);
    let gnu_hash_offset = bytes.len();
    bytes.resize(gnu_hash_offset + 40, 0);
    let entry_hash = gnu_symbol_hash(options.entry.as_bytes());
    let descriptor_hash = gnu_symbol_hash(options.descriptor.as_bytes());
    write_u32(&mut bytes, gnu_hash_offset, 1);
    write_u32(&mut bytes, gnu_hash_offset + 4, 1);
    write_u32(&mut bytes, gnu_hash_offset + 8, 1);
    write_u32(&mut bytes, gnu_hash_offset + 12, 26);
    let bloom = (1_u64 << (entry_hash % 64))
        | (1_u64 << ((entry_hash >> 26) % 64))
        | (1_u64 << (descriptor_hash % 64))
        | (1_u64 << ((descriptor_hash >> 26) % 64));
    write_u64(&mut bytes, gnu_hash_offset + 16, bloom);
    write_u32(&mut bytes, gnu_hash_offset + 24, 1);
    write_u32(&mut bytes, gnu_hash_offset + 28, entry_hash & !1);
    write_u32(&mut bytes, gnu_hash_offset + 32, descriptor_hash | 1);
    let read_load_end = bytes.len();

    align(&mut bytes, 256);
    let text_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0xbf);
    let export_offset = if options.include_export {
        align(&mut bytes, 256);
        let offset = bytes.len();
        bytes.resize(bytes.len() + 64, 0xbe);
        Some(offset)
    } else {
        None
    };
    let text_end = bytes.len();

    align(&mut bytes, 64);
    let dynamic_offset = bytes.len();
    let dynamic_address = (dynamic_offset + 0x2000) as u64;
    let dynamic_entries = [
        (4_i64, hash_offset as u64),
        (5, dynstr_offset as u64),
        (6, dynsym_offset as u64),
        (10, dynstr.len() as u64),
        (11, 24),
        (0x6fff_fef5, gnu_hash_offset as u64),
        (30, 2),
        (0, 0),
    ];
    for (tag, value) in dynamic_entries {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let dynamic_size = bytes.len() - dynamic_offset;

    let mut strtab = vec![0];
    let entry_name = push_name(&mut strtab, options.entry);
    let descriptor_name = push_name(&mut strtab, options.descriptor);
    let export_name = options
        .include_export
        .then(|| push_name(&mut strtab, "ffi_export"));
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);

    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 3 + usize::from(options.include_export);
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    let entry_address = (text_offset + 0x1000) as u64;
    write_fixture_symbol(
        &mut bytes,
        symtab_offset + 24,
        entry_name,
        0x12,
        3,
        TEXT_SECTION_INDEX as u16,
        entry_address,
        64,
    );
    write_fixture_symbol(
        &mut bytes,
        symtab_offset + 48,
        descriptor_name,
        0x11,
        0,
        RODATA_SECTION_INDEX as u16,
        descriptor_offset as u64,
        64,
    );
    if let (Some(name), Some(offset)) = (export_name, export_offset) {
        write_fixture_symbol(
            &mut bytes,
            symtab_offset + 72,
            name,
            0x12,
            0,
            TEXT_SECTION_INDEX as u16,
            (offset + 0x1000) as u64,
            64,
        );
    }
    write_fixture_symbol(
        &mut bytes,
        dynsym_offset + 24,
        dynamic_entry_name,
        0x12,
        3,
        TEXT_SECTION_INDEX as u16,
        entry_address,
        64,
    );
    write_fixture_symbol(
        &mut bytes,
        dynsym_offset + 48,
        dynamic_descriptor_name,
        0x11,
        0,
        RODATA_SECTION_INDEX as u16,
        descriptor_offset as u64,
        64,
    );

    align(&mut bytes, 8);
    let canonical_descriptor_offset = bytes.len();
    if let Some(table) = descriptor_table {
        bytes.extend_from_slice(table);
    }

    write_u32(
        &mut bytes,
        descriptor_offset,
        u32::try_from(options.group_segment_fixed_size).unwrap(),
    );
    write_u32(
        &mut bytes,
        descriptor_offset + 8,
        u32::try_from(kernarg_segment_size(options)).unwrap(),
    );
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - descriptor_offset as u64).unwrap(),
    );
    write_u32(&mut bytes, descriptor_offset + 44, 1);
    write_u32(&mut bytes, descriptor_offset + 48, 0x00af_0081);
    write_u32(
        &mut bytes,
        descriptor_offset + 52,
        0x1390 | u32::from(options.uses_dynamic_stack.unwrap_or(false)),
    );
    let mut kernel_code_properties = 0x001e;
    if options.descriptor_wavefront_size == 32 {
        kernel_code_properties |= 1 << 10;
    }
    if options.uses_dynamic_stack.unwrap_or(false) {
        kernel_code_properties |= 1 << 11;
    }
    write_u16(&mut bytes, descriptor_offset + 56, kernel_code_properties);

    let mut shstr = vec![0];
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let canonical_descriptor_name = push_name(
        &mut shstr,
        if options.include_canonical_descriptor_section_name || descriptor_table.is_some() {
            DEVICE_DESCRIPTOR_SECTION_NAME
        } else {
            ".fixture"
        },
    );
    let shstrtab_name = push_name(&mut shstr, ".shstrtab");
    let dynstr_name = push_name(&mut shstr, ".dynstr");
    let dynsym_name = push_name(&mut shstr, ".dynsym");
    let hash_name = push_name(&mut shstr, ".hash");
    let gnu_hash_name = push_name(&mut shstr, ".gnu.hash");
    let dynamic_name = push_name(&mut shstr, ".dynamic");
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    bytes.resize(
        section_table_offset + SCALAR_SECTION_COUNT * SECTION_HEADER_BYTES,
        0,
    );

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = options.code_object_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, target_flags(options.target));
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, SCALAR_PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, SCALAR_SECTION_COUNT as u16);
    write_u16(&mut bytes, 62, SHSTRTAB_SECTION_INDEX as u16);

    write_fixture_program(
        &mut bytes,
        SCALAR_READ_LOAD_PROGRAM_INDEX,
        1,
        4,
        0,
        0,
        read_load_end as u64,
        read_load_end as u64,
        0x1000,
    );
    write_fixture_program(
        &mut bytes,
        SCALAR_EXEC_LOAD_PROGRAM_INDEX,
        1,
        5,
        text_offset as u64,
        entry_address,
        (text_end - text_offset) as u64,
        (text_end - text_offset) as u64,
        0x1000,
    );
    write_fixture_program(
        &mut bytes,
        SCALAR_WRITE_LOAD_PROGRAM_INDEX,
        1,
        6,
        dynamic_offset as u64,
        dynamic_address,
        dynamic_size as u64,
        dynamic_size as u64,
        0x1000,
    );
    write_fixture_program(
        &mut bytes,
        SCALAR_NOTE_PROGRAM_INDEX,
        4,
        4,
        note_offset as u64,
        note_offset as u64,
        note.len() as u64,
        note.len() as u64,
        4,
    );
    write_fixture_program(
        &mut bytes,
        SCALAR_DYNAMIC_PROGRAM_INDEX,
        2,
        6,
        dynamic_offset as u64,
        dynamic_address,
        dynamic_size as u64,
        dynamic_size as u64,
        8,
    );

    write_fixture_section(
        &mut bytes,
        section_table_offset,
        NOTE_SECTION_INDEX,
        note_name,
        7,
        2,
        note_offset as u64,
        note_offset as u64,
        note.len() as u64,
        0,
        0,
        4,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        RODATA_SECTION_INDEX,
        rodata_name,
        1,
        2,
        rodata_offset as u64,
        rodata_offset as u64,
        64,
        0,
        0,
        64,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        TEXT_SECTION_INDEX,
        text_name,
        1,
        6,
        entry_address,
        text_offset as u64,
        (text_end - text_offset) as u64,
        0,
        0,
        256,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        STRTAB_SECTION_INDEX,
        strtab_name,
        3,
        0,
        0,
        strtab_offset as u64,
        strtab.len() as u64,
        0,
        0,
        1,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        SYMTAB_SECTION_INDEX,
        symtab_name,
        2,
        0,
        0,
        symtab_offset as u64,
        (symbol_count * 24) as u64,
        STRTAB_SECTION_INDEX as u32,
        1,
        8,
        24,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        CANONICAL_DESCRIPTOR_SECTION_INDEX,
        canonical_descriptor_name,
        1,
        0,
        0,
        canonical_descriptor_offset as u64,
        descriptor_table.map_or(0, |table| table.len()) as u64,
        0,
        0,
        8,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        SHSTRTAB_SECTION_INDEX,
        shstrtab_name,
        3,
        0,
        0,
        shstrtab_offset as u64,
        shstr.len() as u64,
        0,
        0,
        1,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        DYNSTR_SECTION_INDEX,
        dynstr_name,
        3,
        2,
        dynstr_offset as u64,
        dynstr_offset as u64,
        dynstr.len() as u64,
        0,
        0,
        1,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        DYNSYM_SECTION_INDEX,
        dynsym_name,
        11,
        2,
        dynsym_offset as u64,
        dynsym_offset as u64,
        3 * 24,
        DYNSTR_SECTION_INDEX as u32,
        1,
        8,
        24,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        HASH_SECTION_INDEX,
        hash_name,
        5,
        2,
        hash_offset as u64,
        hash_offset as u64,
        6 * 4,
        DYNSYM_SECTION_INDEX as u32,
        0,
        4,
        4,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        GNU_HASH_SECTION_INDEX,
        gnu_hash_name,
        0x6fff_fff6,
        2,
        gnu_hash_offset as u64,
        gnu_hash_offset as u64,
        36,
        DYNSYM_SECTION_INDEX as u32,
        0,
        8,
        0,
    );
    write_fixture_section(
        &mut bytes,
        section_table_offset,
        DYNAMIC_SECTION_INDEX,
        dynamic_name,
        6,
        3,
        dynamic_address,
        dynamic_offset as u64,
        dynamic_size as u64,
        DYNSTR_SECTION_INDEX as u32,
        0,
        8,
        16,
    );

    Fixture {
        bytes,
        text_offset,
        descriptor_offset,
    }
}

#[allow(clippy::too_many_arguments)]
fn write_fixture_section(
    bytes: &mut [u8],
    table: usize,
    index: usize,
    name: u32,
    section_type: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
) {
    let header = table + index * SECTION_HEADER_BYTES;
    write_u32(bytes, header, name);
    write_u32(bytes, header + 4, section_type);
    write_u64(bytes, header + 8, flags);
    write_u64(bytes, header + 16, address);
    write_u64(bytes, header + 24, offset);
    write_u64(bytes, header + 32, size);
    write_u32(bytes, header + 40, link);
    write_u32(bytes, header + 44, info);
    write_u64(bytes, header + 48, alignment);
    write_u64(bytes, header + 56, entry_size);
}

#[allow(clippy::too_many_arguments)]
fn write_fixture_program(
    bytes: &mut [u8],
    index: usize,
    program_type: u32,
    flags: u32,
    offset: u64,
    address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
) {
    let header = ELF_HEADER_BYTES + index * PROGRAM_HEADER_BYTES;
    write_u32(bytes, header, program_type);
    write_u32(bytes, header + 4, flags);
    write_u64(bytes, header + 8, offset);
    write_u64(bytes, header + 16, address);
    write_u64(bytes, header + 24, address);
    write_u64(bytes, header + 32, file_size);
    write_u64(bytes, header + 40, memory_size);
    write_u64(bytes, header + 48, alignment);
}

#[allow(clippy::too_many_arguments)]
fn write_fixture_symbol(
    bytes: &mut [u8],
    offset: usize,
    name: u32,
    info: u8,
    other: u8,
    section: u16,
    value: u64,
    size: u64,
) {
    write_u32(bytes, offset, name);
    bytes[offset + 4] = info;
    bytes[offset + 5] = other;
    write_u16(bytes, offset + 6, section);
    write_u64(bytes, offset + 8, value);
    write_u64(bytes, offset + 16, size);
}

fn gnu_symbol_hash(name: &[u8]) -> u32 {
    name.iter().fold(5381_u32, |hash, byte| {
        hash.wrapping_mul(33).wrapping_add(u32::from(*byte))
    })
}

fn legacy_fixture_with_descriptor_table(
    options: FixtureOptions<'_>,
    descriptor_table: Option<&[u8]>,
) -> Fixture {
    const PROGRAM_COUNT: usize = 2;
    let metadata = metadata(options);
    let note = metadata_note(&metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];

    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let descriptor_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0);
    let rodata_end = bytes.len();

    align(&mut bytes, 256);
    let text_offset = bytes.len();
    bytes.resize(bytes.len() + 64, 0xbf);
    let export_offset = if options.include_export {
        align(&mut bytes, 256);
        let offset = bytes.len();
        bytes.resize(bytes.len() + 64, 0xbe);
        Some(offset)
    } else {
        None
    };
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let entry_name = push_name(&mut strtab, options.entry);
    let descriptor_name = push_name(&mut strtab, options.descriptor);
    let export_name = options
        .include_export
        .then(|| push_name(&mut strtab, "ffi_export"));
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);

    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 3 + usize::from(options.include_export);
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    let entry_symbol = symtab_offset + 24;
    write_u32(&mut bytes, entry_symbol, entry_name);
    bytes[entry_symbol + 4] = 0x12;
    bytes[entry_symbol + 5] = 3;
    write_u16(&mut bytes, entry_symbol + 6, TEXT_SECTION_INDEX as u16);
    let entry_address = (text_offset + 0x1000) as u64;
    write_u64(&mut bytes, entry_symbol + 8, entry_address);
    write_u64(&mut bytes, entry_symbol + 16, 64);

    let descriptor_symbol = symtab_offset + 48;
    write_u32(&mut bytes, descriptor_symbol, descriptor_name);
    bytes[descriptor_symbol + 4] = 0x11;
    write_u16(
        &mut bytes,
        descriptor_symbol + 6,
        RODATA_SECTION_INDEX as u16,
    );
    write_u64(&mut bytes, descriptor_symbol + 8, descriptor_offset as u64);
    write_u64(&mut bytes, descriptor_symbol + 16, 64);

    if let (Some(name), Some(offset)) = (export_name, export_offset) {
        let export_symbol = symtab_offset + 72;
        write_u32(&mut bytes, export_symbol, name);
        bytes[export_symbol + 4] = 0x12;
        write_u16(&mut bytes, export_symbol + 6, TEXT_SECTION_INDEX as u16);
        write_u64(&mut bytes, export_symbol + 8, (offset + 0x1000) as u64);
        write_u64(&mut bytes, export_symbol + 16, 64);
    }

    align(&mut bytes, 8);
    let canonical_descriptor_offset = bytes.len();
    if let Some(table) = descriptor_table {
        bytes.extend_from_slice(table);
    }

    write_u32(
        &mut bytes,
        descriptor_offset,
        u32::try_from(options.group_segment_fixed_size).unwrap(),
    );
    write_u32(
        &mut bytes,
        descriptor_offset + 8,
        u32::try_from(kernarg_segment_size(options)).unwrap(),
    );
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - descriptor_offset as u64).unwrap(),
    );
    let (compute_pgm_rsrc3, compute_pgm_rsrc1, compute_pgm_rsrc2) =
        if options.abi == FixtureAbi::RowSoftmaxV1 {
            (10, 0x00af_014a, 0x0390)
        } else {
            (1, 0x00af_0081, 0x1390)
        };
    write_u32(&mut bytes, descriptor_offset + 44, compute_pgm_rsrc3);
    write_u32(&mut bytes, descriptor_offset + 48, compute_pgm_rsrc1);
    write_u32(
        &mut bytes,
        descriptor_offset + 52,
        compute_pgm_rsrc2 | u32::from(options.uses_dynamic_stack.unwrap_or(false)),
    );
    let mut kernel_code_properties = 0x001e;
    if options.descriptor_wavefront_size == 32 {
        kernel_code_properties |= 1 << 10;
    }
    if options.uses_dynamic_stack.unwrap_or(false) {
        kernel_code_properties |= 1 << 11;
    }
    write_u16(&mut bytes, descriptor_offset + 56, kernel_code_properties);

    let mut shstr = vec![0];
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let canonical_descriptor_name = push_name(
        &mut shstr,
        if options.include_canonical_descriptor_section_name || descriptor_table.is_some() {
            DEVICE_DESCRIPTOR_SECTION_NAME
        } else {
            ".fixture"
        },
    );
    let shstrtab_name = push_name(&mut shstr, ".shstrtab");
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    let section_table_offset = bytes.len();
    bytes.resize(section_table_offset + 8 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = options.code_object_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u32(&mut bytes, 48, target_flags(options.target));
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, 8);
    write_u16(&mut bytes, 62, SHSTRTAB_SECTION_INDEX as u16);

    let rodata_program = ELF_HEADER_BYTES;
    write_u32(&mut bytes, rodata_program, 1);
    write_u32(&mut bytes, rodata_program + 4, 4);
    write_u64(&mut bytes, rodata_program + 32, rodata_end as u64);
    write_u64(&mut bytes, rodata_program + 40, rodata_end as u64);
    write_u64(&mut bytes, rodata_program + 48, 0x1000);

    let text_program = ELF_HEADER_BYTES + PROGRAM_HEADER_BYTES;
    write_u32(&mut bytes, text_program, 1);
    write_u32(&mut bytes, text_program + 4, 5);
    write_u64(&mut bytes, text_program + 8, text_offset as u64);
    write_u64(&mut bytes, text_program + 16, (text_offset + 0x1000) as u64);
    write_u64(
        &mut bytes,
        text_program + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(
        &mut bytes,
        text_program + 40,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_program + 48, 0x1000);

    let note_header = section_table_offset + NOTE_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, note_name);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let rodata_header = section_table_offset + RODATA_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, rodata_header, rodata_name);
    write_u32(&mut bytes, rodata_header + 4, 1);
    write_u64(&mut bytes, rodata_header + 8, 2);
    write_u64(&mut bytes, rodata_header + 16, rodata_offset as u64);
    write_u64(&mut bytes, rodata_header + 24, rodata_offset as u64);
    write_u64(
        &mut bytes,
        rodata_header + 32,
        (rodata_end - rodata_offset) as u64,
    );
    write_u64(&mut bytes, rodata_header + 48, 64);

    let text_header = section_table_offset + TEXT_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, text_header, text_name);
    write_u32(&mut bytes, text_header + 4, 1);
    write_u64(&mut bytes, text_header + 8, 6);
    write_u64(&mut bytes, text_header + 16, (text_offset + 0x1000) as u64);
    write_u64(&mut bytes, text_header + 24, text_offset as u64);
    write_u64(
        &mut bytes,
        text_header + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_header + 48, 256);

    let strtab_header = section_table_offset + STRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strtab_header, strtab_name);
    write_u32(&mut bytes, strtab_header + 4, 3);
    write_u64(&mut bytes, strtab_header + 24, strtab_offset as u64);
    write_u64(&mut bytes, strtab_header + 32, strtab.len() as u64);
    write_u64(&mut bytes, strtab_header + 48, 1);

    let symtab_header = section_table_offset + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header, symtab_name);
    write_u32(&mut bytes, symtab_header + 4, 2);
    write_u64(&mut bytes, symtab_header + 24, symtab_offset as u64);
    write_u64(&mut bytes, symtab_header + 32, (symbol_count * 24) as u64);
    write_u32(&mut bytes, symtab_header + 40, STRTAB_SECTION_INDEX as u32);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 48, 8);
    write_u64(&mut bytes, symtab_header + 56, 24);

    let canonical_descriptor_header =
        section_table_offset + CANONICAL_DESCRIPTOR_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(
        &mut bytes,
        canonical_descriptor_header,
        canonical_descriptor_name,
    );
    write_u32(&mut bytes, canonical_descriptor_header + 4, 1);
    write_u64(
        &mut bytes,
        canonical_descriptor_header + 24,
        canonical_descriptor_offset as u64,
    );
    write_u64(
        &mut bytes,
        canonical_descriptor_header + 32,
        descriptor_table.map_or(0, |table| table.len()) as u64,
    );
    write_u64(&mut bytes, canonical_descriptor_header + 48, 8);

    let shstrtab_header = section_table_offset + SHSTRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, shstrtab_header, shstrtab_name);
    write_u32(&mut bytes, shstrtab_header + 4, 3);
    write_u64(&mut bytes, shstrtab_header + 24, shstrtab_offset as u64);
    write_u64(&mut bytes, shstrtab_header + 32, shstr.len() as u64);
    write_u64(&mut bytes, shstrtab_header + 48, 1);

    Fixture {
        bytes,
        text_offset,
        descriptor_offset,
    }
}

fn metadata(options: FixtureOptions<'_>) -> Vec<u8> {
    let explicit_bytes = match options.abi {
        FixtureAbi::SliceF32 => 16,
        FixtureAbi::ScalarAddV1 => 24,
        FixtureAbi::TiledGemmV1 => 64,
        FixtureAbi::RowSoftmaxV1 => 32,
    };
    let mut arguments = match options.abi {
        FixtureAbi::SliceF32 => {
            let alignment = options.include_explicit_argument_alignments.then_some(8);
            vec![
                explicit_pointer_argument(
                    Some("values_ptr"),
                    0,
                    8,
                    alignment,
                    "global_buffer",
                    Some("global"),
                    options
                        .include_pointee_alignment
                        .then_some(options.pointee_alignment),
                ),
                explicit_argument(Some("values_len"), 8, 8, alignment, "by_value", None),
            ]
        }
        FixtureAbi::ScalarAddV1 => vec![
            explicit_pointer_argument(
                Some("input"),
                0,
                8,
                None,
                "global_buffer",
                Some("global"),
                None,
            ),
            explicit_pointer_argument(
                Some("output"),
                8,
                8,
                None,
                "global_buffer",
                Some("global"),
                None,
            ),
            explicit_argument(Some("addend"), 16, 4, None, "by_value", None),
        ],
        FixtureAbi::TiledGemmV1 => (0..4)
            .flat_map(|index| {
                let base = if index == 0 {
                    options.tiled_first_argument_offset
                } else {
                    index * 16
                };
                let value_type = if index < 2 { "u16" } else { "f32" };
                let alignment = options.include_explicit_argument_alignments.then_some(8);
                [
                    typed_explicit_pointer_argument(
                        &format!("arg{index}.data"),
                        base,
                        alignment,
                        value_type,
                    ),
                    typed_explicit_argument(
                        &format!("arg{index}.len"),
                        base + 8,
                        8,
                        alignment,
                        "u64",
                    ),
                ]
            })
            .collect(),
        FixtureAbi::RowSoftmaxV1 => (0..2)
            .flat_map(|index| {
                let base = if index == 0 {
                    options.row_softmax_first_argument_offset
                } else {
                    index * 16
                };
                [
                    explicit_pointer_argument(
                        Some(&format!("arg{index}.data")),
                        base,
                        8,
                        options.include_explicit_argument_alignments.then_some(8),
                        "global_buffer",
                        Some("global"),
                        None,
                    ),
                    explicit_argument(
                        Some(&format!("arg{index}.len")),
                        base + 8,
                        8,
                        options.include_explicit_argument_alignments.then_some(8),
                        "by_value",
                        None,
                    ),
                ]
            })
            .collect(),
    };
    let mut hidden_arguments = v5_hidden_arguments(explicit_bytes);
    if options.include_exact_row_llvm22_hidden_arguments {
        hidden_arguments.extend(
            [
                (80, 8, "hidden_hostcall_buffer"),
                (88, 8, "hidden_multigrid_sync_arg"),
                (96, 8, "hidden_heap_v1"),
                (104, 8, "hidden_default_queue"),
                (112, 8, "hidden_completion_action"),
                (200, 8, "hidden_queue_ptr"),
            ]
            .into_iter()
            .map(|(offset, size, kind)| argument(None, explicit_bytes + offset, size, kind, None)),
        );
    }
    if let Some((relative_offset, size, kind)) = options.optional_hidden_argument {
        hidden_arguments.push(argument(
            None,
            explicit_bytes + relative_offset,
            size,
            kind,
            None,
        ));
    }
    if let Some((relative_offset, size, kind)) = options.second_optional_hidden_argument {
        hidden_arguments.push(argument(
            None,
            explicit_bytes + relative_offset,
            size,
            kind,
            None,
        ));
    }
    if options.include_dynamic_lds_size {
        hidden_arguments.push(argument(
            None,
            explicit_bytes + 120,
            4,
            "hidden_dynamic_lds_size",
            None,
        ));
    }
    if let Some((index, relative_offset, size, kind)) = options.hidden_argument_override {
        hidden_arguments[index] =
            argument(None, explicit_bytes + relative_offset, size, kind, None);
    }
    if let Some(index) = options.omitted_hidden_argument {
        hidden_arguments.remove(index);
    }
    arguments.extend(hidden_arguments);
    if let Some((index, field, value)) = options.argument_extra {
        let Value::Map(fields) = &mut arguments[index] else {
            unreachable!("argument fixtures are maps")
        };
        let value = match value {
            FixtureMetadataValue::String(value) => Value::from(value),
            FixtureMetadataValue::Unsigned(value) => Value::from(value),
            FixtureMetadataValue::Boolean(value) => Value::from(value),
        };
        fields.push((Value::from(field), value));
    }
    let mut kernel = vec![
        (Value::from(".name"), Value::from(options.entry)),
        (Value::from(".symbol"), Value::from(options.descriptor)),
        (Value::from(".args"), Value::Array(arguments)),
        (
            Value::from(".kernarg_segment_size"),
            Value::from(kernarg_segment_size(options)),
        ),
        (Value::from(".kernarg_segment_align"), Value::from(8)),
        (
            Value::from(".group_segment_fixed_size"),
            Value::from(options.group_segment_fixed_size),
        ),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (
            Value::from(".wavefront_size"),
            Value::from(options.wavefront_size),
        ),
        (Value::from(".sgpr_count"), Value::from(options.sgpr_count)),
        (Value::from(".vgpr_count"), Value::from(options.vgpr_count)),
        (
            Value::from(".max_flat_workgroup_size"),
            Value::from(options.max_flat_workgroup_size),
        ),
    ];
    for (field, value) in [
        (".agpr_count", options.agpr_count),
        (".sgpr_spill_count", options.sgpr_spill_count),
        (".vgpr_spill_count", options.vgpr_spill_count),
    ] {
        if let Some(value) = value {
            kernel.push((Value::from(field), Value::from(value)));
        }
    }
    if options.include_required_workgroup_size {
        kernel.push((
            Value::from(".reqd_workgroup_size"),
            Value::Array(
                options
                    .required_workgroup_size
                    .into_iter()
                    .map(Value::from)
                    .collect(),
            ),
        ));
    }
    for (field, maximum) in [
        (".max_num_workgroups_x", options.max_workgroups[0]),
        (".max_num_workgroups_y", options.max_workgroups[1]),
        (".max_num_workgroups_z", options.max_workgroups[2]),
    ] {
        if let Some(maximum) = maximum {
            kernel.push((Value::from(field), Value::from(maximum)));
        }
    }
    if let Some(dimensions) = options.cluster_dims {
        kernel.push((
            Value::from(".cluster_dims"),
            Value::Array(dimensions.into_iter().map(Value::from).collect()),
        ));
    }
    if let Some(kind) = options.kernel_kind {
        kernel.push((Value::from(".kind"), Value::from(kind)));
    }
    if let Some(uses_dynamic_stack) = options.uses_dynamic_stack {
        kernel.push((
            Value::from(".uses_dynamic_stack"),
            Value::from(uses_dynamic_stack),
        ));
    }
    if let Some(uniform_work_group_size) = options.uniform_work_group_size {
        kernel.push((
            Value::from(".uniform_work_group_size"),
            Value::from(uniform_work_group_size),
        ));
    }
    if let Some(workgroup_processor_mode) = options.workgroup_processor_mode {
        kernel.push((
            Value::from(".workgroup_processor_mode"),
            Value::from(workgroup_processor_mode),
        ));
    }
    if let Some(gfx1250_revision) = options.gfx1250_revision {
        kernel.push((
            Value::from(".gfx1250_revision"),
            Value::from(gfx1250_revision),
        ));
    }
    if let Some(device_enqueue_symbol) = options.device_enqueue_symbol {
        kernel.push((
            Value::from(".device_enqueue_symbol"),
            Value::from(device_enqueue_symbol),
        ));
    }
    if let Some(source_language) = options.source_language {
        kernel.push((Value::from(".language"), Value::from(source_language)));
    }
    if let Some(source_language_version) = options.source_language_version {
        kernel.push((
            Value::from(".language_version"),
            Value::Array(
                source_language_version
                    .into_iter()
                    .map(Value::from)
                    .collect(),
            ),
        ));
    }
    if options.include_workgroup_size_hint {
        kernel.push((
            Value::from(".workgroup_size_hint"),
            Value::Array(vec![Value::from(64), Value::from(1), Value::from(1)]),
        ));
    }
    if options.include_vector_type_hint {
        kernel.push((Value::from(".vec_type_hint"), Value::from("float")));
    }
    if options.duplicate_max_workgroups_x {
        kernel.push((Value::from(".max_num_workgroups_x"), Value::from(1)));
        kernel.push((Value::from(".max_num_workgroups_x"), Value::from(1)));
    }
    if options.malformed_max_workgroups_x {
        kernel.push((
            Value::from(".max_num_workgroups_x"),
            Value::from("not-an-integer"),
        ));
    }
    let kernel = Value::Map(kernel);
    let mut root = vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from(format!("amdgcn-amd-amdhsa--{}", options.target)),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(vec![kernel])),
    ];
    if options.include_printf_metadata {
        root.push((Value::from("amdhsa.printf"), Value::Array(Vec::new())));
    }
    let root = Value::Map(root);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
}

fn kernarg_segment_size(options: FixtureOptions<'_>) -> u64 {
    options
        .kernarg_segment_size_override
        .unwrap_or(match options.abi {
            FixtureAbi::SliceF32 => 272,
            FixtureAbi::ScalarAddV1 => 280,
            FixtureAbi::TiledGemmV1 => 320,
            FixtureAbi::RowSoftmaxV1 => 288,
        })
}

fn typed_explicit_argument(
    name: &str,
    offset: u64,
    size: u64,
    alignment: Option<u64>,
    value_type: &str,
) -> Value {
    let mut value = explicit_argument(Some(name), offset, size, alignment, "by_value", None);
    if let Value::Map(fields) = &mut value {
        fields.push((Value::from(".value_type"), Value::from(value_type)));
    }
    value
}

fn typed_explicit_pointer_argument(
    name: &str,
    offset: u64,
    alignment: Option<u64>,
    value_type: &str,
) -> Value {
    let mut value = explicit_pointer_argument(
        Some(name),
        offset,
        8,
        alignment,
        "global_buffer",
        Some("global"),
        None,
    );
    if let Value::Map(fields) = &mut value {
        fields.push((Value::from(".value_type"), Value::from(value_type)));
    }
    value
}

fn explicit_pointer_argument(
    name: Option<&str>,
    offset: u64,
    size: u64,
    alignment: Option<u64>,
    value_kind: &str,
    address_space: Option<&str>,
    pointee_alignment: Option<u64>,
) -> Value {
    let mut value = explicit_argument(name, offset, size, alignment, value_kind, address_space);
    if let (Some(pointee_alignment), Value::Map(fields)) = (pointee_alignment, &mut value) {
        fields.push((
            Value::from(".pointee_align"),
            Value::from(pointee_alignment),
        ));
    }
    value
}

fn explicit_argument(
    name: Option<&str>,
    offset: u64,
    size: u64,
    alignment: Option<u64>,
    value_kind: &str,
    address_space: Option<&str>,
) -> Value {
    let mut value = argument(name, offset, size, value_kind, address_space);
    if let (Some(alignment), Value::Map(fields)) = (alignment, &mut value) {
        fields.push((Value::from(".align"), Value::from(alignment)));
    }
    value
}

fn v5_hidden_arguments(base: u64) -> Vec<Value> {
    [
        (0, 4, "hidden_block_count_x"),
        (4, 4, "hidden_block_count_y"),
        (8, 4, "hidden_block_count_z"),
        (12, 2, "hidden_group_size_x"),
        (14, 2, "hidden_group_size_y"),
        (16, 2, "hidden_group_size_z"),
        (18, 2, "hidden_remainder_x"),
        (20, 2, "hidden_remainder_y"),
        (22, 2, "hidden_remainder_z"),
        (40, 8, "hidden_global_offset_x"),
        (48, 8, "hidden_global_offset_y"),
        (56, 8, "hidden_global_offset_z"),
        (64, 2, "hidden_grid_dims"),
    ]
    .into_iter()
    .map(|(offset, size, kind)| argument(None, base + offset, size, kind, None))
    .collect()
}

fn argument(
    name: Option<&str>,
    offset: u64,
    size: u64,
    value_kind: &str,
    address_space: Option<&str>,
) -> Value {
    let mut fields = vec![
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(size)),
        (Value::from(".value_kind"), Value::from(value_kind)),
    ];
    if let Some(name) = name {
        fields.push((Value::from(".name"), Value::from(name)));
    }
    if let Some(address_space) = address_space {
        fields.push((Value::from(".address_space"), Value::from(address_space)));
    }
    Value::Map(fields)
}

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

fn target_flags(target: &str) -> u32 {
    match target {
        "gfx942" => 0x54c,
        "gfx942:xnack-" => 0x64c,
        "gfx950" => 0x54f,
        _ => panic!("unsupported test target"),
    }
}

fn push_name(strings: &mut Vec<u8>, name: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend_from_slice(name.as_bytes());
    strings.push(0);
    offset
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let remainder = bytes.len() % alignment;
    if remainder != 0 {
        bytes.resize(bytes.len() + alignment - remainder, 0);
    }
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
