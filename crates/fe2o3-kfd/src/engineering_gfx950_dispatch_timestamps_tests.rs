use super::*;
use std::io::Cursor;

struct Fake {
    calls: Vec<String>,
    fail: Option<&'static str>,
    identity: [u64; 3],
    ticks: Vec<(u64, u64)>,
    poisoned: bool,
}

impl Fake {
    fn new(count: usize) -> Self {
        Self {
            calls: Vec::new(),
            fail: None,
            identity: [17, 3, 127],
            ticks: (0..count)
                .map(|slot| (10 + slot as u64 * 10, 15 + slot as u64 * 10))
                .collect(),
            poisoned: false,
        }
    }

    fn step(&mut self, name: &'static str) -> Result<()> {
        self.calls.push(name.into());
        if self.fail == Some(name) {
            return Err(format!("injected {name}"));
        }
        Ok(())
    }
}

impl TimestampBackend for Fake {
    fn admit(&mut self, count: usize) -> Result<[u64; 3]> {
        self.step("admit")?;
        assert_eq!(count, self.ticks.len());
        Ok(self.identity)
    }

    fn begin(&mut self, _: usize) -> Result<()> {
        self.step("begin")
    }

    fn execute(&mut self) -> Result<u64> {
        self.step("execute")?;
        Ok(1234)
    }

    fn observe(&mut self, slot: u32) -> Result<(u64, u64)> {
        self.step("observe")?;
        Ok(self.ticks[slot as usize])
    }

    fn finish(&mut self, identity: [u64; 3], count: usize) -> Result<()> {
        assert_eq!(identity, self.identity);
        assert_eq!(count, self.ticks.len());
        self.step("finish")
    }

    fn poison(&mut self) {
        self.calls.push("poison".into());
        self.poisoned = true;
    }
}

#[test]
fn timestamp_profile_retains_packet_kernel_epoch_and_raw_tick_identity() {
    for count in [1, 16, 64] {
        let mut fake = Fake::new(count);
        let kernels = (1..=count as u64).collect::<Vec<_>>();
        let response = run_profiled_batch(&mut fake, &kernels).unwrap();
        let ResponseV1::DispatchOrderedBatch64ProfiledCompleted {
            device_unique_id,
            queue_epoch,
            elapsed_ns,
            timestamps,
        } = response
        else {
            panic!("wrong response");
        };
        assert_eq!((device_unique_id, queue_epoch, elapsed_ns), (17, 3, 1234));
        assert_eq!(timestamps.len(), count);
        for (slot, stamp) in timestamps.iter().enumerate() {
            assert_eq!(stamp.packet_id, 127 + slot as u64);
            assert_eq!(stamp.kernel, kernels[slot]);
            assert_eq!((stamp.start_tick, stamp.end_tick), fake.ticks[slot]);
        }
        assert_eq!(fake.calls[0..3], ["admit", "begin", "execute"]);
        assert_eq!(fake.calls.last().unwrap(), "finish");
        assert!(!fake.poisoned);
    }
}

#[test]
fn timestamp_profile_failures_are_terminal_without_recovery_restoration() {
    for failure in ["admit", "begin", "execute", "observe", "finish"] {
        let mut fake = Fake::new(2);
        fake.fail = Some(failure);
        assert!(run_profiled_batch(&mut fake, &[1, 2]).is_err());
        assert!(fake.poisoned);
        assert_eq!(fake.calls.last().unwrap(), "poison");
        assert_eq!(
            fake.calls.iter().filter(|call| **call == "finish").count(),
            usize::from(failure == "finish")
        );
        let failed = fake.calls.iter().position(|call| call == failure).unwrap();
        assert_eq!(failed + 2, fake.calls.len());
    }
}

#[test]
fn timestamp_profile_rejects_missing_zero_reversed_or_equal_stamps() {
    for ticks in [(0, 0), (0, 5), (8, 0), (8, 7), (8, 8)] {
        let mut fake = Fake::new(2);
        fake.ticks[1] = ticks;
        let error = run_profiled_batch(&mut fake, &[1, 2]).unwrap_err();
        assert!(error.contains("slot=1"));
        assert!(error.contains(&format!("start_tick={}", ticks.0)));
        assert!(error.contains(&format!("end_tick={}", ticks.1)));
        assert!(!fake.calls.iter().any(|call| call == "finish"));
        assert!(fake.poisoned);
    }
}

#[test]
fn timestamp_profile_does_not_infer_nonoverlap_or_clock_conversion() {
    let mut fake = Fake::new(2);
    fake.ticks = vec![(100, 300), (150, 250)];
    let response = run_profiled_batch(&mut fake, &[1, 2]).unwrap();
    let json = serde_json::to_value(response).unwrap();
    assert_eq!(json["timestamps"][1]["start_tick"], 150);
    assert!(json.get("gpu_elapsed_ns").is_none());
    assert!(json["timestamps"][0].get("duration_ns").is_none());
}

#[test]
fn timestamp_profile_rejects_count_and_frontier_overflow_before_mutation() {
    for count in [0, 65] {
        let mut fake = Fake::new(count);
        assert!(run_profiled_batch(&mut fake, &vec![1; count]).is_err());
        assert_eq!(fake.calls, ["poison"]);
    }
    let mut fake = Fake::new(1);
    fake.identity[2] = u64::MAX;
    assert!(run_profiled_batch(&mut fake, &[1]).is_err());
    assert_eq!(fake.calls, ["admit", "poison"]);
}

fn command(count: usize, timeout_ms: u32) -> CommandV1 {
    CommandV1::DispatchOrderedBatch64Profiled {
        dispatches: vec![
            OrderedBatchDispatchV1 {
                kernel: 1,
                payload_bytes: MAX_KERNARG_BYTES_V1,
                workgroup: [64, 1, 1],
                grid: [64, 1, 1],
                pointers: Vec::new(),
            };
            count
        ],
        timeout_ms,
    }
}

#[test]
fn timestamp_wire_keeps_existing_payload_deadline_and_header_limits() {
    for count in 1..=64 {
        let command = command(count, 600_000);
        assert_eq!(
            command.payload_bytes().unwrap(),
            count * MAX_KERNARG_BYTES_V1 as usize
        );
        let mut frame = Vec::new();
        write_header_v1(&mut frame, &command).unwrap();
        frame.push(0xa5);
        let mut cursor = Cursor::new(frame);
        assert_eq!(
            read_header_v1::<CommandV1>(&mut cursor).unwrap(),
            Some(command)
        );
        assert_eq!(cursor.get_ref()[cursor.position() as usize], 0xa5);
    }
    for (count, timeout) in [(0, 1), (65, 1), (1, 0), (1, 600_001)] {
        assert!(command(count, timeout).payload_bytes().is_err());
    }
    assert_eq!(MAX_HEADER_BYTES_V1, 65_536);
    assert_eq!(MAX_ORDERED_BATCH_DISPATCHES_V1, 16);
    assert_eq!(MAX_SEQUENCE_DISPATCHES_V1, 16);
}

#[test]
fn timestamp_wire_worst_case_response_fits_and_round_trips_exactly() {
    let response = ResponseV1::DispatchOrderedBatch64ProfiledCompleted {
        device_unique_id: u64::MAX,
        queue_epoch: u64::MAX,
        elapsed_ns: u64::MAX,
        timestamps: vec![
            DispatchTimestampTicksV1 {
                packet_id: u64::MAX,
                kernel: u64::MAX,
                start_tick: u64::MAX - 1,
                end_tick: u64::MAX,
            };
            64
        ],
    };
    let mut frame = Vec::new();
    write_header_v1(&mut frame, &response).unwrap();
    assert!(frame.len() < 16_384);
    assert_eq!(
        read_header_v1::<ResponseV1>(&mut Cursor::new(frame)).unwrap(),
        Some(response)
    );
}

#[test]
fn timestamp_wire_rejects_unknown_fields_and_oversized_pointer_headers() {
    let mut json = serde_json::to_value(command(1, 1)).unwrap();
    json["timestamps_in_nanoseconds"] = true.into();
    assert!(serde_json::from_value::<CommandV1>(json).is_err());
    let mut command = command(64, 1);
    let CommandV1::DispatchOrderedBatch64Profiled { dispatches, .. } = &mut command else {
        unreachable!()
    };
    let pointer = PointerFixupV1 {
        kernarg_offset: u32::MAX,
        buffer: u64::MAX,
        buffer_offset: u64::MAX,
        extent_bytes: u64::MAX,
        access: BufferAccessV1::ReadWrite,
    };
    for dispatch in dispatches.iter_mut() {
        dispatch.pointers = vec![pointer.clone(); 13];
    }
    assert!(command.payload_bytes().is_ok());
    let mut frame = Vec::new();
    assert!(write_header_v1(&mut frame, &command).is_err());
    assert!(frame.is_empty());
}
