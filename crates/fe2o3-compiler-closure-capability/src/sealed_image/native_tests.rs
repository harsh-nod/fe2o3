use super::*;
use rustix::fs::{Mode, OFlags};
use zeroize::Zeroize;

const N: usize = 32;
const PAYLOAD: [u8; N] = [0x51; N];
const ROLE: CapabilityRole = CapabilityRole {
    name: "native secret image test",
    memfd_name: "fe2o3-native-secret-image-test",
};

// Borrowing lets the test observe the caller's storage after guard destruction.
struct SecretBuffer<'a, const N: usize>(&'a mut [u8; N]);

impl<const N: usize> Drop for SecretBuffer<'_, N> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

fn image() -> SealedCapabilityImage {
    SealedCapabilityImage::create_fixed(&PAYLOAD, ROLE).unwrap()
}

fn assert_rejected<T>(result: Result<T>, expected: &'static str) {
    match result {
        Err(Error::Rejected(reason)) => assert_eq!(reason, expected),
        Err(error) => panic!("unexpected error: {error:?}"),
        Ok(_) => panic!("unexpected acceptance"),
    }
}

fn references(witness: &File) -> usize {
    // The live witness pins this private inode throughout both observations.
    let identity = witness.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| fs::metadata(entry.ok()?.path()).ok())
        .filter(|metadata| metadata.dev() == identity.dev() && metadata.ino() == identity.ino())
        .count()
}

#[test]
fn fixed_read_uses_guarded_caller_storage_without_changing_the_offset() {
    let image = image();
    rustix::fs::seek(&image.image, rustix::fs::SeekFrom::Start(7)).unwrap();
    let mut bytes = [0; N];
    {
        let guard = SecretBuffer(&mut bytes);
        image.read_fixed_into(guard.0).unwrap();
        assert_eq!(*guard.0, PAYLOAD);
    }
    assert_eq!(bytes, [0; N]);
    assert_eq!(
        rustix::fs::seek(&image.image, rustix::fs::SeekFrom::Current(0)).unwrap(),
        7
    );
    assert_eq!(image.read_fixed::<N>().unwrap(), PAYLOAD);
}

#[test]
fn partial_reads_and_errors_never_retry_or_escape_the_caller_guard() {
    let image = image();
    for outcome in [
        Ok(0),
        Ok(N - 1),
        Err(rustix::io::Errno::INTR),
        Err(rustix::io::Errno::IO),
    ] {
        let mut bytes = [0; N];
        let mut attempts = 0;
        {
            let guard = SecretBuffer(&mut bytes);
            let result = image.read_fixed_into_with(guard.0, |bytes| {
                attempts += 1;
                bytes[..5].copy_from_slice(&PAYLOAD[..5]);
                outcome
            });
            assert_eq!(attempts, 1);
            assert_eq!(&guard.0[..5], &PAYLOAD[..5]);
            match outcome {
                Ok(_) => assert_rejected(result, "short sealed image read"),
                Err(expected) => match result {
                    Err(Error::Io { operation, errno }) => {
                        assert_eq!(operation, "read sealed image");
                        assert_eq!(errno, expected.raw_os_error());
                    }
                    other => panic!("unexpected read result: {other:?}"),
                },
            }
        }
        assert_eq!(bytes, [0; N]);
    }
}

#[test]
fn post_read_rejection_leaves_fully_populated_storage_under_the_caller_guard() {
    let image = image();
    let mut bytes = [0; N];
    let mut attempts = 0;
    {
        let guard = SecretBuffer(&mut bytes);
        let result = image.read_fixed_into_with(guard.0, |bytes| {
            attempts += 1;
            let read = rustix::io::pread(&image.image, bytes.as_mut_slice(), 0)?;
            rustix::fs::fchmod(&image.image, Mode::RUSR | Mode::WUSR)?;
            Ok(read)
        });
        assert_rejected(result, " is not an exact regular mode-0400 file");
        assert_eq!(attempts, 1);
        assert_eq!(*guard.0, PAYLOAD);
    }
    assert_eq!(bytes, [0; N]);
    rustix::fs::fchmod(&image.image, Mode::RUSR).unwrap();
}

#[test]
fn common_admission_and_fixed_length_are_checked_before_reading() {
    let image = image();
    let mut bytes = [0xA5; N - 1];
    {
        let guard = SecretBuffer(&mut bytes);
        let result = image.read_fixed_into_with(guard.0, |_| panic!("unexpected read"));
        assert_rejected(result, "sealed image length disagrees with fixed record");
        assert_eq!(*guard.0, [0xA5; N - 1]);
        rustix::fs::fchmod(&image.image, Mode::RUSR | Mode::WUSR).unwrap();
        let result = image.read_fixed_into_with(guard.0, |_| panic!("unexpected read"));
        assert_rejected(result, " is not an exact regular mode-0400 file");
    }
    assert_eq!(bytes, [0; N - 1]);
}

#[test]
fn read_only_reopen_preserves_the_pinned_object_and_does_not_change_alias_access() {
    let image = image();
    assert_rejected(
        image.validate_secret_fixed(),
        "sealed secret image is not an anonymous current-owner read-only image",
    );
    let witness = image.clone_fixed().unwrap();
    assert_eq!(references(&witness), 2);
    let admitted = image.into_read_only_fixed::<N>().unwrap();
    let identity = witness.metadata().unwrap();
    assert_eq!(admitted.device, identity.dev());
    assert_eq!(admitted.inode, identity.ino());
    assert_eq!(references(&witness), 2);
    assert_eq!(
        rustix::fs::fcntl_getfl(&witness).unwrap() & OFlags::ACCMODE,
        OFlags::RDWR
    );
    assert_eq!(
        rustix::fs::fcntl_getfl(&admitted.image).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert!(
        rustix::io::fcntl_getfd(&admitted.image)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    admitted.validate_secret_fixed().unwrap();
    assert_eq!(references(&witness), 2);
    let mut bytes = [0; N];
    {
        let guard = SecretBuffer(&mut bytes);
        admitted.read_fixed_into(guard.0).unwrap();
        assert_eq!(*guard.0, PAYLOAD);
    }
    assert_eq!(bytes, [0; N]);
    drop(admitted);
    assert_eq!(references(&witness), 1);
}

#[test]
fn reopen_rejects_a_different_inode_with_identical_bytes_and_closes_both_inputs() {
    let original = image();
    let original_witness = original.clone_fixed().unwrap();
    let replacement = image();
    let replacement_witness = replacement.clone_fixed().unwrap();
    let mut attempts = 0;
    let result = original.into_read_only_fixed_with::<N>(|retained| {
        attempts += 1;
        let metadata = retained.metadata().unwrap();
        let expected = original_witness.metadata().unwrap();
        assert_eq!(metadata.dev(), expected.dev());
        assert_eq!(metadata.ino(), expected.ino());
        assert_eq!(references(&original_witness), 2);
        reopen_read_only_fixed(&replacement.image)
    });
    assert_rejected(result, "sealed image identity or length changed");
    assert_eq!(attempts, 1);
    assert_eq!(references(&original_witness), 1);
    assert_eq!(references(&replacement_witness), 2);
}

#[test]
fn reopen_checks_fixed_length_and_readmitted_metadata_with_consuming_cleanup() {
    let original = image();
    let witness = original.clone_fixed().unwrap();
    assert_rejected(
        original.into_read_only_fixed_with::<{ N - 1 }>(|_| panic!("unexpected reopen")),
        "sealed image length disagrees with fixed record",
    );
    assert_eq!(references(&witness), 1);

    let original = image();
    let witness = original.clone_fixed().unwrap();
    let result = original.into_read_only_fixed_with::<N>(|retained| {
        let reopened = reopen_read_only_fixed(retained)?;
        rustix::fs::fchmod(retained, Mode::RUSR | Mode::WUSR)
            .map_err(|e| Error::io("test mode change", e))?;
        Ok(reopened)
    });
    assert_rejected(result, " is not an exact regular mode-0400 file");
    assert_eq!(references(&witness), 1);
}

#[test]
fn replaced_retained_file_is_rejected_before_read_reopen_or_secret_checks() {
    let mut original = image();
    let original_witness = original.clone_fixed().unwrap();
    let replacement = image();
    original.image = replacement.clone_fixed().unwrap();
    let mut bytes = [0; N];
    {
        let guard = SecretBuffer(&mut bytes);
        assert_rejected(
            original.read_fixed_into_with(guard.0, |_| panic!("unexpected read")),
            "sealed image identity or length changed",
        );
    }
    assert_eq!(bytes, [0; N]);
    assert_rejected(
        original.validate_secret_fixed(),
        "sealed image identity or length changed",
    );
    assert_rejected(
        original.into_read_only_fixed_with::<N>(|_| panic!("unexpected reopen")),
        "sealed image identity or length changed",
    );
    assert_eq!(references(&original_witness), 1);
    assert_eq!(references(&replacement.image), 1);
}

#[test]
fn descriptor_path_handles_zero_maximum_and_negative_descriptors_without_formatting() {
    for (fd, expected) in [
        (0, c"/proc/self/fd/0"),
        (3, c"/proc/self/fd/3"),
        (RawFd::MAX, c"/proc/self/fd/2147483647"),
    ] {
        let mut path = [0xA5; PROC_FD_PATH_BYTES];
        assert_eq!(proc_fd_path(fd, &mut path).unwrap(), expected);
    }
    let mut path = [0; PROC_FD_PATH_BYTES];
    assert_rejected(
        proc_fd_path(-1, &mut path),
        "invalid sealed image descriptor",
    );
}

#[test]
fn secret_validation_checks_current_uid_and_gid_when_chown_is_available() {
    if !rustix::process::geteuid().is_root() {
        return;
    }
    let image = image().into_read_only_fixed::<N>().unwrap();
    let uid = rustix::process::geteuid();
    let gid = rustix::process::getegid();
    for (changed_uid, changed_gid) in [
        (rustix::process::Uid::from_raw(uid.as_raw() ^ 1), gid),
        (uid, rustix::process::Gid::from_raw(gid.as_raw() ^ 1)),
    ] {
        rustix::fs::fchown(&image.image, Some(changed_uid), Some(changed_gid)).unwrap();
        assert_rejected(
            image.validate_secret_fixed(),
            "sealed secret image is not an anonymous current-owner read-only image",
        );
        rustix::fs::fchown(&image.image, Some(uid), Some(gid)).unwrap();
        image.validate_secret_fixed().unwrap();
    }
}
