//! Public native issuer admission, not issuer activation or deployment qualification.
//!
//! Build this test target with the pinned musl toolchain and static relocation/linking.
//! Run ONLY `isolated_static_public_admission_matrix --exact --ignored --nocapture`
//! in a disposable root container with explicit FE2O3_RUN_NATIVE_ISSUER_ADMISSION=1.
//! The runner must provide read-only root/executable, private /tmp and namespaces,
//! no network/GPU, default seccomp, no-new-privileges, drop-all capabilities plus
//! CHOWN/KILL/SETUID/SETGID, and CPU/memory/PID and 600-second outer limits.
//! Helpers are ignored subprocess roles, not independently runnable test cases.
//! No missing prerequisite is reported as a skipped success. Fixture management
//! is outside logical verification accounting; every native owner operation in
//! a case uses the same original budget, except deliberate foreign-ledger probes.
#![forbid(unsafe_code)]

use ed25519_dalek::SigningKey;
use fe2o3_broker_authority_service::{
    AdmissionErrorKindV1 as TransportKind, CurrentStaticIssuerMeasurementsV1 as Measurements,
    ExpectedClientProcessIdentityV1, IssuerAdmissionErrorKindV1 as IssuerKind,
    LiveClientPidfdIdentityV2 as Client, ProtectedCompilerExecutionIssuerAdmissionErrorV2 as Error,
    ProtectedCompilerExecutionIssuerAdmissionV2 as Admission,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError, ProtectedIssuerProcessV1 as Process,
    ProtectedServiceAdmissionErrorV2 as ServiceError, ProtectedServiceAdmissionV2 as Service,
    current_static_issuer_measurements_v1,
};
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2 as Key;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorIdentity,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::{
    fs::OFlags,
    net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg,
        sendmsg, socketpair,
    },
    process::{Pid, PidfdFlags, pidfd_open},
};
use std::{
    error::Error as _,
    fs::{self, File},
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const OPT_IN: &str = "FE2O3_RUN_NATIVE_ISSUER_ADMISSION";
const CASE_ENV: &str = "FE2O3_NATIVE_ISSUER_CASE";
const PEER_ENV: &str = "FE2O3_NATIVE_ISSUER_PEER";
const CASE_TEST: &str = "native_admission_case_child";
const PEER_TEST: &str = "native_admission_peer_child";
const CLIENT_ID: u32 = 65_534;
const ANCHOR_ID: u32 = 65_533;
const WORK_LIMIT: usize = 1_000_000_000_000;
const STORAGE_LIMIT: usize = 512 * 1024 * 1024;
const EXTRA: usize = 23;
const PREFIX_WORK: usize = 19;
const STEP: Duration = Duration::from_millis(5);
const IPC_TIMEOUT: Duration = Duration::from_secs(15);
const CASE_TIMEOUT: Duration = Duration::from_secs(60);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

const CASES: &[&str] = &[
    "success",
    "foreign-ledger",
    "wrong-executable",
    "wrong-runtime",
    "policy-generation",
    "policy-executable",
    "policy-runtime",
    "policy-signing-key",
    "policy-anchor-key",
    "root-before",
    "root-after",
    "socket-before",
    "socket-after",
    "client-before",
    "client-after",
    "anchor-before",
    "anchor-after",
];

#[path = "native_service_v2/mod.rs"]
mod native_service;

fn require_container() {
    assert_eq!(
        std::env::var(OPT_IN).as_deref(),
        Ok("1"),
        "explicit isolated-run opt-in required"
    );
    assert!(
        Path::new("/.dockerenv").is_file(),
        "isolated Docker fixture required"
    );
}

fn require_root() {
    require_container();
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    assert_eq!(rustix::process::getegid().as_raw(), 0);
}

fn command(test: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            test,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env(OPT_IN, "1")
        .env("TMPDIR", "/tmp")
        .current_dir("/tmp")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
}

struct ChildGuard(Child);
impl ChildGuard {
    fn wait(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        // Both attempt count and elapsed time are bounded, including scheduler stalls.
        for _ in 0..24_000 {
            if let Some(status) = self.0.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(STEP);
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "native admission child timed out",
        ))
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.0.kill();
        if let Err(error) = self.wait(CLEANUP_TIMEOUT) {
            eprintln!("native admission child cleanup failed: {error}");
        }
    }
}

fn wait_io<T>(timeout: Duration, mut operation: impl FnMut() -> rustix::io::Result<T>) -> T {
    let deadline = Instant::now() + timeout;
    for _ in 0..24_000 {
        match operation() {
            Ok(value) => return value,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
            Err(error) => panic!("native admission fixture I/O: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "native admission fixture I/O timed out"
        );
        thread::sleep(STEP);
    }
    panic!("native admission fixture I/O attempt limit");
}

fn pair() -> (OwnedFd, OwnedFd) {
    socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}

fn send_fd(control: &OwnedFd, fd: &OwnedFd) {
    let descriptors = [fd.as_fd()];
    let sent = wait_io(IPC_TIMEOUT, || {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
        sendmsg(
            control,
            &[IoSlice::new(&[0x46])],
            &mut ancillary,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        )
    });
    assert_eq!(sent, 1);
}

fn receive_fd(control: &OwnedFd) -> OwnedFd {
    wait_io(IPC_TIMEOUT, || {
        let mut tag = [0];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = recvmsg(
            control,
            &mut [IoSliceMut::new(&mut tag)],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        )?;
        assert_eq!(received.bytes, 1);
        assert_eq!(tag, [0x46]);
        assert!(
            !received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        );
        let mut result = None;
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(descriptors) => {
                    for descriptor in descriptors {
                        assert!(
                            result.replace(descriptor).is_none(),
                            "extra transferred descriptor"
                        );
                    }
                }
                _ => panic!("unexpected ancillary message"),
            }
        }
        Ok(result.expect("missing transferred descriptor"))
    })
}

#[test]
#[ignore = "private distinct-UID peer role; launched only by the isolated static matrix"]
fn native_admission_peer_child() {
    require_container();
    let expected = match std::env::var(PEER_ENV).as_deref() {
        Ok("client" | "client-service") => CLIENT_ID,
        Ok("anchor") => ANCHOR_ID,
        _ => panic!("explicit peer role required"),
    };
    assert_eq!(rustix::process::geteuid().as_raw(), expected);
    assert_eq!(rustix::process::getegid().as_raw(), expected);
    let control = rustix::io::fcntl_dupfd_cloexec(std::io::stdin(), 3).unwrap();
    // Creating the admitted pair after credential change binds SO_PEERCRED to
    // this live child, not to the root test process that created the control pair.
    let (endpoint, held_peer) = pair();
    send_fd(&control, &endpoint);
    drop(endpoint);
    if std::env::var(PEER_ENV).as_deref() == Ok("client-service") {
        send_fd(&control, &held_peer);
    }
    let mut stop = [0];
    let count = wait_io(Duration::from_secs(120), || {
        rustix::io::read(&control, &mut stop)
    });
    assert_eq!(count, 1);
    assert_eq!(stop, [0x51]);
    drop(held_peer);
}

struct Peer {
    child: ChildGuard,
    control: OwnedFd,
    endpoint: OwnedFd,
    uid: u32,
}
impl Peer {
    fn spawn(role: &str, uid: u32) -> Self {
        let (control, child_control) = pair();
        let mut command = command(PEER_TEST);
        command
            .env(PEER_ENV, role)
            .uid(uid)
            .gid(uid)
            .stdin(Stdio::from(child_control));
        let child = ChildGuard(command.spawn().unwrap());
        drop(command);
        let endpoint = receive_fd(&control);
        let credentials = rustix::net::sockopt::socket_peercred(&endpoint).unwrap();
        assert_eq!(credentials.pid.as_raw_nonzero().get() as u32, child.0.id());
        assert_eq!(credentials.uid.as_raw(), uid);
        assert_eq!(credentials.gid.as_raw(), uid);
        Self {
            child,
            control,
            endpoint,
            uid,
        }
    }

    fn pidfd(&self) -> OwnedFd {
        pidfd_open(
            Pid::from_raw(self.child.0.id() as i32).unwrap(),
            PidfdFlags::empty(),
        )
        .unwrap()
    }

    fn stop(&mut self) {
        if let Some(status) = self.child.0.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        assert_eq!(
            wait_io(IPC_TIMEOUT, || rustix::io::write(&self.control, &[0x51])),
            1
        );
        assert!(self.child.wait(CLEANUP_TIMEOUT).unwrap().success());
    }
}

struct Root {
    path: PathBuf,
    file: File,
}
impl Root {
    fn create() -> Self {
        let path = PathBuf::from(format!("/tmp/fe2o3-native-issuer-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let file = File::open(&path).unwrap();
        Self { path, file }
    }
    fn invalidate(&self) {
        self.file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap();
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        // Admission writes no journal. Remove only this create-new empty directory.
        if let Err(error) = fs::remove_dir(&self.path) {
            eprintln!("native admission root cleanup failed: {error}");
        }
    }
}

fn duplicate(fd: &impl AsFd) -> OwnedFd {
    rustix::io::fcntl_dupfd_cloexec(fd, 3).unwrap()
}

fn references(fd: &impl AsFd) -> usize {
    let identity = rustix::fs::fstat(fd).unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| {
            let metadata = fs::metadata(entry.ok()?.path()).ok()?;
            Some((metadata.dev(), metadata.ino()) == (identity.st_dev, identity.st_ino))
        })
        .filter(|same| *same)
        .count()
}

fn policy(measurements: Measurements, change: &str, budget: &mut Budget<'_>) -> Policy {
    let mut executable = measurements.executable();
    let mut runtime = measurements.runtime();
    let mut generation = 1;
    let mut signing = SigningKey::from_bytes(&[0x31; 32])
        .verifying_key()
        .to_bytes();
    let mut anchor = SigningKey::from_bytes(&[0x71; 32])
        .verifying_key()
        .to_bytes();
    match change {
        "policy-generation" => generation = 2,
        "wrong-executable" | "policy-executable" => {
            executable = Measurement::new([0x11; 32], 1).unwrap()
        }
        "wrong-runtime" | "policy-runtime" => runtime = Measurement::new([0x22; 32], 1).unwrap(),
        "policy-signing-key" => {
            signing = SigningKey::from_bytes(&[0x32; 32])
                .verifying_key()
                .to_bytes()
        }
        "policy-anchor-key" => {
            anchor = SigningKey::from_bytes(&[0x72; 32])
                .verifying_key()
                .to_bytes()
        }
        "unchanged" => {}
        _ => panic!("unknown policy change"),
    }
    let (policy, growth) =
        Policy::new(generation, executable, runtime, signing, anchor, budget).unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    policy
}

struct Inputs {
    process: Process,
    service: Service,
    policy: Policy,
    key: Key,
    anchor: Anchor,
    storage: usize,
}

fn inputs(
    case: &str,
    measurements: Measurements,
    root: &Root,
    client: &Peer,
    anchor: &Peer,
    budget: &mut Budget<'_>,
) -> Inputs {
    let process = Process::harden().unwrap();
    budget
        .reserve_storage(
            Admission::PROCESS_STORAGE
                + Service::FD_PAIR_STORAGE
                + Client::FD_STORAGE
                + Anchor::PAIR_STORAGE,
        )
        .unwrap();
    let expected =
        ExpectedClientProcessIdentityV1::new(client.child.0.id(), client.uid, client.uid).unwrap();
    let (live, growth) = Client::admit(client.pidfd(), expected, budget).unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    let (service, growth) = Service::admit(
        duplicate(&root.file),
        duplicate(&client.endpoint),
        live,
        budget,
    )
    .unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    let (anchor, growth) = Anchor::admit(
        duplicate(&anchor.endpoint),
        anchor.pidfd(),
        AnchorIdentity::new(anchor.uid, anchor.uid).unwrap(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    let initial_change = if matches!(case, "wrong-executable" | "wrong-runtime") {
        case
    } else {
        "unchanged"
    };
    let mut policy = policy(measurements, initial_change, budget);
    let (key, growth) = {
        let mut seed = [0x31; 32];
        budget.reserve_storage(seed.len()).unwrap();
        let result = Key::create_and_zeroize(&mut seed, &policy, budget).unwrap();
        assert_eq!(seed, [0; 32]);
        result
    };
    budget.release_storage(32).unwrap();
    budget.reserve_storage(growth.additional_storage()).unwrap();
    if case.starts_with("policy-") {
        let old = policy.retained_storage();
        drop(policy);
        budget.release_storage(old).unwrap();
        policy = self::policy(measurements, case, budget);
    }
    let storage = Admission::PROCESS_STORAGE
        + service.retained_storage()
        + policy.retained_storage()
        + key.retained_storage()
        + anchor.retained_storage();
    assert_eq!(budget.storage(), EXTRA + storage);
    Inputs {
        process,
        service,
        policy,
        key,
        anchor,
        storage,
    }
}

fn transport_error(error: &Error, anchor: bool, expected: TransportKind) {
    assert_eq!(error.kind(), Some(IssuerKind::ServiceAdmission));
    let source = error.source().expect("native transport diagnostic");
    let kind = if anchor {
        source
            .downcast_ref::<AnchorError>()
            .expect("anchor error")
            .kind()
    } else {
        source
            .downcast_ref::<ServiceError>()
            .expect("service error")
            .kind()
    };
    assert_eq!(kind, Some(expected));
    assert_eq!(error.resource(), None);
}

fn invalidation(case: &str, root: &Root, client: &mut Peer, anchor: &mut Peer) {
    match case {
        "root-before" | "root-after" => root.invalidate(),
        "socket-before" | "socket-after" => {
            rustix::fs::fcntl_setfl(&client.endpoint, OFlags::RDWR).unwrap();
        }
        "client-before" | "client-after" => client.stop(),
        "anchor-before" | "anchor-after" => anchor.stop(),
        _ => panic!("unknown retained-owner fault"),
    }
}

fn expect_refusal(error: &Error, case: &str) {
    match case {
        "wrong-executable" => assert_eq!(error.kind(), Some(IssuerKind::ExecutablePolicyMismatch)),
        "wrong-runtime" => assert_eq!(error.kind(), Some(IssuerKind::RuntimePolicyMismatch)),
        c if c.starts_with("policy-") => assert_eq!(error.kind(), Some(IssuerKind::KeyCapability)),
        "root-before" | "root-after" => transport_error(error, false, TransportKind::RootMode),
        "socket-before" | "socket-after" => {
            transport_error(error, false, TransportKind::PeerStatusFlags)
        }
        "client-before" | "client-after" => {
            transport_error(error, false, TransportKind::ClientAlreadyDead)
        }
        "anchor-before" | "anchor-after" => {
            transport_error(error, true, TransportKind::ClientAlreadyDead)
        }
        _ => panic!("unexpected refusal case"),
    }
}

#[test]
#[ignore = "private case role; launched only by the isolated static matrix"]
fn native_admission_case_child() {
    require_root();
    let case = std::env::var(CASE_ENV).expect("explicit case role required");
    if native_service::CASES.contains(&case.as_str()) {
        native_service::run(&case);
        return;
    }
    assert!(CASES.contains(&case.as_str()));
    // Validates the real running image; a dynamic test binary fails, never skips.
    let measurements =
        current_static_issuer_measurements_v1().expect("real sealed-static executable required");
    let root = Root::create();
    let mut client = Peer::spawn("client", CLIENT_ID);
    let mut anchor = Peer::spawn("anchor", ANCHOR_ID);
    let reference_floor = (
        references(&root.file),
        references(&client.endpoint),
        references(&anchor.endpoint),
    );
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(EXTRA).unwrap();
    budget.charge_work(PREFIX_WORK).unwrap();
    let original = budget.work_ledger_identity_v1();
    let Inputs {
        process,
        service,
        policy,
        key,
        anchor: anchor_owner,
        storage,
    } = inputs(&case, measurements, &root, &client, &anchor, &mut budget);
    let policy_identity = policy.identity();
    if case.ends_with("-before") {
        invalidation(&case, &root, &mut client, &mut anchor);
    }
    let before = budget.work();
    let result = Admission::admit(process, service, policy, key, anchor_owner, &mut budget);
    assert!(budget.work() > before);
    assert!(budget.work_ledger_identity_v1() == original);
    assert_eq!(budget.storage(), EXTRA + storage);
    if case.starts_with("policy-") || case.starts_with("wrong-") || case.ends_with("-before") {
        expect_refusal(&result.unwrap_err(), &case);
        budget.release_storage(storage).unwrap();
    } else {
        let (admission, growth) =
            result.expect("whole public native issuer admission must succeed");
        assert_eq!(
            admission.retained_storage(),
            storage + growth.additional_storage()
        );
        budget.reserve_storage(growth.additional_storage()).unwrap();
        let retained = admission.retained_storage();
        assert_eq!(admission.policy().identity(), policy_identity);
        assert_eq!(admission.measurements(), measurements);
        assert!(!admission.authenticates_protected_compiler_execution());
        assert!(!admission.grants_compiler_authority());
        let before = budget.work();
        admission
            .validate_continuity(&mut budget)
            .expect("public native continuity must succeed");
        assert!(budget.work() > before);
        assert_eq!(budget.storage(), EXTRA + retained);
        if case == "foreign-ledger" {
            let mut foreign_work = Work::new(WORK_LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, STORAGE_LIMIT);
            foreign.reserve_storage(EXTRA + retained).unwrap();
            assert!(foreign.work_ledger_identity_v1() != original);
            let original_work = budget.work();
            assert_eq!(
                admission
                    .validate_continuity(&mut foreign)
                    .unwrap_err()
                    .resource(),
                Some(Resource::Accounting)
            );
            assert_eq!(budget.work(), original_work);
            assert_eq!(foreign.storage(), EXTRA + retained);
            admission.validate_continuity(&mut budget).unwrap();
            root.invalidate();
            assert_eq!(
                admission
                    .validate_continuity(&mut foreign)
                    .unwrap_err()
                    .resource(),
                Some(Resource::Accounting)
            );
            transport_error(
                &admission.validate_continuity(&mut budget).unwrap_err(),
                false,
                TransportKind::RootMode,
            );
            assert_eq!(foreign.storage(), EXTRA + retained);
            foreign.release_storage(EXTRA + retained).unwrap();
            assert_eq!(foreign.storage(), 0);
        } else if case.ends_with("-after") {
            invalidation(&case, &root, &mut client, &mut anchor);
            expect_refusal(
                &admission.validate_continuity(&mut budget).unwrap_err(),
                &case,
            );
        } else {
            // Moving the Budget view must not change the underlying live work identity.
            let mut moved = budget;
            admission.validate_continuity(&mut moved).unwrap();
            budget = moved;
        }
        assert_eq!(budget.storage(), EXTRA + retained);
        drop(admission);
        budget.release_storage(retained).unwrap();
    }
    assert_eq!(budget.storage(), EXTRA);
    assert!(budget.work_ledger_identity_v1() == original);
    assert_eq!(
        (
            references(&root.file),
            references(&client.endpoint),
            references(&anchor.endpoint)
        ),
        reference_floor
    );
    assert_eq!(
        fs::read_dir(&root.path).unwrap().count(),
        0,
        "admission must not publish durable state"
    );
    client.stop();
    anchor.stop();
    let used = budget.work();
    let peak = budget.peak_storage();
    budget.release_storage(EXTRA).unwrap();
    assert_eq!(budget.storage(), 0);
    drop(budget);
    assert_eq!(work.failed_work(), None);
    let path = root.path.clone();
    drop(root);
    assert!(!path.exists(), "owned empty root must be removed");
    println!("FE2O3_NATIVE_ISSUER_PUBLIC_CASE_OK case={case} work={used} peak_storage={peak}");
}

#[test]
#[ignore = "explicit isolated root container and real musl-static test executable required"]
fn isolated_static_public_admission_matrix() {
    require_root();
    let measurements = current_static_issuer_measurements_v1()
        .expect("matrix requires a real sealed-static running executable");
    assert_ne!(measurements.sealed_static_identity(), [0; 32]);
    let deadline = Instant::now() + Duration::from_secs(570);
    for case in CASES {
        assert!(
            Instant::now() < deadline,
            "native admission matrix deadline exceeded"
        );
        let mut command = command(CASE_TEST);
        command.env(CASE_ENV, case);
        let mut child = ChildGuard(command.spawn().unwrap());
        drop(command);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let status = child
            .wait(CASE_TIMEOUT.min(remaining))
            .expect("bounded native admission case");
        assert!(
            status.success(),
            "public native admission case {case} failed: {status}"
        );
    }
    println!("FE2O3_NATIVE_ISSUER_PUBLIC_MATRIX_OK cases={}", CASES.len());
}

#[test]
#[ignore = "explicit isolated root container and real musl-static executable required"]
fn isolated_static_public_native_service_matrix() {
    require_root();
    current_static_issuer_measurements_v1().expect("real sealed-static executable required");
    for case in native_service::CASES {
        let mut cmd = command(CASE_TEST);
        cmd.env(CASE_ENV, case);
        let mut child = ChildGuard(cmd.spawn().unwrap());
        drop(cmd);
        assert!(child.wait(CASE_TIMEOUT).unwrap().success(), "{case}");
    }
    println!(
        "FE2O3_NATIVE_SERVICE_READINESS_MATRIX_OK cases={}",
        native_service::CASES.len()
    );
}
