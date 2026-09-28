//! Fixed policy slots are exercised only in exact-filter subprocesses.
use crate::{
    COMPILER_EXECUTION_POLICY_CHILD_FD_V1 as FD, CompilerExecutionCapabilityErrorV2 as Error,
};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1 as Measurement;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    fs::File,
    os::fd::AsRawFd,
    os::unix::process::CommandExt,
    process::{Child, Command},
    time::{Duration, Instant},
};

const ISOLATED: &str = "FE2O3_POLICY_INHERITANCE_TEST";
const RECEIVER: &str = "FE2O3_POLICY_INHERITANCE_RECEIVER";

fn command(name: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            name.split_once("::").unwrap().1,
            "--test-threads=1",
            "--nocapture",
        ])
        .env(ISOLATED, name);
    command
}

fn wait(mut child: Child) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated policy inheritance: {status}");
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("isolated policy inheritance timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn isolated(name: &str, run: impl FnOnce()) {
    if std::env::var(ISOLATED).as_deref() == Ok(name) {
        if std::env::var_os(RECEIVER).is_none() {
            // SAFETY: this dedicated subprocess exclusively owns the reserved slot.
            unsafe {
                libc::close(FD);
            }
        }
        run();
    } else {
        wait(command(name).spawn().unwrap());
    }
}

fn flags() -> i32 {
    // SAFETY: scalar inspection does not acquire descriptor ownership.
    unsafe { libc::fcntl(FD, libc::F_GETFD) }
}

fn closed() {
    assert_eq!(flags(), -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

macro_rules! cases {
    ($module:ident, $cap:ident, $policy:ident) => {
        mod $module {
            use super::*;
            use crate::$cap as Cap;
            use fe2o3_compiler_execution_protocol::$policy as Policy;

            fn policy(b: &mut Budget<'_>) -> Policy {
                let (policy, storage) = Policy::new(
                    7,
                    Measurement::new([11; 32], 123).unwrap(),
                    Measurement::new([12; 32], 456).unwrap(),
                    SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes(),
                    SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
                    b,
                )
                .unwrap();
                b.reserve_storage(storage.additional_storage()).unwrap();
                policy
            }

            fn cap(b: &mut Budget<'_>) -> Cap {
                let p = policy(b);
                let (cap, storage) = Cap::create(p, b).unwrap();
                b.reserve_storage(storage.additional_storage()).unwrap();
                cap
            }

            #[test]
            fn exact_policy_survives_exec_and_command_owns_parent_alias() {
                let name = concat!(
                    module_path!(),
                    "::exact_policy_survives_exec_and_command_owns_parent_alias"
                );
                isolated(name, || {
                    let mut work = Work::new(usize::MAX);
                    let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
                    if std::env::var_os(RECEIVER).is_some() {
                        assert_eq!(flags(), 0);
                        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
                        let (received, growth) = Cap::from_inherited_at(FD, &mut b).unwrap();
                        b.reserve_storage(growth.additional_storage()).unwrap();
                        assert_eq!(received.policy(), &policy(&mut b));
                        assert_eq!(flags(), 0);
                        // SAFETY: the child exclusively owns the inherited source.
                        assert_eq!(unsafe { libc::close(FD) }, 0);
                        received.revalidate(&mut b).unwrap();
                        return;
                    }
                    let cap = cap(&mut b);
                    let retained = cap.retained_storage();
                    // Command and harness work are test infrastructure, not claims
                    // about the native operation's separate capability quota.
                    let mut command = command(name);
                    command.env(RECEIVER, "1");
                    b.reserve_storage(Cap::FILE_STORAGE).unwrap();
                    let floor = b.storage();
                    let before = b.work();
                    let ledger = b.work_ledger_identity_v1();
                    cap.inherit_for_child(&mut command, &mut b).unwrap();
                    assert_eq!(b.storage(), floor);
                    assert_eq!(b.work(), before + Cap::IO_WORK);
                    assert!(b.work_ledger_identity_v1() == ledger);
                    assert_eq!(flags(), libc::FD_CLOEXEC);
                    assert_eq!(
                        std::fs::read(format!("/proc/self/fd/{FD}")).unwrap(),
                        cap.policy().canonical_bytes()
                    );
                    drop(cap);
                    b.release_storage(retained).unwrap();
                    wait(command.spawn().unwrap());
                    assert_eq!(flags(), libc::FD_CLOEXEC);
                    drop(command);
                    closed();
                    b.release_storage(Cap::FILE_STORAGE).unwrap();
                    assert_eq!(b.storage(), 0);
                });
            }

            #[test]
            fn resource_refusals_and_occupied_slot_preserve_command_and_owner() {
                isolated(
                    concat!(
                        module_path!(),
                        "::resource_refusals_and_occupied_slot_preserve_command_and_owner"
                    ),
                    || {
                        let mut setup_work = Work::new(usize::MAX);
                        let mut setup = Budget::new(&mut setup_work, 4 * 1024 * 1024);
                        let cap = cap(&mut setup);
                        let floor = cap.retained_storage() + Cap::FILE_STORAGE;
                        for case in 0..4 {
                            let prepaid = floor - usize::from(case == 0);
                            let mut work = Work::new(Cap::IO_WORK - usize::from(case == 1));
                            let mut b = Budget::new(
                                &mut work,
                                floor + Cap::IO_STORAGE - usize::from(case == 2),
                            );
                            b.reserve_storage(prepaid).unwrap();
                            let mut command = Command::new("/bin/true");
                            let result = cap.inherit_for_child(&mut command, &mut b);
                            match case {
                                0 => assert!(matches!(
                                    result,
                                    Err(Error::Resource(Resource::Accounting))
                                )),
                                1 => {
                                    assert!(matches!(
                                        result,
                                        Err(Error::Resource(Resource::Work(_)))
                                    ));
                                    assert_eq!(b.failed_work(), Some(Cap::IO_WORK));
                                }
                                2 => {
                                    assert!(matches!(
                                        result,
                                        Err(Error::Resource(Resource::Storage(_)))
                                    ));
                                    assert_eq!(b.failed_storage(), Some(floor + Cap::IO_STORAGE));
                                }
                                _ => {
                                    result.unwrap();
                                    assert_eq!(b.failed_work(), None);
                                    assert_eq!(b.failed_storage(), None);
                                    assert_eq!(b.peak_storage(), floor + Cap::IO_STORAGE);
                                    assert_eq!(flags(), libc::FD_CLOEXEC);
                                }
                            }
                            assert_eq!(b.storage(), prepaid);
                            assert_eq!(b.work(), if case < 2 { 8 } else { Cap::IO_WORK });
                            if case != 3 {
                                closed();
                            }
                            wait(command.spawn().unwrap());
                            drop(command);
                            closed();
                        }
                        let file = File::open("/dev/null").unwrap();
                        let occupied = rustix::io::fcntl_dupfd_cloexec(&file, FD).unwrap();
                        assert_eq!(occupied.as_raw_fd(), FD);
                        setup.reserve_storage(Cap::FILE_STORAGE).unwrap();
                        let mut command = Command::new("/bin/true");
                        assert!(matches!(
                            cap.inherit_for_child(&mut command, &mut setup),
                            Err(Error::Rejected(
                                "reserved child descriptor is already in use"
                            ))
                        ));
                        assert_eq!(flags(), libc::FD_CLOEXEC);
                        assert_eq!(
                            rustix::fs::fstat(&occupied).unwrap().st_ino,
                            rustix::fs::fstat(&file).unwrap().st_ino
                        );
                        wait(command.spawn().unwrap());
                        drop(occupied);
                        closed();
                        cap.revalidate(&mut setup).unwrap();
                    },
                );
            }

            #[test]
            fn failed_spawn_unwind_and_hostile_child_hook_keep_single_closer() {
                isolated(
                    concat!(
                        module_path!(),
                        "::failed_spawn_unwind_and_hostile_child_hook_keep_single_closer"
                    ),
                    || {
                        let mut work = Work::new(usize::MAX);
                        let mut b = Budget::new(&mut work, 4 * 1024 * 1024);
                        let cap = cap(&mut b);
                        b.reserve_storage(Cap::FILE_STORAGE).unwrap();
                        let floor = b.storage();
                        let mut command = Command::new("/proc/self/fe2o3-no-such-executable");
                        cap.inherit_for_child(&mut command, &mut b).unwrap();
                        assert!(command.spawn().is_err());
                        assert_eq!(flags(), libc::FD_CLOEXEC);
                        drop(command);
                        closed();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let mut command = Command::new("/bin/true");
                            cap.inherit_for_child(&mut command, &mut b).unwrap();
                            panic!("parent callback failed before spawning");
                        }));
                        assert!(result.is_err());
                        closed();
                        let mut command = Command::new("/bin/true");
                        // SAFETY: this test mutates only the isolated child's slot,
                        // before the capability hook runs; fcntl is async-signal-safe.
                        unsafe {
                            command.pre_exec(|| {
                                if libc::fcntl(FD, libc::F_SETFD, 0) < 0 {
                                    return Err(std::io::Error::last_os_error());
                                }
                                Ok(())
                            });
                        }
                        cap.inherit_for_child(&mut command, &mut b).unwrap();
                        assert_eq!(
                            command.spawn().unwrap_err().raw_os_error(),
                            Some(libc::EPERM)
                        );
                        assert_eq!(flags(), libc::FD_CLOEXEC);
                        drop(command);
                        closed();
                        let substituted =
                            crate::native_capability::tests::sealed(cap.policy().canonical_bytes());
                        let mut command = Command::new("/bin/true");
                        // SAFETY: only the isolated child's descriptor table changes.
                        // The separately sealed same-byte image must not replace the
                        // original inode. Parent ownership is unchanged after fork.
                        unsafe {
                            command.pre_exec(move || {
                                if libc::dup2(substituted.as_raw_fd(), FD) < 0
                                    || libc::fcntl(FD, libc::F_SETFD, libc::FD_CLOEXEC) < 0
                                {
                                    return Err(std::io::Error::last_os_error());
                                }
                                Ok(())
                            });
                        }
                        cap.inherit_for_child(&mut command, &mut b).unwrap();
                        assert_eq!(
                            command.spawn().unwrap_err().raw_os_error(),
                            Some(libc::ESTALE)
                        );
                        assert_eq!(flags(), libc::FD_CLOEXEC);
                        drop(command);
                        closed();
                        assert_eq!(b.storage(), floor);
                        cap.revalidate(&mut b).unwrap();
                    },
                );
            }
        }
    };
}

cases!(
    v2,
    CompilerExecutionPolicyCapabilityV2,
    CompilerExecutionIssuerPolicyV2
);
cases!(
    v3,
    CompilerExecutionPolicyCapabilityV3,
    CompilerExecutionIssuerPolicyV3
);
