use std::fmt;

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_amd_target::FeatureState;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
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
const COV6_IMPLICIT_BYTES: usize = 256;
const PHYSICAL_COV6_KERNARG_ALIGNMENT: u64 = 8;
const REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT: u64 =
    reviewed_hsa_resolution_alignment(PHYSICAL_COV6_KERNARG_ALIGNMENT);
const ALPHA_EXPLICIT_BYTES: usize = 40;
const GUARD_PREFIX_ELEMENTS: usize = 8;
const GUARD_SUFFIX_ELEMENTS: usize = 11;
#[cfg(feature = "hardware-test-hooks")]
const HARDWARE_LENGTHS: [usize; 5] = [1, 255, 256, 257, 1023];
#[cfg(feature = "hardware-test-hooks")]
const INPUT_PREFIX: f32 = 12_345.0;
#[cfg(feature = "hardware-test-hooks")]
const INPUT_SUFFIX: f32 = -23_456.0;
const OUTPUT_PREFIX: f32 = 56_789.0;
const OUTPUT_SUFFIX: f32 = -67_890.0;
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_FILL: f32 = 9_876.0;
const S09_ARTIFACT_FACTS: &str = concat!(
    "format=fe2o3-s09-artifact-facts-v1\n",
    "object_format=elf64-amdgpu\n",
    "arch=amdgcn\n",
    "target=gfx942:xnack-\n",
    "optimization=O0\n",
    "source_path=crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs\n",
    "kernel=alpha:alpha.kd\n",
);

#[cfg(feature = "hardware-test-hooks")]
type BoxError = Box<dyn std::error::Error>;

const fn reviewed_hsa_resolution_alignment(physical_alignment: u64) -> u64 {
    if physical_alignment > REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT {
        physical_alignment
    } else {
        REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LengthMismatch {
    expected: usize,
    actual: usize,
}

impl fmt::Display for LengthMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "alpha output has length {}, but input has length {}",
            self.actual, self.expected
        )
    }
}

impl std::error::Error for LengthMismatch {}

fn parse_sha256(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("the pinned SHA-256 must be 64 lowercase hex digits".to_owned());
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "the pinned SHA-256 is malformed".to_owned())?;
    }
    Ok(bytes)
}

fn require_declared_digest(actual: [u8; 32], declared: &str) -> Result<(), String> {
    let expected = parse_sha256(declared)?;
    if actual != expected {
        return Err("file does not match its pinned SHA-256".to_owned());
    }
    Ok(())
}

fn validate_artifact_facts(facts: &[u8]) -> Result<(), String> {
    if facts != S09_ARTIFACT_FACTS.as_bytes() {
        return Err(
            "artifact facts are not exact gfx942:xnack- COV6 alpha/alpha.kd facts".to_owned(),
        );
    }
    Ok(())
}

fn alpha_explicit_kernarg(
    scale: f32,
    input_pointer: u64,
    input_len: usize,
    output_pointer: u64,
    output_len: usize,
) -> Result<[u8; ALPHA_EXPLICIT_BYTES], LengthMismatch> {
    if input_len != output_len {
        return Err(LengthMismatch {
            expected: input_len,
            actual: output_len,
        });
    }
    let mut bytes = [0; ALPHA_EXPLICIT_BYTES];
    put_u32(&mut bytes, 0, scale.to_bits());
    put_u64(&mut bytes, 8, input_pointer);
    put_u64(&mut bytes, 16, input_len as u64);
    put_u64(&mut bytes, 24, output_pointer);
    put_u64(&mut bytes, 32, output_len as u64);
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
        return Err("the S09 controller does not dispatch an empty domain");
    }
    u32::try_from(length.div_ceil(WORKGROUP_SIZE))
        .map_err(|_| "the rounded grid exceeds the gfx942 launch contract")
}

fn alpha_input(length: usize) -> Vec<f32> {
    (0..length)
        .map(|index| ((index % 31) as i32 - 15) as f32 * 0.25)
        .collect()
}

fn alpha_oracle(scale: f32, input: &[f32]) -> Vec<f32> {
    input.iter().map(|value| scale * value).collect()
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
fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_file(
    path_key: &str,
    digest_key: &str,
) -> Result<(Vec<u8>, PayloadDigest), BoxError> {
    let path = std::path::PathBuf::from(
        std::env::var_os(path_key).ok_or_else(|| format!("{path_key} is not set"))?,
    );
    require(path.is_absolute(), format!("{path_key} must be absolute"))?;
    let metadata = std::fs::symlink_metadata(&path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        format!("{path_key} must name a regular non-symlink file"),
    )?;
    require(
        std::fs::canonicalize(&path)? == path,
        format!("{path_key} must already be canonical"),
    )?;
    let declared = std::env::var(digest_key).map_err(|_| format!("{digest_key} is not set"))?;
    let bytes = std::fs::read(path)?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    let actual = *digest.bytes().as_bytes();
    require_declared_digest(actual, &declared)?;
    Ok((bytes, digest))
}

#[cfg(feature = "hardware-test-hooks")]
fn pinned_s09_artifact() -> Result<(Vec<u8>, PayloadDigest), BoxError> {
    require(
        std::env::var("FE2O3_RUN_S09_GFX942_ALPHA").as_deref() == Ok("1"),
        "set FE2O3_RUN_S09_GFX942_ALPHA=1 to opt into the S09 alpha hardware controller",
    )?;
    let (bytes, digest) = read_pinned_file(
        "FE2O3_S09_GFX942_ALPHA_HSACO",
        "FE2O3_S09_GFX942_ALPHA_SHA256",
    )?;
    let (facts, _) = read_pinned_file(
        "FE2O3_S09_GFX942_ALPHA_FACTS",
        "FE2O3_S09_GFX942_ALPHA_FACTS_SHA256",
    )?;
    validate_artifact_facts(&facts)?;
    Ok((bytes, digest))
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
fn device_region_pointer(buffer: &DeviceBuffer<f32>, body_len: usize) -> Result<u64, BoxError> {
    require(
        buffer.len() == GUARD_PREFIX_ELEMENTS + body_len + GUARD_SUFFIX_ELEMENTS,
        "guarded device allocation has the wrong extent",
    )?;
    // SAFETY: the allocation contains the checked prefix and complete body.
    let pointer = unsafe { buffer.raw_device_ptr().add(GUARD_PREFIX_ELEMENTS) };
    require(!pointer.is_null(), "non-empty guarded allocation is null")?;
    Ok(u64::try_from(pointer.addr())?)
}

#[cfg(feature = "hardware-test-hooks")]
unsafe fn dispatch_alpha_cov6(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    length: usize,
    explicit: &[u8; ALPHA_EXPLICIT_BYTES],
) -> Result<(), BoxError> {
    let expected_total = ALPHA_EXPLICIT_BYTES + COV6_IMPLICIT_BYTES;
    require(
        resolution.export_symbol() == "alpha",
        "runtime resolution did not bind the exact alpha entry",
    )?;
    require(
        resolution.kernarg_segment_size() == expected_total as u64,
        format!(
            "alpha exposes {} kernarg bytes, expected {expected_total}",
            resolution.kernarg_segment_size()
        ),
    )?;
    require(
        resolution.kernarg_segment_alignment() == EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT,
        format!(
            "alpha exposes HSA kernarg alignment {}, expected {}",
            resolution.kernarg_segment_alignment(),
            EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT
        ),
    )?;

    let mut storage = RuntimeKernarg::new(
        resolution.kernarg_segment_size(),
        resolution.kernarg_segment_alignment(),
    )?;
    let kernarg = storage.bytes_mut();
    kernarg[..ALPHA_EXPLICIT_BYTES].copy_from_slice(explicit);
    let geometry = HsaLaunchGeometryV1::new([grid_x(length)?, 1, 1], [256, 1, 1], 0);

    // SAFETY: the exact digest-pinned alpha-only contract fixes the explicit
    // layout and complete COV6 hidden span; dispatch is synchronous.
    unsafe {
        adapter.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            ALPHA_EXPLICIT_BYTES,
            ALPHA_EXPLICIT_BYTES,
            COV6_IMPLICIT_BYTES,
            kernarg,
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(
            completion.completed(),
            "S09 alpha dispatch did not complete",
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_length_case(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    context: &std::sync::Arc<GpuContext>,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    length: usize,
) -> Result<(), BoxError> {
    const SCALE: f32 = 1.5;

    let stream = context.default_stream();
    let input_body = alpha_input(length);
    let expected_output = alpha_oracle(SCALE, &input_body);
    let input_host = guarded(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
    let output_initial = guarded(&vec![OUTPUT_FILL; length], OUTPUT_PREFIX, OUTPUT_SUFFIX);
    let input = DeviceBuffer::from_host(&stream, &input_host)?;
    let output = DeviceBuffer::from_host(&stream, &output_initial)?;
    let input_pointer = device_region_pointer(&input, length)?;
    let output_pointer = device_region_pointer(&output, length)?;
    let explicit = alpha_explicit_kernarg(SCALE, input_pointer, length, output_pointer, length)?;

    // SAFETY: all allocations and the exact loaded executable outlive this
    // synchronous call, and the explicit layout was checked above.
    unsafe {
        dispatch_alpha_cov6(adapter, executable, kernel, resolution, length, &explicit)?;
    }

    let input_after = input.to_host_vec(&stream)?;
    let output_after = output.to_host_vec(&stream)?;
    require(
        input_after == input_host,
        "S09 alpha input changed during dispatch",
    )?;
    verify_guarded(
        &output_after,
        &expected_output,
        OUTPUT_PREFIX,
        OUTPUT_SUFFIX,
    )
    .map_err(|error| format!("S09 alpha length {length}: {error}"))?;
    Ok(())
}

/// Executes the local capability S09 alpha-only COV6 hardware controller.
///
/// The fixed runner must first derive the exact physical artifact facts from
/// the same digest-pinned HSACO. This test does not authenticate provenance or
/// promote S09 parity; production admission remains outside this process.
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires the exact S09 alpha-only COV6 HSACO and a gfx942:xnack- GPU"]
fn s09_gfx942_cov6_alpha_only_controller() -> Result<(), BoxError> {
    let (bytes, digest) = pinned_s09_artifact()?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx942",
        "the S09 alpha controller requires gfx942",
    )?;
    require(
        adapter.environment().physical_device().target().xnack() == Some(FeatureState::Disabled),
        "the S09 alpha controller requires gfx942:xnack-",
    )?;

    // SAFETY: the immutable bytes are pinned and retained until the one unload.
    let (executable, load) = unsafe { adapter.load_executable(&bytes, digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(load.finalized_digest() == digest, "load digest changed")?;
        require(
            load.byte_len() == bytes.len() as u64,
            "load byte length changed",
        )?;
        // SAFETY: the physical binder facts require exactly alpha/alpha.kd.
        let (kernels, resolutions) = unsafe { adapter.resolve_kernel_set(&executable, ["alpha"]) }?;
        require(
            kernels.len() == 1,
            "runtime did not return one alpha kernel",
        )?;
        require(
            resolutions.len() == 1,
            "runtime did not return one alpha resolution",
        )?;
        require(
            resolutions[0].executable_object() == executable_identity,
            "alpha resolved from a substituted executable",
        )?;
        let kernel = kernels.get(0).ok_or("runtime omitted alpha")?;
        for length in HARDWARE_LENGTHS {
            run_length_case(
                &mut adapter,
                &context,
                &executable,
                kernel,
                &resolutions[0],
                length,
            )?;
        }
        Ok(())
    })();

    // SAFETY: retained kernels were dropped by the completed closure.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(unload.released(), "the S09 executable was not released")?;
    require(
        unload.executable_object() == executable_identity,
        "unload released a substituted executable",
    )?;
    execution
}

#[test]
fn alpha_only_artifact_facts_are_exact_and_closed() {
    validate_artifact_facts(S09_ARTIFACT_FACTS.as_bytes()).unwrap();

    let extra_kernel = format!("{S09_ARTIFACT_FACTS}kernel=zeta:zeta.kd\n");
    assert!(validate_artifact_facts(extra_kernel.as_bytes()).is_err());

    let wrong_symbol = S09_ARTIFACT_FACTS.replace("alpha:alpha.kd", "alpha:wrong.kd");
    assert!(validate_artifact_facts(wrong_symbol.as_bytes()).is_err());
}

#[test]
fn pinned_digest_rejects_substitution_and_noncanonical_text() {
    let actual = [0xab; 32];
    let declared = "ab".repeat(32);
    require_declared_digest(actual, &declared).unwrap();

    assert!(require_declared_digest([0xcd; 32], &declared).is_err());
    assert!(require_declared_digest(actual, &declared.to_uppercase()).is_err());
    assert!(require_declared_digest(actual, &declared[..63]).is_err());
}

#[test]
fn boundary_lengths_fix_grid_packing_and_cpu_oracle() {
    const SCALE: f32 = 1.5;
    assert_eq!(
        grid_x(0),
        Err("the S09 controller does not dispatch an empty domain")
    );
    for (length, expected_grid) in [(1, 1), (255, 1), (256, 1), (257, 2), (1023, 4)] {
        assert_eq!(grid_x(length), Ok(expected_grid));
        let input = alpha_input(length);
        let output = alpha_oracle(SCALE, &input);
        assert_eq!(output.len(), length);
        let packed = alpha_explicit_kernarg(SCALE, 0x1122, length, 0x3344, length).unwrap();
        assert_eq!(&packed[0..4], &SCALE.to_bits().to_le_bytes());
        assert_eq!(&packed[8..16], &0x1122_u64.to_le_bytes());
        assert_eq!(&packed[16..24], &(length as u64).to_le_bytes());
        assert_eq!(&packed[24..32], &0x3344_u64.to_le_bytes());
        assert_eq!(&packed[32..40], &(length as u64).to_le_bytes());
    }
    assert!(alpha_explicit_kernarg(SCALE, 1, 255, 2, 256).is_err());
}

#[test]
fn output_verification_rejects_body_and_both_canary_mutations() {
    let expected = alpha_oracle(1.5, &alpha_input(3));
    let canonical = guarded(&expected, OUTPUT_PREFIX, OUTPUT_SUFFIX);
    verify_guarded(&canonical, &expected, OUTPUT_PREFIX, OUTPUT_SUFFIX).unwrap();

    for index in [0, GUARD_PREFIX_ELEMENTS, canonical.len() - 1] {
        let mut corrupted = canonical.clone();
        corrupted[index] += 1.0;
        assert!(
            verify_guarded(&corrupted, &expected, OUTPUT_PREFIX, OUTPUT_SUFFIX).is_err(),
            "accepted corruption at {index}"
        );
    }
}

#[test]
fn cov6_runtime_shape_is_frozen() {
    assert_eq!(COV6_IMPLICIT_BYTES, 256);
    assert_eq!(ALPHA_EXPLICIT_BYTES + COV6_IMPLICIT_BYTES, 296);
    assert_eq!(PHYSICAL_COV6_KERNARG_ALIGNMENT, 8);
    assert_eq!(EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT, 16);
}
