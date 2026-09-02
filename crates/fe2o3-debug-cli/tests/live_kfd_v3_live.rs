#![cfg(all(
    feature = "live-validation",
    target_os = "linux",
    target_arch = "x86_64"
))]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::*;
use fe2o3_kernel_ir::{
    DebugSourceMapBindingV1, DebugSourceMapDocumentV1, DebugSourceMapDocumentV2,
    DebugSourceScopeV2, DebugSourceVariableBindingV2, DebugSourceVariableFallbackV2,
    DebugSourceVariableLocationV2, DebugSourceVariableV2, SimulationCompilerExecutionBindingV1,
    SimulationProductionKirIdentityV1, SimulationSourceLineageV1, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV1, VerifiedSimulationBundleV2,
    decode_module_v7,
};
use fe2o3_kfd::{
    DeviceSelector, KfdTargetDebugArtifactIdentityV1, KfdTargetDebugArtifactRoleV1,
    KfdTargetDebugTelemetryDigestV1, KfdTargetDebugTelemetryPayloadV1,
    KfdTargetRuntimeDebugTokenV1, OpenedKfd, admit_inherited_kfd_target_debug_telemetry_v1,
};
use sha2::{Digest, Sha256};

const TARGET_ENV: &str = "FE2O3_LIVE_KFD_V3_LIVE_TARGET";
const TARGET_HSACO_ENV: &str = "FE2O3_LIVE_KFD_V3_DECLARED_HSACO";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-live-kfd-v3-live-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn finish(mut self) {
        let mut child = self.0.take().expect("debugger child remains owned");
        let status = match wait_bounded(&mut child, RESPONSE_TIMEOUT) {
            Some(status) => status,
            None => {
                let _ = child.kill();
                wait_bounded(&mut child, Duration::from_secs(3))
                    .expect("debugger did not exit after bounded kill")
            }
        };
        assert!(status.success(), "debugger failed: {status}");
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = wait_bounded(child, Duration::from_secs(3));
        }
    }
}

fn wait_bounded(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn target_artifact(bytes: &[u8]) -> KfdTargetDebugArtifactIdentityV1 {
    KfdTargetDebugArtifactIdentityV1::new(
        KfdTargetDebugTelemetryDigestV1::from_bytes(digest(bytes)).unwrap(),
        u64::try_from(bytes.len()).unwrap(),
    )
    .unwrap()
}

#[test]
fn live_kfd_v3_live_target() {
    if std::env::var_os(TARGET_ENV).is_none() {
        return;
    }

    let executable = fs::read(std::env::current_exe().unwrap()).unwrap();
    let hsaco = fs::read(
        std::env::var_os(TARGET_HSACO_ENV).expect("target HSACO declaration path is provisioned"),
    )
    .unwrap();
    let executable_identity = target_artifact(&executable);
    let hsaco_identity = target_artifact(&hsaco);
    let process_instance = KfdTargetDebugTelemetryDigestV1::from_bytes(digest(
        [
            b"fe2o3-live-kfd-v3-live-target-process\0".as_slice(),
            executable_identity.digest().as_bytes(),
        ]
        .concat()
        .as_slice(),
    ))
    .unwrap();
    let mut telemetry = admit_inherited_kfd_target_debug_telemetry_v1()
        .unwrap()
        .expect("live target must inherit the cooperative telemetry endpoint");
    telemetry
        .send(KfdTargetDebugTelemetryPayloadV1::SessionStarted {
            process_instance,
            executable: executable_identity,
        })
        .unwrap();
    telemetry
        .send(KfdTargetDebugTelemetryPayloadV1::Artifact {
            role: KfdTargetDebugArtifactRoleV1::ApplicationExecutable,
            ordinal: 0,
            artifact: executable_identity,
        })
        .unwrap();
    telemetry
        .send(KfdTargetDebugTelemetryPayloadV1::Artifact {
            role: KfdTargetDebugArtifactRoleV1::CodeObject,
            ordinal: 0,
            artifact: hsaco_identity,
        })
        .unwrap();

    let unique_id = fe2o3_kfd::topology::discover_default_topology()
        .unwrap()
        .topology()
        .gpu_nodes()
        .first()
        .expect("live KFD test requires one GPU")
        .unique_id();
    let device = OpenedKfd::open_default()
        .unwrap()
        .admit_uapi()
        .unwrap()
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))
        .unwrap();
    let token = KfdTargetRuntimeDebugTokenV1::enable_current_process().unwrap();
    let _queue = token.create_compute_aql_queue(device, 4096).unwrap();
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn mi300x_live_kfd_v3_binds_observes_controls_and_terminates() {
    if std::env::var_os(TARGET_ENV).is_some() {
        return;
    }
    match fs::metadata(fe2o3_kfd::DEFAULT_KFD_PATH) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "SKIP[device_absent]: {} is absent",
                fe2o3_kfd::DEFAULT_KFD_PATH
            );
            return;
        }
        Err(error) => panic!("could not inspect {}: {error}", fe2o3_kfd::DEFAULT_KFD_PATH),
    }

    let directory = TestDirectory::new();
    let inputs = write_inputs(&directory);
    let debugger = env!("CARGO_BIN_EXE_fe2o3-debug");
    let target = std::env::current_exe().unwrap();
    let mut child = Command::new(debugger)
        .arg("live-kfd")
        .arg("--bundle-v2")
        .arg(&inputs.bundle)
        .arg("--request")
        .arg(&inputs.request)
        .arg("--hsaco")
        .arg(&inputs.hsaco)
        .args(["--protocol", "jsonl", "--wave-width", "64", "--"])
        .arg(&target)
        .args(["--exact", "live_kfd_v3_live_target", "--nocapture"])
        .env(TARGET_ENV, "1")
        .env(TARGET_HSACO_ENV, &inputs.hsaco)
        .env_remove("RUST_MIN_STACK")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut reader = BufReader::new(output);
        loop {
            let mut line = Vec::new();
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let child = ChildGuard(Some(child));

    let declared_hsaco = LiveGpuContentIdentityV3 {
        digest: OpaqueIdentityV1::new(digest(&inputs.hsaco_bytes)).unwrap(),
        canonical_bytes: u64::try_from(inputs.hsaco_bytes.len()).unwrap(),
    };
    let mut observed_binding = None;
    let mut runtime_enabled = false;
    for request_id in 1..500 {
        let response = exchange(
            &mut input,
            &receiver,
            LiveGpuDebugRequestV3::GetSessionBinding {
                schema: LiveGpuRequestSchemaV3::V3,
                request_id,
                expected_revision: 0,
            },
        );
        if let LiveGpuDebugResponseV3::Ok {
            session, result, ..
        } = response
        {
            runtime_enabled |= session.runtime_enabled;
            if let LiveGpuDebugResultV3::SessionBinding { binding } = *result
                && matches!(
                    binding.target_declared_code_object,
                    LiveGpuAvailabilityV3::Available { .. }
                )
            {
                observed_binding = Some(binding);
            }
        }
        if runtime_enabled && observed_binding.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        runtime_enabled,
        "target runtime transition was not observed"
    );
    let binding = observed_binding.expect("target telemetry binding was not observed");
    assert_eq!(binding.declared_code_object, declared_hsaco);
    assert!(matches!(
        binding.target_declared_code_object,
        LiveGpuAvailabilityV3::Available { value, ref truth }
            if value == declared_hsaco && truth.origin == LiveGpuTruthOriginV3::Declared
    ));
    assert!(matches!(
        binding.target_telemetry,
        LiveGpuAvailabilityV3::Available { ref value, ref truth }
            if value.records == 3
                && value.artifact_records == 2
                && value.dispatch_records == 0
                && value.allocation_records == 0
                && value.diagnostic_records == 0
                && !value.session_ended
                && truth.origin == LiveGpuTruthOriginV3::Declared
    ));
    assert!(matches!(
        binding.execution_code_object,
        LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            ..
        }
    ));

    let state = exchange(
        &mut input,
        &receiver,
        LiveGpuDebugRequestV3::GetState {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id: 501,
            expected_revision: 0,
        },
    );
    assert!(matches!(
        state,
        LiveGpuDebugResponseV3::Ok {
            session: LiveGpuSessionViewV3 {
                backend: LiveGpuBackendV3::DirectKfd,
                state: LiveGpuSessionStateV3::Running,
                runtime_enabled: true,
                ..
            },
            result,
            ..
        } if matches!(
            *result,
            LiveGpuDebugResultV3::State {
                stopped: LiveGpuAvailabilityV3::Unavailable {
                    reason: LiveGpuUnavailableReasonV3::SessionNotStopped,
                    truth: LiveGpuTruthV3 {
                        origin: LiveGpuTruthOriginV3::Unavailable,
                        ref evidence,
                    },
                },
            } if evidence.is_empty()
        )
    ));

    let stopped_query = exchange(
        &mut input,
        &receiver,
        LiveGpuDebugRequestV3::InspectStoppedScopes {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id: 502,
            expected_revision: 0,
            binding_identity: binding.binding_identity,
            stop_identity: OpaqueIdentityV1::new([0x51; 32]).unwrap(),
            scope: LiveGpuScopeSelectorV3::Dispatch {
                dispatch: LiveGpuDispatchIdentityV3 {
                    domain: LiveGpuDispatchIdentityDomainV3::RuntimeModel,
                    identity: OpaqueIdentityV1::new([0x52; 32]).unwrap(),
                },
            },
            page: LiveGpuPageRequestV3 {
                snapshot_identity: OpaqueIdentityV1::new([0x53; 32]).unwrap(),
                start: 0,
                limit: 16,
            },
        },
    );
    assert!(matches!(
        stopped_query,
        LiveGpuDebugResponseV3::Unavailable {
            operation: LiveGpuOperationV3::InspectStoppedScopes,
            session: LiveGpuSessionViewV3 {
                state: LiveGpuSessionStateV3::Running,
                ..
            },
            reason: LiveGpuUnavailableReasonV3::Unsupported,
            ..
        }
    ));

    let discovery_deadline = Instant::now() + RESPONSE_TIMEOUT;
    let mut request_id = 510;
    let (queue, device_capable) = loop {
        assert!(
            Instant::now() < discovery_deadline,
            "debugger did not observe one complete target queue snapshot"
        );
        let queues = exchange(
            &mut input,
            &receiver,
            LiveGpuDebugRequestV3::InspectHardwareQueues {
                schema: LiveGpuRequestSchemaV3::V3,
                request_id,
                expected_revision: 0,
                page: hardware_page(0),
            },
        );
        request_id += 1;
        let candidate = match queues {
            LiveGpuDebugResponseV3::Ok {
                session:
                    LiveGpuSessionViewV3 {
                        state: LiveGpuSessionStateV3::Running,
                        runtime_enabled: true,
                        ..
                    },
                result,
                ..
            } => match *result {
                LiveGpuDebugResultV3::Hardware {
                    hardware:
                        HardwareDebugResultV2::Queues {
                            generation,
                            items,
                            next_start: 0,
                        },
                } if items.len() == 1
                    && items[0].ring_bytes == 4096
                    && items[0].context_save_area_bytes > 0
                    && !items[0].suspended_by_session =>
                {
                    Some((generation, items[0]))
                }
                _ => None,
            },
            LiveGpuDebugResponseV3::Error {
                session:
                    LiveGpuSessionViewV3 {
                        state: LiveGpuSessionStateV3::Poisoned | LiveGpuSessionStateV3::Terminated,
                        ..
                    },
                error,
                ..
            } => panic!("debugger became terminal during queue discovery: {error:?}"),
            _ => None,
        };
        let Some((generation, candidate)) = candidate else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };

        let devices = exchange(
            &mut input,
            &receiver,
            LiveGpuDebugRequestV3::InspectHardwareDevices {
                schema: LiveGpuRequestSchemaV3::V3,
                request_id,
                expected_revision: 0,
                page: hardware_page(generation),
            },
        );
        request_id += 1;
        let device = match devices {
            LiveGpuDebugResponseV3::Ok {
                session:
                    LiveGpuSessionViewV3 {
                        state: LiveGpuSessionStateV3::Running,
                        runtime_enabled: true,
                        ..
                    },
                result,
                ..
            } => match *result {
                LiveGpuDebugResultV3::Hardware {
                    hardware:
                        HardwareDebugResultV2::Devices {
                            generation: device_generation,
                            items,
                            next_start: 0,
                        },
                } if device_generation == generation => items
                    .into_iter()
                    .find(|device| device.id == candidate.device),
                _ => None,
            },
            LiveGpuDebugResponseV3::Error {
                session:
                    LiveGpuSessionViewV3 {
                        state: LiveGpuSessionStateV3::Poisoned | LiveGpuSessionStateV3::Terminated,
                        ..
                    },
                error,
                ..
            } => panic!("debugger became terminal during device discovery: {error:?}"),
            _ => None,
        };
        let Some(device) = device else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };
        assert_eq!(device.gfx_target_version, 90_402);
        assert_eq!(device.xcc_count, 8);
        break (candidate.id, device.trap_debug_supported);
    };
    if !device_capable {
        eprintln!("SKIP[device_capability_absent]: KFD reports no trap-debug device");
        terminate(&mut input, &receiver, request_id, 0);
        drop(input);
        child.finish();
        return;
    }

    let suspended = exchange(
        &mut input,
        &receiver,
        LiveGpuDebugRequestV3::SuspendQueues {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id,
            expected_revision: 0,
            queues: vec![queue],
            grace_period: 0,
        },
    );
    request_id += 1;
    assert_control_committed(suspended, 1);
    let captured = exchange(
        &mut input,
        &receiver,
        LiveGpuDebugRequestV3::CaptureStoppedQueueEnvelope {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id,
            expected_revision: 1,
            queue,
        },
    );
    request_id += 1;
    assert!(matches!(
        captured,
        LiveGpuDebugResponseV3::Ok {
            operation: LiveGpuOperationV3::CaptureStoppedQueueEnvelope,
            session: LiveGpuSessionViewV3 {
                state: LiveGpuSessionStateV3::Running,
                revision: 1,
                runtime_enabled: true,
                ..
            },
            result,
            ..
        } if matches!(
            *result,
            LiveGpuDebugResultV3::StoppedQueueEnvelope {
                envelope: LiveGpuStoppedQueueEnvelopeV3 {
                    queue: actual_queue,
                    gfx_target_version: 90_402,
                    xcc_count: 8,
                    ownership: LiveGpuStoppedQueueOwnershipV3::SessionRetainedSuspension,
                    resume_required: true,
                    context_save: LiveGpuStoppedQueueContextSaveV3::Available {
                        ref headers,
                        ..
                    },
                    hardware_checkpoint_bytes: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::HardwareCheckpointBytesNotCpuVisible,
                    },
                    waves: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::WaveRecordLayoutNotInKfdUapi,
                    },
                    lanes: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::LaneStateRequiresWaveRecords,
                    },
                    registers: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::RegisterRecordLayoutNotInKfdUapi,
                    },
                    program_counter: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::ProgramCounterRequiresRegisterRecord,
                    },
                    source: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::SourceMapNotBound,
                    },
                    memory: LiveGpuStoppedQueueUnavailableV3 {
                        reason: LiveGpuStoppedQueueUnavailableReasonV3::MemoryValuesNotCaptured,
                    },
                    ..
                },
            } if actual_queue == queue && headers.len() == 8
        )
    ));
    let resumed = exchange(
        &mut input,
        &receiver,
        LiveGpuDebugRequestV3::ResumeQueues {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id,
            expected_revision: 1,
            queues: vec![queue],
        },
    );
    request_id += 1;
    assert_control_committed(resumed, 2);

    terminate(&mut input, &receiver, request_id, 2);
    drop(input);
    child.finish();
}

fn hardware_page(expected_generation: u64) -> HardwarePageRequestV2 {
    HardwarePageRequestV2 {
        expected_generation,
        start: 0,
        limit: 16,
    }
}

fn exchange(
    input: &mut impl Write,
    receiver: &mpsc::Receiver<Vec<u8>>,
    request: LiveGpuDebugRequestV3,
) -> LiveGpuDebugResponseV3 {
    serde_json::to_writer(&mut *input, &request).unwrap();
    input.write_all(b"\n").unwrap();
    input.flush().unwrap();
    let line = receiver
        .recv_timeout(RESPONSE_TIMEOUT)
        .expect("timed out waiting for debugger response");
    decode_live_gpu_response_line_v3(&line, LiveGpuProtocolLimitsV3::default()).unwrap()
}

fn assert_control_committed(response: LiveGpuDebugResponseV3, revision: u64) {
    assert!(matches!(
        response,
        LiveGpuDebugResponseV3::Ok {
            session: LiveGpuSessionViewV3 { revision: actual, .. },
            result,
            ..
        } if actual == revision && matches!(
            *result,
            LiveGpuDebugResultV3::Hardware {
                hardware: HardwareDebugResultV2::QueueControl {
                    effect: HardwareEffectV2::Committed,
                    ..
                }
            }
        )
    ));
}

fn terminate(
    input: &mut impl Write,
    receiver: &mpsc::Receiver<Vec<u8>>,
    request_id: u64,
    expected_revision: u64,
) {
    let response = exchange(
        input,
        receiver,
        LiveGpuDebugRequestV3::Terminate {
            schema: LiveGpuRequestSchemaV3::V3,
            request_id,
            expected_revision,
        },
    );
    assert!(matches!(
        response,
        LiveGpuDebugResponseV3::Ok {
            session: LiveGpuSessionViewV3 {
                state: LiveGpuSessionStateV3::Terminated,
                ..
            },
            result,
            ..
        } if matches!(*result, LiveGpuDebugResultV3::Terminated)
    ));
}

struct Inputs {
    bundle: PathBuf,
    request: PathBuf,
    hsaco: PathBuf,
    hsaco_bytes: Vec<u8>,
}

fn write_inputs(directory: &TestDirectory) -> Inputs {
    let inner = inner_bundle();
    let map = source_map_v2(&inner);
    let bundle_bytes = VerifiedSimulationBundleV2::new(inner, map)
        .unwrap()
        .canonical_bytes()
        .to_vec();
    let request_bytes =
        fs::read(workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"))
            .unwrap();
    let hsaco_bytes = valid_gfx942_hsaco("kernel");
    let bundle = directory.0.join("kernel.fe2sim-v2");
    let request = directory.0.join("request.json");
    let hsaco = directory.0.join("kernel.hsaco");
    fs::write(&bundle, bundle_bytes).unwrap();
    fs::write(&request, request_bytes).unwrap();
    fs::write(&hsaco, &hsaco_bytes).unwrap();
    Inputs {
        bundle,
        request,
        hsaco,
        hsaco_bytes,
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is in workspace/crates")
        .to_owned()
}

fn inner_bundle() -> VerifiedSimulationBundleV1 {
    let kir =
        fs::read(workspace_root().join("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"))
            .unwrap();
    let canonical = VerifiedCanonicalKernelIrV7::from_canonical_bytes(kir.clone()).unwrap();
    let production =
        VerifiedCanonicalKernelIrV8::from_module(decode_module_v7(&kir).unwrap()).unwrap();
    VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([3; 32], 33, [4; 32], 44).unwrap(),
        SimulationProductionKirIdentityV1::v8(
            *production.identity().digest(),
            production.identity().canonical_length(),
        )
        .unwrap(),
        "gfx942:xnack-",
        canonical,
        None,
    )
    .unwrap()
}

fn source_map_v2(inner: &VerifiedSimulationBundleV1) -> DebugSourceMapDocumentV2 {
    let v1 = DebugSourceMapDocumentV1::from_json_bytes(
        &fs::read(workspace_root().join("crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json"))
            .unwrap(),
    )
    .unwrap();
    let span = v1.sites()[0].spans()[0];
    let scope = DebugSourceScopeV2::new([0x31; 32], 0, None, 0, span).unwrap();
    let variable = DebugSourceVariableV2::new(
        [0x41; 32],
        "buffer".into(),
        0,
        scope.identity(),
        DebugSourceVariableFallbackV2::NotInScope,
        vec![
            DebugSourceVariableLocationV2::new(
                0,
                0,
                1,
                DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
            )
            .unwrap(),
        ],
    )
    .unwrap();
    DebugSourceMapDocumentV2::new(
        DebugSourceMapBindingV1::new(
            *inner.subject_identity(),
            *inner.canonical_kir_v7_identity().digest(),
            inner.canonical_kir_v7_identity().canonical_length(),
        )
        .unwrap(),
        v1.files().to_vec(),
        v1.sites().to_vec(),
        v1.eliminated().to_vec(),
        vec![scope],
        vec![variable],
    )
    .unwrap()
}

fn valid_gfx942_hsaco(kernel: &str) -> Vec<u8> {
    let mut metadata = Vec::new();
    msgpack_map(&mut metadata, 3);
    msgpack_string(&mut metadata, "amdhsa.version");
    msgpack_array(&mut metadata, 2);
    msgpack_unsigned(&mut metadata, 1);
    msgpack_unsigned(&mut metadata, 2);
    msgpack_string(&mut metadata, "amdhsa.target");
    msgpack_string(&mut metadata, "amdgcn-amd-amdhsa--gfx942");
    msgpack_string(&mut metadata, "amdhsa.kernels");
    msgpack_array(&mut metadata, 1);
    msgpack_map(&mut metadata, 10);
    for (name, value) in [
        (".name", FixtureValue::String(kernel)),
        (".symbol", FixtureValue::String("kernel.kd")),
        (".kernarg_segment_size", FixtureValue::Unsigned(0)),
        (".kernarg_segment_align", FixtureValue::Unsigned(8)),
        (".group_segment_fixed_size", FixtureValue::Unsigned(0)),
        (".private_segment_fixed_size", FixtureValue::Unsigned(0)),
        (".wavefront_size", FixtureValue::Unsigned(64)),
        (".sgpr_count", FixtureValue::Unsigned(8)),
        (".vgpr_count", FixtureValue::Unsigned(4)),
        (".max_flat_workgroup_size", FixtureValue::Unsigned(64)),
    ] {
        msgpack_string(&mut metadata, name);
        match value {
            FixtureValue::String(value) => msgpack_string(&mut metadata, value),
            FixtureValue::Unsigned(value) => msgpack_unsigned(&mut metadata, value),
        }
    }
    hsaco_with_metadata(&metadata)
}

enum FixtureValue<'a> {
    String(&'a str),
    Unsigned(u64),
}

fn msgpack_string(bytes: &mut Vec<u8>, value: &str) {
    if value.len() < 32 {
        bytes.push(0xa0 | u8::try_from(value.len()).unwrap());
    } else {
        bytes.extend_from_slice(&[0xd9, u8::try_from(value.len()).unwrap()]);
    }
    bytes.extend_from_slice(value.as_bytes());
}

fn msgpack_unsigned(bytes: &mut Vec<u8>, value: u64) {
    if value < 128 {
        bytes.push(u8::try_from(value).unwrap());
    } else {
        bytes.push(0xcf);
        bytes.extend_from_slice(&value.to_be_bytes());
    }
}

fn msgpack_array(bytes: &mut Vec<u8>, length: u8) {
    assert!(length < 16);
    bytes.push(0x90 | length);
}

fn msgpack_map(bytes: &mut Vec<u8>, length: u8) {
    assert!(length < 16);
    bytes.push(0x80 | length);
}

fn hsaco_with_metadata(metadata: &[u8]) -> Vec<u8> {
    const ELF_HEADER_BYTES: usize = 64;
    const SECTION_HEADER_BYTES: usize = 64;
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
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
    write_u32(&mut bytes, 48, 0x54c);
    write_u64(&mut bytes, 40, u64::try_from(section_offset).unwrap());
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(
        &mut bytes,
        note_header + 24,
        u64::try_from(note_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        note_header + 32,
        u64::try_from(note.len()).unwrap(),
    );
    write_u64(&mut bytes, note_header + 48, 4);

    let strings_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strings_header, 7);
    write_u32(&mut bytes, strings_header + 4, 3);
    write_u64(
        &mut bytes,
        strings_header + 24,
        u64::try_from(string_table_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        strings_header + 32,
        u64::try_from(string_table.len()).unwrap(),
    );
    write_u64(&mut bytes, strings_header + 48, 1);
    bytes
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let padding = (alignment - bytes.len() % alignment) % alignment;
    bytes.resize(bytes.len() + padding, 0);
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
