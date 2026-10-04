//! Bounded, explicitly requested diagnostics outside the HSACO observation namespace.

use super::*;
use fe2o3_compiler_ffi::{CompilerModuleHandoffV2, CompilerModuleKindV1};
use serde::Deserialize;

const CAPTURE_NAMESPACE: &str = "fe2o3-engineering-diagnostics-v1";
const MAX_KIR_BYTES: u64 = 8 * 1024 * 1024;
const MAX_LLVM_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CANDIDATE_BYTES: u64 = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedIdentity {
    sha256: String,
    byte_len: u64,
}

impl CapturedIdentity {
    fn matches(&self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64 && self.sha256 == hex(&Sha256::digest(bytes))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    schema: String,
    encoding: String,
    target: String,
    kir: CapturedIdentity,
    llvm: CapturedIdentity,
    authority: bool,
}

struct Captured {
    kir: Vec<u8>,
    llvm: Vec<u8>,
}

pub(crate) fn validate_root(root: &Path, observation_root: &Path) -> Result<(), String> {
    validate_fresh_output_root(root)?;
    if root.file_name() != Some(OsStr::new(CAPTURE_NAMESPACE))
        || root.parent() != observation_root.parent()
    {
        return Err("diagnostic capture root must be the fresh fe2o3-engineering-diagnostics-v1 sibling of --output-root".to_owned());
    }
    Ok(())
}

fn validate_payload(
    candidate: &Candidate,
    kir: &[u8],
    llvm: &[u8],
    actual_llvm: &[u8],
    target: &str,
) -> Result<(), String> {
    if candidate.schema != "EngineeringDiagnosticCandidateV1"
        || candidate.encoding != "commit-bound-target-kir-debug-not-canonical-replay"
        || candidate.authority
        || candidate.target != target
        || kir.is_empty()
        || kir.len() as u64 > MAX_KIR_BYTES
        || llvm.is_empty()
        || llvm.len() as u64 > MAX_LLVM_BYTES
        || !candidate.kir.matches(kir)
        || !candidate.llvm.matches(llvm)
        || llvm != actual_llvm
    {
        return Err(
            "diagnostic candidate is incomplete or differs from the actual handoff".to_owned(),
        );
    }
    Ok(())
}

fn read_capture(
    scratch: &Path,
    handoff: &[u8],
    manifest: &[u8],
    target: &str,
) -> Result<Captured, String> {
    let metadata = read_bounded_regular_file(
        &scratch.join("diagnostic-candidate.json"),
        MAX_CANDIDATE_BYTES,
        false,
    )?;
    let candidate: Candidate =
        serde_json::from_slice(&metadata).map_err(|_| "invalid diagnostic candidate metadata")?;
    let kir = read_bounded_regular_file(
        &scratch.join("diagnostic-target-kir.txt"),
        MAX_KIR_BYTES,
        false,
    )?;
    let llvm = read_bounded_regular_file(
        &scratch.join("diagnostic-pre-worker.ll"),
        MAX_LLVM_BYTES,
        false,
    )?;
    let decoded =
        CompilerModuleHandoffV2::decode(handoff).map_err(|_| "invalid diagnostic handoff")?;
    if decoded.kind() != CompilerModuleKindV1::LlvmTextIr || decoded.target().to_string() != target
    {
        return Err("diagnostic handoff kind or target differs".to_owned());
    }
    validate_payload(&candidate, &kir, &llvm, decoded.module_bytes(), target)?;
    let observation: serde_json::Value =
        serde_json::from_slice(manifest).map_err(|_| "invalid observation manifest")?;
    let expected = serde_json::to_value(identity(ContentIdentityV1::calculate(handoff)))
        .map_err(|_| "cannot encode handoff identity")?;
    if observation.get("compiler_handoff") != Some(&expected)
        || observation
            .get("target")
            .and_then(serde_json::Value::as_str)
            != Some(target)
    {
        return Err("diagnostic handoff is not bound to the completed observation".to_owned());
    }
    Ok(Captured { kir, llvm })
}

#[derive(Serialize)]
struct Receipt<'a> {
    schema: &'static str,
    state: &'static str,
    requires_cli_completion_acknowledgement: bool,
    encoding: &'static str,
    target: &'a str,
    observation: Identity,
    compiler_handoff: Identity,
    kir: Option<Identity>,
    llvm: Option<Identity>,
    grants: Grants,
    source_authentication: bool,
    proof_authority: bool,
}

pub(crate) fn retain(
    root: &Path,
    observation_root: &Path,
    scratch: &Path,
    handoff: &[u8],
    manifest: &[u8],
    target: &str,
) -> Result<Option<(String, u64)>, String> {
    validate_root(root, observation_root)?;
    let capture = read_capture(scratch, handoff, manifest, target).ok();
    let receipt = publish(
        root,
        handoff,
        manifest,
        target,
        capture.as_ref(),
        PublishFault::None,
    )?;
    Ok(capture.map(|_| (hex(&Sha256::digest(&receipt)), receipt.len() as u64)))
}

#[derive(Clone, Copy, PartialEq)]
enum PublishFault {
    None,
    AfterPayload,
    AfterMarker,
}

fn publish(
    root: &Path,
    handoff: &[u8],
    manifest: &[u8],
    target: &str,
    capture: Option<&Captured>,
    fault: PublishFault,
) -> Result<Vec<u8>, String> {
    let parent_path = root.parent().ok_or("diagnostic root has no parent")?;
    let parent = crate::project::PinnedDirectory::open_existing(
        parent_path.to_path_buf(),
        "diagnostic output parent",
    )?;
    rustix::fs::mkdirat(
        parent.file(),
        CAPTURE_NAMESPACE,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
    )
    .map_err(|error| format!("cannot create fresh diagnostic root: {error}"))?;
    let directory = parent
        .open_child(CAPTURE_NAMESPACE, "diagnostic output root")?
        .ok_or("diagnostic output root disappeared")?;
    directory.validate_path("diagnostic output root")?;
    let mut files = Vec::new();
    if let Some(capture) = capture {
        for (name, bytes) in [
            ("target-kir.txt", capture.kir.as_slice()),
            ("pre-worker.ll", capture.llvm.as_slice()),
            ("compiler-handoff-v2", handoff),
        ] {
            let retained = write_new_file_at(directory.file(), name, bytes, 0o600)?;
            files.push((name, retained));
        }
    }
    if fault == PublishFault::AfterPayload {
        return Err("injected diagnostic write failure; no complete marker".to_owned());
    }
    sync_directory(directory.file())
        .map_err(|error| format!("cannot sync diagnostic payloads: {error}"))?;
    directory.validate_path("diagnostic output root")?;
    for (name, file) in &files {
        file.validate_name(directory.file(), name)?;
    }
    let receipt = serde_json::to_vec(&Receipt {
        schema: "EngineeringDiagnosticCaptureV1",
        state: if capture.is_some() {
            "payload-complete"
        } else {
            "omitted-ineligible"
        },
        requires_cli_completion_acknowledgement: true,
        encoding: "commit-bound-target-kir-debug-not-canonical-replay",
        target,
        observation: identity(ContentIdentityV1::calculate(manifest)),
        compiler_handoff: identity(ContentIdentityV1::calculate(handoff)),
        kir: capture.map(|value| identity(ContentIdentityV1::calculate(&value.kir))),
        llvm: capture.map(|value| identity(ContentIdentityV1::calculate(&value.llvm))),
        grants: Grants {
            publication: false,
            load: false,
            launch: false,
        },
        source_authentication: false,
        proof_authority: false,
    })
    .map_err(|_| "cannot encode diagnostic receipt")?;
    if receipt.len() as u64 > MAX_CANDIDATE_BYTES {
        return Err("diagnostic receipt exceeds byte bound".to_owned());
    }
    let marker = write_new_file_at(directory.file(), "capture.json", &receipt, 0o600)?;
    if fault == PublishFault::AfterMarker {
        return Err("injected late diagnostic failure; no CLI acknowledgement".to_owned());
    }
    sync_directory(directory.file())
        .map_err(|error| format!("cannot sync diagnostic root: {error}"))?;
    sync_directory(parent.file())
        .map_err(|error| format!("cannot sync diagnostic parent: {error}"))?;
    directory.validate_path("diagnostic output root")?;
    for (name, file) in &files {
        file.validate_name(directory.file(), name)?;
    }
    marker.validate_name(directory.file(), "capture.json")?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(kir: &[u8], llvm: &[u8]) -> Candidate {
        Candidate {
            schema: "EngineeringDiagnosticCandidateV1".to_owned(),
            encoding: "commit-bound-target-kir-debug-not-canonical-replay".to_owned(),
            target: "gfx950:xnack-".to_owned(),
            kir: CapturedIdentity {
                sha256: hex(&Sha256::digest(kir)),
                byte_len: kir.len() as u64,
            },
            llvm: CapturedIdentity {
                sha256: hex(&Sha256::digest(llvm)),
                byte_len: llvm.len() as u64,
            },
            authority: false,
        }
    }

    #[test]
    fn engineering_capture_requires_exact_payload_and_handoff_llvm() {
        let candidate = candidate(b"kir", b"llvm");
        assert!(validate_payload(&candidate, b"kir", b"llvm", b"llvm", "gfx950:xnack-").is_ok());
        assert!(validate_payload(&candidate, b"bad", b"llvm", b"llvm", "gfx950:xnack-").is_err());
        assert!(validate_payload(&candidate, b"kir", b"llvm", b"else", "gfx950:xnack-").is_err());
        assert!(validate_payload(&candidate, b"kir", b"else", b"else", "gfx950:xnack-").is_err());
        assert!(validate_payload(&candidate, b"kir", b"llvm", b"llvm", "gfx942:xnack-").is_err());
    }

    #[test]
    fn engineering_capture_rejects_authority_and_metadata_extensions() {
        let mut candidate = candidate(b"kir", b"llvm");
        candidate.authority = true;
        assert!(validate_payload(&candidate, b"kir", b"llvm", b"llvm", "gfx950:xnack-").is_err());
        assert!(serde_json::from_str::<Candidate>(r#"{"schema":"x","extra":true}"#).is_err());
    }

    #[test]
    fn engineering_capture_rejects_malformed_actual_handoff() {
        let scratch = ScratchDirectory::new().unwrap();
        let metadata = serde_json::json!({
            "schema": "EngineeringDiagnosticCandidateV1",
            "encoding": "commit-bound-target-kir-debug-not-canonical-replay",
            "target": "gfx950:xnack-", "authority": false,
            "kir": {"sha256": hex(&Sha256::digest(b"kir")), "byte_len": 3},
            "llvm": {"sha256": hex(&Sha256::digest(b"llvm")), "byte_len": 4}
        });
        write_new_file(
            &scratch.path.join("diagnostic-candidate.json"),
            &serde_json::to_vec(&metadata).unwrap(),
            0o600,
        )
        .unwrap();
        write_new_file(
            &scratch.path.join("diagnostic-target-kir.txt"),
            b"kir",
            0o600,
        )
        .unwrap();
        write_new_file(
            &scratch.path.join("diagnostic-pre-worker.ll"),
            b"llvm",
            0o600,
        )
        .unwrap();
        assert!(read_capture(&scratch.path, b"invalid", b"{}", "gfx950:xnack-").is_err());
    }

    #[test]
    fn engineering_capture_canonical_handoff_roundtrip_binds_observation_and_ack() {
        use fe2o3_compiler_ffi::{
            CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleSymbolManifestV1,
            CompilerModuleSymbolRoleV1 as Role, DeviceTargetV1,
        };
        let scratch = ScratchDirectory::new().unwrap();
        let target = DeviceTargetV1::parse("gfx950:xnack-").unwrap();
        let llvm =
            b"; ModuleID = 'capture-test'\ndefine amdgpu_kernel void @kernel() { ret void }\n";
        let handoff = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target,
            CodeObjectVersion::V6,
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                .unwrap(),
            CompilerModuleSymbolManifestV1::new([
                (Role::KernelEntry, "kernel"),
                (Role::KernelDescriptor, "kernel.kd"),
            ])
            .unwrap(),
            llvm,
        )
        .unwrap();
        let metadata = serde_json::json!({
            "schema": "EngineeringDiagnosticCandidateV1",
            "encoding": "commit-bound-target-kir-debug-not-canonical-replay",
            "target": "gfx950:xnack-", "authority": false,
            "kir": {"sha256": hex(&Sha256::digest(b"kir")), "byte_len": 3},
            "llvm": {"sha256": hex(&Sha256::digest(llvm)), "byte_len": llvm.len()}
        });
        write_new_file(
            &scratch.path.join("diagnostic-candidate.json"),
            &serde_json::to_vec(&metadata).unwrap(),
            0o600,
        )
        .unwrap();
        write_new_file(
            &scratch.path.join("diagnostic-target-kir.txt"),
            b"kir",
            0o600,
        )
        .unwrap();
        write_new_file(&scratch.path.join("diagnostic-pre-worker.ll"), llvm, 0o600).unwrap();
        let mut observation = serde_json::json!({"target": "gfx950:xnack-", "compiler_handoff": identity(ContentIdentityV1::calculate(handoff.canonical_bytes()))});
        let manifest = serde_json::to_vec(&observation).unwrap();
        let capture = read_capture(
            &scratch.path,
            handoff.canonical_bytes(),
            &manifest,
            "gfx950:xnack-",
        )
        .unwrap();
        assert_eq!(capture.llvm, llvm);
        let root = scratch.path.join(CAPTURE_NAMESPACE);
        let (sha256, bytes) = retain(
            &root,
            &scratch.path.join(NAMESPACE),
            &scratch.path,
            handoff.canonical_bytes(),
            &manifest,
            "gfx950:xnack-",
        )
        .unwrap()
        .unwrap();
        let receipt = fs::read(root.join("capture.json")).unwrap();
        assert_eq!(sha256, hex(&Sha256::digest(&receipt)));
        assert_eq!(bytes, receipt.len() as u64);
        assert_eq!(
            fs::read(root.join("compiler-handoff-v2")).unwrap(),
            handoff.canonical_bytes()
        );
        observation["compiler_handoff"]["sha256"] = serde_json::Value::String("00".repeat(32));
        assert!(
            read_capture(
                &scratch.path,
                handoff.canonical_bytes(),
                &serde_json::to_vec(&observation).unwrap(),
                "gfx950:xnack-"
            )
            .is_err()
        );
    }

    #[test]
    fn engineering_capture_root_is_fresh_canonical_sibling() {
        let scratch = ScratchDirectory::new().unwrap();
        let observation = scratch.path.join(NAMESPACE);
        let root = scratch.path.join(CAPTURE_NAMESPACE);
        assert!(validate_root(&root, &observation).is_ok());
        assert!(validate_root(&scratch.path.join("other"), &observation).is_err());
        assert!(validate_root(&root, &scratch.path.join("nested").join(NAMESPACE)).is_err());
        std::os::unix::fs::symlink("missing", &root).unwrap();
        assert!(validate_root(&root, &observation).is_err());
    }

    #[test]
    fn engineering_capture_missing_candidate_is_explicitly_omitted() {
        let scratch = ScratchDirectory::new().unwrap();
        let root = scratch.path.join(CAPTURE_NAMESPACE);
        assert!(
            retain(
                &root,
                &scratch.path.join(NAMESPACE),
                &scratch.path,
                b"invalid handoff",
                b"{}",
                "gfx950:xnack-"
            )
            .unwrap()
            .is_none()
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("capture.json")).unwrap()).unwrap();
        assert_eq!(receipt["state"], "omitted-ineligible");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert!(validate_root(&root, &scratch.path.join(NAMESPACE)).is_err());
    }

    #[test]
    fn engineering_capture_publication_failure_has_no_complete_marker() {
        let scratch = ScratchDirectory::new().unwrap();
        let root = scratch.path.join(CAPTURE_NAMESPACE);
        let capture = Captured {
            kir: b"kir".to_vec(),
            llvm: b"llvm".to_vec(),
        };
        assert!(
            publish(
                &root,
                b"handoff",
                b"manifest",
                "gfx950:xnack-",
                Some(&capture),
                PublishFault::AfterPayload
            )
            .is_err()
        );
        assert!(!root.join("capture.json").exists());
    }

    #[test]
    fn engineering_capture_late_failure_cannot_return_completion_acknowledgement() {
        let scratch = ScratchDirectory::new().unwrap();
        let root = scratch.path.join(CAPTURE_NAMESPACE);
        let capture = Captured {
            kir: b"kir".to_vec(),
            llvm: b"llvm".to_vec(),
        };
        assert!(
            publish(
                &root,
                b"handoff",
                b"manifest",
                "gfx950:xnack-",
                Some(&capture),
                PublishFault::AfterMarker
            )
            .is_err()
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("capture.json")).unwrap()).unwrap();
        assert_eq!(receipt["state"], "payload-complete");
        assert_eq!(receipt["requires_cli_completion_acknowledgement"], true);
    }

    fn observation_capture_call_order(source: &str) -> Vec<&'static str> {
        #[derive(Default)]
        struct Calls(Vec<&'static str>);

        impl<'ast> syn::visit::Visit<'ast> for Calls {
            fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
                if let syn::Expr::Path(path) = call.func.as_ref() {
                    if path.qself.is_none() && path.path.is_ident("publish_observation") {
                        self.0.push("publish_observation");
                    } else if path.qself.is_none()
                        && path.path.leading_colon.is_none()
                        && path.path.segments.len() == 3
                        && path.path.segments[0].ident == "support"
                        && path.path.segments[1].ident == "diagnostic_capture"
                        && path.path.segments[2].ident == "retain"
                    {
                        self.0.push("support::diagnostic_capture::retain");
                    }
                }
                syn::visit::visit_expr_call(self, call);
            }
        }

        let parsed = syn::parse_file(source).unwrap();
        let mut functions = parsed.items.iter().filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "run" => Some(function),
            _ => None,
        });
        let run = functions.next().unwrap();
        assert!(functions.next().is_none());
        let mut calls = Calls::default();
        syn::visit::Visit::visit_block(&mut calls, &run.block);
        calls.0
    }

    #[test]
    fn engineering_capture_preserves_environment_and_normal_observation_path() {
        let source = include_str!("../../engineering_hsaco.rs");
        let execution = include_str!("../execution.rs");
        assert!(source.contains("name.starts_with(\"FE2O3_EXTRACT_\")"));
        assert!(execution.contains(".env_clear()"));
        assert!(execution.contains("if options.diagnostic_capture_root.is_some()"));
        let expected = ["publish_observation", "support::diagnostic_capture::retain"];
        assert_eq!(observation_capture_call_order(source), expected);
        for fixture in [
            "fn run() { let published = publish_observation()?; support::diagnostic_capture::retain(); }",
            "fn run() { let published =\n publish_observation()?;\n support::diagnostic_capture::retain(); }",
            "fn unrelated() { support::diagnostic_capture::retain(); } fn run() { publish_observation(); support::diagnostic_capture::retain(); }",
        ] {
            assert_eq!(observation_capture_call_order(fixture), expected);
        }
        for fixture in [
            "fn run() { support::diagnostic_capture::retain(); publish_observation(); }",
            "fn run() { publish_observation(); } fn unrelated() { support::diagnostic_capture::retain(); }",
            "fn run() { publish_observation(); support::diagnostic_capture::retain(); support::diagnostic_capture::retain(); }",
        ] {
            assert_ne!(observation_capture_call_order(fixture), expected);
        }
        assert!(source.contains("Ok(Some((sha256, byte_len))) =>"));
        assert!(
            source
                .contains("FE2O3_ENGINEERING_CAPTURE_COMPLETE_V1 sha256={sha256} bytes={byte_len}")
        );
    }
}
