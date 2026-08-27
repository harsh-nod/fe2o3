//! Non-authoritative gfx950 advanced-kernel hardware and numerical harness.
//!
//! Each ignored test consumes one caller-supplied, digest-pinned COV6 HSACO.
//! It validates the exact single-kernel ABI before using the reviewed raw HSA
//! adapter. This bypasses protected publication authority and exists only to
//! test Rust-produced artifacts on a directly visible gfx950 device.

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_amd_target::FeatureState;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_core::GpuContext;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_host::{
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaHardwareTestBufferV1, ReviewedHsaKernelV1,
    ReviewedHsaRuntimeAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsaco::{CodeObjectVersion, ExplicitValueKind};

#[cfg(feature = "hardware-test-hooks")]
const CHANNELS_V1: usize = 16;
#[cfg(feature = "hardware-test-hooks")]
const KDA_TAPS_V1: usize = 3;
#[cfg(feature = "hardware-test-hooks")]
const PREFILL_TOKENS_V1: usize = 8;
#[cfg(feature = "hardware-test-hooks")]
const ATTENTION_TOKENS_V1: usize = 16;
#[cfg(feature = "hardware-test-hooks")]
const HEAD_DIMENSION_V1: usize = 128;
#[cfg(feature = "hardware-test-hooks")]
const TOKENS_PER_BLOCK_V1: usize = 4;
#[cfg(feature = "hardware-test-hooks")]
const SELECTED_BLOCKS_V1: usize = 2;
#[cfg(feature = "hardware-test-hooks")]
const SELECTED_TOKENS_V1: usize = 3;
#[cfg(feature = "hardware-test-hooks")]
const MIXING_STREAMS_V1: usize = 4;
#[cfg(feature = "hardware-test-hooks")]
const SINKHORN_ITERATIONS_V1: usize = 3;

#[cfg(feature = "hardware-test-hooks")]
const TOKENS: usize = 16;
#[cfg(feature = "hardware-test-hooks")]
const HIDDEN: usize = 128;
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT: usize = 16;
#[cfg(feature = "hardware-test-hooks")]
const EXPERTS: usize = 4;
#[cfg(feature = "hardware-test-hooks")]
const ALL_EXPERTS: usize = 5;
#[cfg(feature = "hardware-test-hooks")]
const TOP_K: usize = 2;
#[cfg(feature = "hardware-test-hooks")]
const DISPATCH_CAPACITY: usize = TOKENS * TOP_K;
#[cfg(feature = "hardware-test-hooks")]
const CANDIDATES: usize = 8;
#[cfg(feature = "hardware-test-hooks")]
const DRAFT_STEPS: usize = 4;
#[cfg(feature = "hardware-test-hooks")]
const STATE_WIDTH: usize = 8;
#[cfg(feature = "hardware-test-hooks")]
const QUERIES: usize = 8;
#[cfg(feature = "hardware-test-hooks")]
const NGRAM: usize = 3;
#[cfg(feature = "hardware-test-hooks")]
const TABLE_SIZE: usize = 16;
#[cfg(feature = "hardware-test-hooks")]
const MUON_DIM: usize = 4;
#[cfg(feature = "hardware-test-hooks")]
const MUON_ELEMENTS: usize = MUON_DIM * MUON_DIM;
#[cfg(feature = "hardware-test-hooks")]
const GRADIENT_SHARDS: usize = 2;
#[cfg(feature = "hardware-test-hooks")]
const MUON_ITERATIONS: usize = 5;
#[cfg(feature = "hardware-test-hooks")]
const MUON_LEARNING_RATE: f32 = 0.05;

#[cfg(feature = "hardware-test-hooks")]
#[path = "../../../examples/gfx950_advanced_attention/src/reference.rs"]
mod attention_reference;
#[cfg(feature = "hardware-test-hooks")]
#[path = "../../../examples/gfx950_advanced_systems/src/reference.rs"]
mod systems_reference;

#[cfg(feature = "hardware-test-hooks")]
const RUN_ENV: &str = "FE2O3_RUN_GFX950_ADVANCED_HARDWARE";
#[cfg(feature = "hardware-test-hooks")]
const HSACO_ENV: &str = "FE2O3_GFX950_ADVANCED_HSACO";
#[cfg(feature = "hardware-test-hooks")]
const SHA256_ENV: &str = "FE2O3_GFX950_ADVANCED_SHA256";
#[cfg(feature = "hardware-test-hooks")]
const HSA_KERNARG_ALIGNMENT: u64 = 16;
#[cfg(feature = "hardware-test-hooks")]
const METADATA_KERNARG_ALIGNMENT: u64 = 8;
#[cfg(feature = "hardware-test-hooks")]
const GUARD_BYTES: usize = 16;
#[cfg(feature = "hardware-test-hooks")]
const PREFIX_BYTE: u8 = 0x95;
#[cfg(feature = "hardware-test-hooks")]
const SUFFIX_BYTE: u8 = 0x59;
#[cfg(feature = "hardware-test-hooks")]
const F32_POISON_BITS: u32 = 0x7fc0_00a5;

#[cfg(feature = "hardware-test-hooks")]
type BoxError = Box<dyn std::error::Error>;

#[cfg(feature = "hardware-test-hooks")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AbiArg {
    Slice,
    U32,
}

#[cfg(feature = "hardware-test-hooks")]
const SIX_SLICES: &[AbiArg] = &[AbiArg::Slice; 6];
#[cfg(feature = "hardware-test-hooks")]
const FIVE_SLICES: &[AbiArg] = &[AbiArg::Slice; 5];
#[cfg(feature = "hardware-test-hooks")]
const FOUR_SLICES: &[AbiArg] = &[AbiArg::Slice; 4];
#[cfg(feature = "hardware-test-hooks")]
const THREE_SLICES: &[AbiArg] = &[AbiArg::Slice; 3];
#[cfg(feature = "hardware-test-hooks")]
const TWO_SLICES: &[AbiArg] = &[AbiArg::Slice; 2];
#[cfg(feature = "hardware-test-hooks")]
const EXPERT_RANK_ARGS: &[AbiArg] = &[
    AbiArg::Slice,
    AbiArg::Slice,
    AbiArg::Slice,
    AbiArg::Slice,
    AbiArg::U32,
    AbiArg::U32,
    AbiArg::Slice,
];
#[cfg(feature = "hardware-test-hooks")]
const NINE_SLICES: &[AbiArg] = &[AbiArg::Slice; 9];

#[cfg(feature = "hardware-test-hooks")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdvancedCase {
    label: &'static str,
    export: &'static str,
    descriptor: &'static str,
    workgroup_x: u32,
    static_lds_bytes: u64,
    args: &'static [AbiArg],
}

#[cfg(feature = "hardware-test-hooks")]
const KDA_DECODE: AdvancedCase = AdvancedCase {
    label: "gfx950 KDA/GDN decode",
    export: "gfx950_kda_gdn_decode",
    descriptor: "gfx950_kda_gdn_decode.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: SIX_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const KDA_PREFILL: AdvancedCase = AdvancedCase {
    label: "gfx950 KDA/GDN prefill",
    export: "gfx950_kda_gdn_prefill",
    descriptor: "gfx950_kda_gdn_prefill.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: SIX_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const SPARSE_ATTENTION: AdvancedCase = AdvancedCase {
    label: "gfx950 content sparse attention",
    export: "gfx950_content_sparse_attention",
    descriptor: "gfx950_content_sparse_attention.kd",
    workgroup_x: 64,
    static_lds_bytes: 2048,
    args: SIX_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const HYBRID_ATTENTION: AdvancedCase = AdvancedCase {
    label: "gfx950 compressed hybrid attention",
    export: "gfx950_compressed_hybrid_attention",
    descriptor: "gfx950_compressed_hybrid_attention.kd",
    workgroup_x: 64,
    static_lds_bytes: 2048,
    args: FIVE_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const ATTNRES: AdvancedCase = AdvancedCase {
    label: "gfx950 AttnRes aggregation",
    export: "gfx950_attnres_aggregate",
    descriptor: "gfx950_attnres_aggregate.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: THREE_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const FOUR_BRANCH: AdvancedCase = AdvancedCase {
    label: "gfx950 four-branch residual",
    export: "gfx950_four_branch_residual",
    descriptor: "gfx950_four_branch_residual.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: FOUR_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const MHC: AdvancedCase = AdvancedCase {
    label: "gfx950 mHC Sinkhorn mix",
    export: "gfx950_mhc_sinkhorn_mix",
    descriptor: "gfx950_mhc_sinkhorn_mix.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: THREE_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const MOE_ROUTE: AdvancedCase = AdvancedCase {
    label: "gfx950 MoE route",
    export: "gfx950_moe_route_fp4_t16_e4_k2_v1",
    descriptor: "gfx950_moe_route_fp4_t16_e4_k2_v1.kd",
    workgroup_x: 256,
    static_lds_bytes: 0,
    args: SIX_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const MOE_EXPERT: AdvancedCase = AdvancedCase {
    label: "gfx950 MoE expert rank",
    export: "gfx950_moe_expert_rank_fp4_fp8_v1",
    descriptor: "gfx950_moe_expert_rank_fp4_fp8_v1.kd",
    workgroup_x: 256,
    static_lds_bytes: 0,
    args: EXPERT_RANK_ARGS,
};
#[cfg(feature = "hardware-test-hooks")]
const COMBINE: AdvancedCase = AdvancedCase {
    label: "gfx950 expert rank combine",
    export: "gfx950_combine_expert_ranks_v1",
    descriptor: "gfx950_combine_expert_ranks_v1.kd",
    workgroup_x: 256,
    static_lds_bytes: 0,
    args: THREE_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const SPECULATIVE: AdvancedCase = AdvancedCase {
    label: "gfx950 speculative transaction",
    export: "gfx950_speculative_transaction_v1",
    descriptor: "gfx950_speculative_transaction_v1.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: NINE_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const NGRAM_GATHER: AdvancedCase = AdvancedCase {
    label: "gfx950 Qwen N-gram gather",
    export: "gfx950_qwen_ngram_gather_v1",
    descriptor: "gfx950_qwen_ngram_gather_v1.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: SIX_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const STAGE_SHARD: AdvancedCase = AdvancedCase {
    label: "gfx950 gradient shard stage",
    export: "gfx950_stage_gradient_shard_v1",
    descriptor: "gfx950_stage_gradient_shard_v1.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: TWO_SLICES,
};
#[cfg(feature = "hardware-test-hooks")]
const MUON: AdvancedCase = AdvancedCase {
    label: "gfx950 Muon update",
    export: "gfx950_muon_update_4x4_v1",
    descriptor: "gfx950_muon_update_4x4_v1.kd",
    workgroup_x: 64,
    static_lds_bytes: 0,
    args: THREE_SLICES,
};

#[cfg(feature = "hardware-test-hooks")]
fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn kernarg_size(args: &[AbiArg]) -> usize {
    let size = args
        .iter()
        .map(|arg| match arg {
            AbiArg::Slice => 16,
            AbiArg::U32 => 4,
        })
        .sum::<usize>();
    size.next_multiple_of(METADATA_KERNARG_ALIGNMENT as usize)
}

#[cfg(feature = "hardware-test-hooks")]
fn expected_metadata_arguments(args: &[AbiArg]) -> Vec<(u64, u64, ExplicitValueKind)> {
    let mut offset = 0_u64;
    let mut result = Vec::new();
    for arg in args {
        match arg {
            AbiArg::Slice => {
                result.push((offset, 8, ExplicitValueKind::GlobalBuffer));
                result.push((offset + 8, 8, ExplicitValueKind::ByValue));
                offset += 16;
            }
            AbiArg::U32 => {
                result.push((offset, 4, ExplicitValueKind::ByValue));
                offset += 4;
            }
        }
    }
    result
}

#[cfg(feature = "hardware-test-hooks")]
fn parse_sha256(text: &str) -> Result<[u8; 32], BoxError> {
    require(
        text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        format!("{SHA256_ENV} must be exactly 64 lowercase hex digits"),
    )?;
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("{SHA256_ENV} is malformed"))?;
    }
    Ok(bytes)
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_hsaco() -> Result<(Vec<u8>, PayloadDigest), BoxError> {
    require(
        std::env::var(RUN_ENV).as_deref() == Ok("1"),
        format!("set {RUN_ENV}=1 to opt into this raw hardware test"),
    )?;
    let path = std::path::PathBuf::from(
        std::env::var_os(HSACO_ENV).ok_or_else(|| format!("{HSACO_ENV} is not set"))?,
    );
    require(path.is_absolute(), format!("{HSACO_ENV} must be absolute"))?;
    let metadata = std::fs::symlink_metadata(&path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        format!("{HSACO_ENV} must name a regular non-symlink file"),
    )?;
    require(
        std::fs::canonicalize(&path)? == path,
        format!("{HSACO_ENV} must already be canonical"),
    )?;
    require(
        (1..=fe2o3_hsaco::MAX_HSACO_BYTES as u64).contains(&metadata.len()),
        format!("{HSACO_ENV} has an invalid byte length"),
    )?;
    let expected =
        parse_sha256(&std::env::var(SHA256_ENV).map_err(|_| format!("{SHA256_ENV} is not set"))?)?;
    let bytes = std::fs::read(&path)?;
    let final_metadata = std::fs::symlink_metadata(&path)?;
    require(
        bytes.len() as u64 == metadata.len()
            && final_metadata.file_type().is_file()
            && !final_metadata.file_type().is_symlink()
            && final_metadata.len() == metadata.len()
            && std::fs::canonicalize(&path)? == path,
        format!("{HSACO_ENV} changed identity while being read"),
    )?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    require(
        digest.bytes().as_bytes() == &expected,
        format!("{HSACO_ENV} does not match its exact SHA-256 pin"),
    )?;
    Ok((bytes, digest))
}

#[cfg(feature = "hardware-test-hooks")]
fn inspect_profile(case: AdvancedCase, bytes: &[u8]) -> Result<(), BoxError> {
    let bound = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)?;
    let inspected = bound.inspection();
    require(
        inspected.code_object_version() == CodeObjectVersion::V6,
        format!("{} must use code object V6", case.label),
    )?;
    require(
        inspected.target().processor() == "gfx950"
            && inspected.target().xnack() == Some(FeatureState::Disabled),
        format!("{} must target exact gfx950:xnack-", case.label),
    )?;
    require(
        !inspected.has_printf_metadata(),
        format!("{} must not carry printf metadata", case.label),
    )?;
    let [kernel] = inspected.kernels() else {
        return Err(format!("{} must declare exactly one kernel", case.label).into());
    };
    let expected_kernarg = kernarg_size(case.args);
    require(
        kernel.name() == case.export && kernel.symbol() == case.descriptor,
        format!("{} has a substituted kernel symbol", case.label),
    )?;
    require(
        kernel.kernarg_segment_size() == expected_kernarg as u64
            && kernel.kernarg_segment_alignment() == METADATA_KERNARG_ALIGNMENT
            && kernel.implicit_argument_offset().is_none()
            && kernel.implicit_argument_size() == 0,
        format!("{} has a substituted explicit kernarg ABI", case.label),
    )?;
    require(
        kernel.required_workgroup_size() == Some([case.workgroup_x, 1, 1])
            && kernel.max_flat_workgroup_size() == case.workgroup_x
            && kernel.wavefront_size() == 64
            && kernel.group_segment_fixed_size() == case.static_lds_bytes
            && !kernel.uses_dynamic_stack(),
        format!("{} has a substituted WG64/LDS profile", case.label),
    )?;
    let arguments = kernel
        .explicit_arguments()
        .iter()
        .map(|argument| (argument.offset(), argument.size(), argument.value_kind()))
        .collect::<Vec<_>>();
    require(
        arguments == expected_metadata_arguments(case.args),
        format!("{} explicit argument metadata changed", case.label),
    )?;
    let [binding] = bound.bindings() else {
        return Err(format!("{} must bind exactly one descriptor", case.label).into());
    };
    let descriptor = binding.descriptor();
    require(
        binding.kernel_index() == 0
            && descriptor.kernarg_size() == expected_kernarg as u32
            && u64::from(descriptor.group_segment_fixed_size()) == case.static_lds_bytes
            && descriptor.wavefront_size() == 64
            && !descriptor.uses_dynamic_stack(),
        format!("{} descriptor disagrees with metadata", case.label),
    )
}

#[cfg(feature = "hardware-test-hooks")]
#[derive(Clone)]
enum ExpectedOutput {
    F32 {
        values: Vec<f32>,
        absolute_tolerance: f32,
        relative_tolerance: f32,
        exact_mask: Option<Vec<bool>>,
    },
    U32(Vec<u32>),
    I32(Vec<i32>),
}

#[cfg(feature = "hardware-test-hooks")]
impl ExpectedOutput {
    fn element_count(&self) -> usize {
        match self {
            Self::F32 { values, .. } => values.len(),
            Self::U32(values) => values.len(),
            Self::I32(values) => values.len(),
        }
    }
}

#[cfg(feature = "hardware-test-hooks")]
struct PlannedBuffer {
    name: &'static str,
    initial: Vec<u8>,
    body_offset: usize,
    elements: usize,
    immutable: bool,
    expected: Option<ExpectedOutput>,
}

#[cfg(feature = "hardware-test-hooks")]
enum PlannedArg {
    Slice { buffer: usize, elements: usize },
    U32(u32),
}

#[cfg(feature = "hardware-test-hooks")]
struct LaunchPlan {
    label: String,
    buffers: Vec<PlannedBuffer>,
    args: Vec<PlannedArg>,
}

#[cfg(feature = "hardware-test-hooks")]
fn value_bytes<T: Copy>(values: &[T]) -> Vec<u8> {
    let byte_len = std::mem::size_of_val(values);
    // SAFETY: every input type used here is a plain integer or f32 and the
    // returned Vec owns a byte-for-byte copy of the complete initialized span.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), byte_len) }.to_vec()
}

#[cfg(feature = "hardware-test-hooks")]
fn input<T: Copy>(name: &'static str, values: &[T]) -> PlannedBuffer {
    PlannedBuffer {
        name,
        initial: value_bytes(values),
        body_offset: 0,
        elements: values.len(),
        immutable: true,
        expected: None,
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn output(name: &'static str, expected: ExpectedOutput) -> PlannedBuffer {
    let mut initial = vec![PREFIX_BYTE; GUARD_BYTES];
    match &expected {
        ExpectedOutput::F32 { values, .. } => {
            for _ in values {
                initial.extend_from_slice(&F32_POISON_BITS.to_le_bytes());
            }
        }
        ExpectedOutput::U32(values) => {
            for value in values {
                initial.extend_from_slice(&(!*value).to_le_bytes());
            }
        }
        ExpectedOutput::I32(values) => {
            for value in values {
                initial.extend_from_slice(&(!*value).to_le_bytes());
            }
        }
    }
    initial.extend(std::iter::repeat_n(SUFFIX_BYTE, GUARD_BYTES));
    PlannedBuffer {
        name,
        initial,
        body_offset: GUARD_BYTES,
        elements: expected.element_count(),
        immutable: false,
        expected: Some(expected),
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_output(name: &'static str, values: Vec<f32>, tolerance: f32) -> PlannedBuffer {
    output(
        name,
        ExpectedOutput::F32 {
            values,
            absolute_tolerance: tolerance,
            relative_tolerance: 0.0,
            exact_mask: None,
        },
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn deterministic_floats(count: usize, salt: usize, scale: f32) -> Vec<f32> {
    (0..count)
        .map(|index| {
            let centered = ((index * 17 + salt * 11) % 19) as i32 - 9;
            scale * centered as f32 / 9.0
        })
        .collect()
}

#[cfg(feature = "hardware-test-hooks")]
fn deterministic_fp8(count: usize, salt: usize) -> Vec<u8> {
    const CODES: [u8; 5] = [0xb8, 0xb0, 0x00, 0x30, 0x38];
    (0..count)
        .map(|index| CODES[(index * 3 + salt) % CODES.len()])
        .collect()
}

#[cfg(feature = "hardware-test-hooks")]
fn repeated_attention_query() -> (Vec<u8>, Vec<u8>) {
    let logical = deterministic_fp8(HEAD_DIMENSION_V1, 1);
    let mut physical = Vec::with_capacity(ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1);
    for _ in 0..ATTENTION_TOKENS_V1 {
        physical.extend_from_slice(&logical);
    }
    (logical, physical)
}

#[cfg(feature = "hardware-test-hooks")]
fn attention_plans(case: AdvancedCase) -> Result<Vec<LaunchPlan>, BoxError> {
    let plan = match case.export {
        "gfx950_kda_gdn_decode" => {
            let history = deterministic_floats(KDA_TAPS_V1 * CHANNELS_V1, 1, 0.6);
            let gates = deterministic_floats(CHANNELS_V1, 2, 0.8);
            let state = deterministic_floats(CHANNELS_V1, 3, 0.5);
            let weights = vec![0.5, -0.25, 0.125];
            let expected = attention_reference::kda_gdn_decode_reference_v1(
                &history, &gates, &state, &weights,
            )
            .map_err(|error| format!("KDA decode reference failed: {error:?}"))?;
            LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("history", &history),
                    input("gate_input", &gates),
                    input("state", &state),
                    input("convolution_weights", &weights),
                    f32_output("state_output", expected.state, 3.0e-3),
                    f32_output("normalized_output", expected.normalized, 3.0e-3),
                ],
                args: (0..6)
                    .map(|buffer| PlannedArg::Slice {
                        elements: [
                            history.len(),
                            gates.len(),
                            state.len(),
                            weights.len(),
                            CHANNELS_V1,
                            CHANNELS_V1,
                        ][buffer],
                        buffer,
                    })
                    .collect(),
            }
        }
        "gfx950_kda_gdn_prefill" => {
            let values = deterministic_floats(PREFILL_TOKENS_V1 * CHANNELS_V1, 4, 0.5);
            let gates = deterministic_floats(PREFILL_TOKENS_V1 * CHANNELS_V1, 5, 0.7);
            let state = deterministic_floats(CHANNELS_V1, 3, 0.5);
            let weights = vec![0.5, -0.25, 0.125];
            let expected = attention_reference::kda_gdn_prefill_reference_v1(
                &values, &gates, &state, &weights,
            )
            .map_err(|error| format!("KDA prefill reference failed: {error:?}"))?;
            let lengths = [
                values.len(),
                gates.len(),
                state.len(),
                weights.len(),
                CHANNELS_V1,
                PREFILL_TOKENS_V1 * CHANNELS_V1,
            ];
            LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("input", &values),
                    input("gate_input", &gates),
                    input("initial_state", &state),
                    input("convolution_weights", &weights),
                    f32_output("final_state", expected.final_state, 3.0e-3),
                    f32_output("normalized_output", expected.normalized, 3.0e-3),
                ],
                args: (0..6)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: lengths[buffer],
                    })
                    .collect(),
            }
        }
        "gfx950_content_sparse_attention" | "gfx950_compressed_hybrid_attention" => {
            let (logical_q, physical_q) = repeated_attention_query();
            let key = deterministic_fp8(ATTENTION_TOKENS_V1 * HEAD_DIMENSION_V1, 2);
            let value = deterministic_fp8(ATTENTION_TOKENS_V1 * CHANNELS_V1, 3);
            if case == SPARSE_ATTENTION {
                let scores = vec![
                    0.10, 0.82, -0.20, 0.35, 0.61, 0.55, 0.14, 0.92, 0.73, -0.10, 0.48, 0.31, 0.41,
                    0.67, 0.22, 0.05,
                ];
                let expected = attention_reference::content_sparse_attention_reference_v1(
                    &logical_q, &key, &value, &scores,
                )
                .map_err(|error| format!("sparse reference failed: {error:?}"))?;
                LaunchPlan {
                    label: case.label.into(),
                    buffers: vec![
                        input("q", &physical_q),
                        input("k", &key),
                        input("v", &value),
                        input("content_scores", &scores),
                        f32_output("output", expected.output, 5.0e-3),
                        output(
                            "selected_output",
                            ExpectedOutput::U32(expected.selected.to_vec()),
                        ),
                    ],
                    args: (0..6)
                        .map(|buffer| PlannedArg::Slice {
                            buffer,
                            elements: [
                                physical_q.len(),
                                key.len(),
                                value.len(),
                                scores.len(),
                                CHANNELS_V1,
                                SELECTED_TOKENS_V1,
                            ][buffer],
                        })
                        .collect(),
                }
            } else {
                let bias = deterministic_floats(ATTENTION_TOKENS_V1, 7, 0.4);
                let expected = attention_reference::compressed_hybrid_attention_reference_v1(
                    &logical_q, &key, &value, &bias,
                )
                .map_err(|error| format!("hybrid reference failed: {error:?}"))?;
                let lengths = [
                    physical_q.len(),
                    key.len(),
                    value.len(),
                    bias.len(),
                    CHANNELS_V1,
                ];
                LaunchPlan {
                    label: case.label.into(),
                    buffers: vec![
                        input("q", &physical_q),
                        input("k", &key),
                        input("v", &value),
                        input("token_bias", &bias),
                        f32_output("output", expected, 5.0e-3),
                    ],
                    args: (0..5)
                        .map(|buffer| PlannedArg::Slice {
                            buffer,
                            elements: lengths[buffer],
                        })
                        .collect(),
                }
            }
        }
        "gfx950_attnres_aggregate" => {
            let values = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 8, 0.7);
            let logits = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 9, 0.9);
            let expected = attention_reference::attnres_aggregate_reference_v1(&values, &logits)
                .map_err(|error| format!("AttnRes reference failed: {error:?}"))?;
            LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("depth_values", &values),
                    input("depth_logits", &logits),
                    f32_output("output", expected, 3.0e-3),
                ],
                args: (0..3)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: [values.len(), logits.len(), CHANNELS_V1][buffer],
                    })
                    .collect(),
            }
        }
        "gfx950_four_branch_residual" => {
            let residual = deterministic_floats(CHANNELS_V1, 10, 0.4);
            let branches = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 11, 0.6);
            let gates = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 12, 0.8);
            let expected = attention_reference::four_branch_residual_reference_v1(
                &residual, &branches, &gates,
            )
            .map_err(|error| format!("residual reference failed: {error:?}"))?;
            let lengths = [residual.len(), branches.len(), gates.len(), CHANNELS_V1];
            LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("residual", &residual),
                    input("branches", &branches),
                    input("gate_logits", &gates),
                    f32_output("output", expected, 3.0e-3),
                ],
                args: (0..4)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: lengths[buffer],
                    })
                    .collect(),
            }
        }
        "gfx950_mhc_sinkhorn_mix" => {
            let streams = deterministic_floats(MIXING_STREAMS_V1 * CHANNELS_V1, 13, 0.7);
            let logits = deterministic_floats(MIXING_STREAMS_V1 * MIXING_STREAMS_V1, 14, 0.5);
            let expected = attention_reference::mhc_sinkhorn_mix_reference_v1(&streams, &logits)
                .map_err(|error| format!("mHC reference failed: {error:?}"))?;
            LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("streams", &streams),
                    input("mixing_logits", &logits),
                    f32_output("output", expected, 3.0e-3),
                ],
                args: (0..3)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: [streams.len(), logits.len(), MIXING_STREAMS_V1 * CHANNELS_V1]
                            [buffer],
                    })
                    .collect(),
            }
        }
        _ => return Err(format!("unknown attention case {}", case.export).into()),
    };
    Ok(vec![plan])
}

#[cfg(feature = "hardware-test-hooks")]
fn make_activations() -> Vec<u8> {
    const CODES: [u8; 5] = [0xa, 0x9, 0x0, 0x1, 0x2];
    (0..TOKENS * HIDDEN)
        .map(|index| {
            let token = index / HIDDEN;
            let depth = index % HIDDEN;
            CODES[(token * 3 + depth * 2 + 1) % CODES.len()]
        })
        .collect()
}

#[cfg(feature = "hardware-test-hooks")]
fn make_expert_weights() -> Vec<u8> {
    const CODES: [u8; 5] = [0xb0, 0xa8, 0x00, 0x28, 0x30];
    (0..ALL_EXPERTS * HIDDEN * OUTPUT)
        .map(|index| {
            let expert = index / (HIDDEN * OUTPUT);
            let within = index % (HIDDEN * OUTPUT);
            let depth = within / OUTPUT;
            let column = within % OUTPUT;
            CODES[(expert * 2 + depth * 3 + column * 4) % CODES.len()]
        })
        .collect()
}

#[cfg(feature = "hardware-test-hooks")]
fn make_router_weights() -> Vec<f32> {
    (0..EXPERTS * HIDDEN)
        .map(|index| {
            let expert = index / HIDDEN;
            let depth = index % HIDDEN;
            (((expert + 1) * (depth % 7) + depth / 13) % 9) as f32 / 64.0 - 4.0 / 64.0
        })
        .collect()
}

#[cfg(feature = "hardware-test-hooks")]
fn moe_fixture() -> (
    Vec<u8>,
    Vec<u8>,
    Vec<f32>,
    systems_reference::MoeRoutingReference,
) {
    let activations = make_activations();
    let expert_weights = make_expert_weights();
    let router_weights = make_router_weights();
    let routing = systems_reference::moe_routing_reference(&activations, &router_weights);
    (activations, expert_weights, router_weights, routing)
}

#[cfg(feature = "hardware-test-hooks")]
fn hash_gram(gram: &[i32]) -> u64 {
    gram.iter()
        .fold(1_469_598_103_934_665_603_u64, |hash, value| {
            (hash ^ (*value as u32 as u64)).wrapping_mul(1_099_511_628_211)
        })
}

#[cfg(feature = "hardware-test-hooks")]
fn systems_plans(case: AdvancedCase) -> Result<Vec<LaunchPlan>, BoxError> {
    match case.export {
        "gfx950_moe_route_fp4_t16_e4_k2_v1" => {
            let (activations, _, router_weights, routing) = moe_fixture();
            let lengths = [
                activations.len(),
                router_weights.len(),
                TOKENS * TOP_K,
                TOKENS * TOP_K,
                EXPERTS,
                EXPERTS * DISPATCH_CAPACITY,
            ];
            Ok(vec![LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("activations", &activations),
                    input("router_weights", &router_weights),
                    output("top_experts", ExpectedOutput::U32(routing.top_experts)),
                    f32_output("top_weights", routing.top_weights, 2.0e-6),
                    output("expert_counts", ExpectedOutput::U32(routing.expert_counts)),
                    output("dispatch", ExpectedOutput::I32(routing.dispatch)),
                ],
                args: (0..6)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: lengths[buffer],
                    })
                    .collect(),
            }])
        }
        "gfx950_moe_expert_rank_fp4_fp8_v1" => {
            let (activations, weights, _, routing) = moe_fixture();
            let mut plans = Vec::new();
            for (rank, first_expert, include_shared) in [(0, 0, true), (1, 2, false)] {
                let expected = systems_reference::moe_rank_reference(
                    &activations,
                    &weights,
                    &routing,
                    first_expert,
                    include_shared,
                );
                let buffers = vec![
                    input("activations", &activations),
                    input("expert_weights", &weights),
                    input("top_experts", &routing.top_experts),
                    input("top_weights", &routing.top_weights),
                    f32_output("output", expected, 3.0e-3),
                ];
                plans.push(LaunchPlan {
                    label: format!("{} rank {rank}", case.label),
                    args: vec![
                        PlannedArg::Slice {
                            buffer: 0,
                            elements: activations.len(),
                        },
                        PlannedArg::Slice {
                            buffer: 1,
                            elements: weights.len(),
                        },
                        PlannedArg::Slice {
                            buffer: 2,
                            elements: routing.top_experts.len(),
                        },
                        PlannedArg::Slice {
                            buffer: 3,
                            elements: routing.top_weights.len(),
                        },
                        PlannedArg::U32(first_expert as u32),
                        PlannedArg::U32(u32::from(include_shared)),
                        PlannedArg::Slice {
                            buffer: 4,
                            elements: TOKENS * OUTPUT,
                        },
                    ],
                    buffers,
                });
            }
            Ok(plans)
        }
        "gfx950_combine_expert_ranks_v1" => {
            let (activations, weights, _, routing) = moe_fixture();
            let rank0 =
                systems_reference::moe_rank_reference(&activations, &weights, &routing, 0, true);
            let rank1 =
                systems_reference::moe_rank_reference(&activations, &weights, &routing, 2, false);
            let expected = rank0.iter().zip(&rank1).map(|(a, b)| a + b).collect();
            Ok(vec![LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("rank0", &rank0),
                    input("rank1", &rank1),
                    f32_output("output", expected, 3.0e-3),
                ],
                args: (0..3)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: TOKENS * OUTPUT,
                    })
                    .collect(),
            }])
        }
        "gfx950_speculative_transaction_v1" => {
            let target = vec![11, 12, 13, 14];
            let mut draft = vec![0; CANDIDATES * DRAFT_STEPS];
            let mut scores = vec![0.0; CANDIDATES * DRAFT_STEPS];
            let thresholds = vec![0.25, 0.35, 0.45, 0.55];
            let mut deltas = vec![0.0; CANDIDATES * DRAFT_STEPS * STATE_WIDTH];
            for candidate in 0..CANDIDATES {
                for step in 0..DRAFT_STEPS {
                    draft[candidate * DRAFT_STEPS + step] = if step == candidate % 5 {
                        90 + candidate as i32
                    } else {
                        target[step]
                    };
                    scores[candidate * DRAFT_STEPS + step] =
                        0.2 + 0.11 * ((candidate + step) % 6) as f32;
                    for element in 0..STATE_WIDTH {
                        deltas[(candidate * DRAFT_STEPS + step) * STATE_WIDTH + element] =
                            0.001 * (1 + candidate + step * 2 + element) as f32;
                    }
                }
            }
            for candidate in [4, 7] {
                for step in 0..DRAFT_STEPS {
                    draft[candidate * DRAFT_STEPS + step] = target[step];
                    scores[candidate * DRAFT_STEPS + step] = 0.9;
                }
            }
            let base = (0..STATE_WIDTH)
                .map(|element| 0.125 * (element as f32 - 3.0))
                .collect::<Vec<_>>();
            let expected = systems_reference::speculative_reference(
                &draft,
                &target,
                &scores,
                &thresholds,
                &base,
                &deltas,
            );
            let exact_mask = expected
                .committed
                .iter()
                .flat_map(|committed| std::iter::repeat_n(*committed == 0, STATE_WIDTH))
                .collect();
            let lengths = [
                draft.len(),
                target.len(),
                scores.len(),
                thresholds.len(),
                base.len(),
                deltas.len(),
                CANDIDATES,
                CANDIDATES,
                CANDIDATES * STATE_WIDTH,
            ];
            Ok(vec![LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("draft_tokens", &draft),
                    input("target_tokens", &target),
                    input("draft_scores", &scores),
                    input("thresholds", &thresholds),
                    input("base_state", &base),
                    input("proposed_deltas", &deltas),
                    output("accepted_steps", ExpectedOutput::U32(expected.accepted)),
                    output("committed", ExpectedOutput::U32(expected.committed)),
                    output(
                        "output_state",
                        ExpectedOutput::F32 {
                            values: expected.state,
                            absolute_tolerance: 1.0e-7,
                            relative_tolerance: 0.0,
                            exact_mask: Some(exact_mask),
                        },
                    ),
                ],
                args: (0..9)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: lengths[buffer],
                    })
                    .collect(),
            }])
        }
        "gfx950_qwen_ngram_gather_v1" => {
            let mut queries = vec![0; QUERIES * NGRAM];
            for query in 0..QUERIES {
                queries[query * NGRAM] = 100 + query as i32;
                queries[query * NGRAM + 1] = 200 + (query % 3) as i32;
                queries[query * NGRAM + 2] = 300 + (query * 2) as i32;
            }
            let mut hashes = vec![0; TABLE_SIZE];
            let mut grams = vec![-1; TABLE_SIZE * NGRAM];
            let mut values = vec![-1; TABLE_SIZE];
            let mut priorities = vec![-1; TABLE_SIZE];
            for query in 0..6 {
                let hash = hash_gram(&queries[query * NGRAM..(query + 1) * NGRAM]);
                let slot = (hash as usize + 3) & (TABLE_SIZE - 1);
                hashes[slot] = hash;
                grams[slot * NGRAM..(slot + 1) * NGRAM]
                    .copy_from_slice(&queries[query * NGRAM..(query + 1) * NGRAM]);
                values[slot] = 1000 + query as i32;
                priorities[slot] = (query % 2) as i32;
            }
            let duplicate_hash = hash_gram(&queries[..NGRAM]);
            hashes[1] = duplicate_hash;
            grams[NGRAM..2 * NGRAM].copy_from_slice(&queries[..NGRAM]);
            values[1] = 4242;
            priorities[1] = 0;
            hashes[2] = duplicate_hash;
            grams[2 * NGRAM..3 * NGRAM].copy_from_slice(&[999, 998, 997]);
            values[2] = 7777;
            priorities[2] = 99;
            let expected =
                systems_reference::ngram_reference(&queries, &hashes, &grams, &values, &priorities);
            let lengths = [
                queries.len(),
                hashes.len(),
                grams.len(),
                values.len(),
                priorities.len(),
                QUERIES,
            ];
            Ok(vec![LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("queries", &queries),
                    input("table_hashes", &hashes),
                    input("table_grams", &grams),
                    input("table_values", &values),
                    input("priorities", &priorities),
                    output("output", ExpectedOutput::I32(expected)),
                ],
                args: (0..6)
                    .map(|buffer| PlannedArg::Slice {
                        buffer,
                        elements: lengths[buffer],
                    })
                    .collect(),
            }])
        }
        "gfx950_stage_gradient_shard_v1" => {
            let shards = (0..GRADIENT_SHARDS * MUON_ELEMENTS)
                .map(|index| {
                    let shard = index / MUON_ELEMENTS;
                    let element = index % MUON_ELEMENTS;
                    0.025 * ((shard + 1) as f32 * (((element * 5 + shard * 3) % 11) as f32 - 5.0))
                })
                .collect::<Vec<_>>();
            Ok((0..GRADIENT_SHARDS)
                .map(|shard| {
                    let values =
                        shards[shard * MUON_ELEMENTS..(shard + 1) * MUON_ELEMENTS].to_vec();
                    LaunchPlan {
                        label: format!("{} {shard}", case.label),
                        buffers: vec![
                            input("input", &values),
                            output(
                                "output",
                                ExpectedOutput::F32 {
                                    values,
                                    absolute_tolerance: 0.0,
                                    relative_tolerance: 0.0,
                                    exact_mask: Some(vec![true; MUON_ELEMENTS]),
                                },
                            ),
                        ],
                        args: vec![
                            PlannedArg::Slice {
                                buffer: 0,
                                elements: MUON_ELEMENTS,
                            },
                            PlannedArg::Slice {
                                buffer: 1,
                                elements: MUON_ELEMENTS,
                            },
                        ],
                    }
                })
                .collect())
        }
        "gfx950_muon_update_4x4_v1" => {
            let shards = (0..GRADIENT_SHARDS * MUON_ELEMENTS)
                .map(|index| {
                    let shard = index / MUON_ELEMENTS;
                    let element = index % MUON_ELEMENTS;
                    0.025 * ((shard + 1) as f32 * (((element * 5 + shard * 3) % 11) as f32 - 5.0))
                })
                .collect::<Vec<_>>();
            let expected = systems_reference::muon_reference(&shards);
            Ok(vec![LaunchPlan {
                label: case.label.into(),
                buffers: vec![
                    input("shards", &shards),
                    f32_output("output", expected.update, 2.0e-6),
                    f32_output("output_norm", vec![expected.norm], 2.0e-6),
                ],
                args: vec![
                    PlannedArg::Slice {
                        buffer: 0,
                        elements: shards.len(),
                    },
                    PlannedArg::Slice {
                        buffer: 1,
                        elements: MUON_ELEMENTS,
                    },
                    PlannedArg::Slice {
                        buffer: 2,
                        elements: 1,
                    },
                ],
            }])
        }
        _ => Err(format!("unknown systems case {}", case.export).into()),
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn plans_for(case: AdvancedCase) -> Result<Vec<LaunchPlan>, BoxError> {
    if [
        KDA_DECODE,
        KDA_PREFILL,
        SPARSE_ATTENTION,
        HYBRID_ATTENTION,
        ATTNRES,
        FOUR_BRANCH,
        MHC,
    ]
    .contains(&case)
    {
        attention_plans(case)
    } else {
        systems_plans(case)
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "hardware-test-hooks")]
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "hardware-test-hooks")]
fn explicit_kernarg(
    case: AdvancedCase,
    plan: &LaunchPlan,
    buffers: &[ReviewedHsaHardwareTestBufferV1],
) -> Result<Vec<u8>, BoxError> {
    require(
        plan.args.len() == case.args.len(),
        format!("{} launch argument count changed", plan.label),
    )?;
    let mut bytes = vec![0; kernarg_size(case.args)];
    let mut offset = 0;
    for (expected, actual) in case.args.iter().zip(&plan.args) {
        match (expected, actual) {
            (AbiArg::Slice, PlannedArg::Slice { buffer, elements }) => {
                require(
                    *elements == plan.buffers[*buffer].elements,
                    format!("{} slice element count changed", plan.buffers[*buffer].name),
                )?;
                put_u64(
                    &mut bytes,
                    offset,
                    buffers[*buffer].device_address(plan.buffers[*buffer].body_offset)?,
                );
                put_u64(&mut bytes, offset + 8, *elements as u64);
                offset += 16;
            }
            (AbiArg::U32, PlannedArg::U32(value)) => {
                put_u32(&mut bytes, offset, *value);
                offset += 4;
            }
            _ => return Err(format!("{} kernarg kind changed", plan.label).into()),
        }
    }
    Ok(bytes)
}

#[cfg(feature = "hardware-test-hooks")]
struct RuntimeKernarg {
    pointer: std::ptr::NonNull<u8>,
    layout: std::alloc::Layout,
}

#[cfg(feature = "hardware-test-hooks")]
impl RuntimeKernarg {
    fn new(size: usize) -> Result<Self, BoxError> {
        let layout = std::alloc::Layout::from_size_align(size, HSA_KERNARG_ALIGNMENT as usize)?;
        // SAFETY: layout is valid and this owner deallocates the result once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("failed to allocate aligned advanced-kernel kernarg")?;
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
unsafe fn dispatch(
    case: AdvancedCase,
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    explicit: &[u8],
) -> Result<(), BoxError> {
    let size = kernarg_size(case.args);
    require(
        resolution.export_symbol() == case.export
            && resolution.kernarg_segment_size() == size as u64
            && resolution.kernarg_segment_alignment() == HSA_KERNARG_ALIGNMENT
            && resolution.group_segment_size() == case.static_lds_bytes,
        format!(
            "runtime resolution differs from the exact {} ABI",
            case.label
        ),
    )?;
    let geometry = HsaLaunchGeometryV1::new([1, 1, 1], [case.workgroup_x, 1, 1], 0);
    let mut storage = RuntimeKernarg::new(size)?;
    let kernarg = storage.bytes_mut();
    kernarg.copy_from_slice(explicit);
    let original = explicit.to_vec();
    // SAFETY: inspection admitted this exact explicit-only single-kernel ABI;
    // all buffers remain live through synchronous completion.
    unsafe {
        let initialization = adapter
            .initialize_implicit_kernarg(executable, kernel, geometry, size, size, 0, kernarg)?;
        require(
            initialization.initialized()
                && initialization.explicit_byte_len() == size as u64
                && initialization.implicit_byte_offset() == size as u64
                && initialization.implicit_byte_len() == 0
                && kernarg == original,
            format!("{} implicit initialization changed the ABI", case.label),
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(
            completion.completed(),
            format!("{} dispatch did not complete", case.label),
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn decode_f32(bytes: &[u8]) -> Result<Vec<f32>, BoxError> {
    require(
        bytes.len() % 4 == 0,
        "f32 output byte length is not aligned",
    )?;
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn decode_u32(bytes: &[u8]) -> Result<Vec<u32>, BoxError> {
    require(
        bytes.len() % 4 == 0,
        "u32 output byte length is not aligned",
    )?;
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn decode_i32(bytes: &[u8]) -> Result<Vec<i32>, BoxError> {
    require(
        bytes.len() % 4 == 0,
        "i32 output byte length is not aligned",
    )?;
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes(chunk.try_into().expect("four-byte chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn verify_buffer(plan: &PlannedBuffer, actual: &[u8]) -> Result<(), BoxError> {
    if plan.immutable {
        return require(
            actual == plan.initial,
            format!("{} input was modified", plan.name),
        );
    }
    let expected = plan
        .expected
        .as_ref()
        .ok_or_else(|| format!("{} output has no oracle", plan.name))?;
    require(
        actual.len() == plan.initial.len()
            && actual[..GUARD_BYTES]
                .iter()
                .all(|byte| *byte == PREFIX_BYTE)
            && actual[actual.len() - GUARD_BYTES..]
                .iter()
                .all(|byte| *byte == SUFFIX_BYTE),
        format!("{} output canary changed", plan.name),
    )?;
    let body = &actual[GUARD_BYTES..actual.len() - GUARD_BYTES];
    match expected {
        ExpectedOutput::F32 {
            values,
            absolute_tolerance,
            relative_tolerance,
            exact_mask,
        } => {
            let actual = decode_f32(body)?;
            let mut maximum_error = 0.0_f32;
            for (index, (actual, expected)) in actual.iter().zip(values).enumerate() {
                let error = (actual - expected).abs();
                let tolerance = absolute_tolerance + relative_tolerance * expected.abs();
                require(
                    actual.is_finite() && expected.is_finite() && error <= tolerance,
                    format!(
                        "{}[{index}] mismatch: actual={actual}, expected={expected}, tolerance={tolerance}",
                        plan.name
                    ),
                )?;
                if exact_mask.as_ref().is_some_and(|mask| mask[index]) {
                    require(
                        actual.to_bits() == expected.to_bits(),
                        format!("{}[{index}] is not bitwise exact", plan.name),
                    )?;
                }
                maximum_error = maximum_error.max(error);
            }
            println!(
                "PASS {} outputs={} max_absolute_error={maximum_error:.9e}",
                plan.name,
                values.len()
            );
        }
        ExpectedOutput::U32(values) => {
            require(
                decode_u32(body)? == *values,
                format!("{} exact u32 output mismatch", plan.name),
            )?;
            println!("PASS {} exact_u32_outputs={}", plan.name, values.len());
        }
        ExpectedOutput::I32(values) => {
            require(
                decode_i32(body)? == *values,
                format!("{} exact i32 output mismatch", plan.name),
            )?;
            println!("PASS {} exact_i32_outputs={}", plan.name, values.len());
        }
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn execute_plan(
    case: AdvancedCase,
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    bytes: &[u8],
    digest: PayloadDigest,
    plan: LaunchPlan,
) -> Result<(), BoxError> {
    let buffers = plan
        .buffers
        .iter()
        .map(|buffer| adapter.allocate_hardware_test_buffer(&buffer.initial))
        .collect::<Result<Vec<_>, _>>()?;
    let explicit = explicit_kernarg(case, &plan, &buffers)?;
    // SAFETY: immutable digest-pinned bytes and allocations are retained until
    // synchronous dispatch and the sole consuming unload complete.
    let (executable, load) = unsafe { adapter.load_executable(bytes, digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(
            load.finalized_digest() == digest && load.byte_len() == bytes.len() as u64,
            format!("{} load observation changed", plan.label),
        )?;
        // SAFETY: profile inspection admitted exactly this export/descriptor.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [case.export]) }?;
        let kernel = kernels
            .get(0)
            .ok_or_else(|| format!("runtime omitted {}", plan.label))?;
        require(
            kernels.len() == 1
                && resolutions.len() == 1
                && resolutions[0].executable_object() == executable_identity,
            format!("runtime resolved a substituted {} kernel", plan.label),
        )?;
        // SAFETY: dispatch owns the reviewed raw launch boundary.
        unsafe {
            dispatch(
                case,
                adapter,
                &executable,
                kernel,
                &resolutions[0],
                &explicit,
            )?;
        }
        for (planned, actual) in plan.buffers.iter().zip(&buffers) {
            verify_buffer(planned, &actual.read_after_synchronous_dispatch())?;
        }
        Ok(())
    })();
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(
        unload.released() && unload.executable_object() == executable_identity,
        format!("{} executable was not released", plan.label),
    )?;
    execution
}

#[cfg(feature = "hardware-test-hooks")]
fn run_case(case: AdvancedCase) -> Result<(), BoxError> {
    let (bytes, digest) = read_pinned_hsaco()?;
    inspect_profile(case, &bytes)?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new_gfx950(context)?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx950"
            && adapter.environment().physical_device().target().xnack()
                == Some(FeatureState::Disabled),
        format!("{} requires a gfx950:xnack- physical device", case.label),
    )?;
    for plan in plans_for(case)? {
        execute_plan(case, &mut adapter, &bytes, digest, plan)?;
    }
    println!("PASS {} production HSA verification", case.label);
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
macro_rules! hardware_case {
    ($name:ident, $case:ident) => {
        #[test]
        #[ignore = "non-authoritative: requires a Rust-produced digest-pinned gfx950:xnack- COV6 HSACO and MI350"]
        fn $name() -> Result<(), BoxError> {
            run_case($case)
        }
    };
}

#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_kda_gdn_decode_rust_cov6_matches_cpu_reference,
    KDA_DECODE
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_kda_gdn_prefill_rust_cov6_matches_cpu_reference,
    KDA_PREFILL
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_content_sparse_attention_rust_cov6_matches_cpu_reference,
    SPARSE_ATTENTION
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_compressed_hybrid_attention_rust_cov6_matches_cpu_reference,
    HYBRID_ATTENTION
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_attnres_aggregate_rust_cov6_matches_cpu_reference,
    ATTNRES
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_four_branch_residual_rust_cov6_matches_cpu_reference,
    FOUR_BRANCH
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(gfx950_mhc_sinkhorn_mix_rust_cov6_matches_cpu_reference, MHC);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(gfx950_moe_route_rust_cov6_matches_cpu_reference, MOE_ROUTE);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_moe_expert_rank_rust_cov6_matches_cpu_reference,
    MOE_EXPERT
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_combine_expert_ranks_rust_cov6_matches_cpu_reference,
    COMBINE
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_speculative_transaction_rust_cov6_matches_cpu_reference,
    SPECULATIVE
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_qwen_ngram_gather_rust_cov6_matches_cpu_reference,
    NGRAM_GATHER
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(
    gfx950_stage_gradient_shard_rust_cov6_matches_cpu_reference,
    STAGE_SHARD
);
#[cfg(feature = "hardware-test-hooks")]
hardware_case!(gfx950_muon_update_rust_cov6_matches_cpu_reference, MUON);

#[cfg(feature = "hardware-test-hooks")]
#[test]
fn output_poison_cannot_match_an_unwritten_expected_element() {
    let float = output(
        "float",
        ExpectedOutput::F32 {
            values: vec![0.0, 1.0],
            absolute_tolerance: 0.0,
            relative_tolerance: 0.0,
            exact_mask: None,
        },
    );
    let float_body = &float.initial[GUARD_BYTES..float.initial.len() - GUARD_BYTES];
    assert!(
        decode_f32(float_body)
            .expect("float poison")
            .iter()
            .all(|value| value.is_nan())
    );
    assert!(verify_buffer(&float, &float.initial).is_err());

    let unsigned = output("unsigned", ExpectedOutput::U32(vec![0, u32::MAX]));
    let unsigned_body = &unsigned.initial[GUARD_BYTES..unsigned.initial.len() - GUARD_BYTES];
    assert_eq!(
        decode_u32(unsigned_body).expect("u32 poison"),
        [u32::MAX, 0]
    );
    assert!(verify_buffer(&unsigned, &unsigned.initial).is_err());

    let signed = output("signed", ExpectedOutput::I32(vec![-1, 0]));
    let signed_body = &signed.initial[GUARD_BYTES..signed.initial.len() - GUARD_BYTES];
    assert_eq!(decode_i32(signed_body).expect("i32 poison"), [0, -1]);
    assert!(verify_buffer(&signed, &signed.initial).is_err());
}

#[cfg(not(feature = "hardware-test-hooks"))]
#[test]
#[ignore = "requires feature hardware-test-hooks and caller-supplied gfx950 HSACO"]
fn gfx950_advanced_hardware_tests_require_explicit_hooks() {}
