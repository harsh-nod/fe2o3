//! Inert snapshot parsing and clock arithmetic, without native completion authority.

use crate::{AMD_SIGNAL_BYTES_V1, AMD_SIGNAL_KIND_USER_V1, AMD_SIGNAL_VALUE_COMPLETE_V1};

pub const AMD_QUEUE_PROPERTIES_OFFSET_V1: usize = 0xb4;
pub const AMD_QUEUE_ENABLE_PROFILING_MASK_V1: u32 = 1 << 3;
pub const AMD_SIGNAL_START_TIMESTAMP_OFFSET_V1: usize = 32;
pub const AMD_SIGNAL_END_TIMESTAMP_OFFSET_V1: usize = 40;

const _: () = {
    assert!(
        core::mem::offset_of!(crate::AmdBusyCompletionSignalV1, start_ts)
            == AMD_SIGNAL_START_TIMESTAMP_OFFSET_V1
    );
    assert!(
        core::mem::offset_of!(crate::AmdBusyCompletionSignalV1, end_ts)
            == AMD_SIGNAL_END_TIMESTAMP_OFFSET_V1
    );
};

pub const AQL_DISPATCH_PROFILING_MANIFEST_V1: &str = r#"schema=rocr-7.2.4-amdhsa-inert-dispatch-profiling-values-v1
platform=linux-x86_64,little-endian,pointer-width:64
rocr_commit=97f5574fe2fdc7bef44fb01545347912ee9f1779,tag:rocm-7.2.4
source.hsa.h=51ea864cc3e83a9ce824c294dd98a5724eeec87b76fafded1a01d406206ce0f5
source.amd_hsa_common.h=abfccdd1eabe77047b16743ce0f729577c99caf8c7b28b880232602a5b46a17f
source.amd_hsa_queue.h=1f45345473ea2a02200748106a43f8aaf97568e11c8182a789684320008592e3
source.amd_hsa_signal.h=ba429b422e91fe370e4241ce8c8d934738b6e3c59b10c1eefd2370d76afe5020
source.queue.h=aa1cd1acea3405e8c18076b406dd91b5433438792f7cbe8ac5bc3d46df25a9ca
queue=properties-offset:180,width:4,profiling-mask:8,default:preserve-all-supplied-bits,opt-in:set-only-profiling-bit
snapshot=exactly-64-bytes,kind:user-1,value:complete-0,event-and-reserved:zero,start:32,end:40,nonzero-ticks,start<=end
clock=caller-supplied-gpu-and-system-counters,nonzero,equal-nonzero-frequency,strictly-increasing-bracket,inclusive-bounds,checked-u128-products,floor-rounding,no-extrapolation,no-host-Instant-domain
authority=inert-values-only,no-native-load,no-acquire-observation,no-signal-generation,no-device-identity,no-currentness,no-clock-sampling,no-queue-mutation,no-profiling-enablement,no-GPU-execution,no-overlap-or-performance-evidence
"#;

pub const AQL_DISPATCH_PROFILING_MANIFEST_SHA256_V1: &str =
    "405cf71113652732b1d3bb7098d0dc3334e91484e86a0fb6922b051e4c783e5a";

/// Pure policy for a supplied property word; it never accesses a native queue.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AmdQueueProfilingPolicyV1 {
    /// Preserves all bits, including any caller-supplied profiling bit.
    #[default]
    Preserve,
    /// Adds the profiling bit without changing any other bit.
    EnableDispatchTimestamps,
}

impl AmdQueueProfilingPolicyV1 {
    pub const fn properties(self, supplied: u32) -> u32 {
        match self {
            Self::Preserve => supplied,
            Self::EnableDispatchTimestamps => supplied | AMD_QUEUE_ENABLE_PROFILING_MASK_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchProfilingValueErrorV1 {
    SnapshotExtent,
    SignalKind(i64),
    SignalValue(i64),
    NonzeroEventOrReservedBytes,
    ZeroGpuTimestamp,
    ReversedGpuInterval,
    ZeroSystemTimestamp,
    ZeroSystemFrequency,
    FrequencyChanged,
    NonIncreasingGpuCorrelation,
    NonIncreasingSystemCorrelation,
    OutsideCorrelation,
    ArithmeticOverflow,
}

/// Caller-supplied GPU ticks, not proof of a dispatch or its completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuDispatchTickIntervalV1 {
    start: u64,
    end: u64,
}

impl GpuDispatchTickIntervalV1 {
    pub const fn new(start: u64, end: u64) -> Result<Self, DispatchProfilingValueErrorV1> {
        if start == 0 || end == 0 {
            return Err(DispatchProfilingValueErrorV1::ZeroGpuTimestamp);
        }
        if end < start {
            return Err(DispatchProfilingValueErrorV1::ReversedGpuInterval);
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }
}

/// Parses caller-owned snapshot bytes only; a zero value is not an acquire proof.
///
/// The later native owner must establish successful acquired completion, exact
/// signal generation/device/queue identity, coherency, and no reuse during the
/// snapshot. This parser cannot establish any of those obligations.
pub fn parse_amd_busy_dispatch_timestamp_snapshot_v1(
    snapshot: &[u8],
) -> Result<GpuDispatchTickIntervalV1, DispatchProfilingValueErrorV1> {
    if snapshot.len() != AMD_SIGNAL_BYTES_V1 {
        return Err(DispatchProfilingValueErrorV1::SnapshotExtent);
    }
    let kind = i64::from_le_bytes(snapshot[0..8].try_into().expect("checked snapshot extent"));
    if kind != AMD_SIGNAL_KIND_USER_V1 {
        return Err(DispatchProfilingValueErrorV1::SignalKind(kind));
    }
    let value = i64::from_le_bytes(snapshot[8..16].try_into().expect("checked snapshot extent"));
    if value != AMD_SIGNAL_VALUE_COMPLETE_V1 {
        return Err(DispatchProfilingValueErrorV1::SignalValue(value));
    }
    if snapshot[16..32]
        .iter()
        .chain(&snapshot[48..64])
        .any(|byte| *byte != 0)
    {
        return Err(DispatchProfilingValueErrorV1::NonzeroEventOrReservedBytes);
    }
    let read_tick = |offset| {
        u64::from_le_bytes(
            snapshot[offset..offset + 8]
                .try_into()
                .expect("checked snapshot extent"),
        )
    };
    GpuDispatchTickIntervalV1::new(
        read_tick(AMD_SIGNAL_START_TIMESTAMP_OFFSET_V1),
        read_tick(AMD_SIGNAL_END_TIMESTAMP_OFFSET_V1),
    )
}

/// Supplied GPU/system correlation numbers, not an authenticated clock sample.
///
/// System ticks are in the supplied system-counter domain, never host `Instant`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuSystemClockSampleV1 {
    gpu: u64,
    system: u64,
    frequency: u64,
}

impl GpuSystemClockSampleV1 {
    pub const fn new(
        gpu: u64,
        system: u64,
        frequency: u64,
    ) -> Result<Self, DispatchProfilingValueErrorV1> {
        if gpu == 0 {
            return Err(DispatchProfilingValueErrorV1::ZeroGpuTimestamp);
        }
        if system == 0 {
            return Err(DispatchProfilingValueErrorV1::ZeroSystemTimestamp);
        }
        if frequency == 0 {
            return Err(DispatchProfilingValueErrorV1::ZeroSystemFrequency);
        }
        Ok(Self {
            gpu,
            system,
            frequency,
        })
    }
}

/// A bounded arithmetic bracket; the owner must bind both samples to one device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuSystemClockBracketV1 {
    before: GpuSystemClockSampleV1,
    after: GpuSystemClockSampleV1,
}

impl GpuSystemClockBracketV1 {
    pub const fn new(
        before: GpuSystemClockSampleV1,
        after: GpuSystemClockSampleV1,
    ) -> Result<Self, DispatchProfilingValueErrorV1> {
        if before.frequency != after.frequency {
            return Err(DispatchProfilingValueErrorV1::FrequencyChanged);
        }
        if after.gpu <= before.gpu {
            return Err(DispatchProfilingValueErrorV1::NonIncreasingGpuCorrelation);
        }
        if after.system <= before.system {
            return Err(DispatchProfilingValueErrorV1::NonIncreasingSystemCorrelation);
        }
        Ok(Self { before, after })
    }

    /// Interpolates within this bracket only, with checked integer arithmetic.
    ///
    /// Endpoints are independently floored to system ticks. Duration is floored
    /// directly from the GPU delta and clock ratio, avoiding endpoint-rounding
    /// error. Unrepresentable intermediate products or nanoseconds are rejected.
    /// This policy does not implement ROCr's drift correction or extrapolation.
    pub fn interpolate(
        self,
        gpu: GpuDispatchTickIntervalV1,
    ) -> Result<CorrelatedSystemClockIntervalV1, DispatchProfilingValueErrorV1> {
        use DispatchProfilingValueErrorV1::ArithmeticOverflow;
        if gpu.start < self.before.gpu || gpu.end > self.after.gpu {
            return Err(DispatchProfilingValueErrorV1::OutsideCorrelation);
        }
        let gpu_span = u128::from(self.after.gpu - self.before.gpu);
        let system_span = u128::from(self.after.system - self.before.system);
        let system_tick = |tick: u64| -> Result<u64, DispatchProfilingValueErrorV1> {
            let scaled = u128::from(tick - self.before.gpu)
                .checked_mul(system_span)
                .ok_or(ArithmeticOverflow)?
                / gpu_span;
            u64::try_from(u128::from(self.before.system) + scaled).map_err(|_| ArithmeticOverflow)
        };
        let numerator = u128::from(gpu.end - gpu.start)
            .checked_mul(system_span)
            .and_then(|value| value.checked_mul(1_000_000_000))
            .ok_or(ArithmeticOverflow)?;
        let denominator = gpu_span
            .checked_mul(u128::from(self.before.frequency))
            .ok_or(ArithmeticOverflow)?;
        Ok(CorrelatedSystemClockIntervalV1 {
            start: system_tick(gpu.start)?,
            end: system_tick(gpu.end)?,
            frequency: self.before.frequency,
            duration_ns: u64::try_from(numerator / denominator).map_err(|_| ArithmeticOverflow)?,
        })
    }
}

/// Arithmetic output in the supplied system-clock domain, with no timing authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CorrelatedSystemClockIntervalV1 {
    start: u64,
    end: u64,
    frequency: u64,
    duration_ns: u64,
}

impl CorrelatedSystemClockIntervalV1 {
    pub const fn start_system_ticks(self) -> u64 {
        self.start
    }

    pub const fn end_system_ticks(self) -> u64 {
        self.end
    }

    pub const fn system_frequency_hz(self) -> u64 {
        self.frequency
    }

    pub const fn duration_ns_floor(self) -> u64 {
        self.duration_ns
    }
}
