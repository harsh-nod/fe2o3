#![cfg(target_os = "linux")]

use fe2o3_host_link_closure::{
    ApprovedStaticHostLldV1, ArtifactProvenanceV1, AuthenticatedHostLinkExecutionV1, ElfClassV1,
    ElfEndianV1, ElfProfileV1, ExecutableToolchainV1, FixedRootSetV1, HostArtifactCatalogV1,
    HostArtifactKindV1, HostLinkBrokerReservationV1, HostLinkClosureV1, HostLinkError,
    HostLinkErrorCodeV1, HostLinkHandoffV1, HostLinkPlanSpecV1, HostLinkPlanV1, OutputTypeV1,
    PlanArgumentV1, ProducerArtifactSpecV1, PublishedHostArtifactV1, ReleaseNonceV1,
    RuntimeDsoClosureV1, Sha256Digest, TargetTripleV1,
    authenticated_host_link_available_capacity_v1,
};
use rustix::fs::SealFlags;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const EXACT_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

fn release_nonce() -> ReleaseNonceV1 {
    ReleaseNonceV1::new([0x73; 32]).unwrap()
}

fn target() -> TargetTripleV1 {
    TargetTripleV1::new("x86_64-unknown-linux-gnu").unwrap()
}

fn minimal_elf(elf_type: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; 64];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&elf_type.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes
}

fn static_output_elf() -> Vec<u8> {
    let mut bytes = vec![0_u8; 121];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&object::elf::ET_EXEC.to_le_bytes());
    bytes[18..20].copy_from_slice(&object::elf::EM_X86_64.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x400078_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
    bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
    bytes[64..68].copy_from_slice(&object::elf::PT_LOAD.to_le_bytes());
    bytes[68..72].copy_from_slice(&(object::elf::PF_R | object::elf::PF_X).to_le_bytes());
    bytes[80..88].copy_from_slice(&0x400000_u64.to_le_bytes());
    bytes[88..96].copy_from_slice(&0x400000_u64.to_le_bytes());
    bytes[96..104].copy_from_slice(&121_u64.to_le_bytes());
    bytes[104..112].copy_from_slice(&121_u64.to_le_bytes());
    bytes[112..120].copy_from_slice(&0x1000_u64.to_le_bytes());
    bytes[120] = 0xc3;
    bytes
}

fn profile(elf_type: u16) -> ElfProfileV1 {
    ElfProfileV1 {
        class: ElfClassV1::Elf64,
        endian: ElfEndianV1::Little,
        elf_type,
        machine: 62,
        interpreter: None,
        soname: None,
        needed: vec![],
        has_writable_executable_segment: false,
        has_executable_stack: false,
    }
}

fn worker_path() -> &'static Path {
    static WORKER: OnceLock<PathBuf> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            if let Some(path) = std::env::var_os("FE2O3_TEST_HOST_LINK_WORKER") {
                return PathBuf::from(path);
            }
            let directory = TempDir::new().unwrap().keep();
            let output = directory.join("fe2o3-host-lld-test-worker");
            let source =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_link_worker.c");
            let result = Command::new("cc")
                .args(["-std=c11", "-O2", "-static", "-Wall", "-Wextra", "-Werror"])
                .arg(&source)
                .arg("-o")
                .arg(&output)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "failed to compile static host-link worker fixture: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            output
        })
        .as_path()
}

fn publish_file(label: &str, kind: HostArtifactKindV1, file: File) -> PublishedHostArtifactV1 {
    PublishedHostArtifactV1::from_producer_fd(
        file,
        ProducerArtifactSpecV1::new(
            label,
            kind,
            ArtifactProvenanceV1::Compiler,
            release_nonce(),
            target(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn source_file(root: &TempDir, name: &str, bytes: &[u8], mode: u32) -> File {
    let mut file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(mode)
        .open(root.path().join(name))
        .unwrap();
    file.write_all(bytes).unwrap();
    file
}

fn prepared_closure(flag: Option<&[u8]>) -> HostLinkClosureV1 {
    prepared_closure_with_tool(flag, worker_path())
}

fn prepared_closure_with_tool(flag: Option<&[u8]>, tool_path: &Path) -> HostLinkClosureV1 {
    let root = TempDir::new().unwrap();
    let wrapper = publish_file(
        "wrapper",
        HostArtifactKindV1::StaticWrapper,
        File::open(worker_path()).unwrap(),
    );
    let wrapper_id = wrapper.id();
    let lld = publish_file(
        "host-lld",
        HostArtifactKindV1::StaticHostLld,
        File::open(tool_path).unwrap(),
    );
    let lld_id = lld.id();
    let object = publish_file(
        "input.o",
        HostArtifactKindV1::Object,
        source_file(&root, "input.o", &minimal_elf(1), 0o644),
    );
    let object_id = object.id();
    let mut arguments = vec![PlanArgumentV1::ProducerArtifact(object_id)];
    if let Some(flag) = flag {
        arguments.push(PlanArgumentV1::Literal(flag.to_vec()));
    }
    let spec = HostLinkPlanSpecV1 {
        release_nonce: release_nonce(),
        target: target(),
        toolchain: ExecutableToolchainV1 {
            static_wrapper: wrapper_id,
            static_host_lld: lld_id,
            llvm_build_identity: "upstream-llvmorg-22.1.8-test".to_owned(),
        },
        output_type: OutputTypeV1::Executable,
        expected_output_mode: 0o555,
        expected_output_elf: profile(2),
        arguments,
        runtime_dsos: RuntimeDsoClosureV1::default(),
    };
    let handoff = HostLinkHandoffV1::new(spec, vec![object, lld, wrapper]).unwrap();
    let (plan, producers) = handoff.into_parts();
    let plan = HostLinkPlanV1::from_sealed_fd(plan, producers).unwrap();
    let mut closure = HostLinkClosureV1::prepare(
        plan,
        FixedRootSetV1::new(vec![]).unwrap(),
        HostArtifactCatalogV1::new(release_nonce(), target()),
    )
    .unwrap();
    closure.prevalidate().unwrap();
    closure
}

trait UnsafeFixtureLaunchV1 {
    fn launch_unsafe_test_fixture(self) -> Result<AuthenticatedHostLinkExecutionV1, HostLinkError>;
}

impl UnsafeFixtureLaunchV1 for HostLinkClosureV1 {
    #[allow(unsafe_code)]
    fn launch_unsafe_test_fixture(self) -> Result<AuthenticatedHostLinkExecutionV1, HostLinkError> {
        // SAFETY: this helper is test-only. The fixture executable was built by this test binary;
        // production code must obtain approval from the W1 evidence authority.
        let approval = unsafe { ApprovedStaticHostLldV1::from_verified_evidence(&self)? };
        self.launch(approval)
    }
}

fn await_admission(execution: &mut AuthenticatedHostLinkExecutionV1) -> Result<(), HostLinkError> {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        match execution.try_admit_output() {
            Ok(_) => return Ok(()),
            Err(error) if error.code() == HostLinkErrorCodeV1::ResultPending => {
                assert!(Instant::now() < deadline, "authenticated launch timed out");
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error),
        }
    }
}

fn await_reaper_capacity(expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while authenticated_host_link_available_capacity_v1() != expected {
        assert!(
            Instant::now() < deadline,
            "deferred pidfd reaper did not restore capacity"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn authenticated_execveat_launch_admits_receiver_owned_output() {
    let closure = prepared_closure(None);
    let plan = closure.plan_digest();
    let binding = closure.closure_digest();
    let mut execution = closure.launch_unsafe_test_fixture().unwrap();
    assert_ne!(execution.process_id(), 0);
    assert_eq!(execution.plan_digest(), plan);
    assert_eq!(execution.closure_digest(), binding);
    await_admission(&mut execution).unwrap();

    let admitted = execution.try_admit_output().unwrap_err();
    assert_eq!(admitted.code(), HostLinkErrorCodeV1::InvalidState);
    execution.revalidate().unwrap();
}

#[test]
fn ordinary_w0_output_carries_no_broker_reservation() {
    let closure = prepared_closure(None);
    let request_nonce = closure.nonce_sha256();
    let mut execution = closure.launch_unsafe_test_fixture().unwrap();
    assert_eq!(execution.broker_reservation(), None);
    await_admission(&mut execution).unwrap();
    let output = execution.into_admitted_output().unwrap();
    assert_eq!(output.broker_reservation(), None);
    assert_eq!(output.request_nonce_sha256(), request_nonce);
}

#[test]
#[allow(unsafe_code)]
fn broker_reservation_rebinds_authenticated_request_and_output_identity() {
    let closure = prepared_closure(None);
    let old_nonce = closure.nonce_sha256();
    let old_request = closure.lld_argv().unwrap().canonical_arguments()[3].clone();
    let reservation =
        HostLinkBrokerReservationV1::from_sha256(Sha256Digest::from_bytes([0xa5; 32])).unwrap();
    let bound = closure.bind_broker_reservation(reservation).unwrap();
    let bound_nonce = bound.request_nonce_sha256();
    assert_ne!(bound_nonce, old_nonce);
    assert_ne!(
        bound.closure().lld_argv().unwrap().canonical_arguments()[3],
        old_request
    );
    assert_eq!(bound.authority(), "none");
    assert_eq!(bound.broker_reservation(), reservation);

    // SAFETY: this test-only fixture stands in for an external W1 tool-evidence authority.
    let approval =
        unsafe { ApprovedStaticHostLldV1::from_verified_evidence(bound.closure()).unwrap() };
    let mut execution = bound.launch(approval).unwrap();
    assert_eq!(execution.broker_reservation(), Some(reservation));
    assert_eq!(execution.nonce_sha256(), bound_nonce);
    await_admission(&mut execution).unwrap();
    let output = execution.into_admitted_output().unwrap();
    assert_eq!(output.broker_reservation(), Some(reservation));
    assert_eq!(output.request_nonce_sha256(), bound_nonce);
}

#[test]
#[allow(unsafe_code)]
fn approval_is_move_only_and_rejects_a_different_plan_binding() {
    let approved_closure = prepared_closure(None);
    // SAFETY: this test deliberately stands in for the future trusted W1 evidence authority.
    let approval =
        unsafe { ApprovedStaticHostLldV1::from_verified_evidence(&approved_closure).unwrap() };
    let different_plan = prepared_closure(Some(b"--fatal-warnings"));
    let error = different_plan
        .launch(approval)
        .err()
        .expect("approval for another plan must not authorize launch");
    assert_eq!(error.code(), HostLinkErrorCodeV1::ToolApproval);
}

#[test]
fn admitted_descriptor_has_exact_bytes_mode_seals_and_offset_independence() {
    let mut execution = prepared_closure(None).launch_unsafe_test_fixture().unwrap();
    await_admission(&mut execution).unwrap();
    let admitted = execution
        .try_admit_output()
        .expect_err("second admission is rejected");
    assert_eq!(admitted.code(), HostLinkErrorCodeV1::InvalidState);

    // Re-run once to inspect the move-only admitted capability returned on first admission.
    let mut execution = prepared_closure(None).launch_unsafe_test_fixture().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match execution.try_admit_output() {
            Ok(admitted) => {
                let mut file = admitted.try_clone_file().unwrap();
                assert_eq!(file.metadata().unwrap().mode() & 0o7777, 0o555);
                assert_eq!(rustix::fs::fcntl_get_seals(&file).unwrap(), EXACT_SEALS);
                let mut bytes = vec![0_u8; admitted.size() as usize];
                assert_eq!(
                    rustix::io::pread(&file, &mut bytes, 0).unwrap(),
                    bytes.len()
                );
                assert_eq!(bytes, static_output_elf());
                file.seek(SeekFrom::Start(31)).unwrap();
                execution.revalidate().unwrap();
                assert_eq!(file.stream_position().unwrap(), 31);
                break;
            }
            Err(error) if error.code() == HostLinkErrorCodeV1::ResultPending => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("unexpected admission failure: {error}"),
        }
    }
}

#[test]
fn nonzero_signal_and_death_before_send_reject() {
    for (flag, expected) in [
        (
            b"--fatal-warnings".as_slice(),
            HostLinkErrorCodeV1::WorkerExit,
        ),
        (b"--discard-all".as_slice(), HostLinkErrorCodeV1::WorkerExit),
        (
            b"--no-undefined".as_slice(),
            HostLinkErrorCodeV1::OutputEmpty,
        ),
    ] {
        let mut execution = prepared_closure(Some(flag))
            .launch_unsafe_test_fixture()
            .unwrap();
        let error = await_admission(&mut execution).unwrap_err();
        assert_eq!(error.code(), expected, "worker mode {:?}", flag);
    }
}

#[test]
fn descendant_sender_and_post_send_mutator_are_denied_by_seccomp() {
    let mut fake = prepared_closure(Some(b"--static"))
        .launch_unsafe_test_fixture()
        .unwrap();
    assert_eq!(
        await_admission(&mut fake).unwrap_err().code(),
        HostLinkErrorCodeV1::WorkerExit
    );

    let mut mutator = prepared_closure(Some(b"--gc-sections"))
        .launch_unsafe_test_fixture()
        .unwrap();
    assert_eq!(
        await_admission(&mut mutator).unwrap_err().code(),
        HostLinkErrorCodeV1::WorkerExit
    );
}

#[test]
fn descriptor_replay_rejects() {
    let mut replay = prepared_closure(Some(b"--discard-locals"))
        .launch_unsafe_test_fixture()
        .unwrap();
    assert_eq!(
        await_admission(&mut replay).unwrap_err().code(),
        HostLinkErrorCodeV1::DuplicateRecord
    );
}

#[test]
fn no_send_hang_hits_the_internal_terminal_deadline() {
    if std::env::var_os("FE2O3_TEST_TIMEOUT_REAP").is_none() {
        let worker = worker_path().to_path_buf();
        let executable = std::env::current_exe().unwrap();
        let status = Command::new(executable)
            .args([
                "--exact",
                "no_send_hang_hits_the_internal_terminal_deadline",
                "--nocapture",
            ])
            .env("FE2O3_TEST_TIMEOUT_REAP", "1")
            .env("FE2O3_TEST_HOST_LINK_WORKER", worker)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }

    let initial_capacity = authenticated_host_link_available_capacity_v1();
    let mut execution = prepared_closure(Some(b"--build-id=none"))
        .launch_unsafe_test_fixture()
        .unwrap();
    let started = Instant::now();
    assert_eq!(
        await_admission(&mut execution).unwrap_err().code(),
        HostLinkErrorCodeV1::WorkerTimeout
    );
    assert!(started.elapsed() < Duration::from_secs(31));
    assert_eq!(
        execution.try_admit_output().unwrap_err().code(),
        HostLinkErrorCodeV1::InvalidState
    );
    await_reaper_capacity(initial_capacity);
}

#[test]
fn stopped_worker_drop_is_bounded_and_reap_is_eventual() {
    if std::env::var_os("FE2O3_TEST_STOPPED_REAP").is_none() {
        let worker = worker_path().to_path_buf();
        let executable = std::env::current_exe().unwrap();
        let status = Command::new(executable)
            .args([
                "--exact",
                "stopped_worker_drop_is_bounded_and_reap_is_eventual",
                "--nocapture",
            ])
            .env("FE2O3_TEST_STOPPED_REAP", "1")
            .env("FE2O3_TEST_HOST_LINK_WORKER", worker)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }

    let initial_capacity = authenticated_host_link_available_capacity_v1();
    let execution = prepared_closure(Some(b"--hash-style=gnu"))
        .launch_unsafe_test_fixture()
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    drop(execution);
    assert!(started.elapsed() < Duration::from_secs(1));
    await_reaper_capacity(initial_capacity);
}

#[test]
fn send_then_hang_hits_the_internal_terminal_deadline() {
    let mut execution = prepared_closure(Some(b"--eh-frame-hdr"))
        .launch_unsafe_test_fixture()
        .unwrap();
    assert_eq!(
        await_admission(&mut execution).unwrap_err().code(),
        HostLinkErrorCodeV1::WorkerTimeout
    );
    assert_eq!(
        execution.try_admit_output().unwrap_err().code(),
        HostLinkErrorCodeV1::InvalidState
    );
}

#[test]
fn late_large_outputs_use_bounded_polls_and_never_admit_past_deadline() {
    let mut executions = [
        Some(
            prepared_closure(Some(b"--no-undefined-version"))
                .launch_unsafe_test_fixture()
                .unwrap(),
        ),
        Some(
            prepared_closure(Some(b"--no-allow-shlib-undefined"))
                .launch_unsafe_test_fixture()
                .unwrap(),
        ),
    ];
    let started = Instant::now();
    let mut terminal = 0;
    while terminal != executions.len() {
        for execution in &mut executions {
            let Some(active) = execution.as_mut() else {
                continue;
            };
            let poll_started = Instant::now();
            match active.try_admit_output() {
                Ok(_) => panic!("late large output was admitted past the fixed deadline"),
                Err(error) if error.code() == HostLinkErrorCodeV1::ResultPending => {}
                Err(error) => {
                    assert_eq!(error.code(), HostLinkErrorCodeV1::WorkerTimeout);
                    *execution = None;
                    terminal += 1;
                }
            }
            assert!(
                poll_started.elapsed() < Duration::from_secs(1),
                "one public admission poll exceeded its bounded work quantum"
            );
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(started.elapsed() >= Duration::from_secs(29));
    assert!(started.elapsed() < Duration::from_secs(31));
}

#[test]
fn unsealed_and_wrong_hash_outputs_reject() {
    for (flag, expected) in [
        (
            b"-Bstatic".as_slice(),
            HostLinkErrorCodeV1::DescriptorUnsealed,
        ),
        (
            b"--strip-debug".as_slice(),
            HostLinkErrorCodeV1::DigestMismatch,
        ),
    ] {
        let mut execution = prepared_closure(Some(flag))
            .launch_unsafe_test_fixture()
            .unwrap();
        assert_eq!(
            await_admission(&mut execution).unwrap_err().code(),
            expected
        );
    }
}

#[test]
fn wrong_result_bindings_and_elf_profile_reject() {
    for (flag, expected) in [
        (b"-O0".as_slice(), HostLinkErrorCodeV1::ReplayMismatch),
        (b"-O1".as_slice(), HostLinkErrorCodeV1::ReplayMismatch),
        (b"-O2".as_slice(), HostLinkErrorCodeV1::WrongNonce),
        (b"-O3".as_slice(), HostLinkErrorCodeV1::ElfPolicy),
    ] {
        let mut execution = prepared_closure(Some(flag))
            .launch_unsafe_test_fixture()
            .unwrap();
        assert_eq!(
            await_admission(&mut execution).unwrap_err().code(),
            expected,
            "worker mode {:?}",
            flag
        );
    }
}

#[test]
fn sender_mode_is_not_result_wire_authority() {
    let mut execution = prepared_closure(Some(b"--no-dynamic-linker"))
        .launch_unsafe_test_fixture()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match execution.try_admit_output() {
            Ok(admitted) => {
                assert_eq!(admitted.mode(), 0o555);
                break;
            }
            Err(error) if error.code() == HostLinkErrorCodeV1::ResultPending => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("unexpected admission failure: {error}"),
        }
    }
}

#[test]
fn wrong_sealed_executable_fails_at_execveat_without_path_fallback() {
    let directory = TempDir::new().unwrap();
    let fake = directory.path().join("not-really-executable");
    std::fs::write(&fake, minimal_elf(2)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = prepared_closure_with_tool(None, &fake)
        .launch_unsafe_test_fixture()
        .err()
        .expect("minimal ELF header cannot execute");
    assert_eq!(error.code(), HostLinkErrorCodeV1::WorkerLaunch);
}

#[test]
fn fast_exit_pidfd_capture_is_reliable_while_child_remains_unreaped() {
    for _ in 0..32 {
        let mut execution = prepared_closure(None).launch_unsafe_test_fixture().unwrap();
        await_admission(&mut execution).unwrap();
    }
}

#[test]
fn authenticated_launch_works_with_a_256_descriptor_soft_limit() {
    if std::env::var_os("FE2O3_TEST_LOW_NOFILE").is_some() {
        assert_eq!(
            rustix::process::getrlimit(rustix::process::Resource::Nofile).current,
            Some(256)
        );
        let mut execution = prepared_closure(None).launch_unsafe_test_fixture().unwrap();
        await_admission(&mut execution).unwrap();
        return;
    }

    let worker = worker_path().to_path_buf();
    let executable = std::env::current_exe().unwrap();
    let status = Command::new("sh")
        .arg("-c")
        .arg("ulimit -n 256; exec \"$1\" --exact authenticated_launch_works_with_a_256_descriptor_soft_limit --nocapture")
        .arg("fe2o3-low-nofile-test")
        .arg(executable)
        .env("FE2O3_TEST_LOW_NOFILE", "1")
        .env("FE2O3_TEST_HOST_LINK_WORKER", worker)
        .status()
        .unwrap();
    assert!(status.success());
}

#[allow(unsafe_code)]
unsafe extern "C" fn ambient_signal_handler(_signal: i32) {}

#[allow(unsafe_code)]
fn install_ambient_signal_state() {
    let caught = rustix::runtime::KernelSigaction {
        sa_handler_kernel: Some(ambient_signal_handler),
        ..rustix::runtime::KernelSigaction::default()
    };
    let ignored = rustix::runtime::KernelSigaction {
        sa_handler_kernel: rustix::runtime::kernel_sig_ign(),
        ..rustix::runtime::KernelSigaction::default()
    };
    let mut blocked = rustix::runtime::KernelSigSet::empty();
    blocked.insert(rustix::runtime::Signal::USR1);
    blocked.insert(rustix::runtime::Signal::USR2);
    // SAFETY: this runs only in an isolated test subprocess. Both signals remain blocked until
    // clone3, so the deliberately minimal caught handler cannot execute in the Rust test parent.
    unsafe {
        rustix::runtime::kernel_sigaction(rustix::runtime::Signal::USR1, Some(caught)).unwrap();
        rustix::runtime::kernel_sigaction(rustix::runtime::Signal::USR2, Some(ignored)).unwrap();
        rustix::runtime::kernel_sigprocmask(rustix::runtime::How::BLOCK, Some(&blocked)).unwrap();
    }
}

#[test]
fn ambient_signal_handlers_ignores_and_mask_are_normalized() {
    if std::env::var_os("FE2O3_TEST_AMBIENT_SIGNALS").is_some() {
        install_ambient_signal_state();
        let mut execution = prepared_closure(None).launch_unsafe_test_fixture().unwrap();
        await_admission(&mut execution).unwrap();
        return;
    }

    let worker = worker_path().to_path_buf();
    let executable = std::env::current_exe().unwrap();
    let status = Command::new(executable)
        .args([
            "--exact",
            "ambient_signal_handlers_ignores_and_mask_are_normalized",
            "--nocapture",
        ])
        .env("FE2O3_TEST_AMBIENT_SIGNALS", "1")
        .env("FE2O3_TEST_HOST_LINK_WORKER", worker)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn ambient_sigchld_ignore_preserves_the_atomic_pidfd_witness() {
    if std::env::var_os("FE2O3_TEST_SIGCHLD_IGNORED").is_some() {
        let mut execution = prepared_closure(None).launch_unsafe_test_fixture().unwrap();
        await_admission(&mut execution).unwrap();
        return;
    }

    let worker = worker_path().to_path_buf();
    let executable = std::env::current_exe().unwrap();
    let status = Command::new("sh")
        .arg("-c")
        .arg("trap '' CHLD; exec \"$1\" --exact ambient_sigchld_ignore_preserves_the_atomic_pidfd_witness --nocapture")
        .arg("fe2o3-sigchld-test")
        .arg(executable)
        .env("FE2O3_TEST_SIGCHLD_IGNORED", "1")
        .env("FE2O3_TEST_HOST_LINK_WORKER", worker)
        .status()
        .unwrap();
    assert!(status.success());
}
