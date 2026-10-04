//! Explicitly non-authoritative raw gfx942 Scalar GEMM V1 hardware smoke test.
//!
//! This test bypasses production prerequisite authentication and can never
//! grant protected evidence. It exists only to exercise a digest-pinned HSACO
//! through the reviewed raw unsafe HSA adapter while the protected controller
//! remains the sole production evidence path.

#[allow(clippy::enum_variant_names)]
#[path = "../../../examples/scalar_gemm_v1/src/harness.rs"]
mod scalar_harness;

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
use scalar_harness::{HARDWARE_CASES, HardwareCase, SCALAR_GEMM_WORKGROUP_X, scalar_gemm_oracle};

#[cfg(feature = "hardware-test-hooks")]
const SCALAR_GEMM_EXPORT: &str = "scalar_gemm_v1";
#[cfg(feature = "hardware-test-hooks")]
const SCALAR_GEMM_DESCRIPTOR: &str = "scalar_gemm_v1.kd";
#[cfg(feature = "hardware-test-hooks")]
const EXPLICIT_KERNARG_BYTES: usize = 64;
#[cfg(feature = "hardware-test-hooks")]
const COV6_IMPLICIT_KERNARG_BYTES: usize = 256;
#[cfg(feature = "hardware-test-hooks")]
const COMPLETE_KERNARG_BYTES: usize = EXPLICIT_KERNARG_BYTES + COV6_IMPLICIT_KERNARG_BYTES;
#[cfg(feature = "hardware-test-hooks")]
const PHYSICAL_KERNARG_ALIGNMENT: u64 = 8;
#[cfg(feature = "hardware-test-hooks")]
const HSA_KERNARG_ALIGNMENT: u64 = 16;
#[cfg(feature = "hardware-test-hooks")]
const CANARY_ELEMENTS: usize = 32;
#[cfg(feature = "hardware-test-hooks")]
const INPUT_LEFT_CANARY: f32 = f32::from_bits(0x7fc0_a001);
#[cfg(feature = "hardware-test-hooks")]
const INPUT_RIGHT_CANARY: f32 = f32::from_bits(0x7fc0_a002);
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_LEFT_CANARY: f32 = f32::from_bits(0x7fc0_c001);
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_RIGHT_CANARY: f32 = f32::from_bits(0x7fc0_c002);
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_c0ff);

#[cfg(feature = "hardware-test-hooks")]
type BoxError = Box<dyn std::error::Error>;

#[cfg(feature = "hardware-test-hooks")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeviceSlice {
    address: u64,
    len: usize,
}

#[cfg(feature = "hardware-test-hooks")]
fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn parse_exact_sha256(hex: &str) -> Result<[u8; 32], BoxError> {
    require(
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "FE2O3_GFX942_SCALAR_GEMM_SHA256 must be exactly 64 lowercase hex digits",
    )?;
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "FE2O3_GFX942_SCALAR_GEMM_SHA256 is malformed")?;
    }
    Ok(bytes)
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_hsaco() -> Result<(Vec<u8>, PayloadDigest), BoxError> {
    require(
        std::env::var("FE2O3_RUN_GFX942_SCALAR_GEMM_SMOKE").as_deref() == Ok("1"),
        "set FE2O3_RUN_GFX942_SCALAR_GEMM_SMOKE=1 to opt into the non-authoritative smoke test",
    )?;
    let path = std::path::PathBuf::from(
        std::env::var_os("FE2O3_GFX942_SCALAR_GEMM_HSACO")
            .ok_or("FE2O3_GFX942_SCALAR_GEMM_HSACO is not set")?,
    );
    require(
        path.is_absolute(),
        "FE2O3_GFX942_SCALAR_GEMM_HSACO must be an absolute path",
    )?;
    let metadata = std::fs::symlink_metadata(&path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "FE2O3_GFX942_SCALAR_GEMM_HSACO must name a regular non-symlink file",
    )?;
    require(
        std::fs::canonicalize(&path)? == path,
        "FE2O3_GFX942_SCALAR_GEMM_HSACO must already be canonical",
    )?;
    require(
        (1..=fe2o3_hsaco::MAX_HSACO_BYTES as u64).contains(&metadata.len()),
        "FE2O3_GFX942_SCALAR_GEMM_HSACO has an invalid byte length",
    )?;

    let expected = parse_exact_sha256(
        &std::env::var("FE2O3_GFX942_SCALAR_GEMM_SHA256")
            .map_err(|_| "FE2O3_GFX942_SCALAR_GEMM_SHA256 is not set")?,
    )?;
    let bytes = std::fs::read(&path)?;
    require(
        bytes.len() as u64 == metadata.len(),
        "FE2O3_GFX942_SCALAR_GEMM_HSACO changed size while being read",
    )?;
    let final_metadata = std::fs::symlink_metadata(&path)?;
    require(
        final_metadata.file_type().is_file()
            && !final_metadata.file_type().is_symlink()
            && final_metadata.len() == metadata.len()
            && std::fs::canonicalize(&path)? == path,
        "FE2O3_GFX942_SCALAR_GEMM_HSACO changed identity while being read",
    )?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    require(
        digest.bytes().as_bytes() == &expected,
        "FE2O3_GFX942_SCALAR_GEMM_HSACO does not match its exact SHA-256 pin",
    )?;
    Ok((bytes, digest))
}

#[cfg(feature = "hardware-test-hooks")]
fn inspect_scalar_gemm_profile(bytes: &[u8]) -> Result<(), BoxError> {
    let bound = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)?;
    let inspected = bound.inspection();
    require(
        inspected.code_object_version() == CodeObjectVersion::V6,
        "Scalar GEMM smoke HSACO must be code object V6",
    )?;
    require(
        inspected.target().processor() == "gfx942"
            && inspected.target().xnack() == Some(FeatureState::Disabled),
        "Scalar GEMM smoke HSACO must target gfx942:xnack-",
    )?;
    require(
        !inspected.has_printf_metadata(),
        "Scalar GEMM smoke HSACO must not carry printf metadata",
    )?;
    let [kernel] = inspected.kernels() else {
        return Err("Scalar GEMM smoke HSACO must declare exactly one kernel".into());
    };
    require(
        kernel.name() == SCALAR_GEMM_EXPORT && kernel.symbol() == SCALAR_GEMM_DESCRIPTOR,
        "Scalar GEMM smoke HSACO has a substituted kernel or descriptor symbol",
    )?;
    require(
        kernel.kernarg_segment_size() == COMPLETE_KERNARG_BYTES as u64
            && kernel.kernarg_segment_alignment() == PHYSICAL_KERNARG_ALIGNMENT
            && kernel.implicit_argument_offset() == Some(EXPLICIT_KERNARG_BYTES as u64)
            && kernel.implicit_argument_size() == COV6_IMPLICIT_KERNARG_BYTES as u64,
        "Scalar GEMM smoke HSACO does not expose the exact 64+256-byte COV6 kernarg",
    )?;
    require(
        kernel.required_workgroup_size() == Some([SCALAR_GEMM_WORKGROUP_X, 1, 1])
            && kernel.max_flat_workgroup_size() == SCALAR_GEMM_WORKGROUP_X
            && kernel.wavefront_size() == 64
            && kernel.group_segment_fixed_size() == 0
            && kernel.private_segment_fixed_size() == 0,
        "Scalar GEMM smoke HSACO does not expose the exact WG256 launch profile",
    )?;

    const EXPECTED_EXPLICIT_FIELDS: [(u64, u64, ExplicitValueKind); 9] = [
        (0, 8, ExplicitValueKind::GlobalBuffer),
        (8, 8, ExplicitValueKind::ByValue),
        (16, 8, ExplicitValueKind::GlobalBuffer),
        (24, 8, ExplicitValueKind::ByValue),
        (32, 8, ExplicitValueKind::GlobalBuffer),
        (40, 8, ExplicitValueKind::ByValue),
        (48, 4, ExplicitValueKind::ByValue),
        (52, 4, ExplicitValueKind::ByValue),
        (56, 4, ExplicitValueKind::ByValue),
    ];
    let actual_fields = kernel
        .explicit_arguments()
        .iter()
        .map(|argument| (argument.offset(), argument.size(), argument.value_kind()))
        .collect::<Vec<_>>();
    require(
        actual_fields.as_slice() == EXPECTED_EXPLICIT_FIELDS,
        "Scalar GEMM smoke HSACO has a substituted explicit ABI",
    )?;

    let [binding] = bound.bindings() else {
        return Err("Scalar GEMM smoke HSACO must bind exactly one kernel descriptor".into());
    };
    let descriptor = binding.descriptor();
    require(
        binding.kernel_index() == 0
            && descriptor.kernarg_size() == COMPLETE_KERNARG_BYTES as u32
            && descriptor.group_segment_fixed_size() == 0
            && descriptor.private_segment_fixed_size() == 0
            && descriptor.wavefront_size() == 64
            && !descriptor.uses_dynamic_stack(),
        "Scalar GEMM descriptor bytes disagree with the inspected profile",
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "hardware-test-hooks")]
fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "hardware-test-hooks")]
fn scalar_gemm_explicit_kernarg(
    a: DeviceSlice,
    b: DeviceSlice,
    c: DeviceSlice,
    dimensions: [u32; 3],
) -> Result<[u8; EXPLICIT_KERNARG_BYTES], BoxError> {
    let mut bytes = [0; EXPLICIT_KERNARG_BYTES];
    put_u64(&mut bytes, 0, a.address);
    put_u64(&mut bytes, 8, u64::try_from(a.len)?);
    put_u64(&mut bytes, 16, b.address);
    put_u64(&mut bytes, 24, u64::try_from(b.len)?);
    put_u64(&mut bytes, 32, c.address);
    put_u64(&mut bytes, 40, u64::try_from(c.len)?);
    put_u32(&mut bytes, 48, dimensions[0]);
    put_u32(&mut bytes, 52, dimensions[1]);
    put_u32(&mut bytes, 56, dimensions[2]);
    require(
        bytes[60..64] == [0; 4],
        "Scalar GEMM explicit tail padding must be zero",
    )?;
    Ok(bytes)
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
            .ok_or("failed to allocate runtime-aligned Scalar GEMM kernarg storage")?;
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
fn guarded(body: &[f32], left: f32, right: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(CANARY_ELEMENTS * 2 + body.len());
    values.extend(std::iter::repeat_n(left, CANARY_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(right, CANARY_ELEMENTS));
    values
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
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().expect("exact f32 chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn require_bits_equal(
    case: &str,
    role: &str,
    actual: &[f32],
    expected: &[f32],
) -> Result<(), BoxError> {
    require(
        actual.len() == expected.len(),
        format!(
            "{case} {role} length changed: {} != {}",
            actual.len(),
            expected.len()
        ),
    )?;
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        require(
            actual.to_bits() == expected.to_bits(),
            format!(
                "{case} {role}[{index}] changed: {:#010x} != {:#010x}",
                actual.to_bits(),
                expected.to_bits()
            ),
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn body_slice(
    buffer: &ReviewedHsaHardwareTestBufferV1,
    body_len: usize,
) -> Result<DeviceSlice, BoxError> {
    let allocation_elements = CANARY_ELEMENTS
        .checked_mul(2)
        .and_then(|guards| guards.checked_add(body_len))
        .ok_or("guarded allocation extent overflow")?;
    require(
        buffer.byte_len() == allocation_elements * std::mem::size_of::<f32>(),
        "guarded allocation has the wrong physical extent",
    )?;
    Ok(DeviceSlice {
        address: buffer.device_address(CANARY_ELEMENTS * std::mem::size_of::<f32>())?,
        len: body_len,
    })
}

#[cfg(feature = "hardware-test-hooks")]
fn verify_guarded_output(
    case: &str,
    actual: &[f32],
    expected_body: &[f32],
) -> Result<(), BoxError> {
    let expected_len = CANARY_ELEMENTS * 2 + expected_body.len();
    require(
        actual.len() == expected_len,
        format!("{case} guarded C allocation length changed"),
    )?;
    let (left, remainder) = actual.split_at(CANARY_ELEMENTS);
    let (body, right) = remainder.split_at(expected_body.len());
    require_bits_equal(
        case,
        "left C canary",
        left,
        &[OUTPUT_LEFT_CANARY; CANARY_ELEMENTS],
    )?;
    require_bits_equal(case, "C output", body, expected_body)?;
    require_bits_equal(
        case,
        "right C canary",
        right,
        &[OUTPUT_RIGHT_CANARY; CANARY_ELEMENTS],
    )
}

#[cfg(feature = "hardware-test-hooks")]
unsafe fn dispatch_scalar_gemm_cov6(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    groups: u32,
    explicit: &[u8; EXPLICIT_KERNARG_BYTES],
) -> Result<(), BoxError> {
    require(
        resolution.export_symbol() == SCALAR_GEMM_EXPORT,
        "runtime resolved a substituted Scalar GEMM export",
    )?;
    require(
        resolution.kernarg_segment_size() == COMPLETE_KERNARG_BYTES as u64
            && resolution.kernarg_segment_alignment() == HSA_KERNARG_ALIGNMENT,
        "runtime did not expose the exact 320-byte aligned Scalar GEMM kernarg",
    )?;
    let expected_groups = groups
        .checked_mul(SCALAR_GEMM_WORKGROUP_X)
        .ok_or("rounded Scalar GEMM grid overflow")?;
    require(
        expected_groups >= SCALAR_GEMM_WORKGROUP_X,
        "Scalar GEMM dispatch must contain at least one complete WG256 workgroup",
    )?;
    let geometry = HsaLaunchGeometryV1::new([groups, 1, 1], [SCALAR_GEMM_WORKGROUP_X, 1, 1], 0);
    let mut storage = RuntimeKernarg::new()?;
    let kernarg = storage.bytes_mut();
    kernarg[..EXPLICIT_KERNARG_BYTES].copy_from_slice(explicit);

    // SAFETY: this explicitly non-authoritative smoke boundary inspected the
    // exact digest-pinned scalar-only COV6 image, packed its frozen 64-byte ABI,
    // retains every allocation and HSA token, initializes all 256 hidden bytes,
    // and waits synchronously. It does not authenticate compiler prerequisites.
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
            "Scalar GEMM raw smoke dispatch did not complete synchronously",
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_case(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    case: HardwareCase,
) -> Result<(), BoxError> {
    let shape = case.shape()?;
    let dimensions = shape.dimensions();
    let expected_groups = shape.expected_groups()?;
    let (a_body, b_body) = case.inputs(shape);
    let expected_c = scalar_gemm_oracle(shape, &a_body, &b_body);
    let a_host = guarded(&a_body, INPUT_LEFT_CANARY, INPUT_RIGHT_CANARY);
    let b_host = guarded(&b_body, INPUT_LEFT_CANARY, INPUT_RIGHT_CANARY);
    let c_host = guarded(
        &vec![OUTPUT_POISON; shape.c_len],
        OUTPUT_LEFT_CANARY,
        OUTPUT_RIGHT_CANARY,
    );
    let a = adapter.allocate_hardware_test_buffer(f32_bytes(&a_host))?;
    let b = adapter.allocate_hardware_test_buffer(f32_bytes(&b_host))?;
    // One physical allocation contains the left canary, C body, and right canary.
    let c = adapter.allocate_hardware_test_buffer(f32_bytes(&c_host))?;
    let explicit = scalar_gemm_explicit_kernarg(
        body_slice(&a, shape.a_len)?,
        body_slice(&b, shape.b_len)?,
        body_slice(&c, shape.c_len)?,
        dimensions,
    )?;

    let dispatched = if let Some(groups) = expected_groups {
        require(
            groups == u32::try_from(shape.c_len.div_ceil(SCALAR_GEMM_WORKGROUP_X as usize))?,
            format!("{} did not derive rounded WG256 geometry", case.name),
        )?;
        // SAFETY: `dispatch_scalar_gemm_cov6` owns the reviewed raw boundary and
        // returns only after every referenced allocation is synchronously idle.
        unsafe {
            dispatch_scalar_gemm_cov6(adapter, executable, kernel, resolution, groups, &explicit)?;
        }
        true
    } else {
        false
    };
    require(
        dispatched == expected_groups.is_some(),
        format!("{} zero-output no-dispatch state changed", case.name),
    )?;

    let a_after = f32_values(&a.read_after_synchronous_dispatch())?;
    let b_after = f32_values(&b.read_after_synchronous_dispatch())?;
    let c_after = f32_values(&c.read_after_synchronous_dispatch())?;
    require_bits_equal(case.name, "immutable A allocation", &a_after, &a_host)?;
    require_bits_equal(case.name, "immutable B allocation", &b_after, &b_host)?;
    verify_guarded_output(case.name, &c_after, &expected_c)?;
    if case.k == 0 {
        let body = &c_after[CANARY_ELEMENTS..CANARY_ELEMENTS + shape.c_len];
        require(
            body.iter().all(|value| value.to_bits() == 0_f32.to_bits()),
            format!("{} did not write positive zero for k=0", case.name),
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_non_authoritative_raw_smoke(bytes: Vec<u8>, digest: PayloadDigest) -> Result<(), BoxError> {
    inspect_scalar_gemm_profile(&bytes)?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context)?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx942"
            && adapter.environment().physical_device().target().xnack()
                == Some(FeatureState::Disabled),
        "Scalar GEMM raw smoke requires a gfx942:xnack- physical device",
    )?;

    // SAFETY: the exact bytes are immutable, SHA-256 pinned, structurally
    // inspected, and retained until the single consuming unload below. This is
    // still not production prerequisite authentication.
    let (executable, load) = unsafe { adapter.load_executable(&bytes, digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(load.finalized_digest() == digest, "load digest changed")?;
        require(
            load.byte_len() == bytes.len() as u64,
            "load byte length changed",
        )?;
        // SAFETY: structural inspection admitted exactly scalar_gemm_v1 and its
        // descriptor; this call deliberately resolves only that one export.
        let (kernels, resolutions) =
            unsafe { adapter.resolve_kernel_set(&executable, [SCALAR_GEMM_EXPORT]) }?;
        require(
            kernels.len() == 1 && resolutions.len() == 1,
            "runtime did not resolve exactly one Scalar GEMM kernel",
        )?;
        require(
            resolutions[0].export_symbol() == SCALAR_GEMM_EXPORT
                && resolutions[0].executable_object() == executable_identity,
            "resolved Scalar GEMM kernel belongs to a substituted executable",
        )?;
        let kernel = kernels
            .get(0)
            .ok_or("runtime omitted the resolved Scalar GEMM kernel")?;
        for case in HARDWARE_CASES {
            run_case(&mut adapter, &executable, kernel, &resolutions[0], *case)?;
        }
        Ok(())
    })();

    // The kernel set has been dropped. This is the sole terminal consuming
    // unload, so the executable cannot be reused after this observation.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(
        unload.released() && unload.executable_object() == executable_identity,
        "reviewed HSA unload did not release the exact Scalar GEMM executable",
    )?;
    execution
}

/// Runs real Scalar GEMM V1 on gfx942 through the reviewed raw unsafe HSA adapter.
///
/// This ignored smoke test **bypasses production prerequisite authentication**
/// and **grants no protected evidence**. Passing it establishes only that the
/// exact SHA-256-pinned scalar-only COV6 bytes execute the canonical hardware
/// case matrix correctly in this process.
///
/// Exact invocation:
///
/// ```text
/// FE2O3_RUN_GFX942_SCALAR_GEMM_SMOKE=1 \
/// FE2O3_GFX942_SCALAR_GEMM_HSACO=/absolute/canonical/scalar-gemm-v1-gfx942.hsaco \
/// FE2O3_GFX942_SCALAR_GEMM_SHA256=<64-lowercase-hex-digits> \
/// cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test gfx942_scalar_gemm_hardware \
///   gfx942_scalar_gemm_v1_raw_smoke_bypasses_production_prerequisite_authentication_and_grants_no_protected_evidence \
///   -- --ignored --exact --nocapture
/// ```
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "non-authoritative: bypasses production prerequisite authentication, grants no protected evidence, and requires gfx942:xnack-"]
fn gfx942_scalar_gemm_v1_raw_smoke_bypasses_production_prerequisite_authentication_and_grants_no_protected_evidence()
-> Result<(), BoxError> {
    let (bytes, digest) = read_pinned_hsaco()?;
    run_non_authoritative_raw_smoke(bytes, digest)
}

#[cfg(all(test, feature = "hardware-test-hooks"))]
mod tests {
    use super::*;

    #[test]
    fn explicit_kernarg_matches_the_frozen_scalar_layout() {
        let packed = scalar_gemm_explicit_kernarg(
            DeviceSlice {
                address: 0x1111,
                len: 15,
            },
            DeviceSlice {
                address: 0x2222,
                len: 35,
            },
            DeviceSlice {
                address: 0x3333,
                len: 21,
            },
            [3, 7, 5],
        )
        .unwrap();
        assert_eq!(&packed[0..8], &0x1111_u64.to_le_bytes());
        assert_eq!(&packed[8..16], &15_u64.to_le_bytes());
        assert_eq!(&packed[16..24], &0x2222_u64.to_le_bytes());
        assert_eq!(&packed[24..32], &35_u64.to_le_bytes());
        assert_eq!(&packed[32..40], &0x3333_u64.to_le_bytes());
        assert_eq!(&packed[40..48], &21_u64.to_le_bytes());
        assert_eq!(&packed[48..52], &3_u32.to_le_bytes());
        assert_eq!(&packed[52..56], &7_u32.to_le_bytes());
        assert_eq!(&packed[56..60], &5_u32.to_le_bytes());
        assert_eq!(&packed[60..64], &[0; 4]);
        assert_eq!(COMPLETE_KERNARG_BYTES, 320);
    }

    #[test]
    fn hardware_matrix_covers_no_dispatch_and_positive_zero() {
        assert!(
            HARDWARE_CASES
                .iter()
                .filter_map(|case| case.shape().ok())
                .any(|shape| shape.expected_groups() == Ok(None))
        );
        assert!(
            HARDWARE_CASES
                .iter()
                .any(|case| case.k == 0 && case.m * case.n > 0)
        );
    }

    #[test]
    fn bitwise_checks_reject_canary_and_signed_zero_substitution() {
        let expected = guarded(&[0.0], OUTPUT_LEFT_CANARY, OUTPUT_RIGHT_CANARY);
        verify_guarded_output("unit", &expected, &[0.0]).unwrap();

        let mut wrong_canary = expected.clone();
        wrong_canary[0] = OUTPUT_RIGHT_CANARY;
        assert!(verify_guarded_output("unit", &wrong_canary, &[0.0]).is_err());

        let mut negative_zero = expected;
        negative_zero[CANARY_ELEMENTS] = -0.0;
        assert!(verify_guarded_output("unit", &negative_zero, &[0.0]).is_err());
    }

    #[test]
    fn sha256_pin_requires_exact_lowercase_encoding() {
        assert!(parse_exact_sha256(&"ab".repeat(32)).is_ok());
        assert!(parse_exact_sha256(&"AB".repeat(32)).is_err());
        assert!(parse_exact_sha256(&"0".repeat(63)).is_err());
    }
}
