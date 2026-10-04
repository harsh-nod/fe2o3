//! Actual CPU backend controls over raw KIR and verified synthetic bundles.
//! These controls do not authenticate ordinary source or observe GPU hardware.

#[test]
fn declared_target_stream_flushes_each_reply_and_propagates_flush_failure() {
    #[derive(Default)]
    struct BufferedSink {
        pending: Vec<u8>,
        delivered: Vec<u8>,
        flushes: usize,
        fail_flush: bool,
    }
    impl std::io::Write for BufferedSink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.pending.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            if self.fail_flush {
                return Err(std::io::Error::other("controlled flush failure"));
            }
            self.delivered.append(&mut self.pending);
            Ok(())
        }
    }
    let mut backend = backend(raw(), true);
    let binding = binding(&mut backend);
    let request = request(&backend, binding);
    let response = backend.handle_declared_target_v1(request);
    let expected =
        encode_declared_target_response_line_v1(&response, backend.protocol_limits).unwrap();
    let mut sink = BufferedSink::default();
    runtime_queries_v1::write_declared_target_v1(&mut sink, &response, backend.protocol_limits)
        .unwrap();
    assert_eq!(sink.flushes, 1);
    assert!(sink.pending.is_empty());
    assert_eq!(sink.delivered, expected);
    sink.fail_flush = true;
    assert_eq!(
        runtime_queries_v1::write_declared_target_v1(&mut sink, &response, backend.protocol_limits),
        Err("failed to flush declared target response".to_owned())
    );
    assert_eq!(sink.flushes, 2);
    assert_eq!(sink.delivered, expected);
}
use super::*;
use fe2o3_kernel_ir::{
    PreparedSimulationBundleV1, SimulationCompilerExecutionBindingV1,
    SimulationProductionKirIdentityV1, SimulationSourceLineageV1, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8,
};
use std::sync::atomic::{AtomicU64, Ordering};

fn raw() -> AdmittedSimulationInputV1 {
    load_debug_simulation_input_bytes_v1(
        include_bytes!("../../fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"),
        include_bytes!("../../fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"),
    )
    .unwrap()
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-declared-target-owner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn bundled(target: &str) -> AdmittedSimulationInputV1 {
    let input = raw();
    let module = input.module.module().clone();
    let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    let prepared = PreparedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([1; 32], 11, [2; 32], 22).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production.identity().digest(),
            production.identity().canonical_length(),
        )
        .unwrap(),
        target,
        VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
    )
    .unwrap();
    let bundle = prepared.finalize_without_source_map().unwrap();
    let directory = Directory::new();
    let bundle_path = directory.0.join("kernel.fe2sim");
    let request_path = directory.0.join("request.json");
    std::fs::write(&bundle_path, bundle.canonical_bytes()).unwrap();
    std::fs::write(
        &request_path,
        include_bytes!("../../fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"),
    )
    .unwrap();
    load_debug_simulation_bundle_v1(&bundle_path, &request_path)
        .unwrap()
        .into_parts()
        .0
}
fn backend(input: AdmittedSimulationInputV1, observed: bool) -> SimulatorBackendV1 {
    SimulatorBackendV1::new_with_maps_schedule_and_observations(
        input,
        DebugWaveWidthV1::Wave32,
        None,
        None,
        None,
        observed,
    )
    .unwrap()
}
fn binding(backend: &mut SimulatorBackendV1) -> RuntimeObservationBindingV1 {
    let request = RuntimeObservationRequestV1::InspectCurrentRecord {
        schema: RuntimeObservationRequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_cursor: backend.session_view().cursor,
        expected_owner: None,
    };
    let response = backend.handle_runtime_observation_v1(request.clone());
    response
        .validate_for_request(&request, backend.protocol_limits)
        .unwrap();
    match response {
        RuntimeObservationResponseV1::Ok { binding, .. }
        | RuntimeObservationResponseV1::Unavailable {
            binding: Some(binding),
            ..
        } => binding,
        other => panic!("runtime owner discovery failed: {other:?}"),
    }
}
fn request(
    backend: &SimulatorBackendV1,
    binding: RuntimeObservationBindingV1,
) -> DeclaredTargetRequestV1 {
    DeclaredTargetRequestV1 {
        schema: DeclaredTargetRequestSchemaV1::V1,
        operation: DeclaredTargetOperationV1::InspectDeclaredTarget,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        expected_binding: binding,
    }
}
fn line(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}
fn query(
    backend: &mut SimulatorBackendV1,
    request: DeclaredTargetRequestV1,
) -> DeclaredTargetResponseV1 {
    let limits = backend.protocol_limits;
    let request = decode_declared_target_request_line_v1(&line(&request), limits).unwrap();
    let response = backend.handle_declared_target_v1(request);
    response.validate_for_request(&request, limits).unwrap();
    let encoded = encode_declared_target_response_line_v1(&response, limits).unwrap();
    assert!(encoded.len() <= MAX_DECLARED_TARGET_LINE_BYTES_V1);
    assert_eq!(
        decode_declared_target_response_line_v1(&encoded, limits).unwrap(),
        response
    );
    response
}
fn refused(response: DeclaredTargetResponseV1, expected: DebugErrorCodeV1) {
    assert!(
        matches!(response, DeclaredTargetResponseV1::Error {
        error: DebugErrorV1 { code, state_changed: false, .. }, ..
    } if code == expected),
        "{response:?}"
    );
}
#[test]
fn both_targets_are_exact_declarations_and_old_config_hashes_stay_unchanged() {
    for (name, expected) in [
        ("gfx942:xnack-", DeclaredGpuTargetV1::Gfx942XnackOff),
        ("gfx950:xnack-", DeclaredGpuTargetV1::Gfx950XnackOff),
    ] {
        let input = bundled(name);
        let retained = input.retained_bundle_target_v1().unwrap().unwrap();
        let old_config = configuration_identity_for_input(&input, DebugWaveWidthV1::Wave32);
        let legacy = backend(bundled(name), false);
        assert_eq!(legacy.configuration_identity, old_config);
        let mut backend = backend(input, true);
        assert_eq!(
            backend.configuration_identity,
            runtime_capture_v1::configuration(old_config, true)
        );
        let binding = binding(&mut backend);
        let before = backend.session_view();
        let count = backend.command_count;
        let request = request(&backend, binding);
        let response = query(&mut backend, request);
        assert_eq!(backend.session_view(), before);
        assert_eq!(backend.command_count, count + 1);
        match response {
            DeclaredTargetResponseV1::Ok {
                binding: actual,
                logical_wave_width: 32,
                target:
                    DeclaredTargetObservationV1::Declared {
                        target,
                        provenance: DeclaredTargetProvenanceV1::VerifiedSimulationBundle,
                        envelope_version: 1,
                        envelope_identity,
                        subject_identity,
                        admitted_module,
                    },
                ..
            } => {
                assert_eq!(target, expected);
                assert_eq!(actual, binding);
                assert_eq!(envelope_identity.as_bytes(), retained.envelope_identity());
                assert_eq!(subject_identity.as_bytes(), retained.subject_identity());
                assert_eq!(
                    admitted_module.sha256.as_bytes(),
                    *retained.module_identity().digest()
                );
                assert_eq!(
                    admitted_module.canonical_bytes.get(),
                    retained.module_identity().canonical_length()
                );
            }
            other => panic!("exact target declaration required: {other:?}"),
        }
    }
}
#[test]
fn raw_is_unavailable_and_disabled_observation_has_no_owner() {
    let mut observed = backend(raw(), true);
    let owner = binding(&mut observed);
    let request = request(&observed, owner);
    assert!(matches!(
        query(&mut observed, request),
        DeclaredTargetResponseV1::Ok {
            target: DeclaredTargetObservationV1::Unavailable {
                reason: DeclaredTargetUnavailableV1::RawInputHasNoDeclaredGpuTarget,
            },
            ..
        }
    ));
    let mut disabled = backend(raw(), false);
    let expected = RuntimeObservationBindingV1 {
        cursor: disabled.session_view().cursor,
        ..owner
    };
    let request = self::request(&disabled, expected);
    refused(
        query(&mut disabled, request),
        DebugErrorCodeV1::InvalidState,
    );
}
#[test]
fn module_substitution_is_rejected_before_backend_capture() {
    let mut input = bundled("gfx942:xnack-");
    let mut module = input.module.module().clone();
    module.id = "different-admitted-module".into();
    input.module = AdmittedSimulationModuleV1::admit(
        VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
        input.simulation_limits,
    )
    .unwrap();
    input.kir_sha256 = *input.module.identity().digest();
    let result = SimulatorBackendV1::new_with_maps_schedule_and_observations(
        input,
        DebugWaveWidthV1::Wave32,
        None,
        None,
        None,
        true,
    );
    let error = match result {
        Ok(_) => panic!("module substitution accepted"),
        Err(error) => error,
    };
    assert!(error.contains("bundle_target_binding_changed"));
}
#[test]
fn stale_cursor_owner_capture_and_configuration_refuse_without_control_mutation() {
    let mut backend = backend(bundled("gfx942:xnack-"), true);
    let actual = binding(&mut backend);
    for field in 0..4 {
        let mut wrong = actual;
        match field {
            0 => {
                wrong.owner.backend_session =
                    RuntimeDecimalU64V1::new(actual.owner.backend_session.get() + 1)
            }
            1 => {
                wrong.owner.capture_instance =
                    RuntimeDecimalU64V1::new(actual.owner.capture_instance.get() + 1)
            }
            2 => wrong.cursor.event_sequence += 1,
            3 => wrong.cursor.configuration_identity = OpaqueIdentityV1::new([97; 32]).unwrap(),
            _ => unreachable!(),
        }
        let before = backend.session_view();
        let count = backend.command_count;
        let request = request(&backend, wrong);
        refused(
            query(&mut backend, request),
            DebugErrorCodeV1::InvalidCursor,
        );
        assert_eq!(backend.session_view(), before);
        assert_eq!(backend.command_count, count);
    }
    let mut other = self::backend(bundled("gfx942:xnack-"), true);
    let request = request(&other, actual);
    refused(query(&mut other, request), DebugErrorCodeV1::InvalidCursor);
}
fn seek(backend: &mut SimulatorBackendV1, sequence: u64) {
    let request = DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: backend.command_count + 1,
        expected_revision: backend.revision,
        cursor: DebugCursorV1 {
            event_sequence: sequence,
            ..backend.session_view().cursor
        },
    };
    assert!(matches!(
        backend.handle(request),
        DebugResponseV1::Ok { .. }
    ));
}
#[test]
fn seek_and_reverse_repeat_require_the_exact_new_revision() {
    let mut backend = backend(bundled("gfx942:xnack-"), true);
    seek(&mut backend, 1);
    let first = binding(&mut backend);
    let stale = request(&backend, first);
    seek(&mut backend, 2);
    refused(query(&mut backend, stale), DebugErrorCodeV1::StaleRevision);
    let current = binding(&mut backend);
    let fresh = request(&backend, current);
    assert!(matches!(
        query(&mut backend, fresh),
        DeclaredTargetResponseV1::Ok { .. }
    ));
    seek(&mut backend, 1);
    let repeated = binding(&mut backend);
    assert_eq!(repeated.owner, first.owner);
    assert_eq!(repeated.cursor.event_sequence, first.cursor.event_sequence);
    assert_ne!(repeated.cursor.state_revision, first.cursor.state_revision);
    refused(query(&mut backend, stale), DebugErrorCodeV1::StaleRevision);
    let fresh = request(&backend, repeated);
    assert!(matches!(
        query(&mut backend, fresh),
        DeclaredTargetResponseV1::Ok { .. }
    ));
}
#[test]
fn termination_and_command_budget_refuse_target_queries() {
    for terminated in [false, true] {
        let mut backend = backend(raw(), true);
        let owner = binding(&mut backend);
        backend.terminated = terminated;
        if !terminated {
            backend.command_count = MAX_SESSION_COMMANDS_V1;
        }
        let request = request(&backend, owner);
        refused(
            query(&mut backend, request),
            if terminated {
                DebugErrorCodeV1::InvalidState
            } else {
                DebugErrorCodeV1::ResourceLimit
            },
        );
    }
}
#[test]
fn actual_jsonl_routes_only_explicit_target_schema_and_keeps_legacy_reply() {
    let mut backend = backend(raw(), true);
    let owner = binding(&mut backend);
    let target_request = request(&backend, owner);
    let legacy = DebugRequestV1::GetState {
        schema: RequestSchemaV1::V1,
        request_id: target_request.request_id + 1,
        expected_revision: backend.revision,
    };
    let mut bytes = line(&target_request);
    bytes.extend_from_slice(&line(&legacy));
    let mut output = Vec::new();
    run_jsonl_v1(backend, &mut io::Cursor::new(bytes), &mut output).unwrap();
    let rows = output.split_inclusive(|b| *b == b'\n').collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    decode_declared_target_response_line_v1(rows[0], ProtocolLimitsV1::default())
        .unwrap()
        .validate_for_request(&target_request, ProtocolLimitsV1::default())
        .unwrap();
    assert!(matches!(
        decode_response_line_v1(rows[1], ProtocolLimitsV1::default()).unwrap(),
        DebugResponseV1::Ok { .. }
    ));
}
