use fe2o3_aql::{
    AqlDispatchGeometryV1, AqlDispatchOrderingV1, AqlKernelDispatchPacketV1,
    AqlPacketBatchPublicationTargetV1, AqlPreparedKernelDispatchProgramV1,
    AqlPreparedKernelDispatchV1, ObservedGpuAddressV1,
};

fn packet(index: usize) -> AqlPreparedKernelDispatchV1 {
    AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        0,
        ObservedGpuAddressV1::new(0x4000).unwrap(),
        ObservedGpuAddressV1::new(0x8000 + index as u64 * 64).unwrap(),
        8,
        ObservedGpuAddressV1::new(0x100000 + index as u64 * 64).unwrap(),
        AqlDispatchOrderingV1::WaitForPrior,
    )
    .unwrap()
}

fn program(
    count: usize,
) -> Result<AqlPreparedKernelDispatchProgramV1, fe2o3_aql::AqlPreparedKernelDispatchBatchErrorV1> {
    AqlPreparedKernelDispatchProgramV1::try_from_packets(
        (0..count)
            .map(packet)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

#[derive(Default)]
struct Target {
    events: Vec<(bool, u32)>,
    fail_at: Option<usize>,
}

impl Target {
    fn event(&mut self, header: bool, index: u32) -> Result<(), ()> {
        let fail = self.fail_at == Some(self.events.len());
        self.events.push((header, index));
        if fail { Err(()) } else { Ok(()) }
    }
}

impl AqlPacketBatchPublicationTargetV1 for Target {
    type Error = ();
    fn write_unpublished(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), ()> {
        assert!(packet.is_unpublished());
        self.event(false, index)
    }
    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<(), ()> {
        assert_eq!(header, AqlDispatchOrderingV1::WaitForPrior.header());
        self.event(true, index)
    }
}

#[test]
fn runtime_cardinality_preserves_v2_limits_and_two_phase_order() {
    for count in [1, 63, 64, 65, 1024, 8192] {
        let batch = program(count).unwrap();
        assert_eq!(batch.packet_count(), count as u32);
        let mut target = Target::default();
        batch.publish_with(&mut target).unwrap();
        assert_eq!(target.events.len(), 2 * count);
        for index in 0..count {
            assert_eq!(target.events[index], (false, index as u32));
            assert_eq!(target.events[count + index], (true, index as u32));
        }
    }
    assert!(program(0).is_err());
    assert!(program(8193).is_err());
}

#[test]
fn runtime_batch_stops_at_every_body_or_header_failure() {
    for fail_at in 0..130 {
        let mut target = Target {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        assert!(program(65).unwrap().publish_with(&mut target).is_err());
        assert_eq!(target.events.len(), fail_at + 1);
        if fail_at < 65 {
            assert!(target.events.iter().all(|(header, _)| !header));
        } else {
            assert_eq!(
                target.events.iter().filter(|(header, _)| !header).count(),
                65
            );
        }
    }
}
