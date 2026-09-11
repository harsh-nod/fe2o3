use super::*;

#[derive(Default)]
struct Fake {
    events: Vec<String>,
    fail_at: Option<usize>,
    poisoned: bool,
    retained: usize,
    completed: bool,
    count: usize,
}

impl Fake {
    fn event(&mut self, event: String) -> Result<()> {
        if self.poisoned {
            return Err("poisoned".into());
        }
        let index = self.events.len();
        self.events.push(event);
        if self.fail_at == Some(index) {
            return Err("injected failure".into());
        }
        Ok(())
    }

    fn packet(index: usize) -> AqlPreparedKernelDispatchV1 {
        AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            0,
            ObservedGpuAddressV1::new(0x40_0000).unwrap(),
            ObservedGpuAddressV1::new(0x10_0000 + index as u64 * 65_536).unwrap(),
            4096,
            ObservedGpuAddressV1::new(0x30_0000 + index as u64 * 64).unwrap(),
            AqlDispatchOrderingV1::WaitForPrior,
        )
        .unwrap()
    }

    fn expose<const N: usize>(&mut self) -> Result<()> {
        let batch =
            AqlPreparedKernelDispatchBatchV2::try_from_packets(std::array::from_fn::<_, N, _>(
                Self::packet,
            ))
            .unwrap();
        expose_batch(batch, self)
    }
}

impl AqlPacketBatchPublicationTargetV1 for Fake {
    type Error = String;
    fn write_unpublished(&mut self, index: u32, packet: &AqlKernelDispatchPacketV1) -> Result<()> {
        assert!(packet.is_unpublished());
        assert_eq!(
            packet.kernarg_address(),
            0x10_0000 + u64::from(index) * 65_536
        );
        assert_eq!(
            packet.completion_signal(),
            0x30_0000 + u64::from(index) * 64
        );
        self.event(format!("body:{index}"))
    }

    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<()> {
        assert_eq!(header, 0x1502);
        assert_eq!(
            self.events
                .iter()
                .filter(|event| event.starts_with("body:"))
                .count(),
            self.count
        );
        self.event(format!("header:{index}"))
    }
}

impl OrderedExposure for Fake {
    fn advance_write(&mut self, count: u32) -> Result<()> {
        assert_eq!(count as usize, self.count);
        self.event("write_reservation".into())
    }
    fn publication_checkpoint(&mut self) -> Result<()> {
        self.event("publication_currentness_exception_counters".into())
    }
    fn ring_final_doorbell(&mut self) -> Result<()> {
        self.event("doorbell".into())
    }
}

impl OrderedBackend for Fake {
    type Prepared = usize;
    type Staged = usize;
    type Pending = usize;
    fn full_fence(&mut self) -> Result<()> {
        self.event("full_fence".into())
    }
    fn prepare(&mut self, index: usize) -> Result<usize> {
        self.event(format!("prepare:{index}"))?;
        Ok(index)
    }
    fn stage(&mut self, prepared: Vec<usize>) -> Result<usize> {
        self.count = prepared.len();
        assert_eq!(prepared, (0..self.count).collect::<Vec<_>>());
        self.event("retain_storage".into())?;
        self.retained = self.count;
        for index in prepared {
            self.event(format!("reset:{index}"))?;
        }
        Ok(self.count)
    }
    fn publish(&mut self, count: usize, _deadline: Instant) -> Result<usize> {
        self.event("ring_capacity_reservation".into())?;
        match count {
            1 => self.expose::<1>()?,
            16 => self.expose::<16>()?,
            _ => panic!("unsupported fake count"),
        }
        Ok(count)
    }
    fn poll_final(&mut self, pending: &mut usize) -> Result<bool> {
        assert_eq!(*pending, self.count);
        self.event("poll_final_identity_counters_exception".into())?;
        Ok(true)
    }
    fn validate_all(&mut self, pending: &usize) -> Result<()> {
        for index in 0..*pending {
            self.event(format!("validate_signal:{index}"))?;
        }
        Ok(())
    }
    fn complete(&mut self, pending: usize) -> Result<()> {
        assert_eq!(pending, self.count);
        self.event("complete_frontier".into())?;
        self.completed = true;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        self.event("pause".into())
    }
    fn poison(&mut self) {
        self.poisoned = true;
    }
}

#[test]
fn ordered_one_and_sixteen_stage_every_argument_before_one_publication_and_final_wait() {
    for count in [1, 16] {
        let mut fake = Fake::default();
        run_ordered_batch(&mut fake, count, 600_000).unwrap();
        assert!(!fake.poisoned);
        assert!(fake.completed);
        assert_eq!(fake.retained, count);
        let first_reset = fake
            .events
            .iter()
            .position(|event| event.starts_with("reset:"))
            .unwrap();
        let last_prepare = fake
            .events
            .iter()
            .rposition(|event| event.starts_with("prepare:"))
            .unwrap();
        assert!(last_prepare < first_reset);
        assert_eq!(
            fake.events
                .iter()
                .filter(|event| *event == "write_reservation")
                .count(),
            1
        );
        assert_eq!(
            fake.events
                .iter()
                .filter(|event| *event == "doorbell")
                .count(),
            1
        );
        assert_eq!(
            fake.events
                .iter()
                .filter(|event| event.starts_with("poll_final"))
                .count(),
            1
        );
        assert_eq!(
            fake.events
                .iter()
                .filter(|event| event.starts_with("validate_signal"))
                .count(),
            count
        );
        assert_eq!(fake.events.last().unwrap(), "full_fence");
    }
}

#[test]
fn every_preparation_reset_publication_completion_and_exit_failure_is_terminal() {
    for count in [1, 16] {
        let mut success = Fake::default();
        run_ordered_batch(&mut success, count, 600_000).unwrap();
        for fail_at in 0..success.events.len() {
            let mut fake = Fake {
                fail_at: Some(fail_at),
                ..Fake::default()
            };
            assert!(run_ordered_batch(&mut fake, count, 600_000).is_err());
            assert!(fake.poisoned);
            assert_eq!(fake.events.len(), fail_at + 1);
            assert_eq!(fake.events, success.events[..=fail_at]);
            let effects = fake.events.iter().any(|event| event == "write_reservation");
            if effects {
                assert_eq!(fake.retained, count);
            }
            let completed = fake.events.iter().any(|event| event == "complete_frontier");
            if !completed {
                assert!(!fake.completed);
            }
            assert!(run_ordered_batch(&mut fake, count, 600_000).is_err());
            assert_eq!(fake.events.len(), fail_at + 1);
        }
    }
}

#[test]
fn ordered_bounds_deadlines_and_every_retained_signal_fail_closed() {
    for (count, timeout) in [
        (0, 1),
        (17, 1),
        (1, 0),
        (1, 600_001),
        (usize::MAX, u32::MAX),
    ] {
        let mut fake = Fake::default();
        assert!(run_ordered_batch(&mut fake, count, timeout).is_err());
        assert!(fake.poisoned);
        assert!(fake.events.is_empty());
    }
    let now = Instant::now();
    assert!(require_deadline(now, now).is_err());
    assert!(require_deadline(now + Duration::from_nanos(1), now).is_err());
    require_deadline(now, now + Duration::from_nanos(1)).unwrap();
    require_signals_complete([AqlCompletionObservationV1::Completed; 16]).unwrap();
    for index in 0..16 {
        for fault in [
            AqlCompletionObservationV1::Pending,
            AqlCompletionObservationV1::Unexpected(-1),
        ] {
            let mut signals = [AqlCompletionObservationV1::Completed; 16];
            signals[index] = fault;
            assert!(require_signals_complete(signals).is_err());
        }
    }
    assert_eq!(ORDERED_KERNARG_BYTES, 1_048_576);
    assert_eq!(MAX_ORDERED_BATCH_DISPATCHES_V1 * AMD_SIGNAL_BYTES_V1, 1024);
    assert!(batch_count::<0>().is_err());
    assert!(batch_count::<17>().is_err());
}

fn command(count: usize, timeout_ms: u32) -> CommandV1 {
    CommandV1::DispatchOrderedBatch {
        dispatches: vec![
            OrderedBatchDispatchV1 {
                kernel: 1,
                payload_bytes: MAX_KERNARG_BYTES_V1,
                workgroup: [64, 1, 1],
                grid: [64, 1, 1],
                pointers: vec![],
            };
            count
        ],
        timeout_ms,
    }
}

#[test]
fn ordered_wire_has_separate_exact_schema_aggregate_deadline_and_limits() {
    for count in 1..=16 {
        let command = command(count, 600_000);
        assert_eq!(command.payload_bytes().unwrap(), count * 65_536);
        let json = serde_json::to_vec(&command).unwrap();
        assert_eq!(serde_json::from_slice::<CommandV1>(&json).unwrap(), command);
    }
    for (count, timeout) in [(0, 1), (17, 1), (1, 0), (1, 600_001)] {
        assert!(command(count, timeout).payload_bytes().is_err());
    }
    let mut over = command(1, 1);
    if let CommandV1::DispatchOrderedBatch { dispatches, .. } = &mut over {
        dispatches[0].payload_bytes = MAX_KERNARG_BYTES_V1 + 1;
    }
    assert!(over.payload_bytes().is_err());
    let mut json = serde_json::to_value(command(1, 1)).unwrap();
    json["dispatches"][0]["timeout_ms"] = 1.into();
    assert!(serde_json::from_value::<CommandV1>(json).is_err());
    let response = ResponseV1::DispatchOrderedBatchCompleted {
        completed_dispatches: 16,
        elapsed_ns: 123,
    };
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["op"], "dispatch_ordered_batch_completed");
    assert_eq!(json["elapsed_ns"], 123);
    assert!(json.get("kernel_elapsed_ns").is_none());
    assert_eq!(
        serde_json::from_value::<ResponseV1>(json).unwrap(),
        response
    );
}

#[test]
fn ordered_ring_capacity_wrap_and_identity_contract_remain_exact() {
    let mut ring = AqlSingleProducerRingModelV1::new(
        AqlRingCapacityV1::from_ring_bytes(4096).unwrap(),
        63,
        63,
    )
    .unwrap();
    let reservation = ring.reserve_fixed_batch_v2(63, 16).unwrap();
    assert_eq!(reservation.next_write(), 79);
    assert_eq!(reservation.entry(0).unwrap().slot_index(), 63);
    assert_eq!(reservation.entry(1).unwrap().slot_index(), 0);
    assert_eq!(reservation.entry(15).unwrap().slot_index(), 14);
    assert!(reservation.entry(16).is_none());
    assert!(require_sequence_capacity(MAX_UNRETIRED_RING_PACKETS_V1 - 15, 0, 16).is_err());
    assert!(require_sequence_capacity(u64::MAX, u64::MAX, 1).is_err());
    assert!(require_completed_frontier(63, 79).is_err());
    for axis in 0..3 {
        let mut altered = [42, 7, 79];
        altered[axis] += 1;
        assert!(require_pending_dispatch_identity([42, 7, 79], altered, false).is_err());
    }
    for (counters, exception) in [((78, 63), 0), ((79, 62), 0), ((79, 80), 0), ((79, 63), 1)] {
        assert!(
            dispatch_completed(
                79,
                63,
                counters,
                AqlCompletionObservationV1::Completed,
                exception
            )
            .is_err()
        );
    }
    assert!(
        dispatch_completed(79, 63, (79, 63), AqlCompletionObservationV1::Completed, 0).unwrap()
    );
}

#[test]
fn optional_arena_and_signals_are_reinitialized_after_confirmed_queue_rollover() {
    let context = include_str!("engineering_gfx950.rs");
    let rollover = context.split("    fn rollover_queue(").nth(1).unwrap()
        .split("    fn close_inner(").next().unwrap();
    let destroy = rollover.find("self.destroy_queue()?").unwrap();
    let pop = rollover.find("while let Some(allocation) = self.internal.pop()").unwrap();
    let release = rollover.find("self.release_resource(allocation)?").unwrap();
    let initialize = rollover.find("self.initialize_queue()?").unwrap();
    assert!(destroy < pop && pop < release && release < initialize);
    assert!(!rollover.contains("kernels.pop"));
    assert!(!rollover.contains("buffers.pop"));
    let queue = context.split("    fn initialize_queue(").nth(1).unwrap()
        .split("    fn ").next().unwrap();
    assert!(queue.contains("!self.internal.is_empty()"));
    assert!(queue.contains("Backend::initialize_engineering_signal("));
    assert!(!queue.contains("ORDERED_KERNARG"));
    let ordered = include_str!("engineering_gfx950_ordered_batch.rs");
    let retain = ordered.split("    fn retain_storage(").nth(1).unwrap()
        .split("    fn publish_fixed").next().unwrap();
    assert!(retain.find("self.context.check_idle()?").unwrap()
        < retain.find("self.context.allocate_resource(").unwrap());
    assert!(retain.find("self.context.internal.push(allocation)").unwrap()
        < retain.find("Backend::initialize_engineering_signal_slots(").unwrap());
    assert!(retain.contains("ORDERED_KERNARG =>"));
    assert!(retain.contains("count if count == ORDERED_KERNARG + 1 => {}"));
}
