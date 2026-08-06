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

const WORKER_ID: &str = "cargo-fe2o3-fixture-worker-v1";
const OUTPUT: &[u8] = b"cargo-fe2o3-fixture-output";
const MISMATCH_OUTPUT: &[u8] = b"cargo-fe2o3-fixture-outpuu";
const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

fn main() {
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
    assert!(matches!(mode, "publish" | "publish-mismatch"));

    let output_dir = env::var_os("FE2O3_HSACO_DIR").unwrap();
    let source = env::var_os("FE2O3_FIXTURE_SOURCE").unwrap();
    let attempt =
        BuildAttempt::from_env_value(&env::var("FE2O3_BUILD_ATTEMPT_V1").unwrap()).unwrap();
    let producer =
        ProducerIdentity::from_codegen("workflow_fixture", Some(Path::new(&source))).unwrap();
    let handoff = canonical_handoff(mode == "publish-mismatch");
    publish_compiler_module_handoff_v1(
        Path::new(&output_dir),
        &producer,
        attempt,
        handoff.canonical_bytes(),
    )
    .unwrap();
}

fn canonical_handoff(mismatch: bool) -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let semantic_identity = [0x53; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let contract_id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "workflow_export",
        calling_convention: "C",
        code_object_version: 5,
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
        CodeObjectVersion::V5,
        CompilerFfiSourceOwnerV1::new(
            "workflow_fixture",
            "workflow_fixture::workflow_export",
            [0x35; 16],
            "_RINvNtCs1234_16workflow_fixture15workflow_export",
        )
        .unwrap(),
        "workflow_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap();
    let mut envelope = CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V5, 1).unwrap();
    envelope.push(contract).unwrap();
    let mut entries = vec![
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
            "workflow_export".to_owned(),
        ),
    ];
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
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V5,
        envelope.finish().unwrap(),
        CompilerModuleSymbolManifestV1::new(entries).unwrap(),
        b"define amdgpu_kernel void @workflow_kernel() { ret void }\ndefine i32 @workflow_export(i32 %value) { ret i32 %value }\n",
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
        MISMATCH_OUTPUT
    } else {
        OUTPUT
    };
    io::stdout()
        .write_all(&response(&request, is_v2, output))
        .unwrap();
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
