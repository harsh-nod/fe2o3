use super::*;
use fe2o3_artifact_transaction::EmitError;
use rustix::fs::{Mode, OFlags};
use std::os::fd::AsRawFd;

#[test]
fn proc_self_fd_directory_admission_rejects_stale_and_non_directory_descriptors() {
    let temp = TestDirectory::new();
    let stale_path = PathBuf::from(format!("/proc/self/fd/{}", i32::MAX));
    assert!(matches!(
        publish(
            &stale_path,
            plan(1, 0x91, 0xf1, b"stale descriptor payload"),
            b"stale descriptor payload",
        ),
        Err(DurableLinkPublicationError::Filesystem(EmitError::Io(error)))
            if error.raw_os_error() == Some(libc::EBADF)
    ));

    let regular_file = temp.path.join("regular-file");
    fs::write(&regular_file, b"not a directory").unwrap();
    let non_directory = rustix::fs::open(
        &regular_file,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    let non_directory_path = PathBuf::from(format!("/proc/self/fd/{}", non_directory.as_raw_fd()));
    assert!(matches!(
        publish(
            &non_directory_path,
            plan(1, 0x92, 0xf2, b"non-directory descriptor payload"),
            b"non-directory descriptor payload",
        ),
        Err(DurableLinkPublicationError::Filesystem(
            EmitError::InvalidArtifactDestination { reason, .. }
        )) if reason == "procfs descriptor does not reference a directory"
    ));
}

#[test]
fn proc_self_fd_currentness_rejects_descriptor_substitution() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let replacement = temp.path.join("replacement");
    fs::create_dir(&replacement).unwrap();
    let mut retained = rustix::fs::open(
        &output,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));
    let bytes = b"descriptor substitution payload";
    let lease = publish(&descriptor_path, plan(1, 0x93, 0xf3, bytes), bytes)
        .unwrap()
        .into_current_lease();
    let substitute = rustix::fs::open(
        &replacement,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();

    rustix::io::dup2(&substitute, &mut retained).unwrap();
    assert!(matches!(
        lease.acquire_current_token(),
        Err(DurableLinkPublicationError::Filesystem(
            EmitError::OutputDirectoryChanged { .. }
        ))
    ));
}
