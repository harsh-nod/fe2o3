use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as MANIFEST_WORK,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy,
    CompilerExecutionServiceLaunchManifestV1 as LegacyManifest,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::File,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::{fs::MetadataExt, process::CommandExt},
    },
    process::{Command, Stdio},
    time::{Duration, Instant},
};

type Inputs = CompilerExecutionIssuerLaunchInputsV2;
type InputError = CompilerExecutionIssuerLaunchInputErrorV2;
const TOTAL_WORK: usize = ENTRY_WORK
    + 2 * PolicyCapability::IO_WORK
    + 2 * LaunchCapability::IO_WORK
    + POLICY_WORK
    + 2 * MANIFEST_WORK;

fn policy(generation: u64) -> Policy {
    let mut w = Work::new(POLICY_WORK);
    let mut b = Budget::new(&mut w, 100_000);
    Policy::new(
        generation,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap()
    .0
}
fn manifest(p: &Policy) -> Manifest {
    let mut w = Work::new(MANIFEST_WORK);
    let mut b = Budget::new(&mut w, 100_000);
    b.reserve_storage(p.retained_storage()).unwrap();
    Manifest::new(
        Client::new(1234, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        p,
        &mut b,
    )
    .unwrap()
    .0
}
fn sealed(bytes: &[u8]) -> File {
    let f = File::from(
        rustix::fs::memfd_create(
            "issuer-native-launch-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    assert_eq!(rustix::io::pwrite(&f, bytes, 0).unwrap(), bytes.len());
    rustix::fs::fchmod(&f, rustix::fs::Mode::RUSR).unwrap();
    rustix::fs::fcntl_add_seals(
        &f,
        rustix::fs::SealFlags::SEAL
            | rustix::fs::SealFlags::SHRINK
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::WRITE,
    )
    .unwrap();
    f
}
fn sources(mode: &str) -> (File, File) {
    let p = policy(if mode == "consistent-other" { 8 } else { 7 });
    let m = manifest(&p);
    let old = LegacyPolicy::new(
        7,
        p.executable(),
        p.runtime(),
        *p.verifying_key(),
        *p.external_anchor_verifying_key(),
    )
    .unwrap();
    let old_manifest = LegacyManifest::new(m.client(), m.external_anchor_service(), &old);
    let p = if mode == "legacy-policy" {
        sealed(old.canonical_bytes())
    } else {
        sealed(p.canonical_bytes())
    };
    let m = match mode {
        "wrong-policy" => sealed(manifest(&policy(8)).canonical_bytes()),
        "legacy-manifest" => sealed(old_manifest.canonical_bytes()),
        "corrupt" => {
            let mut bytes = *m.canonical_bytes();
            bytes[80] ^= 1;
            sealed(&bytes)
        }
        _ => sealed(m.canonical_bytes()),
    };
    (p, m)
}
fn borrowable(file: &File) {
    rustix::io::fcntl_setfd(file, rustix::io::FdFlags::empty()).unwrap();
}

#[test]
fn exact_native_reader_meter_and_failure_boundaries() {
    let (p, m) = sources("valid");
    borrowable(&p);
    borrowable(&m);
    let floor = Inputs::INPUT_STORAGE + 13;
    let mut w = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut w, 1_000_000);
    b.reserve_storage(floor).unwrap();
    let (value, charge) = Inputs::read_at(p.as_raw_fd(), m.as_raw_fd(), &mut b).unwrap();
    assert_eq!(charge.additional_storage(), value.retained_storage());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), TOTAL_WORK);
    let peak = b.peak_storage();
    drop(value);
    for mode in 0..4 {
        let mut w = Work::new(TOTAL_WORK - usize::from(mode == 0));
        let mut b = Budget::new(&mut w, peak - usize::from(mode == 1));
        b.reserve_storage(if mode == 2 {
            Inputs::INPUT_STORAGE - 1
        } else {
            floor
        })
        .unwrap();
        let before = b.storage();
        let result = Inputs::read_at(p.as_raw_fd(), m.as_raw_fd(), &mut b);
        assert_eq!(b.storage(), before);
        match mode {
            0 => assert!(matches!(
                result,
                Err(InputError::Manifest(ManifestError::Resource(
                    Resource::Work(_)
                )))
            )),
            1 => {
                assert!(matches!(result, Err(InputError::Capability(CapabilityError::Policy(
                    fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV2::Resource(Resource::Storage(_))
                ))) | Err(InputError::Capability(CapabilityError::Launch(ManifestError::Resource(Resource::Storage(_)))))
                    | Err(InputError::Capability(CapabilityError::Resource(Resource::Storage(_))))
                    | Err(InputError::Manifest(ManifestError::Resource(Resource::Storage(_))))));
                assert_eq!(b.failed_storage(), Some(peak));
            }
            2 => assert!(matches!(
                result,
                Err(InputError::Resource(Resource::Accounting))
            )),
            _ => {
                result.unwrap();
                assert_eq!(b.peak_storage(), peak);
            }
        }
        assert!(p.metadata().is_ok());
        assert!(m.metadata().is_ok());
    }
}

#[test]
fn native_consumer_refuses_mixed_families_and_wrong_policy_without_fallback() {
    for mode in [
        "wrong-policy",
        "legacy-manifest",
        "legacy-policy",
        "corrupt",
    ] {
        let (p, m) = sources(mode);
        borrowable(&p);
        borrowable(&m);
        let mut w = Work::new(TOTAL_WORK);
        let mut b = Budget::new(&mut w, 1_000_000);
        b.reserve_storage(Inputs::INPUT_STORAGE).unwrap();
        let error = Inputs::read_at(p.as_raw_fd(), m.as_raw_fd(), &mut b).unwrap_err();
        if matches!(mode, "wrong-policy" | "legacy-manifest") {
            assert!(matches!(error, InputError::PolicyMismatch));
        } else {
            assert!(matches!(error, InputError::Capability(_)));
        }
        assert_eq!(b.storage(), Inputs::INPUT_STORAGE);
    }
}

#[test]
fn retained_inputs_survive_source_closure_and_revalidate_at_exact_boundaries() {
    let (p, m) = sources("valid");
    borrowable(&p);
    borrowable(&m);
    let mut w = Work::new(TOTAL_WORK);
    let mut b = Budget::new(&mut w, 1_000_000);
    b.reserve_storage(Inputs::INPUT_STORAGE).unwrap();
    let (inputs, _) = Inputs::read_at(p.as_raw_fd(), m.as_raw_fd(), &mut b).unwrap();
    drop((p, m));
    let work = ENTRY_WORK + PolicyCapability::IO_WORK + LaunchCapability::IO_WORK + MANIFEST_WORK;
    let floor = inputs.retained_storage();
    let peak=floor+Inputs::FRAME_STORAGE+PolicyCapability::IO_STORAGE.max(LaunchCapability::IO_STORAGE)
        .max(fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2);
    for mode in 0..4 {
        let mut w = Work::new(work - usize::from(mode == 0));
        let mut b = Budget::new(&mut w, peak - usize::from(mode == 1));
        b.reserve_storage(floor - usize::from(mode == 2)).unwrap();
        let result = inputs.revalidate(&mut b);
        assert_eq!(b.storage(), floor - usize::from(mode == 2));
        match mode {
            0 => {
                assert!(matches!(
                    result,
                    Err(InputError::Manifest(ManifestError::Resource(
                        Resource::Work(_)
                    )))
                ));
                assert_eq!(b.work(), work - MANIFEST_WORK + 8);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(InputError::Capability(CapabilityError::Resource(
                        Resource::Storage(_)
                    )))
                ));
                assert_eq!(b.failed_storage(), Some(peak));
            }
            2 => {
                assert!(matches!(
                    result,
                    Err(InputError::Resource(Resource::Accounting))
                ));
                assert_eq!(b.work(), 8);
            }
            _ => {
                result.unwrap();
                assert_eq!(b.peak_storage(), peak);
                assert_eq!(b.work(), work);
            }
        }
    }
}

#[test]
fn absent_source_refusal_preserves_the_other_borrowed_file() {
    let (p, m) = sources("valid");
    borrowable(&p);
    borrowable(&m);
    for (policy_fd, manifest_fd) in [(-1, m.as_raw_fd()), (p.as_raw_fd(), -1)] {
        let mut w = Work::new(TOTAL_WORK);
        let mut b = Budget::new(&mut w, 1_000_000);
        b.reserve_storage(Inputs::INPUT_STORAGE).unwrap();
        assert!(matches!(
            Inputs::read_at(policy_fd, manifest_fd, &mut b),
            Err(InputError::Capability(_))
        ));
        assert_eq!(b.storage(), Inputs::INPUT_STORAGE);
        assert!(p.metadata().is_ok());
        assert!(m.metadata().is_ok());
    }
}

#[test]
fn fixed_native_slots_are_admitted_after_a_real_process_exec() {
    for mode in [
        "valid",
        "consistent-other",
        "wrong-policy",
        "legacy-manifest",
        "legacy-policy",
        "corrupt",
        "short-work",
        "short-storage",
        "cloexec",
    ] {
        let (p, m) = sources(mode);
        let p = rustix::io::fcntl_dupfd_cloexec(&p, 32).unwrap();
        let m = rustix::io::fcntl_dupfd_cloexec(&m, 32).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "launch_inputs_v2::tests::inherited_slot_child",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FE2O3_TEST_NATIVE_LAUNCH_CASE", mode);
        // SAFETY: only dup2 syscalls run after fork, over retained high-numbered
        // sources disjoint from both destination slots. The child then execs.
        unsafe {
            command.pre_exec(move || {
                for (source, destination) in
                    [(p.as_raw_fd(), POLICY_FD), (m.as_raw_fd(), LAUNCH_FD)]
                {
                    if libc::dup2(source, destination) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let timed_out = loop {
            if child.try_wait().unwrap().is_some() {
                break false;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                break true;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let output = child.wait_with_output().unwrap();
        assert!(!timed_out, "native input subprocess timed out: {mode}");
        assert!(
            output.status.success(),
            "{mode}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("NATIVE_INPUTS_CHECKED"));
    }
}

#[test]
#[ignore = "isolated fixed-slot subprocess helper; exercised by the parent test"]
fn inherited_slot_child() {
    let mode = std::env::var("FE2O3_TEST_NATIVE_LAUNCH_CASE").unwrap();
    for fd in [POLICY_FD, LAUNCH_FD] {
        // SAFETY: F_GETFD checks a scalar descriptor without taking ownership.
        assert!(unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0);
    }
    // SAFETY: only the parent subprocess fixture invokes this helper and keeps
    // the two installed source descriptors live through the complete test.
    let (p, m) = unsafe {
        (
            BorrowedFd::borrow_raw(POLICY_FD),
            BorrowedFd::borrow_raw(LAUNCH_FD),
        )
    };
    let identities = [rustix::fs::fstat(p).unwrap(), rustix::fs::fstat(m).unwrap()];
    let references = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|e| {
                let metadata = std::fs::metadata(e.ok()?.path()).ok()?;
                Some(usize::from(identities.iter().any(|s| {
                    s.st_dev == metadata.dev() && s.st_ino == metadata.ino()
                })))
            })
            .sum::<usize>()
    };
    assert_eq!(references(), 2);
    if mode == "cloexec" {
        rustix::io::fcntl_setfd(m, rustix::io::FdFlags::CLOEXEC).unwrap();
    }
    rustix::fs::seek(p, rustix::fs::SeekFrom::Start(77)).unwrap();
    rustix::fs::seek(m, rustix::fs::SeekFrom::Start(55)).unwrap();
    let mut w = Work::new(if mode == "short-work" {
        TOTAL_WORK - 1
    } else {
        1_000_000
    });
    let mut b = Budget::new(
        &mut w,
        if mode == "short-storage" {
            Inputs::INPUT_STORAGE
                + Inputs::FRAME_STORAGE
                + PolicyCapability::IO_STORAGE
                + fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2
                - 1
        } else {
            1_000_000
        },
    );
    b.reserve_storage(Inputs::INPUT_STORAGE).unwrap();
    let result = Inputs::from_inherited(&mut b);
    if matches!(mode.as_str(), "valid" | "consistent-other") {
        let (inputs, charge) = result.unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(
            inputs.policy().canonical_bytes(),
            policy(if mode == "valid" { 7 } else { 8 }).canonical_bytes()
        );
        assert_eq!(
            inputs.manifest().canonical_bytes(),
            manifest(&policy(if mode == "valid" { 7 } else { 8 })).canonical_bytes()
        );
        assert_eq!(references(), 4);
        inputs.revalidate(&mut b).unwrap();
        let retained = inputs.retained_storage();
        drop(inputs);
        b.release_storage(retained).unwrap();
    } else {
        assert!(result.is_err());
    }
    assert_eq!(references(), 2);
    assert_eq!(b.storage(), Inputs::INPUT_STORAGE);
    assert!(rustix::io::fcntl_getfd(p).unwrap().is_empty());
    assert_eq!(
        rustix::io::fcntl_getfd(m)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC),
        mode == "cloexec"
    );
    assert_eq!(
        rustix::fs::seek(p, rustix::fs::SeekFrom::Current(0)).unwrap(),
        77
    );
    assert_eq!(
        rustix::fs::seek(m, rustix::fs::SeekFrom::Current(0)).unwrap(),
        55
    );
    assert_eq!(
        rustix::fs::fcntl_getfl(p).unwrap() & rustix::fs::OFlags::ACCMODE,
        rustix::fs::OFlags::RDWR
    );
    println!("NATIVE_INPUTS_CHECKED");
}
