use std::fmt;

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_amd_target::FeatureState;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_artifacts::DigestAlgorithm;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_core::{DeviceBuffer, GpuContext};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_host::{
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaKernelV1, ReviewedHsaRuntimeAdapterV1,
};

const WORKGROUP_SIZE: usize = 256;
#[cfg(feature = "hardware-test-hooks")]
const COV6_IMPLICIT_BYTES: usize = 256;
const PHYSICAL_COV6_KERNARG_ALIGNMENT: u64 = 8;
const REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT: u64 =
    reviewed_hsa_resolution_alignment(PHYSICAL_COV6_KERNARG_ALIGNMENT);
const ALPHA_EXPLICIT_BYTES: usize = 40;
const ZETA_EXPLICIT_BYTES: usize = 56;
const GUARD_PREFIX_ELEMENTS: usize = 8;
const GUARD_SUFFIX_ELEMENTS: usize = 11;
const HARDWARE_LENGTHS: [usize; 5] = [1, 255, 256, 257, 1023];

#[cfg(feature = "hardware-test-hooks")]
const INPUT_PREFIX: f32 = 12_345.0;
#[cfg(feature = "hardware-test-hooks")]
const INPUT_SUFFIX: f32 = -23_456.0;
#[cfg(feature = "hardware-test-hooks")]
const B_PREFIX: f32 = 34_567.0;
#[cfg(feature = "hardware-test-hooks")]
const B_SUFFIX: f32 = -45_678.0;
const ALPHA_PREFIX: f32 = 56_789.0;
const ALPHA_SUFFIX: f32 = -67_890.0;
#[cfg(feature = "hardware-test-hooks")]
const ZETA_PREFIX: f32 = 78_901.0;
#[cfg(feature = "hardware-test-hooks")]
const ZETA_SUFFIX: f32 = -89_012.0;
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_FILL: f32 = 9_876.0;

const fn reviewed_hsa_resolution_alignment(physical_alignment: u64) -> u64 {
    if physical_alignment > REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT {
        physical_alignment
    } else {
        REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LengthMismatch {
    kernel: &'static str,
    expected_argument: usize,
    expected: usize,
    actual_argument: usize,
    actual: usize,
}

impl fmt::Display for LengthMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} argument {} has length {}, but argument {} has length {}",
            self.kernel, self.actual_argument, self.actual, self.expected_argument, self.expected
        )
    }
}

impl std::error::Error for LengthMismatch {}

fn equal_length(kernel: &'static str, lengths: &[usize]) -> Result<usize, LengthMismatch> {
    let expected = lengths[0];
    for (argument, &actual) in lengths.iter().enumerate().skip(1) {
        if actual != expected {
            return Err(LengthMismatch {
                kernel,
                expected_argument: 0,
                expected,
                actual_argument: argument,
                actual,
            });
        }
    }
    Ok(expected)
}

fn alpha_explicit_kernarg(
    scale: f32,
    input_pointer: u64,
    input_len: usize,
    output_pointer: u64,
    output_len: usize,
) -> Result<[u8; ALPHA_EXPLICIT_BYTES], LengthMismatch> {
    let length = equal_length("alpha", &[input_len, output_len])?;
    let mut bytes = [0; ALPHA_EXPLICIT_BYTES];
    put_u32(&mut bytes, 0, scale.to_bits());
    put_u64(&mut bytes, 8, input_pointer);
    put_u64(&mut bytes, 16, length as u64);
    put_u64(&mut bytes, 24, output_pointer);
    put_u64(&mut bytes, 32, length as u64);
    Ok(bytes)
}

fn zeta_explicit_kernarg(
    a_pointer: u64,
    a_len: usize,
    b_pointer: u64,
    b_len: usize,
    bias: f32,
    output_pointer: u64,
    output_len: usize,
) -> Result<[u8; ZETA_EXPLICIT_BYTES], LengthMismatch> {
    let length = equal_length("zeta", &[a_len, b_len, output_len])?;
    let mut bytes = [0; ZETA_EXPLICIT_BYTES];
    put_u64(&mut bytes, 0, a_pointer);
    put_u64(&mut bytes, 8, length as u64);
    put_u64(&mut bytes, 16, b_pointer);
    put_u64(&mut bytes, 24, length as u64);
    put_u32(&mut bytes, 32, bias.to_bits());
    put_u64(&mut bytes, 40, output_pointer);
    put_u64(&mut bytes, 48, length as u64);
    Ok(bytes)
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn grid_x(length: usize) -> Result<u32, &'static str> {
    if length == 0 {
        return Err("the hardware vertical slice does not dispatch an empty domain");
    }
    u32::try_from(length.div_ceil(WORKGROUP_SIZE))
        .map_err(|_| "the rounded grid exceeds the gfx942 launch contract")
}

fn alpha_input(length: usize) -> Vec<f32> {
    (0..length)
        .map(|index| ((index % 31) as i32 - 15) as f32 * 0.25)
        .collect()
}

fn zeta_input(length: usize) -> Vec<f32> {
    (0..length)
        .map(|index| ((index % 17) as i32 - 8) as f32 * 0.5)
        .collect()
}

fn alpha_oracle(scale: f32, input: &[f32]) -> Vec<f32> {
    input.iter().map(|value| scale * value).collect()
}

fn zeta_oracle(a: &[f32], b: &[f32], bias: f32) -> Result<Vec<f32>, LengthMismatch> {
    equal_length("zeta", &[a.len(), b.len()])?;
    Ok(a.iter()
        .zip(b)
        .map(|(a_value, b_value)| a_value + b_value + bias)
        .collect())
}

fn guarded(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(GUARD_PREFIX_ELEMENTS + body.len() + GUARD_SUFFIX_ELEMENTS);
    values.extend(std::iter::repeat_n(prefix, GUARD_PREFIX_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, GUARD_SUFFIX_ELEMENTS));
    values
}

fn verify_guarded(
    actual: &[f32],
    expected_body: &[f32],
    prefix: f32,
    suffix: f32,
) -> Result<(), String> {
    let expected_len = GUARD_PREFIX_ELEMENTS + expected_body.len() + GUARD_SUFFIX_ELEMENTS;
    if actual.len() != expected_len {
        return Err(format!(
            "guarded allocation length changed: expected {expected_len}, got {}",
            actual.len()
        ));
    }
    if let Some(index) = actual[..GUARD_PREFIX_ELEMENTS]
        .iter()
        .position(|value| *value != prefix)
    {
        return Err(format!("prefix canary changed at element {index}"));
    }
    let body_end = GUARD_PREFIX_ELEMENTS + expected_body.len();
    if let Some(index) = actual[GUARD_PREFIX_ELEMENTS..body_end]
        .iter()
        .zip(expected_body)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(format!("body differs from CPU oracle at element {index}"));
    }
    if let Some(index) = actual[body_end..].iter().position(|value| *value != suffix) {
        return Err(format!("suffix canary changed at element {index}"));
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
type BoxError = Box<dyn std::error::Error>;

#[cfg(feature = "hardware-test-hooks")]
struct ResolvedKernel<'executable> {
    kernel: &'executable ReviewedHsaKernelV1,
    resolution: &'executable HsaKernelResolutionObservationV1,
}

#[cfg(feature = "hardware-test-hooks")]
struct ResolvedTwoKernels<'executable> {
    alpha: ResolvedKernel<'executable>,
    zeta: ResolvedKernel<'executable>,
}

#[cfg(feature = "hardware-test-hooks")]
struct RuntimeKernarg {
    pointer: std::ptr::NonNull<u8>,
    layout: std::alloc::Layout,
}

#[cfg(feature = "hardware-test-hooks")]
impl RuntimeKernarg {
    fn new(size: u64, alignment: u64) -> Result<Self, BoxError> {
        let layout = std::alloc::Layout::from_size_align(
            usize::try_from(size)?,
            usize::try_from(alignment)?,
        )?;
        // SAFETY: `layout` is valid and this owner deallocates the result once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("failed to allocate runtime-aligned kernarg storage")?;
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
fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn pinned_hsaco() -> Result<(Vec<u8>, fe2o3_artifacts::PayloadDigest), BoxError> {
    require(
        std::env::var("FE2O3_RUN_GFX942_TWO_KERNEL").as_deref() == Ok("1"),
        "set FE2O3_RUN_GFX942_TWO_KERNEL=1 to opt into the alpha/zeta hardware test",
    )?;
    let path = std::env::var_os("FE2O3_GFX942_ALPHA_ZETA_HSACO")
        .ok_or("FE2O3_GFX942_ALPHA_ZETA_HSACO is not set")?;
    let expected_hex = std::env::var("FE2O3_GFX942_ALPHA_ZETA_SHA256")
        .map_err(|_| "FE2O3_GFX942_ALPHA_ZETA_SHA256 is not set")?;
    let expected = parse_sha256(&expected_hex)?;
    let bytes = std::fs::read(path)?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    require(
        digest.bytes().as_bytes() == &expected,
        "FE2O3_GFX942_ALPHA_ZETA_HSACO does not match the pinned SHA-256",
    )?;
    Ok((bytes, digest))
}

#[cfg(feature = "hardware-test-hooks")]
fn parse_sha256(hex: &str) -> Result<[u8; 32], BoxError> {
    require(
        hex.len() == 64,
        "the pinned SHA-256 must contain 64 hex digits",
    )?;
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "the pinned SHA-256 contains a non-hex digit")?;
    }
    Ok(bytes)
}

#[cfg(feature = "hardware-test-hooks")]
fn device_region_pointer(buffer: &DeviceBuffer<f32>, body_len: usize) -> Result<u64, BoxError> {
    require(
        buffer.len() == GUARD_PREFIX_ELEMENTS + body_len + GUARD_SUFFIX_ELEMENTS,
        "guarded device allocation has the wrong extent",
    )?;
    // SAFETY: the checked allocation contains the prefix and full body. The
    // returned device address is never dereferenced by the host and remains
    // borrowed by the synchronous dispatch that consumes the packed bytes.
    let pointer = unsafe { buffer.raw_device_ptr().add(GUARD_PREFIX_ELEMENTS) };
    require(!pointer.is_null(), "non-empty guarded allocation is null")?;
    Ok(u64::try_from(pointer.addr())?)
}

#[cfg(feature = "hardware-test-hooks")]
unsafe fn dispatch_cov6(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    length: usize,
    explicit: &[u8],
) -> Result<(), BoxError> {
    let expected_total = explicit
        .len()
        .checked_add(COV6_IMPLICIT_BYTES)
        .ok_or("COV6 kernarg size overflow")?;
    require(
        resolution.kernarg_segment_size() == expected_total as u64,
        format!(
            "{} exposes {} kernarg bytes, expected {expected_total}",
            resolution.export_symbol(),
            resolution.kernarg_segment_size()
        ),
    )?;
    require(
        resolution.kernarg_segment_alignment() == EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT,
        format!(
            "{} exposes HSA kernarg alignment {}, expected {} from max(COV6 metadata {}, HSA minimum {})",
            resolution.export_symbol(),
            resolution.kernarg_segment_alignment(),
            EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT,
            PHYSICAL_COV6_KERNARG_ALIGNMENT,
            REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT,
        ),
    )?;

    let mut storage = RuntimeKernarg::new(
        resolution.kernarg_segment_size(),
        resolution.kernarg_segment_alignment(),
    )?;
    let kernarg = storage.bytes_mut();
    kernarg[..explicit.len()].copy_from_slice(explicit);
    let geometry = HsaLaunchGeometryV1::new([grid_x(length)?, 1, 1], [256, 1, 1], 0);

    // SAFETY: this feature-gated test is the temporary integration boundary.
    // It packs the frozen alpha/zeta V3 layouts, retains all allocations and
    // exact HSA handles, initializes the complete COV6 hidden span, and waits
    // synchronously. The production generated general-kernel wrapper must
    // replace this raw path before the same operation can be called safe.
    unsafe {
        adapter.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            explicit.len(),
            explicit.len(),
            COV6_IMPLICIT_BYTES,
            kernarg,
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(completion.completed(), "HSA dispatch did not complete")?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_length_case(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    context: &std::sync::Arc<GpuContext>,
    executable: &ReviewedHsaExecutableV1,
    kernels: &ResolvedTwoKernels<'_>,
    length: usize,
) -> Result<(), BoxError> {
    const SCALE: f32 = 1.5;
    const BIAS: f32 = 0.25;

    let stream_context = adapter.environment().physical_device().hip_ordinal();
    require(
        stream_context == 0,
        "hardware fixture requires HIP ordinal 0",
    )?;

    let stream = context.default_stream();
    let input_body = alpha_input(length);
    let b_body = zeta_input(length);
    let expected_alpha = alpha_oracle(SCALE, &input_body);
    let expected_zeta = zeta_oracle(&expected_alpha, &b_body, BIAS)?;
    let input_host = guarded(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
    let b_host = guarded(&b_body, B_PREFIX, B_SUFFIX);
    let alpha_initial = guarded(&vec![OUTPUT_FILL; length], ALPHA_PREFIX, ALPHA_SUFFIX);
    let zeta_initial = guarded(&vec![OUTPUT_FILL; length], ZETA_PREFIX, ZETA_SUFFIX);

    let input = DeviceBuffer::from_host(&stream, &input_host)?;
    let b = DeviceBuffer::from_host(&stream, &b_host)?;
    let alpha_output = DeviceBuffer::from_host(&stream, &alpha_initial)?;
    let zeta_output = DeviceBuffer::from_host(&stream, &zeta_initial)?;

    let input_pointer = device_region_pointer(&input, length)?;
    let b_pointer = device_region_pointer(&b, length)?;
    let alpha_pointer = device_region_pointer(&alpha_output, length)?;
    let zeta_pointer = device_region_pointer(&zeta_output, length)?;
    let alpha_kernarg =
        alpha_explicit_kernarg(SCALE, input_pointer, length, alpha_pointer, length)?;
    let zeta_kernarg = zeta_explicit_kernarg(
        alpha_pointer,
        length,
        b_pointer,
        length,
        BIAS,
        zeta_pointer,
        length,
    )?;

    // SAFETY: `dispatch_cov6` documents the exact temporary raw boundary. Its
    // synchronous completion lets alpha's output become zeta's immutable input
    // without any in-flight aliasing or lifetime overlap.
    unsafe {
        dispatch_cov6(
            adapter,
            executable,
            kernels.alpha.kernel,
            kernels.alpha.resolution,
            length,
            &alpha_kernarg,
        )?;
        dispatch_cov6(
            adapter,
            executable,
            kernels.zeta.kernel,
            kernels.zeta.resolution,
            length,
            &zeta_kernarg,
        )?;
    }

    let input_after = input.to_host_vec(&stream)?;
    let b_after = b.to_host_vec(&stream)?;
    let alpha_after = alpha_output.to_host_vec(&stream)?;
    let zeta_after = zeta_output.to_host_vec(&stream)?;
    require(
        input_after == input_host,
        "alpha input changed during dispatch",
    )?;
    require(b_after == b_host, "zeta input changed during dispatch")?;
    verify_guarded(&alpha_after, &expected_alpha, ALPHA_PREFIX, ALPHA_SUFFIX)
        .map_err(|error| format!("alpha length {length}: {error}"))?;
    verify_guarded(&zeta_after, &expected_zeta, ZETA_PREFIX, ZETA_SUFFIX)
        .map_err(|error| format!("zeta length {length}: {error}"))?;
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn execute_loaded_two_kernel_slice(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    context: &std::sync::Arc<GpuContext>,
    executable: &ReviewedHsaExecutableV1,
    executable_identity: fe2o3_host::HsaExecutableObjectIdentityV1,
) -> Result<(), BoxError> {
    // SAFETY: the pinned fixture contract requires two distinct exports in this
    // exact finalized COV6 image. Typed ABI authority is still established only
    // by this reviewed test harness, not by symbol resolution itself.
    let (kernels, resolutions) =
        unsafe { adapter.resolve_kernel_set(executable, ["alpha", "zeta"]) }?;
    require(kernels.len() == 2, "the resolved kernel set is not exact")?;
    require(
        resolutions[0].export_symbol() == "alpha" && resolutions[1].export_symbol() == "zeta",
        "resolved symbols do not match the alpha/zeta fixture",
    )?;
    require(
        resolutions
            .iter()
            .all(|resolution| resolution.executable_object() == executable_identity),
        "resolved kernels do not belong to the one loaded executable",
    )?;
    require(
        resolutions[0].kernel_object() != resolutions[1].kernel_object(),
        "alpha and zeta resolved to the same native kernel object",
    )?;
    let alpha = kernels.get(0).ok_or("missing alpha kernel")?;
    let zeta = kernels.get(1).ok_or("missing zeta kernel")?;
    let resolved = ResolvedTwoKernels {
        alpha: ResolvedKernel {
            kernel: alpha,
            resolution: &resolutions[0],
        },
        zeta: ResolvedKernel {
            kernel: zeta,
            resolution: &resolutions[1],
        },
    };
    for length in HARDWARE_LENGTHS {
        run_length_case(adapter, context, executable, &resolved, length)?;
    }
    Ok(())
}

/// Executes the first general typed two-kernel gfx942 raw hardware evidence slice.
///
/// This test intentionally calls the reviewed unsafe HSA adapter directly. It
/// does not exercise, provide, or claim production generated safe dispatch.
///
/// Required invocation:
///
/// ```text
/// cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test gfx942_two_kernel_hardware \
///   gfx942_cov6_alpha_then_zeta_one_executable -- --ignored --exact --nocapture
/// ```
///
/// The environment must set `FE2O3_RUN_GFX942_TWO_KERNEL=1`,
/// `FE2O3_GFX942_ALPHA_ZETA_HSACO`, and
/// `FE2O3_GFX942_ALPHA_ZETA_SHA256`.
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires an explicitly pinned alpha/zeta COV6 HSACO and a gfx942:xnack- GPU"]
fn gfx942_cov6_alpha_then_zeta_one_executable() -> Result<(), BoxError> {
    let (bytes, digest) = pinned_hsaco()?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx942",
        "the hardware vertical slice requires gfx942",
    )?;
    require(
        adapter.environment().physical_device().target().xnack() == Some(FeatureState::Disabled),
        "the hardware vertical slice requires gfx942:xnack-",
    )?;

    // SAFETY: bytes are immutable, digest-pinned, and retained through the one
    // terminal consuming unload below.
    let (executable, load) = unsafe { adapter.load_executable(&bytes, digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(load.finalized_digest() == digest, "load digest changed")?;
        require(
            load.byte_len() == bytes.len() as u64,
            "load byte length changed",
        )?;
        execute_loaded_two_kernel_slice(&mut adapter, &context, &executable, executable_identity)
    })();

    // The retained kernel set has been dropped. This is the only consuming
    // unload call in the harness; Rust ownership makes a second unload of this
    // exact executable unrepresentable.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(unload.released(), "the executable was not released")?;
    require(
        unload.executable_object() == executable_identity,
        "unload released a substituted executable",
    )?;
    execution
}

#[test]
fn equal_length_admission_rejects_before_argument_packing() {
    let alpha = alpha_explicit_kernarg(2.0, 0x1000, 255, 0x2000, 256).unwrap_err();
    assert_eq!(
        alpha,
        LengthMismatch {
            kernel: "alpha",
            expected_argument: 0,
            expected: 255,
            actual_argument: 1,
            actual: 256,
        }
    );

    for (lengths, argument, actual) in [([255, 256, 255], 1, 256), ([255, 255, 256], 2, 256)] {
        let error = zeta_explicit_kernarg(
            0x1000, lengths[0], 0x2000, lengths[1], 0.25, 0x3000, lengths[2],
        )
        .unwrap_err();
        assert_eq!(error.actual_argument, argument);
        assert_eq!(error.actual, actual);
    }
}

#[test]
fn alpha_and_zeta_packing_matches_the_frozen_v3_offsets() {
    let alpha = alpha_explicit_kernarg(1.5, 0x1122, 257, 0x3344, 257).unwrap();
    assert_eq!(&alpha[0..4], &1.5_f32.to_bits().to_le_bytes());
    assert_eq!(&alpha[4..8], &[0; 4]);
    assert_eq!(&alpha[8..16], &0x1122_u64.to_le_bytes());
    assert_eq!(&alpha[16..24], &257_u64.to_le_bytes());
    assert_eq!(&alpha[24..32], &0x3344_u64.to_le_bytes());
    assert_eq!(&alpha[32..40], &257_u64.to_le_bytes());

    let zeta = zeta_explicit_kernarg(0x1122, 1023, 0x3344, 1023, 0.25, 0x5566, 1023).unwrap();
    assert_eq!(&zeta[0..8], &0x1122_u64.to_le_bytes());
    assert_eq!(&zeta[8..16], &1023_u64.to_le_bytes());
    assert_eq!(&zeta[16..24], &0x3344_u64.to_le_bytes());
    assert_eq!(&zeta[24..32], &1023_u64.to_le_bytes());
    assert_eq!(&zeta[32..36], &0.25_f32.to_bits().to_le_bytes());
    assert_eq!(&zeta[36..40], &[0; 4]);
    assert_eq!(&zeta[40..48], &0x5566_u64.to_le_bytes());
    assert_eq!(&zeta[48..56], &1023_u64.to_le_bytes());
}

#[test]
fn hsa_resolution_alignment_applies_the_reviewed_runtime_minimum() {
    assert_eq!(PHYSICAL_COV6_KERNARG_ALIGNMENT, 8);
    assert_eq!(REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT, 16);
    assert_eq!(EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT, 16);
    assert_eq!(reviewed_hsa_resolution_alignment(8), 16);
    assert_eq!(reviewed_hsa_resolution_alignment(16), 16);
    assert_eq!(reviewed_hsa_resolution_alignment(32), 32);
}

#[test]
fn boundary_lengths_have_exact_grids_and_cpu_oracles() {
    let expected_grids = [1, 1, 1, 2, 4];
    for (length, expected_grid) in HARDWARE_LENGTHS.into_iter().zip(expected_grids) {
        assert_eq!(grid_x(length), Ok(expected_grid));
        let input = alpha_input(length);
        let b = zeta_input(length);
        let alpha = alpha_oracle(1.5, &input);
        let zeta = zeta_oracle(&alpha, &b, 0.25).unwrap();
        assert_eq!(alpha.len(), length);
        assert_eq!(zeta.len(), length);
        for index in 0..length {
            assert_eq!(alpha[index], 1.5 * input[index]);
            assert_eq!(zeta[index], alpha[index] + b[index] + 0.25);
        }
    }
    assert_eq!(
        grid_x(0),
        Err("the hardware vertical slice does not dispatch an empty domain")
    );
}

#[test]
fn guarded_verification_detects_prefix_body_and_suffix_corruption() {
    let expected = alpha_oracle(1.5, &alpha_input(3));
    let baseline = guarded(&expected, ALPHA_PREFIX, ALPHA_SUFFIX);
    assert!(verify_guarded(&baseline, &expected, ALPHA_PREFIX, ALPHA_SUFFIX).is_ok());

    let mut prefix = baseline.clone();
    prefix[GUARD_PREFIX_ELEMENTS - 1] = 0.0;
    assert!(
        verify_guarded(&prefix, &expected, ALPHA_PREFIX, ALPHA_SUFFIX)
            .unwrap_err()
            .contains("prefix")
    );

    let mut body = baseline.clone();
    body[GUARD_PREFIX_ELEMENTS + 1] = 0.0;
    assert!(
        verify_guarded(&body, &expected, ALPHA_PREFIX, ALPHA_SUFFIX)
            .unwrap_err()
            .contains("CPU oracle")
    );

    let mut suffix = baseline;
    suffix[GUARD_PREFIX_ELEMENTS + expected.len()] = 0.0;
    assert!(
        verify_guarded(&suffix, &expected, ALPHA_PREFIX, ALPHA_SUFFIX)
            .unwrap_err()
            .contains("suffix")
    );
}
