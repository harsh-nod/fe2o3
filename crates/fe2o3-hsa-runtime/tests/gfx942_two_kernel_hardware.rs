#![cfg(feature = "qualification-oracles-test-only")]

use std::fmt;

#[cfg(feature = "hardware-test-hooks")]
use std::io::{Read, Seek};
#[cfg(feature = "hardware-test-hooks")]
use std::os::fd::{FromRawFd, RawFd};
#[cfg(feature = "hardware-test-hooks")]
use std::os::unix::fs::MetadataExt;

use object::{Object, ObjectSection};
use serde::Deserialize;

use fe2o3_artifacts::DigestAlgorithm;

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_amd_target::FeatureState;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    Dimensions, LaunchContract, Mutability, Name, PointerWidth, ScalarType,
    derive_generated_host_contract_identity_v1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_core::{DeviceBuffer, GpuContext};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_device::KernelMarkerV1;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_host::{
    AlphaZetaCov6DispatchIdentityV1, AlphaZetaCov6KernelRoleV1, AuthenticatedWorkerV2ExecutableV1,
    CompilerGeneratedAlphaZetaCov6ArgumentsV1, CompilerGeneratedArgumentLayoutV1,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
    CompilerGeneratedSemanticWitnessErrorV1, GeneratedAlphaZetaCov6ArgumentBindingV1,
    GeneratedArgumentLayoutError, GeneratedArgumentPackError, GeneratedArgumentPackingPlanV1,
    GeneratedDeviceScalarV1, GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice,
    HsaExecutableObjectIdentityV1, HsaKernelResolutionObservationV1, HsaLaunchGeometryV1,
    LoadedHsaExecutableV1, ObservedContext, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1, ValidatedCompilerGeneratedSemanticWitnessV1,
    WorkerV2PrerequisiteAuthenticatorV1, WorkerV2PrerequisiteDecisionV1,
    WorkerV2PrerequisiteRequestV1, WorkerV2SafetyPropertiesV1, semantic_witness_from_backend_v1,
    validate_compiler_generated_semantic_witness_v1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaHardwareTestBufferV1, ReviewedHsaKernelV1,
    ReviewedHsaRuntimeAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use reserved_fe2o3_symbols::{
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
};
#[cfg(feature = "hardware-test-hooks")]
use std::cell::Cell;
#[cfg(feature = "hardware-test-hooks")]
use std::ffi::{CString, c_char, c_int, c_void};
#[cfg(feature = "hardware-test-hooks")]
use std::fs::OpenOptions;
#[cfg(feature = "hardware-test-hooks")]
use std::io::ErrorKind;

#[cfg(feature = "hardware-test-hooks")]
#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
}

#[cfg(feature = "hardware-test-hooks")]
const RTLD_NOW: c_int = 2;

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
const COMPILER_EVIDENCE_GOLDEN: &str =
    include_str!("../../../tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json");
const COMPILER_EVIDENCE_TOOL_MANIFEST: &str =
    include_str!("../../../tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json");
const COMPILER_EVIDENCE_TRANSITION: &str = include_str!(
    "../../../tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.json"
);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerEvidenceKernelV1 {
    name: String,
    symbol: String,
    kernarg_bytes: u64,
    kernarg_alignment: u64,
    required_workgroup: [u32; 3],
    max_workgroup: u32,
    wavefront: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerEvidenceGoldenV1 {
    schema: String,
    target: String,
    code_object_version: u8,
    rocm_version: String,
    llvm_package_version: String,
    llvm_build_identity: String,
    rust_toolchain: String,
    tool_manifest_path: String,
    tool_manifest_sha256: String,
    worker_protocol: String,
    linker_path: String,
    response_binding: String,
    publication_route: String,
    worker_build_identity: String,
    worker_executable_sha256: String,
    source_path: String,
    source_sha256: String,
    generator_test: String,
    hardware_test: String,
    descriptor_section: String,
    hsaco_sha256: String,
    hsaco_bytes: u64,
    max_hsaco_bytes: u64,
    transition_path: String,
    transition_sha256: String,
    transition_signature_path: String,
    transition_public_key_path: String,
    transition_signature_algorithm: String,
    kernels: Vec<CompilerEvidenceKernelV1>,
    boundary_lengths: Vec<usize>,
}

fn compiler_evidence_golden() -> Result<CompilerEvidenceGoldenV1, String> {
    compiler_evidence_golden_from_str(COMPILER_EVIDENCE_GOLDEN)
}

fn compiler_evidence_golden_from_str(source: &str) -> Result<CompilerEvidenceGoldenV1, String> {
    let golden: CompilerEvidenceGoldenV1 = serde_json::from_str(source)
        .map_err(|error| format!("invalid compiler-evidence golden JSON: {error}"))?;
    if golden.schema != "fe2o3-gfx942-alpha-zeta-compiler-evidence-v1"
        || golden.target != "gfx942:xnack-"
        || golden.code_object_version != 6
        || golden.rocm_version != "7.2.4"
        || golden.llvm_package_version != "22.0.0git"
        || golden.llvm_build_identity != "7.2.4"
        || golden.rust_toolchain != "nightly-2026-04-03"
        || golden.tool_manifest_path != "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
        || golden.tool_manifest_sha256 != sha256_hex(COMPILER_EVIDENCE_TOOL_MANIFEST.as_bytes())
        || golden.worker_protocol != "v2"
        || golden.linker_path != "llvm-lld-library-apis"
        || golden.response_binding != "canonical-request-and-compiler-envelope"
        || golden.publication_route != "raw-inspection-cov6-canonical-finalization"
        || golden.descriptor_section != ".fe2o3.kd.v1"
        || golden.generator_test
            != "worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses"
        || golden.hardware_test != "gfx942_cov6_repository_golden_alpha_then_zeta_one_executable"
        || golden.max_hsaco_bytes != 16 * 1024 * 1024
        || golden.transition_path
            != "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.json"
        || golden.transition_sha256 != sha256_hex(COMPILER_EVIDENCE_TRANSITION.as_bytes())
        || golden.transition_signature_path
            != "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.sig"
        || golden.transition_public_key_path
            != "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.pub"
        || golden.transition_signature_algorithm != "ed25519-sha512"
        || golden.hsaco_bytes == 0
        || golden.hsaco_bytes > golden.max_hsaco_bytes
        || golden.boundary_lengths != HARDWARE_LENGTHS
    {
        return Err("compiler-evidence golden profile changed".to_owned());
    }
    for (label, digest) in [
        ("worker executable", &golden.worker_executable_sha256),
        ("source", &golden.source_sha256),
        ("HSACO", &golden.hsaco_sha256),
    ] {
        parse_sha256(digest).map_err(|error| format!("invalid {label} digest: {error}"))?;
    }
    if !golden
        .worker_build_identity
        .starts_with("fe2o3-worker-v1-sha256-")
        || golden.worker_build_identity.len() != "fe2o3-worker-v1-sha256-".len() + 64
        || golden.source_path
            != "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs"
    {
        return Err("compiler-evidence generator closure changed".to_owned());
    }
    if golden.kernels.len() != 2 {
        return Err("compiler-evidence golden must contain exactly two kernels".to_owned());
    }
    let expected = [("alpha", "alpha.kd", 296, 8), ("zeta", "zeta.kd", 312, 8)];
    for (kernel, (name, symbol, bytes, alignment)) in golden.kernels.iter().zip(expected) {
        if kernel.name != name
            || kernel.symbol != symbol
            || kernel.kernarg_bytes != bytes
            || kernel.kernarg_alignment != alignment
            || kernel.required_workgroup != [256, 1, 1]
            || kernel.max_workgroup != 256
            || kernel.wavefront != 64
        {
            return Err(format!(
                "compiler-evidence kernel contract changed for {name}"
            ));
        }
    }
    Ok(golden)
}

fn sha256_hex(bytes: &[u8]) -> String {
    DigestAlgorithm::Sha256
        .calculate(bytes)
        .bytes()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_repository_golden_hsaco(bytes: &[u8]) -> Result<[u8; 32], String> {
    let golden = compiler_evidence_golden()?;
    if bytes.len() as u64 != golden.hsaco_bytes || bytes.len() as u64 > golden.max_hsaco_bytes {
        return Err("repository golden HSACO has the wrong bounded size".to_owned());
    }
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    let digest_bytes = *digest.bytes().as_bytes();
    if digest_bytes != parse_sha256(&golden.hsaco_sha256)? {
        return Err("repository golden HSACO digest changed".to_owned());
    }

    let physical = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(bytes)
        .map_err(|error| format!("repository golden physical inspection failed: {error}"))?;
    if physical.inspection().target().to_string() != golden.target
        || physical.inspection().code_object_version().number() != golden.code_object_version
        || physical.inspection().kernels().len() != golden.kernels.len()
        || physical.bindings().len() != golden.kernels.len()
    {
        return Err("repository golden target, COV, or kernel closure changed".to_owned());
    }
    for (index, (kernel, expected)) in physical
        .inspection()
        .kernels()
        .iter()
        .zip(&golden.kernels)
        .enumerate()
    {
        if kernel.name() != expected.name
            || kernel.symbol() != expected.symbol
            || kernel.kernarg_segment_size() != expected.kernarg_bytes
            || kernel.kernarg_segment_alignment() != expected.kernarg_alignment
            || kernel.required_workgroup_size() != Some(expected.required_workgroup)
            || kernel.max_flat_workgroup_size() != expected.max_workgroup
            || kernel.wavefront_size() != expected.wavefront
            || physical.bindings()[index].kernel_index() != index
            || physical.bindings()[index].entry_size() == 0
        {
            return Err(format!(
                "repository golden physical contract changed for {}",
                expected.name
            ));
        }
    }

    let object = object::File::parse(bytes)
        .map_err(|error| format!("repository golden ELF parsing failed: {error}"))?;
    let descriptor = object
        .section_by_name(&golden.descriptor_section)
        .ok_or_else(|| "repository golden canonical descriptor section is absent".to_owned())?;
    if descriptor
        .data()
        .map_err(|error| format!("repository golden descriptor read failed: {error}"))?
        .is_empty()
    {
        return Err("repository golden canonical descriptor section is empty".to_owned());
    }
    Ok(digest_bytes)
}

#[cfg(feature = "hardware-test-hooks")]
const GENERATED_SAFE_TEST_SEED: u8 = 0xa7;
#[cfg(feature = "hardware-test-hooks")]
const ALPHA_TEST_BINDING: [u8; 32] = [0x61; 32];
#[cfg(feature = "hardware-test-hooks")]
const ZETA_TEST_BINDING: [u8; 32] = [0x7a; 32];
#[cfg(feature = "hardware-test-hooks")]
const ALPHA_TEST_HOST_CONTRACT: [u8; 32] = [
    149, 219, 170, 144, 118, 68, 97, 9, 43, 235, 107, 123, 185, 90, 192, 247, 80, 112, 25, 186,
    186, 157, 128, 188, 5, 15, 155, 59, 206, 210, 56, 199,
];
#[cfg(feature = "hardware-test-hooks")]
const ZETA_TEST_HOST_CONTRACT: [u8; 32] = [
    246, 186, 214, 113, 9, 38, 46, 43, 129, 202, 66, 224, 213, 242, 145, 196, 184, 137, 97, 101,
    58, 160, 169, 160, 136, 228, 129, 211, 20, 61, 128, 197,
];

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
fn alpha_generated_test_kernel() {}

#[cfg(feature = "hardware-test-hooks")]
fn zeta_generated_test_kernel() {}

#[cfg(feature = "hardware-test-hooks")]
struct AlphaGeneratedSafeTestKernel;

#[cfg(feature = "hardware-test-hooks")]
struct ZetaGeneratedSafeTestKernel;

// SAFETY: this feature-gated marker is an exact test expectation for the
// fixture's `alpha` role. It is not a production-generated marker.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl KernelMarkerV1 for AlphaGeneratedSafeTestKernel {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "alpha";
    const EXPORT_NAME: &'static str = "alpha";
    const FUNCTION: Self::Function = alpha_generated_test_kernel;
    const REGISTRATION: &'static Self::Registration = &();
}

// SAFETY: this feature-gated marker is an exact test expectation for the
// fixture's `zeta` role. It is not a production-generated marker.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl KernelMarkerV1 for ZetaGeneratedSafeTestKernel {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "zeta";
    const EXPORT_NAME: &'static str = "zeta";
    const FUNCTION: Self::Function = zeta_generated_test_kernel;
    const REGISTRATION: &'static Self::Registration = &();
}

#[cfg(feature = "hardware-test-hooks")]
fn backend_semantic_witness_fixture_bytes(
    kernel_binding: [u8; 32],
    generated_host_contract: [u8; 32],
) -> Vec<u8> {
    let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
    let byte_len = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();
    let mut bytes = Vec::with_capacity(byte_len);
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(byte_len)
            .expect("witness length fits u32")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&kernel_binding);
    bytes.extend_from_slice(&generated_host_contract);
    bytes.extend_from_slice(
        &u16::try_from(profile.len())
            .expect("profile length fits u16")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(profile);
    bytes
}

#[cfg(feature = "hardware-test-hooks")]
fn parse_test_backend_semantic_witness(
    kernel_binding: [u8; 32],
    generated_host_contract: [u8; 32],
) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    let bytes = backend_semantic_witness_fixture_bytes(kernel_binding, generated_host_contract);
    // SAFETY: this test-only immutable vector is initialized and retained for
    // the complete parser call. The parser checks both expected identities.
    unsafe {
        semantic_witness_from_backend_v1(
            bytes.as_ptr(),
            bytes.len(),
            kernel_binding,
            generated_host_contract,
        )
    }
}

// SAFETY: the profile, binding, and test semantic-witness bytes describe the
// exact alpha marker expected by this ignored integration harness. This is an
// explicit test trust boundary, not production backend authentication.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl CompilerGeneratedKernelExpectationV1 for AlphaGeneratedSafeTestKernel {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: ALPHA_TEST_HOST_CONTRACT,
        };
    const KERNEL_BINDING_ID_V1: [u8; 32] = ALPHA_TEST_BINDING;

    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        parse_test_backend_semantic_witness(ALPHA_TEST_BINDING, ALPHA_TEST_HOST_CONTRACT)
    }
}

// SAFETY: the profile, binding, and test semantic-witness bytes describe the
// exact zeta marker expected by this ignored integration harness. This is an
// explicit test trust boundary, not production backend authentication.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl CompilerGeneratedKernelExpectationV1 for ZetaGeneratedSafeTestKernel {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: ZETA_TEST_HOST_CONTRACT,
        };
    const KERNEL_BINDING_ID_V1: [u8; 32] = ZETA_TEST_BINDING;

    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        parse_test_backend_semantic_witness(ZETA_TEST_BINDING, ZETA_TEST_HOST_CONTRACT)
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn generated_scalar_field(name: &str, offset: u64) -> AbiField {
    AbiField::new(
        Name::new(name).expect("generated scalar name is canonical"),
        offset,
        4,
        4,
        AbiKind::Scalar(ScalarType::F32),
        Mutability::Immutable,
        Access::ByValue,
        AddressSpace::Value,
        <f32 as GeneratedDeviceScalarV1>::scalar_type_identity_v1(PointerWidth::Bits64),
        ArgumentOwnership::ByValue,
        AliasClass::Value,
    )
    .expect("generated f32 scalar field is canonical")
}

#[cfg(feature = "hardware-test-hooks")]
fn generated_slice_field(name: &str, offset: u64, read_write: bool) -> AbiField {
    AbiField::new(
        Name::new(name).expect("generated slice name is canonical"),
        offset,
        16,
        8,
        AbiKind::Slice {
            element_size: 4,
            element_alignment: 4,
        },
        if read_write {
            Mutability::Mutable
        } else {
            Mutability::Immutable
        },
        if read_write {
            Access::ReadWrite
        } else {
            Access::ReadOnly
        },
        AddressSpace::Global,
        if read_write {
            <f32 as GeneratedDeviceScalarV1>::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
        } else {
            <f32 as GeneratedDeviceScalarV1>::shared_slice_type_identity_v1(PointerWidth::Bits64)
        },
        if read_write {
            ArgumentOwnership::UniqueBorrow
        } else {
            ArgumentOwnership::SharedBorrow
        },
        if read_write {
            AliasClass::Exclusive
        } else {
            AliasClass::SharedReadOnly
        },
    )
    .expect("generated f32 slice field is canonical")
}

#[cfg(feature = "hardware-test-hooks")]
fn alpha_generated_fields() -> Vec<AbiField> {
    vec![
        generated_scalar_field("scale", 0),
        generated_slice_field("input", 8, false),
        generated_slice_field("output", 24, true),
    ]
}

#[cfg(feature = "hardware-test-hooks")]
fn zeta_generated_fields() -> Vec<AbiField> {
    vec![
        generated_slice_field("a", 0, false),
        generated_slice_field("b", 16, false),
        generated_scalar_field("bias", 32),
        generated_slice_field("output", 40, true),
    ]
}

#[cfg(feature = "hardware-test-hooks")]
fn alpha_generated_abi() -> AbiLayout {
    AbiLayout::new(
        ALPHA_EXPLICIT_BYTES as u64,
        8,
        PointerWidth::Bits64,
        alpha_generated_fields(),
    )
    .expect("generated alpha ABI is canonical")
}

#[cfg(feature = "hardware-test-hooks")]
fn zeta_generated_abi() -> AbiLayout {
    AbiLayout::new(
        ZETA_EXPLICIT_BYTES as u64,
        8,
        PointerWidth::Bits64,
        zeta_generated_fields(),
    )
    .expect("generated zeta ABI is canonical")
}

#[cfg(feature = "hardware-test-hooks")]
fn alpha_zeta_generated_launch() -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(WORKGROUP_SIZE as u32, 1, 1)
                .expect("generated alpha/zeta block dimensions are canonical"),
        ),
        Dimensions::new(u32::MAX, 1, 1)
            .expect("generated alpha/zeta grid dimensions are canonical"),
        0,
        0,
    )
    .expect("generated alpha/zeta launch is canonical")
}

#[cfg(feature = "hardware-test-hooks")]
fn alpha_generated_layout()
-> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
    CompilerGeneratedArgumentLayoutV1::new(
        ALPHA_EXPLICIT_BYTES as u64,
        8,
        PointerWidth::Bits64,
        alpha_generated_fields(),
    )
}

#[cfg(feature = "hardware-test-hooks")]
fn zeta_generated_layout() -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>
{
    CompilerGeneratedArgumentLayoutV1::new(
        ZETA_EXPLICIT_BYTES as u64,
        8,
        PointerWidth::Bits64,
        zeta_generated_fields(),
    )
}

#[cfg(feature = "hardware-test-hooks")]
struct AlphaGeneratedArguments<'allocation> {
    scale: f32,
    input: GeneratedReadDeviceSlice<'allocation, f32>,
    output: GeneratedReadWriteDeviceSlice<'allocation, f32>,
    bound: Cell<bool>,
}

#[cfg(feature = "hardware-test-hooks")]
impl<'allocation> AlphaGeneratedArguments<'allocation> {
    fn new(
        scale: f32,
        input: GeneratedReadDeviceSlice<'allocation, f32>,
        output: GeneratedReadWriteDeviceSlice<'allocation, f32>,
    ) -> Self {
        Self {
            scale,
            input,
            output,
            bound: Cell::new(false),
        }
    }
}

// SAFETY: this non-clone owner binds the exact alpha layout and retains both
// checked generated slice capabilities until synchronous dispatch completes.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl<'allocation>
    CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, AlphaGeneratedSafeTestKernel>
    for AlphaGeneratedArguments<'allocation>
{
    fn dispatch_identity_v1() -> AlphaZetaCov6DispatchIdentityV1 {
        AlphaZetaCov6DispatchIdentityV1::new(
            AlphaZetaCov6KernelRoleV1::Alpha,
            ALPHA_TEST_BINDING,
            ALPHA_TEST_HOST_CONTRACT,
        )
    }

    fn generated_argument_layout_v1()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        alpha_generated_layout()
    }

    fn bind_arguments_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation>, GeneratedArgumentPackError>
    {
        assert!(!self.bound.replace(true), "alpha arguments bound twice");
        let slices = vec![
            self.input.bind_argument_pair(plan, 1)?,
            self.output.bind_argument_pair(plan, 2)?,
        ];
        // SAFETY: the scalar and opaque slice pairs cover alpha's exact source
        // argument order, and this owner retains their capabilities.
        Ok(unsafe {
            GeneratedAlphaZetaCov6ArgumentBindingV1::from_compiler_generated_parts_v1(
                vec![plan.scalar(0, self.scale)?],
                slices,
            )
        })
    }
}

#[cfg(feature = "hardware-test-hooks")]
struct ZetaGeneratedArguments<'allocation> {
    a: GeneratedReadDeviceSlice<'allocation, f32>,
    b: GeneratedReadDeviceSlice<'allocation, f32>,
    bias: f32,
    output: GeneratedReadWriteDeviceSlice<'allocation, f32>,
    bound: Cell<bool>,
}

#[cfg(feature = "hardware-test-hooks")]
impl<'allocation> ZetaGeneratedArguments<'allocation> {
    fn new(
        a: GeneratedReadDeviceSlice<'allocation, f32>,
        b: GeneratedReadDeviceSlice<'allocation, f32>,
        bias: f32,
        output: GeneratedReadWriteDeviceSlice<'allocation, f32>,
    ) -> Self {
        Self {
            a,
            b,
            bias,
            output,
            bound: Cell::new(false),
        }
    }
}

// SAFETY: this non-clone owner binds the exact zeta layout and retains all
// checked generated slice capabilities until synchronous dispatch completes.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl<'allocation>
    CompilerGeneratedAlphaZetaCov6ArgumentsV1<'allocation, ZetaGeneratedSafeTestKernel>
    for ZetaGeneratedArguments<'allocation>
{
    fn dispatch_identity_v1() -> AlphaZetaCov6DispatchIdentityV1 {
        AlphaZetaCov6DispatchIdentityV1::new(
            AlphaZetaCov6KernelRoleV1::Zeta,
            ZETA_TEST_BINDING,
            ZETA_TEST_HOST_CONTRACT,
        )
    }

    fn generated_argument_layout_v1()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        zeta_generated_layout()
    }

    fn bind_arguments_v1(
        &self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedAlphaZetaCov6ArgumentBindingV1<'allocation>, GeneratedArgumentPackError>
    {
        assert!(!self.bound.replace(true), "zeta arguments bound twice");
        let slices = vec![
            self.a.bind_argument_pair(plan, 0)?,
            self.b.bind_argument_pair(plan, 1)?,
            self.output.bind_argument_pair(plan, 3)?,
        ];
        // SAFETY: the scalar and opaque slice pairs cover zeta's exact source
        // argument order, and this owner retains their capabilities.
        Ok(unsafe {
            GeneratedAlphaZetaCov6ArgumentBindingV1::from_compiler_generated_parts_v1(
                vec![plan.scalar(2, self.bias)?],
                slices,
            )
        })
    }
}

/// Explicitly fake prerequisite authentication for this ignored hardware test.
///
/// It echoes the admitted request and manufactures nonzero measurement digests.
/// It therefore exercises lifecycle validation but provides no production
/// compiler, Verus, proof-to-executable, layout, or effect authentication.
#[cfg(feature = "hardware-test-hooks")]
struct ExplicitlyFakePrerequisiteAuthenticator;

// SAFETY: this implementation is deliberately valid only as a feature-gated
// test fixture. The ignored test name and documentation expose that no real
// prerequisite authentication is established.
#[cfg(feature = "hardware-test-hooks")]
unsafe impl<K: CompilerGeneratedKernelExpectationV1> WorkerV2PrerequisiteAuthenticatorV1<K>
    for ExplicitlyFakePrerequisiteAuthenticator
{
    type Error = core::convert::Infallible;

    unsafe fn authenticate(
        &mut self,
        request: &WorkerV2PrerequisiteRequestV1<'_, K>,
    ) -> Result<WorkerV2PrerequisiteDecisionV1, Self::Error> {
        let artifact = request.artifact_identity();
        Ok(WorkerV2PrerequisiteDecisionV1::new(
            request.challenge_identity().clone(),
            request.finalized_digest(),
            artifact.kernel_id(),
            artifact.executable_digest(),
            request.target(),
            request.code_object_version(),
            artifact.name().as_str(),
            artifact.symbol().as_str(),
            artifact.abi().clone(),
            artifact.launch().clone(),
            request.marker_binding_identity(),
            DigestAlgorithm::Sha256.calculate(b"fake-hardware-test-compiler"),
            DigestAlgorithm::Sha256.calculate(b"fake-hardware-test-verus"),
            DigestAlgorithm::Sha256.calculate(b"fake-hardware-test-proof-executable"),
            DigestAlgorithm::Sha256.calculate(b"fake-hardware-test-rust-layout"),
            DigestAlgorithm::Sha256.calculate(b"fake-hardware-test-rust-effects"),
            WorkerV2SafetyPropertiesV1::required(),
        ))
    }
}

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
fn verify_compiler_evidence_device_sandbox() -> Result<(), BoxError> {
    require(
        std::env::var("FE2O3_VERIFY_DEVICE_LANDLOCK").as_deref() == Ok("1"),
        "compiler evidence did not enable the device Landlock fixture",
    )?;
    let fixture = std::env::var("FE2O3_DEVICE_LANDLOCK_SHM_FIXTURE")?;
    let create_path = std::env::var("FE2O3_DEVICE_LANDLOCK_SHM_CREATE")?;
    require(
        fixture.starts_with("/dev/shm/") && create_path.starts_with("/dev/shm/"),
        "device Landlock fixtures escaped /dev/shm",
    )?;

    let read_error = std::fs::read(&fixture).expect_err("Landlock allowed /dev/shm read");
    require(
        read_error.kind() == ErrorKind::PermissionDenied,
        format!("/dev/shm read did not fail with EACCES: {read_error}"),
    )?;
    let create_error = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&create_path)
        .expect_err("Landlock allowed /dev/shm create");
    require(
        create_error.kind() == ErrorKind::PermissionDenied,
        format!("/dev/shm create did not fail with EACCES: {create_error}"),
    )?;

    let name = CString::new(fixture)?;
    // SAFETY: `name` is a live NUL-terminated path. The expected null result
    // creates no handle and therefore needs no matching `dlclose`.
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
    require(handle.is_null(), "Landlock allowed dlopen from /dev/shm")?;
    println!("compiler-evidence /dev/shm create/read/dlopen denial: PASS");
    Ok(())
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
fn pinned_repository_golden_hsaco() -> Result<(Vec<u8>, fe2o3_artifacts::PayloadDigest), BoxError> {
    require(
        std::env::var("FE2O3_RUN_GFX942_TWO_KERNEL").as_deref() == Ok("1"),
        "set FE2O3_RUN_GFX942_TWO_KERNEL=1 to opt into the alpha/zeta hardware test",
    )?;
    let golden = compiler_evidence_golden()?;
    let declared = std::env::var("FE2O3_GFX942_ALPHA_ZETA_SHA256")
        .map_err(|_| "FE2O3_GFX942_ALPHA_ZETA_SHA256 is not set")?;
    require(
        declared == golden.hsaco_sha256,
        "the configured HSACO digest is not the repository compiler-evidence golden",
    )?;
    let path = std::path::PathBuf::from(
        std::env::var_os("FE2O3_GFX942_ALPHA_ZETA_HSACO")
            .ok_or("FE2O3_GFX942_ALPHA_ZETA_HSACO is not set")?,
    );
    require(
        path.is_absolute(),
        "the repository golden HSACO path must be absolute",
    )?;
    let metadata = std::fs::symlink_metadata(&path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "the repository golden HSACO must be a regular non-symlink file",
    )?;
    require(
        std::fs::canonicalize(&path)? == path,
        "the repository golden HSACO path must already be canonical",
    )?;
    require(
        metadata.len() == golden.hsaco_bytes && metadata.len() <= golden.max_hsaco_bytes,
        "the repository golden HSACO has the wrong bounded file size",
    )?;
    let path_bytes = std::fs::read(&path)?;
    let retained_fd: RawFd = std::env::var("FE2O3_GFX942_ALPHA_ZETA_RETAINED_FD")
        .map_err(|_| "FE2O3_GFX942_ALPHA_ZETA_RETAINED_FD is not set")?
        .parse()
        .map_err(|_| "FE2O3_GFX942_ALPHA_ZETA_RETAINED_FD is not a file descriptor")?;
    require(retained_fd >= 3, "the retained HSACO descriptor is invalid")?;
    // SAFETY: the evidence controller transfers ownership of this inherited
    // descriptor to the hardware-test child for exactly this consumption.
    let mut retained = unsafe { std::fs::File::from_raw_fd(retained_fd) };
    let retained_metadata = retained.metadata()?;
    require(
        retained_metadata.dev() == metadata.dev() && retained_metadata.ino() == metadata.ino(),
        "the retained HSACO descriptor and direct dirent differ",
    )?;
    retained.rewind()?;
    let mut bytes = Vec::new();
    retained.read_to_end(&mut bytes)?;
    require(
        bytes == path_bytes,
        "the retained HSACO descriptor and direct dirent bytes differ",
    )?;
    let digest_bytes = validate_repository_golden_hsaco(&bytes)?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    require(
        digest.bytes().as_bytes() == &digest_bytes,
        "the repository golden digest changed after structured validation",
    )?;
    Ok((bytes, digest))
}

fn parse_sha256(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("the pinned SHA-256 must contain exactly 64 hex digits".to_owned());
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "the pinned SHA-256 contains a non-hex digit".to_owned())?;
    }
    Ok(bytes)
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: `f32` has no invalid bit patterns and the byte extent is checked.
    unsafe {
        core::slice::from_raw_parts(values.as_ptr().cast::<u8>(), core::mem::size_of_val(values))
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn f32_values(bytes: &[u8]) -> Result<Vec<f32>, BoxError> {
    require(
        bytes.len().is_multiple_of(core::mem::size_of::<f32>()),
        "hardware-test HSA buffer has a partial f32",
    )?;
    Ok(bytes
        .chunks_exact(core::mem::size_of::<f32>())
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().expect("exact f32 chunk")))
        .collect())
}

#[cfg(feature = "hardware-test-hooks")]
fn hsa_region_pointer(
    buffer: &ReviewedHsaHardwareTestBufferV1,
    body_len: usize,
) -> Result<u64, BoxError> {
    let element_count = GUARD_PREFIX_ELEMENTS
        .checked_add(body_len)
        .and_then(|length| length.checked_add(GUARD_SUFFIX_ELEMENTS))
        .ok_or("guarded HSA allocation extent overflow")?;
    require(
        buffer.byte_len() == element_count * core::mem::size_of::<f32>(),
        "guarded HSA allocation has the wrong extent",
    )?;
    Ok(buffer.device_address(GUARD_PREFIX_ELEMENTS * core::mem::size_of::<f32>())?)
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

    let input_body = alpha_input(length);
    let b_body = zeta_input(length);
    let expected_alpha = alpha_oracle(SCALE, &input_body);
    let expected_zeta = zeta_oracle(&expected_alpha, &b_body, BIAS)?;
    let input_host = guarded(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
    let b_host = guarded(&b_body, B_PREFIX, B_SUFFIX);
    let alpha_initial = guarded(&vec![OUTPUT_FILL; length], ALPHA_PREFIX, ALPHA_SUFFIX);
    let zeta_initial = guarded(&vec![OUTPUT_FILL; length], ZETA_PREFIX, ZETA_SUFFIX);

    let input = adapter.allocate_hardware_test_buffer(f32_bytes(&input_host))?;
    let b = adapter.allocate_hardware_test_buffer(f32_bytes(&b_host))?;
    let alpha_output = adapter.allocate_hardware_test_buffer(f32_bytes(&alpha_initial))?;
    let zeta_output = adapter.allocate_hardware_test_buffer(f32_bytes(&zeta_initial))?;

    let input_pointer = hsa_region_pointer(&input, length)?;
    let b_pointer = hsa_region_pointer(&b, length)?;
    let alpha_pointer = hsa_region_pointer(&alpha_output, length)?;
    let zeta_pointer = hsa_region_pointer(&zeta_output, length)?;
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

    let input_after = f32_values(&input.read_after_synchronous_dispatch())?;
    let b_after = f32_values(&b.read_after_synchronous_dispatch())?;
    let alpha_after = f32_values(&alpha_output.read_after_synchronous_dispatch())?;
    let zeta_after = f32_values(&zeta_output.read_after_synchronous_dispatch())?;
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
        run_length_case(adapter, executable, &resolved, length)?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_generated_safe_length_case(
    loaded: &mut LoadedHsaExecutableV1<AlphaGeneratedSafeTestKernel, ReviewedHsaRuntimeAdapterV1>,
    context: &std::sync::Arc<GpuContext>,
    observed: &ObservedContext,
    executable_identity: HsaExecutableObjectIdentityV1,
    length: usize,
) -> Result<(), BoxError> {
    const SCALE: f32 = 1.5;
    const BIAS: f32 = 0.25;

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
    let mut alpha_output = DeviceBuffer::from_host(&stream, &alpha_initial)?;
    let mut zeta_output = DeviceBuffer::from_host(&stream, &zeta_initial)?;
    let body_end = GUARD_PREFIX_ELEMENTS
        .checked_add(length)
        .ok_or("guarded body range overflowed")?;

    let alpha_completion = {
        let arguments = AlphaGeneratedArguments::new(
            SCALE,
            GeneratedReadDeviceSlice::from_view(
                observed,
                input.view(GUARD_PREFIX_ELEMENTS..body_end)?,
            )?,
            GeneratedReadWriteDeviceSlice::from_view_mut(
                observed,
                alpha_output.view_mut(GUARD_PREFIX_ELEMENTS..body_end)?,
            )?,
        );
        let mut fake_authenticator = ExplicitlyFakePrerequisiteAuthenticator;
        let prepared = loaded
            .prepare_generated_alpha_zeta_cov6_selected_kernel_v1::<
                AlphaGeneratedSafeTestKernel,
                _,
                _,
            >(observed, &mut fake_authenticator, arguments)?;
        require(
            prepared.geometry()
                == HsaLaunchGeometryV1::new([grid_x(length)?, 1, 1], [256, 1, 1], 0),
            "generated alpha geometry changed",
        )?;
        require(
            prepared.explicit_byte_len() == ALPHA_EXPLICIT_BYTES
                && prepared.physical_kernarg_byte_len()
                    == ALPHA_EXPLICIT_BYTES + COV6_IMPLICIT_BYTES
                && prepared.physical_kernarg_alignment()
                    == EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT as usize,
            "generated alpha COV6 kernarg facts changed",
        )?;
        prepared.dispatch()?
    };
    require(
        alpha_completion.artifact_identity().name().as_str() == "alpha",
        "generated alpha completion changed role",
    )?;
    require(
        alpha_completion.completed_dispatch().dispatch().completed(),
        "generated alpha dispatch did not complete",
    )?;
    require(
        alpha_completion.completed_dispatch().executable_object() == executable_identity,
        "generated alpha completion changed executable",
    )?;

    let zeta_completion = {
        // Alpha completion above released its exclusive owner before this
        // immutable capability for the same checked body can be constructed.
        let arguments = ZetaGeneratedArguments::new(
            GeneratedReadDeviceSlice::from_view(
                observed,
                alpha_output.view(GUARD_PREFIX_ELEMENTS..body_end)?,
            )?,
            GeneratedReadDeviceSlice::from_view(
                observed,
                b.view(GUARD_PREFIX_ELEMENTS..body_end)?,
            )?,
            BIAS,
            GeneratedReadWriteDeviceSlice::from_view_mut(
                observed,
                zeta_output.view_mut(GUARD_PREFIX_ELEMENTS..body_end)?,
            )?,
        );
        let mut fake_authenticator = ExplicitlyFakePrerequisiteAuthenticator;
        let prepared = loaded
            .prepare_generated_alpha_zeta_cov6_selected_kernel_v1::<
                ZetaGeneratedSafeTestKernel,
                _,
                _,
            >(observed, &mut fake_authenticator, arguments)?;
        require(
            prepared.geometry()
                == HsaLaunchGeometryV1::new([grid_x(length)?, 1, 1], [256, 1, 1], 0),
            "generated zeta geometry changed",
        )?;
        require(
            prepared.explicit_byte_len() == ZETA_EXPLICIT_BYTES
                && prepared.physical_kernarg_byte_len()
                    == ZETA_EXPLICIT_BYTES + COV6_IMPLICIT_BYTES
                && prepared.physical_kernarg_alignment()
                    == EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT as usize,
            "generated zeta COV6 kernarg facts changed",
        )?;
        prepared.dispatch()?
    };
    require(
        zeta_completion.artifact_identity().name().as_str() == "zeta",
        "generated zeta completion changed role",
    )?;
    require(
        zeta_completion.completed_dispatch().dispatch().completed(),
        "generated zeta dispatch did not complete",
    )?;
    require(
        zeta_completion.completed_dispatch().executable_object() == executable_identity,
        "generated zeta completion changed executable",
    )?;

    let input_after = input.to_host_vec(&stream)?;
    let b_after = b.to_host_vec(&stream)?;
    let alpha_after = alpha_output.to_host_vec(&stream)?;
    let zeta_after = zeta_output.to_host_vec(&stream)?;
    require(
        input_after == input_host,
        "generated-safe alpha input changed during dispatch",
    )?;
    require(
        b_after == b_host,
        "generated-safe zeta input changed during dispatch",
    )?;
    verify_guarded(&alpha_after, &expected_alpha, ALPHA_PREFIX, ALPHA_SUFFIX)
        .map_err(|error| format!("generated-safe alpha length {length}: {error}"))?;
    verify_guarded(&zeta_after, &expected_zeta, ZETA_PREFIX, ZETA_SUFFIX)
        .map_err(|error| format!("generated-safe zeta length {length}: {error}"))?;
    Ok(())
}

/// Exercises the generated safe alpha/zeta SPI through the reviewed lifecycle.
///
/// The marker semantic witnesses and prerequisite authenticator are explicit
/// test fixtures. This test provides no production proof-authentication claim.
/// Unlike `gfx942_cov6_alpha_then_zeta_one_executable`, it does not manually
/// pack arguments or call the unsafe runtime dispatch API.
///
/// Required invocation:
///
/// ```text
/// cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test gfx942_two_kernel_hardware \
///   gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator \
///   -- --ignored --exact --nocapture
/// ```
///
/// The environment must set `FE2O3_RUN_GFX942_TWO_KERNEL=1`,
/// `FE2O3_GFX942_ALPHA_ZETA_HSACO`, and
/// `FE2O3_GFX942_ALPHA_ZETA_SHA256`.
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires a pinned alpha/zeta COV6 HSACO, gfx942:xnack-, and uses a fake prerequisite authenticator"]
fn gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator() -> Result<(), BoxError>
{
    let (bytes, digest) = pinned_hsaco()?;
    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx942",
        "the generated-safe hardware slice requires gfx942",
    )?;
    require(
        adapter.environment().physical_device().target().xnack() == Some(FeatureState::Disabled),
        "the generated-safe hardware slice requires gfx942:xnack-",
    )?;

    let (admission, publication_directory) =
        fe2o3_host::__hardware_test::admitted_alpha_zeta_cov6_hardware_for_lifecycle_test(
            GENERATED_SAFE_TEST_SEED,
            bytes,
            ALPHA_TEST_BINDING,
            ZETA_TEST_BINDING,
            &observed,
        );
    let mut fake_authenticator = ExplicitlyFakePrerequisiteAuthenticator;
    let authenticated =
        AuthenticatedWorkerV2ExecutableV1::<AlphaGeneratedSafeTestKernel>::authenticate(
            admission,
            &mut fake_authenticator,
        )
        .map_err(|error| format!("fake prerequisite authentication failed: {error:?}"))?;
    let authorized = authenticated
        .authorize_hsa_load(adapter)
        .map_err(|error| format!("reviewed HSA load authorization failed: {error:?}"))?;
    let mut loaded = authorized
        .load()
        .map_err(|error| format!("reviewed HSA executable load failed: {error:?}"))?;
    require(
        loaded.load_observation().finalized_digest() == digest,
        "generated-safe load changed the finalized digest",
    )?;
    let executable_identity = loaded.load_observation().executable_object();

    for length in HARDWARE_LENGTHS {
        run_generated_safe_length_case(
            &mut loaded,
            &context,
            &observed,
            executable_identity,
            length,
        )?;
    }

    let unloaded = loaded
        .unload()
        .map_err(|error| format!("reviewed HSA executable unload failed: {error:?}"))?;
    require(
        unloaded.finalized_digest() == digest,
        "generated-safe unload changed the finalized digest",
    )?;
    require(
        unloaded.executable_object() == executable_identity,
        "generated-safe unload released a substituted executable",
    )?;
    require(
        unloaded.unload_observation().released(),
        "generated-safe unload did not release the executable",
    )?;
    drop(publication_directory);
    Ok(())
}

/// Runs real alpha/zeta dispatch while generation N+1 waits for the retained publication lock.
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires a pinned alpha/zeta COV6 HSACO, gfx942:xnack-, and uses a fake prerequisite authenticator"]
fn gfx942_alpha_zeta_dispatch_retains_currentness_through_unload() -> Result<(), BoxError> {
    let (bytes, _) = pinned_hsaco()?;
    let context = GpuContext::new(0)?;
    let observed = ObservedContext::observe(&context)?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx942"
            && adapter.environment().physical_device().target().xnack()
                == Some(FeatureState::Disabled),
        "the turnover hardware slice requires gfx942:xnack-",
    )?;

    let (admission, publication_directory) =
        fe2o3_host::__hardware_test::admitted_alpha_zeta_cov6_hardware_for_lifecycle_test(
            GENERATED_SAFE_TEST_SEED,
            bytes,
            ALPHA_TEST_BINDING,
            ZETA_TEST_BINDING,
            &observed,
        );
    let mut fake_authenticator = ExplicitlyFakePrerequisiteAuthenticator;
    let authenticated =
        AuthenticatedWorkerV2ExecutableV1::<AlphaGeneratedSafeTestKernel>::authenticate(
            admission,
            &mut fake_authenticator,
        )
        .map_err(|error| format!("fake prerequisite authentication failed: {error:?}"))?;
    let currentness =
        fe2o3_host::__hardware_test::acquire_retained_currentness_token(&authenticated)
            .map_err(|error| format!("currentness acquisition failed: {error:?}"))?;
    let authorized = authenticated
        .authorize_hsa_load(adapter)
        .map_err(|error| format!("reviewed HSA load authorization failed: {error:?}"))?;
    let mut loaded =
        fe2o3_host::__hardware_test::load_with_retained_currentness(authorized, &currentness)
            .map_err(|error| format!("reviewed HSA executable load failed: {error:?}"))?;

    let turnover =
        fe2o3_host::__hardware_test::begin_test_publication_turnover(&publication_directory);

    currentness.revalidate_locked_currentness()?;
    let executable_identity = loaded.load_observation().executable_object();
    run_generated_safe_length_case(&mut loaded, &context, &observed, executable_identity, 257)?;
    require(
        !turnover.completed(),
        "generation N+1 completed during real alpha/zeta dispatch",
    )?;
    currentness.revalidate_locked_currentness()?;
    loaded
        .unload()
        .map_err(|error| format!("reviewed HSA executable unload failed: {error:?}"))?;
    require(
        !turnover.completed(),
        "generation N+1 completed before currentness release after unload",
    )?;
    drop(currentness);

    turnover.finish();
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_raw_two_kernel_hardware_slice(
    bytes: Vec<u8>,
    digest: fe2o3_artifacts::PayloadDigest,
) -> Result<(), BoxError> {
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
        execute_loaded_two_kernel_slice(&mut adapter, &executable, executable_identity)
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

/// Regenerates the repository compiler-evidence golden and executes both kernels.
///
/// `scripts/test-gfx942-compiler-evidence.sh` is the required controller. It
/// rebuilds the measured C++ Worker, runs native CTests, invokes the canonical
/// V2 request/response and COV6 finalization test, checks the repository digest,
/// then invokes this test on MI300X. This test alone does not authenticate
/// compiler execution or create production authority.
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires the repository-generated alpha/zeta COV6 golden and a gfx942:xnack- GPU"]
fn gfx942_cov6_repository_golden_alpha_then_zeta_one_executable() -> Result<(), BoxError> {
    verify_compiler_evidence_device_sandbox()?;
    let (bytes, digest) = pinned_repository_golden_hsaco()?;
    run_raw_two_kernel_hardware_slice(bytes, digest)
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
    run_raw_two_kernel_hardware_slice(bytes, digest)
}

#[test]
fn repository_compiler_evidence_golden_is_exact_and_bounded() {
    let golden = compiler_evidence_golden().unwrap();
    assert_eq!(golden.boundary_lengths, HARDWARE_LENGTHS);
    assert_eq!(golden.kernels.len(), 2);
    assert!(golden.hsaco_bytes <= golden.max_hsaco_bytes);

    let mut malformed: serde_json::Value = serde_json::from_str(COMPILER_EVIDENCE_GOLDEN).unwrap();
    malformed["target"] = serde_json::Value::String("gfx942".to_owned());
    assert!(
        compiler_evidence_golden_from_str(&serde_json::to_string(&malformed).unwrap()).is_err()
    );

    let mut substituted = vec![0_u8; golden.hsaco_bytes as usize];
    substituted[0] = 1;
    assert!(validate_repository_golden_hsaco(&substituted).is_err());
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

#[cfg(feature = "hardware-test-hooks")]
#[test]
fn generated_safe_marker_fixtures_bind_exact_backend_witnesses_and_layouts() {
    assert!(
        validate_compiler_generated_semantic_witness_v1::<AlphaGeneratedSafeTestKernel>().is_ok()
    );
    assert!(
        validate_compiler_generated_semantic_witness_v1::<ZetaGeneratedSafeTestKernel>().is_ok()
    );
    assert_ne!(
        backend_semantic_witness_fixture_bytes(ALPHA_TEST_BINDING, ALPHA_TEST_HOST_CONTRACT),
        backend_semantic_witness_fixture_bytes(ZETA_TEST_BINDING, ZETA_TEST_HOST_CONTRACT),
    );
    assert!(alpha_generated_layout().is_ok());
    assert!(zeta_generated_layout().is_ok());
}

#[cfg(feature = "hardware-test-hooks")]
#[test]
fn generated_safe_host_contract_constants_match_canonical_derivation() {
    let launch = alpha_zeta_generated_launch();
    let alpha = derive_generated_host_contract_identity_v1(
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        ALPHA_TEST_BINDING,
        "alpha",
        "alpha",
        &alpha_generated_abi(),
        &launch,
    );
    assert_eq!(*alpha.as_bytes(), ALPHA_TEST_HOST_CONTRACT);

    let zeta = derive_generated_host_contract_identity_v1(
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        ZETA_TEST_BINDING,
        "zeta",
        "zeta",
        &zeta_generated_abi(),
        &launch,
    );
    assert_eq!(*zeta.as_bytes(), ZETA_TEST_HOST_CONTRACT);
}
