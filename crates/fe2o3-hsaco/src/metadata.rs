use fe2o3_amd_target::AmdTargetId;
use rmpv::ValueRef;

use crate::{
    ArgumentAccess, ArgumentAddressSpace, COV6_IMPLICIT_ARGUMENT_BYTES, CodeObjectVersion,
    ExplicitArgument, ExplicitValueKind, ExplicitValueType, Gfx1250Revision, HiddenArgument,
    HiddenValueKind, InspectedHsaco, InspectedKernel, InspectionError, KernelKind,
    MAX_ARGUMENTS_PER_KERNEL, MAX_KERNARG_BYTES, MAX_KERNELS, MetadataVersion,
    ParsedExplicitArgument, hidden_argument, inspected_hsaco, messagepack::decode_bounded,
};

const TARGET_PREFIX: &str = "amdgcn-amd-amdhsa--";
const MAX_ENTRY_NAME_BYTES: usize = 256;
const MAX_SEGMENT_BYTES: u64 = 0xffff_ffff;

type MapEntry<'data> = (ValueRef<'data>, ValueRef<'data>);

struct ParsedArguments {
    explicit: Vec<ExplicitArgument>,
    hidden: Vec<HiddenArgument>,
    implicit_offset: Option<u64>,
    implicit_size: u64,
}

pub(crate) fn inspect_metadata(
    code_object_version: CodeObjectVersion,
    e_flags: u32,
    bytes: &[u8],
) -> Result<InspectedHsaco, InspectionError> {
    let document = decode_bounded(bytes)?;
    let root = expect_map(&document, "metadata root")?;
    validate_root_keys(root)?;

    let metadata_version = parse_metadata_version(required(root, "amdhsa.version")?)?;
    validate_metadata_version(code_object_version, metadata_version)?;

    let target_text = expect_string(required(root, "amdhsa.target")?, "amdhsa.target")?;
    let target_id = target_text
        .strip_prefix(TARGET_PREFIX)
        .ok_or(InspectionError::InvalidTargetPrefix)?;
    let target = AmdTargetId::parse(target_id).map_err(|_| InspectionError::InvalidTargetId)?;
    if target.to_string() != target_id {
        return Err(InspectionError::NonCanonicalTargetId);
    }
    if target.amdhsa_elf_flags_v4_plus() != e_flags {
        return Err(InspectionError::TargetFlagsMismatch);
    }

    let has_printf_metadata = get(root, "amdhsa.printf").is_some();
    if let Some(printf) = get(root, "amdhsa.printf") {
        validate_string_array(printf, "amdhsa.printf")?;
    }

    let kernel_values = expect_array(required(root, "amdhsa.kernels")?, "amdhsa.kernels")?;
    if kernel_values.len() > MAX_KERNELS {
        return Err(InspectionError::TooManyKernels);
    }
    let mut kernels = Vec::with_capacity(kernel_values.len());
    for value in kernel_values {
        let kernel = parse_kernel(value, code_object_version, target)?;
        if kernels
            .iter()
            .any(|previous: &InspectedKernel| previous.name() == kernel.name())
        {
            return Err(InspectionError::DuplicateKernelName);
        }
        if kernels
            .iter()
            .any(|previous| previous.symbol() == kernel.symbol())
        {
            return Err(InspectionError::DuplicateKernelSymbol);
        }
        kernels.push(kernel);
    }

    Ok(inspected_hsaco(
        code_object_version,
        metadata_version,
        target,
        has_printf_metadata,
        kernels,
    ))
}

fn parse_metadata_version(value: &ValueRef<'_>) -> Result<MetadataVersion, InspectionError> {
    let version = expect_array(value, "amdhsa.version")?;
    if version.len() != 2 {
        return Err(InspectionError::InvalidFieldValue("amdhsa.version"));
    }
    let major = read_u32(&version[0], "amdhsa.version")?;
    let minor = read_u32(&version[1], "amdhsa.version")?;
    Ok(MetadataVersion::new(major, minor))
}

fn validate_metadata_version(
    code_object_version: CodeObjectVersion,
    version: MetadataVersion,
) -> Result<(), InspectionError> {
    let supported = matches!((version.major(), version.minor()), (1, 1) | (1, 2));
    if !supported {
        return Err(InspectionError::UnsupportedMetadataVersion);
    }
    let matches_code_object = match code_object_version {
        CodeObjectVersion::V4 => (version.major(), version.minor()) == (1, 1),
        CodeObjectVersion::V5 | CodeObjectVersion::V6 => {
            (version.major(), version.minor()) == (1, 2)
        }
    };
    if !matches_code_object {
        return Err(InspectionError::MetadataVersionMismatch);
    }
    Ok(())
}

fn validate_root_keys(map: &[MapEntry<'_>]) -> Result<(), InspectionError> {
    for (key, _) in map {
        let key = value_as_str(key).ok_or(InspectionError::NonStringMapKey)?;
        if !matches!(
            key,
            "amdhsa.version" | "amdhsa.target" | "amdhsa.printf" | "amdhsa.kernels"
        ) {
            return Err(InspectionError::UnknownRootField);
        }
    }
    Ok(())
}

fn parse_kernel(
    value: &ValueRef<'_>,
    code_object_version: CodeObjectVersion,
    target: AmdTargetId,
) -> Result<InspectedKernel, InspectionError> {
    let map = expect_map(value, "kernel")?;
    validate_kernel_keys(map, code_object_version)?;
    validate_unpreserved_kernel_fields(map)?;

    let name = parse_entry_name(required(map, ".name")?, ".name")?;
    let symbol = parse_entry_name(required(map, ".symbol")?, ".symbol")?;
    let kernarg_segment_size = read_u64(
        required(map, ".kernarg_segment_size")?,
        ".kernarg_segment_size",
    )?;
    if kernarg_segment_size > MAX_KERNARG_BYTES || !kernarg_segment_size.is_multiple_of(4) {
        return Err(InspectionError::InvalidFieldValue(".kernarg_segment_size"));
    }
    let kernarg_segment_alignment = read_u64(
        required(map, ".kernarg_segment_align")?,
        ".kernarg_segment_align",
    )?;
    validate_alignment(kernarg_segment_alignment, ".kernarg_segment_align")?;

    let group_segment_fixed_size = read_bounded_segment_size(
        required(map, ".group_segment_fixed_size")?,
        ".group_segment_fixed_size",
    )?;
    let private_segment_fixed_size = read_bounded_segment_size(
        required(map, ".private_segment_fixed_size")?,
        ".private_segment_fixed_size",
    )?;
    let wavefront_size = read_u32(required(map, ".wavefront_size")?, ".wavefront_size")?;
    if !matches!(wavefront_size, 32 | 64) {
        return Err(InspectionError::InvalidFieldValue(".wavefront_size"));
    }

    let sgpr_count = read_u16(required(map, ".sgpr_count")?, ".sgpr_count")?;
    let vgpr_count = read_u16(required(map, ".vgpr_count")?, ".vgpr_count")?;
    let agpr_count = optional_u32(map, ".agpr_count")?;
    let sgpr_spill_count = optional_u32(map, ".sgpr_spill_count")?;
    let vgpr_spill_count = optional_u32(map, ".vgpr_spill_count")?;
    let max_flat_workgroup_size = read_u32(
        required(map, ".max_flat_workgroup_size")?,
        ".max_flat_workgroup_size",
    )?;
    if max_flat_workgroup_size == 0 {
        return Err(InspectionError::InvalidFieldValue(
            ".max_flat_workgroup_size",
        ));
    }

    let required_workgroup_size = parse_required_workgroup_size(map, max_flat_workgroup_size)?;
    let max_workgroups = [
        parse_max_workgroups_axis(map, ".max_num_workgroups_x", ".max_num_work_groups_x")?,
        parse_max_workgroups_axis(map, ".max_num_workgroups_y", ".max_num_work_groups_y")?,
        parse_max_workgroups_axis(map, ".max_num_workgroups_z", ".max_num_work_groups_z")?,
    ];
    let cluster_dims = parse_cluster_dims(map)?;
    let kind = match optional_string(map, ".kind")? {
        None | Some("normal") => KernelKind::Normal,
        Some("init") => KernelKind::Init,
        Some("fini") => KernelKind::Fini,
        Some(_) => return Err(InspectionError::InvalidFieldValue(".kind")),
    };
    let uniform_work_group_size = match optional_u64(map, ".uniform_work_group_size")? {
        None | Some(0) => false,
        Some(1) => true,
        Some(_) => {
            return Err(InspectionError::InvalidFieldValue(
                ".uniform_work_group_size",
            ));
        }
    };
    let uses_dynamic_stack = optional_boolean(map, ".uses_dynamic_stack")?.unwrap_or(false);
    let workgroup_processor_mode = optional_boolean_or_flag(map, ".workgroup_processor_mode")?;
    let gfx1250_revision = parse_gfx1250_revision(map, target)?;
    let device_enqueue_symbol = optional_string(map, ".device_enqueue_symbol")?
        .map(|symbol| parse_entry_name_text(symbol, ".device_enqueue_symbol"))
        .transpose()?;

    let argument_values = match get(map, ".args") {
        Some(value) => expect_array(value, ".args")?,
        None => &[],
    };
    if argument_values.len() > MAX_ARGUMENTS_PER_KERNEL {
        return Err(InspectionError::TooManyArguments);
    }
    let ParsedArguments {
        explicit: explicit_arguments,
        hidden: hidden_arguments,
        implicit_offset: implicit_argument_offset,
        implicit_size: implicit_argument_size,
    } = parse_arguments(argument_values, kernarg_segment_size, code_object_version)?;

    Ok(InspectedKernel {
        name,
        symbol,
        kernarg_segment_size,
        kernarg_segment_alignment,
        group_segment_fixed_size,
        private_segment_fixed_size,
        wavefront_size,
        sgpr_count,
        vgpr_count,
        agpr_count,
        sgpr_spill_count,
        vgpr_spill_count,
        max_flat_workgroup_size,
        required_workgroup_size,
        max_workgroups,
        cluster_dims,
        kind,
        uniform_work_group_size,
        uses_dynamic_stack,
        workgroup_processor_mode,
        gfx1250_revision,
        device_enqueue_symbol,
        implicit_argument_offset,
        implicit_argument_size,
        explicit_arguments,
        hidden_arguments,
    })
}

fn parse_arguments(
    values: &[ValueRef<'_>],
    kernarg_segment_size: u64,
    code_object_version: CodeObjectVersion,
) -> Result<ParsedArguments, InspectionError> {
    let mut explicit_arguments = Vec::new();
    let mut hidden_arguments = Vec::new();
    let mut previous_offset = None;
    let mut previous_end = 0u64;
    let mut last_explicit_end = 0u64;
    let mut hidden_seen = false;

    for value in values {
        let map = expect_map(value, "kernel argument")?;
        validate_argument_fields(map)?;
        let offset = read_u64(required(map, ".offset")?, ".offset")?;
        let size = read_u64(required(map, ".size")?, ".size")?;
        let end = offset
            .checked_add(size)
            .ok_or(InspectionError::InvalidArgumentRange)?;
        if size == 0 || end > kernarg_segment_size {
            return Err(InspectionError::InvalidArgumentRange);
        }
        if previous_offset.is_some_and(|previous| offset < previous) {
            return Err(InspectionError::ArgumentsOutOfOrder);
        }
        if offset < previous_end {
            return Err(InspectionError::OverlappingArguments);
        }
        previous_offset = Some(offset);
        previous_end = end;

        let alignment = optional_u64(map, ".align")?;
        if let Some(alignment) = alignment {
            validate_alignment(alignment, ".align")?;
            if !offset.is_multiple_of(alignment) {
                return Err(InspectionError::InvalidFieldValue(".align"));
            }
        }
        let argument_name = optional_string(map, ".name")?;
        let type_name = optional_string(map, ".type_name")?.map(Box::<str>::from);
        let pointee_alignment = optional_u64(map, ".pointee_align")?;
        if let Some(alignment) = pointee_alignment {
            validate_alignment(alignment, ".pointee_align")?;
        }
        let address_space = optional_address_space(map)?;
        let access = optional_access(map, ".access")?;
        let actual_access = optional_access(map, ".actual_access")?;
        let is_const = optional_boolean(map, ".is_const")?;
        let is_restrict = optional_boolean(map, ".is_restrict")?;
        let is_volatile = optional_boolean(map, ".is_volatile")?;
        let is_pipe = optional_boolean(map, ".is_pipe")?;
        let value_type = optional_value_type(map)?;
        let value_kind = expect_string(required(map, ".value_kind")?, ".value_kind")?;
        match parse_value_kind(value_kind, code_object_version)? {
            ParsedValueKind::Explicit(value_kind) => {
                if hidden_seen {
                    return Err(InspectionError::ExplicitArgumentAfterHidden);
                }
                let name = argument_name
                    .map(|name| validate_argument_name(name).map(Box::<str>::from))
                    .transpose()?;
                if let Some(name) = name.as_deref()
                    && explicit_arguments
                        .iter()
                        .any(|argument: &ExplicitArgument| argument.name() == Some(name))
                {
                    return Err(InspectionError::DuplicateArgumentName);
                }
                let argument = ParsedExplicitArgument {
                    name,
                    type_name,
                    offset,
                    size,
                    alignment,
                    value_kind,
                    value_type,
                    address_space,
                    access,
                    actual_access,
                    pointee_alignment,
                    is_const,
                    is_restrict,
                    is_volatile,
                    is_pipe,
                };
                explicit_arguments.push(argument.into());
                last_explicit_end = end;
            }
            ParsedValueKind::Hidden(value_kind) => {
                if has_explicit_only_qualifier(map) {
                    return Err(InspectionError::ExplicitQualifierOnHiddenArgument);
                }
                hidden_seen = true;
                hidden_arguments.push(hidden_argument(offset, size, value_kind));
            }
        }
    }

    let implicit_base = align_up(last_explicit_end, 8)?;
    let explicit_segment_size = align_up(last_explicit_end, 4)?;
    let (implicit_argument_offset, implicit_argument_size) = if let Some(first_hidden) =
        hidden_arguments.first()
    {
        let expected_base = implicit_base;
        if first_hidden.offset() != expected_base {
            return Err(InspectionError::InvalidImplicitArgumentSpan);
        }
        let size = kernarg_segment_size
            .checked_sub(expected_base)
            .ok_or(InspectionError::InvalidImplicitArgumentSpan)?;
        (Some(expected_base), size)
    } else if kernarg_segment_size == explicit_segment_size {
        (None, 0)
    } else if code_object_version == CodeObjectVersion::V4 && kernarg_segment_size > implicit_base {
        let size = kernarg_segment_size
            .checked_sub(implicit_base)
            .ok_or(InspectionError::InvalidImplicitArgumentSpan)?;
        (Some(implicit_base), size)
    } else {
        return Err(InspectionError::InvalidImplicitArgumentSpan);
    };

    if code_object_version == CodeObjectVersion::V6
        && !hidden_arguments.is_empty()
        && implicit_argument_size != COV6_IMPLICIT_ARGUMENT_BYTES
    {
        return Err(InspectionError::InvalidImplicitArgumentSpan);
    }
    validate_hidden_argument_layout(
        &hidden_arguments,
        code_object_version,
        implicit_argument_size,
    )?;

    Ok(ParsedArguments {
        explicit: explicit_arguments,
        hidden: hidden_arguments,
        implicit_offset: implicit_argument_offset,
        implicit_size: implicit_argument_size,
    })
}

fn validate_kernel_keys(
    map: &[MapEntry<'_>],
    code_object_version: CodeObjectVersion,
) -> Result<(), InspectionError> {
    for (key, _) in map {
        let key = value_as_str(key).ok_or(InspectionError::NonStringMapKey)?;
        let version_requirement = match key {
            ".name"
            | ".symbol"
            | ".language"
            | ".language_version"
            | ".args"
            | ".reqd_workgroup_size"
            | ".workgroup_size_hint"
            | ".vec_type_hint"
            | ".device_enqueue_symbol"
            | ".kernarg_segment_size"
            | ".group_segment_fixed_size"
            | ".private_segment_fixed_size"
            | ".kernarg_segment_align"
            | ".wavefront_size"
            | ".sgpr_count"
            | ".vgpr_count"
            | ".agpr_count"
            | ".max_flat_workgroup_size"
            | ".sgpr_spill_count"
            | ".vgpr_spill_count"
            | ".kind"
            | ".max_num_workgroups_x"
            | ".max_num_workgroups_y"
            | ".max_num_workgroups_z"
            | ".max_num_work_groups_x"
            | ".max_num_work_groups_y"
            | ".max_num_work_groups_z" => None,
            ".uses_dynamic_stack"
            | ".workgroup_processor_mode"
            | ".uniform_work_group_size"
            | ".gfx1250_revision" => Some(CodeObjectVersion::V5),
            ".cluster_dims" => Some(CodeObjectVersion::V6),
            _ => return Err(InspectionError::UnknownKernelField),
        };
        if matches!(version_requirement, Some(CodeObjectVersion::V5))
            && code_object_version == CodeObjectVersion::V4
            || matches!(version_requirement, Some(CodeObjectVersion::V6))
                && code_object_version != CodeObjectVersion::V6
        {
            return Err(InspectionError::UnsupportedFieldForCodeObjectVersion(
                match key {
                    ".uses_dynamic_stack" => ".uses_dynamic_stack",
                    ".workgroup_processor_mode" => ".workgroup_processor_mode",
                    ".uniform_work_group_size" => ".uniform_work_group_size",
                    ".gfx1250_revision" => ".gfx1250_revision",
                    ".cluster_dims" => ".cluster_dims",
                    _ => unreachable!(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_unpreserved_kernel_fields(map: &[MapEntry<'_>]) -> Result<(), InspectionError> {
    // Source-language and optimization hints do not describe executable
    // resource use or launch behavior, so inspection syntax-checks but does
    // not retain them as authority-bearing evidence.
    optional_string(map, ".language")?;
    optional_string(map, ".vec_type_hint")?;
    for field in [".language_version", ".workgroup_size_hint"] {
        if let Some(value) = get(map, field) {
            validate_integer_array(
                value,
                field,
                Some(if field == ".language_version" { 2 } else { 3 }),
            )?;
        }
    }
    Ok(())
}

fn parse_gfx1250_revision(
    map: &[MapEntry<'_>],
    target: AmdTargetId,
) -> Result<Option<Gfx1250Revision>, InspectionError> {
    let Some(value) = optional_string(map, ".gfx1250_revision")? else {
        return Ok(None);
    };
    let revision = match value {
        "A0" => Gfx1250Revision::A0,
        "B0" => Gfx1250Revision::B0,
        _ => return Err(InspectionError::InvalidFieldValue(".gfx1250_revision")),
    };

    // Pinned to GCNSubtarget.cpp/.h and AMDGPUHSAMetadataStreamer.cpp at LLVM
    // revision 846473237377990d00b9c353f6a2c86116b52ea5. A0 requires the
    // GFX1250 instruction family (gfx1250 or gfx1251). The revision's temporary
    // EnableGFX1250B0Specific switch globally enables B0 before emission, so
    // B0 cannot be narrowed by processor without rejecting that LLVM's output.
    if revision == Gfx1250Revision::A0 && !matches!(target.processor(), "gfx1250" | "gfx1251") {
        return Err(InspectionError::InvalidFieldValue(".gfx1250_revision"));
    }
    Ok(Some(revision))
}

fn parse_required_workgroup_size(
    map: &[MapEntry<'_>],
    max_flat_workgroup_size: u32,
) -> Result<Option<[u32; 3]>, InspectionError> {
    let Some(value) = get(map, ".reqd_workgroup_size") else {
        return Ok(None);
    };
    let dims = read_u32_triplet(value, ".reqd_workgroup_size")?;
    if dims == [0, 0, 0] {
        return Ok(None);
    }
    if dims.contains(&0)
        || flat_product(dims, ".reqd_workgroup_size")? > u64::from(max_flat_workgroup_size)
    {
        return Err(InspectionError::InvalidFieldValue(".reqd_workgroup_size"));
    }
    Ok(Some(dims))
}

fn parse_max_workgroups_axis(
    map: &[MapEntry<'_>],
    emitted_key: &'static str,
    documented_key: &'static str,
) -> Result<Option<u32>, InspectionError> {
    let emitted = get(map, emitted_key);
    let documented = get(map, documented_key);
    if emitted.is_some() && documented.is_some() {
        return Err(InspectionError::ConflictingFieldAliases(emitted_key));
    }
    let Some(value) = emitted.or(documented) else {
        return Ok(None);
    };
    let limit = read_u32(value, emitted_key)?;
    if limit == 0 {
        return Err(InspectionError::InvalidFieldValue(emitted_key));
    }
    Ok(Some(limit))
}

fn parse_cluster_dims(map: &[MapEntry<'_>]) -> Result<Option<[u32; 3]>, InspectionError> {
    let Some(value) = get(map, ".cluster_dims") else {
        return Ok(None);
    };
    let dims = read_u32_triplet(value, ".cluster_dims")?;
    // Pinned to ClusterDimsAttr and the V6 metadata streamer at LLVM revision
    // 846473237377990d00b9c353f6a2c86116b52ea5. The streamer emits only fixed
    // dimensions, not the all-zero no-cluster or all-1024 variable sentinels.
    // Hardware capability limits are intentionally left to launch authority.
    if dims == [0, 0, 0] || dims == [1024, 1024, 1024] {
        return Err(InspectionError::InvalidFieldValue(".cluster_dims"));
    }
    Ok(Some(dims))
}

fn read_u32_triplet(
    value: &ValueRef<'_>,
    field: &'static str,
) -> Result<[u32; 3], InspectionError> {
    let values = expect_array(value, field)?;
    if values.len() != 3 {
        return Err(InspectionError::InvalidFieldValue(field));
    }
    Ok([
        read_u32(&values[0], field)?,
        read_u32(&values[1], field)?,
        read_u32(&values[2], field)?,
    ])
}

fn flat_product(dims: [u32; 3], field: &'static str) -> Result<u64, InspectionError> {
    dims.into_iter().try_fold(1u64, |product, dimension| {
        product
            .checked_mul(u64::from(dimension))
            .ok_or(InspectionError::InvalidFieldValue(field))
    })
}

fn validate_argument_fields(map: &[MapEntry<'_>]) -> Result<(), InspectionError> {
    for (key, _) in map {
        let key = value_as_str(key).ok_or(InspectionError::NonStringMapKey)?;
        if !matches!(
            key,
            ".name"
                | ".type_name"
                | ".value_type"
                | ".size"
                | ".offset"
                | ".align"
                | ".value_kind"
                | ".pointee_align"
                | ".address_space"
                | ".access"
                | ".actual_access"
                | ".is_const"
                | ".is_restrict"
                | ".is_volatile"
                | ".is_pipe"
        ) {
            return Err(InspectionError::UnknownArgumentField);
        }
    }
    optional_string(map, ".name")?;
    optional_string(map, ".type_name")?;
    optional_value_type(map)?;
    for field in [".is_const", ".is_restrict", ".is_volatile", ".is_pipe"] {
        optional_boolean(map, field)?;
    }
    Ok(())
}

fn parse_entry_name(
    value: &ValueRef<'_>,
    field: &'static str,
) -> Result<Box<str>, InspectionError> {
    parse_entry_name_text(expect_string(value, field)?, field)
}

fn parse_entry_name_text(name: &str, field: &'static str) -> Result<Box<str>, InspectionError> {
    if name.is_empty() || name.len() > MAX_ENTRY_NAME_BYTES || name.contains('\0') {
        return Err(InspectionError::InvalidFieldValue(field));
    }
    Ok(name.into())
}

fn validate_argument_name(name: &str) -> Result<&str, InspectionError> {
    if name.is_empty() || name.len() > MAX_ENTRY_NAME_BYTES || name.contains('\0') {
        return Err(InspectionError::InvalidFieldValue(".name"));
    }
    Ok(name)
}

fn read_bounded_segment_size(
    value: &ValueRef<'_>,
    field: &'static str,
) -> Result<u64, InspectionError> {
    let size = read_u64(value, field)?;
    if size > MAX_SEGMENT_BYTES {
        return Err(InspectionError::InvalidFieldValue(field));
    }
    Ok(size)
}

fn validate_alignment(alignment: u64, field: &'static str) -> Result<(), InspectionError> {
    if alignment == 0 || !alignment.is_power_of_two() || alignment > MAX_KERNARG_BYTES {
        return Err(InspectionError::InvalidFieldValue(field));
    }
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64, InspectionError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|rounded| rounded & !mask)
        .ok_or(InspectionError::InvalidImplicitArgumentSpan)
}

enum ParsedValueKind {
    Explicit(ExplicitValueKind),
    Hidden(HiddenValueKind),
}

fn parse_value_kind(
    value: &str,
    code_object_version: CodeObjectVersion,
) -> Result<ParsedValueKind, InspectionError> {
    let kind = match value {
        "by_value" => ParsedValueKind::Explicit(ExplicitValueKind::ByValue),
        "global_buffer" => ParsedValueKind::Explicit(ExplicitValueKind::GlobalBuffer),
        "dynamic_shared_pointer" => {
            ParsedValueKind::Explicit(ExplicitValueKind::DynamicSharedPointer)
        }
        "sampler" => ParsedValueKind::Explicit(ExplicitValueKind::Sampler),
        "image" => ParsedValueKind::Explicit(ExplicitValueKind::Image),
        "pipe" => ParsedValueKind::Explicit(ExplicitValueKind::Pipe),
        "queue" => ParsedValueKind::Explicit(ExplicitValueKind::Queue),
        "hidden_block_count_x" => ParsedValueKind::Hidden(HiddenValueKind::BlockCountX),
        "hidden_block_count_y" => ParsedValueKind::Hidden(HiddenValueKind::BlockCountY),
        "hidden_block_count_z" => ParsedValueKind::Hidden(HiddenValueKind::BlockCountZ),
        "hidden_group_size_x" => ParsedValueKind::Hidden(HiddenValueKind::GroupSizeX),
        "hidden_group_size_y" => ParsedValueKind::Hidden(HiddenValueKind::GroupSizeY),
        "hidden_group_size_z" => ParsedValueKind::Hidden(HiddenValueKind::GroupSizeZ),
        "hidden_remainder_x" => ParsedValueKind::Hidden(HiddenValueKind::RemainderX),
        "hidden_remainder_y" => ParsedValueKind::Hidden(HiddenValueKind::RemainderY),
        "hidden_remainder_z" => ParsedValueKind::Hidden(HiddenValueKind::RemainderZ),
        "hidden_global_offset_x" => ParsedValueKind::Hidden(HiddenValueKind::GlobalOffsetX),
        "hidden_global_offset_y" => ParsedValueKind::Hidden(HiddenValueKind::GlobalOffsetY),
        "hidden_global_offset_z" => ParsedValueKind::Hidden(HiddenValueKind::GlobalOffsetZ),
        "hidden_grid_dims" => ParsedValueKind::Hidden(HiddenValueKind::GridDimensions),
        "hidden_none" => ParsedValueKind::Hidden(HiddenValueKind::None),
        "hidden_printf_buffer" => ParsedValueKind::Hidden(HiddenValueKind::PrintfBuffer),
        "hidden_hostcall_buffer" => ParsedValueKind::Hidden(HiddenValueKind::HostcallBuffer),
        "hidden_heap_v1" => ParsedValueKind::Hidden(HiddenValueKind::HeapV1),
        "hidden_default_queue" => ParsedValueKind::Hidden(HiddenValueKind::DefaultQueue),
        "hidden_completion_action" => ParsedValueKind::Hidden(HiddenValueKind::CompletionAction),
        "hidden_multigrid_sync_arg" => {
            ParsedValueKind::Hidden(HiddenValueKind::MultigridSyncArgument)
        }
        "hidden_dynamic_lds_size" => ParsedValueKind::Hidden(HiddenValueKind::DynamicLdsSize),
        "hidden_private_base" => ParsedValueKind::Hidden(HiddenValueKind::PrivateBase),
        "hidden_shared_base" => ParsedValueKind::Hidden(HiddenValueKind::SharedBase),
        "hidden_queue_ptr" => ParsedValueKind::Hidden(HiddenValueKind::QueuePointer),
        _ => return Err(InspectionError::UnknownValueKind),
    };
    if code_object_version == CodeObjectVersion::V4
        && matches!(
            kind,
            ParsedValueKind::Hidden(
                HiddenValueKind::BlockCountX
                    | HiddenValueKind::BlockCountY
                    | HiddenValueKind::BlockCountZ
                    | HiddenValueKind::GroupSizeX
                    | HiddenValueKind::GroupSizeY
                    | HiddenValueKind::GroupSizeZ
                    | HiddenValueKind::RemainderX
                    | HiddenValueKind::RemainderY
                    | HiddenValueKind::RemainderZ
                    | HiddenValueKind::GridDimensions
                    | HiddenValueKind::HeapV1
                    | HiddenValueKind::DynamicLdsSize
                    | HiddenValueKind::PrivateBase
                    | HiddenValueKind::SharedBase
                    | HiddenValueKind::QueuePointer
            )
        )
    {
        return Err(InspectionError::UnsupportedValueKindForCodeObjectVersion);
    }
    Ok(kind)
}

fn has_explicit_only_qualifier(map: &[MapEntry<'_>]) -> bool {
    [
        ".name",
        ".type_name",
        ".value_type",
        ".align",
        ".pointee_align",
        ".address_space",
        ".access",
        ".actual_access",
        ".is_const",
        ".is_restrict",
        ".is_volatile",
        ".is_pipe",
    ]
    .into_iter()
    .any(|field| get(map, field).is_some())
}

fn validate_hidden_argument_layout(
    arguments: &[HiddenArgument],
    code_object_version: CodeObjectVersion,
    implicit_argument_size: u64,
) -> Result<(), InspectionError> {
    if let Some(first) = arguments.first() {
        let base = first.offset();
        for argument in arguments {
            let relative_end = argument
                .offset()
                .checked_sub(base)
                .and_then(|offset| offset.checked_add(argument.size()))
                .ok_or(InspectionError::InvalidImplicitArgumentSpan)?;
            if relative_end > implicit_argument_size {
                return Err(InspectionError::InvalidImplicitArgumentSpan);
            }
        }
    }
    match code_object_version {
        CodeObjectVersion::V4 => {
            validate_v4_hidden_argument_layout(arguments, implicit_argument_size)
        }
        CodeObjectVersion::V5 | CodeObjectVersion::V6 => {
            if arguments.is_empty() {
                Ok(())
            } else {
                validate_v5_hidden_argument_layout(arguments)
            }
        }
    }
}

fn validate_v4_hidden_argument_layout(
    arguments: &[HiddenArgument],
    implicit_argument_size: u64,
) -> Result<(), InspectionError> {
    let record_count = arguments.len();
    let minimum_size = u64::try_from(record_count)
        .map_err(|_| InspectionError::InvalidHiddenArgumentLayout)?
        .checked_mul(8)
        .ok_or(InspectionError::InvalidHiddenArgumentLayout)?;
    let maximum_size = if record_count < 7 {
        Some(
            minimum_size
                .checked_add(8)
                .ok_or(InspectionError::InvalidHiddenArgumentLayout)?,
        )
    } else {
        None
    };
    if implicit_argument_size < minimum_size
        || maximum_size.is_some_and(|maximum| implicit_argument_size > maximum)
    {
        return Err(InspectionError::InvalidHiddenArgumentLayout);
    }
    let Some(first) = arguments.first() else {
        return Ok(());
    };
    if !first.offset().is_multiple_of(8) {
        return Err(InspectionError::InvalidHiddenArgumentLayout);
    }
    let base = first.offset();
    for (index, argument) in arguments.iter().copied().enumerate() {
        let expected_offset = base
            .checked_add(
                u64::try_from(index).map_err(|_| InspectionError::InvalidHiddenArgumentLayout)? * 8,
            )
            .ok_or(InspectionError::InvalidHiddenArgumentLayout)?;
        let kind_is_valid = match index {
            0 => argument.value_kind() == HiddenValueKind::GlobalOffsetX,
            1 => argument.value_kind() == HiddenValueKind::GlobalOffsetY,
            2 => argument.value_kind() == HiddenValueKind::GlobalOffsetZ,
            3 => matches!(
                argument.value_kind(),
                HiddenValueKind::PrintfBuffer
                    | HiddenValueKind::HostcallBuffer
                    | HiddenValueKind::None
            ),
            4 => matches!(
                argument.value_kind(),
                HiddenValueKind::DefaultQueue | HiddenValueKind::None
            ),
            5 => matches!(
                argument.value_kind(),
                HiddenValueKind::CompletionAction | HiddenValueKind::None
            ),
            6 => matches!(
                argument.value_kind(),
                HiddenValueKind::MultigridSyncArgument | HiddenValueKind::None
            ),
            _ => false,
        };
        if argument.offset() != expected_offset || argument.size() != 8 || !kind_is_valid {
            return Err(InspectionError::InvalidHiddenArgumentLayout);
        }
    }
    Ok(())
}

fn validate_v5_hidden_argument_layout(arguments: &[HiddenArgument]) -> Result<(), InspectionError> {
    const REQUIRED: &[(u64, u64, HiddenValueKind)] = &[
        (0, 4, HiddenValueKind::BlockCountX),
        (4, 4, HiddenValueKind::BlockCountY),
        (8, 4, HiddenValueKind::BlockCountZ),
        (12, 2, HiddenValueKind::GroupSizeX),
        (14, 2, HiddenValueKind::GroupSizeY),
        (16, 2, HiddenValueKind::GroupSizeZ),
        (18, 2, HiddenValueKind::RemainderX),
        (20, 2, HiddenValueKind::RemainderY),
        (22, 2, HiddenValueKind::RemainderZ),
        (40, 8, HiddenValueKind::GlobalOffsetX),
        (48, 8, HiddenValueKind::GlobalOffsetY),
        (56, 8, HiddenValueKind::GlobalOffsetZ),
        (64, 2, HiddenValueKind::GridDimensions),
    ];
    const OPTIONAL: &[(u64, u64, HiddenValueKind)] = &[
        (72, 8, HiddenValueKind::PrintfBuffer),
        (80, 8, HiddenValueKind::HostcallBuffer),
        (88, 8, HiddenValueKind::MultigridSyncArgument),
        (96, 8, HiddenValueKind::HeapV1),
        (104, 8, HiddenValueKind::DefaultQueue),
        (112, 8, HiddenValueKind::CompletionAction),
        (120, 4, HiddenValueKind::DynamicLdsSize),
        (192, 4, HiddenValueKind::PrivateBase),
        (196, 4, HiddenValueKind::SharedBase),
        (200, 8, HiddenValueKind::QueuePointer),
    ];

    if arguments.len() < REQUIRED.len() || !arguments[0].offset().is_multiple_of(8) {
        return Err(InspectionError::InvalidHiddenArgumentLayout);
    }
    let base = arguments[0].offset();
    for (argument, expected) in arguments.iter().copied().zip(REQUIRED.iter().copied()) {
        validate_hidden_argument_at(argument, base, expected)?;
    }
    for argument in arguments.iter().copied().skip(REQUIRED.len()) {
        let relative_offset = argument
            .offset()
            .checked_sub(base)
            .ok_or(InspectionError::InvalidHiddenArgumentLayout)?;
        let expected = OPTIONAL
            .iter()
            .copied()
            .find(|(offset, _, _)| *offset == relative_offset)
            .ok_or(InspectionError::InvalidHiddenArgumentLayout)?;
        validate_hidden_argument_at(argument, base, expected)?;
    }
    Ok(())
}

fn validate_hidden_argument_at(
    argument: HiddenArgument,
    base: u64,
    expected: (u64, u64, HiddenValueKind),
) -> Result<(), InspectionError> {
    let expected_offset = base
        .checked_add(expected.0)
        .ok_or(InspectionError::InvalidHiddenArgumentLayout)?;
    if argument.offset() != expected_offset
        || argument.size() != expected.1
        || argument.value_kind() != expected.2
    {
        return Err(InspectionError::InvalidHiddenArgumentLayout);
    }
    Ok(())
}

fn optional_address_space(
    map: &[MapEntry<'_>],
) -> Result<Option<ArgumentAddressSpace>, InspectionError> {
    let Some(value) = optional_string(map, ".address_space")? else {
        return Ok(None);
    };
    let address_space = match value {
        "private" => ArgumentAddressSpace::Private,
        "global" => ArgumentAddressSpace::Global,
        "constant" => ArgumentAddressSpace::Constant,
        "local" => ArgumentAddressSpace::Local,
        "generic" => ArgumentAddressSpace::Generic,
        "region" => ArgumentAddressSpace::Region,
        _ => return Err(InspectionError::UnknownAddressSpace),
    };
    Ok(Some(address_space))
}

fn optional_value_type(map: &[MapEntry<'_>]) -> Result<Option<ExplicitValueType>, InspectionError> {
    let Some(value) = optional_string(map, ".value_type")? else {
        return Ok(None);
    };
    let value_type = match value {
        "struct" => ExplicitValueType::Struct,
        "i8" => ExplicitValueType::I8,
        "u8" => ExplicitValueType::U8,
        "i16" => ExplicitValueType::I16,
        "u16" => ExplicitValueType::U16,
        "f16" => ExplicitValueType::F16,
        "i32" => ExplicitValueType::I32,
        "u32" => ExplicitValueType::U32,
        "f32" => ExplicitValueType::F32,
        "i64" => ExplicitValueType::I64,
        "u64" => ExplicitValueType::U64,
        "f64" => ExplicitValueType::F64,
        _ => return Err(InspectionError::UnknownValueType),
    };
    Ok(Some(value_type))
}

fn optional_access(
    map: &[MapEntry<'_>],
    field: &'static str,
) -> Result<Option<ArgumentAccess>, InspectionError> {
    let Some(value) = optional_string(map, field)? else {
        return Ok(None);
    };
    let access = match value {
        "read_only" => ArgumentAccess::ReadOnly,
        "write_only" => ArgumentAccess::WriteOnly,
        "read_write" => ArgumentAccess::ReadWrite,
        _ => return Err(InspectionError::UnknownAccess),
    };
    Ok(Some(access))
}

fn required<'value, 'data>(
    map: &'value [MapEntry<'data>],
    key: &'static str,
) -> Result<&'value ValueRef<'data>, InspectionError> {
    get(map, key).ok_or(InspectionError::MissingField(key))
}

fn get<'value, 'data>(
    map: &'value [MapEntry<'data>],
    key: &str,
) -> Option<&'value ValueRef<'data>> {
    map.iter()
        .find(|(candidate, _)| value_as_str(candidate) == Some(key))
        .map(|(_, value)| value)
}

fn expect_map<'value, 'data>(
    value: &'value ValueRef<'data>,
    field: &'static str,
) -> Result<&'value [MapEntry<'data>], InspectionError> {
    match value {
        ValueRef::Map(map) => Ok(map),
        _ => Err(InspectionError::InvalidFieldType(field)),
    }
}

fn expect_array<'value, 'data>(
    value: &'value ValueRef<'data>,
    field: &'static str,
) -> Result<&'value [ValueRef<'data>], InspectionError> {
    match value {
        ValueRef::Array(array) => Ok(array),
        _ => Err(InspectionError::InvalidFieldType(field)),
    }
}

fn expect_string<'value>(
    value: &'value ValueRef<'_>,
    field: &'static str,
) -> Result<&'value str, InspectionError> {
    value_as_str(value).ok_or(InspectionError::InvalidFieldType(field))
}

fn value_as_str<'value>(value: &'value ValueRef<'_>) -> Option<&'value str> {
    match value {
        ValueRef::String(value) => value.as_str(),
        _ => None,
    }
}

fn optional_string<'value, 'data>(
    map: &'value [MapEntry<'data>],
    field: &'static str,
) -> Result<Option<&'value str>, InspectionError> {
    get(map, field)
        .map(|value| expect_string(value, field))
        .transpose()
}

fn optional_boolean(
    map: &[MapEntry<'_>],
    field: &'static str,
) -> Result<Option<bool>, InspectionError> {
    get(map, field)
        .map(|value| match value {
            ValueRef::Boolean(value) => Ok(*value),
            _ => Err(InspectionError::InvalidFieldType(field)),
        })
        .transpose()
}

fn optional_boolean_or_flag(
    map: &[MapEntry<'_>],
    field: &'static str,
) -> Result<Option<bool>, InspectionError> {
    get(map, field)
        .map(|value| match value {
            ValueRef::Boolean(value) => Ok(*value),
            _ => match read_u64(value, field)? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(InspectionError::InvalidFieldValue(field)),
            },
        })
        .transpose()
}

fn read_u64(value: &ValueRef<'_>, field: &'static str) -> Result<u64, InspectionError> {
    value
        .as_u64()
        .ok_or(InspectionError::InvalidFieldType(field))
}

fn read_u32(value: &ValueRef<'_>, field: &'static str) -> Result<u32, InspectionError> {
    u32::try_from(read_u64(value, field)?).map_err(|_| InspectionError::InvalidFieldValue(field))
}

fn read_u16(value: &ValueRef<'_>, field: &'static str) -> Result<u16, InspectionError> {
    u16::try_from(read_u64(value, field)?).map_err(|_| InspectionError::InvalidFieldValue(field))
}

fn optional_u64(map: &[MapEntry<'_>], field: &'static str) -> Result<Option<u64>, InspectionError> {
    get(map, field)
        .map(|value| read_u64(value, field))
        .transpose()
}

fn optional_u32(map: &[MapEntry<'_>], field: &'static str) -> Result<Option<u32>, InspectionError> {
    get(map, field)
        .map(|value| read_u32(value, field))
        .transpose()
}

fn validate_integer_array(
    value: &ValueRef<'_>,
    field: &'static str,
    expected_length: Option<usize>,
) -> Result<(), InspectionError> {
    let values = expect_array(value, field)?;
    if expected_length.is_some_and(|expected| values.len() != expected) {
        return Err(InspectionError::InvalidFieldValue(field));
    }
    for value in values {
        read_u32(value, field)?;
    }
    Ok(())
}

fn validate_string_array(value: &ValueRef<'_>, field: &'static str) -> Result<(), InspectionError> {
    for value in expect_array(value, field)? {
        expect_string(value, field)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::align_up;
    use crate::InspectionError;

    #[test]
    fn checked_alignment_rejects_overflow() {
        assert_eq!(
            align_up(u64::MAX, 8),
            Err(InspectionError::InvalidImplicitArgumentSpan)
        );
        assert_eq!(align_up(u64::MAX - 7, 8), Ok(u64::MAX - 7));
    }
}
