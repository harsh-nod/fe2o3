//! Non-authoritative one-row gfx942 Row Softmax V1 hardware evidence.
//!
//! The ignored hardware test executes only an exact SHA-256-pinned COV6 image
//! after checking its physical metadata and digest-pinned, observed LLVM 22
//! disassembly. The exact verified objdump ELF is copied into a sealed memfd;
//! its first exec is identity-checked at a ptrace exec stop and any second exec
//! is rejected before replacement code runs. A no-descendant seccomp profile,
//! pidfd supervision, and `PTRACE_O_EXITKILL` bound the process. The pinned HSACO
//! reaches objdump only through a separate inherited, sealed, read-only memfd.
//! The disassembly remains observational and does not
//! authenticate the tool's provenance, dynamic loader, shared libraries, or
//! host process environment.
//! It deliberately bypasses production prerequisite authentication and cannot
//! grant protected compiler or execution evidence. In particular, it does not
//! establish compiler origin, source/proof binding, production publication,
//! load or launch authority, exact real-number equivalence, or race freedom.

use fe2o3_host::HsaLaunchGeometryV1;
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind, ExplicitValueType, HiddenValueKind,
};
use sha2::{Digest, Sha256};

#[cfg(any(test, feature = "hardware-test-hooks"))]
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(any(test, feature = "hardware-test-hooks"))]
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
#[cfg(any(test, feature = "hardware-test-hooks"))]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(any(test, feature = "hardware-test-hooks"))]
use std::os::unix::{ffi::OsStrExt, process::ExitStatusExt};

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_amd_target::FeatureState;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_core::GpuContext;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_host::{
    HsaKernelResolutionObservationV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaHardwareTestBufferV1, ReviewedHsaKernelV1,
    ReviewedHsaRuntimeAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsaco::CodeObjectVersion;

const ELEMENTS: usize = 64;
const WORKGROUP_X: u32 = 64;
const EXPLICIT_KERNARG_BYTES: usize = 32;
const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;
const COMPLETE_KERNARG_BYTES: usize = EXPLICIT_KERNARG_BYTES + COV6_IMPLICIT_KERNARG_BYTES;
const PHYSICAL_KERNARG_ALIGNMENT: u64 = 8;
#[cfg(feature = "hardware-test-hooks")]
const HSA_KERNARG_ALIGNMENT: u64 = 16;
const CANARY_ELEMENTS: usize = 32;
const ROW_SOFTMAX_V1_EXPORT: &str = "row_softmax_v1";
const TARGET: &str = "gfx942:xnack-";
#[cfg(any(test, feature = "hardware-test-hooks"))]
const MAX_LLVM_OBJDUMP_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(any(test, feature = "hardware-test-hooks"))]
const MAX_LLVM_OBJDUMP_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
#[cfg(any(test, feature = "hardware-test-hooks"))]
const LLVM_OBJDUMP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);
#[cfg(any(test, feature = "hardware-test-hooks"))]
const READY_DESCRIPTOR: RawFd = 197;
#[cfg(any(test, feature = "hardware-test-hooks"))]
const EXECUTABLE_DESCRIPTOR: RawFd = 198;
#[cfg(any(test, feature = "hardware-test-hooks"))]
const ARTIFACT_DESCRIPTOR: RawFd = 199;
#[cfg(any(test, feature = "hardware-test-hooks"))]
const RELOCATED_DESCRIPTOR_MINIMUM: RawFd = 256;

const INPUT_PREFIX: f32 = f32::from_bits(0x7fc0_a001);
const INPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_a002);
const OUTPUT_PREFIX: f32 = f32::from_bits(0x7fc0_d001);
const OUTPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_d002);
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_d0ff);

// This conservative evidence envelope covers ordinary FP32 reduction order and
// approximate hardware exp for the finite corpus below. It is not a derived
// device error model, a correctly-rounded-exp claim, or exact real equivalence.
const SOFTMAX_ABSOLUTE_TOLERANCE: f32 = 3.0e-6;
const SOFTMAX_RELATIVE_TOLERANCE: f32 = 3.0e-5;
const SOFTMAX_SUM_TOLERANCE: f32 = 6.0e-5;

type BoxError = Box<dyn std::error::Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundKernelEntry {
    address: u64,
    file_offset: u64,
    size: u64,
}

impl BoundKernelEntry {
    fn end(self) -> Result<u64, BoxError> {
        self.address
            .checked_add(self.size)
            .ok_or_else(|| "Row Softmax V1 entry address range overflowed".into())
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn parse_exact_sha256(variable: &str, hex: &str) -> Result<[u8; 32], BoxError> {
    require(
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        format!("{variable} must be exactly 64 lowercase hex digits"),
    )?;
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{variable} is malformed"))?;
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ArgumentFact {
    name: Option<String>,
    type_name: Option<String>,
    offset: u64,
    size: u64,
    alignment: Option<u64>,
    kind: ExplicitValueKind,
    value_type: Option<ExplicitValueType>,
    address_space: Option<ArgumentAddressSpace>,
    access: Option<ArgumentAccess>,
    actual_access: Option<ArgumentAccess>,
    pointee_alignment: Option<u64>,
    is_const: Option<bool>,
    is_restrict: Option<bool>,
    is_volatile: Option<bool>,
    is_pipe: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HiddenArgumentFact {
    offset: u64,
    size: u64,
    kind: HiddenValueKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MetadataFacts {
    code_object_version: u8,
    target: String,
    has_printf_metadata: bool,
    kernel_count: usize,
    kernel_name: String,
    descriptor_symbol: String,
    kernarg_size: u64,
    kernarg_alignment: u64,
    implicit_offset: Option<u64>,
    implicit_size: u64,
    required_workgroup: Option<[u32; 3]>,
    max_flat_workgroup: u32,
    max_workgroups: [Option<u32>; 3],
    cluster_dims: Option<[u32; 3]>,
    uniform_workgroup: bool,
    workgroup_processor_mode: Option<bool>,
    wavefront_size: u32,
    group_segment_size: u64,
    private_segment_size: u64,
    normal_kernel: bool,
    sgpr_spill_count: u32,
    vgpr_spill_count: u32,
    uses_dynamic_stack: bool,
    arguments: Vec<ArgumentFact>,
    hidden_arguments: Vec<HiddenArgumentFact>,
    binding_count: usize,
    binding_kernel_index: usize,
    descriptor_kernarg_size: u32,
    descriptor_group_segment_size: u32,
    descriptor_private_segment_size: u32,
    descriptor_private_segment_enabled: bool,
    descriptor_wavefront_size: u32,
    descriptor_uses_dynamic_stack: bool,
}

impl MetadataFacts {
    fn expected(kernel_symbol: &str) -> Self {
        Self {
            code_object_version: 6,
            target: TARGET.to_owned(),
            has_printf_metadata: false,
            kernel_count: 1,
            kernel_name: kernel_symbol.to_owned(),
            descriptor_symbol: format!("{kernel_symbol}.kd"),
            kernarg_size: COMPLETE_KERNARG_BYTES as u64,
            kernarg_alignment: PHYSICAL_KERNARG_ALIGNMENT,
            implicit_offset: Some(EXPLICIT_KERNARG_BYTES as u64),
            implicit_size: COV6_IMPLICIT_KERNARG_BYTES as u64,
            required_workgroup: Some([WORKGROUP_X, 1, 1]),
            max_flat_workgroup: WORKGROUP_X,
            max_workgroups: [Some(1), Some(1), Some(1)],
            cluster_dims: None,
            uniform_workgroup: true,
            workgroup_processor_mode: None,
            wavefront_size: 64,
            group_segment_size: 0,
            private_segment_size: 0,
            normal_kernel: true,
            sgpr_spill_count: 0,
            vgpr_spill_count: 0,
            uses_dynamic_stack: false,
            arguments: (0_u64..2)
                .flat_map(|slice| {
                    let base = slice * 16;
                    [
                        ArgumentFact {
                            name: Some(format!("arg{slice}.data")),
                            type_name: None,
                            offset: base,
                            size: 8,
                            alignment: None,
                            kind: ExplicitValueKind::GlobalBuffer,
                            value_type: None,
                            address_space: Some(ArgumentAddressSpace::Global),
                            access: None,
                            actual_access: None,
                            pointee_alignment: None,
                            is_const: None,
                            is_restrict: None,
                            is_volatile: None,
                            is_pipe: None,
                        },
                        ArgumentFact {
                            name: Some(format!("arg{slice}.len")),
                            type_name: None,
                            offset: base + 8,
                            size: 8,
                            alignment: None,
                            kind: ExplicitValueKind::ByValue,
                            value_type: None,
                            address_space: None,
                            access: None,
                            actual_access: None,
                            pointee_alignment: None,
                            is_const: None,
                            is_restrict: None,
                            is_volatile: None,
                            is_pipe: None,
                        },
                    ]
                })
                .collect(),
            hidden_arguments: expected_cov6_hidden_arguments(),
            binding_count: 1,
            binding_kernel_index: 0,
            descriptor_kernarg_size: COMPLETE_KERNARG_BYTES as u32,
            descriptor_group_segment_size: 0,
            descriptor_private_segment_size: 0,
            descriptor_private_segment_enabled: false,
            descriptor_wavefront_size: 64,
            descriptor_uses_dynamic_stack: false,
        }
    }
}

fn validate_metadata(facts: &MetadataFacts, kernel_symbol: &str) -> Result<(), BoxError> {
    let expected = MetadataFacts::expected(kernel_symbol);
    require(
        facts.arguments.len() == expected.arguments.len(),
        "Row Softmax V1 must expose exactly four physical slice fields",
    )?;
    for (index, argument) in facts.arguments.iter().enumerate() {
        validate_argument_fact(index, argument)?;
    }
    require(
        facts.hidden_arguments == expected.hidden_arguments,
        format!(
            "Row Softmax V1 must expose exactly the 13 mandatory COV6 hidden records: {:#?}",
            facts.hidden_arguments
        ),
    )?;
    require(
        matches!(facts.cluster_dims, None | Some([1, 1, 1])),
        "Row Softmax V1 cluster dimensions must be absent or exactly [1, 1, 1]",
    )?;
    require(
        matches!(facts.workgroup_processor_mode, None | Some(false)),
        "Row Softmax V1 must not enable WGP mode on gfx942",
    )?;
    let mut normalized = facts.clone();
    normalized.arguments = expected.arguments.clone();
    normalized.cluster_dims = expected.cluster_dims;
    normalized.workgroup_processor_mode = expected.workgroup_processor_mode;
    require(
        normalized == expected,
        format!(
            "Row Softmax V1 metadata or descriptor differs from the exact \
             gfx942:xnack- COV6/WG64/288-byte two-slice profile: {facts:#?}"
        ),
    )
}

fn expected_cov6_hidden_arguments() -> Vec<HiddenArgumentFact> {
    [
        (32, 4, HiddenValueKind::BlockCountX),
        (36, 4, HiddenValueKind::BlockCountY),
        (40, 4, HiddenValueKind::BlockCountZ),
        (44, 2, HiddenValueKind::GroupSizeX),
        (46, 2, HiddenValueKind::GroupSizeY),
        (48, 2, HiddenValueKind::GroupSizeZ),
        (50, 2, HiddenValueKind::RemainderX),
        (52, 2, HiddenValueKind::RemainderY),
        (54, 2, HiddenValueKind::RemainderZ),
        (72, 8, HiddenValueKind::GlobalOffsetX),
        (80, 8, HiddenValueKind::GlobalOffsetY),
        (88, 8, HiddenValueKind::GlobalOffsetZ),
        (96, 2, HiddenValueKind::GridDimensions),
    ]
    .into_iter()
    .map(|(offset, size, kind)| HiddenArgumentFact { offset, size, kind })
    .collect()
}

fn validate_argument_fact(index: usize, argument: &ArgumentFact) -> Result<(), BoxError> {
    let slice = index / 2;
    let pointer = index.is_multiple_of(2);
    let expected_name = if pointer {
        format!("arg{slice}.data")
    } else {
        format!("arg{slice}.len")
    };
    require(
        argument.name.as_deref() == Some(expected_name.as_str()),
        format!("argument {index} must retain exact name `{expected_name}`"),
    )?;
    require(
        argument.type_name.is_none(),
        format!("argument {index} has an unauthenticated source type spelling"),
    )?;
    require(
        argument.offset == index as u64 * 8
            && argument.size == 8
            && matches!(argument.alignment, None | Some(8)),
        format!("argument {index} has a contradictory physical extent or alignment"),
    )?;
    require(
        argument.is_volatile != Some(true) && argument.is_pipe != Some(true),
        format!("argument {index} has an unsupported volatile or pipe qualifier"),
    )?;

    if pointer {
        let input = slice == 0;
        let expected_access = if input {
            ArgumentAccess::ReadOnly
        } else {
            ArgumentAccess::ReadWrite
        };
        require(
            argument.kind == ExplicitValueKind::GlobalBuffer
                && matches!(argument.value_type, None | Some(ExplicitValueType::F32))
                && argument.address_space == Some(ArgumentAddressSpace::Global)
                && matches!(argument.pointee_alignment, None | Some(4))
                && (argument.access.is_none() || argument.access == Some(expected_access))
                && if input {
                    matches!(
                        argument.actual_access,
                        None | Some(ArgumentAccess::ReadOnly)
                    ) && matches!(argument.is_const, None | Some(true))
                        && matches!(argument.is_restrict, None | Some(false))
                } else {
                    matches!(
                        argument.actual_access,
                        None | Some(ArgumentAccess::WriteOnly | ArgumentAccess::ReadWrite)
                    ) && matches!(argument.is_const, None | Some(false))
                        && matches!(argument.is_restrict, None | Some(true))
                },
            format!("argument {index} contradicts the exact f32 slice pointer contract"),
        )?;
    } else {
        require(
            argument.kind == ExplicitValueKind::ByValue
                && matches!(argument.value_type, None | Some(ExplicitValueType::U64))
                && argument.address_space.is_none()
                && argument.access.is_none()
                && argument.actual_access.is_none()
                && argument.pointee_alignment.is_none()
                && argument.is_const != Some(true)
                && argument.is_restrict != Some(true),
            format!("argument {index} contradicts the exact u64 slice-length contract"),
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn inspect_metadata(bytes: &[u8], kernel_symbol: &str) -> Result<BoundKernelEntry, BoxError> {
    let bound = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)?;
    let inspection = bound.inspection();
    let kernel = inspection
        .kernels()
        .first()
        .ok_or("Row Softmax V1 HSACO declares no kernel")?;
    let binding = bound
        .bindings()
        .first()
        .ok_or("Row Softmax V1 HSACO has no descriptor binding")?;
    let descriptor = binding.descriptor();
    let facts = MetadataFacts {
        code_object_version: match inspection.code_object_version() {
            CodeObjectVersion::V6 => 6,
            other => other.number(),
        },
        target: inspection.target().to_string(),
        has_printf_metadata: inspection.has_printf_metadata(),
        kernel_count: inspection.kernels().len(),
        kernel_name: kernel.name().to_owned(),
        descriptor_symbol: kernel.symbol().to_owned(),
        kernarg_size: kernel.kernarg_segment_size(),
        kernarg_alignment: kernel.kernarg_segment_alignment(),
        implicit_offset: kernel.implicit_argument_offset(),
        implicit_size: kernel.implicit_argument_size(),
        required_workgroup: kernel.required_workgroup_size(),
        max_flat_workgroup: kernel.max_flat_workgroup_size(),
        max_workgroups: kernel.max_workgroups(),
        cluster_dims: kernel.cluster_dims(),
        uniform_workgroup: kernel.uniform_work_group_size(),
        workgroup_processor_mode: kernel.workgroup_processor_mode(),
        wavefront_size: kernel.wavefront_size(),
        group_segment_size: kernel.group_segment_fixed_size(),
        private_segment_size: kernel.private_segment_fixed_size(),
        normal_kernel: kernel.kind() == fe2o3_hsaco::KernelKind::Normal,
        sgpr_spill_count: kernel.sgpr_spill_count().unwrap_or(0),
        vgpr_spill_count: kernel.vgpr_spill_count().unwrap_or(0),
        uses_dynamic_stack: kernel.uses_dynamic_stack(),
        arguments: kernel
            .explicit_arguments()
            .iter()
            .map(|argument| ArgumentFact {
                name: argument.name().map(str::to_owned),
                type_name: argument.type_name().map(str::to_owned),
                offset: argument.offset(),
                size: argument.size(),
                alignment: argument.alignment(),
                kind: argument.value_kind(),
                value_type: argument.value_type(),
                address_space: argument.address_space(),
                access: argument.access(),
                actual_access: argument.actual_access(),
                pointee_alignment: argument.pointee_alignment(),
                is_const: argument.is_const(),
                is_restrict: argument.is_restrict(),
                is_volatile: argument.is_volatile(),
                is_pipe: argument.is_pipe(),
            })
            .collect(),
        hidden_arguments: kernel
            .hidden_arguments()
            .iter()
            .map(|argument| HiddenArgumentFact {
                offset: argument.offset(),
                size: argument.size(),
                kind: argument.value_kind(),
            })
            .collect(),
        binding_count: bound.bindings().len(),
        binding_kernel_index: binding.kernel_index(),
        descriptor_kernarg_size: descriptor.kernarg_size(),
        descriptor_group_segment_size: descriptor.group_segment_fixed_size(),
        descriptor_private_segment_size: descriptor.private_segment_fixed_size(),
        descriptor_private_segment_enabled: descriptor.private_segment_enabled(),
        descriptor_wavefront_size: descriptor.wavefront_size(),
        descriptor_uses_dynamic_stack: descriptor.uses_dynamic_stack(),
    };
    validate_metadata(&facts, kernel_symbol)?;
    let entry = BoundKernelEntry {
        address: binding.entry_address(),
        file_offset: binding.entry_file_offset(),
        size: binding.entry_size(),
    };
    require(entry.size != 0, "Row Softmax V1 ELF entry is empty")?;
    require(
        entry
            .file_offset
            .checked_add(entry.size)
            .is_some_and(|end| end <= bytes.len() as u64),
        "Row Softmax V1 ELF entry file range exceeds the pinned HSACO",
    )?;
    let _ = entry.end()?;
    Ok(entry)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisassembledInstruction {
    address: u64,
    byte_len: u64,
    mnemonic: String,
}

fn parse_function_header(line: &str) -> Result<Option<(u64, &str)>, BoxError> {
    let line = line.trim();
    let Some((address, symbol)) = line.split_once(" <") else {
        return Ok(None);
    };
    let Some(symbol) = symbol.strip_suffix(">:") else {
        return Ok(None);
    };
    require(
        address.len() == 16 && address.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "llvm-objdump emitted a non-canonical function-header address",
    )?;
    require(
        !symbol.is_empty()
            && symbol
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$')),
        "llvm-objdump emitted an invalid function-header symbol",
    )?;
    Ok(Some((u64::from_str_radix(address, 16)?, symbol)))
}

fn parse_instruction(line: &str) -> Result<DisassembledInstruction, BoxError> {
    require(
        line.starts_with('\t'),
        "symbol-scoped llvm-objdump body contains a non-instruction line",
    )?;
    let (assembly, encoding) = line
        .split_once("//")
        .ok_or("llvm-objdump instruction omitted its address/encoding annotation")?;
    let mnemonic = assembly
        .split_ascii_whitespace()
        .next()
        .ok_or("llvm-objdump emitted an empty instruction")?;
    require(
        mnemonic
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "llvm-objdump emitted a non-canonical instruction mnemonic",
    )?;
    let (address, words) = encoding
        .trim()
        .split_once(':')
        .ok_or("llvm-objdump instruction annotation omitted its address")?;
    require(
        address.len() == 12 && address.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "llvm-objdump emitted a non-canonical instruction address",
    )?;
    let words = words.split_ascii_whitespace().collect::<Vec<_>>();
    require(
        matches!(words.len(), 1 | 2)
            && words
                .iter()
                .all(|word| word.len() == 8 && word.bytes().all(|byte| byte.is_ascii_hexdigit())),
        "llvm-objdump emitted a non-canonical AMDGPU instruction encoding",
    )?;
    Ok(DisassembledInstruction {
        address: u64::from_str_radix(address, 16)?,
        byte_len: (words.len() * 4) as u64,
        mnemonic: mnemonic.to_owned(),
    })
}

/// Checks only bounded, symbol-scoped text shape reported by pinned objdump.
///
/// Mnemonic presence and absence do not establish CFG, dataflow, reachability,
/// natural-exponential scaling, or semantic use of any observed instruction.
/// This observation provides no machine-code semantic validation and is not a
/// substitute for the separate finite numerical execution checks.
fn validate_observational_isa_shape(
    disassembly: &str,
    kernel_symbol: &str,
    entry: BoundKernelEntry,
) -> Result<(), BoxError> {
    require(
        !disassembly.contains('\0'),
        "llvm-objdump output contains a NUL byte",
    )?;
    let lines = disassembly.lines().collect::<Vec<_>>();
    let mut headers = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        if let Some((address, symbol)) = parse_function_header(line)? {
            headers.push((line_index, address, symbol));
        }
    }
    require(
        headers.len() == 1,
        format!(
            "symbol-scoped llvm-objdump emitted {} function headers instead of one",
            headers.len()
        ),
    )?;
    let (header_index, header_address, header_symbol) = headers[0];
    let preamble = lines[..header_index]
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    require(
        preamble.len() == 2
            && preamble[0]
                .strip_suffix(":\tfile format elf64-amdgpu")
                .is_some_and(|path| !path.is_empty())
            && preamble[1] == "Disassembly of section .text:",
        "llvm-objdump output omitted the exact AMDGPU .text preamble",
    )?;
    require(
        header_symbol == kernel_symbol && header_address == entry.address,
        "llvm-objdump function header differs from the bound Row Softmax V1 ELF entry",
    )?;

    let instructions = lines
        .iter()
        .skip(header_index + 1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_instruction(line))
        .collect::<Result<Vec<_>, _>>()?;
    require(
        !instructions.is_empty(),
        "symbol-scoped llvm-objdump emitted an empty function body",
    )?;
    let mut expected_address = entry.address;
    for instruction in &instructions {
        require(
            instruction.address == expected_address,
            "llvm-objdump instruction addresses do not exactly cover the bound ELF entry",
        )?;
        expected_address = expected_address
            .checked_add(instruction.byte_len)
            .ok_or("llvm-objdump instruction address overflow")?;
    }
    require(
        expected_address == entry.end()?,
        "llvm-objdump function body does not exactly cover the bound ELF entry size",
    )?;

    let exp_count = instructions
        .iter()
        .filter(|instruction| instruction.mnemonic == "v_exp_f32")
        .count();
    require(
        exp_count == 1,
        format!("expected exactly one retained vector exponential, found {exp_count}"),
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("global_load_")),
        "observed ISA shape has no global-load mnemonic",
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("global_store_")),
        "observed ISA shape has no global-store mnemonic",
    )?;
    let terminators = instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| instruction.mnemonic == "s_endpgm")
        .collect::<Vec<_>>();
    require(
        terminators.len() == 1,
        "observed ISA shape must contain exactly one termination mnemonic",
    )?;
    require(
        instructions[terminators[0].0 + 1..]
            .iter()
            .all(|instruction| instruction.mnemonic == "s_nop"),
        "bound Row Softmax V1 entry contains executable instructions after s_endpgm",
    )?;

    for (family, description) in [
        ("s_call", "call"),
        ("s_swappc", "call"),
        ("s_getpc", "call sequence"),
        ("s_setpc", "call/return sequence"),
        ("scratch_", "scratch-memory access"),
        ("ds_", "LDS access in the direct-global slice"),
    ] {
        require(
            !instructions
                .iter()
                .any(|instruction| instruction.mnemonic.starts_with(family)),
            format!("observed ISA shape contains forbidden {description} family `{family}`"),
        )?;
    }
    require(
        !instructions
            .iter()
            .any(|instruction| instruction.mnemonic.contains("atomic")),
        "observed ISA shape contains an atomic mnemonic",
    )?;
    require(
        !instructions
            .iter()
            .any(|instruction| instruction.mnemonic.contains("mfma")),
        "Row Softmax V1 observed ISA shape contains an unrelated matrix mnemonic",
    )
}

fn representative_inputs() -> Vec<[f32; ELEMENTS]> {
    let uniform = [0.0; ELEMENTS];

    let mut centered_ramp = [0.0; ELEMENTS];
    for (index, value) in centered_ramp.iter_mut().enumerate() {
        *value = (index as f32 - 31.5) * 0.125;
    }

    let mut repeated_ties = [0.0; ELEMENTS];
    for (index, value) in repeated_ties.iter_mut().enumerate() {
        *value = 80.0 - ((index * 17) % 11) as f32 * 0.75;
    }

    let mut dominant_pair = [-80.0; ELEMENTS];
    dominant_pair[7] = 100.0;
    dominant_pair[41] = 100.0;
    dominant_pair[23] = 99.0;

    let mut finite_extrema = [f32::MIN; ELEMENTS];
    finite_extrema[3] = f32::MAX;
    finite_extrema[37] = 0.0;

    let mut subnormals = [0.0; ELEMENTS];
    for (index, value) in subnormals.iter_mut().enumerate() {
        *value = match index % 4 {
            0 => f32::from_bits(1),
            1 => -f32::from_bits(1),
            2 => f32::from_bits(0x007f_ffff),
            _ => -f32::from_bits(0x007f_ffff),
        };
    }

    let mut near_ties = [1.0; ELEMENTS];
    for (index, value) in near_ties.iter_mut().enumerate() {
        *value = f32::from_bits(1.0_f32.to_bits() + (index % 8) as u32);
    }

    let mut large_translation = [0.0; ELEMENTS];
    for (index, value) in large_translation.iter_mut().enumerate() {
        *value = 1_048_576.0 + (index % 8) as f32 * 0.125;
    }

    vec![
        uniform,
        centered_ramp,
        repeated_ties,
        dominant_pair,
        finite_extrema,
        subnormals,
        near_ties,
        large_translation,
    ]
}

/// Computes stable softmax in f64 and rounds each result to f32.
///
/// V1 evidence deliberately admits only exactly 64 finite f32 inputs. NaN and
/// either infinity are rejected rather than assigned kernel semantics. This
/// host oracle is a numerical reference for a finite corpus, not a proof of
/// exact real-number equivalence or correctly rounded device math.
fn softmax_oracle(input: &[f32]) -> Result<Vec<f32>, BoxError> {
    require(
        input.len() == ELEMENTS,
        "Row Softmax V1 oracle requires exactly 64 elements",
    )?;
    require(
        input.iter().all(|value| value.is_finite()),
        "Row Softmax V1 finite evidence policy rejects NaN and infinity",
    )?;
    let maximum = input
        .iter()
        .copied()
        .map(f64::from)
        .fold(f64::NEG_INFINITY, f64::max);
    let exponentials = input
        .iter()
        .map(|value| (f64::from(*value) - maximum).exp())
        .collect::<Vec<_>>();
    let sum = exponentials.iter().sum::<f64>();
    require(
        sum.is_finite() && sum > 0.0,
        "Row Softmax V1 oracle normalization is not positive and finite",
    )?;
    Ok(exponentials
        .into_iter()
        .map(|value| (value / sum) as f32)
        .collect())
}

fn guarded_f32(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(CANARY_ELEMENTS * 2 + body.len());
    values.extend(std::iter::repeat_n(prefix, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, CANARY_ELEMENTS));
    values
}

fn verify_exact_f32(role: &str, actual: &[f32], expected: &[f32]) -> Result<(), BoxError> {
    require(
        actual.len() == expected.len(),
        format!("{role} length changed"),
    )?;
    for (index, (observed, expected)) in actual.iter().zip(expected).enumerate() {
        require(
            observed.to_bits() == expected.to_bits(),
            format!(
                "{role}[{index}] changed: {:#010x} != {:#010x}",
                observed.to_bits(),
                expected.to_bits()
            ),
        )?;
    }
    Ok(())
}

struct GuardedF32Slices<'a> {
    prefix: &'a [f32],
    body: &'a [f32],
    suffix: &'a [f32],
}

fn split_guarded<'a>(role: &str, actual: &'a [f32]) -> Result<GuardedF32Slices<'a>, BoxError> {
    require(
        actual.len() == ELEMENTS + 2 * CANARY_ELEMENTS,
        format!("{role} guarded allocation length changed"),
    )?;
    let (actual_prefix, remainder) = actual.split_at(CANARY_ELEMENTS);
    let (actual_body, actual_suffix) = remainder.split_at(ELEMENTS);
    Ok(GuardedF32Slices {
        prefix: actual_prefix,
        body: actual_body,
        suffix: actual_suffix,
    })
}

fn verify_guarded_input(actual: &[f32], expected_body: &[f32]) -> Result<(), BoxError> {
    let guarded = split_guarded("input", actual)?;
    verify_exact_f32(
        "input prefix canary",
        guarded.prefix,
        &[INPUT_PREFIX; CANARY_ELEMENTS],
    )?;
    verify_exact_f32("input body", guarded.body, expected_body)?;
    verify_exact_f32(
        "input suffix canary",
        guarded.suffix,
        &[INPUT_SUFFIX; CANARY_ELEMENTS],
    )
}

fn verify_softmax_body(actual: &[f32], expected: &[f32]) -> Result<(), BoxError> {
    require(
        actual.len() == ELEMENTS && expected.len() == ELEMENTS,
        "Row Softmax V1 output has the wrong logical extent",
    )?;
    for (index, (observed, reference)) in actual.iter().zip(expected).enumerate() {
        require(
            observed.is_finite() && *observed >= 0.0 && *observed <= 1.0,
            format!("output[{index}] is not a finite probability: {observed}"),
        )?;
        let error = (*observed - *reference).abs();
        let limit = SOFTMAX_ABSOLUTE_TOLERANCE + SOFTMAX_RELATIVE_TOLERANCE * reference.abs();
        require(
            error <= limit,
            format!(
                "output[{index}] differs from the stable f64 oracle: observed={observed} \
                 reference={reference} error={error} limit={limit}"
            ),
        )?;
    }
    let sum = actual.iter().sum::<f32>();
    require(
        (sum - 1.0).abs() <= SOFTMAX_SUM_TOLERANCE,
        format!("Row Softmax V1 output sum {sum} is outside the normalization tolerance"),
    )
}

fn verify_guarded_output(actual: &[f32], expected_body: &[f32]) -> Result<(), BoxError> {
    let guarded = split_guarded("output", actual)?;
    verify_exact_f32(
        "output prefix canary",
        guarded.prefix,
        &[OUTPUT_PREFIX; CANARY_ELEMENTS],
    )?;
    verify_softmax_body(guarded.body, expected_body)?;
    verify_exact_f32(
        "output suffix canary",
        guarded.suffix,
        &[OUTPUT_SUFFIX; CANARY_ELEMENTS],
    )
}

fn launch_geometry() -> HsaLaunchGeometryV1 {
    // The runtime adapter interprets `grid` as block counts and derives the
    // AQL global work-item grid by multiplying it with the WG64 dimensions.
    HsaLaunchGeometryV1::new([1, 1, 1], [WORKGROUP_X, 1, 1], 0)
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn explicit_kernarg(addresses: [u64; 2]) -> [u8; EXPLICIT_KERNARG_BYTES] {
    let mut bytes = [0; EXPLICIT_KERNARG_BYTES];
    for (index, address) in addresses.into_iter().enumerate() {
        let offset = index * 16;
        put_u64(&mut bytes, offset, address);
        put_u64(&mut bytes, offset + 8, ELEMENTS as u64);
    }
    bytes
}

#[cfg(feature = "hardware-test-hooks")]
struct PinnedArtifact {
    bytes: Vec<u8>,
    digest: PayloadDigest,
    kernel_symbol: String,
    objdump: PinnedObjdump,
}

#[cfg(feature = "hardware-test-hooks")]
struct PinnedObjdump {
    bytes: Vec<u8>,
    digest: [u8; 32],
    executable: PrivateExecutableMaterialization,
    observed_version: String,
}

#[cfg(feature = "hardware-test-hooks")]
impl PinnedObjdump {
    fn verify_bytes(&self) -> Result<(), BoxError> {
        require(
            sha256(&self.bytes) == self.digest,
            "retained llvm-objdump bytes no longer match their exact digest",
        )?;
        self.executable.verify(&self.bytes)
    }

    fn output<I, S>(&self, arguments: I) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.verify_bytes()?;
        let output = self.executable.output(&self.bytes, arguments)?;
        self.verify_bytes()?;
        Ok(output)
    }

    fn output_with_artifact<I, S>(
        &self,
        artifact: &PrivateArtifactMaterialization,
        arguments: I,
    ) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.verify_bytes()?;
        let output = self
            .executable
            .output_with_artifact(&self.bytes, artifact, arguments)?;
        self.verify_bytes()?;
        Ok(output)
    }

    fn observe_version(&self) -> Result<String, BoxError> {
        let output = self.output(["--version"])?;
        require(
            output.status.success() && output.stderr.is_empty(),
            "digest-pinned llvm-objdump --version failed or emitted stderr",
        )?;
        observed_llvm_22_version(&output.stdout)
    }
}

fn observed_llvm_22_version(stdout: &[u8]) -> Result<String, BoxError> {
    require(
        !stdout.is_empty() && stdout.len() <= 64 * 1024 && !stdout.contains(&0),
        "llvm-objdump --version output has an invalid bounded shape",
    )?;
    let stdout = std::str::from_utf8(stdout)?;
    let first = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or("llvm-objdump --version omitted its version line")?
        .trim();
    let marker = "LLVM version ";
    let version = first
        .split_once(marker)
        .map(|(_, version)| version)
        .ok_or("llvm-objdump did not identify an observed LLVM version")?;
    let token = version
        .split_ascii_whitespace()
        .next()
        .ok_or("llvm-objdump omitted its observed LLVM version token")?;
    require(
        token
            .split_once('.')
            .is_some_and(|(major, rest)| major == "22" && !rest.is_empty()),
        format!("llvm-objdump must report observed LLVM major 22, found `{first}`"),
    )?;
    Ok(first.to_owned())
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
struct PrivateArtifactMaterialization {
    file: std::fs::File,
    byte_len: u64,
    device: u64,
    inode: u64,
    digest: [u8; 32],
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
impl PrivateArtifactMaterialization {
    fn new(bytes: &[u8]) -> Result<Self, BoxError> {
        require(
            !bytes.is_empty() && bytes.len() <= fe2o3_hsaco::MAX_HSACO_BYTES,
            "private HSACO materialization has an invalid bounded length",
        )?;
        // SAFETY: memfd_create returns a fresh descriptor or a negative error.
        let descriptor = unsafe {
            libc::syscall(
                libc::SYS_memfd_create,
                c"fe2o3-row-softmax-v1-hsaco".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        } as libc::c_int;
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: memfd_create returned a fresh owned descriptor.
        let mut writable = unsafe { std::fs::File::from_raw_fd(descriptor) };
        writable.write_all(bytes)?;
        writable.flush()?;
        writable.sync_all()?;
        // SAFETY: fchmod and fcntl operate on this live private descriptor.
        if unsafe { libc::fchmod(writable.as_raw_fd(), 0o400) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let required_seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        // SAFETY: F_ADD_SEALS accepts the integer seal mask for this memfd.
        if unsafe { libc::fcntl(writable.as_raw_fd(), libc::F_ADD_SEALS, required_seals) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let written_metadata = writable.metadata()?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(format!("/proc/self/fd/{}", writable.as_raw_fd()))?;
        let metadata = file.metadata()?;
        require(
            metadata.dev() == written_metadata.dev()
                && metadata.ino() == written_metadata.ino()
                && metadata.len() == written_metadata.len(),
            "sealed HSACO identity changed while reopening it read-only",
        )?;
        drop(writable);
        let materialized = Self {
            file,
            byte_len: bytes.len() as u64,
            device: metadata.dev(),
            inode: metadata.ino(),
            digest: sha256(bytes),
        };
        materialized.verify(bytes)?;
        Ok(materialized)
    }

    fn verify(&self, expected: &[u8]) -> Result<(), BoxError> {
        self.verify_retained()?;
        let mut retained = self.file.try_clone()?;
        retained.seek(SeekFrom::Start(0))?;
        let mut retained_bytes = Vec::new();
        retained.read_to_end(&mut retained_bytes)?;
        require(
            retained_bytes == expected
                && expected.len() as u64 == self.byte_len
                && sha256(expected) == self.digest,
            "sealed HSACO expected bytes changed",
        )
    }

    fn verify_retained(&self) -> Result<(), BoxError> {
        let retained_metadata = self.file.metadata()?;
        // SAFETY: F_GET_SEALS and F_GETFL inspect this live retained descriptor.
        let seals = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GET_SEALS) };
        let status = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GETFL) };
        let required_seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        require(
            seals >= 0
                && seals & required_seals == required_seals
                && status >= 0
                && status & libc::O_ACCMODE == libc::O_RDONLY
                && retained_metadata.dev() == self.device
                && retained_metadata.ino() == self.inode
                && retained_metadata.len() == self.byte_len
                && retained_metadata.permissions().mode() & 0o777 == 0o400,
            "sealed HSACO descriptor identity, seals, or permissions changed",
        )?;
        let mut retained = self.file.try_clone()?;
        retained.seek(SeekFrom::Start(0))?;
        let mut retained_bytes = Vec::new();
        retained.read_to_end(&mut retained_bytes)?;
        require(
            retained_bytes.len() as u64 == self.byte_len && sha256(&retained_bytes) == self.digest,
            "sealed HSACO descriptor bytes changed",
        )
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn bounded_reader<R>(
    mut reader: R,
    limit: usize,
    stream: &'static str,
) -> std::thread::JoinHandle<Result<Vec<u8>, String>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        reader
            .by_ref()
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read llvm-objdump {stream}: {error}"))?;
        if bytes.len() > limit {
            return Err(format!(
                "llvm-objdump {stream} exceeded its {limit}-byte limit"
            ));
        }
        Ok(bytes)
    })
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn collect_finished_reader(
    thread: &mut Option<std::thread::JoinHandle<Result<Vec<u8>, String>>>,
    bytes: &mut Option<Vec<u8>>,
    stream: &str,
) -> Result<(), BoxError> {
    if thread.as_ref().is_some_and(|thread| thread.is_finished()) {
        let result = thread
            .take()
            .expect("finished reader remains owned")
            .join()
            .map_err(|_| format!("llvm-objdump {stream} reader panicked"))?;
        *bytes = Some(result.map_err(|error| -> BoxError { error.into() })?);
    }
    Ok(())
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn pipe(flags: libc::c_int) -> Result<(OwnedFd, OwnedFd), BoxError> {
    let mut descriptors = [-1; 2];
    // SAFETY: pipe2 initializes both entries on success; ownership transfers
    // exactly once to the returned OwnedFd values.
    if unsafe { libc::pipe2(descriptors.as_mut_ptr(), flags) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: successful pipe2 returned two fresh owned descriptors.
    Ok(unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    })
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn pidfd_open(pid: libc::pid_t) -> Result<OwnedFd, BoxError> {
    // SAFETY: pidfd_open has no pointer arguments and the positive child pid
    // remains owned by this supervisor.
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as libc::c_int;
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: pidfd_open returned a fresh owned descriptor.
    let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
    let fdinfo = std::fs::read_to_string(format!("/proc/self/fdinfo/{}", descriptor.as_raw_fd()))?;
    let observed = fdinfo
        .lines()
        .find_map(|line| line.strip_prefix("Pid:\t"))
        .ok_or("pidfd fdinfo omitted its live PID")?;
    require(
        observed.parse::<libc::pid_t>()? == pid,
        "pidfd does not identify the exact objdump child",
    )?;
    Ok(descriptor)
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn relocate_source_descriptor(descriptor: RawFd) -> Result<OwnedFd, BoxError> {
    // SAFETY: F_DUPFD_CLOEXEC duplicates one live source at or above the
    // collision-free child relocation floor.
    let relocated = unsafe {
        libc::fcntl(
            descriptor,
            libc::F_DUPFD_CLOEXEC,
            RELOCATED_DESCRIPTOR_MINIMUM,
        )
    };
    if relocated < RELOCATED_DESCRIPTOR_MINIMUM {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: fcntl returned one fresh owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(relocated) })
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn reap_pid_until(
    pid: libc::pid_t,
    deadline: std::time::Instant,
) -> Result<std::process::ExitStatus, BoxError> {
    loop {
        let mut raw_status = 0;
        // SAFETY: waitpid observes only the exact child owned by this process.
        let result = unsafe { libc::waitpid(pid, &mut raw_status, libc::WNOHANG | libc::__WALL) };
        if result == pid {
            if libc::WIFEXITED(raw_status) || libc::WIFSIGNALED(raw_status) {
                return Ok(std::process::ExitStatus::from_raw(raw_status));
            }
            continue;
        }
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
        require(
            std::time::Instant::now() < deadline,
            "objdump child did not become reapable before the cleanup deadline",
        )?;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn ptrace_request(request: libc::c_uint, pid: libc::pid_t, data: usize) -> Result<(), BoxError> {
    // SAFETY: ptrace receives the exact unreaped tracee pid; requests used here
    // take no address argument and an integer option/signal in `data`.
    let result = unsafe {
        libc::ptrace(
            request,
            pid,
            std::ptr::null_mut::<libc::c_void>(),
            data as *mut libc::c_void,
        )
    };
    if result == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn establish_one_shot_exec_trace(
    pid: libc::pid_t,
    deadline: std::time::Instant,
    artifact: Option<&PrivateArtifactMaterialization>,
) -> Result<Option<std::process::ExitStatus>, BoxError> {
    loop {
        let mut status = 0;
        // SAFETY: waitpid observes only the exact fork child and includes ptrace stops.
        let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG | libc::__WALL) };
        if result == pid {
            if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
                return Ok(Some(std::process::ExitStatus::from_raw(status)));
            }
            require(
                libc::WIFSTOPPED(status) && libc::WSTOPSIG(status) == libc::SIGSTOP,
                "objdump child did not enter its controlled pre-seccomp trace stop",
            )?;
            verify_child_artifact_descriptor(pid, artifact)?;
            let options = (libc::PTRACE_O_TRACEEXEC | libc::PTRACE_O_EXITKILL) as usize;
            ptrace_request(libc::PTRACE_SETOPTIONS, pid, options)?;
            ptrace_request(libc::PTRACE_CONT, pid, 0)?;
            return Ok(None);
        }
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
        require(
            std::time::Instant::now() < deadline,
            "objdump child did not reach its controlled trace stop before the deadline",
        )?;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn verify_child_artifact_descriptor(
    pid: libc::pid_t,
    expected: Option<&PrivateArtifactMaterialization>,
) -> Result<(), BoxError> {
    let path = format!("/proc/{pid}/fd/{ARTIFACT_DESCRIPTOR}");
    let Some(expected) = expected else {
        return match std::fs::File::open(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
            Ok(_) => Err("objdump child retained an unauthorized artifact descriptor".into()),
        };
    };
    let mut observed = std::fs::File::open(path)?;
    let metadata = observed.metadata()?;
    // SAFETY: F_GET_SEALS and F_GETFL inspect the stopped child's inherited memfd.
    let seals = unsafe { libc::fcntl(observed.as_raw_fd(), libc::F_GET_SEALS) };
    let status = unsafe { libc::fcntl(observed.as_raw_fd(), libc::F_GETFL) };
    let required_seals =
        libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut observed)
        .take(fe2o3_hsaco::MAX_HSACO_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        seals >= 0
            && seals & required_seals == required_seals
            && status >= 0
            && status & libc::O_ACCMODE == libc::O_RDONLY
            && metadata.dev() == expected.device
            && metadata.ino() == expected.inode
            && metadata.len() == expected.byte_len
            && metadata.permissions().mode() & 0o777 == 0o400
            && bytes.len() as u64 == expected.byte_len
            && sha256(&bytes) == expected.digest,
        "objdump child did not retain the exact sealed HSACO descriptor",
    )
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn terminate_and_reap(pid: libc::pid_t, pidfd: RawFd) -> Result<(), BoxError> {
    // SAFETY: pidfd_send_signal targets the retained identity, not a reused
    // numeric pid. The remaining arguments are the documented null defaults.
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd,
            libc::SIGKILL,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    reap_pid_until(
        pid,
        std::time::Instant::now() + std::time::Duration::from_secs(5),
    )?;
    Ok(())
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn contain_or_abort(pid: libc::pid_t, pidfd: RawFd, already_reaped: bool) {
    if !already_reaped && terminate_and_reap(pid, pidfd).is_err() {
        std::process::abort();
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn verify_traced_executable_identity(
    pid: libc::pid_t,
    expected: &PrivateExecutableMaterialization,
    expected_bytes: &[u8],
) -> Result<(), BoxError> {
    let mut executed = std::fs::File::open(format!("/proc/{pid}/exe"))?;
    let metadata = executed.metadata()?;
    // SAFETY: F_GET_SEALS and F_GETFL inspect the ptrace-stopped executable object.
    let seals = unsafe { libc::fcntl(executed.as_raw_fd(), libc::F_GET_SEALS) };
    let status = unsafe { libc::fcntl(executed.as_raw_fd(), libc::F_GETFL) };
    let required_seals =
        libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut executed)
        .take(MAX_LLVM_OBJDUMP_BYTES + 1)
        .read_to_end(&mut bytes)?;
    require(
        seals >= 0
            && seals & required_seals == required_seals
            && status >= 0
            && status & libc::O_ACCMODE == libc::O_RDONLY
            && metadata.dev() == expected.device
            && metadata.ino() == expected.inode
            && metadata.len() == expected.byte_len
            && metadata.permissions().mode() & 0o777 == 0o500
            && bytes == expected_bytes
            && sha256(&bytes) == expected.digest,
        "first objdump exec event does not identify the exact sealed executable",
    )
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn observe_traced_child(
    pid: libc::pid_t,
    expected: &PrivateExecutableMaterialization,
    expected_bytes: &[u8],
    artifact: Option<&PrivateArtifactMaterialization>,
    exec_observed: &mut bool,
) -> Result<Option<std::process::ExitStatus>, BoxError> {
    let mut status = 0;
    // SAFETY: waitpid observes only the exact ptrace child without blocking.
    let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG | libc::__WALL) };
    if result == 0 {
        return Ok(None);
    }
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(None);
        }
        return Err(error.into());
    }
    if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
        return Ok(Some(std::process::ExitStatus::from_raw(status)));
    }
    require(
        libc::WIFSTOPPED(status)
            && libc::WSTOPSIG(status) == libc::SIGTRAP
            && (status >> 16) as libc::c_uint == libc::PTRACE_EVENT_EXEC as libc::c_uint,
        "objdump child entered an unexpected ptrace stop",
    )?;
    require(
        !*exec_observed,
        "objdump attempted a forbidden second executable replacement",
    )?;
    verify_traced_executable_identity(pid, expected, expected_bytes)?;
    verify_child_artifact_descriptor(pid, artifact)?;
    *exec_observed = true;
    ptrace_request(libc::PTRACE_CONT, pid, 0)?;
    Ok(None)
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn seccomp_filter(executable_descriptor: RawFd) -> Vec<libc::sock_filter> {
    const LOAD_WORD_ABSOLUTE: u16 = 0x20;
    const JUMP_EQUAL: u16 = 0x15;
    const JUMP_GREATER_EQUAL: u16 = 0x35;
    const RETURN: u16 = 0x06;
    const KILL_PROCESS: u32 = 0x8000_0000;
    const ALLOW: u32 = 0x7fff_0000;
    const ERRNO_EPERM: u32 = 0x0005_0000 | libc::EPERM as u32;
    const AUDIT_ARCH_X86_64: u32 = 0xc000_003e;
    const X32_SYSCALL_BIT: u32 = 0x4000_0000;
    const OFFSET_NR: u32 = 0;
    const OFFSET_ARCH: u32 = 4;
    const OFFSET_ARG0: u32 = 16;
    const OFFSET_ARG4: u32 = 48;

    let statement = |code, value| libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k: value,
    };
    let jump = |value, yes, no| libc::sock_filter {
        code: JUMP_EQUAL,
        jt: yes,
        jf: no,
        k: value,
    };
    let mut filter = vec![
        statement(LOAD_WORD_ABSOLUTE, OFFSET_ARCH),
        jump(AUDIT_ARCH_X86_64, 1, 0),
        statement(RETURN, KILL_PROCESS),
        statement(LOAD_WORD_ABSOLUTE, OFFSET_NR),
        libc::sock_filter {
            code: JUMP_GREATER_EQUAL,
            jt: 0,
            jf: 1,
            k: X32_SYSCALL_BIT,
        },
        statement(RETURN, KILL_PROCESS),
    ];
    for denied in [
        libc::SYS_clone,
        libc::SYS_clone3,
        libc::SYS_fork,
        libc::SYS_vfork,
        libc::SYS_setsid,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_ptrace,
        libc::SYS_execve,
    ] {
        filter.push(jump(denied as u32, 0, 1));
        filter.push(statement(RETURN, ERRNO_EPERM));
    }
    filter.extend([
        jump(libc::SYS_execveat as u32, 0, 5),
        statement(LOAD_WORD_ABSOLUTE, OFFSET_ARG0),
        jump(executable_descriptor as u32, 0, 2),
        statement(LOAD_WORD_ABSOLUTE, OFFSET_ARG4),
        jump(libc::AT_EMPTY_PATH as u32, 1, 0),
        statement(RETURN, ERRNO_EPERM),
        statement(RETURN, ALLOW),
    ]);
    filter
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
#[derive(Clone, Copy)]
struct ChildExecPlan {
    executable: RawFd,
    artifact: RawFd,
    stdout: RawFd,
    stderr: RawFd,
    ready: RawFd,
    devnull: RawFd,
    argv: *const *const libc::c_char,
    environment: *const *const libc::c_char,
    filter: *mut libc::sock_filter,
    filter_len: u16,
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
unsafe fn child_exec_descriptor(plan: ChildExecPlan) -> ! {
    let sources_are_relocated = plan.executable >= RELOCATED_DESCRIPTOR_MINIMUM
        && (plan.artifact < 0 || plan.artifact >= RELOCATED_DESCRIPTOR_MINIMUM)
        && plan.stdout >= RELOCATED_DESCRIPTOR_MINIMUM
        && plan.stderr >= RELOCATED_DESCRIPTOR_MINIMUM
        && plan.ready >= RELOCATED_DESCRIPTOR_MINIMUM
        && plan.devnull >= RELOCATED_DESCRIPTOR_MINIMUM;
    // SAFETY: every source was independently relocated above all destinations,
    // so these dup3 calls cannot overwrite a source needed by a later install.
    if !sources_are_relocated
        || unsafe { libc::dup3(plan.devnull, libc::STDIN_FILENO, 0) } < 0
        || unsafe { libc::dup3(plan.stdout, libc::STDOUT_FILENO, 0) } < 0
        || unsafe { libc::dup3(plan.stderr, libc::STDERR_FILENO, 0) } < 0
        || unsafe { libc::dup3(plan.ready, READY_DESCRIPTOR, libc::O_CLOEXEC) } < 0
        || unsafe { libc::dup3(plan.executable, EXECUTABLE_DESCRIPTOR, libc::O_CLOEXEC) } < 0
        || (plan.artifact >= 0 && unsafe { libc::dup3(plan.artifact, ARTIFACT_DESCRIPTOR, 0) } < 0)
    {
        // SAFETY: terminal child failure avoids non-async-signal-safe cleanup.
        unsafe { libc::_exit(126) };
    }
    if plan.artifact < 0 {
        // SAFETY: no artifact is authorized for this invocation; EBADF is benign.
        unsafe { libc::close(ARTIFACT_DESCRIPTOR) };
    }
    // SAFETY: close_range leaves only stdio and the three fixed descriptors.
    if unsafe { libc::syscall(libc::SYS_close_range, 3_u32, 196_u32, 0_u32) } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_close_range,
                200_u32,
                u32::MAX,
                libc::CLOSE_RANGE_UNSHARE,
            )
        } != 0
        || unsafe { libc::setsid() } < 0
    {
        // SAFETY: terminal child failure avoids non-async-signal-safe cleanup.
        unsafe { libc::_exit(126) };
    }
    let zero_limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_NPROC, &zero_limit) } != 0
        || unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero_limit) } != 0
        || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe {
            libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        } == -1
        || unsafe {
            libc::syscall(
                libc::SYS_tgkill,
                libc::getpid(),
                libc::syscall(libc::SYS_gettid),
                libc::SIGSTOP,
            )
        } != 0
    {
        // SAFETY: terminal child failure avoids non-async-signal-safe cleanup.
        unsafe { libc::_exit(126) };
    }
    let program = libc::sock_fprog {
        len: plan.filter_len,
        filter: plan.filter,
    };
    if unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            &program as *const libc::sock_fprog,
        )
    } != 0
        || unsafe { libc::write(READY_DESCRIPTOR, [1_u8].as_ptr().cast(), 1) } != 1
    {
        // SAFETY: terminal child failure avoids non-async-signal-safe cleanup.
        unsafe { libc::_exit(126) };
    }
    // SAFETY: the sealed descriptor and C vectors remain live in this forked
    // address space; AT_EMPTY_PATH binds execution to that descriptor.
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            EXECUTABLE_DESCRIPTOR,
            c"".as_ptr(),
            plan.argv,
            plan.environment,
            libc::AT_EMPTY_PATH,
        );
        libc::_exit(127);
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn bounded_descriptor_output<I, S>(
    executable: &PrivateExecutableMaterialization,
    expected_executable: &[u8],
    artifact: Option<&PrivateArtifactMaterialization>,
    arguments: I,
    deadline: std::time::Duration,
    output_limit: usize,
) -> Result<std::process::Output, BoxError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    require(
        !deadline.is_zero() && output_limit.checked_add(1).is_some(),
        "llvm-objdump subprocess bounds must be nonzero",
    )?;
    let expires = std::time::Instant::now()
        .checked_add(deadline)
        .ok_or("llvm-objdump deadline overflowed")?;
    let mut argv = vec![std::ffi::CString::new("llvm-objdump")?];
    for argument in arguments {
        argv.push(std::ffi::CString::new(argument.as_ref().as_bytes())?);
    }
    let mut argv_pointers = argv.iter().map(|value| value.as_ptr()).collect::<Vec<_>>();
    argv_pointers.push(std::ptr::null());
    let environment = [
        c"LANG=C".as_ptr(),
        c"LC_ALL=C".as_ptr(),
        c"TZ=UTC".as_ptr(),
        c"PATH=/usr/bin:/bin".as_ptr(),
        std::ptr::null(),
    ];
    let (stdout_read, stdout_write) = pipe(libc::O_CLOEXEC)?;
    let (stderr_read, stderr_write) = pipe(libc::O_CLOEXEC)?;
    let (ready_read, ready_write) = pipe(libc::O_CLOEXEC | libc::O_NONBLOCK)?;
    let devnull = std::fs::OpenOptions::new().read(true).open("/dev/null")?;
    let relocated_executable = relocate_source_descriptor(executable.file.as_raw_fd())?;
    let relocated_artifact = artifact
        .map(|artifact| relocate_source_descriptor(artifact.file.as_raw_fd()))
        .transpose()?;
    let relocated_stdout = relocate_source_descriptor(stdout_write.as_raw_fd())?;
    let relocated_stderr = relocate_source_descriptor(stderr_write.as_raw_fd())?;
    let relocated_ready = relocate_source_descriptor(ready_write.as_raw_fd())?;
    let relocated_devnull = relocate_source_descriptor(devnull.as_raw_fd())?;
    let mut filter = seccomp_filter(EXECUTABLE_DESCRIPTOR);
    let filter_len = u16::try_from(filter.len()).map_err(|_| "seccomp filter is too large")?;
    let child_plan = ChildExecPlan {
        executable: relocated_executable.as_raw_fd(),
        artifact: relocated_artifact.as_ref().map_or(-1, AsRawFd::as_raw_fd),
        stdout: relocated_stdout.as_raw_fd(),
        stderr: relocated_stderr.as_raw_fd(),
        ready: relocated_ready.as_raw_fd(),
        devnull: relocated_devnull.as_raw_fd(),
        argv: argv_pointers.as_ptr(),
        environment: environment.as_ptr(),
        filter: filter.as_mut_ptr(),
        filter_len,
    };

    // SAFETY: all allocations and C vectors are complete before fork. The child
    // performs only direct syscalls before its trace stop, descriptor-bound
    // execveat, or _exit.
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if pid == 0 {
        // SAFETY: this is the dedicated post-fork child path described above.
        unsafe { child_exec_descriptor(child_plan) }
    }
    drop(stdout_write);
    drop(stderr_write);
    drop(ready_write);
    drop(relocated_executable);
    drop(relocated_artifact);
    drop(relocated_stdout);
    drop(relocated_stderr);
    drop(relocated_ready);
    drop(relocated_devnull);

    let pidfd = match pidfd_open(pid) {
        Ok(pidfd) => pidfd,
        Err(error) => {
            // SAFETY: pid is the unreaped child returned by fork.
            unsafe { libc::kill(pid, libc::SIGKILL) };
            let _ = reap_pid_until(
                pid,
                std::time::Instant::now() + std::time::Duration::from_secs(5),
            );
            return Err(error);
        }
    };
    match establish_one_shot_exec_trace(pid, expires, artifact) {
        Ok(None) => {}
        Ok(Some(_)) => {
            return Err("objdump child exited before one-shot exec supervision was armed".into());
        }
        Err(error) => {
            contain_or_abort(pid, pidfd.as_raw_fd(), false);
            return Err(error);
        }
    }
    // SAFETY: ownership of each pipe read descriptor transfers to one File.
    let stdout = unsafe { std::fs::File::from_raw_fd(stdout_read.into_raw_fd()) };
    // SAFETY: ownership of each pipe read descriptor transfers to one File.
    let stderr = unsafe { std::fs::File::from_raw_fd(stderr_read.into_raw_fd()) };
    let mut stdout_thread = Some(bounded_reader(stdout, output_limit, "stdout"));
    let mut stderr_thread = Some(bounded_reader(stderr, output_limit, "stderr"));
    let mut stdout_bytes = None;
    let mut stderr_bytes = None;
    let mut status = None;
    let mut containment_ready = false;
    let mut exec_observed = false;

    loop {
        if let Err(error) = collect_finished_reader(&mut stdout_thread, &mut stdout_bytes, "stdout")
        {
            contain_or_abort(pid, pidfd.as_raw_fd(), status.is_some());
            return Err(error);
        }
        if let Err(error) = collect_finished_reader(&mut stderr_thread, &mut stderr_bytes, "stderr")
        {
            contain_or_abort(pid, pidfd.as_raw_fd(), status.is_some());
            return Err(error);
        }
        if !containment_ready {
            let mut byte = 0_u8;
            // SAFETY: ready_read is a live nonblocking pipe descriptor.
            let count =
                unsafe { libc::read(ready_read.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) };
            if count == 1 {
                if byte != 1 {
                    contain_or_abort(pid, pidfd.as_raw_fd(), status.is_some());
                    return Err("objdump containment handshake was malformed".into());
                }
                containment_ready = true;
            } else if count < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::WouldBlock
                    && error.kind() != std::io::ErrorKind::Interrupted
                {
                    contain_or_abort(pid, pidfd.as_raw_fd(), status.is_some());
                    return Err(error.into());
                }
            }
        }
        if status.is_none() {
            match observe_traced_child(
                pid,
                executable,
                expected_executable,
                artifact,
                &mut exec_observed,
            ) {
                Ok(Some(observed)) => status = Some(observed),
                Ok(None) => {}
                Err(error) => {
                    contain_or_abort(pid, pidfd.as_raw_fd(), false);
                    return Err(error);
                }
            }
        }
        if status.is_some() && !containment_ready {
            return Err("objdump exited before the containment handshake".into());
        }
        if status.is_some()
            && containment_ready
            && exec_observed
            && stdout_bytes.is_some()
            && stderr_bytes.is_some()
        {
            break;
        }
        if status.is_some() && !exec_observed {
            return Err("objdump exited without the exact one-shot exec event".into());
        }
        if std::time::Instant::now() >= expires {
            contain_or_abort(pid, pidfd.as_raw_fd(), status.is_some());
            return Err("llvm-objdump exceeded its execution deadline".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    Ok(std::process::Output {
        status: status.expect("completed child has an exit status"),
        stdout: stdout_bytes.expect("completed stdout reader returned bytes"),
        stderr: stderr_bytes.expect("completed stderr reader returned bytes"),
    })
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
struct PrivateExecutableMaterialization {
    file: std::fs::File,
    byte_len: u64,
    device: u64,
    inode: u64,
    digest: [u8; 32],
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
impl PrivateExecutableMaterialization {
    fn new(bytes: &[u8]) -> Result<Self, BoxError> {
        require(
            bytes.starts_with(b"\x7fELF") && bytes.len() as u64 <= MAX_LLVM_OBJDUMP_BYTES,
            "llvm-objdump must be a bounded ELF executable image",
        )?;
        // SAFETY: memfd_create returns a fresh descriptor or a negative error.
        let descriptor = unsafe {
            libc::syscall(
                libc::SYS_memfd_create,
                c"fe2o3-row-softmax-v1-objdump".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        } as libc::c_int;
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: memfd_create returned a fresh owned descriptor.
        let mut writable = unsafe { std::fs::File::from_raw_fd(descriptor) };
        writable.write_all(bytes)?;
        writable.flush()?;
        writable.sync_all()?;
        // SAFETY: fchmod and fcntl operate on this live private descriptor.
        if unsafe { libc::fchmod(writable.as_raw_fd(), 0o500) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let required_seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        // SAFETY: F_ADD_SEALS accepts the integer seal mask for this memfd.
        if unsafe { libc::fcntl(writable.as_raw_fd(), libc::F_ADD_SEALS, required_seals) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let written_metadata = writable.metadata()?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(format!("/proc/self/fd/{}", writable.as_raw_fd()))?;
        let metadata = file.metadata()?;
        require(
            metadata.dev() == written_metadata.dev()
                && metadata.ino() == written_metadata.ino()
                && metadata.len() == written_metadata.len(),
            "sealed llvm-objdump identity changed while reopening it read-only",
        )?;
        drop(writable);
        let materialized = Self {
            file,
            byte_len: bytes.len() as u64,
            device: metadata.dev(),
            inode: metadata.ino(),
            digest: sha256(bytes),
        };
        materialized.verify(bytes)?;
        Ok(materialized)
    }

    fn verify(&self, expected: &[u8]) -> Result<(), BoxError> {
        let retained_metadata = self.file.metadata()?;
        // SAFETY: F_GET_SEALS and F_GETFL inspect this live retained descriptor.
        let seals = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GET_SEALS) };
        let status = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GETFL) };
        let required_seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        require(
            seals >= 0
                && seals & required_seals == required_seals
                && status >= 0
                && status & libc::O_ACCMODE == libc::O_RDONLY
                && retained_metadata.len() == self.byte_len
                && retained_metadata.dev() == self.device
                && retained_metadata.ino() == self.inode
                && retained_metadata.permissions().mode() & 0o777 == 0o500,
            "sealed llvm-objdump descriptor identity or permissions changed",
        )?;
        let mut retained = self.file.try_clone()?;
        retained.seek(SeekFrom::Start(0))?;
        let mut retained_bytes = Vec::new();
        retained.read_to_end(&mut retained_bytes)?;
        require(
            retained_bytes == expected
                && sha256(&retained_bytes) == self.digest
                && sha256(expected) == self.digest,
            "sealed llvm-objdump descriptor bytes changed",
        )
    }

    fn output<I, S>(&self, expected: &[u8], arguments: I) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.output_with_limits(
            expected,
            arguments,
            LLVM_OBJDUMP_DEADLINE,
            MAX_LLVM_OBJDUMP_OUTPUT_BYTES,
        )
    }

    fn output_with_limits<I, S>(
        &self,
        expected: &[u8],
        arguments: I,
        deadline: std::time::Duration,
        output_limit: usize,
    ) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.output_with_artifact_and_limits(expected, None, arguments, deadline, output_limit)
    }

    fn output_with_artifact<I, S>(
        &self,
        expected: &[u8],
        artifact: &PrivateArtifactMaterialization,
        arguments: I,
    ) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.output_with_artifact_and_limits(
            expected,
            Some(artifact),
            arguments,
            LLVM_OBJDUMP_DEADLINE,
            MAX_LLVM_OBJDUMP_OUTPUT_BYTES,
        )
    }

    fn output_with_artifact_and_limits<I, S>(
        &self,
        expected: &[u8],
        artifact: Option<&PrivateArtifactMaterialization>,
        arguments: I,
        deadline: std::time::Duration,
        output_limit: usize,
    ) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.verify(expected)?;
        if let Some(artifact) = artifact {
            artifact.verify_retained()?;
        }
        let output =
            bounded_descriptor_output(self, expected, artifact, arguments, deadline, output_limit)?;
        self.verify(expected)?;
        if let Some(artifact) = artifact {
            artifact.verify_retained()?;
        }
        Ok(output)
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn create_private_directory(label: &str) -> Result<std::path::PathBuf, BoxError> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    for attempt in 0..64_u32 {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-row-softmax-v1-{label}-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => {
                let metadata = std::fs::symlink_metadata(&path)?;
                require(
                    metadata.file_type().is_dir()
                        && !metadata.file_type().is_symlink()
                        && metadata.permissions().mode() & 0o777 == 0o700,
                    "private materialization directory has invalid identity or permissions",
                )?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("could not create a fresh private materialization directory".into())
}

#[cfg(feature = "hardware-test-hooks")]
fn canonical_regular_file(variable: &str) -> Result<std::path::PathBuf, BoxError> {
    let path = std::path::PathBuf::from(
        std::env::var_os(variable).ok_or_else(|| format!("{variable} is not set"))?,
    );
    require(path.is_absolute(), format!("{variable} must be absolute"))?;
    let metadata = std::fs::symlink_metadata(&path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        format!("{variable} must name a regular non-symlink file"),
    )?;
    require(
        std::fs::canonicalize(&path)? == path,
        format!("{variable} must already be canonical"),
    )?;
    Ok(path)
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_objdump() -> Result<PinnedObjdump, BoxError> {
    let path = canonical_regular_file("FE2O3_LLVM_OBJDUMP")?;
    let metadata = std::fs::metadata(&path)?;
    require(
        (1..=MAX_LLVM_OBJDUMP_BYTES).contains(&metadata.len())
            && metadata.permissions().mode() & 0o111 != 0,
        "FE2O3_LLVM_OBJDUMP has an invalid bounded executable shape",
    )?;
    let expected = parse_exact_sha256(
        "FE2O3_LLVM_OBJDUMP_SHA256",
        &std::env::var("FE2O3_LLVM_OBJDUMP_SHA256")
            .map_err(|_| "FE2O3_LLVM_OBJDUMP_SHA256 is not set")?,
    )?;
    let bytes = std::fs::read(&path)?;
    require(
        bytes.len() as u64 == metadata.len() && sha256(&bytes) == expected,
        "FE2O3_LLVM_OBJDUMP does not match its exact SHA-256 pin",
    )?;
    let final_metadata = std::fs::symlink_metadata(&path)?;
    require(
        final_metadata.file_type().is_file()
            && !final_metadata.file_type().is_symlink()
            && final_metadata.len() == metadata.len()
            && final_metadata.dev() == metadata.dev()
            && final_metadata.ino() == metadata.ino()
            && final_metadata.permissions().mode() & 0o111 != 0
            && std::fs::canonicalize(&path)? == path,
        "FE2O3_LLVM_OBJDUMP caller pathname changed while its bytes were captured",
    )?;
    let executable = PrivateExecutableMaterialization::new(&bytes)?;
    let mut tool = PinnedObjdump {
        digest: expected,
        bytes,
        executable,
        observed_version: String::new(),
    };
    tool.verify_bytes()?;
    tool.observed_version = tool.observe_version()?;
    tool.verify_bytes()?;
    Ok(tool)
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_artifact() -> Result<PinnedArtifact, BoxError> {
    require(
        std::env::var("FE2O3_RUN_GFX942_ROW_SOFTMAX_V1_HARDWARE").as_deref() == Ok("1"),
        "set FE2O3_RUN_GFX942_ROW_SOFTMAX_V1_HARDWARE=1 to opt into this non-authoritative test",
    )?;
    let kernel_symbol = std::env::var("FE2O3_GFX942_ROW_SOFTMAX_V1_KERNEL_SYMBOL")
        .map_err(|_| "FE2O3_GFX942_ROW_SOFTMAX_V1_KERNEL_SYMBOL is not set")?;
    require(
        kernel_symbol == ROW_SOFTMAX_V1_EXPORT,
        format!("FE2O3_GFX942_ROW_SOFTMAX_V1_KERNEL_SYMBOL must equal `{ROW_SOFTMAX_V1_EXPORT}`"),
    )?;
    let path = canonical_regular_file("FE2O3_GFX942_ROW_SOFTMAX_V1_HSACO")?;
    let metadata = std::fs::metadata(&path)?;
    require(
        (1..=fe2o3_hsaco::MAX_HSACO_BYTES as u64).contains(&metadata.len()),
        "FE2O3_GFX942_ROW_SOFTMAX_V1_HSACO has an invalid byte length",
    )?;
    let expected = parse_exact_sha256(
        "FE2O3_GFX942_ROW_SOFTMAX_V1_SHA256",
        &std::env::var("FE2O3_GFX942_ROW_SOFTMAX_V1_SHA256")
            .map_err(|_| "FE2O3_GFX942_ROW_SOFTMAX_V1_SHA256 is not set")?,
    )?;
    let bytes = std::fs::read(&path)?;
    require(
        bytes.len() as u64 == metadata.len(),
        "Row Softmax V1 HSACO changed size while being read",
    )?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    require(
        digest.bytes().as_bytes() == &expected,
        "Row Softmax V1 HSACO does not match its exact SHA-256 pin",
    )?;
    let final_metadata = std::fs::symlink_metadata(&path)?;
    require(
        final_metadata.file_type().is_file()
            && !final_metadata.file_type().is_symlink()
            && final_metadata.len() == metadata.len()
            && final_metadata.dev() == metadata.dev()
            && final_metadata.ino() == metadata.ino()
            && final_metadata.nlink() == 1
            && std::fs::canonicalize(&path)? == path,
        "Row Softmax V1 caller-supplied HSACO path changed while being read",
    )?;
    let objdump = read_pinned_objdump()?;
    Ok(PinnedArtifact {
        bytes,
        digest,
        kernel_symbol,
        objdump,
    })
}

#[cfg(feature = "hardware-test-hooks")]
fn inspect_observational_isa_shape(
    artifact: &PinnedArtifact,
    entry: BoundKernelEntry,
) -> Result<(), BoxError> {
    let materialized = PrivateArtifactMaterialization::new(&artifact.bytes)?;
    let arguments = [
        std::ffi::OsString::from(format!("--disassemble-symbols={}", artifact.kernel_symbol)),
        std::ffi::OsString::from(format!("--start-address=0x{:x}", entry.address)),
        std::ffi::OsString::from(format!("--stop-address=0x{:x}", entry.end()?)),
        std::ffi::OsString::from("--mcpu=gfx942"),
        std::ffi::OsString::from(format!("/proc/self/fd/{ARTIFACT_DESCRIPTOR}")),
    ];
    let output = artifact
        .objdump
        .output_with_artifact(&materialized, arguments)?;
    require(
        output.status.success() && output.stderr.is_empty(),
        format!(
            "digest-pinned observed LLVM 22 llvm-objdump rejected Row Softmax V1: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    let disassembly = String::from_utf8(output.stdout)
        .map_err(|_| "digest-pinned llvm-objdump emitted non-UTF-8 output")?;
    validate_observational_isa_shape(&disassembly, &artifact.kernel_symbol, entry)?;
    materialized.verify(&artifact.bytes)?;
    artifact.objdump.verify_bytes()?;
    require(
        artifact.objdump.observe_version()? == artifact.objdump.observed_version,
        "digest-pinned llvm-objdump observed version changed across inspection",
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: `f32` has no invalid bit patterns and the byte extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_values(bytes: &[u8]) -> Result<Vec<f32>, BoxError> {
    require(
        bytes.len().is_multiple_of(std::mem::size_of::<f32>()),
        "hardware-test allocation contains a partial f32",
    )?;
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().expect("exact f32 chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn body_address(buffer: &ReviewedHsaHardwareTestBufferV1) -> Result<u64, BoxError> {
    require(
        buffer.byte_len() == (ELEMENTS + 2 * CANARY_ELEMENTS) * std::mem::size_of::<f32>(),
        "guarded hardware allocation has the wrong physical extent",
    )?;
    let address = buffer.device_address(CANARY_ELEMENTS * std::mem::size_of::<f32>())?;
    require(
        address.is_multiple_of(std::mem::align_of::<f32>() as u64),
        "guarded f32 body address is misaligned",
    )?;
    Ok(address)
}

#[cfg(feature = "hardware-test-hooks")]
struct RuntimeKernarg {
    pointer: std::ptr::NonNull<u8>,
    layout: std::alloc::Layout,
}

#[cfg(feature = "hardware-test-hooks")]
impl RuntimeKernarg {
    fn new() -> Result<Self, BoxError> {
        let layout = std::alloc::Layout::from_size_align(
            COMPLETE_KERNARG_BYTES,
            HSA_KERNARG_ALIGNMENT as usize,
        )?;
        // SAFETY: `layout` is valid and this owner deallocates the result once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("failed to allocate runtime-aligned Row Softmax V1 kernarg")?;
        Ok(Self { pointer, layout })
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: the allocation is live and exactly `layout.size()` bytes.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}

#[cfg(feature = "hardware-test-hooks")]
impl Drop for RuntimeKernarg {
    fn drop(&mut self) {
        // SAFETY: this owner deallocates the exact live allocation once.
        unsafe { std::alloc::dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

#[cfg(feature = "hardware-test-hooks")]
unsafe fn dispatch_one_row(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    kernel_symbol: &str,
    explicit: &[u8; EXPLICIT_KERNARG_BYTES],
) -> Result<(), BoxError> {
    require(
        resolution.export_symbol() == kernel_symbol
            && resolution.kernarg_segment_size() == COMPLETE_KERNARG_BYTES as u64
            && resolution.kernarg_segment_alignment() == HSA_KERNARG_ALIGNMENT,
        "runtime resolution differs from the exact Row Softmax V1 export and kernarg",
    )?;
    let geometry = launch_geometry();
    require(
        geometry.grid() == [1, 1, 1] && geometry.workgroup() == [WORKGROUP_X, 1, 1],
        "one-row launch must be one block count expanded by the adapter to 64 AQL work-items",
    )?;
    let mut storage = RuntimeKernarg::new()?;
    let kernarg = storage.bytes_mut();
    kernarg[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);

    // SAFETY: the exact digest-pinned single-kernel COV6 image was structurally
    // and textually inspected. Two live guarded allocations supply the frozen
    // 32-byte two-slice ABI, all 256 hidden bytes are initialized by the adapter,
    // and this call waits synchronously before any token or allocation is used
    // again. This raw boundary does not authenticate compiler prerequisites.
    unsafe {
        adapter.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            EXPLICIT_KERNARG_BYTES,
            EXPLICIT_KERNARG_BYTES,
            COV6_IMPLICIT_KERNARG_BYTES,
            kernarg,
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(
            completion.completed(),
            "Row Softmax V1 dispatch did not complete synchronously",
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn execute_one_row(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    kernel_symbol: &str,
) -> Result<(), BoxError> {
    for (case_index, input_body) in representative_inputs().into_iter().enumerate() {
        let expected_output = softmax_oracle(&input_body)?;
        let input_host = guarded_f32(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
        let output_host = guarded_f32(&[OUTPUT_POISON; ELEMENTS], OUTPUT_PREFIX, OUTPUT_SUFFIX);
        let input = adapter.allocate_hardware_test_buffer(f32_bytes(&input_host))?;
        let output = adapter.allocate_hardware_test_buffer(f32_bytes(&output_host))?;
        let input_address = body_address(&input)?;
        let output_address = body_address(&output)?;
        let body_bytes = (ELEMENTS * std::mem::size_of::<f32>()) as u64;
        let input_end = input_address
            .checked_add(body_bytes)
            .ok_or("input device address range overflowed")?;
        let output_end = output_address
            .checked_add(body_bytes)
            .ok_or("output device address range overflowed")?;
        require(
            input_end <= output_address || output_end <= input_address,
            "Row Softmax V1 input and output bodies must be disjoint",
        )?;
        let explicit = explicit_kernarg([input_address, output_address]);

        // SAFETY: `dispatch_one_row` owns and documents the only raw launch
        // boundary and returns only after both allocations are synchronously idle.
        unsafe {
            dispatch_one_row(
                adapter,
                executable,
                kernel,
                resolution,
                kernel_symbol,
                &explicit,
            )?;
        }

        let input_after = f32_values(&input.read_after_synchronous_dispatch())?;
        let output_after = f32_values(&output.read_after_synchronous_dispatch())?;
        verify_guarded_input(&input_after, &input_body)
            .map_err(|error| format!("case {case_index}: {error}"))?;
        verify_guarded_output(&output_after, &expected_output)
            .map_err(|error| format!("case {case_index}: {error}"))?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_non_authoritative_hardware_evidence(artifact: PinnedArtifact) -> Result<(), BoxError> {
    let entry = inspect_metadata(&artifact.bytes, &artifact.kernel_symbol)?;
    inspect_observational_isa_shape(&artifact, entry)?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    let physical_target = adapter.environment().physical_device().target();
    require(
        physical_target.processor() == "gfx942"
            && physical_target.xnack() == Some(FeatureState::Disabled),
        "Row Softmax V1 hardware evidence requires a gfx942:xnack- physical device",
    )?;

    // SAFETY: exact immutable bytes are SHA-256 pinned, metadata/descriptor/ISA
    // inspected, and retained through the sole terminal unload. This remains a
    // non-authoritative test because it does not authenticate the producer.
    let (executable, load) = unsafe { adapter.load_executable(&artifact.bytes, artifact.digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(
            load.finalized_digest() == artifact.digest
                && load.byte_len() == artifact.bytes.len() as u64,
            "loaded Row Softmax V1 bytes differ from the pinned artifact",
        )?;
        // SAFETY: inspection checked exactly one matching export and descriptor.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [artifact.kernel_symbol.as_str()]) }?;
        require(
            kernels.len() == 1
                && resolutions.len() == 1
                && resolutions[0].export_symbol() == artifact.kernel_symbol
                && resolutions[0].executable_object() == executable_identity,
            "runtime resolved a substituted Row Softmax V1 kernel",
        )?;
        let kernel = kernels
            .get(0)
            .ok_or("runtime omitted the resolved Row Softmax V1 kernel")?;
        execute_one_row(
            &mut adapter,
            &executable,
            kernel,
            &resolutions[0],
            &artifact.kernel_symbol,
        )
    })();

    // Kernel tokens are dropped before the sole terminal consuming unload.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(
        unload.released() && unload.executable_object() == executable_identity,
        "reviewed HSA unload did not release the exact Row Softmax V1 executable",
    )?;
    execution
}

/// Executes eight representative 64-element FP32 Row Softmax V1 rows.
///
/// This ignored test bypasses production prerequisite authentication and grants
/// no protected evidence. Its guards detect only changed values within each
/// finite guarded allocation. They do not detect beyond-guard accesses,
/// value-preserving writes, same-value races, or output-inert reads. Its finite
/// numerical checks do not bind the HSACO to Rust source or a Verus proof, prove
/// race freedom, grant publication/load/launch authority, or establish exact
/// real-number softmax. The separate mnemonic observation proves no CFG,
/// dataflow, reachability, natural-exp scaling, or semantic use. NaN and
/// infinity are outside this V1 evidence policy.
///
/// ```text
/// FE2O3_RUN_GFX942_ROW_SOFTMAX_V1_HARDWARE=1 \
/// FE2O3_GFX942_ROW_SOFTMAX_V1_HSACO=/absolute/canonical/row-softmax-v1.hsaco \
/// FE2O3_GFX942_ROW_SOFTMAX_V1_SHA256=<64-lowercase-hex-digits> \
/// FE2O3_GFX942_ROW_SOFTMAX_V1_KERNEL_SYMBOL=row_softmax_v1 \
/// FE2O3_LLVM_OBJDUMP=/absolute/canonical/llvm-objdump \
/// FE2O3_LLVM_OBJDUMP_SHA256=<64-lowercase-hex-digits> \
/// cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test row_softmax_v1_hardware \
///   gfx942_row_softmax_v1_one_row_raw_hardware_evidence \
///   -- --ignored --exact --nocapture
/// ```
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "non-authoritative: requires exact pinned COV6 HSACO, observed LLVM 22 objdump, and gfx942:xnack-"]
fn gfx942_row_softmax_v1_one_row_raw_hardware_evidence() -> Result<(), BoxError> {
    run_non_authoritative_hardware_evidence(read_pinned_artifact()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    static DESCRIPTOR_EXECUTION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn valid_entry() -> BoundKernelEntry {
        BoundKernelEntry {
            address: 0x1000,
            file_offset: 0x200,
            size: 24,
        }
    }

    fn valid_disassembly() -> String {
        format!(
            "private.hsaco:\tfile format elf64-amdgpu\n\n\
             Disassembly of section .text:\n\n\
             0000000000001000 <{ROW_SOFTMAX_V1_EXPORT}>:\n\
             \tglobal_load_dword v0, v[0:1], off // 000000001000: DEADBEEF\n\
             \tv_exp_f32 v0, v0 // 000000001004: DEADBEEF\n\
             \tglobal_store_dword v[0:1], v0, off // 000000001008: DEADBEEF\n\
             \ts_endpgm // 00000000100C: DEADBEEF\n\
             \ts_nop 0 // 000000001010: DEADBEEF\n\
             \ts_nop 0 // 000000001014: DEADBEEF\n"
        )
    }

    #[test]
    fn launch_uses_one_block_count_and_one_wave64() {
        let geometry = launch_geometry();
        assert_eq!(geometry.grid(), [1, 1, 1]);
        assert_eq!(geometry.workgroup(), [64, 1, 1]);
        assert_eq!(geometry.dynamic_shared_memory_bytes(), 0);
        let packed = explicit_kernarg([0x11, 0x22]);
        for (index, address) in [0x11_u64, 0x22].into_iter().enumerate() {
            let offset = index * 16;
            assert_eq!(&packed[offset..offset + 8], &address.to_le_bytes());
            assert_eq!(&packed[offset + 8..offset + 16], &64_u64.to_le_bytes());
        }
        assert_eq!(EXPLICIT_KERNARG_BYTES, 32);
        assert_eq!(COMPLETE_KERNARG_BYTES, 288);
    }

    #[test]
    fn metadata_validator_rejects_profile_substitutions() {
        let expected = MetadataFacts::expected(ROW_SOFTMAX_V1_EXPORT);
        validate_metadata(&expected, ROW_SOFTMAX_V1_EXPORT).unwrap();

        let mut qualified = expected.clone();
        for slice in 0..2 {
            let pointer = &mut qualified.arguments[slice * 2];
            pointer.alignment = Some(8);
            pointer.value_type = Some(ExplicitValueType::F32);
            pointer.pointee_alignment = Some(4);
            pointer.access = Some(if slice == 0 {
                ArgumentAccess::ReadOnly
            } else {
                ArgumentAccess::ReadWrite
            });
            pointer.actual_access = pointer.access;
            pointer.is_const = Some(slice == 0);
            pointer.is_restrict = Some(slice == 1);
            pointer.is_volatile = Some(false);
            pointer.is_pipe = Some(false);
            let length = &mut qualified.arguments[slice * 2 + 1];
            length.alignment = Some(8);
            length.value_type = Some(ExplicitValueType::U64);
            length.is_const = Some(false);
            length.is_restrict = Some(false);
            length.is_volatile = Some(false);
            length.is_pipe = Some(false);
        }
        validate_metadata(&qualified, ROW_SOFTMAX_V1_EXPORT).unwrap();

        for (cluster_dims, wgp_mode) in [
            (Some([1, 1, 1]), None),
            (None, Some(false)),
            (Some([1, 1, 1]), Some(false)),
        ] {
            let mut equivalent = expected.clone();
            equivalent.cluster_dims = cluster_dims;
            equivalent.workgroup_processor_mode = wgp_mode;
            validate_metadata(&equivalent, ROW_SOFTMAX_V1_EXPORT).unwrap();
        }

        let mut expanded_four_slice = expected.clone();
        expanded_four_slice.kernarg_size = 320;
        expanded_four_slice.implicit_offset = Some(64);
        expanded_four_slice
            .arguments
            .extend(expected.arguments.clone());
        expanded_four_slice.descriptor_kernarg_size = 320;
        assert!(validate_metadata(&expanded_four_slice, ROW_SOFTMAX_V1_EXPORT).is_err());

        let mutations: &[fn(&mut MetadataFacts)] = &[
            |facts| facts.code_object_version = 5,
            |facts| facts.target = "gfx942:xnack+".to_owned(),
            |facts| facts.has_printf_metadata = true,
            |facts| facts.kernel_count = 2,
            |facts| facts.kernel_name = "substituted".to_owned(),
            |facts| facts.kernarg_size = 320,
            |facts| facts.kernarg_alignment = 16,
            |facts| facts.implicit_offset = Some(64),
            |facts| facts.implicit_size = 0,
            |facts| facts.required_workgroup = Some([1, 1, 1]),
            |facts| facts.max_flat_workgroup = 1024,
            |facts| facts.max_workgroups[0] = Some(2),
            |facts| facts.max_workgroups[1] = None,
            |facts| facts.max_workgroups[2] = Some(u32::MAX),
            |facts| facts.cluster_dims = Some([2, 1, 1]),
            |facts| facts.uniform_workgroup = false,
            |facts| facts.workgroup_processor_mode = Some(true),
            |facts| facts.wavefront_size = 32,
            |facts| facts.group_segment_size = 1024,
            |facts| facts.private_segment_size = 4,
            |facts| facts.normal_kernel = false,
            |facts| facts.sgpr_spill_count = 1,
            |facts| facts.vgpr_spill_count = 1,
            |facts| facts.arguments[3].kind = ExplicitValueKind::GlobalBuffer,
            |facts| facts.arguments[0].address_space = None,
            |facts| facts.binding_count = 0,
            |facts| facts.binding_kernel_index = 1,
            |facts| facts.descriptor_kernarg_size = 320,
            |facts| facts.descriptor_private_segment_enabled = true,
            |facts| facts.descriptor_uses_dynamic_stack = true,
        ];
        for mutate in mutations {
            let mut hostile = expected.clone();
            mutate(&mut hostile);
            assert!(validate_metadata(&hostile, ROW_SOFTMAX_V1_EXPORT).is_err());
        }

        let mut shortened_hidden = expected.clone();
        shortened_hidden.hidden_arguments.pop();
        assert!(validate_metadata(&shortened_hidden, ROW_SOFTMAX_V1_EXPORT).is_err());
        for extra_kind in [
            HiddenValueKind::DynamicLdsSize,
            HiddenValueKind::HostcallBuffer,
        ] {
            let mut extra = expected.clone();
            extra.hidden_arguments.push(HiddenArgumentFact {
                offset: if extra_kind == HiddenValueKind::DynamicLdsSize {
                    152
                } else {
                    128
                },
                size: if extra_kind == HiddenValueKind::DynamicLdsSize {
                    4
                } else {
                    8
                },
                kind: extra_kind,
            });
            assert!(validate_metadata(&extra, ROW_SOFTMAX_V1_EXPORT).is_err());
        }
        for mutate in [
            |record: &mut HiddenArgumentFact| record.offset += 1,
            |record: &mut HiddenArgumentFact| record.size *= 2,
            |record: &mut HiddenArgumentFact| record.kind = HiddenValueKind::DynamicLdsSize,
        ] {
            let mut hostile = expected.clone();
            mutate(&mut hostile.hidden_arguments[6]);
            assert!(validate_metadata(&hostile, ROW_SOFTMAX_V1_EXPORT).is_err());
        }

        let argument_mutations: &[fn(&mut MetadataFacts)] = &[
            |facts| facts.arguments[0].name = Some("input.data".to_owned()),
            |facts| facts.arguments[1].name = None,
            |facts| facts.arguments[0].type_name = Some("float*".to_owned()),
            |facts| facts.arguments[0].alignment = Some(4),
            |facts| facts.arguments[1].alignment = Some(4),
            |facts| facts.arguments[0].value_type = Some(ExplicitValueType::U64),
            |facts| facts.arguments[1].value_type = Some(ExplicitValueType::F32),
            |facts| facts.arguments[0].access = Some(ArgumentAccess::ReadWrite),
            |facts| facts.arguments[2].access = Some(ArgumentAccess::ReadOnly),
            |facts| facts.arguments[0].actual_access = Some(ArgumentAccess::ReadWrite),
            |facts| facts.arguments[2].actual_access = Some(ArgumentAccess::ReadOnly),
            |facts| facts.arguments[0].pointee_alignment = Some(8),
            |facts| facts.arguments[1].pointee_alignment = Some(4),
            |facts| facts.arguments[0].is_const = Some(false),
            |facts| facts.arguments[2].is_const = Some(true),
            |facts| facts.arguments[0].is_restrict = Some(true),
            |facts| facts.arguments[2].is_restrict = Some(false),
            |facts| facts.arguments[0].is_volatile = Some(true),
            |facts| facts.arguments[2].is_pipe = Some(true),
            |facts| facts.arguments[1].access = Some(ArgumentAccess::ReadOnly),
            |facts| facts.arguments[3].address_space = Some(ArgumentAddressSpace::Global),
            |facts| facts.arguments[1].is_const = Some(true),
            |facts| facts.arguments[3].is_restrict = Some(true),
        ];
        for mutate in argument_mutations {
            let mut hostile = expected.clone();
            mutate(&mut hostile);
            assert!(validate_metadata(&hostile, ROW_SOFTMAX_V1_EXPORT).is_err());
        }
    }

    #[test]
    fn observational_isa_shape_rejects_missing_effects_and_forbidden_families() {
        let valid = valid_disassembly();
        validate_observational_isa_shape(&valid, ROW_SOFTMAX_V1_EXPORT, valid_entry()).unwrap();

        for hostile in [
            valid.replace("global_store_dword", "v_mov_b32"),
            valid.replace("global_load_dword", "v_mov_b32"),
            valid.replace("v_exp_f32", "v_mov_b32"),
            valid.replace("s_endpgm", "s_call_b64 s[0:1]"),
            valid.replace("s_endpgm", "scratch_store_dword off, v0"),
            valid.replace("s_endpgm", "global_atomic_add v0, v1, v2"),
            valid.replace("s_endpgm", "ds_write_b32 v0, v1"),
            valid.replace("v_exp_f32", "v_mfma_f32_16x16x16_bf16"),
            valid.replace("000000001000:", "0000000000001000:"),
        ] {
            assert!(
                validate_observational_isa_shape(&hostile, ROW_SOFTMAX_V1_EXPORT, valid_entry())
                    .is_err()
            );
        }
    }

    #[test]
    fn observational_isa_shape_rejects_exp_owned_only_by_a_helper() {
        let scalar_kernel = valid_disassembly().replace("v_exp_f32 v0, v0", "v_add_f32 v0, v1");
        let helper = "\n0000000000002000 <helper>:\n\
                      \tv_exp_f32 v0, v0 // 000000002000: DEADBEEF\n\
                      \ts_endpgm // 000000002004: DEADBEEF\n";
        let hostile = format!("{scalar_kernel}{helper}");
        assert!(
            validate_observational_isa_shape(&hostile, ROW_SOFTMAX_V1_EXPORT, valid_entry())
                .is_err()
        );
    }

    #[test]
    fn observational_isa_shape_does_not_claim_register_dataflow() {
        let unrelated_registers = valid_disassembly()
            .replace("v_exp_f32 v0, v0", "v_exp_f32 v7, v9")
            .replace(
                "global_store_dword v[0:1], v0",
                "global_store_dword v[4:5], v42",
            );
        validate_observational_isa_shape(
            &unrelated_registers,
            ROW_SOFTMAX_V1_EXPORT,
            valid_entry(),
        )
        .unwrap();
    }

    #[test]
    fn sealed_hsaco_handoff_ignores_same_uid_path_substitution() {
        let _execution = DESCRIPTOR_EXECUTION_LOCK.lock().unwrap();
        let caller_directory = create_private_directory("caller").unwrap();
        let caller_path = caller_directory.join("caller.hsaco");
        let original = b"original digest-pinned HSACO bytes";
        let substituted = b"substituted caller pathname bytes";
        {
            let mut caller = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&caller_path)
                .unwrap();
            caller.write_all(original).unwrap();
            caller.sync_all().unwrap();
        }
        let inspected_bytes = std::fs::read(&caller_path).unwrap();
        let materialized = PrivateArtifactMaterialization::new(&inspected_bytes).unwrap();

        std::fs::write(&caller_path, substituted).unwrap();
        assert_eq!(std::fs::read(&caller_path).unwrap(), substituted);
        materialized.verify(original).unwrap();
        let cat = std::fs::read(std::fs::canonicalize("/bin/cat").unwrap()).unwrap();
        let executable = PrivateExecutableMaterialization::new(&cat).unwrap();
        let output = executable
            .output_with_artifact(
                &cat,
                &materialized,
                [format!("/proc/self/fd/{ARTIFACT_DESCRIPTOR}")],
            )
            .unwrap();
        assert!(output.status.success() && output.stderr.is_empty());
        assert_eq!(output.stdout, original);
        materialized.verify(original).unwrap();

        std::fs::remove_file(caller_path).unwrap();
        std::fs::remove_dir(caller_directory).unwrap();
    }

    #[test]
    fn private_objdump_execution_ignores_caller_path_substitution() {
        let _execution = DESCRIPTOR_EXECUTION_LOCK.lock().unwrap();
        assert!(PrivateExecutableMaterialization::new(b"#!/bin/sh\nexit 0\n").is_err());
        let caller_directory = create_private_directory("caller-objdump").unwrap();
        let caller_path = caller_directory.join("llvm-objdump");
        let substitute_path = caller_directory.join("hostile-objdump");
        let captured = std::fs::read(std::fs::canonicalize("/bin/sh").unwrap()).unwrap();
        let substituted = std::fs::read(std::fs::canonicalize("/bin/false").unwrap()).unwrap();
        for (path, bytes) in [
            (&caller_path, captured.as_slice()),
            (&substitute_path, &substituted),
        ] {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o700)
                .open(path)
                .unwrap();
            file.write_all(bytes).unwrap();
            file.sync_all().unwrap();
        }
        let captured_bytes = std::fs::read(&caller_path).unwrap();
        let executable = PrivateExecutableMaterialization::new(&captured_bytes).unwrap();

        std::fs::rename(&substitute_path, &caller_path).unwrap();
        assert_eq!(std::fs::read(&caller_path).unwrap(), substituted);
        let output = executable
            .output(&captured, ["-c", "printf 'captured descriptor\\n'"])
            .unwrap();
        assert!(output.status.success() && output.stderr.is_empty());
        assert_eq!(output.stdout, b"captured descriptor\n");
        executable.verify(&captured).unwrap();

        std::fs::remove_file(caller_path).unwrap();
        std::fs::remove_dir(caller_directory).unwrap();
    }

    #[test]
    fn private_objdump_execution_enforces_output_bound() {
        let _execution = DESCRIPTOR_EXECUTION_LOCK.lock().unwrap();
        let shell = std::fs::read(std::fs::canonicalize("/bin/sh").unwrap()).unwrap();
        let noisy_executable = PrivateExecutableMaterialization::new(&shell).unwrap();
        let overflow = noisy_executable
            .output_with_limits(
                &shell,
                ["-c", "while :; do printf '0123456789abcdef'; done"],
                std::time::Duration::from_secs(5),
                128,
            )
            .unwrap_err();
        assert!(overflow.to_string().contains("exceeded"));
    }

    #[test]
    fn seccomp_contains_double_fork_setsid_pipe_holder() {
        let _execution = DESCRIPTOR_EXECUTION_LOCK.lock().unwrap();
        let shell = std::fs::read(std::fs::canonicalize("/bin/sh").unwrap()).unwrap();
        let executable = PrivateExecutableMaterialization::new(&shell).unwrap();
        let directory = create_private_directory("escape-marker").unwrap();
        let marker = directory.join("escaped");
        let command = format!(
            "( ( /usr/bin/setsid /bin/sh -c 'printf escaped > {}; /bin/sleep 30' & ) & ); \
             printf containment-returned",
            marker.display()
        );
        let output = executable
            .output_with_limits(
                &shell,
                ["-c", command.as_str()],
                std::time::Duration::from_secs(10),
                4096,
            )
            .unwrap();
        assert!(!marker.exists());
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn one_shot_exec_trace_rejects_rebind_and_second_execveat() {
        let _execution = DESCRIPTOR_EXECUTION_LOCK.lock().unwrap();
        let python_path = ["/usr/bin/python3", "/usr/local/bin/python3"]
            .into_iter()
            .find(|path| std::path::Path::new(path).is_file())
            .expect("one-shot exec adversary requires python3");
        let python = std::fs::read(std::fs::canonicalize(python_path).unwrap()).unwrap();
        let executable = PrivateExecutableMaterialization::new(&python).unwrap();
        let script = format!(
            "import ctypes, os\n\
             target = os.open('/bin/true', os.O_RDONLY)\n\
             os.dup2(target, {EXECUTABLE_DESCRIPTOR})\n\
             argv = (ctypes.c_char_p * 2)(b'true', None)\n\
             envp = (ctypes.c_char_p * 1)(None)\n\
             libc = ctypes.CDLL(None, use_errno=True)\n\
             result = libc.syscall({}, {EXECUTABLE_DESCRIPTOR}, b'', argv, envp, {})\n\
             raise OSError(ctypes.get_errno(), 'second execveat', result)\n",
            libc::SYS_execveat,
            libc::AT_EMPTY_PATH,
        );
        let error = executable
            .output_with_limits(
                &python,
                ["-c", script.as_str()],
                std::time::Duration::from_secs(10),
                4096,
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("forbidden second executable replacement"),
            "{error}"
        );
        executable.verify(&python).unwrap();
    }

    #[test]
    fn relocated_sources_survive_reserved_descriptor_collisions_under_fd_pressure() {
        let _execution = DESCRIPTOR_EXECUTION_LOCK.lock().unwrap();
        let mut pressure = Vec::new();
        loop {
            let file = std::fs::File::open("/dev/null").unwrap();
            let descriptor = file.as_raw_fd();
            assert!(descriptor <= READY_DESCRIPTOR);
            if descriptor == READY_DESCRIPTOR {
                drop(file);
                break;
            }
            pressure.push(file);
        }

        let cat = std::fs::read(std::fs::canonicalize("/bin/cat").unwrap()).unwrap();
        let mut executable = PrivateExecutableMaterialization::new(&cat).unwrap();
        let payload = b"reserved descriptor collision survived\n";
        let mut artifact = PrivateArtifactMaterialization::new(payload).unwrap();
        // SAFETY: the pressure guard owns every lower descriptor. Moving these
        // two retained objects deliberately creates the historical cross-source
        // collision without replacing an unrelated descriptor.
        unsafe {
            assert_eq!(
                libc::dup3(
                    executable.file.as_raw_fd(),
                    READY_DESCRIPTOR,
                    libc::O_CLOEXEC,
                ),
                READY_DESCRIPTOR
            );
            executable.file = std::fs::File::from_raw_fd(READY_DESCRIPTOR);
            assert_eq!(
                libc::dup3(
                    artifact.file.as_raw_fd(),
                    EXECUTABLE_DESCRIPTOR,
                    libc::O_CLOEXEC,
                ),
                EXECUTABLE_DESCRIPTOR
            );
            artifact.file = std::fs::File::from_raw_fd(EXECUTABLE_DESCRIPTOR);
        }
        assert_eq!(executable.file.as_raw_fd(), READY_DESCRIPTOR);
        assert_eq!(artifact.file.as_raw_fd(), EXECUTABLE_DESCRIPTOR);
        executable.verify(&cat).unwrap();
        artifact.verify(payload).unwrap();

        let output = executable
            .output_with_artifact(
                &cat,
                &artifact,
                [format!("/proc/self/fd/{ARTIFACT_DESCRIPTOR}")],
            )
            .unwrap();
        assert!(output.status.success() && output.stderr.is_empty());
        assert_eq!(output.stdout, payload);
        drop(pressure);
    }

    #[test]
    fn observed_objdump_version_requires_llvm_major_22() {
        assert_eq!(
            observed_llvm_22_version(b"AMD LLVM version 22.0.0git\n").unwrap(),
            "AMD LLVM version 22.0.0git"
        );
        for hostile in [
            b"AMD LLVM version 21.0.0\n".as_slice(),
            b"AMD LLVM version 122.0.0\n",
            b"LLVM version 22\n",
            b"unidentified objdump 22.0.0\n",
        ] {
            assert!(observed_llvm_22_version(hostile).is_err());
        }
    }

    #[test]
    fn canary_validators_detect_prefix_body_and_suffix_corruption() {
        let input_body = [0.0; ELEMENTS];
        let input = guarded_f32(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
        verify_guarded_input(&input, &input_body).unwrap();
        for index in [0, CANARY_ELEMENTS, input.len() - 1] {
            let mut hostile = input.clone();
            hostile[index] = f32::from_bits(hostile[index].to_bits() ^ 1);
            assert!(verify_guarded_input(&hostile, &input_body).is_err());
        }

        let expected = softmax_oracle(&input_body).unwrap();
        let output = guarded_f32(&expected, OUTPUT_PREFIX, OUTPUT_SUFFIX);
        verify_guarded_output(&output, &expected).unwrap();
        for index in [0, output.len() - 1] {
            let mut hostile = output.clone();
            hostile[index] = f32::from_bits(hostile[index].to_bits() ^ 1);
            assert!(verify_guarded_output(&hostile, &expected).is_err());
        }
        let mut wrong_body = output.clone();
        wrong_body[CANARY_ELEMENTS + 17] += 0.01;
        assert!(verify_guarded_output(&wrong_body, &expected).is_err());
        let mut nan_body = output;
        nan_body[CANARY_ELEMENTS + 17] = f32::NAN;
        assert!(verify_guarded_output(&nan_body, &expected).is_err());
    }

    #[test]
    fn stable_oracle_covers_representative_rows_and_translation_invariance() {
        let inputs = representative_inputs();
        assert_eq!(inputs.len(), 8);
        for input in &inputs {
            let expected = softmax_oracle(input).unwrap();
            assert_eq!(expected.len(), ELEMENTS);
            assert!(
                expected
                    .iter()
                    .all(|value| value.is_finite() && *value >= 0.0)
            );
            assert!((expected.iter().sum::<f32>() - 1.0).abs() <= f32::EPSILON * 4.0);
            verify_softmax_body(&expected, &expected).unwrap();
        }

        let ramp = representative_inputs()[1];
        let translated = ramp.map(|value| value + 1024.0);
        assert_eq!(
            softmax_oracle(&ramp).unwrap(),
            softmax_oracle(&translated).unwrap()
        );

        let uniform = softmax_oracle(&[0.0; ELEMENTS]).unwrap();
        assert!(uniform.iter().all(|value| *value == 1.0 / 64.0));

        let extrema = softmax_oracle(&inputs[4]).unwrap();
        assert_eq!(extrema[3], 1.0);
        assert!(
            extrema
                .iter()
                .enumerate()
                .all(|(index, value)| index == 3 || *value == 0.0)
        );

        let subnormals = softmax_oracle(&inputs[5]).unwrap();
        assert!(subnormals.iter().all(|value| *value == 1.0 / 64.0));

        let untranslated_large: [f32; ELEMENTS] =
            std::array::from_fn(|index| (index % 8) as f32 * 0.125);
        assert_eq!(
            softmax_oracle(&inputs[7]).unwrap(),
            softmax_oracle(&untranslated_large).unwrap()
        );
    }

    #[test]
    fn oracle_and_output_policy_reject_nonfinite_or_malformed_values() {
        assert!(softmax_oracle(&[0.0; ELEMENTS - 1]).is_err());
        for nonfinite in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut input = [0.0; ELEMENTS];
            input[31] = nonfinite;
            assert!(softmax_oracle(&input).is_err());
        }

        let expected = softmax_oracle(&[0.0; ELEMENTS]).unwrap();
        let mut output = expected.clone();
        output[5] = f32::INFINITY;
        assert!(verify_softmax_body(&output, &expected).is_err());
        output[5] = -0.1;
        assert!(verify_softmax_body(&output, &expected).is_err());
    }

    #[test]
    fn digest_parser_requires_exact_lowercase_encoding() {
        assert!(parse_exact_sha256("PIN", &"ab".repeat(32)).is_ok());
        assert!(parse_exact_sha256("PIN", &"AB".repeat(32)).is_err());
        assert!(parse_exact_sha256("PIN", &"0".repeat(63)).is_err());

        let original = b"source-derived row-softmax HSACO";
        let substituted = b"source-derived row-softmax hsaco";
        assert_ne!(sha256(original), sha256(substituted));
    }
}
