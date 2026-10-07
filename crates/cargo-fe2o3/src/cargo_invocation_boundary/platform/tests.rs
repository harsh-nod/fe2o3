use super::*;

#[test]
fn authorization_is_one_use_and_process_specific() {
    let registry = InvocationAuthorizationRegistryV1::new();
    let first = ProcessIdentityV1 {
        pid: 10,
        start_time_ticks: 20,
    };
    let other = ProcessIdentityV1 {
        pid: 10,
        start_time_ticks: 21,
    };
    registry.authorize_test_process(first).unwrap();
    assert!(registry.consume(other).is_err());
    registry.consume(first).unwrap();
    assert!(registry.consume(first).is_err());
}

#[test]
fn proof_authorization_consumes_the_original_pidfd_once() {
    use std::os::fd::AsRawFd;
    let registry = InvocationAuthorizationRegistryV1::new();
    let current = ProcessIdentityV1::observe(std::process::id()).unwrap();
    let original = open_process_pidfd(current.pid()).unwrap();
    let original_fd = original.as_raw_fd();
    registry
        .authorize_with_process_fd(current, Some(original))
        .unwrap();
    let retained = registry.consume_original(current).unwrap();
    assert_eq!(retained.as_raw_fd(), original_fd);
    assert!(pidfd_is_live(&retained));
    assert!(registry.consume_original(current).is_err());
}

#[test]
fn proof_authorization_rejects_synthetic_permit_without_reopening_pid() {
    let registry = InvocationAuthorizationRegistryV1::new();
    let current = ProcessIdentityV1::observe(std::process::id()).unwrap();
    registry.authorize_test_process(current).unwrap();
    assert!(
        registry
            .consume_original(current)
            .unwrap_err()
            .contains("original exec-permit pidfd")
    );
    assert!(registry.consume(current).is_err());
}

#[test]
fn any_later_exec_notification_revokes_the_old_process_permit() {
    let registry = InvocationAuthorizationRegistryV1::new();
    let process = ProcessIdentityV1 {
        pid: 10,
        start_time_ticks: 20,
    };
    registry.authorize_test_process(process).unwrap();
    registry.revoke_pid(process.pid);
    assert!(registry.consume(process).is_err());
}

#[test]
fn pidfd_pins_the_current_kernel_process_lifetime() {
    let process = open_process_pidfd(std::process::id()).unwrap();
    assert!(pidfd_is_live(&process));
}

#[test]
fn dead_wrapper_permits_are_pruned_before_capacity_accounting() {
    let mut command = Command::new("/bin/sleep");
    command.arg("60");
    let mut child = crate::process_execution::spawn(&mut command).unwrap();
    let child_identity = ProcessIdentityV1::observe(child.id()).unwrap();
    let child_pidfd = open_process_pidfd(child.id()).unwrap();
    let registry = InvocationAuthorizationRegistryV1::new();
    registry
        .authorize_with_process_fd(child_identity, Some(child_pidfd))
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();

    let current = ProcessIdentityV1::observe(std::process::id()).unwrap();
    registry.authorize_test_process(current).unwrap();
    let state = registry
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(state.len(), 1);
    assert!(state.contains_key(&current));
}

#[test]
fn non_leader_thread_is_normalized_to_process_tgid() {
    let (tid_sender, tid_receiver) = std::sync::mpsc::sync_channel(1);
    let (release_sender, release_receiver) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        // SAFETY: gettid has no arguments or memory effects.
        let tid = unsafe { libc::syscall(libc::SYS_gettid) } as u32;
        tid_sender.send(tid).unwrap();
        release_receiver.recv().unwrap();
    });
    let tid = tid_receiver.recv().unwrap();
    assert_ne!(tid, std::process::id());
    assert_eq!(thread_group_id(tid).unwrap(), std::process::id());
    release_sender.send(()).unwrap();
    worker.join().unwrap();
}

#[test]
fn clone_parent_and_non_cargo_images_cannot_qualify_as_cargo_launches() {
    assert!(is_direct_pinned_cargo_child(41, 41, true, true, true));
    assert!(
        !is_direct_pinned_cargo_child(7, 41, true, true, true),
        "CLONE_PARENT cannot substitute a different parent"
    );
    assert!(
        !is_direct_pinned_cargo_child(41, 41, true, false, true),
        "a build-script image with Cargo as parent cannot qualify"
    );
    assert!(!is_direct_pinned_cargo_child(41, 41, false, true, true));
    assert!(
        !is_direct_pinned_cargo_child(41, 41, true, true, false),
        "re-entering the Cargo image cannot restore first-exec eligibility"
    );
}

#[test]
fn wrapper_authority_requires_the_same_admitted_trampoline_process() {
    assert!(is_admitted_trampoline_wrapper_exec(
        41, 41, true, true, true
    ));
    assert!(!is_admitted_trampoline_wrapper_exec(
        7, 41, true, true, true
    ));
    assert!(!is_admitted_trampoline_wrapper_exec(
        41, 41, false, true, true
    ));
    assert!(!is_admitted_trampoline_wrapper_exec(
        41, 41, true, false, true
    ));
    assert!(!is_admitted_trampoline_wrapper_exec(
        41, 41, true, true, false
    ));
}

#[test]
fn protected_cargo_trampoline_and_wrapper_roles_require_distinct_images() {
    let cargo = [1; 32];
    let wrapper = [2; 32];
    let trampoline = [3; 32];
    assert!(protected_image_digests_are_distinct(
        cargo, wrapper, trampoline
    ));
    assert!(!protected_image_digests_are_distinct(
        cargo, cargo, trampoline
    ));
    assert!(!protected_image_digests_are_distinct(cargo, wrapper, cargo));
    assert!(!protected_image_digests_are_distinct(
        cargo, wrapper, wrapper
    ));
}

#[test]
fn proc_self_and_execveat_empty_paths_are_resolved_in_the_tracee() {
    assert_eq!(
        resolve_process_local_proc_path(71, Path::new("/proc/self/exe")).unwrap(),
        PathBuf::from("/proc/71/exe")
    );
    assert_eq!(
        resolve_process_local_proc_path(71, Path::new("/proc/self/fd/9")).unwrap(),
        PathBuf::from("/proc/71/fd/9")
    );
    assert_eq!(
        resolve_execveat_path(71, 9, b"", libc::AT_EMPTY_PATH as u64).unwrap(),
        PathBuf::from("/proc/71/fd/9")
    );

    // SAFETY: all-zero is a valid inert seccomp notification value for this pure test.
    let mut notification: libc::seccomp_notif = unsafe { std::mem::zeroed() };
    notification.data.nr = libc::SYS_execveat as i32;
    notification.data.args[0] = PROTECTED_WRAPPER_CHILD_FD as u64;
    notification.data.args[4] = libc::AT_EMPTY_PATH as u64;
    assert!(is_fixed_empty_execveat(
        notification,
        PROTECTED_WRAPPER_CHILD_FD
    ));
    notification.data.args[4] |= libc::AT_SYMLINK_NOFOLLOW as u64;
    assert!(!is_fixed_empty_execveat(
        notification,
        PROTECTED_WRAPPER_CHILD_FD
    ));
}
