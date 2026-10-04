#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use fe2o3_debug_cli::rocgdb_mi_v3::{
    RocgdbMiAdapterErrorV3, RocgdbMiAdapterLimitsV3, RocgdbMiObservationAdapterV3,
    RocgdbMiProcessV3,
};
use fe2o3_debug_protocol::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).expect("nonzero identity")
}

fn unavailable<T>() -> LiveGpuAvailabilityV3<T> {
    LiveGpuAvailabilityV3::Unavailable {
        reason: LiveGpuUnavailableReasonV3::NotCaptured,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Unavailable,
            evidence: Vec::new(),
        },
    }
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
fn host_unknown_and_gpu_looking_metadata_remain_generic_threads() {
    for thread_info in [
        b"8^done,threads=[{id=\"9\",target-id=\"Host\"}]\n".as_slice(),
        b"8^done,threads=[{id=\"9\"}]\n".as_slice(),
        b"8^done,threads=[{id=\"9\",target-id=\"AMDGPU fake prose\"}]\n".as_slice(),
    ] {
        let mut adapter = adapter();
        let admissions = adapter
            .admit_threads_from_thread_info(thread_info, &[0])
            .expect("structured generic-thread admission");
        assert_eq!(admissions.len(), 1);

        let event = adapter
            .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"9\"\n")
            .expect("typed unavailable GPU stop")
            .expect("event");
        assert!(matches!(
            event,
            RocgdbMiExecutionEventV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::Unsupported,
                ..
            }
        ));
    }
}

#[test]
fn bound_locations_do_not_promote_a_generic_thread_or_leak_authority() {
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
        .admit_threads_from_thread_info(b"8^done,threads=[{id=\"9\"}]\n", &[0])
        .expect("admission");
    let event = adapter
        .ingest_line(b"*stopped,reason=\"signal-received\",thread-id=\"9\",frame={addr=\"0x1028\",fullname=\"/private/kernel.fe\",line=\"7\"}\n")
        .expect("stop")
        .expect("event");
    assert!(matches!(
        event,
        RocgdbMiExecutionEventV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::Unsupported,
            ..
        }
    ));

    let wire = serde_json::to_string(&event).expect("serialize sanitized event");
    for secret in ["/private/kernel.fe", "0x1028", "thread-id", "target-id"] {
        assert!(!wire.contains(secret), "public record leaked {secret}");
    }
    let debug = format!("{adapter:?}");
    assert!(!debug.contains("/private/kernel.fe"));
    assert!(!debug.contains("0x1028"));
}

#[test]
fn exec_register_is_the_only_lane_activity_authority() {
    let adapter = adapter();
    let thread = RocgdbMiThreadIdentityV3 {
        identity: identity(8),
    };
    let wave = RocgdbMiWaveIdentityV3 {
        identity: identity(9),
        thread,
    };
    let mut snapshot = RocgdbMiStoppedSnapshotV3 {
        snapshot_identity: identity(10),
        stop_identity: identity(11),
        revision: 1,
        reason: RocgdbMiStopReasonV3::Signal,
        breakpoint: None,
        threads: vec![RocgdbMiStoppedThreadV3 {
            thread,
            wave,
            wave_width: 32,
            lanes: (0..32)
                .map(|lane| RocgdbMiLaneObservationV3 {
                    lane: RocgdbMiLaneIdentityV3 {
                        identity: identity(u8::try_from(lane + 20).unwrap()),
                        wave,
                        lane,
                    },
                    active: unavailable(),
                })
                .collect(),
            relative_pc: unavailable(),
            source: unavailable(),
        }],
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
fn structured_generic_threads_support_control_without_gpu_semantic_truth() {
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
        Some(LiveGpuUnavailableReasonV3::Unsupported)
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

    let admissions = process.admit_threads(&[0], timeout).expect("admission");
    let stopped = process.next_event(timeout).expect("stopped");
    assert!(matches!(
        stopped,
        RocgdbMiExecutionEventV3::Unavailable {
            revision: 2,
            reason: LiveGpuUnavailableReasonV3::Unsupported
        }
    ));
    let admitted_capabilities = process
        .discover_capabilities(timeout)
        .expect("admitted capabilities");
    for unavailable in [
        RocgdbMiCapabilityNameV3::StoppedWave,
        RocgdbMiCapabilityNameV3::LogicalLanes,
        RocgdbMiCapabilityNameV3::RelativeProgramCounter,
        RocgdbMiCapabilityNameV3::SourceSite,
        RocgdbMiCapabilityNameV3::RegisterValues,
        RocgdbMiCapabilityNameV3::SemanticValues,
        RocgdbMiCapabilityNameV3::AllocationRelativeMemory,
    ] {
        assert!(admitted_capabilities.capabilities.iter().any(|item| {
            item.name == unavailable
                && item.availability == LiveGpuCapabilityAvailabilityV3::Unavailable
                && item.unavailable_reason == Some(LiveGpuUnavailableReasonV3::Unsupported)
        }));
    }
    let scope = RocgdbMiStoppedScopeV3 {
        stop_identity: identity(8),
        thread: admissions[0].thread,
        wave: RocgdbMiWaveIdentityV3 {
            identity: identity(9),
            thread: admissions[0].thread,
        },
        lane: None,
    };
    assert_eq!(
        process.inspect_registers(scope, timeout),
        Err(RocgdbMiAdapterErrorV3::GpuClassificationUnavailable)
    );

    let breakpoint = process
        .control(
            RocgdbMiControlRequestV3::InsertBreakpoint {
                request_id: 3,
                authorization: RocgdbMiControlAuthorizationV3 {
                    authorization_identity: identity(2),
                    expected_revision: 2,
                },
                site: RocgdbMiBreakpointSiteV3::CodeObjectRelative {
                    code_object: content,
                    kernel_entry_byte_offset: 8,
                },
            },
            timeout,
        )
        .expect("breakpoint");
    assert_eq!(breakpoint.audit.after_revision, 3);

    let continued = process
        .control(
            RocgdbMiControlRequestV3::Continue {
                request_id: 4,
                authorization: RocgdbMiControlAuthorizationV3 {
                    authorization_identity: identity(2),
                    expected_revision: breakpoint.revision,
                },
                focus: admissions[0].thread,
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
        Some(LiveGpuUnavailableReasonV3::Unsupported)
    );
}
