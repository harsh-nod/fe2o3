//! Closed 256 MiB persistent-window experiment; measurements carry no authority.

use super::*;
use fe2o3_kfd::{
    Gfx942PersistentSdmaDirectionV1 as Direction, Gfx942SdmaPersistentWaitCpuV1 as Cpu,
};

const NATIVE_SCHEMA: &str = "fe2o3.kfd-directional-native-wait-diagnostic.v1";

fn validate_v1(run: &CompletedRunV1) -> BenchmarkResult<()> {
    let policy = run
        .config
        .policy
        .native_policy()
        .ok_or("native capture requires a native policy")?;
    if run.config.bytes != MAX_COPY_BYTES || run.rounds.len() != run.config.rounds {
        return Err("native wait diagnostic shape mismatch".into());
    }
    let records = run.native.as_ref().ok_or("missing native wait capture")?;
    if records.len()
        != run
            .config
            .rounds
            .checked_mul(4)
            .ok_or("record count overflow")?
    {
        return Err("native wait diagnostic record count mismatch".into());
    }
    let first_bytes = u64::from(fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) * 63;
    let mut prior_submission = 0;
    for (round_index, chunk) in records.chunks_exact(4).enumerate() {
        for (direction_index, pair) in chunk.chunks_exact(2).enumerate() {
            let direction = if direction_index == 0 {
                Direction::HostToDevice
            } else {
                Direction::DeviceToHost
            };
            let timing = if direction_index == 0 {
                run.rounds[round_index].h2d
            } else {
                run.rounds[round_index].d2h
            };
            if timing.counts.waits != 2
                || timing.counts.flushes != 2
                || timing.total_ns == 0
                || timing.submit_ns.checked_add(timing.progress_ns) != Some(timing.total_ns)
                || pair[0].backend_submission <= prior_submission
            {
                return Err("native wait timing or submission order mismatch".into());
            }
            let scans = pair.iter().try_fold(0_u128, |sum, record| {
                sum.checked_add(u128::from(record.native.scan_ns?))
            });
            if scans.is_none_or(|sum| sum > timing.progress_ns) {
                return Err("native scan intervals exceed enclosing progress interval".into());
            }
            prior_submission = pair[0].backend_submission;
            for (window, record) in pair.iter().enumerate() {
                let offset = if window == 0 { 0 } else { first_bytes };
                let bytes = if window == 0 {
                    first_bytes
                } else {
                    MAX_COPY_BYTES as u64 - first_bytes
                };
                let packets = if window == 0 { 63 } else { 2 };
                if record.backend_submission != prior_submission
                    || record.direction != direction
                    || record.completed_prefix_bytes != offset
                    || record.host_offset != offset
                    || record.device_offset != offset
                    || u64::from(record.window_bytes) != bytes
                    || record.packet_count != packets
                    || record.native.packet_count != packets
                    || record.native.sleep_ceiling != policy
                    || record.native.scan_ns.is_none()
                {
                    return Err("native wait window identity or policy mismatch".into());
                }
                let counters = record
                    .native
                    .counters
                    .ok_or("invalid native wait counters")?;
                let pauses = counters
                    .spin_pauses
                    .checked_add(counters.yield_pauses)
                    .and_then(|sum| sum.checked_add(counters.sleep_pauses));
                if pauses.and_then(|sum| sum.checked_add(1)) != Some(counters.scan_rounds)
                    || counters.scan_rounds.checked_mul(packets as u64)
                        != Some(counters.completion_observations)
                    || u128::from(counters.requested_sleep_ns)
                        > u128::from(counters.sleep_pauses) * u128::from(policy.nanoseconds())
                    || counters.max_requested_sleep_ns > policy.nanoseconds()
                    || counters.max_requested_sleep_ns > counters.requested_sleep_ns
                    || (counters.sleep_pauses == 0 && counters.max_requested_sleep_ns != 0)
                    || u128::from(counters.requested_sleep_ns)
                        > u128::from(counters.sleep_pauses)
                            * u128::from(counters.max_requested_sleep_ns)
                {
                    return Err("native wait counter decomposition mismatch".into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn write_v1(run: &CompletedRunV1, output: &mut impl Write) -> BenchmarkResult<()> {
    validate_v1(run)?;
    let config = run.config;
    let policy = config
        .policy
        .native_policy()
        .expect("validated native policy");
    writeln!(
        output,
        "schema={NATIVE_SCHEMA} record=config backend=kfd unique_id={:016x} target=gfx942:xnack- bytes={} depth=1 warmups={} samples={} wait_policy={} outer_timeout_ns={} active_spin_floor_ns=50000 native_sleep_ceiling_ns={} packets_per_transfer=65 windows_per_transfer=2 max_packets_per_window=63 h2d_engine_index=1 d2h_engine_index=0 optional_context_journal=disabled timing=host-submit-and-progress native_scan_timing=host-scan-including-cpu-observation-overhead",
        config.unique_id,
        config.bytes,
        config.warmups,
        config.samples,
        config.policy.name(),
        COMPLETION_TIMEOUT.as_nanos(),
        policy.nanoseconds(),
    )?;
    for (index, round) in run.rounds.iter().enumerate() {
        let phase = if index < config.warmups {
            "warmup"
        } else {
            "sample"
        };
        writeln!(
            output,
            "schema={NATIVE_SCHEMA} record=round index={index} phase={phase} pattern={} h2d_submit_ns={} h2d_progress_ns={} h2d_total_ns={} h2d_wait_calls={} h2d_flush_calls={} d2h_submit_ns={} d2h_progress_ns={} d2h_total_ns={} d2h_wait_calls={} d2h_flush_calls={} checked_bytes={}",
            pattern(index),
            round.h2d.submit_ns,
            round.h2d.progress_ns,
            round.h2d.total_ns,
            round.h2d.counts.waits,
            round.h2d.counts.flushes,
            round.d2h.submit_ns,
            round.d2h.progress_ns,
            round.d2h.total_ns,
            round.d2h.counts.waits,
            round.d2h.counts.flushes,
            config.bytes,
        )?;
        for (slot, record) in run.native.as_ref().expect("validated capture")
            [index * 4..index * 4 + 4]
            .iter()
            .enumerate()
        {
            let direction = if slot < 2 { "h2d" } else { "d2h" };
            let counters = record.native.counters.expect("validated counters");
            write!(
                output,
                "schema={NATIVE_SCHEMA} record=native-window round={index} phase={phase} direction={direction} window={} backend_submission={} completed_prefix_bytes={} host_offset={} device_offset={} window_bytes={} packet_count={} native_sleep_ceiling_ns={} scan_rounds={} completion_observations={} spin_pauses={} yield_pauses={} sleep_pauses={} requested_sleep_ns={} max_requested_sleep_ns={} scan_ns={}",
                slot % 2,
                record.backend_submission,
                record.completed_prefix_bytes,
                record.host_offset,
                record.device_offset,
                record.window_bytes,
                record.packet_count,
                policy.nanoseconds(),
                counters.scan_rounds,
                counters.completion_observations,
                counters.spin_pauses,
                counters.yield_pauses,
                counters.sleep_pauses,
                counters.requested_sleep_ns,
                counters.max_requested_sleep_ns,
                record.native.scan_ns.expect("validated elapsed interval"),
            )?;
            match record.native.cpu {
                Cpu::Available {
                    thread_cpu_ns,
                    voluntary_context_switches,
                    involuntary_context_switches,
                } => {
                    writeln!(
                        output,
                        " cpu_status=available thread_cpu_ns={thread_cpu_ns} voluntary_context_switches={voluntary_context_switches} involuntary_context_switches={involuntary_context_switches}"
                    )?;
                }
                Cpu::Unavailable | Cpu::Invalid => {
                    let status = if record.native.cpu == Cpu::Unavailable {
                        "unavailable"
                    } else {
                        "invalid"
                    };
                    writeln!(
                        output,
                        " cpu_status={status} thread_cpu_ns=none voluntary_context_switches=none involuntary_context_switches=none"
                    )?;
                }
            }
        }
    }
    writeln!(
        output,
        "schema={NATIVE_SCHEMA} record=complete validated_rounds={} measured_rounds={} checked_bytes_per_round={} native_windows={} validation=full-returned-buffer-every-round teardown=explicit-complete",
        config.rounds,
        config.samples,
        config.bytes,
        config.rounds * 4,
    )?;
    output.flush()?;
    Ok(())
}
