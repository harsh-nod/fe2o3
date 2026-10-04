//! Isolated CPU composition of production APIs, with test keys/publication and no GPU authority.
use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV1, SupervisorCompilerObserverRegistryV1,
};
use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV1;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1, COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1, CompilerExecutionClientProfileV1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerPolicyV1,
    sealed_static_issuer_runtime_measurement_v1,
};
use fe2o3_compiler_execution_supervisor::{
    AdmittedIssuerProgramV1, ProtectedIssuerLaunchPreparationErrorV1, ProtectedIssuerServiceV1,
    ProtectedIssuerServiceWorkerCountV1, ProtectedIssuerSessionErrorV1,
    ProtectedIssuerSessionOutcomeV1, ProtectedIssuerSessionTimeoutsV1,
    ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1, ProtectedIssuerTerminationV1,
    ProvisionedStaticExecutableMeasurementV1,
};
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV1;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};

const SERVICE_UID: u32 = 61000;
const SERVICE_GID: u32 = 1000;
const ANCHOR_UID: u32 = 61001;
const ANCHOR_GID: u32 = 61001;
const CLIENT_UID: u32 = 1000;
const TIMEOUT: Duration = Duration::from_secs(45);

struct ChildOwner {
    child: Child,
    status: Option<ExitStatus>,
}

impl ChildOwner {
    fn poll(&mut self) -> Option<ExitStatus> {
        if self.status.is_none() {
            self.status = self.child.try_wait().unwrap();
        }
        self.status
    }

    fn identity(&self, uid: u32, gid: u32) -> LiveClientPidfdIdentityV1 {
        // The direct child is still unreaped, so this first capture cannot target a reused PID.
        LiveClientPidfdIdentityV1::admit(
            rustix::process::pidfd_open(
                rustix::process::Pid::from_raw(self.child.id() as i32).unwrap(),
                rustix::process::PidfdFlags::empty(),
            )
            .unwrap(),
            ExpectedClientProcessIdentityV1::new(self.child.id(), uid, gid).unwrap(),
        )
        .unwrap()
    }
}

impl Drop for ChildOwner {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn environment_path(name: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("missing {name}")));
    assert!(path.is_absolute(), "{name} must be absolute");
    path
}

fn measurement(path: &Path) -> CompilerExecutionIssuerMeasurementV1 {
    let bytes = fs::read(path).unwrap();
    CompilerExecutionIssuerMeasurementV1::new(Sha256::digest(&bytes).into(), bytes.len() as u64)
        .unwrap()
}

fn directory(path: &Path, uid: u32, gid: u32, mode: u32) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    rustix::fs::chown(
        path,
        Some(rustix::process::Uid::from_raw(uid)),
        Some(rustix::process::Gid::from_raw(gid)),
    )
    .unwrap();
}

fn inherited(fd: i32) -> OwnedFd {
    // SAFETY: only this exact ignored helper claims the fixed private test descriptor table.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    rustix::io::fcntl_setfd(&owned, rustix::io::FdFlags::CLOEXEC).unwrap();
    owned
}

struct PrivateNamespaces([File; 2]);

fn file_identity(file: &File) -> (u64, u64) {
    let stat = rustix::fs::fstat(file).unwrap();
    (stat.st_dev, stat.st_ino)
}

impl PrivateNamespaces {
    fn revalidate(&self) {
        for (retained, path) in self
            .0
            .iter()
            .zip(["/proc/self/ns/mnt", "/proc/self/ns/pid"])
        {
            assert_eq!(
                file_identity(retained),
                file_identity(&File::open(path).unwrap())
            );
        }
    }
}

fn require_private_namespaces() -> PrivateNamespaces {
    let current = [
        File::open("/proc/self/ns/mnt").unwrap(),
        File::open("/proc/self/ns/pid").unwrap(),
    ];
    // The controller retains its original namespaces across bwrap. Do not trust an env marker.
    for ((path, namespace_type), current) in [
        ("/run/fe2o3-test-ns/mnt", libc::CLONE_NEWNS),
        ("/run/fe2o3-test-ns/pid", libc::CLONE_NEWPID),
    ]
    .into_iter()
    .zip(&current)
    {
        let original = File::open(path).unwrap();
        assert_eq!(
            rustix::fs::fstatfs(&original).unwrap().f_type,
            libc::NSFS_MAGIC
        );
        // SAFETY: NS_GET_NSTYPE takes no pointer argument and only inspects the open descriptor.
        let actual = unsafe { libc::ioctl(original.as_raw_fd(), 0xb703) };
        assert_eq!(actual, namespace_type, "invalid original namespace: {path}");
        assert_ne!(
            file_identity(&original),
            file_identity(current),
            "requires a private namespace: {path}"
        );
    }
    for path in ["/proc/self/ns/pid_for_children", "/proc/1/ns/pid"] {
        assert_eq!(
            file_identity(&current[1]),
            file_identity(&File::open(path).unwrap())
        );
    }
    for (path, mode) in [("/etc", 0o755), ("/run", 0o755), ("/tmp", 0o1777)] {
        let directory = File::open(path).unwrap();
        let stat = rustix::fs::fstatfs(&directory).unwrap();
        assert_eq!(
            stat.f_type,
            libc::TMPFS_MAGIC,
            "requires private tmpfs: {path}"
        );
        let stat = rustix::fs::fstat(&directory).unwrap();
        assert_eq!(
            (stat.st_uid, stat.st_gid, stat.st_mode & 0o7777),
            (0, 0, mode)
        );
    }
    PrivateNamespaces(current)
}

fn spawn(mut command: Command, descriptors: Vec<OwnedFd>, service: bool) -> ChildOwner {
    let sources: Vec<_> = descriptors
        .iter()
        .map(|fd| rustix::io::fcntl_dupfd_cloexec(fd, 240).unwrap())
        .collect();
    let raw: Vec<_> = sources.iter().map(AsRawFd::as_raw_fd).collect();
    let parent = std::process::id();
    command.stdin(Stdio::null());
    // SAFETY: only async-signal-safe descriptor, credential and profile syscalls in the child.
    unsafe {
        command.pre_exec(move || {
            if service {
                for capability in 0..=63 {
                    if libc::prctl(libc::PR_CAPBSET_DROP, capability, 0, 0, 0) != 0
                        && io::Error::last_os_error().raw_os_error() != Some(libc::EINVAL)
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                if libc::prctl(
                    libc::PR_SET_SECUREBITS,
                    fe2o3_protected_service_profile::PROTECTED_SERVICE_SECUREBITS_V1,
                    0,
                    0,
                    0,
                ) != 0
                    || libc::prctl(
                        libc::PR_CAP_AMBIENT,
                        libc::PR_CAP_AMBIENT_CLEAR_ALL,
                        0,
                        0,
                        0,
                    ) != 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            let (uid, gid) = if service {
                (SERVICE_UID, SERVICE_GID)
            } else {
                (CLIENT_UID, CLIENT_UID)
            };
            if libc::setgroups(0, std::ptr::null()) != 0
                || libc::setresgid(gid, gid, gid) != 0
                || libc::setresuid(uid, uid, uid) != 0
            {
                return Err(io::Error::last_os_error());
            }
            if service {
                let header = [0x2008_0522_u32, 0];
                let data = [0_u32; 6];
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::syscall(libc::SYS_capset, header.as_ptr(), data.as_ptr()) != 0
                    || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                    || libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                libc::umask(0o077);
            }
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() as u32 != parent
            {
                return Err(io::Error::last_os_error());
            }
            for (index, source) in raw.iter().enumerate() {
                let destination = 200 + index as i32;
                if libc::dup2(*source, destination) != destination
                    || libc::fcntl(destination, libc::F_SETFD, 0) != 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let child = command.spawn().unwrap();
    ChildOwner {
        child,
        status: None,
    }
}

#[test]
#[ignore = "requires disposable root namespaces and prebuilt static service/application images"]
fn root_composed_production_application_startup() {
    assert_eq!(
        std::env::var("FE2O3_COMPOSED_NAMESPACE_V1").as_deref(),
        Ok("1")
    );
    assert!(rustix::process::geteuid().is_root());
    assert!(rustix::process::getegid().is_root());
    let namespaces = require_private_namespaces();
    let launcher = environment_path("FE2O3_STATIC_PREEXEC_LAUNCHER");
    let issuer = environment_path("FE2O3_STATIC_COMPILER_EXECUTION_ISSUER");
    let helper = environment_path("FE2O3_ROOT_ANCHOR_HELPER");
    let daemon = environment_path("FE2O3_ROOT_ANCHOR_DAEMON");
    let cargo_test = environment_path("FE2O3_COMPOSED_CARGO_TEST");
    let application = environment_path("FE2O3_COMPOSED_STATIC_CONSUMER");
    let images = [
        measurement(&std::env::current_exe().unwrap()),
        measurement(&launcher),
        measurement(&issuer),
        measurement(&helper),
        measurement(&daemon),
    ];
    for (index, image) in images.iter().enumerate() {
        for other in &images[index + 1..] {
            assert_ne!(image, other, "executable roles must be distinct");
        }
    }
    let root = tempfile::tempdir().unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let state = root.path().join("anchor");
    directory(&state, ANCHOR_UID, ANCHOR_GID, 0o700);
    let lifecycle_path = root.path().join(
        Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
            .file_name()
            .unwrap(),
    );
    fs::write(&lifecycle_path, []).unwrap();
    fs::set_permissions(&lifecycle_path, fs::Permissions::from_mode(0o400)).unwrap();
    let mut issuer_seed = [0x31; 32];
    let mut anchor_seed = [0x37; 32];
    let policy = CompilerExecutionIssuerPolicyV1::new(
        1,
        measurement(&issuer),
        sealed_static_issuer_runtime_measurement_v1(),
        SigningKey::from_bytes(&issuer_seed)
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&anchor_seed)
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    let anchor_service =
        CompilerExecutionExternalAnchorServiceIdentityV1::new(ANCHOR_UID, ANCHOR_GID).unwrap();
    let deployment = CompilerExecutionSupervisorDeploymentV1::new(
        SERVICE_UID,
        SERVICE_GID,
        anchor_service,
        measurement(&std::env::current_exe().unwrap()),
        measurement(&launcher),
        &policy,
    )
    .unwrap();
    let anchor_deployment = CompilerExecutionExternalAnchorDeploymentV1::new(
        &deployment,
        &policy,
        measurement(&daemon),
    )
    .unwrap();
    let provisioning = CompilerExecutionExternalAnchorProvisioningV1::new(
        &anchor_deployment,
        measurement(&helper),
    )
    .unwrap();
    let anchor_key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
        &mut anchor_seed,
        &anchor_deployment,
    )
    .unwrap();
    let signing =
        CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut issuer_seed, &policy)
            .unwrap();
    let policy_capability = CompilerExecutionPolicyCapabilityV1::create(policy.clone()).unwrap();
    let deployment_capability =
        CompilerExecutionSupervisorDeploymentCapabilityV1::create(deployment.clone()).unwrap();
    let anchor = PreparedExternalAnchorOccurrenceV1::prepare(
        File::open(helper).unwrap(),
        File::open(daemon).unwrap(),
        File::open(&state).unwrap(),
        CompilerExecutionServiceLifecycleLeaseV1::open(&File::open(&state).unwrap()).unwrap(),
        anchor_deployment,
        provisioning,
        anchor_key,
    )
    .unwrap()
    .launch(Duration::from_secs(30))
    .unwrap();

    // Every fixed path must be absent in the disposable mount namespace. Never overwrite it.
    directory(Path::new("/etc/fe2o3"), 0, 0, 0o755);
    directory(Path::new("/etc/fe2o3/compiler-execution"), 0, 0, 0o755);
    directory(Path::new("/run/fe2o3"), 0, 0, 0o755);
    let profile = CompilerExecutionClientProfileV1::new(
        SERVICE_UID,
        SERVICE_GID,
        anchor_service,
        policy.clone(),
    )
    .unwrap();
    fs::write(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        profile.canonical_bytes(),
    )
    .unwrap();
    fs::set_permissions(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
        fs::Permissions::from_mode(0o444),
    )
    .unwrap();

    for case in [
        "descriptor",
        "roster",
        "delayed",
        "fallback",
        "cancelled",
        "custodian-publication",
    ] {
        println!("BEGIN composed startup {case}");
        let case_root = root.path().join(case);
        directory(&case_root, SERVICE_UID, SERVICE_GID, 0o700);
        let client_root = root.path().join(format!("{case}-client"));
        directory(&client_root, CLIENT_UID, CLIENT_UID, 0o700);
        let listener = rustix::net::socket_with(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        rustix::net::bind(
            &listener,
            &rustix::net::SocketAddrUnix::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1)
                .unwrap(),
        )
        .unwrap();
        rustix::fs::chown(
            COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
            Some(rustix::process::Uid::ROOT),
            Some(rustix::process::Gid::from_raw(SERVICE_GID)),
        )
        .unwrap();
        fs::set_permissions(
            COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
            fs::Permissions::from_mode(0o660),
        )
        .unwrap();
        let credentials = IssuerServiceCredentialProfileV1::new(SERVICE_UID, SERVICE_GID).unwrap();
        let lifecycle =
            CompilerExecutionServiceLifecycleLeaseV1::open(&File::open(&case_root).unwrap())
                .unwrap();
        let (prepared, [registry_peer, root_pidfd]) =
            PreparedRootCompilerObserverRegistryV1::prepare(
                policy.clone(),
                credentials,
                anchor_service,
            )
            .unwrap();
        let (anchor_peer, anchor_pidfd) = anchor
            .try_clone_for_supervisor(&deployment, &policy)
            .unwrap()
            .into_ordered_descriptors();
        let (ready_read, ready_write) =
            pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        let descriptors = vec![
            listener,
            File::open(&case_root).unwrap().into(),
            File::open(&launcher).unwrap().into(),
            File::open(&issuer).unwrap().into(),
            policy_capability.try_clone_for_transfer().unwrap().into(),
            signing.try_clone_for_transfer().unwrap().into(),
            deployment_capability
                .try_clone_for_transfer()
                .unwrap()
                .into(),
            anchor_peer,
            anchor_pidfd,
            registry_peer,
            root_pidfd,
            rustix::io::fcntl_dupfd_cloexec(&lifecycle, 0).unwrap(),
            ready_write,
        ];
        let mut command = helper_command(&std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "composed_startup::service_helper",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FE2O3_COMPOSED_CASE", case);
        let mut service = spawn(command, descriptors, true);
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match rustix::io::read(&ready_read, &mut [0_u8; 1]) {
                Ok(1) => break,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
                other => panic!("service startup failed: {other:?}"),
            }
            assert!(service.poll().is_none());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut registry = prepared
            .bind(service.identity(SERVICE_UID, SERVICE_GID))
            .unwrap();
        let mut command = helper_command(&cargo_test);
        command
            .args([
                "--exact",
                "composed_production_application_startup_helper",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FE2O3_COMPOSED_CASE", case)
            .env("FE2O3_COMPOSED_STATIC_CONSUMER", &application)
            .env("HOME", &client_root)
            .env("TMPDIR", &client_root);
        if let Some(worker) = std::env::var_os("FE2O3_TEST_HSACO_WORKER_FIXTURE") {
            command.env("FE2O3_TEST_HSACO_WORKER_FIXTURE", worker);
        }
        let mut client = spawn(command, Vec::new(), false);
        let mut first_progress = false;
        let mut delayed_steps = 0;
        let mut closing = false;
        let mut custodian_extracted = false;
        loop {
            let progress = registry.step(|error| {
                println!("{case}: root session retired: {error}");
                let expected = match error {
                    CompilerExecutionObserverErrorV1::Closed => true,
                    CompilerExecutionObserverErrorV1::Admission(error) => error.kind()
                        == fe2o3_broker_authority_service::AdmissionErrorKindV1::ClientAlreadyDead,
                    _ => false,
                };
                assert!(expected, "unexpected root session failure");
            });
            match progress {
                Ok(true) => {
                    if case == "delayed" {
                        delayed_steps += 1;
                        std::thread::sleep(Duration::from_millis(200));
                    } else if case == "cancelled" && !first_progress {
                        registry.cancel_all();
                        closing = true;
                    }
                    first_progress = true;
                }
                Ok(_) => {}
                Err(
                    CompilerExecutionObserverErrorV1::Closed
                    | CompilerExecutionObserverErrorV1::Admission(_),
                ) => {
                    closing = true;
                }
                Err(error) => panic!("{case}: root registry failed: {error}"),
            }
            if let Some(handoff) = registry.take_published_application_custodian() {
                assert_eq!(case, "custodian-publication");
                assert!(!custodian_extracted, "duplicate custodian publication");
                handoff.revalidate().unwrap();
                assert_eq!(
                    handoff.transcript().binding(),
                    *handoff.binding().identity().as_bytes()
                );
                assert!(registry.take_published_application_custodian().is_none());
                custodian_extracted = true;
                // This routing fixture deliberately contains the original app before Ready.
                // It does not install a manager or claim proof/current-record admission.
                drop(handoff);
            }
            let client_done = client.poll();
            let service_done = service.poll();
            if let Some(status) = client_done {
                assert!(status.success(), "{case}: Cargo helper failed");
            }
            if let Some(status) = service_done {
                assert!(status.success(), "{case}: service helper failed");
            }
            if service_done.is_some() && !closing {
                registry.cancel_all();
                closing = true;
            }
            if client_done.is_some() && service_done.is_some() && registry.is_drained() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{case}: composed startup timed out"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(first_progress);
        assert_eq!(custodian_extracted, case == "custodian-publication");
        if case == "delayed" {
            assert!(
                delayed_steps > 2,
                "registration transitions were not delayed"
            );
        }
        lifecycle.revalidate().unwrap();
        anchor.validate_continuity().unwrap();
        namespaces.revalidate();
        fs::remove_file(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1).unwrap();
        println!(
            "PASS composed startup {case}: Cargo/app handshake and issuer reaping; registry drained"
        );
    }
    anchor.shutdown().unwrap();
    namespaces.revalidate();
    fs::remove_file(COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1).unwrap();
    fs::remove_dir("/etc/fe2o3/compiler-execution").unwrap();
    fs::remove_dir("/etc/fe2o3").unwrap();
    fs::remove_dir("/run/fe2o3").unwrap();
}

#[test]
#[ignore = "private exact-profile supervisor helper for composed startup"]
fn service_helper() {
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable).unwrap();
    let listener = inherited(200);
    let state = File::from(inherited(201));
    let launcher = File::from(inherited(202));
    let issuer = File::from(inherited(203));
    let policy =
        CompilerExecutionPolicyCapabilityV1::from_file(File::from(inherited(204))).unwrap();
    let key_template = File::from(inherited(205));
    let deployment =
        CompilerExecutionSupervisorDeploymentCapabilityV1::from_file(File::from(inherited(206)))
            .unwrap();
    let anchor_peer = inherited(207);
    let anchor_pidfd = inherited(208);
    let registry_peer = inherited(209);
    let root_pidfd = inherited(210);
    let lifecycle =
        CompilerExecutionServiceLifecycleLeaseV1::admit(File::from(inherited(211)), &state)
            .unwrap();
    let ready = inherited(212);
    let manifest = deployment.deployment();
    let credentials = IssuerServiceCredentialProfileV1::new(SERVICE_UID, SERVICE_GID).unwrap();
    fe2o3_protected_service_profile::validate_current_protected_service_profile_v1(credentials)
        .unwrap();
    if std::env::var("FE2O3_COMPOSED_CASE").as_deref() == Ok("fallback") {
        let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
        let mut filter = [
            instruction(0x20, 0, 0, 4),
            instruction(0x15, 1, 0, 0xc000_003e),
            instruction(0x06, 0, 0, 0x8000_0000),
            instruction(0x20, 0, 0, 0),
            instruction(0x15, 0, 1, libc::SYS_clone3 as u32),
            instruction(0x06, 0, 0, 0x0005_0000 | libc::ENOSYS as u32),
            instruction(0x06, 0, 0, 0x7fff_0000),
        ];
        let program = libc::sock_fprog {
            len: filter.len() as u16,
            filter: filter.as_mut_ptr(),
        };
        // SAFETY: this disposable helper only tightens its inherited syscall policy.
        assert_eq!(
            unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program) },
            0
        );
    }
    let key = CompilerExecutionSigningKeyCapabilityV1::reissue_root_template_for_current_service(
        key_template,
        manifest,
        policy.policy(),
    )
    .unwrap();
    let program = AdmittedIssuerProgramV1::provision(
        launcher,
        ProvisionedStaticExecutableMeasurementV1::new(
            manifest.launcher().sha256(),
            manifest.launcher().byte_len(),
        )
        .unwrap(),
        issuer,
        policy,
    )
    .unwrap();
    let external = ProtectedExternalAnchorServiceAdmissionV1::admit(
        anchor_peer,
        anchor_pidfd,
        manifest.external_anchor_service(),
    )
    .unwrap();
    let supervisor =
        ProtectedIssuerSupervisorV1::bind(program, credentials, state, key, external).unwrap();
    let registry = SupervisorCompilerObserverRegistryV1::admit(
        registry_peer,
        root_pidfd,
        supervisor.policy(),
        credentials,
        manifest.external_anchor_service(),
    )
    .unwrap();
    let supervisor = supervisor.with_observer_registry(registry).unwrap();
    let boundary = Duration::from_secs(30);
    let timeouts = ProtectedIssuerSessionTimeoutsV1::new(
        boundary,
        boundary,
        boundary,
        boundary,
        Duration::from_secs(300),
    )
    .unwrap();
    let service = ProtectedIssuerServiceV1::bind(supervisor, listener, timeouts).unwrap();
    let stop = service.shutdown_handle();
    assert_eq!(rustix::io::write(&ready, b"R").unwrap(), 1);
    drop(ready);
    let report = service
        .run(
            ProtectedIssuerServiceWorkerCountV1::new(1).unwrap(),
            |outcome| {
                match &outcome {
                    ProtectedIssuerSessionOutcomeV1::Completed(exited) => {
                        // This startup-only app closes FD195 without sending a current-record audit.
                        assert_eq!(
                            exited.termination(),
                            ProtectedIssuerTerminationV1::Exited { status: 1 }
                        );
                        println!(
                            "issuer reaped: pid={} termination={:?}",
                            exited.pid(),
                            exited.termination()
                        )
                    }
                    ProtectedIssuerSessionOutcomeV1::Rejected(error) => {
                        assert!(matches!(
                            error,
                            ProtectedIssuerSessionErrorV1::Preparation(
                                ProtectedIssuerLaunchPreparationErrorV1::Supervisor(
                                    ProtectedIssuerSupervisorErrorV1::Observer(_)
                                )
                            )
                        ));
                        println!("issuer rejected: {error}")
                    }
                    _ => panic!("unexpected outcome"),
                }
                stop.request();
            },
        )
        .unwrap();
    lifecycle.revalidate().unwrap();
    if std::env::var("FE2O3_COMPOSED_CASE").as_deref() == Ok("cancelled") {
        assert_eq!(report.completed(), 0);
        assert_eq!(report.rejected(), 1);
    } else {
        assert_eq!(report.completed(), 1);
        assert_eq!(report.rejected(), 0);
    }
}

fn helper_command(path: &Path) -> Command {
    let mut command = Command::new(path);
    command.env_clear().env("PATH", "/usr/bin:/bin");
    if let Some(path) = std::env::var_os("LD_LIBRARY_PATH") {
        command.env("LD_LIBRARY_PATH", path);
    }
    command
}
