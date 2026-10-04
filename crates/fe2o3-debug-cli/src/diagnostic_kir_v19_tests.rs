//! Diagnostic synthetic graphs only; no authenticated Rust or hardware owner.
use super::*;
use fe2o3_kir_sim_cli::load_debug_simulation_input_bytes_v19;
#[path = "../../fe2o3-kir-sim-cli/tests/fixtures/diagnostic_kir_v19.rs"]
mod fixture;

fn input(diamond: bool, selector: u32) -> AdmittedSimulationInputV1 {
    let owner = fixture::owner(&if diamond {
        fixture::diamond()
    } else {
        fixture::single()
    });
    load_debug_simulation_input_bytes_v19(owner.canonical_bytes(), &fixture::request(selector))
        .unwrap()
}
fn scalar(record: &SimulationDebugRecordV1, id: u32) -> Option<u32> {
    let SimulationDebugRecordKindV1::Checkpoint {
        stack: SimulationDebugCollectionV1::Captured(frames),
        ..
    } = &record.kind
    else {
        panic!("logical checkpoint expected");
    };
    let SimulationDebugCollectionV1::Captured(values) = &frames[0].values else {
        panic!();
    };
    values
        .iter()
        .find(|binding| binding.value == ValueId(id))
        .map(|binding| {
            let SimulationDebugValueV1::Scalar(value) = binding.observed else {
                panic!();
            };
            u32::try_from(value.bits()).unwrap()
        })
}

#[test]
fn authored_step_has_logical_before_after_and_reversible_checkpoint_values() {
    let mut backend = SimulatorBackendV1::new(input(false, 0), DebugWaveWidthV1::Wave64).unwrap();
    assert!(!backend.failed_execution);
    assert_eq!(backend.module.identity().wire_version(), 19);
    assert_eq!(
        backend.session.transcript().completeness(),
        DebugTranscriptCompletenessV1::Complete
    );
    assert!(backend.source_map_identity.is_none());
    assert!(backend.source_variables_v2.is_none());
    assert!(backend.diagnosis_input.is_none());
    let mut before = Vec::new();
    let mut after = Vec::new();
    for (index, record) in backend.session.transcript().records().iter().enumerate() {
        if record.site.block != fe2o3_kernel_ir::BlockId(0) || record.site.operation != 1 {
            continue;
        }
        match record.kind {
            SimulationDebugRecordKindV1::Checkpoint {
                phase: SimulationDebugCheckpointPhaseV1::BeforeOperation,
                ..
            } => {
                assert_eq!(scalar(record, 1), Some(19));
                assert_eq!(scalar(record, 5), None);
                before.push(index);
            }
            SimulationDebugRecordKindV1::Checkpoint {
                phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
                ..
            } => {
                assert_eq!(scalar(record, 5), Some(19));
                after.push(index);
            }
            _ => panic!("authored move has no physical capture or memory substep"),
        }
    }
    assert_eq!((before.len(), after.len()), (64, 64));
    for (index, expected) in [
        (after[0], Some(19)),
        (before[0], None),
        (after[0], Some(19)),
    ] {
        assert!(matches!(
            backend.session.seek_record_index(index),
            DebugNavigationV1::Stopped(_)
        ));
        let record =
            &backend.session.transcript().records()[backend.session.cursor_record_index().unwrap()];
        assert_eq!(scalar(record, 5), expected);
    }
}

#[test]
fn both_diamond_branches_finish_with_current_logical_memory() {
    for selector in [0, 1, u32::MAX] {
        let backend =
            SimulatorBackendV1::new(input(true, selector), DebugWaveWidthV1::Wave64).unwrap();
        assert!(!backend.failed_execution);
        let record = backend
            .session
            .transcript()
            .records()
            .iter()
            .rev()
            .find(|record| matches!(record.kind, SimulationDebugRecordKindV1::Checkpoint { .. }))
            .unwrap();
        let SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Captured(memory),
            ..
        } = &record.kind
        else {
            panic!();
        };
        assert_eq!(
            memory[0].bytes,
            fixture::output(if selector == 0 { 19 } else { 42 })
        );
    }
}

#[test]
fn v19_configuration_binds_typed_request_and_has_no_v17_domain_alias() {
    let mut admitted = input(false, 0);
    let capture = SimulationDebugCaptureLimitsV1::new(64, 4096, 16384, 16 * 1024 * 1024).unwrap();
    let debugger = DebuggerLimitsV1::new(1_000_000, 16_000_000, 256 * 1024 * 1024).unwrap();
    let old =
        configuration_identity(&admitted, DebugWaveWidthV1::Wave64, capture, debugger).unwrap();
    assert!(
        super::super::diagnostic_kir_v17::configuration_identity(
            &admitted,
            DebugWaveWidthV1::Wave64,
            capture,
            debugger
        )
        .is_err()
    );
    let raw_hash = admitted.request_sha256;
    admitted.request.grid.0[0] = 128;
    let new =
        configuration_identity(&admitted, DebugWaveWidthV1::Wave64, capture, debugger).unwrap();
    assert_ne!(old, new);
    assert_eq!(raw_hash, admitted.request_sha256);
}

#[test]
fn source_map_runtime_wave_and_schedule_controls_refuse_at_cli_and_session_boundaries() {
    let baseline = [
        "sim",
        "--diagnostic-kir-v19",
        "/missing/program",
        "--request",
        "/missing/request",
    ];
    assert!(matches!(
        parse_options(baseline.map(OsString::from).into_iter())
            .unwrap()
            .program,
        ProgramInputV1::DiagnosticKirV19(_)
    ));
    let subject = "11".repeat(32);
    for extra in [
        vec!["--wave-width", "32"],
        vec!["--replay-schedule", "/missing/schedule"],
        vec!["--runtime-observations", "v1"],
        vec![
            "--source-map",
            "/missing/map",
            "--source-bundle-subject",
            subject.as_str(),
        ],
        vec!["--diagnostic-kir-v17", "/missing/other"],
        vec!["--diagnostic-kir-v19", "/missing/again"],
        vec!["--bundle-v6", "/missing/bundle"],
    ] {
        assert!(parse_options(baseline.into_iter().chain(extra).map(OsString::from)).is_err());
    }
    assert!(SimulatorBackendV1::new(input(false, 0), DebugWaveWidthV1::Wave32).is_err());
    assert!(
        SimulatorBackendV1::new_with_maps_schedule_and_observations(
            input(false, 0),
            DebugWaveWidthV1::Wave64,
            None,
            None,
            None,
            true
        )
        .is_err()
    );
}

#[test]
fn diagnosis_v2_never_labels_v19_as_canonical_v7_or_source_lineage() {
    let mut backend = SimulatorBackendV1::new(input(false, 0), DebugWaveWidthV1::Wave64).unwrap();
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
}
