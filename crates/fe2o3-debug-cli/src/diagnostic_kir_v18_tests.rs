//! Generic V18 CPU capture and original-ledger lifetime controls.
use super::*;
use fe2o3_kir_sim_cli::load_debug_simulation_input_bytes_v18;
use std::io::{Cursor, Read};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "../../fe2o3-kir-sim-cli/tests/fixtures/diagnostic_kir_v18.rs"]
mod fixture;
const FLOOR: usize = 97;

fn with_input(value: u32, observe: impl FnOnce(AdmittedSimulationInputV1)) {
    let bytes = fixture::bytes(&fixture::module());
    let mut work = Work::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<(), Resource> = budget.with_prepaid_scope(FLOOR, 0, 0, 0, |budget| {
        let (input, receipt) =
            load_debug_simulation_input_bytes_v18(&bytes, &fixture::request(value), budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage())?;
        observe(input);
        Ok(())
    });
    result.unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn generic_capture_steps_and_inspects_dispatch_workgroup_wave_lane_and_memory() {
    with_input(37, |input| {
        let mut backend = SimulatorBackendV1::new(input, DebugWaveWidthV1::Wave64).unwrap();
        assert!(!backend.failed_execution);
        assert_eq!(backend.module.identity().wire_version(), 18);
        assert_eq!(backend.module.module().storage_layouts.len(), 2);
        assert_eq!(
            backend.session.transcript().completeness(),
            DebugTranscriptCompletenessV1::Complete
        );
        assert!(backend.source_map_identity.is_none() && backend.diagnosis_input.is_none());
        let stepped = backend.handle(DebugRequestV1::Step {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: backend.revision,
            direction: StepDirectionV1::Forward,
            granularity: StepGranularityV1::Operation,
            count: 1,
            focus: None,
        });
        stepped.validate(backend.protocol_limits).unwrap();
        assert!(matches!(stepped, DebugResponseV1::Ok { .. }));
        for scope in [
            ExecutionScopeSelectorV1::Dispatch,
            ExecutionScopeSelectorV1::Workgroup {
                workgroup: [0, 0, 0],
            },
            ExecutionScopeSelectorV1::Wave {
                workgroup: [0, 0, 0],
                wave: 0,
            },
            ExecutionScopeSelectorV1::Lane {
                workgroup: [0, 0, 0],
                wave: 0,
                lane: 0,
            },
        ] {
            let reply = backend.handle(DebugRequestV1::InspectScope {
                schema: RequestSchemaV1::V1,
                request_id: 2,
                expected_revision: backend.revision,
                scope,
                include_children: true,
                page: PageRequestV1 {
                    cursor: None,
                    limit: 64,
                },
            });
            reply.validate(backend.protocol_limits).unwrap();
            assert!(matches!(reply, DebugResponseV1::Ok { .. }), "{reply:?}");
        }
        let index = backend
            .session
            .transcript()
            .records()
            .iter()
            .rposition(|record| {
                matches!(record.kind, SimulationDebugRecordKindV1::Checkpoint { .. })
            })
            .unwrap();
        let record = &backend.session.transcript().records()[index];
        let SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Captured(memory),
            ..
        } = &record.kind
        else {
            panic!("checkpoint memory")
        };
        assert_eq!(memory[0].bytes, fixture::output(37));
        let allocation = memory[0].allocation;
        assert!(matches!(
            backend.session.seek_record_index(index),
            DebugNavigationV1::Stopped(_)
        ));
        let reply = backend.handle(DebugRequestV1::ReadMemory {
            schema: RequestSchemaV1::V1,
            request_id: 3,
            expected_revision: backend.revision,
            allocation: AllocationIdentityV1 {
                ordinal: allocation,
                generation: 0,
            },
            byte_offset: 0,
            byte_len: 4,
        });
        reply.validate(backend.protocol_limits).unwrap();
        let DebugResponseV1::Ok { result, .. } = reply else {
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
        assert_eq!(bytes, "0x30000000");
        assert_eq!(initialized, "0x0f");
    });
}

#[test]
fn raw_v18_diagnosis_remains_unavailable_without_fabricated_source_lineage() {
    with_input(37, |input| {
        let mut backend = SimulatorBackendV1::new(input, DebugWaveWidthV1::Wave64).unwrap();
        let before = backend.session_view();
        let response = backend.handle_diagnosis_v2(DiagnosisRequestV2::Diagnose {
            schema: DiagnosisRequestSchemaV2::V2,
            request_id: 1,
            expected_revision: backend.revision,
            filter: DiagnosisFilterV2::default(),
            page: PageRequestV1 {
                cursor: None,
                limit: 8,
            },
        });
        response.validate(backend.protocol_limits).unwrap();
        assert!(matches!(&response, DiagnosisResponseV2::Error {
            error: DebugErrorV1 { code: DebugErrorCodeV1::UnsupportedSchema, state_changed: false, message, .. }, ..
        } if message == DIAGNOSIS_UNAVAILABLE));
        let text = serde_json::to_string(&response).unwrap();
        assert!(!text.contains("canonical_kir_v7") && !text.contains("source_lineage"));
        assert_eq!(backend.session_view(), before);
    });
}

#[test]
fn configuration_binds_v18_request_limits_and_profile_without_legacy_alias() {
    with_input(37, |mut input| {
        let capture =
            SimulationDebugCaptureLimitsV1::new(64, 4096, 16384, 16 * 1024 * 1024).unwrap();
        let debugger = DebuggerLimitsV1::new(1_000_000, 16_000_000, 256 * 1024 * 1024).unwrap();
        let identity = |input: &AdmittedSimulationInputV1| {
            configuration_identity(input, DebugWaveWidthV1::Wave64, capture, debugger).unwrap()
        };
        let initial = identity(&input);
        assert!(
            super::super::diagnostic_kir_v17::configuration_identity(
                &input,
                DebugWaveWidthV1::Wave64,
                capture,
                debugger
            )
            .is_err()
        );
        input.request.grid.0[0] = 128;
        assert_ne!(initial, identity(&input));
        input.request.grid.0[0] = 64;
        input.simulation_limits.max_steps -= 1;
        assert_ne!(initial, identity(&input));
    });
}

#[test]
fn forbidden_options_are_rejected_before_any_input_or_protocol_output() {
    let baseline = [
        "sim",
        "--diagnostic-kir-v18",
        "/missing/program",
        "--request",
        "/missing/request",
    ];
    assert!(matches!(
        parse_options(baseline.map(OsString::from).into_iter())
            .unwrap()
            .program,
        ProgramInputV1::DiagnosticKirV18(_)
    ));
    let subject = "11".repeat(32);
    for extra in [
        vec!["--wave-width", "32"],
        vec!["--replay-schedule", "/missing/replay"],
        vec!["--runtime-observations", "v1"],
        vec![
            "--source-map",
            "/missing/map",
            "--source-bundle-subject",
            subject.as_str(),
        ],
        vec!["--diagnostic-kir-v18", "/missing/second"],
        vec!["--diagnostic-kir-v17", "/missing/second"],
        vec!["--diagnostic-kir-v19", "/missing/second"],
        vec!["--bundle-v6", "/missing/bundle"],
    ] {
        assert!(parse_options(baseline.into_iter().chain(extra).map(OsString::from)).is_err());
    }
    with_input(37, |input| {
        assert!(SimulatorBackendV1::new(input, DebugWaveWidthV1::Wave32).is_err())
    });
    with_input(37, |input| {
        assert!(
            SimulatorBackendV1::new_with_maps_schedule_and_observations(
                input,
                DebugWaveWidthV1::Wave64,
                None,
                None,
                None,
                true
            )
            .is_err()
        )
    });
}

fn state_line() -> Vec<u8> {
    let mut line = serde_json::to_vec(&DebugRequestV1::GetState {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
    })
    .unwrap();
    line.push(b'\n');
    line
}
struct FailingWriter;
impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("injected output failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct PanickingReader;
impl Read for PanickingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("injected protocol panic")
    }
}
impl BufRead for PanickingReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        panic!("injected protocol panic")
    }
    fn consume(&mut self, _: usize) {}
}

#[test]
fn actual_protocol_success_io_error_and_panic_restore_the_original_ledger() {
    let files = fixture::Files::new(&fixture::bytes(&fixture::module()), &fixture::request(37));
    let mut work = Work::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut output = Vec::new();
    run_with_budget(
        &files.kir,
        &files.request,
        DebugWaveWidthV1::Wave64,
        &mut budget,
        &mut Cursor::new(state_line()),
        &mut output,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let replies: Vec<DebugResponseV1> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 1);
    assert!(matches!(&replies[0], DebugResponseV1::Ok { .. }));
    let work_after_success = budget.work();
    assert!(
        run_with_budget(
            &files.kir,
            &files.request,
            DebugWaveWidthV1::Wave64,
            &mut budget,
            &mut Cursor::new(state_line()),
            &mut FailingWriter
        )
        .is_err()
    );
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work() > work_after_success);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        run_with_budget(
            &files.kir,
            &files.request,
            DebugWaveWidthV1::Wave64,
            &mut budget,
            &mut PanickingReader,
            &mut Vec::new(),
        )
    }));
    assert!(panic.is_err());
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn preflight_refusal_drops_view_and_preserves_original_account() {
    let files = fixture::Files::new(
        &fixture::bytes(&fixture::storage_module(true)),
        &fixture::storage_request(),
    );
    let mut work = Work::new(fixture::BOUND);
    let mut budget = Budget::new(&mut work, fixture::BOUND);
    budget.reserve_storage(FLOOR).unwrap();
    let mut output = Vec::new();
    assert!(matches!(
        run_with_budget(
            &files.kir,
            &files.request,
            DebugWaveWidthV1::Wave64,
            &mut budget,
            &mut Cursor::new(state_line()),
            &mut output
        ),
        Err(RunError::Session(_))
    ));
    assert!(output.is_empty());
    assert_eq!(budget.storage(), FLOOR);
}
