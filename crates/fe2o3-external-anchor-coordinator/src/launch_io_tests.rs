use super::*;
use fe2o3_external_anchor_provisioner::ExternalAnchorProvisioningReadyDispositionV1 as Disposition;
use std::{os::fd::AsRawFd, os::unix::fs::MetadataExt};

#[test]
fn exact_anchor_wire_decoding_retains_one_right_and_closes_it_on_every_mutation() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let original = file.as_file().metadata().unwrap();
    let refs = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .filter(|m| (m.dev(), m.ino()) == (original.dev(), original.ino()))
            .count()
    };
    for disposition in [Disposition::Initialized, Disposition::Existing] {
        let bytes = *Ready::new(disposition).canonical_bytes();
        let fd: OwnedFd = file.as_file().try_clone().unwrap().into();
        let raw = fd.as_raw_fd();
        let (ready, received) = decode(bytes, Some(fd)).unwrap();
        assert_eq!(ready.disposition(), disposition);
        assert_eq!(received.as_raw_fd(), raw);
        drop(received);
        assert_eq!(refs(), 1);
        for offset in 0..READY_BYTES {
            let mut changed = bytes;
            changed[offset] ^= 0x80;
            let fd = file.as_file().try_clone().unwrap().into();
            assert!(matches!(
                decode(changed, Some(fd)),
                Err(Failure::MalformedReadyTransfer)
            ));
            assert_eq!(refs(), 1, "descriptor survived malformed byte {offset}");
        }
        assert!(matches!(
            decode(bytes, None),
            Err(Failure::MalformedReadyTransfer)
        ));
    }
}
