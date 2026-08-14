//! Non-authoritative one-tile gfx942 Tiled GEMM V1 hardware evidence.
//!
//! The ignored hardware test executes only an exact SHA-256-pinned COV6 image
//! after checking its physical metadata and digest-pinned, observed LLVM 22
//! disassembly. The disassembly is observational and does not authenticate the
//! tool's provenance. Its executable-byte pin does not pin the dynamic loader,
//! shared libraries, or host process environment.
//! It deliberately bypasses production prerequisite authentication and cannot
//! grant protected compiler or execution evidence.

use fe2o3_host::HsaLaunchGeometryV1;
use fe2o3_hsaco::{ArgumentAddressSpace, ExplicitValueKind};
use sha2::{Digest, Sha256};

#[cfg(any(test, feature = "hardware-test-hooks"))]
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(any(test, feature = "hardware-test-hooks"))]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

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

const TILE: usize = 16;
const ELEMENTS: usize = TILE * TILE;
const WORKGROUP_X: u32 = 64;
const EXPLICIT_KERNARG_BYTES: usize = 64;
const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;
const COMPLETE_KERNARG_BYTES: usize = EXPLICIT_KERNARG_BYTES + COV6_IMPLICIT_KERNARG_BYTES;
const PHYSICAL_KERNARG_ALIGNMENT: u64 = 8;
#[cfg(feature = "hardware-test-hooks")]
const HSA_KERNARG_ALIGNMENT: u64 = 16;
const CANARY_ELEMENTS: usize = 32;
const TILED_GEMM_V1_EXPORT: &str = "tiled_gemm_v1";
const TARGET: &str = "gfx942:xnack-";
#[cfg(any(test, feature = "hardware-test-hooks"))]
const MAX_LLVM_OBJDUMP_BYTES: u64 = 512 * 1024 * 1024;

const A_PREFIX: u16 = 0x7fc1;
const A_SUFFIX: u16 = 0x7fc2;
#[cfg(feature = "hardware-test-hooks")]
const B_PREFIX: u16 = 0x7fd1;
#[cfg(feature = "hardware-test-hooks")]
const B_SUFFIX: u16 = 0x7fd2;
#[cfg(feature = "hardware-test-hooks")]
const C_PREFIX: f32 = f32::from_bits(0x7fc0_c001);
#[cfg(feature = "hardware-test-hooks")]
const C_SUFFIX: f32 = f32::from_bits(0x7fc0_c002);
const D_PREFIX: f32 = f32::from_bits(0x7fc0_d001);
const D_SUFFIX: f32 = f32::from_bits(0x7fc0_d002);
#[cfg(feature = "hardware-test-hooks")]
const D_POISON: f32 = f32::from_bits(0x7fc0_d0ff);

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
            .ok_or_else(|| "Tiled GEMM V1 entry address range overflowed".into())
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
    offset: u64,
    size: u64,
    kind: ExplicitValueKind,
    address_space: Option<ArgumentAddressSpace>,
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
    wavefront_size: u32,
    group_segment_size: u64,
    private_segment_size: u64,
    normal_kernel: bool,
    sgpr_spill_count: u32,
    vgpr_spill_count: u32,
    uses_dynamic_stack: bool,
    arguments: Vec<ArgumentFact>,
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
            wavefront_size: 64,
            group_segment_size: 0,
            private_segment_size: 0,
            normal_kernel: true,
            sgpr_spill_count: 0,
            vgpr_spill_count: 0,
            uses_dynamic_stack: false,
            arguments: [
                (0_u64, ExplicitValueKind::GlobalBuffer),
                (8, ExplicitValueKind::ByValue),
                (16, ExplicitValueKind::GlobalBuffer),
                (24, ExplicitValueKind::ByValue),
                (32, ExplicitValueKind::GlobalBuffer),
                (40, ExplicitValueKind::ByValue),
                (48, ExplicitValueKind::GlobalBuffer),
                (56, ExplicitValueKind::ByValue),
            ]
            .into_iter()
            .map(|(offset, kind)| ArgumentFact {
                offset,
                size: 8,
                kind,
                address_space: if kind == ExplicitValueKind::GlobalBuffer {
                    Some(ArgumentAddressSpace::Global)
                } else {
                    None
                },
            })
            .collect(),
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
    require(
        facts == &MetadataFacts::expected(kernel_symbol),
        format!(
            "Tiled GEMM V1 metadata or descriptor differs from the exact \
             gfx942:xnack- COV6/WG64/320-byte functional slice profile: {facts:#?}"
        ),
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn inspect_metadata(bytes: &[u8], kernel_symbol: &str) -> Result<BoundKernelEntry, BoxError> {
    let bound = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)?;
    let inspection = bound.inspection();
    let kernel = inspection
        .kernels()
        .first()
        .ok_or("Tiled GEMM V1 HSACO declares no kernel")?;
    let binding = bound
        .bindings()
        .first()
        .ok_or("Tiled GEMM V1 HSACO has no descriptor binding")?;
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
                offset: argument.offset(),
                size: argument.size(),
                kind: argument.value_kind(),
                address_space: argument.address_space(),
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
    require(entry.size != 0, "Tiled GEMM V1 ELF entry is empty")?;
    require(
        entry
            .file_offset
            .checked_add(entry.size)
            .is_some_and(|end| end <= bytes.len() as u64),
        "Tiled GEMM V1 ELF entry file range exceeds the pinned HSACO",
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

fn validate_isa(
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
        "llvm-objdump function header differs from the bound Tiled GEMM V1 ELF entry",
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

    let mfma_count = instructions
        .iter()
        .filter(|instruction| instruction.mnemonic == "v_mfma_f32_16x16x16_bf16")
        .count();
    require(
        mfma_count == 1,
        format!("expected exactly one retained BF16 MFMA, found {mfma_count}"),
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("global_load_")),
        "final ISA has no global-load instruction",
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("global_store_")),
        "final ISA has no global-store instruction",
    )?;
    let terminators = instructions
        .iter()
        .enumerate()
        .filter(|(_, instruction)| instruction.mnemonic == "s_endpgm")
        .collect::<Vec<_>>();
    require(
        terminators.len() == 1,
        "final ISA must contain exactly one kernel termination instruction",
    )?;
    require(
        instructions[terminators[0].0 + 1..]
            .iter()
            .all(|instruction| instruction.mnemonic == "s_nop"),
        "bound Tiled GEMM V1 entry contains executable instructions after s_endpgm",
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
            format!("final ISA contains forbidden {description} family `{family}`"),
        )?;
    }
    require(
        !instructions
            .iter()
            .any(|instruction| instruction.mnemonic.contains("atomic")),
        "final ISA contains an atomic instruction",
    )
}

fn bf16_to_f32(bits: u16) -> f32 {
    f32::from_bits(u32::from(bits) << 16)
}

fn tiled_inputs() -> ([u16; ELEMENTS], [u16; ELEMENTS], [f32; ELEMENTS]) {
    const BF16_DYADICS: [u16; 6] = [0x3f80, 0xbf80, 0x3f00, 0xbf00, 0x3e80, 0xbe80];
    const C_DYADICS: [f32; 6] = [31.25, 31.5, 31.75, 32.25, 32.5, 32.75];
    let mut a = [0; ELEMENTS];
    let mut b = [0; ELEMENTS];
    let mut c = [0.0; ELEMENTS];
    for row in 0..TILE {
        for column in 0..TILE {
            let index = row * TILE + column;
            a[index] = BF16_DYADICS[(row + 3 * column) % BF16_DYADICS.len()];
            b[index] = BF16_DYADICS[(2 * row + column + 1) % BF16_DYADICS.len()];
            c[index] = C_DYADICS[(row + 2 * column) % C_DYADICS.len()];
        }
    }
    (a, b, c)
}

/// Computes `D = A * B + C` for the exact row-major 16x16x16 tile.
///
/// A and B are chosen from `+/-{1, 1/2, 1/4}` and C from small quarter
/// multiples near 32. Every product, every partial sum, and every final value
/// is therefore an exactly representable normal FP32 dyadic. The magnitude is
/// bounded well below the FP32 precision limit, so MFMA association and fused
/// evaluation cannot change any result bit for this finite evidence corpus.
fn tiled_oracle(a: &[u16], b: &[u16], c: &[f32]) -> Result<Vec<f32>, BoxError> {
    require(
        a.len() == ELEMENTS && b.len() == ELEMENTS && c.len() == ELEMENTS,
        "Tiled GEMM V1 oracle requires exact 16x16 A, B, and C extents",
    )?;
    let mut output = vec![0.0; ELEMENTS];
    for row in 0..TILE {
        for column in 0..TILE {
            let mut accumulator = c[row * TILE + column];
            for depth in 0..TILE {
                accumulator +=
                    bf16_to_f32(a[row * TILE + depth]) * bf16_to_f32(b[depth * TILE + column]);
            }
            output[row * TILE + column] = accumulator;
        }
    }
    Ok(output)
}

fn guarded_u16(body: &[u16], prefix: u16, suffix: u16) -> Vec<u16> {
    let mut values = Vec::with_capacity(CANARY_ELEMENTS * 2 + body.len());
    values.extend(std::iter::repeat_n(prefix, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, CANARY_ELEMENTS));
    values
}

fn guarded_f32(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(CANARY_ELEMENTS * 2 + body.len());
    values.extend(std::iter::repeat_n(prefix, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, CANARY_ELEMENTS));
    values
}

fn verify_guarded_u16(
    role: &str,
    actual: &[u16],
    expected_body: &[u16],
    prefix: u16,
    suffix: u16,
) -> Result<(), BoxError> {
    require(
        actual.len() == expected_body.len() + 2 * CANARY_ELEMENTS,
        format!("{role} guarded allocation length changed"),
    )?;
    let (actual_prefix, remainder) = actual.split_at(CANARY_ELEMENTS);
    let (actual_body, actual_suffix) = remainder.split_at(expected_body.len());
    require(
        actual_prefix.iter().all(|value| *value == prefix),
        format!("{role} prefix canary changed"),
    )?;
    require(actual_body == expected_body, format!("{role} body changed"))?;
    require(
        actual_suffix.iter().all(|value| *value == suffix),
        format!("{role} suffix canary changed"),
    )
}

fn verify_guarded_f32(
    role: &str,
    actual: &[f32],
    expected_body: &[f32],
    prefix: f32,
    suffix: f32,
) -> Result<(), BoxError> {
    require(
        actual.len() == expected_body.len() + 2 * CANARY_ELEMENTS,
        format!("{role} guarded allocation length changed"),
    )?;
    let (actual_prefix, remainder) = actual.split_at(CANARY_ELEMENTS);
    let (actual_body, actual_suffix) = remainder.split_at(expected_body.len());
    let compare = |label: &str, observed: &[f32], expected: &[f32]| -> Result<(), BoxError> {
        require(
            observed.len() == expected.len(),
            format!("{role} {label} length changed"),
        )?;
        for (index, (observed, expected)) in observed.iter().zip(expected).enumerate() {
            require(
                observed.to_bits() == expected.to_bits(),
                format!(
                    "{role} {label}[{index}] changed: {:#010x} != {:#010x}",
                    observed.to_bits(),
                    expected.to_bits()
                ),
            )?;
        }
        Ok(())
    };
    compare("prefix canary", actual_prefix, &[prefix; CANARY_ELEMENTS])?;
    compare("body", actual_body, expected_body)?;
    compare("suffix canary", actual_suffix, &[suffix; CANARY_ELEMENTS])
}

fn launch_geometry() -> HsaLaunchGeometryV1 {
    // The runtime adapter interprets `grid` as block counts and derives the
    // AQL global work-item grid by multiplying it with the WG64 dimensions.
    HsaLaunchGeometryV1::new([1, 1, 1], [WORKGROUP_X, 1, 1], 0)
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn explicit_kernarg(addresses: [u64; 4]) -> [u8; EXPLICIT_KERNARG_BYTES] {
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
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
    file: std::fs::File,
    device: u64,
    inode: u64,
    digest: [u8; 32],
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
impl PrivateArtifactMaterialization {
    fn new(bytes: &[u8]) -> Result<Self, BoxError> {
        let directory = create_private_directory("hsaco")?;
        let path = directory.join("artifact.hsaco");
        let result = (|| -> Result<Self, BoxError> {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(bytes)?;
            file.flush()?;
            file.sync_all()?;
            require(
                file.metadata()?.len() == bytes.len() as u64,
                "private HSACO materialization has the wrong written length",
            )?;
            let written_metadata = file.metadata()?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))?;
            std::fs::File::open(&directory)?.sync_all()?;
            drop(file);
            let file = std::fs::OpenOptions::new().read(true).open(&path)?;
            let metadata = file.metadata()?;
            require(
                metadata.dev() == written_metadata.dev()
                    && metadata.ino() == written_metadata.ino()
                    && metadata.len() == written_metadata.len(),
                "private HSACO identity changed while reopening it read-only",
            )?;
            let materialized = Self {
                directory: directory.clone(),
                path: path.clone(),
                file,
                device: metadata.dev(),
                inode: metadata.ino(),
                digest: sha256(bytes),
            };
            materialized.verify(bytes)?;
            Ok(materialized)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_dir(&directory);
        }
        result
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn verify(&self, expected: &[u8]) -> Result<(), BoxError> {
        let retained_metadata = self.file.metadata()?;
        let path_metadata = std::fs::symlink_metadata(&self.path)?;
        require(
            retained_metadata.dev() == self.device
                && retained_metadata.ino() == self.inode
                && path_metadata.dev() == self.device
                && path_metadata.ino() == self.inode
                && path_metadata.file_type().is_file()
                && !path_metadata.file_type().is_symlink()
                && path_metadata.nlink() == 1
                && path_metadata.len() == expected.len() as u64
                && path_metadata.permissions().mode() & 0o777 == 0o400,
            "private HSACO materialization identity or permissions changed",
        )?;
        let mut retained = self.file.try_clone()?;
        retained.seek(SeekFrom::Start(0))?;
        let mut retained_bytes = Vec::new();
        retained.read_to_end(&mut retained_bytes)?;
        let path_bytes = std::fs::read(&self.path)?;
        require(
            retained_bytes == expected
                && path_bytes == expected
                && sha256(&retained_bytes) == self.digest
                && sha256(&path_bytes) == self.digest
                && sha256(expected) == self.digest,
            "private HSACO materialization bytes changed",
        )
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
impl Drop for PrivateArtifactMaterialization {
    fn drop(&mut self) {
        if std::fs::remove_file(&self.path).is_err()
            || std::fs::remove_dir(&self.directory).is_err()
        {
            std::process::abort();
        }
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
struct PrivateExecutableMaterialization {
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
    file: std::fs::File,
    device: u64,
    inode: u64,
    digest: [u8; 32],
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
impl PrivateExecutableMaterialization {
    fn new(bytes: &[u8]) -> Result<Self, BoxError> {
        require(
            !bytes.is_empty() && bytes.len() as u64 <= MAX_LLVM_OBJDUMP_BYTES,
            "private llvm-objdump materialization has an invalid bounded length",
        )?;
        let directory = create_private_directory("objdump")?;
        let path = directory.join("llvm-objdump");
        let result = (|| -> Result<Self, BoxError> {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(bytes)?;
            file.flush()?;
            file.sync_all()?;
            let written_metadata = file.metadata()?;
            require(
                written_metadata.len() == bytes.len() as u64,
                "private llvm-objdump materialization has the wrong written length",
            )?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500))?;
            std::fs::File::open(&directory)?.sync_all()?;
            drop(file);
            let file = std::fs::OpenOptions::new().read(true).open(&path)?;
            let metadata = file.metadata()?;
            require(
                metadata.dev() == written_metadata.dev()
                    && metadata.ino() == written_metadata.ino()
                    && metadata.len() == written_metadata.len(),
                "private llvm-objdump identity changed while reopening it read-only",
            )?;
            let materialized = Self {
                directory: directory.clone(),
                path: path.clone(),
                file,
                device: metadata.dev(),
                inode: metadata.ino(),
                digest: sha256(bytes),
            };
            materialized.verify(bytes)?;
            Ok(materialized)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_dir(&directory);
        }
        result
    }

    fn verify(&self, expected: &[u8]) -> Result<(), BoxError> {
        let retained_metadata = self.file.metadata()?;
        let path_metadata = std::fs::symlink_metadata(&self.path)?;
        require(
            retained_metadata.dev() == self.device
                && retained_metadata.ino() == self.inode
                && path_metadata.dev() == self.device
                && path_metadata.ino() == self.inode
                && path_metadata.file_type().is_file()
                && !path_metadata.file_type().is_symlink()
                && path_metadata.nlink() == 1
                && path_metadata.len() == expected.len() as u64
                && path_metadata.permissions().mode() & 0o777 == 0o500,
            "private llvm-objdump identity or permissions changed",
        )?;
        let mut retained = self.file.try_clone()?;
        retained.seek(SeekFrom::Start(0))?;
        let mut retained_bytes = Vec::new();
        retained.read_to_end(&mut retained_bytes)?;
        let path_bytes = std::fs::read(&self.path)?;
        require(
            retained_bytes == expected
                && path_bytes == expected
                && sha256(&retained_bytes) == self.digest
                && sha256(&path_bytes) == self.digest
                && sha256(expected) == self.digest,
            "private llvm-objdump bytes changed",
        )
    }

    fn output<I, S>(&self, expected: &[u8], arguments: I) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.verify(expected)?;
        let output = std::process::Command::new(&self.path)
            .args(arguments)
            .output()?;
        self.verify(expected)?;
        Ok(output)
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
impl Drop for PrivateExecutableMaterialization {
    fn drop(&mut self) {
        if std::fs::remove_file(&self.path).is_err()
            || std::fs::remove_dir(&self.directory).is_err()
        {
            std::process::abort();
        }
    }
}

#[cfg(any(test, feature = "hardware-test-hooks"))]
fn create_private_directory(label: &str) -> Result<std::path::PathBuf, BoxError> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    for attempt in 0..64_u32 {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-tiled-gemm-v1-{label}-{}-{nonce}-{attempt}",
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
        std::env::var("FE2O3_RUN_GFX942_TILED_GEMM_V1_HARDWARE").as_deref() == Ok("1"),
        "set FE2O3_RUN_GFX942_TILED_GEMM_V1_HARDWARE=1 to opt into this non-authoritative test",
    )?;
    let kernel_symbol = std::env::var("FE2O3_GFX942_TILED_GEMM_V1_KERNEL_SYMBOL")
        .map_err(|_| "FE2O3_GFX942_TILED_GEMM_V1_KERNEL_SYMBOL is not set")?;
    require(
        kernel_symbol == TILED_GEMM_V1_EXPORT,
        format!("FE2O3_GFX942_TILED_GEMM_V1_KERNEL_SYMBOL must equal `{TILED_GEMM_V1_EXPORT}`"),
    )?;
    let path = canonical_regular_file("FE2O3_GFX942_TILED_GEMM_V1_HSACO")?;
    let metadata = std::fs::metadata(&path)?;
    require(
        (1..=fe2o3_hsaco::MAX_HSACO_BYTES as u64).contains(&metadata.len()),
        "FE2O3_GFX942_TILED_GEMM_V1_HSACO has an invalid byte length",
    )?;
    let expected = parse_exact_sha256(
        "FE2O3_GFX942_TILED_GEMM_V1_SHA256",
        &std::env::var("FE2O3_GFX942_TILED_GEMM_V1_SHA256")
            .map_err(|_| "FE2O3_GFX942_TILED_GEMM_V1_SHA256 is not set")?,
    )?;
    let bytes = std::fs::read(&path)?;
    require(
        bytes.len() as u64 == metadata.len(),
        "Tiled GEMM V1 HSACO changed size while being read",
    )?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    require(
        digest.bytes().as_bytes() == &expected,
        "Tiled GEMM V1 HSACO does not match its exact SHA-256 pin",
    )?;
    let final_metadata = std::fs::symlink_metadata(&path)?;
    require(
        final_metadata.file_type().is_file()
            && !final_metadata.file_type().is_symlink()
            && final_metadata.len() == metadata.len()
            && std::fs::canonicalize(&path)? == path,
        "Tiled GEMM V1 caller-supplied HSACO path changed while being read",
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
fn inspect_final_isa(artifact: &PinnedArtifact, entry: BoundKernelEntry) -> Result<(), BoxError> {
    let materialized = PrivateArtifactMaterialization::new(&artifact.bytes)?;
    let arguments = [
        std::ffi::OsString::from(format!("--disassemble-symbols={}", artifact.kernel_symbol)),
        std::ffi::OsString::from(format!("--start-address=0x{:x}", entry.address)),
        std::ffi::OsString::from(format!("--stop-address=0x{:x}", entry.end()?)),
        std::ffi::OsString::from("--mcpu=gfx942"),
        materialized.path().as_os_str().to_owned(),
    ];
    let output = artifact.objdump.output(arguments)?;
    require(
        output.status.success() && output.stderr.is_empty(),
        format!(
            "digest-pinned observed LLVM 22 llvm-objdump rejected Tiled GEMM V1: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    let disassembly = String::from_utf8(output.stdout)
        .map_err(|_| "digest-pinned llvm-objdump emitted non-UTF-8 output")?;
    validate_isa(&disassembly, &artifact.kernel_symbol, entry)?;
    materialized.verify(&artifact.bytes)?;
    artifact.objdump.verify_bytes()?;
    require(
        artifact.objdump.observe_version()? == artifact.objdump.observed_version,
        "digest-pinned llvm-objdump observed version changed across inspection",
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn u16_bytes(values: &[u16]) -> &[u8] {
    // SAFETY: `u16` has no invalid bit patterns and the byte extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: `f32` has no invalid bit patterns and the byte extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn u16_values(bytes: &[u8]) -> Result<Vec<u16>, BoxError> {
    require(
        bytes.len().is_multiple_of(std::mem::size_of::<u16>()),
        "hardware-test allocation contains a partial u16",
    )?;
    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_ne_bytes(chunk.try_into().expect("exact u16 chunk")))
        .collect())
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
fn body_address(
    buffer: &ReviewedHsaHardwareTestBufferV1,
    element_size: usize,
) -> Result<u64, BoxError> {
    require(
        buffer.byte_len() == (ELEMENTS + 2 * CANARY_ELEMENTS) * element_size,
        "guarded hardware allocation has the wrong physical extent",
    )?;
    Ok(buffer.device_address(CANARY_ELEMENTS * element_size)?)
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
            .ok_or("failed to allocate runtime-aligned Tiled GEMM V1 kernarg")?;
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
unsafe fn dispatch_one_tile(
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
        "runtime resolution differs from the exact Tiled GEMM V1 export and kernarg",
    )?;
    let geometry = launch_geometry();
    require(
        geometry.grid() == [1, 1, 1] && geometry.workgroup() == [WORKGROUP_X, 1, 1],
        "one-tile launch must be one block count expanded by the adapter to 64 AQL work-items",
    )?;
    let mut storage = RuntimeKernarg::new()?;
    let kernarg = storage.bytes_mut();
    kernarg[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);

    // SAFETY: the exact digest-pinned single-kernel COV6 image was structurally
    // and textually inspected. Four live guarded allocations supply the frozen
    // 64-byte pointer/length ABI, all 256 hidden bytes are initialized by the adapter,
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
            "Tiled GEMM V1 dispatch did not complete synchronously",
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn execute_one_tile(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    kernel_symbol: &str,
) -> Result<(), BoxError> {
    let (a_body, b_body, c_body) = tiled_inputs();
    let expected_d = tiled_oracle(&a_body, &b_body, &c_body)?;
    let a_host = guarded_u16(&a_body, A_PREFIX, A_SUFFIX);
    let b_host = guarded_u16(&b_body, B_PREFIX, B_SUFFIX);
    let c_host = guarded_f32(&c_body, C_PREFIX, C_SUFFIX);
    let d_host = guarded_f32(&[D_POISON; ELEMENTS], D_PREFIX, D_SUFFIX);
    let a = adapter.allocate_hardware_test_buffer(u16_bytes(&a_host))?;
    let b = adapter.allocate_hardware_test_buffer(u16_bytes(&b_host))?;
    let c = adapter.allocate_hardware_test_buffer(f32_bytes(&c_host))?;
    let d = adapter.allocate_hardware_test_buffer(f32_bytes(&d_host))?;
    let explicit = explicit_kernarg([
        body_address(&a, std::mem::size_of::<u16>())?,
        body_address(&b, std::mem::size_of::<u16>())?,
        body_address(&c, std::mem::size_of::<f32>())?,
        body_address(&d, std::mem::size_of::<f32>())?,
    ]);

    // SAFETY: `dispatch_one_tile` owns and documents the only raw launch
    // boundary and returns only after all four allocations are synchronously idle.
    unsafe {
        dispatch_one_tile(
            adapter,
            executable,
            kernel,
            resolution,
            kernel_symbol,
            &explicit,
        )?;
    }

    let a_after = u16_values(&a.read_after_synchronous_dispatch())?;
    let b_after = u16_values(&b.read_after_synchronous_dispatch())?;
    let c_after = f32_values(&c.read_after_synchronous_dispatch())?;
    let d_after = f32_values(&d.read_after_synchronous_dispatch())?;
    verify_guarded_u16("immutable A", &a_after, &a_body, A_PREFIX, A_SUFFIX)?;
    verify_guarded_u16("immutable B", &b_after, &b_body, B_PREFIX, B_SUFFIX)?;
    verify_guarded_f32("immutable C", &c_after, &c_body, C_PREFIX, C_SUFFIX)?;
    verify_guarded_f32("output D", &d_after, &expected_d, D_PREFIX, D_SUFFIX)
}

#[cfg(feature = "hardware-test-hooks")]
fn run_non_authoritative_hardware_evidence(artifact: PinnedArtifact) -> Result<(), BoxError> {
    let entry = inspect_metadata(&artifact.bytes, &artifact.kernel_symbol)?;
    inspect_final_isa(&artifact, entry)?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    let physical_target = adapter.environment().physical_device().target();
    require(
        physical_target.processor() == "gfx942"
            && physical_target.xnack() == Some(FeatureState::Disabled),
        "Tiled GEMM V1 hardware evidence requires a gfx942:xnack- physical device",
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
            "loaded Tiled GEMM V1 bytes differ from the pinned artifact",
        )?;
        // SAFETY: inspection admitted exactly one matching export and descriptor.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [artifact.kernel_symbol.as_str()]) }?;
        require(
            kernels.len() == 1
                && resolutions.len() == 1
                && resolutions[0].export_symbol() == artifact.kernel_symbol
                && resolutions[0].executable_object() == executable_identity,
            "runtime resolved a substituted Tiled GEMM V1 kernel",
        )?;
        let kernel = kernels
            .get(0)
            .ok_or("runtime omitted the resolved Tiled GEMM V1 kernel")?;
        execute_one_tile(
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
        "reviewed HSA unload did not release the exact Tiled GEMM V1 executable",
    )?;
    execution
}

/// Executes one direct-global `16x16x16` BF16/BF16/F32 Tiled GEMM V1 tile.
///
/// This ignored test bypasses production prerequisite authentication and grants
/// no protected evidence. Its guards detect only changed values within each
/// finite guarded allocation. They do not detect beyond-guard accesses,
/// value-preserving writes, same-value races, or output-inert reads.
///
/// ```text
/// FE2O3_RUN_GFX942_TILED_GEMM_V1_HARDWARE=1 \
/// FE2O3_GFX942_TILED_GEMM_V1_HSACO=/absolute/canonical/tiled-gemm-v1.hsaco \
/// FE2O3_GFX942_TILED_GEMM_V1_SHA256=<64-lowercase-hex-digits> \
/// FE2O3_GFX942_TILED_GEMM_V1_KERNEL_SYMBOL=tiled_gemm_v1 \
/// FE2O3_LLVM_OBJDUMP=/absolute/canonical/llvm-objdump \
/// FE2O3_LLVM_OBJDUMP_SHA256=<64-lowercase-hex-digits> \
/// cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test tiled_gemm_v1_hardware \
///   gfx942_tiled_gemm_v1_one_tile_raw_hardware_evidence \
///   -- --ignored --exact --nocapture
/// ```
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "non-authoritative: requires exact pinned COV6 HSACO, observed LLVM 22 objdump, and gfx942:xnack-"]
fn gfx942_tiled_gemm_v1_one_tile_raw_hardware_evidence() -> Result<(), BoxError> {
    run_non_authoritative_hardware_evidence(read_pinned_artifact()?)
}

#[cfg(test)]
mod tests {
    use super::*;

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
             0000000000001000 <{TILED_GEMM_V1_EXPORT}>:\n\
             \tglobal_load_dword v0, v[0:1], off // 000000001000: DEADBEEF\n\
             \tv_mfma_f32_16x16x16_bf16 a[0:3], v0, v1, a[0:3] // 000000001004: DEADBEEF FEEDFACE\n\
             \tglobal_store_dwordx4 v[0:1], v[0:3], off // 00000000100C: DEADBEEF\n\
             \ts_endpgm // 000000001010: DEADBEEF\n\
             \ts_nop 0 // 000000001014: DEADBEEF\n"
        )
    }

    #[test]
    fn launch_uses_one_block_count_and_one_wave64() {
        let geometry = launch_geometry();
        assert_eq!(geometry.grid(), [1, 1, 1]);
        assert_eq!(geometry.workgroup(), [64, 1, 1]);
        assert_eq!(geometry.dynamic_shared_memory_bytes(), 0);
        let packed = explicit_kernarg([0x11, 0x22, 0x33, 0x44]);
        for (index, address) in [0x11_u64, 0x22, 0x33, 0x44].into_iter().enumerate() {
            let offset = index * 16;
            assert_eq!(&packed[offset..offset + 8], &address.to_le_bytes());
            assert_eq!(&packed[offset + 8..offset + 16], &256_u64.to_le_bytes());
        }
        assert_eq!(EXPLICIT_KERNARG_BYTES, 64);
        assert_eq!(COMPLETE_KERNARG_BYTES, 320);
    }

    #[test]
    fn metadata_validator_rejects_profile_substitutions() {
        let expected = MetadataFacts::expected(TILED_GEMM_V1_EXPORT);
        validate_metadata(&expected, TILED_GEMM_V1_EXPORT).unwrap();

        let mut fragment_probe = expected.clone();
        fragment_probe.kernarg_size = 288;
        fragment_probe.implicit_offset = Some(32);
        fragment_probe.arguments = (0_u64..8)
            .map(|index| ArgumentFact {
                offset: index * 2,
                size: 2,
                kind: ExplicitValueKind::ByValue,
                address_space: None,
            })
            .chain((0_u64..4).map(|index| ArgumentFact {
                offset: 16 + index * 4,
                size: 4,
                kind: ExplicitValueKind::ByValue,
                address_space: None,
            }))
            .collect();
        fragment_probe.descriptor_kernarg_size = 288;
        assert!(validate_metadata(&fragment_probe, TILED_GEMM_V1_EXPORT).is_err());

        let mutations: [fn(&mut MetadataFacts); 19] = [
            |facts| facts.code_object_version = 5,
            |facts| facts.target = "gfx942:xnack+".to_owned(),
            |facts| facts.kernel_count = 2,
            |facts| facts.kernel_name = "substituted".to_owned(),
            |facts| facts.kernarg_size = 288,
            |facts| facts.implicit_offset = Some(32),
            |facts| facts.required_workgroup = Some([1, 1, 1]),
            |facts| facts.wavefront_size = 32,
            |facts| facts.group_segment_size = 1024,
            |facts| facts.private_segment_size = 4,
            |facts| facts.normal_kernel = false,
            |facts| facts.sgpr_spill_count = 1,
            |facts| facts.vgpr_spill_count = 1,
            |facts| facts.arguments[3].kind = ExplicitValueKind::GlobalBuffer,
            |facts| facts.arguments[0].address_space = None,
            |facts| facts.binding_count = 0,
            |facts| facts.descriptor_kernarg_size = 288,
            |facts| facts.descriptor_private_segment_enabled = true,
            |facts| facts.descriptor_uses_dynamic_stack = true,
        ];
        for mutate in mutations {
            let mut hostile = expected.clone();
            mutate(&mut hostile);
            assert!(validate_metadata(&hostile, TILED_GEMM_V1_EXPORT).is_err());
        }
    }

    #[test]
    fn isa_validator_rejects_missing_effects_and_forbidden_families() {
        let valid = valid_disassembly();
        validate_isa(&valid, TILED_GEMM_V1_EXPORT, valid_entry()).unwrap();

        for hostile in [
            valid.replace("global_store_dwordx4", "v_mov_b32"),
            valid.replace("global_load_dword", "v_mov_b32"),
            valid.replace("s_endpgm", "s_call_b64 s[0:1]"),
            valid.replace("s_endpgm", "scratch_store_dword off, v0"),
            valid.replace("s_endpgm", "global_atomic_add v0, v1, v2"),
            valid.replace("s_endpgm", "ds_write_b32 v0, v1"),
            valid.replace(
                "v_mfma_f32_16x16x16_bf16",
                "helper_v_mfma_f32_16x16x16_bf16",
            ),
            valid.replace("000000001000:", "0000000000001000:"),
        ] {
            assert!(validate_isa(&hostile, TILED_GEMM_V1_EXPORT, valid_entry()).is_err());
        }
    }

    #[test]
    fn isa_validator_rejects_mfma_owned_only_by_a_helper() {
        let scalar_kernel = valid_disassembly().replace(
            "v_mfma_f32_16x16x16_bf16 a[0:3], v0, v1, a[0:3]",
            "v_add_f32 v0, v1, v2",
        );
        let helper = "\n0000000000002000 <helper>:\n\
                      \tv_mfma_f32_16x16x16_bf16 a[0:3], v0, v1, a[0:3] // 000000002000: DEADBEEF FEEDFACE\n\
                      \ts_endpgm // 000000002008: DEADBEEF\n";
        let hostile = format!("{scalar_kernel}{helper}");
        assert!(validate_isa(&hostile, TILED_GEMM_V1_EXPORT, valid_entry()).is_err());
    }

    #[test]
    fn private_materialization_ignores_a_substituted_caller_path() {
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
        assert_eq!(std::fs::read(materialized.path()).unwrap(), original);

        std::fs::remove_file(caller_path).unwrap();
        std::fs::remove_dir(caller_directory).unwrap();
    }

    #[test]
    fn private_objdump_execution_ignores_caller_path_substitution() {
        let caller_directory = create_private_directory("caller-objdump").unwrap();
        let caller_path = caller_directory.join("llvm-objdump");
        let substitute_path = caller_directory.join("hostile-objdump");
        let captured = b"#!/bin/sh\ncase \"$1\" in\n  --version) printf 'AMD LLVM version 22.0.0git\\n' ;;\n  --disassemble-symbols=tiled_gemm_v1) printf 'captured disassembly\\n' ;;\n  *) exit 64 ;;\nesac\n";
        let substituted = b"#!/bin/sh\nprintf 'substituted caller executable\\n'\n";
        for (path, bytes) in [
            (&caller_path, captured.as_slice()),
            (&substitute_path, substituted),
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
        let version = executable.output(captured, ["--version"]).unwrap();
        assert!(version.status.success() && version.stderr.is_empty());
        assert_eq!(
            observed_llvm_22_version(&version.stdout).unwrap(),
            "AMD LLVM version 22.0.0git"
        );
        let disassembly = executable
            .output(captured, ["--disassemble-symbols=tiled_gemm_v1"])
            .unwrap();
        assert!(disassembly.status.success() && disassembly.stderr.is_empty());
        assert_eq!(disassembly.stdout, b"captured disassembly\n");
        executable.verify(captured).unwrap();

        std::fs::remove_file(caller_path).unwrap();
        std::fs::remove_dir(caller_directory).unwrap();
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
        let u16_body = [0x3f80, 0xbf00];
        let u16_valid = guarded_u16(&u16_body, A_PREFIX, A_SUFFIX);
        verify_guarded_u16("A", &u16_valid, &u16_body, A_PREFIX, A_SUFFIX).unwrap();
        for index in [0, CANARY_ELEMENTS, u16_valid.len() - 1] {
            let mut hostile = u16_valid.clone();
            hostile[index] ^= 1;
            assert!(verify_guarded_u16("A", &hostile, &u16_body, A_PREFIX, A_SUFFIX).is_err());
        }

        let f32_body = [1.0, 2.0];
        let f32_valid = guarded_f32(&f32_body, D_PREFIX, D_SUFFIX);
        verify_guarded_f32("D", &f32_valid, &f32_body, D_PREFIX, D_SUFFIX).unwrap();
        for index in [0, CANARY_ELEMENTS, f32_valid.len() - 1] {
            let mut hostile = f32_valid.clone();
            hostile[index] = f32::from_bits(hostile[index].to_bits() ^ 1);
            assert!(verify_guarded_f32("D", &hostile, &f32_body, D_PREFIX, D_SUFFIX).is_err());
        }
    }

    #[test]
    fn dyadic_oracle_is_bitwise_and_rejects_output_substitution() {
        let (a, b, c) = tiled_inputs();
        let expected = tiled_oracle(&a, &b, &c).unwrap();
        assert_eq!(expected.len(), ELEMENTS);
        assert!(
            expected
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
        );
        let valid = guarded_f32(&expected, D_PREFIX, D_SUFFIX);
        verify_guarded_f32("D", &valid, &expected, D_PREFIX, D_SUFFIX).unwrap();

        let mut one_ulp_wrong = valid;
        one_ulp_wrong[CANARY_ELEMENTS + 137] =
            f32::from_bits(one_ulp_wrong[CANARY_ELEMENTS + 137].to_bits() + 1);
        assert!(verify_guarded_f32("D", &one_ulp_wrong, &expected, D_PREFIX, D_SUFFIX).is_err());
        assert!(tiled_oracle(&a[..ELEMENTS - 1], &b, &c).is_err());
    }

    #[test]
    fn digest_parser_requires_exact_lowercase_encoding() {
        assert!(parse_exact_sha256("PIN", &"ab".repeat(32)).is_ok());
        assert!(parse_exact_sha256("PIN", &"AB".repeat(32)).is_err());
        assert!(parse_exact_sha256("PIN", &"0".repeat(63)).is_err());
    }
}
