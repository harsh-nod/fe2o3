use fe2o3_aql::*;

fn address(value: u64) -> ObservedGpuAddressV1 {
    ObservedGpuAddressV1::new(value).unwrap()
}

fn barrier() -> AqlPreparedPeerPacketV1 {
    AqlPreparedPeerPacketV1::Barrier(
        AqlPeerBarrierAndPacketV1::new_unpublished(
            [address(0x2040), address(0x2080)],
            address(0x3040),
        )
        .unwrap(),
    )
}

fn kernel(ordering: AqlDispatchOrderingV1) -> AqlPreparedPeerPacketV1 {
    AqlPreparedPeerPacketV1::Kernel(
        AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            0,
            address(0x1000),
            address(0x4000),
            16,
            address(0x2040),
            ordering,
        )
        .unwrap(),
    )
}

fn ordered_kernel() -> AqlPreparedPeerPacketV1 {
    kernel(AqlDispatchOrderingV1::WaitForPrior)
}

#[derive(Default)]
struct Target {
    bodies: Vec<(u32, [u8; 64])>,
    headers: Vec<(u32, u16)>,
    body_failure: Option<u32>,
    header_failure: Option<u32>,
}

impl Target {
    fn body(&mut self, index: u32, body: [u8; 64]) -> Result<(), &'static str> {
        assert!(self.headers.is_empty());
        self.bodies.push((index, body));
        if self.body_failure == Some(index) {
            return Err("body failure");
        }
        Ok(())
    }
}

impl AqlPeerPacketBatchPublicationTargetV1 for Target {
    type Error = &'static str;
    fn write_unpublished_kernel(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<(), Self::Error> {
        self.body(index, packet.encode_unpublished_le())
    }
    fn write_unpublished_barrier(
        &mut self,
        index: u32,
        packet: &AqlPeerBarrierAndPacketV1,
    ) -> Result<(), Self::Error> {
        self.body(index, packet.encode_unpublished_le())
    }
    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<(), Self::Error> {
        self.headers.push((index, header));
        if self.header_failure == Some(index) {
            return Err("header failure");
        }
        Ok(())
    }
}

#[test]
fn peer_barrier_layout_and_exact_wire_bytes() {
    assert_eq!(std::mem::size_of::<AqlPeerBarrierAndPacketV1>(), 64);
    assert_eq!(std::mem::align_of::<AqlPeerBarrierAndPacketV1>(), 8);
    let mut target = Target::default();
    AqlPreparedPeerPacketBatchV1::try_from_packets([barrier()])
        .unwrap()
        .publish_with(&mut target)
        .unwrap();
    let mut expected = [0; 64];
    expected[..4].copy_from_slice(&1_u32.to_le_bytes());
    expected[8..16].copy_from_slice(&0x2040_u64.to_le_bytes());
    expected[16..24].copy_from_slice(&0x2080_u64.to_le_bytes());
    expected[56..64].copy_from_slice(&0x3040_u64.to_le_bytes());
    assert_eq!(target.bodies, vec![(0, expected)]);
    assert_eq!(target.headers, vec![(0, 0x1503)]);
}

#[test]
fn rejects_each_misaligned_signal() {
    for index in 0..2 {
        let mut dependencies = [address(0x2040), address(0x2080)];
        dependencies[index] = address(0x2008);
        assert_eq!(
            AqlPeerBarrierAndPacketV1::new_unpublished(dependencies, address(0x3040)),
            Err(AqlPeerBarrierAndPacketErrorV1::DependencySignal {
                index,
                error: AqlAddressObservationError::Misaligned,
            })
        );
    }
    assert_eq!(
        AqlPeerBarrierAndPacketV1::new_unpublished(
            [address(0x2040), address(0x2080)],
            address(0x3008)
        ),
        Err(AqlPeerBarrierAndPacketErrorV1::CompletionSignal(
            AqlAddressObservationError::Misaligned
        ))
    );
}

#[test]
fn rejects_duplicate_and_self_dependencies() {
    assert_eq!(
        AqlPeerBarrierAndPacketV1::new_unpublished(
            [address(0x2040), address(0x2040)],
            address(0x3040)
        ),
        Err(AqlPeerBarrierAndPacketErrorV1::DuplicateDependencies)
    );
    for completion in [0x2040, 0x2080] {
        assert_eq!(
            AqlPeerBarrierAndPacketV1::new_unpublished(
                [address(0x2040), address(0x2080)],
                address(completion)
            ),
            Err(AqlPeerBarrierAndPacketErrorV1::SelfDependency)
        );
    }
}

#[test]
fn encodes_all_64_signal_address_bits() {
    let a = (1_u64 << 63) + 64;
    let b = (1_u64 << 63) + 128;
    let c = (1_u64 << 63) + 192;
    let packet = AqlPreparedPeerPacketV1::Barrier(
        AqlPeerBarrierAndPacketV1::new_unpublished([address(a), address(b)], address(c)).unwrap(),
    );
    let mut target = Target::default();
    AqlPreparedPeerPacketBatchV1::try_from_packets([packet])
        .unwrap()
        .publish_with(&mut target)
        .unwrap();
    let bytes = target.bodies[0].1;
    assert_eq!(&bytes[8..16], &a.to_le_bytes());
    assert_eq!(&bytes[16..24], &b.to_le_bytes());
    assert_eq!(&bytes[56..64], &c.to_le_bytes());
}

#[test]
fn mixed_batch_writes_all_bodies_before_any_header() {
    let batch = AqlPreparedPeerPacketBatchV1::try_from_packets([
        ordered_kernel(),
        barrier(),
        ordered_kernel(),
    ])
    .unwrap();
    assert_eq!(batch.packet_count(), 3);
    let mut target = Target::default();
    batch.publish_with(&mut target).unwrap();
    assert_eq!(
        target.bodies.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(target.headers, vec![(0, 0x1502), (1, 0x1503), (2, 0x1502)]);
    for (_, bytes) in target.bodies {
        assert_eq!(u16::from_le_bytes(bytes[..2].try_into().unwrap()), 1);
    }
}

#[test]
fn body_failure_never_exposes_a_header() {
    for index in 0..3 {
        let batch = AqlPreparedPeerPacketBatchV1::try_from_packets([
            ordered_kernel(),
            barrier(),
            ordered_kernel(),
        ])
        .unwrap();
        let mut target = Target {
            body_failure: Some(index),
            ..Default::default()
        };
        assert_eq!(batch.publish_with(&mut target), Err("body failure"));
        assert_eq!(target.bodies.len(), index as usize + 1);
        assert!(target.headers.is_empty());
    }
}

#[test]
fn header_failure_stops_without_later_publication() {
    for index in 0..3 {
        let batch = AqlPreparedPeerPacketBatchV1::try_from_packets([
            ordered_kernel(),
            barrier(),
            ordered_kernel(),
        ])
        .unwrap();
        let mut target = Target {
            header_failure: Some(index),
            ..Default::default()
        };
        assert_eq!(batch.publish_with(&mut target), Err("header failure"));
        assert_eq!(target.bodies.len(), 3);
        assert_eq!(target.headers.len(), index as usize + 1);
    }
}

#[test]
fn unordered_kernel_is_rejected_at_any_batch_position() {
    for index in 0..3 {
        let packets = std::array::from_fn::<_, 3, _>(|i| {
            if i == index {
                kernel(AqlDispatchOrderingV1::Independent)
            } else {
                barrier()
            }
        });
        assert_eq!(
            AqlPreparedPeerPacketBatchV1::try_from_packets(packets),
            Err(AqlPeerPacketBatchErrorV1::UnorderedKernel { index })
        );
    }
}

#[test]
fn packet_count_bounds_preserve_the_existing_policy() {
    assert_eq!(
        AqlPreparedPeerPacketBatchV1::<0>::try_from_packets([]),
        Err(AqlPeerPacketBatchErrorV1::Count(
            AqlPreparedKernelDispatchBatchErrorV1::ZeroPacketCount
        ))
    );
    fn packets<const N: usize>() -> Box<[AqlPreparedPeerPacketV1; N]> {
        (0..N)
            .map(|_| barrier())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .unwrap()
    }
    assert_eq!(
        AqlPreparedPeerPacketBatchV1::try_from_boxed_packets(packets::<8192>())
            .unwrap()
            .packet_count(),
        8192
    );
    assert_eq!(
        AqlPreparedPeerPacketBatchV1::try_from_boxed_packets(packets::<8193>()),
        Err(AqlPeerPacketBatchErrorV1::Count(
            AqlPreparedKernelDispatchBatchErrorV1::PacketCountExceedsReviewedMaximum {
                requested: 8193,
                maximum: 8192
            }
        ))
    );
}

#[test]
fn peer_header_gate_does_not_relax_existing_publication() {
    for setup in 0..=4 {
        assert_eq!(
            is_reviewed_aql_peer_publication_v1(0x1502, setup),
            (1..=3).contains(&setup)
        );
        assert_eq!(
            is_reviewed_aql_peer_publication_v1(0x1503, setup),
            setup == 0
        );
        assert!(!is_reviewed_aql_peer_publication_v1(0x1402, setup));
        assert!(!is_reviewed_aql_peer_publication_v1(0x1403, setup));
        assert!(!is_reviewed_aql_publication_v1(0x1503, setup));
    }
    for header in [0, 1, 2, 3, 0x103, 0x503, 0x1103, 0xffff] {
        assert!(!is_reviewed_aql_peer_publication_v1(header, 0));
    }
}
