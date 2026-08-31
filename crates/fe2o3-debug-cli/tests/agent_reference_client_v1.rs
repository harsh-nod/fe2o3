use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_debug_cli::reference_archive_v1::*;
use fe2o3_kernel_ir::*;
use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use rmpv::{Value, encode::write_value};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};

const ELF_HEADER_BYTES: usize = 64;
const SECTION_HEADER_BYTES: usize = 64;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fe2o3-agent-reference-{label}-{}",
        std::process::id()
    ))
}

fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
}

fn install_executable(source: &Path, destination: &Path) {
    fs::copy(source, destination).unwrap();
    fs::set_permissions(destination, fs::Permissions::from_mode(0o700)).unwrap();
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn operation(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn divergent_barrier_module() -> Module {
    let barrier = WorkgroupBarrier {
        memory_scope: SynchronizationScope::Workgroup,
        semantics: BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]),
        convergence: Convergence::uniform(SynchronizationScope::Workgroup),
    };
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        operation(
            1,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        operation(2, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        operation(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exits = BasicBlock::new(BlockId(1));
    exits.terminator = Some(Terminator::Return { values: vec![] });
    let mut waits = BasicBlock::new(BlockId(2));
    waits.operations.push(Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(barrier),
    ));
    waits.terminator = Some(Terminator::Return { values: vec![] });
    let capability = TargetCapability::WorkgroupBarrier;
    let mut function = Function::kernel_entry(
        "divergent_barrier_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, exits, waits],
    );
    function.required_capabilities.insert(capability.clone());
    let mut kernel = Kernel::new(
        "divergent_barrier",
        "divergent_barrier_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.required_capabilities.insert(capability.clone());
    let mut module = Module::new("agent-reference::divergent-barrier");
    module.required_capabilities.insert(capability);
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn content(byte: u8) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: 64,
    }
}

fn opaque(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn source(first_end: u64, second_end: u64) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "rocprofiler-sdk-tool": [{
            "metadata": {"pid": 7},
            "buffer_records": {"kernel_dispatch": [
                {
                    "start_timestamp": 100,
                    "end_timestamp": first_end,
                    "dispatch_info": {
                        "agent_id": {"handle": 17},
                        "dispatch_id": 1,
                        "workgroup_size": {"x": 64, "y": 1, "z": 1},
                        "grid_size": {"x": 256, "y": 1, "z": 1}
                    }
                },
                {
                    "start_timestamp": 200,
                    "end_timestamp": second_end,
                    "dispatch_info": {
                        "agent_id": {"handle": 17},
                        "dispatch_id": 2,
                        "workgroup_size": {"x": 32, "y": 1, "z": 1},
                        "grid_size": {"x": 128, "y": 1, "z": 1}
                    }
                }
            ]},
            "callback_records": {},
            "counters": []
        }]
    }))
    .unwrap()
}

fn profiler_binding(artifact: &[u8], kernel_ir: u8) -> ProfilerDispatchBindingV4 {
    ProfilerDispatchBindingV4 {
        environment: ProfilerEnvironmentBindingV4 {
            environment: content(10),
            collector_tool: content(11),
            collector_configuration: content(12),
            stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                source_agent_id: 17,
                stable_identity: content(13),
            }],
        },
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(kernel_ir), 97)
            .unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: OpaqueIdentityV1::new(Sha256::digest(artifact).into()).unwrap(),
            canonical_len: artifact.len() as u64,
            format_version: 1,
        }),
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    }
}

struct Treatment {
    manifest: Vec<u8>,
    workload: Vec<u8>,
    source: Vec<u8>,
    bundle: Vec<u8>,
    schedule: Vec<u8>,
    artifact: Vec<u8>,
    isa: Vec<u8>,
}

fn treatment(
    workload: &[u8],
    source: Vec<u8>,
    artifact: Vec<u8>,
    kernel_ir: u8,
    schedule: &[u8],
    isa: &[u8],
) -> Treatment {
    let binding = profiler_binding(&artifact, kernel_ir);
    let bundle = encode_profiler_bundle_v4(
        &import_rocprofv3_json_profiler_bundle_v4(&source, binding).unwrap(),
    )
    .unwrap();
    let manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
        semantic_workload: workload,
        raw_profiler_source: &source,
        bundle: &bundle,
        schedule,
        artifact: &artifact,
        kernel_ordinal: 0,
        isa_projection: Some(isa),
        counters: None,
        pc_samples: None,
    })
    .unwrap();
    Treatment {
        manifest,
        workload: workload.to_vec(),
        source,
        bundle,
        schedule: schedule.to_vec(),
        artifact,
        isa: isa.to_vec(),
    }
}

fn write_treatment(root: &Path, name: &str, treatment: &Treatment) -> JsonValue {
    let manifest = root.join(format!("{name}.manifest"));
    let workload = root.join(format!("{name}.workload"));
    let source = root.join(format!("{name}.raw.json"));
    let bundle = root.join(format!("{name}.bundle"));
    let schedule = root.join(format!("{name}.schedule"));
    let artifact = root.join(format!("{name}.hsaco"));
    let isa = root.join(format!("{name}.isa"));
    for (path, bytes) in [
        (&manifest, treatment.manifest.as_slice()),
        (&workload, treatment.workload.as_slice()),
        (&source, treatment.source.as_slice()),
        (&bundle, treatment.bundle.as_slice()),
        (&schedule, treatment.schedule.as_slice()),
        (&artifact, treatment.artifact.as_slice()),
        (&isa, treatment.isa.as_slice()),
    ] {
        write(path, bytes);
    }
    json!({
        "manifest": manifest,
        "semantic_workload": workload,
        "raw_profiler_source": source,
        "bundle": bundle,
        "schedule": schedule,
        "artifact": artifact,
        "isa_projection": isa,
        "counters": null,
        "pc_samples": null,
    })
}

#[test]
fn fresh_process_client_completes_three_diagnoses_and_minimum_plan() {
    let temp = temp_root("acceptance");
    fs::create_dir(&temp).unwrap();
    let oob_request = temp.join("oob-request.json");
    let oob_request_bytes = br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#;
    write(&oob_request, oob_request_bytes);
    let barrier_kernel = temp.join("barrier.kir");
    let barrier_kernel_bytes = VerifiedCanonicalKernelIrV7::from_module(divergent_barrier_module())
        .unwrap()
        .canonical_bytes()
        .to_vec();
    write(&barrier_kernel, &barrier_kernel_bytes);
    let barrier_request = temp.join("barrier-request.json");
    write(
        &barrier_request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"divergent_barrier","grid":[2,1,1],"workgroup":[2,1,1],"arguments":[]}"#,
    );
    let workload = br#"{"kernel":"generic","shape":[256,2,1]}"#;
    let baseline = treatment(
        workload,
        source(140, 260),
        hsaco(7, 0),
        1,
        b"schedule-v1",
        b"isa-v1",
    );
    let candidate = treatment(
        workload,
        source(170, 310),
        hsaco(11, 2),
        2,
        b"schedule-v2",
        b"isa-v2",
    );
    let debugger = temp.join("fe2o3-debug");
    let profiler_service = temp.join("fe2o3-agent-profiler-service");
    install_executable(Path::new(env!("CARGO_BIN_EXE_fe2o3-debug")), &debugger);
    install_executable(
        Path::new(env!("CARGO_BIN_EXE_fe2o3-agent-profiler-service")),
        &profiler_service,
    );
    let workflow_path = temp.join("workflow.json");
    write(
        &workflow_path,
        &serde_json::to_vec(&json!({
            "schema": "fe2o3-agent-reference-workflow-v1",
            "trusted_debugger_executable": debugger,
            "trusted_profiler_service_executable": profiler_service,
            "out_of_bounds": {
                "kernel": workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"),
                "request": oob_request,
            },
            "barrier_divergence": {
                "kernel": barrier_kernel,
                "request": barrier_request,
            },
            "baseline": write_treatment(&temp, "baseline", &baseline),
            "candidate": write_treatment(&temp, "candidate", &candidate),
        }))
        .unwrap(),
    );
    let forbidden_temp = temp.join("must-not-create-debugger-snapshots");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_fe2o3-agent-reference-client"))
            .arg("--workflow")
            .arg(&workflow_path)
            .env("TMPDIR", &forbidden_temp)
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "client failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    assert!(!forbidden_temp.exists());
    assert_eq!(first.stdout, second.stdout);
    let report: JsonValue = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["schema"], "fe2o3-agent-reference-report-v1");
    assert_eq!(report["out_of_bounds"]["class"], "memory_out_of_bounds");
    assert_eq!(
        report["barrier_divergence"]["class"],
        "workgroup_barrier_divergence"
    );
    assert_eq!(report["out_of_bounds"]["simulated"], true);
    assert_eq!(report["out_of_bounds"]["hardware_observed"], false);
    assert!(report["out_of_bounds"]["citation_count"].as_u64().unwrap() > 0);
    assert!(
        report["barrier_divergence"]["citation_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        report["out_of_bounds"]["claim_truth"],
        "inferred_from_simulated_semantic_trace"
    );
    assert_eq!(
        report["out_of_bounds"]["diagnosis"]["context"]["workgroup"]["origin"],
        "observed"
    );
    assert_eq!(
        report["out_of_bounds"]["diagnosis"]["memory_region"]["origin"],
        "observed"
    );
    assert_eq!(
        report["barrier_divergence"]["diagnosis"]["barrier"]["origin"],
        "observed"
    );
    for case in ["out_of_bounds", "barrier_divergence"] {
        assert_eq!(
            report[case]["citations"],
            report[case]["diagnosis"]["evidence"]["citations"]
        );
        for citation in report[case]["citations"].as_array().unwrap() {
            assert!(!citation["field"].as_str().unwrap().is_empty());
            assert_eq!(
                citation["source_record_identity"].as_str().unwrap().len(),
                64
            );
            assert_eq!(citation["claim_identity"].as_str().unwrap().len(), 64);
        }
    }
    assert!(
        !report["variant"]["ranked_explanations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        report["variant"]["claim_truth"],
        "conservative_co_observation_not_causal_attribution"
    );
    assert_eq!(report["next_capture"]["first_page_returned"], 1);
    assert_eq!(report["next_capture"]["second_page_returned"], 1);
    assert_eq!(
        report["next_capture"]["claim_truth"],
        "inferred_minimum_next_capture_plan"
    );
    assert!(
        !report["next_capture"]["evidence"]["captures"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !report["next_capture"]["evidence"]["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        report["authority"],
        "read_only_no_execution_attach_scheduling_or_collection_authority"
    );
    for executable in ["debugger", "profiler_service"] {
        assert_eq!(
            report["executable_manifest"][executable]["scheme"],
            "sha256_of_exact_executable_bytes"
        );
        assert_eq!(
            report["executable_manifest"][executable]["sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert!(
            report["executable_manifest"][executable]["bytes"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    let text = String::from_utf8(first.stdout).unwrap();
    for forbidden in ["/dev/kfd", "pid", "native_address", "attach_process"] {
        assert!(!text.contains(forbidden));
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn installed_fresh_process_uses_only_pinned_archive_and_production_binaries() {
    let temp = temp_root("archive-acceptance");
    fs::create_dir(&temp).unwrap();
    let debugger = temp.join("fe2o3-debug");
    let profiler_service = temp.join("fe2o3-agent-profiler-service");
    let reference_client = temp.join("fe2o3-agent-reference-client");
    let archive_path = temp.join("evidence.fe2archive");
    install_executable(Path::new(env!("CARGO_BIN_EXE_fe2o3-debug")), &debugger);
    install_executable(
        Path::new(env!("CARGO_BIN_EXE_fe2o3-agent-profiler-service")),
        &profiler_service,
    );
    install_executable(
        Path::new(env!("CARGO_BIN_EXE_fe2o3-agent-reference-client")),
        &reference_client,
    );

    let oob_kernel = include_bytes!("../../fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir");
    let oob_request = br#"{"schema":"fe2o3-simulation-request-v1","kernel":"fill","grid":[4,1,1],"workgroup":[64,1,1],"arguments":[{"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":"0x00000000"}]}"#;
    let barrier_kernel = VerifiedCanonicalKernelIrV7::from_module(divergent_barrier_module())
        .unwrap()
        .canonical_bytes()
        .to_vec();
    let barrier_request = br#"{"schema":"fe2o3-simulation-request-v1","kernel":"divergent_barrier","grid":[2,1,1],"workgroup":[2,1,1],"arguments":[]}"#;
    let workload = br#"{"kernel":"generic","shape":[256,2,1]}"#;
    let baseline = treatment(
        workload,
        source(140, 260),
        hsaco(7, 0),
        1,
        b"schedule-v1",
        b"isa-v1",
    );
    let candidate = treatment(
        workload,
        source(170, 310),
        hsaco(11, 2),
        2,
        b"schedule-v2",
        b"isa-v2",
    );
    let archive = encode_reference_evidence_archive_v1(ReferenceEvidenceArchiveInputV1 {
        out_of_bounds: ReferenceSimulatorCaseInputV1 {
            kernel: oob_kernel,
            request: oob_request,
        },
        barrier_divergence: ReferenceSimulatorCaseInputV1 {
            kernel: &barrier_kernel,
            request: barrier_request,
        },
        baseline: ReferenceTreatmentInputV1 {
            manifest: &baseline.manifest,
            semantic_workload: &baseline.workload,
            raw_profiler_source: &baseline.source,
            bundle: &baseline.bundle,
            schedule: &baseline.schedule,
            artifact: &baseline.artifact,
            isa_projection: Some(&baseline.isa),
            counters: None,
            pc_samples: None,
        },
        candidate: ReferenceTreatmentInputV1 {
            manifest: &candidate.manifest,
            semantic_workload: &candidate.workload,
            raw_profiler_source: &candidate.source,
            bundle: &candidate.bundle,
            schedule: &candidate.schedule,
            artifact: &candidate.artifact,
            isa_projection: Some(&candidate.isa),
            counters: None,
            pc_samples: None,
        },
    })
    .unwrap();
    let digest = lower_hex(&reference_evidence_archive_sha256_v1(&archive));
    write(&archive_path, &archive);

    let staged = fs::read_dir(&temp)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        staged,
        [
            "evidence.fe2archive",
            "fe2o3-agent-profiler-service",
            "fe2o3-agent-reference-client",
            "fe2o3-debug",
        ]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect::<std::collections::BTreeSet<_>>()
    );

    let run = || {
        Command::new(&reference_client)
            .args(["--archive", "evidence.fe2archive", "--archive-sha256"])
            .arg(&digest)
            .args([
                "--debugger",
                "fe2o3-debug",
                "--profiler-service",
                "fe2o3-agent-profiler-service",
            ])
            .current_dir(&temp)
            .env_clear()
            .env("PATH", "/hostile/path")
            .env("LD_LIBRARY_PATH", "/hostile/loader")
            .env("LANG", "hostile_LOCALE")
            .env("TMPDIR", temp.join("must-not-exist"))
            .env("ROCM_PATH", "/hostile/rocm")
            .env("ASAN_OPTIONS", "hostile=1")
            .env("FE2O3_HOSTILE", "1")
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "archive client failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    assert!(!temp.join("must-not-exist").exists());
    let report: JsonValue = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        report["schema"],
        REFERENCE_EVIDENCE_ARCHIVE_REPORT_SCHEMA_V1
    );
    assert_eq!(
        report["archive_schema"],
        REFERENCE_EVIDENCE_ARCHIVE_SCHEMA_V1
    );
    assert_eq!(report["archive"]["sha256"], digest);
    assert_eq!(report["archive"]["bytes"], archive.len() as u64);
    assert_eq!(report["members"].as_array().unwrap().len(), 18);
    let member = |role: &str| {
        report["members"]
            .as_array()
            .unwrap()
            .iter()
            .find(|member| member["role"] == role)
            .unwrap()
    };
    for (role, bytes, case) in [
        (
            "out-of-bounds/kernel.kir-v7",
            oob_kernel.as_slice(),
            "out_of_bounds",
        ),
        (
            "out-of-bounds/request.json",
            oob_request.as_slice(),
            "out_of_bounds",
        ),
        (
            "barrier/kernel.kir-v7",
            barrier_kernel.as_slice(),
            "barrier_divergence",
        ),
        (
            "barrier/request.json",
            barrier_request.as_slice(),
            "barrier_divergence",
        ),
    ] {
        let raw_member_sha256 = lower_hex(&<[u8; 32]>::from(sha2::Sha256::digest(bytes)));
        assert_eq!(member(role)["sha256"], raw_member_sha256);
        assert_eq!(member(role)["bytes"], bytes.len() as u64);
        let (admitted, admitted_sha256) = if role.ends_with("kernel.kir-v7") {
            let verified = VerifiedCanonicalKernelIrV7::from_canonical_bytes(bytes.to_vec())
                .expect("archive kernel member is canonical KIR V7");
            (
                &report["workflow"][case]["diagnosis"]["input"]["canonical_kir_v7"]["value"],
                lower_hex(verified.identity().digest()),
            )
        } else {
            (
                &report["workflow"][case]["diagnosis"]["input"]["dispatch_request"]["value"],
                raw_member_sha256.clone(),
            )
        };
        if role.ends_with("kernel.kir-v7") {
            assert_ne!(admitted_sha256, raw_member_sha256);
        }
        assert_eq!(admitted["sha256"], admitted_sha256);
        assert_eq!(admitted["canonical_bytes"], bytes.len() as u64);
    }
    assert_eq!(
        report["authority"],
        "read_only_no_execution_attach_scheduling_or_collection_authority"
    );
    assert_eq!(
        report["workflow"]["authority"],
        "read_only_no_execution_attach_scheduling_or_collection_authority"
    );
    assert_eq!(
        report["workflow"]["out_of_bounds"]["class"],
        "memory_out_of_bounds"
    );
    assert_eq!(
        report["workflow"]["barrier_divergence"]["class"],
        "workgroup_barrier_divergence"
    );
    for case in ["out_of_bounds", "barrier_divergence"] {
        let diagnosis = &report["workflow"][case];
        assert_eq!(
            diagnosis["citations"],
            diagnosis["diagnosis"]["evidence"]["citations"]
        );
        assert!(!diagnosis["citations"].as_array().unwrap().is_empty());
    }
    assert!(
        !report["workflow"]["variant"]["ranked_explanations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        report["workflow"]["next_capture"]["minimum_additional_captures"],
        1
    );
    let text = String::from_utf8(first.stdout).unwrap();
    assert!(!text.contains(workspace_root().to_str().unwrap()));
    for forbidden in ["/dev/kfd", "attach_process", "native_address"] {
        assert!(!text.contains(forbidden));
    }
    for private_transport in [
        "/proc/self/fd/",
        "fe2o3-reference-executable-v1",
        "fe2o3-reference-debug-input-v1",
    ] {
        assert!(!text.contains(private_transport));
    }
    fs::remove_dir_all(temp).unwrap();
}

#[cfg(unix)]
#[test]
fn archive_path_links_wrong_pin_and_oversize_are_rejected_before_execution() {
    use std::os::unix::fs::symlink;

    let temp = temp_root("archive-path-hostile");
    fs::create_dir(&temp).unwrap();
    let archive = encode_reference_evidence_archive_v1(ReferenceEvidenceArchiveInputV1 {
        out_of_bounds: ReferenceSimulatorCaseInputV1 {
            kernel: b"ok",
            request: b"or",
        },
        barrier_divergence: ReferenceSimulatorCaseInputV1 {
            kernel: b"bk",
            request: b"br",
        },
        baseline: ReferenceTreatmentInputV1 {
            manifest: b"m",
            semantic_workload: b"w",
            raw_profiler_source: b"r",
            bundle: b"b",
            schedule: b"s",
            artifact: b"a",
            isa_projection: None,
            counters: None,
            pc_samples: None,
        },
        candidate: ReferenceTreatmentInputV1 {
            manifest: b"M",
            semantic_workload: b"W",
            raw_profiler_source: b"R",
            bundle: b"B",
            schedule: b"S",
            artifact: b"A",
            isa_projection: None,
            counters: None,
            pc_samples: None,
        },
    })
    .unwrap();
    let target = temp.join("target.fe2archive");
    write(&target, &archive);
    let client = env!("CARGO_BIN_EXE_fe2o3-agent-reference-client");
    let run = |path: &Path, digest: &str| {
        Command::new(client)
            .arg("--archive")
            .arg(path)
            .args([
                "--archive-sha256",
                digest,
                "--debugger",
                "must-not-open-debugger",
                "--profiler-service",
                "must-not-open-profiler",
            ])
            .output()
            .unwrap()
    };

    let wrong_pin = run(&target, &"0".repeat(64));
    assert!(!wrong_pin.status.success());
    assert!(
        String::from_utf8(wrong_pin.stderr)
            .unwrap()
            .contains("identity mismatch")
    );

    let link = temp.join("link.fe2archive");
    symlink(&target, &link).unwrap();
    let linked = run(
        &link,
        &lower_hex(&reference_evidence_archive_sha256_v1(&archive)),
    );
    assert!(!linked.status.success());
    assert!(
        String::from_utf8(linked.stderr)
            .unwrap()
            .contains("securely open reference evidence archive")
    );

    let hard_link = temp.join("hard.fe2archive");
    fs::hard_link(&target, &hard_link).unwrap();
    let linked = run(
        &hard_link,
        &lower_hex(&reference_evidence_archive_sha256_v1(&archive)),
    );
    assert!(!linked.status.success());
    assert!(
        String::from_utf8(linked.stderr)
            .unwrap()
            .contains("bounded regular file")
    );
    fs::remove_file(&link).unwrap();
    fs::remove_file(&hard_link).unwrap();

    let oversized = temp.join("oversized.fe2archive");
    let file = fs::File::create(&oversized).unwrap();
    file.set_len(MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1 + 1)
        .unwrap();
    drop(file);
    let oversized = run(&oversized, &"0".repeat(64));
    assert!(!oversized.status.success());
    assert!(
        String::from_utf8(oversized.stderr)
            .unwrap()
            .contains("bounded regular file")
    );
    fs::remove_dir_all(temp).unwrap();
}

#[cfg(unix)]
#[test]
fn workflow_symlinks_and_hard_links_are_rejected_before_any_child_session() {
    use std::os::unix::fs::symlink;

    let temp = temp_root("symlink");
    fs::create_dir(&temp).unwrap();
    let target = temp.join("target.json");
    write(&target, b"{}");
    let link = temp.join("workflow.json");
    symlink(&target, &link).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-agent-reference-client"))
        .arg("--workflow")
        .arg(&link)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("securely open workflow")
    );
    assert!(output.stdout.is_empty());

    let hard_link = temp.join("hard-linked-workflow.json");
    fs::hard_link(&target, &hard_link).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-agent-reference-client"))
        .arg("--workflow")
        .arg(&hard_link)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("bounded regular file")
    );
    assert!(output.stdout.is_empty());
    fs::remove_dir_all(temp).unwrap();
}

fn hsaco(vgpr_count: u64, spill_count: u64) -> Vec<u8> {
    let kernel = map(vec![
        (".name", Value::from("generic")),
        (".symbol", Value::from("generic.kd")),
        (".kernarg_segment_size", Value::from(0)),
        (".kernarg_segment_align", Value::from(8)),
        (".group_segment_fixed_size", Value::from(0)),
        (".private_segment_fixed_size", Value::from(16)),
        (".wavefront_size", Value::from(64)),
        (".sgpr_count", Value::from(14)),
        (".vgpr_count", Value::from(vgpr_count)),
        (".agpr_count", Value::from(3)),
        (".sgpr_spill_count", Value::from(spill_count)),
        (".vgpr_spill_count", Value::from(4)),
        (".workgroup_processor_mode", Value::from(1)),
        (".max_flat_workgroup_size", Value::from(1024)),
    ]);
    let metadata = map(vec![
        (
            "amdhsa.version",
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        ("amdhsa.target", Value::from("amdgcn-amd-amdhsa--gfx1151")),
        ("amdhsa.kernels", Value::Array(vec![kernel])),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &metadata).unwrap();
    elf_with_metadata(&encoded)
}

fn map(fields: Vec<(&str, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(key, value)| (Value::from(key), value))
            .collect(),
    )
}

fn elf_with_metadata(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
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
    write_u64(&mut bytes, 40, section_offset as u64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);
    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);
    let strings_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strings_header, 7);
    write_u32(&mut bytes, strings_header + 4, 3);
    write_u64(&mut bytes, strings_header + 24, string_table_offset as u64);
    write_u64(&mut bytes, strings_header + 32, string_table.len() as u64);
    write_u64(&mut bytes, strings_header + 48, 1);
    bytes
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let remainder = bytes.len() % alignment;
    if remainder != 0 {
        bytes.resize(bytes.len() + alignment - remainder, 0);
    }
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
