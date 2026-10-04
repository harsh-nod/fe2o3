use super::*;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

#[test]
fn observation_never_replays_attempt_recovery_even_during_token_revalidation() {
    let temp = TestDirectory::new();
    let producer = producer("observer_attempt_residue_v3");
    let attempt = begin(&temp.0, &producer, 25);
    publish_with_currentness(&temp.0, &producer, attempt, &outer(125));
    let (lease, token) = try_observe_compiler_module_handoff_currentness_in_slot_v3(
        &temp.0,
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
    )
    .unwrap();
    let canonical = temp.0.join(crate::ATTEMPT_FILE);
    let recovery = temp.0.join(crate::RECOVERY_ATTEMPT_FILE);
    let bytes = fs::read(&canonical).unwrap();
    let inode = fs::metadata(&canonical).unwrap().ino();
    fs::copy(&canonical, &recovery).unwrap();
    assert!(token.revalidate_locked_currentness().is_err());
    assert!(recovery.exists());
    drop(token);
    assert!(lease.acquire_current_token().is_err());
    assert!(
        try_observe_compiler_module_handoff_currentness_in_slot_v3(
            &temp.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV3::Production
        )
        .is_err()
    );
    assert_eq!(fs::read(&canonical).unwrap(), bytes);
    assert_eq!(fs::metadata(&canonical).unwrap().ino(), inode);
    assert_eq!(fs::read(&recovery).unwrap(), bytes);
    recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap();
    assert!(!recovery.exists());
}

#[test]
fn observation_rejects_fifo_control_and_lock_files_without_blocking() {
    for entry in [
        crate::ATTEMPT_FILE,
        crate::RECOVERY_ATTEMPT_FILE,
        crate::LOCK_FILE,
    ] {
        let temp = TestDirectory::new();
        let producer = producer("observer_fifo_v3");
        let attempt = begin(&temp.0, &producer, 26);
        publish_with_currentness(&temp.0, &producer, attempt, &outer(126));
        let path = temp.0.join(entry);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        let c_path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: the path is a terminated CString and the FIFO is in this owned test directory.
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        assert!(
            try_observe_compiler_module_handoff_currentness_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Production
            )
            .is_err(),
            "{entry}"
        );
        assert_eq!(
            fs::symlink_metadata(&path).unwrap().mode() & libc::S_IFMT,
            libc::S_IFIFO
        );
    }
}

#[test]
fn observation_contends_without_waiting_and_retains_one_unbroken_lock() {
    let temp = TestDirectory::new();
    let producer = producer("observer_lock_v3");
    let attempt = begin(&temp.0, &producer, 21);
    let lease =
        publish_with_currentness(&temp.0, &producer, attempt, &outer(121)).into_current_lease();
    let winner = acquire_token(&lease);
    let root = fs::File::open(&temp.0).unwrap();
    let procfd = std::path::PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd()));
    for path in [&temp.0, &procfd] {
        assert!(matches!(
            try_observe_compiler_module_handoff_currentness_in_slot_v3(
                path,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Production
            ),
            Err(CompilerModuleHandoffErrorV3::Busy)
        ));
        winner.revalidate_locked_currentness().unwrap();
    }
    drop(winner);
    let (observed, token) = try_observe_compiler_module_handoff_currentness_in_slot_v3(
        &procfd,
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
    )
    .unwrap();
    assert_eq!(observed.receipt(), lease.receipt());
    observed.validate_current_token(&token).unwrap();
    token.revalidate_locked_currentness().unwrap();
    assert!(matches!(
        lease.acquire_current_token(),
        Err(CompilerModuleHandoffErrorV3::Busy)
    ));
    drop(token);
    lease.acquire_current_token().unwrap();
}

#[test]
fn observation_and_later_lease_acquisition_never_create_a_missing_lock() {
    let temp = TestDirectory::new();
    let producer = producer("observer_missing_lock_v3");
    let attempt = begin(&temp.0, &producer, 22);
    publish_with_currentness(&temp.0, &producer, attempt, &outer(122));
    let (lease, token) = try_observe_compiler_module_handoff_currentness_in_slot_v3(
        &temp.0,
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
    )
    .unwrap();
    drop(token);
    let lock_path = temp.0.join(crate::LOCK_FILE);
    fs::remove_file(&lock_path).unwrap();
    assert!(lease.acquire_current_token().is_err());
    assert!(!lock_path.exists());
    assert!(
        try_observe_compiler_module_handoff_currentness_in_slot_v3(
            &temp.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV3::Production
        )
        .is_err()
    );
    assert!(!lock_path.exists());
    // The original repair API retains its lock-creation behavior.
    recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap();
    assert!(lock_path.is_file());
}

#[test]
fn observation_rejects_slot_residue_without_cleaning_or_repairing_it() {
    let temp = TestDirectory::new();
    let producer = producer("observer_residue_v3");
    let attempt = begin(&temp.0, &producer, 23);
    publish_with_currentness(&temp.0, &producer, attempt, &outer(123));
    let slot = slot_path(
        &temp.0,
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
    );
    let residue = slot.join(".tmp-observer-residue");
    fs::write(&residue, b"unchanged").unwrap();
    fs::set_permissions(&residue, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        try_observe_compiler_module_handoff_currentness_in_slot_v3(
            &temp.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV3::Production
        )
        .is_err()
    );
    assert_eq!(fs::read(&residue).unwrap(), b"unchanged");
    recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap();
    assert!(!residue.exists());
}

#[test]
fn observation_rejects_noncanonical_payload() {
    let temp = TestDirectory::new();
    let producer = producer("observer_payload_v3");
    let attempt = begin(&temp.0, &producer, 24);
    publish_with_currentness(&temp.0, &producer, attempt, &outer(124));
    let slot = slot_path(
        &temp.0,
        &producer,
        attempt,
        CompilerModuleHandoffSlotV3::Production,
    );
    let payload = fs::OpenOptions::new()
        .write(true)
        .open(slot.join(PAYLOAD_ENTRY))
        .unwrap();
    payload.write_all_at(b"X", 0).unwrap();
    assert!(
        try_observe_compiler_module_handoff_currentness_in_slot_v3(
            &temp.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV3::Production
        )
        .is_err()
    );
}
