//! Observational MI300X evidence for committed K32 LDS GEMM Slice 2.
//!
//! The ignored test generates its HSACO in-process from the canonical Kernel
//! IR through `dialect_amdgcn`, SHA-256-pinned upstream `llc` and `ld.lld`.
//! COMGR is neither invoked nor admitted. The resulting bytes, metadata, ISA,
//! and hardware results are observations of the IR-derived path only. They do
//! not bind the IR to source or proofs, authenticate a publisher, or establish
//! illegal-access absence or race freedom.

use fe2o3_host::HsaLaunchGeometryV1;
use fe2o3_hsaco::{ArgumentAddressSpace, ExplicitValueKind};
use fe2o3_kernel_ir::{
    TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS, TILED_GEMM_LDS_K32_V2_K, TILED_GEMM_LDS_K32_V2_KERNEL_ID,
    TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS,
};

#[cfg(feature = "hardware-test-hooks")]
use dialect_amdgcn::lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir;
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
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_kernel_ir::{
    TILED_GEMM_LDS_K32_V2_STATIC_LDS_BYTES, TiledGemmLdsK32V2Profile, tiled_gemm_lds_k32_v2_module,
};
#[cfg(feature = "hardware-test-hooks")]
use sha2::{Digest, Sha256};

const TILE: usize = 16;
const DEPTH: usize = 32;
const INPUT_ELEMENTS: usize = TILE * DEPTH;
const OUTPUT_ELEMENTS: usize = TILE * TILE;
const WORKGROUP_X: u32 = 64;
const STATIC_LDS_BYTES: u64 = 1024;
const EXPLICIT_KERNARG_BYTES: usize = 48;
const IMPLICIT_KERNARG_BYTES: usize = 256;
const COMPLETE_KERNARG_BYTES: usize = EXPLICIT_KERNARG_BYTES + IMPLICIT_KERNARG_BYTES;
const PHYSICAL_KERNARG_ALIGNMENT: u64 = 8;
#[cfg(feature = "hardware-test-hooks")]
const HSA_KERNARG_ALIGNMENT: u64 = 16;
const CANARY_ELEMENTS: usize = 32;
const TARGET: &str = "gfx942:xnack-";

const A_PREFIX: u16 = 0x7fc1;
const A_SUFFIX: u16 = 0x7fc2;
const B_PREFIX: u16 = 0x7fd1;
const B_SUFFIX: u16 = 0x7fd2;
const C_PREFIX: f32 = f32::from_bits(0x7fc0_c001);
const C_SUFFIX: f32 = f32::from_bits(0x7fc0_c002);
const C_POISON: f32 = f32::from_bits(0x7fc0_c0ff);

type BoxError = Box<dyn std::error::Error>;

fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
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
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{variable} is malformed"))?;
    }
    Ok(digest)
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
    fn expected() -> Self {
        let arguments = [0_u64, 16, 32]
            .into_iter()
            .flat_map(|offset| {
                [
                    ArgumentFact {
                        offset,
                        size: 8,
                        kind: ExplicitValueKind::GlobalBuffer,
                        address_space: Some(ArgumentAddressSpace::Global),
                    },
                    ArgumentFact {
                        offset: offset + 8,
                        size: 8,
                        kind: ExplicitValueKind::ByValue,
                        address_space: None,
                    },
                ]
            })
            .collect();
        Self {
            code_object_version: 6,
            target: TARGET.to_owned(),
            has_printf_metadata: false,
            kernel_count: 1,
            kernel_name: TILED_GEMM_LDS_K32_V2_KERNEL_ID.to_owned(),
            descriptor_symbol: format!("{TILED_GEMM_LDS_K32_V2_KERNEL_ID}.kd"),
            kernarg_size: COMPLETE_KERNARG_BYTES as u64,
            kernarg_alignment: PHYSICAL_KERNARG_ALIGNMENT,
            implicit_offset: Some(EXPLICIT_KERNARG_BYTES as u64),
            implicit_size: IMPLICIT_KERNARG_BYTES as u64,
            required_workgroup: Some([WORKGROUP_X, 1, 1]),
            max_flat_workgroup: WORKGROUP_X,
            wavefront_size: 64,
            group_segment_size: STATIC_LDS_BYTES,
            private_segment_size: 0,
            sgpr_spill_count: 0,
            vgpr_spill_count: 0,
            uses_dynamic_stack: false,
            arguments,
            binding_count: 1,
            binding_kernel_index: 0,
            descriptor_kernarg_size: COMPLETE_KERNARG_BYTES as u32,
            descriptor_group_segment_size: STATIC_LDS_BYTES as u32,
            descriptor_private_segment_size: 0,
            descriptor_private_segment_enabled: false,
            descriptor_wavefront_size: 64,
            descriptor_uses_dynamic_stack: false,
        }
    }
}

fn validate_metadata(facts: &MetadataFacts) -> Result<(), BoxError> {
    require(
        facts == &MetadataFacts::expected(),
        format!(
            "Slice 2 metadata differs from exact gfx942:xnack-/COV6/3-slice/\
             WG64/wave64/1024-byte-LDS/zero-scratch profile: {facts:#?}"
        ),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundKernelEntry {
    address: u64,
    size: u64,
}

impl BoundKernelEntry {
    fn end(self) -> Result<u64, BoxError> {
        self.address
            .checked_add(self.size)
            .ok_or_else(|| "Slice 2 entry address overflow".into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn inspect_metadata(bytes: &[u8]) -> Result<BoundKernelEntry, BoxError> {
    require(
        TILED_GEMM_LDS_K32_V2_STATIC_LDS_BYTES as u64 == STATIC_LDS_BYTES,
        "runtime and Kernel IR LDS byte constants disagree",
    )?;
    let bound = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)?;
    let inspection = bound.inspection();
    let kernel = inspection
        .kernels()
        .first()
        .ok_or("Slice 2 HSACO declares no kernel")?;
    let binding = bound
        .bindings()
        .first()
        .ok_or("Slice 2 HSACO has no descriptor binding")?;
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
    validate_metadata(&facts)?;
    let entry = BoundKernelEntry {
        address: binding.entry_address(),
        size: binding.entry_size(),
    };
    require(entry.size != 0, "Slice 2 ELF entry is empty")?;
    let _ = entry.end()?;
    Ok(entry)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisassembledInstruction {
    address: u64,
    byte_len: u64,
    mnemonic: String,
    operands: String,
}

impl DisassembledInstruction {
    fn scalar_branch_target(&self) -> Result<Option<u64>, BoxError> {
        if self.mnemonic != "s_branch" && !self.mnemonic.starts_with("s_cbranch_") {
            return Ok(None);
        }
        require(
            self.byte_len == 4,
            "scalar branch has a non-SOPP instruction width",
        )?;
        let immediate = self
            .operands
            .split_ascii_whitespace()
            .next()
            .ok_or("scalar branch omitted its displacement")?;
        let encoded = if let Some(hex) = immediate.strip_prefix("0x") {
            u16::from_str_radix(hex, 16)?
        } else {
            immediate.parse::<u16>()?
        };
        let displacement = i64::from(encoded as i16) * 4;
        let next = self
            .address
            .checked_add(self.byte_len)
            .ok_or("scalar branch address overflow")?;
        let target = if displacement < 0 {
            next.checked_sub(displacement.unsigned_abs())
        } else {
            next.checked_add(displacement as u64)
        }
        .ok_or("scalar branch target overflow")?;
        Ok(Some(target))
    }
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
        "llvm-objdump emitted a non-canonical function address",
    )?;
    require(
        !symbol.is_empty()
            && symbol
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$')),
        "llvm-objdump emitted an invalid function symbol",
    )?;
    Ok(Some((u64::from_str_radix(address, 16)?, symbol)))
}

fn parse_instruction(line: &str) -> Result<DisassembledInstruction, BoxError> {
    require(
        line.starts_with('\t'),
        "symbol-scoped disassembly contains a non-instruction line",
    )?;
    let (assembly, encoding) = line
        .split_once("//")
        .ok_or("llvm-objdump instruction omitted address/encoding")?;
    let assembly = assembly.trim();
    let mut assembly_tokens = assembly.splitn(2, char::is_whitespace);
    let mnemonic = assembly_tokens
        .next()
        .ok_or("llvm-objdump emitted an empty instruction")?;
    require(
        mnemonic
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "llvm-objdump emitted a non-canonical mnemonic",
    )?;
    let (address, words) = encoding
        .trim()
        .split_once(':')
        .ok_or("llvm-objdump instruction omitted its address")?;
    require(
        address.len() == 12 && address.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "llvm-objdump emitted a non-canonical instruction address",
    )?;
    let encoding_tokens = words.split_ascii_whitespace().collect::<Vec<_>>();
    let word_count = encoding_tokens
        .iter()
        .take_while(|word| word.len() == 8 && word.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .count();
    require(
        matches!(word_count, 1 | 2),
        "llvm-objdump emitted a non-canonical AMDGPU encoding",
    )?;
    let annotations = &encoding_tokens[word_count..];
    require(
        annotations.is_empty()
            || (annotations.len() == 1
                && annotations[0].starts_with('<')
                && annotations[0].ends_with('>')
                && annotations[0][1..annotations[0].len() - 1]
                    .bytes()
                    .all(|byte| {
                        byte.is_ascii_alphanumeric()
                            || matches!(byte, b'_' | b'.' | b'$' | b'+' | b'-')
                    })),
        "llvm-objdump emitted a non-canonical branch annotation",
    )?;
    Ok(DisassembledInstruction {
        address: u64::from_str_radix(address, 16)?,
        byte_len: (word_count * 4) as u64,
        mnemonic: mnemonic.to_owned(),
        operands: assembly_tokens.next().unwrap_or_default().trim().to_owned(),
    })
}

fn validate_isa(
    disassembly: &str,
    entry: BoundKernelEntry,
) -> Result<Vec<DisassembledInstruction>, BoxError> {
    require(
        !disassembly.contains('\0'),
        "llvm-objdump output contains a NUL byte",
    )?;
    let lines = disassembly.lines().collect::<Vec<_>>();
    let mut headers = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some((address, symbol)) = parse_function_header(line)? {
            headers.push((index, address, symbol));
        }
    }
    require(
        headers.len() == 1,
        format!(
            "symbol-scoped disassembly contains {} function headers",
            headers.len()
        ),
    )?;
    let (header_index, header_address, header_symbol) = headers[0];
    require(
        header_symbol == TILED_GEMM_LDS_K32_V2_KERNEL_ID && header_address == entry.address,
        "disassembly header differs from the bound Slice 2 ELF entry",
    )?;
    let instructions = lines
        .iter()
        .skip(header_index + 1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_instruction(line))
        .collect::<Result<Vec<_>, _>>()?;
    require(!instructions.is_empty(), "Slice 2 disassembly is empty")?;
    let mut expected_address = entry.address;
    for instruction in &instructions {
        require(
            instruction.address == expected_address,
            "disassembly does not exactly cover the bound ELF entry",
        )?;
        expected_address = expected_address
            .checked_add(instruction.byte_len)
            .ok_or("instruction address overflow")?;
    }
    require(
        expected_address == entry.end()?,
        "disassembly size differs from the bound ELF entry",
    )?;

    let count = |mnemonic: &str| {
        instructions
            .iter()
            .filter(|instruction| instruction.mnemonic == mnemonic)
            .count()
    };
    require(
        count("s_barrier") == 2,
        "final ISA must contain two s_barrier",
    )?;
    require(
        count("v_mfma_f32_16x16x16_bf16") == 1,
        "final ISA must contain one BF16 MFMA",
    )?;
    let mfma = instructions
        .iter()
        .find(|instruction| instruction.mnemonic == "v_mfma_f32_16x16x16_bf16")
        .expect("required by exact cardinality");
    let loop_backedges = instructions
        .iter()
        .map(|instruction| {
            instruction
                .scalar_branch_target()
                .map(|target| target.map(|target| (instruction, target)))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    require(
        loop_backedges.iter().any(|(branch, target)| {
            *target <= mfma.address && mfma.address < branch.address && *target < branch.address
        }),
        "final ISA does not place the BF16 MFMA inside an observed scalar loop backedge",
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("ds_write")),
        "final ISA contains no LDS write",
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("ds_read")),
        "final ISA contains no LDS read",
    )?;
    require(
        instructions
            .iter()
            .filter(|instruction| instruction.mnemonic.starts_with("ds_"))
            .all(|instruction| {
                instruction.mnemonic.starts_with("ds_read")
                    || instruction.mnemonic.starts_with("ds_write")
            }),
        "final ISA contains an unreviewed LDS instruction",
    )?;
    require(
        instructions
            .iter()
            .any(|instruction| instruction.mnemonic.starts_with("global_load_"))
            && instructions
                .iter()
                .any(|instruction| instruction.mnemonic.starts_with("global_store_")),
        "final ISA omits a global load or store",
    )?;
    require(
        !instructions.iter().any(|instruction| {
            instruction.mnemonic.contains("atomic")
                || instruction.mnemonic.starts_with("scratch_")
                || instruction.mnemonic == "flat_scratch"
                || instruction.mnemonic.starts_with("s_call")
                || instruction.mnemonic.starts_with("s_swappc")
                || instruction.mnemonic.starts_with("s_getpc")
                || instruction.mnemonic.starts_with("s_setpc")
        }),
        "final ISA contains an atomic, scratch operation, or machine call",
    )?;
    require(
        count("s_endpgm") == 1,
        "final ISA must contain one termination instruction",
    )?;
    Ok(instructions)
}

#[cfg(feature = "hardware-test-hooks")]
#[derive(Debug)]
struct PinnedTool {
    variable: &'static str,
    path: std::path::PathBuf,
    bytes_digest: [u8; 32],
    device: u64,
    inode: u64,
    len: u64,
}

#[cfg(feature = "hardware-test-hooks")]
impl PinnedTool {
    fn read(variable: &'static str, digest_variable: &'static str) -> Result<Self, BoxError> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let path = std::path::PathBuf::from(
            std::env::var_os(variable).ok_or_else(|| format!("{variable} is not set"))?,
        );
        require(path.is_absolute(), format!("{variable} must be absolute"))?;
        let metadata = std::fs::symlink_metadata(&path)?;
        require(
            metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.permissions().mode() & 0o111 != 0
                && (1..=512 * 1024 * 1024).contains(&metadata.len()),
            format!("{variable} must be a bounded executable regular file"),
        )?;
        require(
            std::fs::canonicalize(&path)? == path,
            format!("{variable} must already be canonical"),
        )?;
        let expected = parse_exact_sha256(
            digest_variable,
            &std::env::var(digest_variable).map_err(|_| format!("{digest_variable} is not set"))?,
        )?;
        let bytes = std::fs::read(&path)?;
        require(
            bytes.len() as u64 == metadata.len() && sha256(&bytes) == expected,
            format!("{variable} does not match its exact SHA-256 pin"),
        )?;
        let tool = Self {
            variable,
            path,
            bytes_digest: expected,
            device: metadata.dev(),
            inode: metadata.ino(),
            len: metadata.len(),
        };
        tool.verify()?;
        Ok(tool)
    }

    fn verify(&self) -> Result<(), BoxError> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let metadata = std::fs::symlink_metadata(&self.path)?;
        require(
            metadata.file_type().is_file()
                && !metadata.file_type().is_symlink()
                && metadata.permissions().mode() & 0o111 != 0
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
                && metadata.len() == self.len
                && std::fs::canonicalize(&self.path)? == self.path
                && sha256(&std::fs::read(&self.path)?) == self.bytes_digest,
            format!("{} changed after its bytes were admitted", self.variable),
        )
    }

    fn output<I, S>(&self, arguments: I) -> Result<std::process::Output, BoxError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        use std::process::Stdio;

        self.verify()?;
        let output = std::process::Command::new(&self.path)
            .args(arguments)
            .env_clear()
            .env("HOME", "/nonexistent")
            .env("LC_ALL", "C")
            .env("PATH", "/usr/bin:/bin")
            .env("TZ", "UTC")
            .stdin(Stdio::null())
            .output()?;
        self.verify()?;
        require(
            output.stdout.len() <= 16 * 1024 * 1024 && output.stderr.len() <= 16 * 1024 * 1024,
            format!("{} emitted unbounded output", self.variable),
        )?;
        Ok(output)
    }

    fn require_version(&self, marker: &str) -> Result<(), BoxError> {
        let output = self.output(["--version"])?;
        require(
            output.status.success() && output.stderr.is_empty(),
            format!("{} --version failed or emitted stderr", self.variable),
        )?;
        let stdout = std::str::from_utf8(&output.stdout)?;
        require(
            stdout
                .lines()
                .next()
                .is_some_and(|line| line.contains(marker)),
            format!(
                "{} must report upstream LLVM 22, found {stdout:?}",
                self.variable
            ),
        )
    }
}

#[cfg(feature = "hardware-test-hooks")]
struct Toolchain {
    llc: PinnedTool,
    lld: PinnedTool,
    objdump: PinnedTool,
}

#[cfg(feature = "hardware-test-hooks")]
impl Toolchain {
    fn from_environment() -> Result<Self, BoxError> {
        require(
            std::env::var("FE2O3_RUN_GFX942_TILED_GEMM_LDS_K32_V2_HARDWARE").as_deref() == Ok("1"),
            "set FE2O3_RUN_GFX942_TILED_GEMM_LDS_K32_V2_HARDWARE=1 to opt in",
        )?;
        for (variable, expected) in [
            ("HSA_XNACK", "0"),
            ("HIP_VISIBLE_DEVICES", "0"),
            ("ROCR_VISIBLE_DEVICES", "0"),
        ] {
            require(
                std::env::var(variable).as_deref() == Ok(expected),
                format!("{variable} must equal {expected:?}"),
            )?;
        }
        let tools = Self {
            llc: PinnedTool::read("FE2O3_LLC", "FE2O3_LLC_SHA256")?,
            lld: PinnedTool::read("FE2O3_LLD", "FE2O3_LLD_SHA256")?,
            objdump: PinnedTool::read("FE2O3_LLVM_OBJDUMP", "FE2O3_LLVM_OBJDUMP_SHA256")?,
        };
        tools.llc.require_version("LLVM version 22.")?;
        tools.lld.require_version("LLD 22.")?;
        tools.objdump.require_version("LLVM version 22.")?;
        Ok(tools)
    }

    fn verify(&self) -> Result<(), BoxError> {
        self.llc.verify()?;
        self.lld.verify()?;
        self.objdump.verify()
    }
}

#[cfg(feature = "hardware-test-hooks")]
struct TemporaryDirectory(std::path::PathBuf);

#[cfg(feature = "hardware-test-hooks")]
impl TemporaryDirectory {
    fn new() -> Result<Self, BoxError> {
        use std::os::unix::fs::DirBuilderExt;

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        for attempt in 0..64_u32 {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-tiled-gemm-lds-k32-v2-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not create a private temporary directory".into())
    }

    fn join(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

#[cfg(feature = "hardware-test-hooks")]
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.0).is_err() {
            std::process::abort();
        }
    }
}

#[cfg(feature = "hardware-test-hooks")]
struct ObservationalArtifact {
    bytes: Vec<u8>,
    digest: PayloadDigest,
    disassembly: String,
}

#[cfg(feature = "hardware-test-hooks")]
fn checked_tool_output(
    tool: &PinnedTool,
    arguments: Vec<std::ffi::OsString>,
    operation: &str,
) -> Result<(), BoxError> {
    let output = tool.output(arguments)?;
    require(
        output.status.success() && output.stdout.is_empty() && output.stderr.is_empty(),
        format!(
            "{operation} failed or emitted output; stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn generate_observational_artifact(tools: &Toolchain) -> Result<ObservationalArtifact, BoxError> {
    let llvm = lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir(
        &tiled_gemm_lds_k32_v2_module(),
        TiledGemmLdsK32V2Profile::exact_gfx942_xnack_minus_cov6(),
    )?
    .into_string();
    require(
        !llvm.to_ascii_lowercase().contains("comgr"),
        "dialect output contains a COMGR marker",
    )?;
    let directory = TemporaryDirectory::new()?;
    let input = directory.join("slice2.ll");
    let object = directory.join("slice2.o");
    let hsaco = directory.join("slice2.hsaco");
    std::fs::write(&input, llvm.as_bytes())?;
    checked_tool_output(
        &tools.llc,
        vec![
            "-mtriple=amdgcn-amd-amdhsa".into(),
            "-mcpu=gfx942".into(),
            "-mattr=-xnack".into(),
            "--amdhsa-code-object-version=6".into(),
            "-filetype=obj".into(),
            "-O=2".into(),
            input.as_os_str().to_owned(),
            "-o".into(),
            object.as_os_str().to_owned(),
        ],
        "upstream llc Slice 2 compilation",
    )?;
    checked_tool_output(
        &tools.lld,
        vec![
            "-shared".into(),
            "--no-undefined".into(),
            object.as_os_str().to_owned(),
            "-o".into(),
            hsaco.as_os_str().to_owned(),
        ],
        "upstream ld.lld Slice 2 link",
    )?;
    tools.verify()?;
    let bytes = std::fs::read(&hsaco)?;
    require(
        (1..=fe2o3_hsaco::MAX_HSACO_BYTES).contains(&bytes.len()),
        "generated Slice 2 HSACO has an invalid bounded length",
    )?;
    for forbidden in [b"amd_comgr".as_slice(), b"libamd_comgr".as_slice()] {
        require(
            !bytes
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "generated Slice 2 HSACO contains a COMGR reference",
        )?;
    }
    let entry = inspect_metadata(&bytes)?;
    let disassembly_output = tools.objdump.output([
        std::ffi::OsString::from(format!(
            "--disassemble-symbols={TILED_GEMM_LDS_K32_V2_KERNEL_ID}"
        )),
        std::ffi::OsString::from(format!("--start-address=0x{:x}", entry.address)),
        std::ffi::OsString::from(format!("--stop-address=0x{:x}", entry.end()?)),
        std::ffi::OsString::from("--mcpu=gfx942"),
        hsaco.as_os_str().to_owned(),
    ])?;
    require(
        disassembly_output.status.success() && disassembly_output.stderr.is_empty(),
        format!(
            "upstream llvm-objdump rejected generated Slice 2: {}",
            String::from_utf8_lossy(&disassembly_output.stderr)
        ),
    )?;
    let disassembly = String::from_utf8(disassembly_output.stdout)?;
    let _ = validate_isa(&disassembly, entry)?;
    tools.verify()?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    Ok(ObservationalArtifact {
        bytes,
        digest,
        disassembly,
    })
}

fn bf16_to_f32(bits: u16) -> f32 {
    f32::from_bits(u32::from(bits) << 16)
}

#[derive(Clone, Debug)]
struct HardwareCase {
    name: &'static str,
    a: Vec<u16>,
    b: Vec<u16>,
    exact: bool,
}

fn hardware_cases() -> Vec<HardwareCase> {
    let zeros = HardwareCase {
        name: "zero",
        a: vec![0; INPUT_ELEMENTS],
        b: vec![0; INPUT_ELEMENTS],
        exact: true,
    };

    let mut identity_a = vec![0; INPUT_ELEMENTS];
    let mut identity_b = vec![0; INPUT_ELEMENTS];
    let identity_palette = [0x3f80, 0xbf80, 0x3f00, 0xbf00, 0x4000, 0xc000];
    for row in 0..TILE {
        identity_a[row * DEPTH + row] = 0x3f80;
        identity_a[row * DEPTH + TILE + row] = 0x3f00;
    }
    for depth in 0..DEPTH {
        for column in 0..TILE {
            identity_b[depth * TILE + column] =
                identity_palette[(7 * depth + 3 * column) % identity_palette.len()];
        }
    }
    let identity = HardwareCase {
        name: "identity-like",
        a: identity_a,
        b: identity_b,
        exact: true,
    };

    let dyadic_palette = [0x3f80, 0xbf80, 0x3f00, 0xbf00, 0x3e80, 0xbe80];
    let mut dyadic_a = vec![0; INPUT_ELEMENTS];
    let mut dyadic_b = vec![0; INPUT_ELEMENTS];
    for row in 0..TILE {
        for depth in 0..DEPTH {
            dyadic_a[row * DEPTH + depth] =
                dyadic_palette[(row + 3 * depth) % dyadic_palette.len()];
        }
    }
    for depth in 0..DEPTH {
        for column in 0..TILE {
            dyadic_b[depth * TILE + column] =
                dyadic_palette[(2 * depth + column + 1) % dyadic_palette.len()];
        }
    }
    let dyadic = HardwareCase {
        name: "dyadic",
        a: dyadic_a,
        b: dyadic_b,
        exact: true,
    };

    let random_palette = [
        0x3c80, 0xbc80, 0x3d80, 0xbd80, 0x3e80, 0xbe80, 0x3f00, 0xbf00, 0x3f80, 0xbf80, 0x4000,
        0xc000, 0x4080, 0xc080,
    ];
    let mut state = 0x6a09_e667_f3bc_c909_u64;
    let mut next_random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        random_palette[(state as usize) % random_palette.len()]
    };
    let random = HardwareCase {
        name: "deterministic-random",
        a: (0..INPUT_ELEMENTS).map(|_| next_random()).collect(),
        b: (0..INPUT_ELEMENTS).map(|_| next_random()).collect(),
        exact: false,
    };

    let mut cancellation_a = vec![0; INPUT_ELEMENTS];
    let mut cancellation_b = vec![0; INPUT_ELEMENTS];
    for row in 0..TILE {
        for depth in 0..DEPTH {
            cancellation_a[row * DEPTH + depth] = if (row + depth).is_multiple_of(2) {
                0x4100
            } else {
                0xc100
            };
        }
    }
    for depth in 0..DEPTH {
        for column in 0..TILE {
            cancellation_b[depth * TILE + column] = if (depth + column).is_multiple_of(2) {
                0x3f80
            } else {
                0xbf80
            };
        }
    }
    let cancellation = HardwareCase {
        name: "signed-cancellation",
        a: cancellation_a,
        b: cancellation_b,
        exact: false,
    };

    let mut adversarial_a = vec![0; INPUT_ELEMENTS];
    let mut adversarial_b = vec![0; INPUT_ELEMENTS];
    for row in 0..TILE {
        for depth in 0..DEPTH {
            let magnitude = if (depth & 3) < 2 { 0x7f7f } else { 0x0080 };
            let sign = if (row + depth).is_multiple_of(2) {
                0
            } else {
                0x8000
            };
            adversarial_a[row * DEPTH + depth] = magnitude ^ sign;
        }
    }
    for depth in 0..DEPTH {
        for column in 0..TILE {
            let sign = if column.is_multiple_of(2) { 0 } else { 0x8000 };
            adversarial_b[depth * TILE + column] = match depth & 3 {
                0 | 1 => 0x0080 ^ sign,
                _ => 0x7f7f ^ sign,
            };
        }
    }
    let adversarial = HardwareCase {
        name: "adversarial-finite-bf16",
        a: adversarial_a,
        b: adversarial_b,
        exact: false,
    };

    vec![zeros, identity, dyadic, random, cancellation, adversarial]
}

#[derive(Clone, Copy, Debug)]
struct OracleValue {
    rounded: f32,
    absolute_tolerance: f64,
}

fn gemm_oracle(case: &HardwareCase) -> Result<Vec<OracleValue>, BoxError> {
    require(
        case.a.len() == INPUT_ELEMENTS && case.b.len() == INPUT_ELEMENTS,
        format!(
            "{} does not contain exact 16x32 and 32x16 inputs",
            case.name
        ),
    )?;
    let mut output = Vec::with_capacity(OUTPUT_ELEMENTS);
    for row in 0..TILE {
        for column in 0..TILE {
            let mut sum = 0.0_f64;
            let mut absolute_sum = 0.0_f64;
            for depth in 0..DEPTH {
                let lhs = f64::from(bf16_to_f32(case.a[row * DEPTH + depth]));
                let rhs = f64::from(bf16_to_f32(case.b[depth * TILE + column]));
                let product = lhs * rhs;
                sum += product;
                absolute_sum += product.abs();
            }
            require(
                sum.is_finite() && absolute_sum.is_finite(),
                format!("{} oracle produced a non-finite result", case.name),
            )?;
            let rounded = sum as f32;
            require(
                rounded.is_finite(),
                format!("{} result is outside finite FP32", case.name),
            )?;
            let rounding_allowance = if case.exact {
                0.0
            } else {
                // This is deliberately an observational tolerance, not a
                // machine-arithmetic proof. It bounds many FP32 summation
                // orders over 32 exact BF16 products with ample audit margin.
                128.0 * f64::from(f32::EPSILON) * absolute_sum + f64::from(f32::MIN_POSITIVE)
            };
            output.push(OracleValue {
                rounded,
                absolute_tolerance: rounding_allowance,
            });
        }
    }
    Ok(output)
}

fn validate_output(
    case: &HardwareCase,
    observed: &[f32],
    oracle: &[OracleValue],
) -> Result<(), BoxError> {
    require(
        observed.len() == OUTPUT_ELEMENTS && oracle.len() == OUTPUT_ELEMENTS,
        format!("{} output or oracle extent changed", case.name),
    )?;
    for (index, (actual, expected)) in observed.iter().zip(oracle).enumerate() {
        let valid = if case.exact {
            actual.to_bits() == expected.rounded.to_bits()
        } else {
            actual.is_finite()
                && (f64::from(*actual) - f64::from(expected.rounded)).abs()
                    <= expected.absolute_tolerance
        };
        require(
            valid,
            format!(
                "{} output[{index}] ({}, {}) differs: observed={:#010x} ({actual:e}), \
                 expected={:#010x} ({:e}), tolerance={:e}",
                case.name,
                index / TILE,
                index % TILE,
                actual.to_bits(),
                expected.rounded.to_bits(),
                expected.rounded,
                expected.absolute_tolerance,
            ),
        )?;
    }
    Ok(())
}

fn guarded_u16(body: &[u16], prefix: u16, suffix: u16) -> Vec<u16> {
    let mut values = Vec::with_capacity(body.len() + 2 * CANARY_ELEMENTS);
    values.extend(std::iter::repeat_n(prefix, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, CANARY_ELEMENTS));
    values
}

fn guarded_f32(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(body.len() + 2 * CANARY_ELEMENTS);
    values.extend(std::iter::repeat_n(prefix, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, CANARY_ELEMENTS));
    values
}

type GuardedParts<'a, T> = (&'a [T], &'a [T], &'a [T]);

fn split_guarded<T>(values: &[T], body_elements: usize) -> Result<GuardedParts<'_, T>, BoxError> {
    require(
        values.len() == body_elements + 2 * CANARY_ELEMENTS,
        "guarded allocation has the wrong extent",
    )?;
    let (prefix, rest) = values.split_at(CANARY_ELEMENTS);
    let (body, suffix) = rest.split_at(body_elements);
    Ok((prefix, body, suffix))
}

fn verify_guarded_u16(
    role: &str,
    actual: &[u16],
    expected_body: &[u16],
    prefix: u16,
    suffix: u16,
) -> Result<(), BoxError> {
    let (actual_prefix, actual_body, actual_suffix) = split_guarded(actual, expected_body.len())?;
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

fn verify_output_allocation(
    case: &HardwareCase,
    actual: &[f32],
    oracle: &[OracleValue],
) -> Result<(), BoxError> {
    let (prefix, body, suffix) = split_guarded(actual, OUTPUT_ELEMENTS)?;
    require(
        prefix
            .iter()
            .all(|value| value.to_bits() == C_PREFIX.to_bits()),
        format!("{} C prefix canary changed", case.name),
    )?;
    validate_output(case, body, oracle)?;
    require(
        suffix
            .iter()
            .all(|value| value.to_bits() == C_SUFFIX.to_bits()),
        format!("{} C suffix canary changed", case.name),
    )
}

fn launch_geometry() -> HsaLaunchGeometryV1 {
    // The adapter's grid is a block count; it expands this to 64 AQL work-items.
    HsaLaunchGeometryV1::new([1, 1, 1], [WORKGROUP_X, 1, 1], 0)
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn explicit_kernarg(addresses: [u64; 3]) -> [u8; EXPLICIT_KERNARG_BYTES] {
    let mut bytes = [0; EXPLICIT_KERNARG_BYTES];
    for (index, (address, elements)) in addresses
        .into_iter()
        .zip([INPUT_ELEMENTS, INPUT_ELEMENTS, OUTPUT_ELEMENTS])
        .enumerate()
    {
        let offset = index * 16;
        put_u64(&mut bytes, offset, address);
        put_u64(&mut bytes, offset + 8, elements as u64);
    }
    bytes
}

#[cfg(feature = "hardware-test-hooks")]
fn u16_bytes(values: &[u16]) -> &[u8] {
    // SAFETY: u16 has no invalid bit patterns and the byte extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 has no invalid bit patterns and the byte extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn u16_values(bytes: &[u8]) -> Result<Vec<u16>, BoxError> {
    require(
        bytes.len().is_multiple_of(std::mem::size_of::<u16>()),
        "hardware allocation contains a partial u16",
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
        "hardware allocation contains a partial f32",
    )?;
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().expect("exact f32 chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn body_address(
    buffer: &ReviewedHsaHardwareTestBufferV1,
    body_elements: usize,
    element_size: usize,
) -> Result<u64, BoxError> {
    require(
        buffer.byte_len() == (body_elements + 2 * CANARY_ELEMENTS) * element_size,
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
        // SAFETY: layout is valid and this owner deallocates the result once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("failed to allocate aligned Slice 2 kernarg")?;
        Ok(Self { pointer, layout })
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: the allocation is live and exactly layout.size() bytes.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}

#[cfg(feature = "hardware-test-hooks")]
impl Drop for RuntimeKernarg {
    fn drop(&mut self) {
        // SAFETY: this owner deallocates its exact live allocation once.
        unsafe { std::alloc::dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

#[cfg(feature = "hardware-test-hooks")]
unsafe fn dispatch_one_workgroup(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    explicit: &[u8; EXPLICIT_KERNARG_BYTES],
) -> Result<(), BoxError> {
    require(
        resolution.export_symbol() == TILED_GEMM_LDS_K32_V2_KERNEL_ID
            && resolution.kernarg_segment_size() == COMPLETE_KERNARG_BYTES as u64
            && resolution.kernarg_segment_alignment() == HSA_KERNARG_ALIGNMENT,
        "runtime resolution differs from exact Slice 2 symbol/kernarg ABI",
    )?;
    let geometry = launch_geometry();
    require(
        geometry.grid() == [1, 1, 1]
            && geometry.workgroup() == [WORKGROUP_X, 1, 1]
            && geometry.dynamic_shared_memory_bytes() == 0,
        "Slice 2 launch must be exactly one static-LDS WG64",
    )?;
    let mut storage = RuntimeKernarg::new()?;
    let kernarg = storage.bytes_mut();
    kernarg[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);

    // SAFETY: metadata and ISA inspection admitted the generated single-kernel
    // image. Three live guarded buffers supply the exact 48-byte slice ABI;
    // the adapter initializes all COV6 hidden bytes and waits synchronously.
    unsafe {
        adapter.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            EXPLICIT_KERNARG_BYTES,
            EXPLICIT_KERNARG_BYTES,
            IMPLICIT_KERNARG_BYTES,
            kernarg,
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(completion.completed(), "Slice 2 dispatch did not complete")?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn execute_case(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    case: &HardwareCase,
) -> Result<(), BoxError> {
    let oracle = gemm_oracle(case)?;
    let a_host = guarded_u16(&case.a, A_PREFIX, A_SUFFIX);
    let b_host = guarded_u16(&case.b, B_PREFIX, B_SUFFIX);
    let c_host = guarded_f32(&[C_POISON; OUTPUT_ELEMENTS], C_PREFIX, C_SUFFIX);
    let a = adapter.allocate_hardware_test_buffer(u16_bytes(&a_host))?;
    let b = adapter.allocate_hardware_test_buffer(u16_bytes(&b_host))?;
    let c = adapter.allocate_hardware_test_buffer(f32_bytes(&c_host))?;
    let explicit = explicit_kernarg([
        body_address(&a, INPUT_ELEMENTS, std::mem::size_of::<u16>())?,
        body_address(&b, INPUT_ELEMENTS, std::mem::size_of::<u16>())?,
        body_address(&c, OUTPUT_ELEMENTS, std::mem::size_of::<f32>())?,
    ]);

    // SAFETY: dispatch_one_workgroup owns and documents the sole raw launch
    // boundary and returns only after all three allocations are idle.
    unsafe {
        dispatch_one_workgroup(adapter, executable, kernel, resolution, &explicit)?;
    }

    let a_after = u16_values(&a.read_after_synchronous_dispatch())?;
    let b_after = u16_values(&b.read_after_synchronous_dispatch())?;
    let c_after = f32_values(&c.read_after_synchronous_dispatch())?;
    verify_guarded_u16(
        &format!("{} immutable A", case.name),
        &a_after,
        &case.a,
        A_PREFIX,
        A_SUFFIX,
    )?;
    verify_guarded_u16(
        &format!("{} immutable B", case.name),
        &b_after,
        &case.b,
        B_PREFIX,
        B_SUFFIX,
    )?;
    verify_output_allocation(case, &c_after, &oracle)
}

#[cfg(feature = "hardware-test-hooks")]
fn run_observational_hardware_evidence() -> Result<(), BoxError> {
    let tools = Toolchain::from_environment()?;
    let artifact = generate_observational_artifact(&tools)?;
    require(
        !artifact.disassembly.is_empty(),
        "retained observational disassembly is empty",
    )?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    let physical_target = adapter.environment().physical_device().target();
    require(
        physical_target.processor() == "gfx942"
            && physical_target.xnack() == Some(FeatureState::Disabled),
        "Slice 2 hardware evidence requires a gfx942:xnack- physical device",
    )?;

    // SAFETY: exact generated bytes are retained and digest-bound after
    // metadata/ISA inspection. This still authenticates no source or publisher.
    let (executable, load) = unsafe { adapter.load_executable(&artifact.bytes, artifact.digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(
            load.finalized_digest() == artifact.digest
                && load.byte_len() == artifact.bytes.len() as u64,
            "loaded bytes differ from generated Slice 2 artifact",
        )?;
        // SAFETY: inspection admitted one exact symbol and descriptor.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [TILED_GEMM_LDS_K32_V2_KERNEL_ID]) }?;
        require(
            kernels.len() == 1
                && resolutions.len() == 1
                && resolutions[0].export_symbol() == TILED_GEMM_LDS_K32_V2_KERNEL_ID
                && resolutions[0].executable_object() == executable_identity,
            "runtime resolved a substituted Slice 2 kernel",
        )?;
        let kernel = kernels.get(0).ok_or("runtime omitted Slice 2 kernel")?;
        for case in hardware_cases() {
            execute_case(&mut adapter, &executable, kernel, &resolutions[0], &case)?;
        }
        Ok(())
    })();

    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(
        unload.released() && unload.executable_object() == executable_identity,
        "reviewed HSA unload did not release the exact Slice 2 executable",
    )?;
    tools.verify()?;
    execution
}

/// Generates and executes the fixed LDS-tiled `16x16x32` Slice 2 on MI300X.
///
/// This is observational evidence only. It does not establish source
/// correspondence, compiler refinement, proof validity, Worker V2 admission,
/// or publisher authority. In particular, it makes no illegal-access-absence
/// or race-freedom claim. Guard values detect changed in-allocation values,
/// but not beyond-guard accesses, value-preserving writes, or races.
///
/// ```text
/// FE2O3_RUN_GFX942_TILED_GEMM_LDS_K32_V2_HARDWARE=1 \
/// HSA_XNACK=0 HIP_VISIBLE_DEVICES=0 ROCR_VISIBLE_DEVICES=0 \
/// FE2O3_LLC=/absolute/canonical/llc FE2O3_LLC_SHA256=<sha256> \
/// FE2O3_LLD=/absolute/canonical/ld.lld FE2O3_LLD_SHA256=<sha256> \
/// FE2O3_LLVM_OBJDUMP=/absolute/canonical/llvm-objdump \
/// FE2O3_LLVM_OBJDUMP_SHA256=<sha256> \
/// cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test tiled_gemm_lds_k32_v2_hardware \
///   gfx942_tiled_gemm_lds_k32_v2_observational_hardware_evidence \
///   -- --ignored --exact --nocapture
/// ```
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "observational: requires SHA-pinned upstream LLVM 22 and gfx942:xnack-"]
fn gfx942_tiled_gemm_lds_k32_v2_observational_hardware_evidence() -> Result<(), BoxError> {
    run_observational_hardware_evidence()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_entry() -> BoundKernelEntry {
        BoundKernelEntry {
            address: 0x1000,
            size: 44,
        }
    }

    fn valid_disassembly() -> String {
        format!(
            "slice2.hsaco:\tfile format elf64-amdgpu\n\n\
             Disassembly of section .text:\n\n\
             0000000000001000 <{TILED_GEMM_LDS_K32_V2_KERNEL_ID}>:\n\
             \tglobal_load_dword v0, v[0:1], off // 000000001000: DEADBEEF\n\
             \tds_write_b32 v0, v1 // 000000001004: DEADBEEF\n\
             \ts_barrier // 000000001008: DEADBEEF\n\
             \tds_read_b32 v0, v1 // 00000000100C: DEADBEEF\n\
             \tv_mfma_f32_16x16x16_bf16 a[0:3], v0, v1, a[0:3] // 000000001010: DEADBEEF FEEDFACE\n\
             \ts_barrier // 000000001018: DEADBEEF\n\
             \ts_cmp_lt_u32 s0, 2 // 00000000101C: DEADBEEF\n\
             \ts_cbranch_scc1 65528 // 000000001020: DEADBEEF <{TILED_GEMM_LDS_K32_V2_KERNEL_ID}+0x4>\n\
             \tglobal_store_dwordx4 v[0:1], v[0:3], off // 000000001024: DEADBEEF\n\
             \ts_endpgm // 000000001028: DEADBEEF\n"
        )
    }

    #[test]
    fn launch_and_kernarg_are_exactly_one_wg64_and_three_slices() {
        let geometry = launch_geometry();
        assert_eq!(geometry.grid(), [1, 1, 1]);
        assert_eq!(geometry.workgroup(), [64, 1, 1]);
        assert_eq!(geometry.dynamic_shared_memory_bytes(), 0);
        let packed = explicit_kernarg([0x11, 0x22, 0x33]);
        for (index, (address, elements)) in [0x11_u64, 0x22, 0x33]
            .into_iter()
            .zip([512_u64, 512, 256])
            .enumerate()
        {
            let offset = index * 16;
            assert_eq!(&packed[offset..offset + 8], &address.to_le_bytes());
            assert_eq!(&packed[offset + 8..offset + 16], &elements.to_le_bytes());
        }
        assert_eq!(DEPTH, TILED_GEMM_LDS_K32_V2_K as usize);
        assert_eq!(
            INPUT_ELEMENTS,
            TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS as usize
        );
        assert_eq!(
            OUTPUT_ELEMENTS,
            TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS as usize
        );
        assert_eq!(EXPLICIT_KERNARG_BYTES, 48);
        assert_eq!(COMPLETE_KERNARG_BYTES, 304);
        assert_eq!(STATIC_LDS_BYTES, 1024);
    }

    #[test]
    fn metadata_validator_rejects_every_pinned_profile_class() {
        let expected = MetadataFacts::expected();
        validate_metadata(&expected).unwrap();
        let mutations: [fn(&mut MetadataFacts); 21] = [
            |facts| facts.code_object_version = 5,
            |facts| facts.target = "gfx942:xnack+".to_owned(),
            |facts| facts.has_printf_metadata = true,
            |facts| facts.kernel_count = 2,
            |facts| facts.kernel_name = "substituted".to_owned(),
            |facts| facts.descriptor_symbol = "substituted.kd".to_owned(),
            |facts| facts.kernarg_size = 320,
            |facts| facts.implicit_offset = Some(64),
            |facts| facts.required_workgroup = Some([256, 1, 1]),
            |facts| facts.wavefront_size = 32,
            |facts| facts.group_segment_size = 0,
            |facts| facts.private_segment_size = 4,
            |facts| facts.sgpr_spill_count = 1,
            |facts| facts.vgpr_spill_count = 1,
            |facts| facts.uses_dynamic_stack = true,
            |facts| facts.arguments[1].kind = ExplicitValueKind::GlobalBuffer,
            |facts| facts.arguments[0].address_space = None,
            |facts| facts.binding_count = 0,
            |facts| facts.descriptor_group_segment_size = 0,
            |facts| facts.descriptor_private_segment_enabled = true,
            |facts| facts.descriptor_uses_dynamic_stack = true,
        ];
        for mutate in mutations {
            let mut hostile = expected.clone();
            mutate(&mut hostile);
            assert!(validate_metadata(&hostile).is_err());
        }
    }

    #[test]
    fn isa_validator_requires_two_barriers_and_loop_body_mfma_and_rejects_forbidden_ops() {
        let valid = valid_disassembly();
        validate_isa(&valid, valid_entry()).unwrap();
        for hostile in [
            valid.replace("ds_write_b32", "v_mov_b32"),
            valid.replace("ds_read_b32", "v_mov_b32"),
            valid.replacen("s_barrier", "s_nop", 1),
            valid.replace("v_mfma_f32_16x16x16_bf16", "v_add_f32"),
            valid.replace("s_cbranch_scc1 65528", "s_cbranch_scc1 1"),
            valid.replace("s_cbranch_scc1", "s_nop"),
            valid.replace("global_load_dword", "v_mov_b32"),
            valid.replace("global_store_dwordx4", "v_mov_b32"),
            valid.replace("s_endpgm", "global_atomic_add"),
            valid.replace("s_endpgm", "s_call_b64"),
            valid.replace("s_endpgm", "s_swappc_b64"),
            valid.replace("s_endpgm", "s_setpc_b64"),
            valid.replace("s_endpgm", "scratch_store_dword"),
            valid.replace("ds_read_b32", "ds_bpermute_b32"),
        ] {
            assert!(validate_isa(&hostile, valid_entry()).is_err());
        }
    }

    #[test]
    fn corpus_covers_required_finite_classes_and_all_outputs() {
        let cases = hardware_cases();
        assert_eq!(
            cases.iter().map(|case| case.name).collect::<Vec<_>>(),
            [
                "zero",
                "identity-like",
                "dyadic",
                "deterministic-random",
                "signed-cancellation",
                "adversarial-finite-bf16",
            ]
        );
        for case in cases {
            assert_eq!(case.a.len(), INPUT_ELEMENTS);
            assert_eq!(case.b.len(), INPUT_ELEMENTS);
            assert!(
                case.a
                    .iter()
                    .chain(&case.b)
                    .all(|bits| bf16_to_f32(*bits).is_finite())
            );
            let oracle = gemm_oracle(&case).unwrap();
            assert_eq!(oracle.len(), OUTPUT_ELEMENTS);
            let observed = oracle.iter().map(|value| value.rounded).collect::<Vec<_>>();
            validate_output(&case, &observed, &oracle).unwrap();
        }
    }

    #[test]
    fn numerical_validator_checks_each_output_and_detects_substitution() {
        for case in hardware_cases() {
            let oracle = gemm_oracle(&case).unwrap();
            let valid = oracle.iter().map(|value| value.rounded).collect::<Vec<_>>();
            validate_output(&case, &valid, &oracle).unwrap();
            let poison = vec![C_POISON; OUTPUT_ELEMENTS];
            assert!(validate_output(&case, &poison, &oracle).is_err());
            for index in [0, 137, OUTPUT_ELEMENTS - 1] {
                let mut hostile = valid.clone();
                hostile[index] = if case.exact {
                    f32::from_bits(hostile[index].to_bits() ^ 1)
                } else {
                    hostile[index] + (oracle[index].absolute_tolerance as f32 * 4.0 + 1.0)
                };
                assert!(validate_output(&case, &hostile, &oracle).is_err());
            }
        }
    }

    #[test]
    fn canary_validators_cover_all_three_allocation_boundaries() {
        let body = vec![0x3f80; INPUT_ELEMENTS];
        for (role, prefix, suffix) in [("A", A_PREFIX, A_SUFFIX), ("B", B_PREFIX, B_SUFFIX)] {
            let valid = guarded_u16(&body, prefix, suffix);
            verify_guarded_u16(role, &valid, &body, prefix, suffix).unwrap();
            for index in [0, CANARY_ELEMENTS, valid.len() - 1] {
                let mut hostile = valid.clone();
                hostile[index] ^= 1;
                assert!(verify_guarded_u16(role, &hostile, &body, prefix, suffix).is_err());
            }
        }

        let case = &hardware_cases()[0];
        let oracle = gemm_oracle(case).unwrap();
        let body = oracle.iter().map(|value| value.rounded).collect::<Vec<_>>();
        let valid = guarded_f32(&body, C_PREFIX, C_SUFFIX);
        verify_output_allocation(case, &valid, &oracle).unwrap();
        for index in [0, CANARY_ELEMENTS, valid.len() - 1] {
            let mut hostile = valid.clone();
            hostile[index] = f32::from_bits(hostile[index].to_bits() ^ 1);
            assert!(verify_output_allocation(case, &hostile, &oracle).is_err());
        }
    }

    #[test]
    fn digest_parser_is_exact_and_lowercase() {
        assert!(parse_exact_sha256("PIN", &"ab".repeat(32)).is_ok());
        assert!(parse_exact_sha256("PIN", &"AB".repeat(32)).is_err());
        assert!(parse_exact_sha256("PIN", &"0".repeat(63)).is_err());
    }
}
