use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    let substituted = Command::new(env!("CARGO_BIN_EXE_fe2o3-agent-reference-client"))
        .arg("--workflow")
        .arg(&workflow_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let snapshot =
        std::env::temp_dir().join(format!("fe2o3-agent-reference-{}-2.json", substituted.id()));
    let replacement = temp.join("substituted-snapshot.json");
    let mut changed_request = oob_request_bytes.to_vec();
    changed_request.push(b'\n');
    write(&replacement, &changed_request);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !snapshot.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        snapshot.exists(),
        "client did not expose its bounded snapshot"
    );
    fs::rename(&replacement, &snapshot).unwrap();
    let substituted = substituted.wait_with_output().unwrap();
    assert!(!substituted.status.success());
    assert!(substituted.stdout.is_empty());
    assert!(
        String::from_utf8(substituted.stderr)
            .unwrap()
            .contains("debugger did not use the exact loaded request bytes")
    );

    let run = || {
        Command::new(env!("CARGO_BIN_EXE_fe2o3-agent-reference-client"))
            .arg("--workflow")
            .arg(&workflow_path)
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
