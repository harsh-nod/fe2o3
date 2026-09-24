//! Inert shared canonical fixtures, not rustc source or native qualification.
use super::*;
use fe2o3_kernel_ir as physical_entry_fixture_ir;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    Gfx942PhysicalEntryOpcodeV20 as Opcode, OperationKind, encode_module_v20,
};
use fe2o3_kir_sim::SimulationDebugCheckpointPhaseV1 as Phase;
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_entry_v20.rs"]
mod fixture;
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Files {
    directory: PathBuf,
    kir: PathBuf,
    request: PathBuf,
}
impl Files {
    fn new(select: bool, selector: u32, edited: bool) -> Self {
        let mut module = fixture::module(select);
        if edited {
            for operation in module.functions[0]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .flat_map(|b| &mut b.operations)
            {
                if let OperationKind::Gfx942PhysicalEntryStep(step) = &mut operation.kind {
                    match step.instruction.opcode {
                        Opcode::VectorMove32 if step.instruction.destination == 8 => {
                            step.instruction.destination = 22
                        }
                        Opcode::GlobalStoreDword => step.instruction.source1 = 22,
                        _ => {}
                    }
                }
            }
        }
        let directory = std::env::temp_dir().join(format!(
            "fe2o3-v20-debug-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let kir = directory.join("canonical.kir");
        let request = directory.join("request.json");
        std::fs::write(&kir, encode_module_v20(&module).unwrap()).unwrap();
        let document = serde_json::json!({
         "schema":"fe2o3-simulation-request-v1","kernel":"physical_fixture",
         "grid":[64,1,1],"workgroup":[64,1,1],
         "arguments":[
          {"kind":"buffer","element":"u32","access":"read_write","alignment":4,"bytes":format!("0x{}","a5".repeat(256)),"initialized":format!("0x{}","00".repeat(32))},
          {"kind":"scalar","type":"u32","bits":"0x00000013"},
          {"kind":"scalar","type":"u32","bits":"0x00000017"},
          {"kind":"scalar","type":"u32","bits":"0x0000002a"},
          {"kind":"scalar","type":"u32","bits":format!("0x{selector:08x}")}
         ]
        });
        std::fs::write(&request, serde_json::to_vec(&document).unwrap()).unwrap();
        Self {
            directory,
            kir,
            request,
        }
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.kir);
        let _ = std::fs::remove_file(&self.request);
        let _ = std::fs::remove_dir(&self.directory);
    }
}
fn backend(files: &Files) -> (Input, Backend) {
    let mut ledger = Owned::new(Work::new(WORK), STORAGE);
    ledger
        .with_budget(|b| {
            b.reserve_storage(73 + SCRATCH)?;
            b.charge_work(29 + RESPONSE * 2)
        })
        .unwrap();
    let (input, receipt) = ledger
        .with_budget(|b| load_physical_entry_debug_input_v20(&files.kir, &files.request, b))
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    let backend = capture(&input, ledger).unwrap();
    (input, backend)
}
fn response(backend: &mut Backend, request: DebugRequestV1) -> DebugResponseV1 {
    backend.session.charge_query_work(QUERY_WORK).unwrap();
    let mut bytes = Vec::new();
    protocol::respond(backend, request, &mut bytes, protocol_limits()).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn seek(backend: &mut Backend, sequence: u64) -> DebugResponseV1 {
    let request = DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: backend.revision,
        cursor: DebugCursorV1 {
            configuration_identity: backend.configuration,
            event_sequence: sequence,
            state_revision: backend.revision,
        },
    };
    response(backend, request)
}
fn step(backend: &Backend, direction: StepDirectionV1, count: u32) -> DebugRequestV1 {
    DebugRequestV1::Step {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: backend.revision,
        direction,
        granularity: StepGranularityV1::Event,
        count,
        focus: None,
    }
}
fn scope(record: Record<'_>) -> ExecutionScopeSelectorV1 {
    ExecutionScopeSelectorV1::Lane {
        workgroup: record.invocation().workgroup.map(|n| n as u32),
        wave: 0,
        lane: record.invocation().local[0] as u16,
    }
}
fn inspect(backend: &Backend, page: PageRequestV1) -> DebugRequestV1 {
    DebugRequestV1::InspectValues {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: backend.revision,
        scope: scope(backend.session.current().unwrap()),
        frame: Some(1),
        selector: ValueSelectorV1::All,
        page,
    }
}
fn memory(backend: &mut Backend) -> DebugResponseV1 {
    let (allocation, _) = backend
        .session
        .current()
        .unwrap()
        .memory_allocation(0)
        .unwrap();
    let request = DebugRequestV1::ReadMemory {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: backend.revision,
        allocation: AllocationIdentityV1 {
            ordinal: allocation,
            generation: 0,
        },
        byte_offset: 0,
        byte_len: 4,
    };
    response(backend, request)
}
fn captured_bytes(response: DebugResponseV1) -> (String, String) {
    let DebugResponseV1::Ok { result, .. } = response else {
        panic!("memory reply")
    };
    let DebugResultV1::Memory { memory, .. } = *result else {
        panic!("memory result")
    };
    let MemoryAvailabilityV1::Captured {
        bytes, initialized, ..
    } = memory.availability
    else {
        panic!("captured memory")
    };
    (bytes, initialized)
}
#[test]
fn actual_cpu_one_diamond_and_register_edit_round_trip_memory() {
    for (select, selector, edited, expected) in [
        (false, 0, false, "0x13000000"),
        (true, 0, false, "0x13000000"),
        (true, 1, false, "0x17000000"),
        (false, 0, true, "0x13000000"),
    ] {
        let files = Files::new(select, selector, edited);
        let (_input, mut b) = backend(&files);
        assert_eq!(b.session.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
        assert!(b.session.capture_stop().is_none());
        let module = fixture::module(select);
        let (block,operation)=module.functions[0].body.as_ref().unwrap().blocks.iter().find_map(|block|block.operations.iter().enumerate().find_map(|(i,o)|
   matches!(&o.kind,OperationKind::Gfx942PhysicalEntryStep(s) if s.instruction.opcode==Opcode::GlobalStoreDword).then_some((block.id,i as u32)))).unwrap();
        let mut before = None;
        let mut after = None;
        for index in 0..b.session.records_len() {
            let record = b.session.record(index).unwrap();
            if record.site().block == block
                && record.site().operation == operation
                && record.invocation().global[0] == 0
            {
                match record.phase() {
                    Some(Phase::BeforeOperation) => before = Some(index),
                    Some(Phase::AfterOperation) => after = Some(index),
                    _ => {}
                }
            }
        }
        for (index, bytes, initialized) in [
            (before.unwrap(), "0xa5a5a5a5", "0x00"),
            (after.unwrap(), expected, "0x0f"),
            (before.unwrap(), "0xa5a5a5a5", "0x00"),
            (after.unwrap(), expected, "0x0f"),
        ] {
            assert!(matches!(
                seek(&mut b, index as u64 + 1),
                DebugResponseV1::Ok { .. }
            ));
            assert_eq!(
                captured_bytes(memory(&mut b)),
                (bytes.into(), initialized.into())
            );
        }
        assert!(!b.view().hardware_observed);
        assert!(b.view().simulated);
        assert!(!b.view().performance_prediction);
    }
}
#[test]
fn symbolic_ssa_bindings_are_present_but_never_numeric() {
    let files = Files::new(false, 0, false);
    let (_input, mut b) = backend(&files);
    let index = (0..b.session.records_len())
        .find(|&i| {
            let r = b.session.record(i).unwrap();
            r.phase().is_some()
                && (0..r.binding_count(0).unwrap_or(0))
                    .any(|j| r.binding(0, j).unwrap().symbolic_kind().is_some())
        })
        .unwrap();
    seek(&mut b, index as u64 + 1);
    let expected: Vec<_> = {
        let r = b.session.current().unwrap();
        (0..r.binding_count(0).unwrap())
            .filter_map(|i| {
                let value = r.binding(0, i).unwrap();
                value.symbolic_kind().map(|_| u64::from(value.value().0))
            })
            .collect()
    };
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: None,
            limit: 64,
        },
    );
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("values")
    };
    let DebugResultV1::Values { values, .. } = *result else {
        panic!("values")
    };
    for ordinal in expected {
        let value=values.iter().find(|v|matches!(v.path.root,ValueRootV1::Ssa{value_ordinal,..}if value_ordinal==ordinal)).unwrap();
        assert!(matches!(
            value.availability,
            ValueAvailabilityV1::Unavailable {
                reason: ValueUnavailableReasonV1::NotRepresented
            }
        ));
    }
}
#[test]
fn stale_cursor_revision_and_page_are_transactional_refusals() {
    let files = Files::new(false, 0, false);
    let (_input, mut b) = backend(&files);
    let index = (0..b.session.records_len())
        .find(|&i| {
            b.session.record(i).unwrap().phase().is_some()
                && b.session.record(i).unwrap().binding_count(0).unwrap_or(0) > 1
        })
        .unwrap();
    seek(&mut b, index as u64 + 1);
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: None,
            limit: 1,
        },
    );
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("values")
    };
    let DebugResultV1::Values {
        next_cursor: Some(page),
        ..
    } = *result
    else {
        panic!("page")
    };
    let old = b.view();
    seek(&mut b, index as u64 + 1);
    let current = b.view();
    let request = DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: b.revision,
        cursor: old.cursor,
    };
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Error { .. }
    ));
    assert_eq!(b.view(), current);
    let request = DebugRequestV1::GetState {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: old.revision,
    };
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Error { .. }
    ));
    assert_eq!(b.view(), current);
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: Some(page),
            limit: 1,
        },
    );
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Unavailable { .. }
    ));
    assert_eq!(b.view(), current);
}
#[test]
fn output_one_short_never_commits_cursor_or_revision() {
    let files = Files::new(false, 0, false);
    let (_input, mut baseline) = backend(&files);
    let index = (0..baseline.session.records_len())
        .find(|&i| baseline.session.record(i).unwrap().phase().is_some())
        .unwrap();
    let request = DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
        cursor: DebugCursorV1 {
            configuration_identity: baseline.configuration,
            event_sequence: index as u64 + 1,
            state_revision: 0,
        },
    };
    let mut encoded = Vec::new();
    baseline.session.charge_query_work(QUERY_WORK).unwrap();
    protocol::respond(
        &mut baseline,
        request.clone(),
        &mut encoded,
        protocol_limits(),
    )
    .unwrap();
    let exact = encoded.len();
    let (_input, mut limited) = backend(&files);
    let before = limited.view();
    let mut limits = protocol_limits();
    limits.max_response_line_bytes = exact - 1;
    let mut refused = Vec::new();
    limited.session.charge_query_work(QUERY_WORK).unwrap();
    let _ = protocol::respond(&mut limited, request, &mut refused, limits);
    assert_eq!(limited.view(), before);
    if !refused.is_empty() {
        assert!(matches!(
            serde_json::from_slice::<DebugResponseV1>(&refused).unwrap(),
            DebugResponseV1::Error { .. }
        ));
    }
}
#[test]
fn terminal_noop_library_is_free_but_cli_command_and_seek_are_charged() {
    let files = Files::new(false, 0, false);
    let (_input, mut b) = backend(&files);
    let end = b.session.records_len() as u64 + 1;
    seek(&mut b, end);
    let before = b.session.usage().work;
    assert_eq!(b.session.step_forward(), Navigation::End);
    assert_eq!(b.session.usage().work, before);
    let request = step(&b, StepDirectionV1::Forward, 1);
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("terminal")
    };
    let DebugResultV1::Control {
        events_advanced, ..
    } = *result
    else {
        panic!("control")
    };
    assert_eq!(events_advanced, 0);
    assert_eq!(b.session.usage().work, before + QUERY_WORK + 1);
    let request = step(&b, StepDirectionV1::Reverse, 1);
    response(&mut b, request);
    assert_eq!(b.sequence(), end - 1);
    seek(&mut b, 0);
    assert_eq!(b.session.cursor(), None);
}
#[test]
fn unsupported_capabilities_do_not_claim_source_hardware_or_replay() {
    let files = Files::new(false, 0, false);
    let (_input, mut b) = backend(&files);
    let caps = views::capabilities();
    for cap in [
        DebugCapabilityNameV1::RegisterValues,
        DebugCapabilityNameV1::HardwareWaveState,
        DebugCapabilityNameV1::SourceSites,
        DebugCapabilityNameV1::DeterministicReplay,
        DebugCapabilityNameV1::Breakpoints,
    ] {
        assert_eq!(
            caps.iter().find(|c| c.name == cap).unwrap().availability,
            CapabilityAvailabilityV1::Unavailable
        );
    }
    let mut request = step(&b, StepDirectionV1::Forward, 1);
    if let DebugRequestV1::Step { granularity, .. } = &mut request {
        *granularity = StepGranularityV1::Operation;
    }
    let before = b.view();
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Unavailable { .. }
    ));
    assert_eq!(b.view(), before);
}
#[test]
fn command_framing_schema_and_work_denial_keep_observation_unchanged() {
    let files = Files::new(false, 0, false);
    let (_input, mut b) = backend(&files);
    let before = b.view();
    let line=b"{\"schema\":\"fe2o3-debug-request-v2\",\"operation\":\"get_state\",\"request_id\":1,\"expected_revision\":0}\n";
    let mut output = Vec::new();
    assert!(protocol::run(&mut b, &mut &line[..], &mut output, protocol_limits()).is_err());
    assert_eq!(b.view(), before);
    assert!(matches!(
        serde_json::from_slice::<DebugResponseV1>(&output).unwrap(),
        DebugResponseV1::Error { .. }
    ));
    let used = b.session.usage().work;
    b.session.charge_query_work(WORK - used).unwrap();
    let line=b"{\"schema\":\"fe2o3-debug-request-v1\",\"operation\":\"get_state\",\"request_id\":1,\"expected_revision\":0}\n";
    let mut output = Vec::new();
    assert!(protocol::run(&mut b, &mut &line[..], &mut output, protocol_limits()).is_err());
    assert_eq!(b.view(), before);
    assert!(b.session.usage().failed_work.is_some());
}
#[test]
fn bounded_path_loader_exact_and_one_short_restore_prior_floor() {
    let files = Files::new(true, 1, false);
    let run = |wl, sl| {
        let mut work = Work::new(wl);
        let mut budget = Budget::new(&mut work, sl);
        budget.reserve_storage(73).unwrap();
        budget.charge_work(29).unwrap();
        let result = load_physical_entry_debug_input_v20(&files.kir, &files.request, &mut budget);
        let ok = result.is_ok();
        assert_eq!(budget.storage(), 73);
        if let Ok((input, receipt)) = result {
            assert_eq!(
                input.canonical().identity().digest(),
                input.module().identity().digest()
            );
            assert!(receipt.retained_storage() > 0);
        }
        let accepted = budget.work();
        let peak = budget.peak_storage();
        let failed_storage = budget.failed_storage();
        (ok, accepted, peak, work.failed_work(), failed_storage)
    };
    let (ok, w, p, _, _) = run(WORK, STORAGE);
    assert!(ok);
    assert!(run(w, p).0);
    let short = run(w - 1, p);
    assert!(!short.0);
    assert!(short.3.is_some());
    let short = run(w, p - 1);
    assert!(!short.0);
    assert!(short.4.is_some());
}
#[test]
fn loader_rejects_v21_unknown_request_fields_and_non_wave64() {
    let files = Files::new(false, 0, false);
    let mut bytes = std::fs::read(&files.kir).unwrap();
    bytes[8..10].copy_from_slice(&21u16.to_le_bytes());
    std::fs::write(&files.kir, bytes).unwrap();
    let refused = |files: &Files| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(73).unwrap();
        let result = load_physical_entry_debug_input_v20(&files.kir, &files.request, &mut budget);
        assert_eq!(budget.storage(), 73);
        assert!(result.is_err());
    };
    refused(&files);
    for mode in 0..2 {
        let files = Files::new(false, 0, false);
        let mut request: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&files.request).unwrap()).unwrap();
        if mode == 0 {
            request["source_authority"] = true.into();
        } else {
            request["workgroup"] = serde_json::json!([32, 1, 1]);
        }
        std::fs::write(&files.request, serde_json::to_vec(&request).unwrap()).unwrap();
        refused(&files);
    }
}
#[test]
fn command_arguments_are_closed_and_legacy_inputs_are_not_silently_reinterpreted() {
    let args = |values: &[&str]| values.iter().map(OsString::from).collect();
    assert!(
        parse(args(&[
            "sim",
            "--diagnostic-kir-v20",
            "a",
            "--request",
            "b"
        ]))
        .is_ok()
    );
    assert!(
        parse(args(&[
            "sim",
            "--diagnostic-kir-v20",
            "a",
            "--request",
            "b",
            "--wave-width",
            "64",
            "--protocol",
            "jsonl"
        ]))
        .is_ok()
    );
    for values in [
        &[
            "sim",
            "--diagnostic-kir-v20",
            "a",
            "--request",
            "b",
            "--wave-width",
            "32",
        ][..],
        &[
            "sim",
            "--diagnostic-kir-v20",
            "a",
            "--request",
            "b",
            "--diagnostic-kir-v20",
            "c",
        ],
        &[
            "sim",
            "--diagnostic-kir-v20",
            "a",
            "--request",
            "b",
            "--source",
            "x",
        ],
        &["sim", "--kir", "a", "--request", "b"],
    ] {
        assert!(parse(args(values)).is_err());
    }
}
#[test]
fn real_jsonl_forward_reverse_and_termination_use_frozen_v1_responses() {
    let files = Files::new(true, 1, false);
    let (_input, mut b) = backend(&files);
    let requests = [
        DebugRequestV1::DiscoverCapabilities {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: 0,
        },
        step(&b, StepDirectionV1::Forward, 1),
        DebugRequestV1::Step {
            schema: RequestSchemaV1::V1,
            request_id: 3,
            expected_revision: 1,
            direction: StepDirectionV1::Reverse,
            granularity: StepGranularityV1::Event,
            count: 1,
            focus: None,
        },
        DebugRequestV1::Terminate {
            schema: RequestSchemaV1::V1,
            request_id: 4,
            expected_revision: 2,
        },
    ];
    let mut lines = Vec::new();
    for r in requests {
        lines.extend(serde_json::to_vec(&r).unwrap());
        lines.push(b'\n');
    }
    let mut output = Vec::new();
    protocol::run(&mut b, &mut &lines[..], &mut output, protocol_limits()).unwrap();
    let responses: Vec<DebugResponseV1> = output
        .split(|b| *b == b'\n')
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::from_slice(s).unwrap())
        .collect();
    assert_eq!(responses.len(), 4);
    assert!(
        responses
            .iter()
            .all(|r| matches!(r, DebugResponseV1::Ok { .. }))
    );
    assert!(b.terminated);
    assert_eq!(b.sequence(), 0);
}

#[test]
#[ignore = "root-owned actual unchanged canonical V20/source-request qualification"]
fn supplied_actual_v20_input_uses_same_public_loader_and_jsonl_session() {
    let kir = PathBuf::from(
        std::env::var_os("FE2O3_V20_DEBUG_KIR").expect("explicit actual canonical input"),
    );
    let request =
        PathBuf::from(std::env::var_os("FE2O3_V20_DEBUG_REQUEST").expect("explicit request input"));
    let mut ledger = Owned::new(Work::new(WORK), STORAGE);
    ledger
        .with_budget(|b| {
            b.reserve_storage(SCRATCH)?;
            b.charge_work(RESPONSE * 2)
        })
        .unwrap();
    let (input, receipt) = ledger
        .with_budget(|b| load_physical_entry_debug_input_v20(&kir, &request, b))
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    let mut b = capture(&input, ledger).unwrap();
    assert!(b.session.capture_stop().is_none());
    let request = step(&b, StepDirectionV1::Forward, 1);
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Ok { .. }
    ));
    let request = step(&b, StepDirectionV1::Reverse, 1);
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Ok { .. }
    ));
    assert_eq!(b.sequence(), 0);
    let symbolic = (0..b.session.records_len())
        .find(|&i| {
            let r = b.session.record(i).unwrap();
            r.phase().is_some()
                && (0..r.binding_count(0).unwrap_or(0))
                    .any(|j| r.binding(0, j).unwrap().symbolic_kind().is_some())
        })
        .unwrap();
    seek(&mut b, symbolic as u64 + 1);
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: None,
            limit: 64,
        },
    );
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("actual values")
    };
    let DebugResultV1::Values { values, .. } = *result else {
        panic!("actual values")
    };
    assert!(values.iter().any(|v| matches!(
        v.availability,
        ValueAvailabilityV1::Unavailable {
            reason: ValueUnavailableReasonV1::NotRepresented
        }
    )));
    // Bounded diagnostic indexes from actual immutable typed Session records.
    // This is not a reconstructed source owner, executable graph or ISA oracle.
    let mut lane_zero_checkpoints = Vec::new();
    let mut final_checkpoint_index = None;
    for index in 0..b.session.records_len() {
        let record = b.session.record(index).unwrap();
        let Some(phase) = record.phase() else {
            continue;
        };
        final_checkpoint_index = Some(index);
        if record.invocation().global == [0, 0, 0] {
            assert!(lane_zero_checkpoints.len() < 128);
            lane_zero_checkpoints.push(serde_json::json!({
                "index":index,
                "block":record.site().block.0,
                "operation":record.site().operation,
                "phase":match phase {
                    Phase::BeforeOperation => "before_operation",
                    Phase::AfterOperation => "after_operation",
                },
            }));
        }
    }
    assert!(!lane_zero_checkpoints.is_empty());
    let final_checkpoint_index = final_checkpoint_index.expect("actual final CPU checkpoint");
    let output = serde_json::json!({"schema":"fe2o3-physical-entry-cpu-debug-cli-observation-v20",
 "canonical_sha256":hex_bytes(input.canonical().identity().digest()),"canonical_bytes":input.canonical().identity().canonical_length(),
 "request_sha256":hex_bytes(input.request_digest()),"records":b.session.records_len(),"work":b.session.usage().work,
 "lane_zero_checkpoints":lane_zero_checkpoints,"final_checkpoint_index":final_checkpoint_index,
 "source_custody":false,"hardware_observed":false,"resumable_execution":false,"protected_authority":false});
    println!("{output}");
}
