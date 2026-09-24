//! The borrowed inspection returns the same packet that publish_with consumes.
use super::*;
struct Recorder {
    bytes: Option<[u8; 64]>,
    header: Option<u16>,
}
impl AqlPacketPublicationTargetV1 for Recorder {
    type Error = ();
    fn write_unpublished(&mut self, p: &AqlKernelDispatchPacketV1) -> Result<(), ()> {
        assert!(self.bytes.is_none());
        self.bytes = Some(p.encode_unpublished_le());
        Ok(())
    }
    fn publish_release_header(&mut self, h: u16) -> Result<(), ()> {
        assert!(self.bytes.is_some());
        assert!(self.header.is_none());
        self.header = Some(h);
        Ok(())
    }
}
#[test]
fn borrowed_packet_is_exact_immutable_unpublished_packet_later_consumed() {
    let p = AqlKernelDispatchPacketV1::new_unpublished(
        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        0,
        0,
        ObservedGpuAddressV1::new(0x1000).unwrap(),
        ObservedGpuAddressV1::new(0x2000).unwrap(),
        8,
        ObservedGpuAddressV1::new(0x3000).unwrap(),
    )
    .unwrap();
    assert!(p.unpublished_packet().is_unpublished());
    assert_eq!(p.unpublished_packet().completion_signal(), 0x3000);
    let before = p.unpublished_packet().encode_unpublished_le();
    let mut r = Recorder {
        bytes: None,
        header: None,
    };
    p.publish_with(&mut r).unwrap();
    assert_eq!(r.bytes, Some(before));
    assert_eq!(r.header, Some(0x1402));
}
