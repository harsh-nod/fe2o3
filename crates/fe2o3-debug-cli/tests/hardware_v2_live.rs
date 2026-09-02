#![cfg(all(
    feature = "live-validation",
    target_os = "linux",
    target_arch = "x86_64"
))]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::*;
use fe2o3_kfd::{DeviceSelector, KfdTargetRuntimeDebugTokenV1, OpenedKfd};

const TARGET_ENV: &str = "FE2O3_HARDWARE_DEBUG_V2_LIVE_TARGET";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

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

#[test]
fn hardware_v2_live_target() {
    if std::env::var_os(TARGET_ENV).is_none() {
        return;
    }
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
fn mi300x_launch_runtime_snapshot_event_suspend_resume_and_cleanup() {
    if std::env::var_os(TARGET_ENV).is_some() {
        return;
    }
    match std::fs::metadata(fe2o3_kfd::DEFAULT_KFD_PATH) {
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

    let debugger = env!("CARGO_BIN_EXE_fe2o3-debug");
    let target = std::env::current_exe().unwrap();
    let mut child = Command::new(debugger)
        .arg("hardware")
        .arg("--")
        .arg(target)
        .args(["--exact", "hardware_v2_live_target", "--nocapture"])
        .env(TARGET_ENV, "1")
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

    let mut runtime_enabled = false;
    for request_id in 1..20 {
        let state = exchange(
            &mut input,
            &receiver,
            HardwareDebugRequestV2::GetState {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision: 0,
            },
        );
        runtime_enabled = matches!(
            state,
            HardwareDebugResponseV2::Ok {
                session: HardwareSessionViewV2 {
                    runtime_enabled: true,
                    observation_sequence: 1..,
                    ..
                },
                ..
            }
        );
        if runtime_enabled {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        runtime_enabled,
        "target runtime transition was not observed"
    );

    let mut queue = None;
    let mut device_capable = false;
    for request_id in 20..40 {
        let devices = exchange(
            &mut input,
            &receiver,
            HardwareDebugRequestV2::InspectHardwareDevices {
                schema: HardwareRequestSchemaV2::V2,
                request_id,
                expected_control_revision: 0,
                page: HardwarePageRequestV2 {
                    expected_generation: 0,
                    start: 0,
                    limit: 16,
                },
            },
        );
        if let HardwareDebugResponseV2::Ok {
            result: HardwareDebugResultV2::Devices { items, .. },
            ..
        } = devices
        {
            device_capable = items.iter().any(|device| device.trap_debug_supported);
        }
        let queues = exchange(
            &mut input,
            &receiver,
            HardwareDebugRequestV2::InspectHardwareQueues {
                schema: HardwareRequestSchemaV2::V2,
                request_id: request_id + 100,
                expected_control_revision: 0,
                page: HardwarePageRequestV2 {
                    expected_generation: 0,
                    start: 0,
                    limit: 16,
                },
            },
        );
        if let HardwareDebugResponseV2::Ok {
            result: HardwareDebugResultV2::Queues { items, .. },
            ..
        } = queues
        {
            queue = items.first().map(|item| item.id);
        }
        if queue.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let queue = queue.expect("debugger did not observe the real target queue");
    if !device_capable {
        eprintln!("SKIP[device_capability_absent]: KFD reports no trap-debug device");
        terminate(&mut input, &receiver, 200, 0);
        drop(input);
        child.finish();
        return;
    }

    let suspended = exchange(
        &mut input,
        &receiver,
        HardwareDebugRequestV2::SuspendQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 201,
            expected_control_revision: 0,
            queues: vec![queue],
            grace_period: 0,
        },
    );
    assert_control_committed(suspended, 1);
    let resumed = exchange(
        &mut input,
        &receiver,
        HardwareDebugRequestV2::ResumeQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 202,
            expected_control_revision: 1,
            queues: vec![queue],
        },
    );
    assert_control_committed(resumed, 2);

    let events = exchange(
        &mut input,
        &receiver,
        HardwareDebugRequestV2::QueryHardwareExceptionEvents {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 203,
            expected_control_revision: 2,
            page: HardwareEventPageRequestV2 {
                after_sequence: 0,
                limit: 32,
                wait_milliseconds: 0,
            },
        },
    );
    assert!(matches!(
        events,
        HardwareDebugResponseV2::Ok {
            result: HardwareDebugResultV2::Events { ref items, .. },
            ..
        } if items.iter().any(|event| matches!(
            event.payload,
            HardwareEventPayloadV2::RuntimeTransition {
                state: HardwareRuntimeStateV2::Enabled
            }
        ))
    ));

    terminate(&mut input, &receiver, 204, 2);
    drop(input);
    child.finish();
}

fn exchange(
    input: &mut impl Write,
    receiver: &mpsc::Receiver<Vec<u8>>,
    request: HardwareDebugRequestV2,
) -> HardwareDebugResponseV2 {
    serde_json::to_writer(&mut *input, &request).unwrap();
    input.write_all(b"\n").unwrap();
    input.flush().unwrap();
    let line = receiver
        .recv_timeout(RESPONSE_TIMEOUT)
        .expect("timed out waiting for debugger response");
    decode_hardware_response_line_v2(&line, HardwareProtocolLimitsV2::default()).unwrap()
}

fn assert_control_committed(response: HardwareDebugResponseV2, revision: u64) {
    assert!(matches!(
        response,
        HardwareDebugResponseV2::Ok {
            session: HardwareSessionViewV2 {
                control_revision,
                ..
            },
            result: HardwareDebugResultV2::QueueControl {
                effect: HardwareEffectV2::Committed,
                ..
            },
            ..
        } if control_revision == revision
    ));
}

fn terminate(
    input: &mut impl Write,
    receiver: &mpsc::Receiver<Vec<u8>>,
    request_id: u64,
    expected_control_revision: u64,
) {
    let response = exchange(
        input,
        receiver,
        HardwareDebugRequestV2::Terminate {
            schema: HardwareRequestSchemaV2::V2,
            request_id,
            expected_control_revision,
        },
    );
    assert!(matches!(
        response,
        HardwareDebugResponseV2::Ok {
            session: HardwareSessionViewV2 {
                state: HardwareSessionStateV2::Terminated,
                ..
            },
            result: HardwareDebugResultV2::Terminated,
            ..
        }
    ));
}
