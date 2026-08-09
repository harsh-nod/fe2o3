use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::exit;

use fe2o3_artifact_transaction::{
    BuildAttempt, ProducerIdentity, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiContractV1, CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
#[path = "../../src/worker_v2_artifact_container.rs"]
mod worker_v2_artifact_container;

#[allow(dead_code)]
#[path = "../../src/worker_v2_restart.rs"]
mod worker_v2_restart;
use worker_v2_restart as restart_support;

const WORKER_ID: &str = "cargo-fe2o3-fixture-worker-v1";
const OUTPUT: &[u8] = b"cargo-fe2o3-fixture-output";
const MISMATCH_OUTPUT: &[u8] = b"cargo-fe2o3-fixture-outpuu";
const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

fn main() {
    if env::args().nth(1).as_deref() == Some("--stage-restart") {
        stage_restart();
        return;
    }
    if let Ok(mode) = env::var("FE2O3_FIXTURE_RUSTC_MODE") {
        fake_rustc(&mode);
        return;
    }
    worker();
}

fn fake_rustc(mode: &str) {
    if let Some(marker) = env::var_os("FE2O3_FIXTURE_RUSTC_MARKER") {
        let attempt =
            env::var("FE2O3_BUILD_ATTEMPT_V1").unwrap_or_else(|_| "no-attempt".to_owned());
        fs::write(marker, attempt).unwrap();
    }
    if mode == "fail" {
        exit(23);
    }
    if mode == "no-handoff" {
        return;
    }
    if mode == "device-requires-attempt" {
        if env::var_os("FE2O3_BUILD_ATTEMPT_V1").is_none() {
            eprintln!("device-producing fixture rejected a missing managed attempt");
            exit(42);
        }
        return;
    }
    assert!(matches!(
        mode,
        "publish" | "publish-valid" | "publish-mismatch" | "stop-after-handoff"
    ));

    let output_dir = env::var_os("FE2O3_HSACO_DIR").unwrap();
    let source = env::var_os("FE2O3_FIXTURE_SOURCE").unwrap();
    let attempt =
        BuildAttempt::from_env_value(&env::var("FE2O3_BUILD_ATTEMPT_V1").unwrap()).unwrap();
    let producer =
        ProducerIdentity::from_codegen("workflow_fixture", Some(Path::new(&source))).unwrap();
    let handoff = canonical_handoff(
        mode == "publish-mismatch",
        env::var_os("FE2O3_TEST_WORKER_V2_COV6").is_some(),
        env::var_os("FE2O3_TEST_WORKER_V2_ALPHA_ZETA").is_some(),
    );
    publish_compiler_module_handoff_v1(
        Path::new(&output_dir),
        &producer,
        attempt,
        handoff.canonical_bytes(),
    )
    .unwrap();
    if mode == "stop-after-handoff" {
        fs::write(
            env::var_os("FE2O3_FIXTURE_HANDOFF_MARKER").unwrap(),
            b"ready",
        )
        .unwrap();
        rustix::process::kill_process(
            rustix::process::getppid().unwrap(),
            rustix::process::Signal::STOP,
        )
        .unwrap();
    }
}

fn stage_restart() {
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
        DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
        KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
        PinnedWorkerIdentityV1, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
        ValidatedResponseIdentityV1, consume_compiler_module_handoff_v1,
        persist_worker_v2_publication_intent_v1,
    };

    let arguments = env::args_os().collect::<Vec<_>>();
    let output_dir = Path::new(&arguments[2]);
    let source = Path::new(&arguments[3]);
    let attempt = BuildAttempt::from_env_value(arguments[4].to_str().unwrap()).unwrap();
    let producer = ProducerIdentity::from_codegen("workflow_fixture", Some(source)).unwrap();
    consume_compiler_module_handoff_v1(output_dir, &producer, attempt).unwrap();

    let output = b"restart-recovered-inert-worker-v2-output";
    let output_identity: [u8; 32] = Sha256::digest(output).into();
    let plan = DurableLinkPublicationPlanV1::new(
        attempt,
        LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes([0x71; 32]),
            KernelSetIdentityV1::from_bytes([0x72; 32]),
            TargetIdentityV1::from_bytes([0x73; 32]),
        ),
        CanonicalLinkRequestIdentityV1::from_bytes([0x74; 32]),
        PinnedWorkerIdentityV1::from_bytes([0x75; 32]),
        ValidatedResponseIdentityV1::from_bytes([0x76; 32]),
        LinkedOutputIdentityV1::from_bytes(output_identity),
        FinalizationIdentityV1::from_bytes([0x77; 32]),
        FinalizedOutputIdentityV1::from_bytes(output_identity),
        AtomicPublicationIdentityV1::from_bytes([0x78; 32]),
    );
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x79; 32]);
    let store = restart_support::WorkerV2ResumeStoreV1::open(output_dir, &producer).unwrap();
    store
        .persist_pending(
            restart_support::WorkerV2PublicationKindV1::Raw,
            attempt,
            restart_support::restart_admission_commitment_v1(
                restart_support::WorkerV2PublicationKindV1::Raw,
                plan,
                upstream,
                output,
            ),
        )
        .unwrap();
    let intent = persist_worker_v2_publication_intent_v1(
        output_dir, &producer, attempt, plan, upstream, output,
    )
    .unwrap();
    store
        .persist_ready(
            restart_support::WorkerV2PublicationKindV1::Raw,
            attempt,
            intent.record().identity(),
        )
        .unwrap();
}

fn canonical_handoff(mismatch: bool, cov6: bool, alpha_zeta: bool) -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let code_object_version = if cov6 {
        CodeObjectVersion::V6
    } else {
        CodeObjectVersion::V5
    };
    let semantic_identity = [0x53; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let contract_id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "ffi_export",
        calling_convention: "C",
        code_object_version: if cov6 { 6 } else { 5 },
        target: "gfx942:xnack-",
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    });
    let contract = CompilerFfiContractV1::new(
        contract_id,
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        target,
        code_object_version,
        CompilerFfiSourceOwnerV1::new(
            "workflow_fixture",
            "workflow_fixture::ffi_export",
            [0x35; 16],
            "_RINvNtCs1234_16workflow_fixture10ffi_export",
        )
        .unwrap(),
        "ffi_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap();
    let mut envelope = CompilerFfiEnvelopeBuilderV1::new(target, code_object_version, 1).unwrap();
    envelope.push(contract).unwrap();
    let mut entries = if alpha_zeta {
        vec![
            (CompilerModuleSymbolRoleV1::KernelEntry, "alpha".to_owned()),
            (CompilerModuleSymbolRoleV1::KernelEntry, "zeta".to_owned()),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                "alpha.kd".to_owned(),
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                "zeta.kd".to_owned(),
            ),
            (
                CompilerModuleSymbolRoleV1::DeviceFfiExport,
                "ffi_export".to_owned(),
            ),
        ]
    } else {
        vec![
            (
                CompilerModuleSymbolRoleV1::KernelEntry,
                "workflow_kernel".to_owned(),
            ),
            (
                CompilerModuleSymbolRoleV1::KernelDescriptor,
                "workflow_kernel.kd".to_owned(),
            ),
            (
                CompilerModuleSymbolRoleV1::DeviceFfiExport,
                "ffi_export".to_owned(),
            ),
        ]
    };
    if mismatch {
        entries.push((
            CompilerModuleSymbolRoleV1::KernelEntry,
            "workflow_mismatch".to_owned(),
        ));
        entries.push((
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "workflow_mismatch.kd".to_owned(),
        ));
    }
    entries.sort();
    let module: &[u8] = if alpha_zeta {
        b"define amdgpu_kernel void @alpha() { ret void }\ndefine amdgpu_kernel void @zeta() { ret void }\ndefine i32 @ffi_export(i32 %value) { ret i32 %value }\n"
    } else {
        b"define amdgpu_kernel void @workflow_kernel() { ret void }\ndefine i32 @ffi_export(i32 %value) { ret i32 %value }\n"
    };
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        code_object_version,
        envelope.finish().unwrap(),
        CompilerModuleSymbolManifestV1::new(entries).unwrap(),
        module,
    )
    .unwrap()
}

fn worker() {
    let mut prefix = [0_u8; 46];
    io::stdin().read_exact(&mut prefix).unwrap();
    let mut request = prefix.to_vec();
    io::stdin().read_to_end(&mut request).unwrap();
    let is_v2 = &request[..8] == b"F3LREQ02";
    let output = if is_v2 && contains(&request, b"workflow_mismatch") {
        MISMATCH_OUTPUT.to_vec()
    } else if let Some(input) = elf_input_payload(field(&request, if is_v2 { 10 } else { 6 })) {
        linked_elf(input)
    } else {
        OUTPUT.to_vec()
    };
    io::stdout()
        .write_all(&response(&request, is_v2, &output))
        .unwrap();
}

fn linked_elf(input: &[u8]) -> Vec<u8> {
    let mut output = input.to_vec();
    let text = output
        .windows(16)
        .position(|window| window == [0xbf; 16])
        .expect("synthetic provider has a text body");
    output[text] ^= 1;
    output
}

fn elf_input_payload(mut inputs: &[u8]) -> Option<&[u8]> {
    let count = u32::from_le_bytes(inputs.get(..4)?.try_into().ok()?);
    inputs = inputs.get(4..)?;
    for _ in 0..count {
        let length = u64::from_le_bytes(inputs.get(33..41)?.try_into().ok()?);
        let length = usize::try_from(length).ok()?;
        let payload = inputs.get(41..41_usize.checked_add(length)?)?;
        if payload.starts_with(b"\x7fELF") {
            return Some(payload);
        }
        inputs = inputs.get(41_usize.checked_add(length)?..)?;
    }
    None
}

fn response(request: &[u8], is_v2: bool, output_bytes: &[u8]) -> Vec<u8> {
    let request_id: [u8; 32] = request[14..46].try_into().unwrap();
    let request_identity: [u8; 32] = field(request, if is_v2 { 15 } else { 10 })
        .try_into()
        .unwrap();
    let mut bytes = if is_v2 {
        b"F3LRSP02".to_vec()
    } else {
        b"F3LRSP01".to_vec()
    };
    push_field(&mut bytes, 1, &request_id);
    push_field(&mut bytes, 2, &request_identity);
    let offset = if is_v2 {
        push_field(&mut bytes, 3, field(request, 8));
        1
    } else {
        0
    };
    push_field(&mut bytes, 3 + offset, WORKER_ID.as_bytes());
    push_field(&mut bytes, 4 + offset, &[9]);
    push_field(&mut bytes, 5 + offset, &0_u32.to_le_bytes());
    let output_identity: [u8; 32] = Sha256::digest(output_bytes).into();
    let mut output = vec![1];
    output.extend_from_slice(&output_identity);
    output.extend_from_slice(&(output_bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(output_bytes);
    push_field(&mut bytes, 6 + offset, &output);
    bytes
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn field(bytes: &[u8], wanted: u16) -> &[u8] {
    let mut offset = 8;
    while offset < bytes.len() {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let len = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap()) as usize;
        offset += 6;
        if tag == wanted {
            return &bytes[offset..offset + len];
        }
        offset += len;
    }
    panic!("missing field {wanted}")
}

fn push_field(bytes: &mut Vec<u8>, tag: u16, value: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}
