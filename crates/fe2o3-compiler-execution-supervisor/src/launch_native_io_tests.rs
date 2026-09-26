// Shared image tests expand against the concrete family error and adapter.
use fe2o3_static_preexec_manifest::StaticPreexecManifestErrorV1 as StaticError;
use rustix::fs::{MemfdFlags, SealFlags};

fn static_fixture() -> StaticManifest {
    let entries: [_; 3] = std::array::from_fn(|index| {
        Descriptor::for_index(
            index,
            index as i32,
            Object::new(u64::MAX, index as u64 + 2, 1, 0o100600),
        )
        .unwrap()
    });
    StaticManifest::from_descriptors(42, 1, Object::new(u64::MAX, 1, 1, 0o100500), &entries)
        .unwrap()
}

fn raw_image(bytes: &[u8], mode: Mode, seals: SealFlags, access: OFlags) -> File {
    let writable = rustix::fs::memfd_create(
        c"fe2o3-native-prepared-test",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map(File::from)
    .unwrap();
    assert_eq!(
        rustix::io::pwrite(&writable, bytes, 0).unwrap(),
        bytes.len()
    );
    rustix::fs::fchmod(&writable, mode).unwrap();
    rustix::fs::fcntl_add_seals(&writable, seals).unwrap();
    if access == OFlags::RDWR {
        return writable;
    }
    let directory = rustix::fs::open(
        c"/proc/self/fd",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    rustix::fs::openat(
        directory,
        rustix::path::DecInt::from_fd(&writable),
        access | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .unwrap()
}

#[test]
fn static_image_creation_binds_canonical_bytes_metadata_and_read_only_seals() {
    let manifest = static_fixture();
    let (file, object) = image::create(&manifest).unwrap();
    assert_eq!(
        references(object),
        1,
        "creation retains no writable or directory fd"
    );
    let stat = rustix::fs::fstat(&file).unwrap();
    assert_eq!(
        rustix::fs::FileType::from_raw_mode(stat.st_mode),
        rustix::fs::FileType::RegularFile
    );
    assert_eq!(stat.st_mode & 0o7777, MANIFEST_MODE_V1);
    assert_eq!(stat.st_uid, rustix::process::geteuid().as_raw());
    assert_eq!(stat.st_gid, rustix::process::getegid().as_raw());
    assert_eq!(stat.st_nlink, 0);
    assert_eq!(stat.st_size, BYTES as i64);
    assert_eq!(rustix::io::fcntl_getfd(&file).unwrap(), FdFlags::CLOEXEC);
    assert_eq!(
        rustix::fs::fcntl_getfl(&file).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert_eq!(
        rustix::fs::fcntl_get_seals(&file).unwrap(),
        REQUIRED_MANIFEST_SEALS_V1
    );
    let mut bytes = [0; BYTES + 1];
    assert_eq!(rustix::io::pread(&file, &mut bytes[..], 0).unwrap(), BYTES);
    assert_eq!(&bytes[..BYTES], &manifest.encode());
    image::validate(&file, &manifest, object).unwrap();
    assert!(rustix::io::pwrite(&file, &[0], 0).is_err());
    assert!(rustix::fs::ftruncate(&file, 0).is_err());
    assert!(rustix::fs::fcntl_add_seals(&file, SealFlags::WRITE).is_err());
    image::validate(&file, &manifest, object).unwrap();
    let witness = Witness::new(&file, 1);
    drop(file);
    witness.assert_released();
}

#[test]
fn static_image_equal_bytes_do_not_substitute_for_the_original_object() {
    let manifest = static_fixture();
    let (original, expected) = image::create(&manifest).unwrap();
    let (substitute, other) = image::create(&manifest).unwrap();
    assert!(!checks::same_object(&expected, &other));
    image::validate(&original, &manifest, expected).unwrap();
    image::validate(&substitute, &manifest, other).unwrap();
    assert!(matches!(
        image::validate(&substitute, &manifest, expected),
        Err(Error::InvalidDescriptor { .. })
    ));
    for changed in [
        Object::new(
            expected.device() ^ 1,
            expected.inode(),
            expected.size(),
            expected.mode(),
        ),
        Object::new(
            expected.device(),
            expected.inode() ^ 1,
            expected.size(),
            expected.mode(),
        ),
        Object::new(
            expected.device(),
            expected.inode(),
            expected.size() + 1,
            expected.mode(),
        ),
        Object::new(
            expected.device(),
            expected.inode(),
            expected.size(),
            expected.mode() ^ 1,
        ),
        Object::new_process_pidfd(
            expected.device(),
            expected.inode(),
            expected.size(),
            expected.mode(),
        ),
    ] {
        assert!(matches!(
            image::validate(&original, &manifest, changed),
            Err(Error::InvalidDescriptor { .. })
        ));
    }
}

#[test]
fn static_image_revalidation_refuses_descriptor_mode_length_and_seal_drift() {
    let manifest = static_fixture();
    let (file, object) = image::create(&manifest).unwrap();
    rustix::io::fcntl_setfd(&file, FdFlags::empty()).unwrap();
    assert!(matches!(
        image::validate(&file, &manifest, object),
        Err(Error::InvalidDescriptor { .. })
    ));
    rustix::io::fcntl_setfd(&file, FdFlags::CLOEXEC).unwrap();
    rustix::fs::fchmod(&file, Mode::RUSR | Mode::WUSR).unwrap();
    assert!(matches!(
        image::validate(&file, &manifest, object),
        Err(Error::InvalidDescriptor { .. })
    ));
    rustix::fs::fchmod(&file, Mode::RUSR).unwrap();
    image::validate(&file, &manifest, object).unwrap();
    let bytes = manifest.encode();
    let extended = [0; BYTES + 1];
    for (bytes, mode, seals, access) in [
        (
            &bytes[..BYTES - 1],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        ),
        (
            &extended[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR | Mode::WUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDWR,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::WRITE,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::GROW,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::SHRINK,
            OFlags::RDONLY,
        ),
        (
            &bytes[..],
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1 - SealFlags::SEAL,
            OFlags::RDONLY,
        ),
    ] {
        let file = raw_image(bytes, mode, seals, access);
        let actual = checks::object_identity(&file, "fixture image").unwrap();
        assert!(matches!(
            image::validate(&file, &manifest, actual),
            Err(Error::InvalidDescriptor { .. })
        ));
    }
}

#[test]
fn static_image_revalidation_checks_decoding_and_exact_expected_bytes() {
    let manifest = static_fixture();
    let canonical = manifest.encode();
    for (offset, expected) in [
        (0, Some(StaticError::InvalidMagic)),
        (
            64 + 3 * 40,
            Some(StaticError::NonzeroInactiveDescriptor { index: 3 }),
        ),
        (24, None),
    ] {
        let mut bytes = canonical;
        bytes[offset] ^= 2;
        let file = raw_image(
            &bytes,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        );
        let actual = checks::object_identity(&file, "fixture image").unwrap();
        match (
            image::validate(&file, &manifest, actual).unwrap_err(),
            expected,
        ) {
            (Error::StaticManifest(actual), Some(expected)) => assert_eq!(actual, expected),
            (Error::DescriptorChanged("static pre-exec manifest bytes"), None) => {}
            (actual, expected) => {
                panic!("unexpected byte refusal: {actual:?}, expected {expected:?}")
            }
        }
    }
}
