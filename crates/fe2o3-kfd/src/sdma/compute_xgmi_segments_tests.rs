use super::*;
use crate::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
use fe2o3_runtime_model::MAX_ORDERED_PEER_COPY_SEGMENTS_V1;

fn segment(source_offset: u64, destination_offset: u64, byte_len: u64) -> OrderedPeerCopySegmentV1 {
    OrderedPeerCopySegmentV1 {
        source_offset,
        destination_offset,
        byte_len,
    }
}

#[test]
fn compute_xgmi_segments_plan_keeps_relative_order_duplicates_and_overlap() {
    let descriptors = [segment(20, 40, 30), segment(7, 45, 10), segment(20, 40, 30)];
    let plan =
        Gfx942ComputeXgmiSegmentsPlanV1::new(128, 256, 3, 100, 17, 200, &descriptors).unwrap();
    assert_eq!(plan.source_logical_bytes(), 128);
    assert_eq!(plan.destination_logical_bytes(), 256);
    assert_eq!((plan.source_offset(), plan.source_len()), (3, 100));
    assert_eq!(
        (plan.destination_offset(), plan.destination_len()),
        (17, 200)
    );
    assert_eq!(plan.total_bytes(), 70);
    assert_eq!(plan.packet_count(), 3);
    assert_eq!(
        plan.windows()
            .iter()
            .map(|w| (w.source_offset(), w.destination_offset(), w.bytes()))
            .collect::<Vec<_>>(),
        [(23, 57, 30), (10, 62, 10), (23, 57, 30)]
    );
    assert_eq!(plan.windows()[0], plan.windows()[2]);
}

#[test]
fn compute_xgmi_segments_plan_rejects_entire_invalid_roster_and_logical_padding() {
    use Gfx942ComputeXgmiSegmentsPlanErrorV1 as E;
    use OrderedPeerCopyAdmissionErrorV1 as A;
    let valid = segment(0, 0, 1);
    let plan = |segments: &[OrderedPeerCopySegmentV1]| {
        Gfx942ComputeXgmiSegmentsPlanV1::new(64, 128, 3, 61, 17, 111, segments)
    };
    assert_eq!(plan(&[]), Err(E::Segments(A::Count)));
    for (invalid, error) in [
        (segment(0, 0, 0), A::Length),
        (segment(61, 0, 1), A::SourceRange),
        (segment(0, 111, 1), A::DestinationRange),
        (segment(u64::MAX, 0, 1), A::SourceRange),
        (segment(0, u64::MAX, 1), A::DestinationRange),
    ] {
        assert_eq!(plan(&[valid, invalid]), Err(E::Segments(error)));
    }
    assert_eq!(
        plan(&vec![valid; MAX_ORDERED_PEER_COPY_SEGMENTS_V1 + 1]),
        Err(E::Segments(A::Count))
    );
    assert_eq!(
        Gfx942ComputeXgmiSegmentsPlanV1::new(64, 128, 3, 62, 17, 111, &[valid]),
        Err(E::SourceExtent)
    );
    assert_eq!(
        Gfx942ComputeXgmiSegmentsPlanV1::new(64, 128, 3, 61, 17, 112, &[valid]),
        Err(E::DestinationExtent)
    );
    assert_eq!(
        Gfx942ComputeXgmiSegmentsPlanV1::new(
            u64::MAX,
            u64::MAX,
            0,
            u64::MAX,
            0,
            u64::MAX,
            &[segment(0, 0, u64::MAX), valid],
        ),
        Err(E::Segments(A::TotalOverflow))
    );
    for (source, destination) in [(u64::MAX, 0), (0, u64::MAX)] {
        assert!(
            Gfx942ComputeXgmiSegmentsPlanV1::new(
                u64::MAX,
                u64::MAX,
                source,
                1,
                destination,
                1,
                &[valid],
            )
            .is_err()
        );
    }
}

#[test]
fn compute_xgmi_segments_plan_bounds_each_packet_roster_without_flattening() {
    let maximum = MAX_ORDERED_PEER_COPY_SEGMENTS_V1;
    let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let bytes = cap * maximum as u64;
    let descriptors = vec![segment(0, 0, bytes); maximum];
    let plan = Gfx942ComputeXgmiSegmentsPlanV1::new(
        bytes + 1,
        bytes + 2,
        1,
        bytes,
        2,
        bytes,
        &descriptors,
    )
    .unwrap();
    assert_eq!(plan.windows().len(), maximum);
    assert_eq!(plan.packet_count(), maximum * maximum);
    assert_eq!(plan.total_bytes(), bytes * maximum as u64);
    for window in plan.windows() {
        assert_eq!(window.plan().count(), maximum);
        assert_eq!(window.packet(maximum), None);
        assert_eq!(
            window.packet(maximum - 1).unwrap().bytes,
            GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
        );
    }
    assert_eq!(
        Gfx942ComputeXgmiSegmentsPlanV1::new(
            bytes + 1,
            bytes + 1,
            0,
            bytes + 1,
            0,
            bytes + 1,
            &[segment(0, 0, 1), segment(0, 0, bytes + 1)],
        ),
        Err(Gfx942ComputeXgmiSegmentsPlanErrorV1::PacketExtent)
    );
}
