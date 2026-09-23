//! Normal external consumer: only exported debugger/simulator APIs are used.
use fe2o3_kir_debugger::*;
use fe2o3_kir_sim::*;
#[path = "../src/runtime_observation_v1/fixtures_tests.rs"]
mod fixtures;

fn options() -> RuntimeObservationOptionsV1 {
    RuntimeObservationOptionsV1::new(
        RuntimeOriginCaptureModeV1::Enabled(
            RuntimeOriginCaptureLimitsV1::new(4096, 256 * 1024).unwrap(),
        ),
        RuntimeFrameCaptureModeV1::Enabled(
            RuntimeFrameCaptureLimitsV1::new(4096, 16_384, 3 * 1024 * 1024).unwrap(),
        ),
        RuntimeAllocationCaptureModeV1::Enabled(
            RuntimeAllocationCaptureLimitsV1::new(4096, 64, 1024 * 1024).unwrap(),
        ),
        Some(SimulationAllocationReuseV1::exact_private_and_workgroup(8192).unwrap()),
    )
    .unwrap()
}

#[test]
fn external_consumer_uses_only_fresh_sealed_capture_and_named_controls() {
    for (module, request) in [
        fixtures::loops(),
        fixtures::memory(),
        fixtures::barrier(),
        fixtures::private_allocations(),
    ] {
        let capture = SimulationDebugCaptureLimitsV1::new(8, 64, 8, 256).unwrap();
        let baseline = capture_debugger_run_v1(
            &module,
            &request,
            fixtures::TARGET,
            fixtures::simulation_limits(),
            capture,
            fixtures::debugger_limits(4096),
            DebugWaveWidthV1::Wave64,
        );
        let observed = capture_debugger_observed_run_v1(
            &module,
            &request,
            fixtures::TARGET,
            fixtures::simulation_limits(),
            capture,
            fixtures::debugger_limits(4096),
            DebugWaveWidthV1::Wave64,
            options(),
        )
        .unwrap();
        fixtures::assert_result_eq(observed.execution(), &baseline.execution);
        assert_eq!(observed.transcript().legacy(), &baseline.transcript);
        assert_eq!(
            observed.transcript().origin_coverage(),
            RuntimeObservationCoverageV1::Complete
        );
        assert_eq!(
            observed.transcript().frame_coverage(),
            RuntimeObservationCoverageV1::Complete
        );
        assert_eq!(
            observed.transcript().allocation_coverage(),
            RuntimeObservationCoverageV1::Complete
        );
        let (_, transcript) = observed.into_parts();
        let instance = transcript.capture_instance();
        let focus = transcript.legacy().records()[0].invocation;
        let mut session = transcript.into_session();
        let mut work = RuntimeReplayWorkV1::new(1_000_000).unwrap();
        session
            .step_into(RuntimeNavigationDirectionV1::Forward, focus, &mut work)
            .unwrap();
        assert_eq!(session.capture_instance(), instance);
        assert_eq!(session.current_origin().unwrap().invocation(), focus);
        let frames = session.current_frames().unwrap();
        assert_eq!(frames.get(0).unwrap().activation(), 1);
        assert_eq!(frames.get(0).unwrap().legacy().depth, 0);
        assert_eq!(
            frames.get(0).unwrap().parent(),
            SimulationDebugFrameParentV1::Root
        );
        let allocations = session.current_allocations().unwrap();
        assert_eq!(allocations.record(), session.legacy().current().unwrap());
        assert_eq!(
            allocations.through_sequence() as usize,
            allocations.transitions().len()
        );
        session.seek_entry(&mut work).unwrap();
        assert_eq!(
            session.current_origin().unwrap_err(),
            RuntimeOriginMissingV1::NoCurrentRecord
        );
        assert_eq!(
            session.current_frames().unwrap_err(),
            RuntimeFrameMissingV1::NoCurrentRecord
        );
        assert_eq!(
            session.current_allocations().unwrap_err(),
            RuntimeAllocationMissingV1::NoCurrentRecord
        );
    }
}
