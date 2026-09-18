use super::*;
use fe2o3_kfd::{
    Gfx942PersistentSdmaDirectionV1 as Direction, Gfx942SdmaPersistentWaitCountersV1 as Counters,
    Gfx942SdmaPersistentWaitCpuV1 as Cpu, Gfx942SdmaPersistentWaitDiagnosticsV1 as Diagnostic,
};
use fe2o3_runtime::KfdRuntimeDirectionalWaitObservationV1 as Observation;

fn fixture(policy: WaitPolicyV1) -> CompletedRunV1 {
    let mut run = completed_fixture();
    run.config.policy = policy;
    let first_bytes = u64::from(fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) * 63;
    let mut records = Vec::new();
    for (round_index, round) in run.rounds.iter_mut().enumerate() {
        for timing in [&mut round.h2d, &mut round.d2h] {
            timing.counts = ProgressCountsV1 {
                waits: 2,
                flushes: 2,
            };
            timing.progress_ns = 100;
            timing.total_ns = timing.submit_ns + 100;
        }
        for (direction_index, direction) in [Direction::HostToDevice, Direction::DeviceToHost]
            .into_iter()
            .enumerate()
        {
            for window in 0..2 {
                let offset = if window == 0 { 0 } else { first_bytes };
                let packets = if window == 0 { 63 } else { 2 };
                records.push(Observation {
                    backend_submission: (round_index * 2 + direction_index + 1) as u64,
                    direction,
                    completed_prefix_bytes: offset,
                    host_offset: offset,
                    device_offset: offset,
                    window_bytes: if window == 0 {
                        first_bytes
                    } else {
                        MAX_COPY_BYTES as u64 - first_bytes
                    } as u32,
                    packet_count: packets,
                    native: Diagnostic {
                        sleep_ceiling: policy.native_policy().unwrap(),
                        packet_count: packets,
                        counters: Some(Counters {
                            scan_rounds: 2,
                            completion_observations: 2 * packets as u64,
                            sleep_pauses: 1,
                            requested_sleep_ns: 17,
                            max_requested_sleep_ns: 17,
                            ..Default::default()
                        }),
                        scan_ns: Some(20 + round_index as u64),
                        cpu: match round_index {
                            0 => Cpu::Available {
                                thread_cpu_ns: 3,
                                voluntary_context_switches: 1,
                                involuntary_context_switches: 0,
                            },
                            1 => Cpu::Unavailable,
                            _ => Cpu::Invalid,
                        },
                    },
                });
            }
        }
    }
    run.native = Some(records);
    run
}

#[test]
fn native_modes_require_the_closed_shape_and_use_full_remaining_budgets() {
    for (mode, policy) in [
        ("diagnostic-native-sleep1ms", WaitPolicyV1::NativeSleep1ms),
        ("diagnostic-native-sleep25us", WaitPolicyV1::NativeSleep25us),
    ] {
        let mut args = arguments(mode);
        assert_eq!(parse_config_v1(&args).unwrap().unwrap().policy, policy);
        args[1] = (MAX_COPY_BYTES - 1).to_string();
        assert!(parse_config_v1(&args).is_err());
        let mut driver = Driver::new([
            Step::Flush(Ok(())),
            Step::Wait(Ok(RuntimePollV1::Pending)),
            Step::Flush(Ok(())),
            Step::Wait(Ok(RuntimePollV1::Succeeded)),
        ]);
        let mut remaining = [Duration::from_secs(1), Duration::from_nanos(17)].into_iter();
        let counts = drive_v1(&mut driver, policy, || remaining.next().unwrap()).unwrap();
        assert_eq!(
            counts,
            ProgressCountsV1 {
                waits: 2,
                flushes: 2
            }
        );
        assert_eq!(
            driver.waits,
            [Duration::from_secs(1), Duration::from_nanos(17)]
        );
        assert!(driver.steps.is_empty());
    }
}

#[test]
fn native_schema_preserves_order_cpu_statuses_and_policy() {
    for policy in [WaitPolicyV1::NativeSleep1ms, WaitPolicyV1::NativeSleep25us] {
        let run = fixture(policy);
        let mut output = Vec::new();
        run.write(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        let rows: Vec<_> = output.lines().map(fields).collect();
        assert_eq!(rows.len(), 17);
        assert!(
            rows.iter()
                .all(|row| row["schema"] == "fe2o3.kfd-directional-native-wait-diagnostic.v1")
        );
        assert_eq!(rows[0]["wait_policy"], policy.name());
        assert_eq!(
            rows[0]["native_sleep_ceiling_ns"].parse::<u64>().unwrap(),
            policy.native_policy().unwrap().nanoseconds()
        );
        for round in 0..3 {
            assert_eq!(rows[1 + round * 5]["record"], "round");
            for slot in 0..4 {
                let row = &rows[2 + round * 5 + slot];
                assert_eq!(row["round"].parse::<usize>().unwrap(), round);
                assert_eq!(row["direction"], if slot < 2 { "h2d" } else { "d2h" });
                assert_eq!(row["window"].parse::<usize>().unwrap(), slot % 2);
                assert_eq!(
                    row["backend_submission"].parse::<usize>().unwrap(),
                    round * 2 + slot / 2 + 1
                );
                assert_eq!(row["packet_count"], if slot % 2 == 0 { "63" } else { "2" });
                assert_eq!(row["max_requested_sleep_ns"], "17");
                assert_eq!(
                    row["cpu_status"],
                    ["available", "unavailable", "invalid"][round]
                );
                assert_eq!(row["thread_cpu_ns"], if round == 0 { "3" } else { "none" });
            }
        }
        assert_eq!(rows[16]["record"], "complete");
        assert_eq!(rows[16]["native_windows"], "12");
    }
}

#[test]
fn invalid_native_captures_reject_before_writing_any_bytes() {
    let mutations: &[fn(&mut CompletedRunV1)] = &[
        |run| run.native = None,
        |run| {
            run.native.as_mut().unwrap().pop();
        },
        |run| {
            let rows = run.native.as_mut().unwrap();
            rows.push(rows[0]);
        },
        |run| run.native.as_mut().unwrap().swap(0, 1),
        |run| run.native.as_mut().unwrap()[1].backend_submission += 1,
        |run| run.native.as_mut().unwrap()[0].direction = Direction::DeviceToHost,
        |run| run.native.as_mut().unwrap()[1].completed_prefix_bytes = 0,
        |run| run.native.as_mut().unwrap()[1].host_offset = 0,
        |run| run.native.as_mut().unwrap()[1].device_offset = 0,
        |run| run.native.as_mut().unwrap()[0].window_bytes += 1,
        |run| run.native.as_mut().unwrap()[0].packet_count = 62,
        |run| run.native.as_mut().unwrap()[0].native.packet_count = 62,
        |run| {
            run.native.as_mut().unwrap()[0].native.sleep_ceiling =
                fe2o3_kfd::Gfx942SdmaPersistentDiagnosticSleepCeilingV1::Millis1
        },
        |run| run.native.as_mut().unwrap()[0].native.scan_ns = None,
        |run| run.native.as_mut().unwrap()[0].native.scan_ns = Some(101),
        |run| run.native.as_mut().unwrap()[0].native.counters = None,
        |run| {
            run.native.as_mut().unwrap()[0]
                .native
                .counters
                .as_mut()
                .unwrap()
                .scan_rounds += 1
        },
        |run| {
            run.native.as_mut().unwrap()[0]
                .native
                .counters
                .as_mut()
                .unwrap()
                .completion_observations += 1
        },
        |run| {
            run.native.as_mut().unwrap()[0]
                .native
                .counters
                .as_mut()
                .unwrap()
                .max_requested_sleep_ns = 25_001
        },
        |run| {
            run.native.as_mut().unwrap()[0]
                .native
                .counters
                .as_mut()
                .unwrap()
                .requested_sleep_ns = 25_001
        },
        |run| run.rounds[0].h2d.counts.waits = 3,
        |run| run.rounds[0].h2d.total_ns = 0,
        |run| run.config.bytes -= 1,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut run = fixture(WaitPolicyV1::NativeSleep25us);
        mutate(&mut run);
        let mut output = Vec::new();
        assert!(run.write(&mut output).is_err(), "mutation {index}");
        assert!(output.is_empty(), "mutation {index}");
    }
}

#[test]
fn native_output_propagates_early_late_and_flush_failures() {
    let run = fixture(WaitPolicyV1::NativeSleep25us);
    let mut complete = Vec::new();
    run.write(&mut complete).unwrap();
    for remaining in [0, 100, complete.len() - 1, complete.len()] {
        let mut writer = LimitedWriter {
            remaining,
            fail_flush: remaining == complete.len(),
            bytes: Vec::new(),
        };
        assert!(run.write(&mut writer).is_err());
        assert_eq!(writer.bytes, complete[..remaining]);
    }
}
