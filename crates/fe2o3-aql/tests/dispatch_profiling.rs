use fe2o3_aql::{
    AMD_QUEUE_ENABLE_PROFILING_MASK_V1, AMD_QUEUE_PROPERTIES_OFFSET_V1,
    AMD_SIGNAL_END_TIMESTAMP_OFFSET_V1, AMD_SIGNAL_START_TIMESTAMP_OFFSET_V1,
    AQL_DISPATCH_PROFILING_MANIFEST_SHA256_V1, AQL_DISPATCH_PROFILING_MANIFEST_V1,
    AmdQueueProfilingPolicyV1, DispatchProfilingValueErrorV1 as Error,
    GpuDispatchTickIntervalV1 as Ticks, GpuSystemClockBracketV1 as Bracket,
    GpuSystemClockSampleV1 as Sample, parse_amd_busy_dispatch_timestamp_snapshot_v1 as parse,
};
use sha2::{Digest, Sha256};

fn sample(gpu: u64, system: u64, frequency: u64) -> Sample {
    Sample::new(gpu, system, frequency).unwrap()
}

fn snapshot(start: u64, end: u64) -> [u8; 64] {
    let mut bytes = [0; 64];
    bytes[0] = 1;
    bytes[32..40].copy_from_slice(&start.to_le_bytes());
    bytes[40..48].copy_from_slice(&end.to_le_bytes());
    bytes
}

#[test]
fn manifest_is_frozen_and_explicitly_inert() {
    let actual: String = Sha256::digest(AQL_DISPATCH_PROFILING_MANIFEST_V1)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(actual, AQL_DISPATCH_PROFILING_MANIFEST_SHA256_V1);
    for obligation in [
        "no-native-load",
        "no-acquire-observation",
        "no-signal-generation",
        "no-host-Instant-domain",
    ] {
        assert!(AQL_DISPATCH_PROFILING_MANIFEST_V1.contains(obligation));
    }
}

#[test]
fn properties_match_the_independent_c_oracle_and_default_preserves_every_bit() {
    assert_eq!(AMD_QUEUE_PROPERTIES_OFFSET_V1, 180);
    assert_eq!(AMD_QUEUE_ENABLE_PROFILING_MASK_V1, 8);
    for (input, output) in [
        (0, 8),
        (2, 10),
        (8, 8),
        (0xa5a5_5a5a, 0xa5a5_5a5a),
        (u32::MAX, u32::MAX),
    ] {
        assert_eq!(
            AmdQueueProfilingPolicyV1::default().properties(input),
            input
        );
        assert_eq!(
            AmdQueueProfilingPolicyV1::EnableDispatchTimestamps.properties(input),
            output
        );
    }
    for bit in 0..32 {
        for input in [1 << bit, !(1 << bit)] {
            assert_eq!(
                AmdQueueProfilingPolicyV1::default().properties(input),
                input
            );
            let enabled = AmdQueueProfilingPolicyV1::EnableDispatchTimestamps.properties(input);
            assert_eq!((enabled ^ input) & !8, 0);
            assert_ne!(enabled & 8, 0);
        }
    }
}

#[test]
fn snapshot_matches_the_independent_c_signal_image() {
    assert_eq!(AMD_SIGNAL_START_TIMESTAMP_OFFSET_V1, 32);
    assert_eq!(AMD_SIGNAL_END_TIMESTAMP_OFFSET_V1, 40);
    let encoded = "01000000000000000000000000000000000000000000000000000000000000000807060504030201887766554433221100000000000000000000000000000000";
    let bytes: Vec<u8> = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.start(), 0x0102_0304_0506_0708);
    assert_eq!(parsed.end(), 0x1122_3344_5566_7788);
}

#[test]
fn snapshot_extent_kind_and_success_value_are_exact() {
    for size in [0, 1, 31, 40, 63, 65, 128] {
        assert_eq!(parse(&vec![0; size]), Err(Error::SnapshotExtent));
    }
    for kind in [0_i64, -1, -2, 2, i64::MAX] {
        let mut bytes = snapshot(10, 20);
        bytes[..8].copy_from_slice(&kind.to_le_bytes());
        assert_eq!(parse(&bytes), Err(Error::SignalKind(kind)));
    }
    for value in [1_i64, -1, 2, i64::MIN, i64::MAX] {
        let mut bytes = snapshot(10, 20);
        bytes[8..16].copy_from_slice(&value.to_le_bytes());
        assert_eq!(parse(&bytes), Err(Error::SignalValue(value)));
    }
}

#[test]
fn busy_snapshot_rejects_each_event_or_reserved_byte() {
    for offset in (16..32).chain(48..64) {
        let mut bytes = snapshot(10, 20);
        bytes[offset] = 1;
        assert_eq!(parse(&bytes), Err(Error::NonzeroEventOrReservedBytes));
    }
}

#[test]
fn gpu_ticks_reject_zero_and_reversal_but_allow_quantized_equality() {
    for (start, end) in [(0, 0), (0, 1), (1, 0)] {
        assert_eq!(parse(&snapshot(start, end)), Err(Error::ZeroGpuTimestamp));
    }
    assert_eq!(parse(&snapshot(11, 10)), Err(Error::ReversedGpuInterval));
    assert_eq!(Ticks::new(u64::MAX, u64::MAX).unwrap().end(), u64::MAX);
    assert_eq!(parse(&snapshot(10, 10)), Ok(Ticks::new(10, 10).unwrap()));
}

#[test]
fn clock_samples_and_brackets_reject_invalid_domains() {
    assert_eq!(Sample::new(0, 1, 1), Err(Error::ZeroGpuTimestamp));
    assert_eq!(Sample::new(1, 0, 1), Err(Error::ZeroSystemTimestamp));
    assert_eq!(Sample::new(1, 1, 0), Err(Error::ZeroSystemFrequency));
    let before = sample(10, 100, 1000);
    assert_eq!(
        Bracket::new(before, sample(20, 200, 1001)),
        Err(Error::FrequencyChanged)
    );
    for gpu in [9, 10] {
        assert_eq!(
            Bracket::new(before, sample(gpu, 200, 1000)),
            Err(Error::NonIncreasingGpuCorrelation)
        );
    }
    for system in [99, 100] {
        assert_eq!(
            Bracket::new(before, sample(20, system, 1000)),
            Err(Error::NonIncreasingSystemCorrelation)
        );
    }
}

#[test]
fn interpolation_is_bracketed_and_uses_the_system_frequency() {
    let bracket = Bracket::new(sample(100, 1000, 1000), sample(200, 1200, 1000)).unwrap();
    let interval = bracket.interpolate(Ticks::new(120, 180).unwrap()).unwrap();
    assert_eq!(interval.start_system_ticks(), 1040);
    assert_eq!(interval.end_system_ticks(), 1160);
    assert_eq!(interval.system_frequency_hz(), 1000);
    assert_eq!(interval.duration_ns_floor(), 120_000_000);
    let whole = bracket.interpolate(Ticks::new(100, 200).unwrap()).unwrap();
    assert_eq!(whole.duration_ns_floor(), 200_000_000);
    for ticks in [(99, 100), (200, 201), (99, 201)] {
        assert_eq!(
            bracket.interpolate(Ticks::new(ticks.0, ticks.1).unwrap()),
            Err(Error::OutsideCorrelation)
        );
    }
}

#[test]
fn rounding_keeps_duration_independent_of_endpoint_quantization() {
    let bracket = Bracket::new(sample(1, 100, 3), sample(4, 102, 3)).unwrap();
    let interval = bracket.interpolate(Ticks::new(2, 3).unwrap()).unwrap();
    assert_eq!(
        (interval.start_system_ticks(), interval.end_system_ticks()),
        (100, 101)
    );
    assert_eq!(interval.duration_ns_floor(), 222_222_222);
    assert_eq!(
        bracket
            .interpolate(Ticks::new(2, 2).unwrap())
            .unwrap()
            .duration_ns_floor(),
        0
    );
}

#[test]
fn checked_arithmetic_rejects_u128_product_and_u64_result_overflow() {
    let product = Bracket::new(sample(1, 1, 1), sample(u64::MAX, u64::MAX, 1)).unwrap();
    assert_eq!(
        product.interpolate(Ticks::new(1, u64::MAX).unwrap()),
        Err(Error::ArithmeticOverflow)
    );
    let result = Bracket::new(sample(1, 1, 1), sample(2, u64::MAX, 1)).unwrap();
    assert_eq!(
        result.interpolate(Ticks::new(1, 2).unwrap()),
        Err(Error::ArithmeticOverflow)
    );
    let edge = Bracket::new(
        sample(u64::MAX - 1, u64::MAX - 1, u64::MAX),
        sample(u64::MAX, u64::MAX, u64::MAX),
    )
    .unwrap();
    let interval = edge
        .interpolate(Ticks::new(u64::MAX - 1, u64::MAX).unwrap())
        .unwrap();
    assert_eq!(interval.end_system_ticks(), u64::MAX);
    assert_eq!(interval.duration_ns_floor(), 0);
}
