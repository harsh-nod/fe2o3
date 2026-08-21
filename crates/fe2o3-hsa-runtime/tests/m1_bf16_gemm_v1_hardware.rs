//! Unqualified hardware observation for the exact M1 BF16 GEMM V1 slice.
//!
//! Caller-supplied bytes and a matching digest do not authenticate a Worker V2
//! transcript or mint compiler, load, launch, completion, or correctness
//! authority. This ignored harness deliberately remains outside the protected
//! runtime path. Qualification must bind the same bytes to
//! `AdmittedBf16GemmKernelArtifactV1` before treating a run as M1 evidence.

#![cfg(target_os = "linux")]

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_bf16_gemm_v1::{
    BF16_GEMM_A_BYTES_V1, BF16_GEMM_B_BYTES_V1, BF16_GEMM_C_BYTES_V1, Bf16GemmBufferContractV1,
};
use fe2o3_bf16_gemm_v1::{
    BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1, BF16_GEMM_KERNEL_KERNARG_ALIGNMENT_V1,
    BF16_GEMM_KERNEL_KERNARG_BYTES_V1, BF16_GEMM_KERNEL_SYMBOL_V1, BF16_GEMM_KERNEL_TARGET_V1,
    BF16_GEMM_KERNEL_WORKGROUP_X_V1, Bf16GemmKernelDispatchShapeV1,
};
use fe2o3_host::HsaLaunchGeometryV1;
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitValueKind, ExplicitValueType,
    inspect_and_bind_kernel_descriptors,
};
#[cfg(feature = "hardware-test-hooks")]
use sha2::{Digest as _, Sha256};

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

const TILE: usize = 16;
const ELEMENTS: usize = TILE * TILE;
const EXPLICIT_KERNARG_BYTES: usize = 24;
const HIDDEN_KERNARG_BYTES: usize = 256;
const COMPLETE_KERNARG_BYTES: usize = EXPLICIT_KERNARG_BYTES + HIDDEN_KERNARG_BYTES;
#[cfg(feature = "hardware-test-hooks")]
const HSA_KERNARG_ALIGNMENT: u64 = 16;
const CANARY_ELEMENTS: usize = 32;
#[cfg(feature = "hardware-test-hooks")]
const A_PREFIX: u16 = 0x7fc1;
#[cfg(feature = "hardware-test-hooks")]
const A_SUFFIX: u16 = 0x7fc2;
#[cfg(feature = "hardware-test-hooks")]
const B_PREFIX: u16 = 0x7fd1;
#[cfg(feature = "hardware-test-hooks")]
const B_SUFFIX: u16 = 0x7fd2;
const C_PREFIX: f32 = f32::from_bits(0x7fc0_c001);
const C_SUFFIX: f32 = f32::from_bits(0x7fc0_c002);
#[cfg(feature = "hardware-test-hooks")]
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

fn parse_exact_sha256(value: &str) -> Result<[u8; 32], BoxError> {
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "artifact SHA-256 must be exactly 64 lowercase hexadecimal digits",
    )?;
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    Ok(bytes)
}

fn validate_exact_artifact(bytes: &[u8]) -> Result<(), BoxError> {
    fe2o3_amdhsa_loader::validate(
        bytes,
        fe2o3_amdhsa_loader::AdmittedProfile::Gfx942XnackOffCov6,
    )
    .map_err(|error| format!("strict loader rejected BF16 GEMM artifact: {error:?}"))?;
    let bound = inspect_and_bind_kernel_descriptors(bytes)?;
    let inspection = bound.inspection();
    let [kernel] = inspection.kernels() else {
        return Err("BF16 GEMM artifact must contain exactly one kernel".into());
    };
    let [binding] = bound.bindings() else {
        return Err("BF16 GEMM artifact must contain exactly one descriptor binding".into());
    };
    require(
        inspection.code_object_version() == CodeObjectVersion::V6
            && inspection.target().to_string() == BF16_GEMM_KERNEL_TARGET_V1
            && !inspection.has_printf_metadata()
            && kernel.name() == BF16_GEMM_KERNEL_SYMBOL_V1
            && kernel.symbol() == BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1
            && kernel.kernarg_segment_size() == BF16_GEMM_KERNEL_KERNARG_BYTES_V1
            && kernel.kernarg_segment_alignment() == BF16_GEMM_KERNEL_KERNARG_ALIGNMENT_V1
            && kernel.implicit_argument_offset() == Some(EXPLICIT_KERNARG_BYTES as u64)
            && kernel.implicit_argument_size() == HIDDEN_KERNARG_BYTES as u64
            && kernel.required_workgroup_size() == Some([BF16_GEMM_KERNEL_WORKGROUP_X_V1, 1, 1])
            && kernel.max_flat_workgroup_size() == BF16_GEMM_KERNEL_WORKGROUP_X_V1
            && kernel.wavefront_size() == 64
            && kernel.group_segment_fixed_size() == 0
            && kernel.private_segment_fixed_size() == 0
            && kernel.sgpr_spill_count().unwrap_or(0) == 0
            && kernel.vgpr_spill_count().unwrap_or(0) == 0
            && !kernel.uses_dynamic_stack()
            && binding.kernel_index() == 0
            && binding.descriptor().kernarg_size() == COMPLETE_KERNARG_BYTES as u32
            && binding.descriptor().group_segment_fixed_size() == 0
            && binding.descriptor().private_segment_fixed_size() == 0
            && !binding.descriptor().private_segment_enabled()
            && binding.descriptor().wavefront_size() == 64
            && !binding.descriptor().uses_dynamic_stack(),
        "BF16 GEMM artifact metadata, descriptor, or resource profile drifted",
    )?;
    let expected = [
        (
            "a_bf16",
            0,
            8,
            ExplicitValueType::I16,
            ArgumentAccess::ReadOnly,
        ),
        (
            "b_bf16",
            8,
            2,
            ExplicitValueType::I16,
            ArgumentAccess::ReadOnly,
        ),
        (
            "c_f32",
            16,
            4,
            ExplicitValueType::F32,
            ArgumentAccess::WriteOnly,
        ),
    ];
    require(
        kernel.explicit_arguments().len() == expected.len(),
        "BF16 GEMM explicit argument count drifted",
    )?;
    for (index, (actual, expected)) in kernel.explicit_arguments().iter().zip(expected).enumerate()
    {
        require(
            actual.name() == Some(expected.0)
                && actual.offset() == expected.1
                && actual.size() == 8
                && actual.alignment().is_none_or(|alignment| alignment == 8)
                && actual
                    .pointee_alignment()
                    .is_none_or(|alignment| alignment == expected.2)
                && actual
                    .value_type()
                    .is_none_or(|value_type| value_type == expected.3)
                && actual.value_kind() == ExplicitValueKind::GlobalBuffer
                && actual.address_space() == Some(ArgumentAddressSpace::Global)
                && actual.access() == Some(expected.4),
            format!("BF16 GEMM explicit argument {index} drifted"),
        )?;
    }
    Ok(())
}

fn bf16_to_f32(bits: u16) -> f32 {
    f32::from_bits(u32::from(bits) << 16)
}

fn inputs() -> ([u16; ELEMENTS], [u16; ELEMENTS]) {
    const VALUES: [u16; 6] = [0x3f80, 0xbf80, 0x3f00, 0xbf00, 0x3e80, 0xbe80];
    let mut a = [0; ELEMENTS];
    let mut b = [0; ELEMENTS];
    for row in 0..TILE {
        for column in 0..TILE {
            let index = row * TILE + column;
            a[index] = VALUES[(row + 3 * column) % VALUES.len()];
            b[index] = VALUES[(2 * row + column + 1) % VALUES.len()];
        }
    }
    (a, b)
}

fn oracle(a: &[u16], b: &[u16]) -> Result<[f32; ELEMENTS], BoxError> {
    require(
        a.len() == ELEMENTS && b.len() == ELEMENTS,
        "BF16 GEMM oracle requires exact 16x16 A and B extents",
    )?;
    let mut output = [0.0; ELEMENTS];
    for row in 0..TILE {
        for column in 0..TILE {
            let mut accumulator = 0.0_f32;
            for depth in 0..TILE {
                accumulator +=
                    bf16_to_f32(a[row * TILE + depth]) * bf16_to_f32(b[depth * TILE + column]);
            }
            output[row * TILE + column] = accumulator;
        }
    }
    Ok(output)
}

#[cfg(feature = "hardware-test-hooks")]
fn guarded_u16(body: &[u16], prefix: u16, suffix: u16) -> Vec<u16> {
    let mut values = Vec::with_capacity(body.len() + 2 * CANARY_ELEMENTS);
    values.extend(std::iter::repeat_n(prefix, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, CANARY_ELEMENTS));
    values
}

fn guarded_f32(body: &[f32]) -> Vec<f32> {
    let mut values = Vec::with_capacity(body.len() + 2 * CANARY_ELEMENTS);
    values.extend(std::iter::repeat_n(C_PREFIX, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(C_SUFFIX, CANARY_ELEMENTS));
    values
}

#[cfg(feature = "hardware-test-hooks")]
fn require_u16_guarded(
    role: &str,
    actual: &[u16],
    body: &[u16],
    prefix: u16,
    suffix: u16,
) -> Result<(), BoxError> {
    require(
        actual.len() == body.len() + 2 * CANARY_ELEMENTS
            && actual[..CANARY_ELEMENTS]
                .iter()
                .all(|value| *value == prefix)
            && actual[CANARY_ELEMENTS..CANARY_ELEMENTS + body.len()] == *body
            && actual[CANARY_ELEMENTS + body.len()..]
                .iter()
                .all(|value| *value == suffix),
        format!("{role} body or canary changed"),
    )
}

fn require_f32_guarded(actual: &[f32], body: &[f32]) -> Result<(), BoxError> {
    let expected = guarded_f32(body);
    require(
        actual.len() == expected.len()
            && actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.to_bits() == expected.to_bits()),
        "BF16 GEMM FP32 output or canary differs bitwise",
    )
}

fn launch_geometry() -> HsaLaunchGeometryV1 {
    let shape = Bf16GemmKernelDispatchShapeV1::exact();
    HsaLaunchGeometryV1::new(shape.hsa_adapter_block_counts(), shape.workgroup(), 0)
}

fn explicit_kernarg(addresses: [u64; 3]) -> [u8; EXPLICIT_KERNARG_BYTES] {
    let mut bytes = [0; EXPLICIT_KERNARG_BYTES];
    for (index, address) in addresses.into_iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&address.to_le_bytes());
    }
    bytes
}

#[cfg(feature = "hardware-test-hooks")]
struct ObservedPinnedArtifact {
    bytes: Vec<u8>,
    digest: PayloadDigest,
}

#[cfg(feature = "hardware-test-hooks")]
fn read_observed_artifact() -> Result<ObservedPinnedArtifact, BoxError> {
    require(
        std::env::var("FE2O3_RUN_M1_BF16_GEMM_V1_HARDWARE").as_deref() == Ok("1"),
        "set FE2O3_RUN_M1_BF16_GEMM_V1_HARDWARE=1 to opt in",
    )?;
    let path = std::path::PathBuf::from(
        std::env::var_os("FE2O3_M1_BF16_GEMM_V1_HSACO")
            .ok_or("FE2O3_M1_BF16_GEMM_V1_HSACO is not set")?,
    );
    let canonical = std::fs::canonicalize(&path)?;
    let metadata = std::fs::symlink_metadata(&path)?;
    require(
        canonical == path
            && metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && (1..=fe2o3_hsaco::MAX_HSACO_BYTES as u64).contains(&metadata.len()),
        "BF16 GEMM HSACO path must be canonical, regular, and bounded",
    )?;
    let expected = parse_exact_sha256(
        &std::env::var("FE2O3_M1_BF16_GEMM_V1_SHA256")
            .map_err(|_| "FE2O3_M1_BF16_GEMM_V1_SHA256 is not set")?,
    )?;
    let bytes = std::fs::read(&path)?;
    require(
        bytes.len() as u64 == metadata.len() && sha256(&bytes) == expected,
        "BF16 GEMM HSACO size or SHA-256 differs from the observation pin",
    )?;
    validate_exact_artifact(&bytes)?;
    Ok(ObservedPinnedArtifact {
        digest: DigestAlgorithm::Sha256.calculate(&bytes),
        bytes,
    })
}

#[cfg(feature = "hardware-test-hooks")]
fn u16_bytes(values: &[u16]) -> &[u8] {
    // SAFETY: `u16` has no invalid bit patterns and the extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: `f32` has no invalid bit patterns and the extent is exact.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn u16_values(bytes: &[u8]) -> Result<Vec<u16>, BoxError> {
    require(bytes.len().is_multiple_of(2), "partial u16 readback")?;
    Ok(bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_ne_bytes(chunk.try_into().expect("exact u16 chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_values(bytes: &[u8]) -> Result<Vec<f32>, BoxError> {
    require(bytes.len().is_multiple_of(4), "partial f32 readback")?;
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
        "guarded BF16 GEMM allocation extent drifted",
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
        // SAFETY: `layout` is valid and this owner deallocates exactly once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("BF16 GEMM kernarg allocation failed")?;
        Ok(Self { pointer, layout })
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: the allocation is live for its exact layout.
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
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    explicit: &[u8; EXPLICIT_KERNARG_BYTES],
) -> Result<(), BoxError> {
    require(
        resolution.export_symbol() == BF16_GEMM_KERNEL_SYMBOL_V1
            && resolution.kernarg_segment_size() == COMPLETE_KERNARG_BYTES as u64
            && resolution.kernarg_segment_alignment() == HSA_KERNARG_ALIGNMENT,
        "runtime BF16 GEMM kernel resolution drifted",
    )?;
    let geometry = launch_geometry();
    let shape = Bf16GemmKernelDispatchShapeV1::exact();
    require(
        geometry.grid() == shape.hsa_adapter_block_counts()
            && geometry.workgroup() == shape.workgroup()
            && shape.hsa_block_counts_expand_to_aql_grid(geometry.grid()),
        "HSA block counts do not expand to the exact BF16 GEMM AQL workitem grid",
    )?;
    let mut storage = RuntimeKernarg::new()?;
    let kernarg = storage.bytes_mut();
    kernarg[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);
    // SAFETY: this ignored observation uses one inspected, digest-pinned image,
    // three live guarded buffers, exact initialized kernarg bytes, and a
    // synchronous terminal wait. It creates no production authority.
    unsafe {
        adapter.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            EXPLICIT_KERNARG_BYTES,
            EXPLICIT_KERNARG_BYTES,
            HIDDEN_KERNARG_BYTES,
            kernarg,
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(
            completion.completed(),
            "BF16 GEMM dispatch did not complete",
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn execute(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
) -> Result<(), BoxError> {
    let (a_body, b_body) = inputs();
    let expected = oracle(&a_body, &b_body)?;
    let a_host = guarded_u16(&a_body, A_PREFIX, A_SUFFIX);
    let b_host = guarded_u16(&b_body, B_PREFIX, B_SUFFIX);
    let c_host = guarded_f32(&[C_POISON; ELEMENTS]);
    let a = adapter.allocate_hardware_test_buffer(u16_bytes(&a_host))?;
    let b = adapter.allocate_hardware_test_buffer(u16_bytes(&b_host))?;
    let c = adapter.allocate_hardware_test_buffer(f32_bytes(&c_host))?;
    let addresses = [
        body_address(&a, 2)?,
        body_address(&b, 2)?,
        body_address(&c, 4)?,
    ];
    let numerical = Bf16GemmBufferContractV1::new(
        addresses[0],
        BF16_GEMM_A_BYTES_V1,
        addresses[1],
        BF16_GEMM_B_BYTES_V1,
        addresses[2],
        BF16_GEMM_C_BYTES_V1,
    )?;
    require(
        numerical.addresses() == addresses
            && !numerical.authenticates_device_memory()
            && !numerical.grants_launch_authority(),
        "numerical BF16 GEMM buffer observation drifted",
    )?;
    let explicit = explicit_kernarg(addresses);
    // SAFETY: `dispatch` owns the sole raw boundary and waits synchronously.
    unsafe { dispatch(adapter, executable, kernel, resolution, &explicit)? };

    let a_after = u16_values(&a.read_after_synchronous_dispatch())?;
    let b_after = u16_values(&b.read_after_synchronous_dispatch())?;
    let c_after = f32_values(&c.read_after_synchronous_dispatch())?;
    require_u16_guarded("A", &a_after, &a_body, A_PREFIX, A_SUFFIX)?;
    require_u16_guarded("B", &b_after, &b_body, B_PREFIX, B_SUFFIX)?;
    require_f32_guarded(&c_after, &expected)
}

/// Executes one unqualified exact `16x16x16` BF16/BF16-to-FP32 observation.
///
/// The supplied digest is an observation pin, not independent artifact
/// approval. A qualifying runner must instead retain the sealed Worker V2
/// admission lineage through load and dispatch.
///
/// ```text
/// FE2O3_RUN_M1_BF16_GEMM_V1_HARDWARE=1 \
/// FE2O3_M1_BF16_GEMM_V1_HSACO=/canonical/bf16-gemm-v1.hsaco \
/// FE2O3_M1_BF16_GEMM_V1_SHA256=<64-lowercase-hex-digits> \
/// HSA_XNACK=0 HIP_VISIBLE_DEVICES=0 ROCR_VISIBLE_DEVICES=0 \
/// cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test m1_bf16_gemm_v1_hardware \
///   gfx942_m1_bf16_gemm_v1_unqualified_hardware_observation \
///   -- --ignored --exact --nocapture
/// ```
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "unqualified: requires caller-pinned HSACO and gfx942:xnack-"]
fn gfx942_m1_bf16_gemm_v1_unqualified_hardware_observation() -> Result<(), BoxError> {
    let artifact = read_observed_artifact()?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    let target = adapter.environment().physical_device().target();
    require(
        target.processor() == "gfx942" && target.xnack() == Some(FeatureState::Disabled),
        "BF16 GEMM observation requires one gfx942:xnack- device",
    )?;
    // SAFETY: this is an explicitly unqualified observation. Exact bytes are
    // retained through one load, synchronous execution, and terminal unload.
    let (executable, load) = unsafe { adapter.load_executable(&artifact.bytes, artifact.digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(
            load.finalized_digest() == artifact.digest
                && load.byte_len() == artifact.bytes.len() as u64,
            "loaded BF16 GEMM bytes drifted",
        )?;
        // SAFETY: exact inspection admitted one matching export/descriptor.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [BF16_GEMM_KERNEL_SYMBOL_V1]) }?;
        require(
            kernels.len() == 1
                && resolutions.len() == 1
                && resolutions[0].executable_object() == executable_identity,
            "runtime resolved a substituted BF16 GEMM kernel",
        )?;
        execute(
            &mut adapter,
            &executable,
            kernels.get(0).ok_or("resolved BF16 GEMM kernel missing")?,
            &resolutions[0],
        )
    })();
    // SAFETY: all kernel borrows and synchronous buffer uses ended above.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(
        unload.released() && unload.executable_object() == executable_identity,
        "BF16 GEMM executable did not unload terminally",
    )?;
    execution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_and_kernarg_are_exact() {
        let geometry = launch_geometry();
        assert_eq!(geometry.grid(), [1, 1, 1]);
        assert_eq!(geometry.workgroup(), [64, 1, 1]);
        let shape = Bf16GemmKernelDispatchShapeV1::exact();
        assert_eq!(shape.grid(), [64, 1, 1]);
        assert!(shape.hsa_block_counts_expand_to_aql_grid(geometry.grid()));
        assert!(!shape.hsa_block_counts_expand_to_aql_grid([64, 1, 1]));
        assert_eq!(geometry.dynamic_shared_memory_bytes(), 0);
        let packed = explicit_kernarg([0x1000, 0x2000, 0x3000]);
        for (index, address) in [0x1000_u64, 0x2000, 0x3000].into_iter().enumerate() {
            assert_eq!(&packed[index * 8..index * 8 + 8], &address.to_le_bytes());
        }
        assert_eq!(COMPLETE_KERNARG_BYTES, 280);
    }

    #[test]
    fn dyadic_oracle_and_canaries_reject_mutations() {
        let (a, b) = inputs();
        let expected = oracle(&a, &b).unwrap();
        assert!(expected.iter().all(|value| value.is_finite()));
        let guarded = guarded_f32(&expected);
        require_f32_guarded(&guarded, &expected).unwrap();
        for index in [0, CANARY_ELEMENTS + 137, guarded.len() - 1] {
            let mut hostile = guarded.clone();
            hostile[index] = f32::from_bits(hostile[index].to_bits() ^ 1);
            assert!(require_f32_guarded(&hostile, &expected).is_err());
        }
    }

    #[test]
    fn digest_parser_rejects_noncanonical_pins() {
        assert!(parse_exact_sha256(&"ab".repeat(32)).is_ok());
        assert!(parse_exact_sha256(&"AB".repeat(32)).is_err());
        assert!(parse_exact_sha256(&"0".repeat(63)).is_err());
    }

    #[test]
    fn invalid_artifact_never_reaches_observation() {
        assert!(validate_exact_artifact(b"not an ELF").is_err());
    }
}
