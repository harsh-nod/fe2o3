use super::*;
use alloc::{vec, vec::Vec};

fn packet(index: usize, ordering: AqlDispatchOrderingV1) -> AqlPreparedKernelDispatchV1 {
    AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        0,
        ObservedGpuAddressV1::new(0x400000).unwrap(),
        ObservedGpuAddressV1::new(0x100000 + index as u64 * 64).unwrap(),
        64,
        ObservedGpuAddressV1::new(0x800000 + index as u64 * 64).unwrap(),
        ordering,
    )
    .unwrap()
}

fn program(count: usize) -> AqlPreparedClosedKernelDispatchProgramV1 {
    AqlPreparedClosedKernelDispatchProgramV1::try_from_packets(
        (0..count)
            .map(|index| packet(index, AqlDispatchOrderingV1::WaitForPrior))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
    .unwrap()
}

#[test]
fn exact_closed_headers_bind_boundaries_and_never_expand_ordinary_admission() {
    for count in [1, 2, 3, 64, 65, 652, 8192] {
        let program = program(count);
        for index in 0..count as u32 {
            let header = program.header_for_packet(index).unwrap();
            let expected = match (index == 0, index + 1 == count as u32) {
                (true, true) => 0x1502,
                (true, false) => 0x0d02,
                (false, true) => 0x1302,
                (false, false) => 0x0b02,
            };
            assert_eq!(header.header(), expected);
            assert!(header.matches_position(index, count as u32));
            assert!(!header.matches_position(index + 1, count as u32));
            assert!(!header.matches_position(index, count as u32 + 1));
            for setup in 0..=4 {
                assert_eq!(
                    header.admits(index, count as u32, setup),
                    (1..=3).contains(&setup)
                );
            }
            if count != 1 {
                assert_eq!(AqlDispatchOrderingV1::from_header(expected), None);
                assert!(!is_reviewed_aql_publication_v1(expected, 1));
            }
        }
        assert_eq!(program.header_for_packet(count as u32), None);
        assert_eq!(program.header_for_packet(u32::MAX), None);
    }
}

#[test]
fn closed_program_rejects_count_independent_and_reused_signal() {
    for count in [0, 8193] {
        let packets = (0..count)
            .map(|index| packet(index, AqlDispatchOrderingV1::WaitForPrior))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert!(matches!(
            AqlPreparedClosedKernelDispatchProgramV1::try_from_packets(packets),
            Err(AqlPreparedClosedProgramErrorV1::PacketCount(_))
        ));
    }
    for bad in [0, 1, 651] {
        let packets = (0..652)
            .map(|index| {
                packet(
                    index,
                    if index == bad {
                        AqlDispatchOrderingV1::Independent
                    } else {
                        AqlDispatchOrderingV1::WaitForPrior
                    },
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert_eq!(
            AqlPreparedClosedKernelDispatchProgramV1::try_from_packets(packets),
            Err(AqlPreparedClosedProgramErrorV1::IndependentDispatch { index: bad })
        );
    }
    let packets = vec![
        packet(0, AqlDispatchOrderingV1::WaitForPrior),
        packet(0, AqlDispatchOrderingV1::WaitForPrior),
    ]
    .into_boxed_slice();
    assert_eq!(
        AqlPreparedClosedKernelDispatchProgramV1::try_from_packets(packets),
        Err(AqlPreparedClosedProgramErrorV1::ReusedCompletionSignal { index: 1 })
    );
}

#[derive(Default)]
struct Target {
    events: Vec<(bool, u32)>,
    fail_at: Option<usize>,
}

impl Target {
    fn event(&mut self, header: bool, index: u32) -> Result<(), ()> {
        self.events.push((header, index));
        if self.fail_at == Some(self.events.len() - 1) {
            Err(())
        } else {
            Ok(())
        }
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
    fn publish_release_header(&mut self, _: u32, _: u16) -> Result<(), ()> {
        panic!("closed program cannot use ordinary raw-header publication")
    }
}

impl AqlClosedProgramPublicationTargetV1 for Target {
    fn publish_closed_release_header(
        &mut self,
        index: u32,
        header: AqlClosedProgramHeaderV1,
    ) -> Result<(), ()> {
        assert!(header.matches_position(index, 4));
        assert_eq!(self.events.iter().filter(|(header, _)| !header).count(), 4);
        self.event(true, index)
    }
}

#[test]
fn closed_program_publication_failures_never_skip_or_interleave_bodies_and_headers() {
    let mut success = Target::default();
    program(4).publish_with(&mut success).unwrap();
    assert_eq!(
        success.events,
        vec![
            (false, 0),
            (false, 1),
            (false, 2),
            (false, 3),
            (true, 0),
            (true, 1),
            (true, 2),
            (true, 3)
        ]
    );
    for fail_at in 0..8 {
        let mut target = Target {
            fail_at: Some(fail_at),
            ..Default::default()
        };
        assert!(program(4).publish_with(&mut target).is_err());
        assert_eq!(target.events, success.events[..=fail_at]);
    }
}
