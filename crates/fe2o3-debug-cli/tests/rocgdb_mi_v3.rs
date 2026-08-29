#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use fe2o3_debug_cli::rocgdb_mi_v3::{
    RocgdbMiAdapterLimitsV3, RocgdbMiObservationAdapterV3, RocgdbMiProcessV3,
};
use fe2o3_debug_protocol::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).expect("nonzero identity")
}

fn adapter() -> RocgdbMiObservationAdapterV3 {
    RocgdbMiObservationAdapterV3::new(
        identity(1),
        identity(2),
        32,
        RocgdbMiAdapterLimitsV3::default(),
    )
    .expect("adapter")
}

#[test]
fn admits_only_caller_selected_structured_thread_tuple() {
    let mut adapter = adapter();
    let admissions = adapter
        .admit_gpu_threads_from_thread_info(
            b"8^done,threads=[{id=\"7\",target-id=\"AMDGPU fake prose\"},{id=\"9\",target-id=\"Host\"}]\n",
            &[1],
        )
        .expect("structured admission");
    assert_eq!(admissions.len(), 1);

    let outside = adapter
        .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"7\"\n")
        .expect("typed host stop")
        .expect("event");
    assert!(matches!(
        outside,
        RocgdbMiExecutionEventV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::OutsideCaptureScope,
            ..
        }
    ));
}

#[test]
fn maps_only_bound_locations_and_never_serializes_native_authority() {
    let mut adapter = adapter();
    let content = LiveGpuContentIdentityV3 {
        digest: identity(3),
        canonical_bytes: 0x200,
    };
    let source = LiveGpuSourceSpanV3 {
        source_map_identity: identity(4),
        file_identity: identity(5),
        byte_start: 10,
        byte_end: 20,
    };
    adapter
        .bind_code_object(content, 0x1000, 0x200, 0x1020)
        .expect("code object binding");
    adapter
        .bind_source_line(OsStr::new("/private/kernel.fe"), 7, source)
        .expect("source binding");
    adapter
        .admit_gpu_threads_from_thread_info(b"8^done,threads=[{id=\"9\"}]\n", &[0])
        .expect("admission");
    let event = adapter
        .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"9\",frame={addr=\"0x1028\",fullname=\"/private/kernel.fe\",line=\"7\"}\n")
        .expect("stop")
        .expect("event");
    let RocgdbMiExecutionEventV3::Stopped { snapshot } = event else {
        panic!("expected stopped snapshot");
    };
    let thread = &snapshot.threads[0];
    assert!(matches!(
        thread.relative_pc,
        LiveGpuAvailabilityV3::Available {
            value: LiveGpuRelativePcV3 {
                kernel_entry_byte_offset: 8
            },
            ..
        }
    ));
    assert!(
        matches!(thread.source, LiveGpuAvailabilityV3::Available { value, .. } if value == source)
    );
    assert!(thread.lanes.iter().all(|lane| matches!(
        lane.active,
        LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotCaptured,
            ..
        }
    )));

    let wire = serde_json::to_string(&snapshot).expect("serialize sanitized snapshot");
    for secret in ["/private/kernel.fe", "0x1028", "thread-id", "target-id"] {
        assert!(!wire.contains(secret), "public record leaked {secret}");
    }
    let debug = format!("{adapter:?}");
    assert!(!debug.contains("/private/kernel.fe"));
    assert!(!debug.contains("0x1028"));
}

#[test]
fn exec_register_is_the_only_lane_activity_authority() {
    let mut adapter = adapter();
    adapter
        .admit_gpu_threads_from_thread_info(b"8^done,threads=[{id=\"9\"}]\n", &[0])
        .expect("admission");
    let event = adapter
        .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"9\"\n")
        .expect("stop")
        .expect("event");
    let RocgdbMiExecutionEventV3::Stopped { mut snapshot } = event else {
        panic!("expected stop");
    };
    let stopped = &snapshot.threads[0];
    let scope = RocgdbMiStoppedScopeV3 {
        stop_identity: snapshot.stop_identity,
        thread: stopped.thread,
        wave: stopped.wave,
        lane: None,
    };
    let truth = LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Observed,
        evidence: vec![LiveGpuEvidenceRefV3 {
            kind: LiveGpuEvidenceKindV3::RuntimeObservation,
            identity: identity(6),
        }],
    };
    let registers = RocgdbMiRegisterSnapshotV3 {
        scope,
        registers: vec![LiveGpuRegisterValueV3 {
            register_identity: identity(7),
            name: "exec".to_owned(),
            class: LiveGpuRegisterClassV3::Predicate,
            kind: LiveGpuValueKindV3::UnsignedInteger,
            lane: None,
            value: LiveGpuAvailabilityV3::Available {
                value: LiveGpuValueEncodingV3::Bits {
                    bit_width: 8,
                    bits: "05".to_owned(),
                },
                truth,
            },
        }],
    };
    adapter
        .apply_exec_mask(&mut snapshot, &registers)
        .expect("exec mask");
    let lanes = &snapshot.threads[0].lanes;
    assert!(matches!(
        lanes[0].active,
        LiveGpuAvailabilityV3::Available { value: true, .. }
    ));
    assert!(matches!(
        lanes[1].active,
        LiveGpuAvailabilityV3::Available { value: false, .. }
    ));
    assert!(matches!(
        lanes[2].active,
        LiveGpuAvailabilityV3::Available { value: true, .. }
    ));
}

#[test]
fn token_correlates_queries_and_authorized_controls() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_rocgdb_mi_v3.py");
    let mut process = RocgdbMiProcessV3::spawn(
        &fixture,
        identity(1),
        identity(2),
        32,
        RocgdbMiAdapterLimitsV3::default(),
    )
    .expect("fake MI process");
    let timeout = Duration::from_secs(5);
    let capabilities = process
        .discover_capabilities(timeout)
        .expect("capabilities");
    capabilities.validate().expect("valid capabilities");
    let stopped_wave = capabilities
        .capabilities
        .iter()
        .find(|item| item.name == RocgdbMiCapabilityNameV3::StoppedWave)
        .expect("stopped wave capability");
    assert_eq!(
        stopped_wave.availability,
        LiveGpuCapabilityAvailabilityV3::Unavailable
    );
    assert_eq!(
        stopped_wave.unavailable_reason,
        Some(LiveGpuUnavailableReasonV3::NotCaptured)
    );

    let content = LiveGpuContentIdentityV3 {
        digest: identity(3),
        canonical_bytes: 0x200,
    };
    process
        .adapter_mut()
        .bind_code_object(content, 0x1000, 0x200, 0x1020)
        .expect("code object");
    process
        .adapter_mut()
        .bind_allocation(
            AllocationIdentityV1 {
                ordinal: 1,
                generation: 1,
            },
            0x2000,
            0x100,
            LiveGpuMemorySpaceV3::Global,
        )
        .expect("allocation");
    let launch = process
        .launch_target(
            RocgdbMiControlRequestV3::Launch {
                request_id: 1,
                authorization: RocgdbMiControlAuthorizationV3 {
                    authorization_identity: identity(2),
                    expected_revision: 0,
                },
            },
            std::path::Path::new("/bin/true"),
            &[OsString::from("argument with spaces")],
            timeout,
        )
        .expect("launch result");
    assert!(matches!(
        launch.outcome,
        RocgdbMiControlOutcomeV3::Applied {
            effect: RocgdbMiControlEffectV3::Committed,
            ..
        }
    ));
    assert_eq!(launch.audit.before_revision, 0);
    assert_eq!(launch.audit.after_revision, 1);
    assert!(matches!(
        process.next_event(timeout).expect("running"),
        RocgdbMiExecutionEventV3::Running { revision: 1 }
    ));

    let admissions = process.admit_gpu_threads(&[0], timeout).expect("admission");
    let stopped = process.next_event(timeout).expect("stopped");
    let RocgdbMiExecutionEventV3::Stopped { snapshot } = stopped else {
        panic!("expected stop");
    };
    let thread = &snapshot.threads[0];
    assert_eq!(thread.thread, admissions[0].thread);
    let admitted_capabilities = process
        .discover_capabilities(timeout)
        .expect("admitted capabilities");
    for available in [
        RocgdbMiCapabilityNameV3::StoppedWave,
        RocgdbMiCapabilityNameV3::LogicalLanes,
        RocgdbMiCapabilityNameV3::RelativeProgramCounter,
        RocgdbMiCapabilityNameV3::RegisterValues,
        RocgdbMiCapabilityNameV3::SemanticValues,
        RocgdbMiCapabilityNameV3::AllocationRelativeMemory,
    ] {
        assert!(admitted_capabilities.capabilities.iter().any(|item| {
            item.name == available
                && item.availability == LiveGpuCapabilityAvailabilityV3::Available
        }));
    }
    let scope = RocgdbMiStoppedScopeV3 {
        stop_identity: snapshot.stop_identity,
        thread: thread.thread,
        wave: thread.wave,
        lane: None,
    };

    let registers = process
        .inspect_registers(scope, timeout)
        .expect("registers");
    assert_eq!(registers.registers.len(), 2);
    assert!(matches!(
        registers.registers[0].value,
        LiveGpuAvailabilityV3::Available { .. }
    ));
    let values = process.inspect_values(scope, timeout).expect("values");
    assert!(matches!(
        values.values[1].value,
        LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::OptimizedOut,
            ..
        }
    ));
    let first = process
        .evaluate_expression(scope, identity(8), "first", "first", timeout)
        .expect("first expression");
    let second = process
        .evaluate_expression(scope, identity(9), "second", "second", timeout)
        .expect("second expression");
    let (
        LiveGpuAvailabilityV3::Available {
            truth: first_truth, ..
        },
        LiveGpuAvailabilityV3::Available {
            truth: second_truth,
            ..
        },
    ) = (&first.value, &second.value)
    else {
        panic!("available expressions");
    };
    assert_ne!(
        first_truth.evidence, second_truth.evidence,
        "response bytes must bind observation truth"
    );

    let memory = process
        .read_memory(
            RocgdbMiMemoryReadRequestV3 {
                request_id: 2,
                expected_revision: snapshot.revision,
                scope,
                allocation: AllocationIdentityV1 {
                    ordinal: 1,
                    generation: 1,
                },
                byte_offset: 4,
                byte_len: 2,
            },
            timeout,
        )
        .expect("memory");
    assert!(
        matches!(memory.memory.value, LiveGpuAvailabilityV3::Available { ref value, .. } if value.bytes == "a10f")
    );

    let breakpoint = process
        .control(
            RocgdbMiControlRequestV3::InsertBreakpoint {
                request_id: 3,
                authorization: RocgdbMiControlAuthorizationV3 {
                    authorization_identity: identity(2),
                    expected_revision: snapshot.revision,
                },
                site: RocgdbMiBreakpointSiteV3::CodeObjectRelative {
                    code_object: content,
                    kernel_entry_byte_offset: 8,
                },
            },
            timeout,
        )
        .expect("breakpoint");
    assert_eq!(breakpoint.audit.after_revision, snapshot.revision + 1);

    let continued = process
        .control(
            RocgdbMiControlRequestV3::Continue {
                request_id: 4,
                authorization: RocgdbMiControlAuthorizationV3 {
                    authorization_identity: identity(2),
                    expected_revision: breakpoint.revision,
                },
                focus: thread.thread,
            },
            timeout,
        )
        .expect("continue");
    assert_eq!(
        continued.audit.after_revision,
        continued.audit.before_revision + 1
    );
    assert!(
        matches!(process.next_event(timeout).expect("correlated running"), RocgdbMiExecutionEventV3::Running { revision } if revision == continued.revision)
    );
}

#[test]
#[ignore = "requires an installed ROCgdb"]
fn installed_rocgdb_reports_structured_capabilities() {
    let executable = std::env::var_os("FE2O3_ROCGDB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/rocgdb"));
    let mut process = RocgdbMiProcessV3::spawn(
        &executable,
        identity(10),
        identity(11),
        64,
        RocgdbMiAdapterLimitsV3::default(),
    )
    .expect("installed ROCgdb");
    let capabilities = process
        .discover_capabilities(Duration::from_secs(15))
        .expect("structured capability discovery");
    capabilities.validate().expect("valid capabilities");
    for required in [
        RocgdbMiCapabilityNameV3::Launch,
        RocgdbMiCapabilityNameV3::Attach,
        RocgdbMiCapabilityNameV3::StructuredThreads,
        RocgdbMiCapabilityNameV3::Breakpoints,
        RocgdbMiCapabilityNameV3::Continue,
        RocgdbMiCapabilityNameV3::Pause,
        RocgdbMiCapabilityNameV3::Step,
    ] {
        let capability = capabilities
            .capabilities
            .iter()
            .find(|capability| capability.name == required)
            .expect("complete capability record");
        assert_eq!(
            capability.availability,
            LiveGpuCapabilityAvailabilityV3::Available,
            "missing installed ROCgdb capability {required:?}"
        );
    }
    let stopped_wave = capabilities
        .capabilities
        .iter()
        .find(|capability| capability.name == RocgdbMiCapabilityNameV3::StoppedWave)
        .expect("stopped wave capability");
    assert_eq!(
        stopped_wave.availability,
        LiveGpuCapabilityAvailabilityV3::Unavailable
    );
    assert_eq!(
        stopped_wave.unavailable_reason,
        Some(LiveGpuUnavailableReasonV3::NotCaptured)
    );
}
