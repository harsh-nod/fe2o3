#![cfg(target_os = "linux")]

#[allow(dead_code)]
#[path = "../src/live_kfd_v3.rs"]
mod live_kfd_v3;

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV1, DebugSourceMapDocumentV2,
    DebugSourceScopeV2, DebugSourceVariableBindingV2, DebugSourceVariableFallbackV2,
    DebugSourceVariableLocationV2, DebugSourceVariableV2, SimulationCompilerExecutionBindingV1,
    SimulationProductionKirIdentityV1, SimulationSourceLineageV1, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV1, VerifiedSimulationBundleV2,
    decode_module_v7,
};
use sha2::{Digest, Sha256};

use live_kfd_v3::{
    LiveKfdBindingErrorCodeV3, LiveKfdBindingLimitsV3, LiveKfdHostLaunchContentV3,
    LiveKfdInputRoleV3, LiveKfdSemanticCapabilityAvailabilityV3, LiveKfdSemanticCapabilityNameV3,
    LiveKfdSemanticSessionPlanV3, LiveKfdSemanticUnavailableReasonV3,
    admit_live_kfd_semantic_session_v3, admit_live_kfd_semantic_session_with_hook_v3,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-live-kfd-v3-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is in workspace/crates")
        .to_owned()
}

fn inner_bundle() -> VerifiedSimulationBundleV1 {
    let kir =
        fs::read(workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"))
            .unwrap();
    let canonical = VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir.clone()).unwrap();
    let production =
        VerifiedCanonicalKernelIrV8::from_module(decode_module_v7(&kir).unwrap()).unwrap();
    VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([3; 32], 33, [4; 32], 44).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production.identity().digest(),
            production.identity().canonical_length(),
        )
        .unwrap(),
        "gfx942:xnack-",
        canonical,
        None,
    )
    .unwrap()
}

fn source_map_v2(inner: &VerifiedSimulationBundleV1) -> DebugSourceMapDocumentV2 {
    let v1 = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(workspace_root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json"))
            .unwrap(),
    )
    .unwrap();
    let span = v1.sites()[0].spans()[0];
    let scope = DebugSourceScopeV2::new([0x31; 32], 0, None, 0, span).unwrap();
    let variable = DebugSourceVariableV2::new(
        [0x41; 32],
        "buffer".into(),
        0,
        scope.identity(),
        DebugSourceVariableFallbackV2::NotInScope,
        vec![
            DebugSourceVariableLocationV2::new(
                0,
                0,
                1,
                DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
            )
            .unwrap(),
        ],
    )
    .unwrap();
    DebugSourceMapDocumentV2::new(
        DebugSourceMapBindingV1::new(
            *inner.subject_identity(),
            *inner.canonical_kir_v7_identity().digest(),
            inner.canonical_kir_v7_identity().canonical_length(),
        )
        .unwrap(),
        v1.files().to_vec(),
        v1.sites().to_vec(),
        v1.eliminated().to_vec(),
        vec![scope],
        vec![variable],
    )
    .unwrap()
}

fn valid_hsaco(kernel: &str) -> Vec<u8> {
    let mut metadata = Vec::new();
    msgpack_map(&mut metadata, 3);
    msgpack_string(&mut metadata, "amdhsa.version");
    msgpack_array(&mut metadata, 2);
    msgpack_unsigned(&mut metadata, 1);
    msgpack_unsigned(&mut metadata, 2);
    msgpack_string(&mut metadata, "amdhsa.target");
    msgpack_string(&mut metadata, "amdgcn-amd-amdhsa--gfx1151");
    msgpack_string(&mut metadata, "amdhsa.kernels");
    msgpack_array(&mut metadata, 1);
    msgpack_map(&mut metadata, 10);
    for (name, value) in [
        (".name", FixtureValue::String(kernel)),
        (".symbol", FixtureValue::String("kernel.kd")),
        (".kernarg_segment_size", FixtureValue::Unsigned(0)),
        (".kernarg_segment_align", FixtureValue::Unsigned(8)),
        (".group_segment_fixed_size", FixtureValue::Unsigned(0)),
        (".private_segment_fixed_size", FixtureValue::Unsigned(0)),
        (".wavefront_size", FixtureValue::Unsigned(32)),
        (".sgpr_count", FixtureValue::Unsigned(8)),
        (".vgpr_count", FixtureValue::Unsigned(4)),
        (".max_flat_workgroup_size", FixtureValue::Unsigned(64)),
    ] {
        msgpack_string(&mut metadata, name);
        match value {
            FixtureValue::String(value) => msgpack_string(&mut metadata, value),
            FixtureValue::Unsigned(value) => msgpack_unsigned(&mut metadata, value),
        }
    }
    hsaco_with_metadata(&metadata)
}

enum FixtureValue<'a> {
    String(&'a str),
    Unsigned(u64),
}

fn msgpack_string(bytes: &mut Vec<u8>, value: &str) {
    if value.len() < 32 {
        bytes.push(0xa0 | u8::try_from(value.len()).unwrap());
    } else {
        bytes.extend_from_slice(&[0xd9, u8::try_from(value.len()).unwrap()]);
    }
    bytes.extend_from_slice(value.as_bytes());
}

fn msgpack_unsigned(bytes: &mut Vec<u8>, value: u64) {
    if value < 128 {
        bytes.push(u8::try_from(value).unwrap());
    } else {
        bytes.push(0xcf);
        bytes.extend_from_slice(&value.to_be_bytes());
    }
}

fn msgpack_array(bytes: &mut Vec<u8>, length: u8) {
    assert!(length < 16);
    bytes.push(0x90 | length);
}

fn msgpack_map(bytes: &mut Vec<u8>, length: u8) {
    assert!(length < 16);
    bytes.push(0x80 | length);
}

fn hsaco_with_metadata(metadata: &[u8]) -> Vec<u8> {
    const ELF_HEADER_BYTES: usize = 64;
    const SECTION_HEADER_BYTES: usize = 64;
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&32_u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);

    let mut bytes = vec![0; ELF_HEADER_BYTES];
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    let string_table = b"\0.note\0.shstrtab\0";
    let string_table_offset = bytes.len();
    bytes.extend_from_slice(string_table);
    align(&mut bytes, 8);
    let section_offset = bytes.len();
    bytes.resize(section_offset + 3 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u32(&mut bytes, 48, 0x4a);
    write_u64(&mut bytes, 40, u64::try_from(section_offset).unwrap());
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(
        &mut bytes,
        note_header + 24,
        u64::try_from(note_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        note_header + 32,
        u64::try_from(note.len()).unwrap(),
    );
    write_u64(&mut bytes, note_header + 48, 4);

    let strings_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strings_header, 7);
    write_u32(&mut bytes, strings_header + 4, 3);
    write_u64(
        &mut bytes,
        strings_header + 24,
        u64::try_from(string_table_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        strings_header + 32,
        u64::try_from(string_table.len()).unwrap(),
    );
    write_u64(&mut bytes, strings_header + 48, 1);
    bytes
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let padding = (alignment - bytes.len() % alignment) % alignment;
    bytes.resize(bytes.len() + padding, 0);
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

struct Inputs {
    bundle: PathBuf,
    request: PathBuf,
    hsaco: PathBuf,
    host: PathBuf,
    bundle_bytes: Vec<u8>,
    request_bytes: Vec<u8>,
    hsaco_bytes: Vec<u8>,
    host_bytes: Vec<u8>,
}

fn inputs(directory: &TestDirectory) -> Inputs {
    let inner = inner_bundle();
    let map = source_map_v2(&inner);
    let bundle = VerifiedSimulationBundleV2::new(inner, map).unwrap();
    let bundle_bytes = bundle.canonical_bytes().to_vec();
    let request_bytes =
        fs::read(workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"))
            .unwrap();
    let hsaco_bytes = valid_hsaco("kernel");
    let host_bytes = b"#!/bin/sh\nexit 0\n".to_vec();
    let bundle_path = directory.0.join("kernel.fe2sim");
    let request_path = directory.0.join("request.json");
    let hsaco_path = directory.0.join("kernel.hsaco");
    let host_path = directory.0.join("host");
    fs::write(&bundle_path, &bundle_bytes).unwrap();
    fs::write(&request_path, &request_bytes).unwrap();
    fs::write(&hsaco_path, &hsaco_bytes).unwrap();
    fs::write(&host_path, &host_bytes).unwrap();
    fs::set_permissions(&host_path, fs::Permissions::from_mode(0o700)).unwrap();
    Inputs {
        bundle: bundle_path,
        request: request_path,
        hsaco: hsaco_path,
        host: host_path,
        bundle_bytes,
        request_bytes,
        hsaco_bytes,
        host_bytes,
    }
}

fn plan(inputs: &Inputs, hsaco: Option<PathBuf>) -> LiveKfdSemanticSessionPlanV3 {
    LiveKfdSemanticSessionPlanV3::try_new(
        &inputs.bundle,
        &inputs.request,
        hsaco,
        &inputs.host,
        LiveKfdBindingLimitsV3::default(),
    )
    .unwrap()
}

#[test]
fn exact_binding_retains_cpu_inputs_without_load_or_execution_claims() {
    let directory = TestDirectory::new();
    let inputs = inputs(&directory);
    let mut binding = admit_live_kfd_semantic_session_v3(plan(&inputs, Some(inputs.hsaco.clone())))
        .expect("bind exact live KFD semantic inputs");

    assert_eq!(binding.bundle_bytes(), inputs.bundle_bytes);
    assert_eq!(binding.request_bytes(), inputs.request_bytes);
    assert_eq!(
        binding.declared_hsaco_bytes(),
        Some(inputs.hsaco_bytes.as_slice())
    );
    assert_eq!(
        binding.bundle_content_identity().sha256(),
        <[u8; 32]>::from(Sha256::digest(&inputs.bundle_bytes))
    );
    assert_eq!(
        binding.request_content_identity().sha256(),
        binding.admitted_input().request_sha256
    );
    let declared = binding.declared_hsaco().unwrap();
    assert_eq!(declared.code_object_version(), 6);
    assert!(!declared.claims_loaded());
    assert!(!declared.claims_executed());
    assert_eq!(
        binding.observed_host().content().sha256(),
        <[u8; 32]>::from(Sha256::digest(&inputs.host_bytes))
    );
    assert!(!binding.observed_host().claims_launched());
    assert!(!binding.observed_host().claims_executed());
    assert!(matches!(
        binding.host_launch_content(),
        LiveKfdHostLaunchContentV3::ObservedPreExec(_)
    ));
    assert!(
        !binding
            .host_launch_content()
            .claims_target_instructions_executed()
    );
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
    assert!(!binding.authenticates_gpu_execution());

    let capabilities = binding.capabilities();
    let capability = |name| {
        capabilities
            .iter()
            .find(|capability| capability.name == name)
            .unwrap()
            .availability
    };
    assert_eq!(
        capability(LiveKfdSemanticCapabilityNameV3::CpuReference),
        LiveKfdSemanticCapabilityAvailabilityV3::Available
    );
    assert_eq!(
        capability(LiveKfdSemanticCapabilityNameV3::HsacoLoadedIdentity),
        LiveKfdSemanticCapabilityAvailabilityV3::Unavailable(
            LiveKfdSemanticUnavailableReasonV3::LoadNotObserved
        )
    );
    assert_eq!(
        capability(LiveKfdSemanticCapabilityNameV3::GpuExecutionIdentity),
        LiveKfdSemanticCapabilityAvailabilityV3::Unavailable(
            LiveKfdSemanticUnavailableReasonV3::ExecutionNotObserved
        )
    );
    assert_eq!(
        capability(LiveKfdSemanticCapabilityNameV3::ExactLaunchedHostContent),
        LiveKfdSemanticCapabilityAvailabilityV3::Unavailable(
            LiveKfdSemanticUnavailableReasonV3::ExecStopNotObserved
        )
    );

    let native_fd = binding.host_executable_fd().as_raw_fd();
    let debug = format!("{binding:?}");
    for path in [&inputs.bundle, &inputs.request, &inputs.hsaco, &inputs.host] {
        assert!(!debug.contains(path.to_str().unwrap()));
    }
    assert!(!debug.contains(&format!("/proc/self/fd/{native_fd}")));
    assert!(!debug.contains(&format!("native_fd={native_fd}")));

    binding.record_host_exec_sigtrap_v3().unwrap();
    assert_eq!(
        binding.host_launch_content(),
        LiveKfdHostLaunchContentV3::ExactLaunchedAfterExecSigtrap {
            content: binding.observed_host().content()
        }
    );
    assert!(binding.capabilities().iter().any(|capability| {
        capability.name == LiveKfdSemanticCapabilityNameV3::ExactLaunchedHostContent
            && capability.availability == LiveKfdSemanticCapabilityAvailabilityV3::Available
    }));
    assert!(!binding.authenticates_gpu_execution());
}

#[test]
fn omitted_hsaco_is_explicitly_unavailable() {
    let directory = TestDirectory::new();
    let inputs = inputs(&directory);
    let binding = admit_live_kfd_semantic_session_v3(plan(&inputs, None)).unwrap();
    assert_eq!(binding.declared_hsaco(), None);
    assert_eq!(binding.declared_hsaco_bytes(), None);
    assert!(binding.capabilities().iter().any(|capability| {
        capability.name == LiveKfdSemanticCapabilityNameV3::DeclaredHsacoBytes
            && capability.availability
                == LiveKfdSemanticCapabilityAvailabilityV3::Unavailable(
                    LiveKfdSemanticUnavailableReasonV3::HsacoNotDeclared,
                )
    }));
}

#[test]
fn rejects_symlinks_hard_links_role_aliases_and_nonregular_inputs() {
    let directory = TestDirectory::new();
    let inputs = inputs(&directory);

    let linked_host = directory.0.join("linked-host");
    symlink(&inputs.host, &linked_host).unwrap();
    let error = admit_live_kfd_semantic_session_v3(
        LiveKfdSemanticSessionPlanV3::try_new(
            &inputs.bundle,
            &inputs.request,
            Some(inputs.hsaco.clone()),
            &linked_host,
            LiveKfdBindingLimitsV3::default(),
        )
        .unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::HostExecutable);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::SymlinkRejected);
    fs::remove_file(&linked_host).unwrap();

    let hard_link = directory.0.join("request-hard-link");
    fs::hard_link(&inputs.request, &hard_link).unwrap();
    let error =
        admit_live_kfd_semantic_session_v3(plan(&inputs, Some(inputs.hsaco.clone()))).unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::Request);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::HardLinkRejected);
    fs::remove_file(&hard_link).unwrap();

    let error = admit_live_kfd_semantic_session_v3(plan(&inputs, Some(inputs.request.clone())))
        .unwrap_err();
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::InputAlias);

    let error = admit_live_kfd_semantic_session_v3(
        LiveKfdSemanticSessionPlanV3::try_new(
            &inputs.bundle,
            &inputs.request,
            Some(directory.0.clone()),
            &inputs.host,
            LiveKfdBindingLimitsV3::default(),
        )
        .unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::DeclaredHsaco);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::NonRegular);
}

#[test]
fn rejects_size_overruns_and_nonexecutable_host_files() {
    let directory = TestDirectory::new();
    let inputs = inputs(&directory);
    let limits = LiveKfdBindingLimitsV3::try_new(
        inputs.bundle_bytes.len(),
        inputs.request_bytes.len(),
        inputs.hsaco_bytes.len() - 1,
        inputs.host_bytes.len(),
    )
    .unwrap();
    let error = admit_live_kfd_semantic_session_v3(
        LiveKfdSemanticSessionPlanV3::try_new(
            &inputs.bundle,
            &inputs.request,
            Some(inputs.hsaco.clone()),
            &inputs.host,
            limits,
        )
        .unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::DeclaredHsaco);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::TooLarge);

    fs::set_permissions(&inputs.host, fs::Permissions::from_mode(0o600)).unwrap();
    let error =
        admit_live_kfd_semantic_session_v3(plan(&inputs, Some(inputs.hsaco.clone()))).unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::HostExecutable);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::NotExecutable);
}

#[test]
fn rejects_a_declared_file_that_is_not_a_strictly_inspected_hsaco() {
    let directory = TestDirectory::new();
    let inputs = inputs(&directory);
    fs::write(&inputs.hsaco, b"\x7fELFnot-an-amdgpu-code-object").unwrap();
    let error =
        admit_live_kfd_semantic_session_v3(plan(&inputs, Some(inputs.hsaco.clone()))).unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::DeclaredHsaco);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::HsacoRejected);
}

#[test]
fn rejects_a_request_changed_between_capture_and_admission() {
    let directory = TestDirectory::new();
    let initial_inputs = inputs(&directory);
    let request_path = initial_inputs.request.clone();
    let mut changed = initial_inputs.request_bytes.clone();
    changed.push(b'\n');
    let error = admit_live_kfd_semantic_session_with_hook_v3(
        plan(&initial_inputs, Some(initial_inputs.hsaco.clone())),
        move || fs::write(request_path, changed).unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::Request);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::InputChanged);

    let directory = TestDirectory::new();
    let inputs = inputs(&directory);
    let request_path = inputs.request.clone();
    let error = admit_live_kfd_semantic_session_with_hook_v3(
        plan(&inputs, Some(inputs.hsaco.clone())),
        move || fs::write(request_path, b"malformed changed request").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.role(), LiveKfdInputRoleV3::Request);
    assert_eq!(error.code(), LiveKfdBindingErrorCodeV3::InputChanged);
}

#[test]
fn session_identity_changes_with_declared_hsaco_content() {
    let first_directory = TestDirectory::new();
    let first = inputs(&first_directory);
    let first_binding =
        admit_live_kfd_semantic_session_v3(plan(&first, Some(first.hsaco.clone()))).unwrap();

    let second_directory = TestDirectory::new();
    let second = inputs(&second_directory);
    fs::write(&second.hsaco, valid_hsaco("other_kernel")).unwrap();
    let second_binding =
        admit_live_kfd_semantic_session_v3(plan(&second, Some(second.hsaco.clone()))).unwrap();
    assert_ne!(
        first_binding.session_identity(),
        second_binding.session_identity()
    );
}
