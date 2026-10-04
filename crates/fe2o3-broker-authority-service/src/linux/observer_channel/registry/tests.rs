use super::*;
use crate::linux::observer_channel::tests::{
    ChildOwner, bytes_file, child_identity, inherited, policy, read_inherited,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

fn packet(kind: RegistryKind) -> RegistryPacket {
    RegistryPacket {
        kind,
        sequence: 1,
        nonce: [1; 32],
        registration: if matches!(
            kind,
            RegistryKind::Register | RegistryKind::RegisterApplication
        ) {
            [0; 32]
        } else {
            [2; 32]
        },
        body: vec![
            3;
            match kind {
                RegistryKind::Register => COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1,
                RegistryKind::RegisterApplication => WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1,
                RegistryKind::Registered
                | RegistryKind::RegisteredApplication
                | RegistryKind::AttachApplication => 32,
                RegistryKind::ApplicationInstalled => 64,
                RegistryKind::Bind | RegistryKind::Bound => 52,
            }
        ],
    }
}

#[test]
fn registration_codec_is_canonical_and_distinct_from_occurrence_protocol() {
    for kind in [
        RegistryKind::Register,
        RegistryKind::Registered,
        RegistryKind::Bind,
        RegistryKind::Bound,
        RegistryKind::RegisterApplication,
        RegistryKind::RegisteredApplication,
        RegistryKind::AttachApplication,
        RegistryKind::ApplicationInstalled,
    ] {
        let expected = packet(kind);
        let bytes = expected.encode().unwrap();
        assert_eq!(RegistryPacket::decode(&bytes).unwrap(), expected);
        assert!(Packet::decode(&bytes).is_err());
        assert!(
            fe2o3_runtime_protocol::WorkerV3ApplicationSessionMessageV1::decode(&bytes).is_err()
        );
        for length in 0..bytes.len() {
            assert!(RegistryPacket::decode(&bytes[..length]).is_err());
        }
        for offset in [0, 8, 9, 15] {
            let mut changed = bytes.clone();
            changed[offset] = 255;
            assert!(RegistryPacket::decode(&changed).is_err());
        }
        for (a, b) in [(16, 24), (24, 56)] {
            let mut changed = bytes.clone();
            changed[a..b].fill(0);
            assert!(RegistryPacket::decode(&changed).is_err());
        }
        let mut changed = bytes.clone();
        changed[56..88].fill(
            if matches!(
                kind,
                RegistryKind::Register | RegistryKind::RegisterApplication
            ) {
                1
            } else {
                0
            },
        );
        assert!(RegistryPacket::decode(&changed).is_err());
        let mut extra = bytes;
        extra.push(0);
        assert!(RegistryPacket::decode(&extra).is_err());
    }
}

#[test]
fn registration_enforces_exact_rights_and_sender_not_socket_creator() {
    let (a, b) = observer_pair().unwrap();
    let sender = Endpoint::admit(a).unwrap();
    let receiver = Endpoint::admit(b).unwrap();
    let actual = current_identity().unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let child = ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap());
    let other = child_identity(&child.0);
    for kind in [
        RegistryKind::Register,
        RegistryKind::Registered,
        RegistryKind::Bind,
        RegistryKind::Bound,
        RegistryKind::RegisterApplication,
        RegistryKind::RegisteredApplication,
        RegistryKind::AttachApplication,
        RegistryKind::ApplicationInstalled,
    ] {
        let packet = packet(kind);
        for count in 0..=2 {
            let rights = vec![actual.pidfd.as_fd(); count];
            assert!(
                sender
                    .send_bytes(&packet.encode().unwrap(), &rights)
                    .unwrap()
            );
            assert_eq!(
                RegistryPacket::receive(&receiver, &actual).is_ok(),
                count == packet.rights()
            );
        }
        assert!(
            packet
                .send(&sender, &vec![actual.pidfd.as_fd(); packet.rights()])
                .unwrap()
        );
        assert!(RegistryPacket::receive(&receiver, &other).is_err());
    }
}

#[test]
fn registered_owner_drop_does_not_shutdown_launched_alias() {
    let (a, b) = observer_pair().unwrap();
    let receiver = Endpoint::admit(b).unwrap();
    let root = current_identity().unwrap();
    let launch = CompilerExecutionServiceLaunchManifestV1::new(
        CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1000).unwrap(),
        CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap(),
        &policy(),
    );
    let registered = RegisteredCompilerObserverV1 {
        endpoint: Endpoint::admit(a).unwrap(),
        root,
        id: [1; 32],
        launch,
        registry: receiver.identity,
    };
    let [alias, _pidfd] = registered.try_clone_for_launch().unwrap();
    drop(registered);
    assert!(!receiver.closed().unwrap());
    let alias = Endpoint::admit(alias).unwrap();
    assert!(packet(RegistryKind::Bound).send(&alias, &[]).unwrap());
    assert!(
        RegistryPacket::receive(&receiver, &current_identity().unwrap())
            .unwrap()
            .is_some()
    );
}

#[test]
fn production_launch_requires_distinct_roles_and_exact_policy_anchor() {
    let credentials = ProtectedServiceCredentialProfileV1::new(61000, 61000).unwrap();
    let anchor = CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap();
    let policy = policy();
    for uid in [0, 61000, 61001, 1000] {
        let launch = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(1234, uid, 1000).unwrap(),
            anchor,
            &policy,
        );
        assert_eq!(
            validate_launch(&launch, &policy, credentials, anchor, true).is_ok(),
            uid == 1000
        );
        let other_anchor =
            CompilerExecutionExternalAnchorServiceIdentityV1::new(61002, 61002).unwrap();
        assert!(validate_launch(&launch, &policy, credentials, other_anchor, false).is_err());
    }
}

#[test]
#[ignore = "requires root and credential capabilities for real Cargo/application process fixtures"]
fn root_application_registry_transitions() {
    use crate::linux::application_observation::tests::{Fixture, build_helper};
    use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorHandoffV1;
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let build = tempfile::tempdir().unwrap();
    let helper = build_helper(build.path());
    for scenario in [
        "attached",
        "independent",
        "pending_marker",
        "pending_retired",
        "prepared_expiry",
    ] {
        let mut fixture = Fixture::spawn(&helper, true);
        let original = fixture.registration();
        let policy = policy();
        let anchor = CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap();
        let launch = CompilerExecutionServiceLaunchManifestV1::new(
            original.compiler_handoff().launch_manifest().client(),
            anchor,
            &policy,
        );
        let binding = WorkerV3ApplicationRegistrationBindingV1::new(
            CompilerExecutionSupervisorHandoffV1::new(
                original.compiler_handoff().submitter(),
                launch.clone(),
            )
            .unwrap(),
            original.occurrence().clone(),
            original.descriptors(),
            original.expectation(),
            original.challenge(),
        )
        .unwrap();
        let credentials = ProtectedServiceCredentialProfileV1::new(1000, 1000).unwrap();
        let (prepared, _transfers) = PreparedRootCompilerObserverRegistryV1::prepare_inner(
            policy,
            credentials,
            anchor,
            false,
        )
        .unwrap();
        let (parent_control, child_control) = observer_pair().unwrap();
        let supervisor = spawn_helper(
            "linux::observer_channel::registry::tests::application_registry_parent_helper",
            &[child_control],
            "application_parent",
            false,
        );
        let supervisor_identity = LiveClientPidfdIdentityV1::admit(
            rustix::process::pidfd_open(
                rustix::process::Pid::from_raw(supervisor.0.id() as i32).unwrap(),
                rustix::process::PidfdFlags::empty(),
            )
            .unwrap(),
            ExpectedClientProcessIdentityV1::new(supervisor.0.id(), 1000, 1000).unwrap(),
        )
        .unwrap();
        let parent_control = Endpoint::admit(parent_control).unwrap();
        let deadline = Instant::now() + TIMEOUT;
        let (issuer_bytes, mut issuer_rights) = loop {
            if let Some(message) = parent_control.receive_bytes(&supervisor_identity).unwrap() {
                break message;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(issuer_rights.len(), 1);
        let identity = LiveClientPidfdIdentityV1::admit(
            issuer_rights.remove(0),
            ExpectedClientProcessIdentityV1::new(
                u32::from_le_bytes(issuer_bytes.try_into().unwrap()),
                1000,
                1000,
            )
            .unwrap(),
        )
        .unwrap();
        let mut registry = RootCompilerObserverRegistryV1 {
            prepared,
            supervisor: supervisor_identity,
            sessions: Vec::new(),
            applications: Vec::new(),
            pending: None,
            next_sequence: 1,
            closing: false,
        };
        let compiler_peer = fixture.compiler_peer();
        let application = fixture.application_identity();
        let parent = fixture.parent_identity();
        let register = |kind, body| RegistryPacket {
            kind,
            body,
            ..packet(kind)
        };
        registry
            .register(
                register(
                    RegistryKind::RegisterApplication,
                    binding.canonical_bytes().to_vec(),
                ),
                vec![
                    rustix::io::fcntl_dupfd_cloexec(&compiler_peer, 0).unwrap(),
                    rustix::io::fcntl_dupfd_cloexec(&application.pidfd, 0).unwrap(),
                ],
            )
            .unwrap();
        assert!(registry.applications.is_empty());
        let registered = registry.pending.take().unwrap();
        assert_eq!(registered.packet.kind, RegistryKind::RegisteredApplication);
        assert_eq!(registered.packet.body, binding.identity().as_bytes());
        registry.validate_pending(&registered).unwrap();
        let id = registered.packet.registration;
        let mut bind_body = Vec::new();
        encode_identity(&identity, &mut bind_body);
        bind_body.extend_from_slice(launch.identity().as_bytes());
        let bind = RegistryPacket {
            registration: id,
            body: bind_body,
            ..packet(RegistryKind::Bind)
        };
        assert!(
            registry
                .bind_issuer(
                    bind.clone(),
                    vec![rustix::io::fcntl_dupfd_cloexec(&identity.pidfd, 0).unwrap()]
                )
                .is_err()
        );
        assert_eq!(registry.sessions.len(), 1);
        let attach = RegistryPacket {
            registration: id,
            body: binding.identity().as_bytes().to_vec(),
            ..packet(RegistryKind::AttachApplication)
        };
        for mutation in 0..4 {
            let mut changed = attach.clone();
            let rights = match mutation {
                0 => {
                    changed.registration = [99; 32];
                    vec![
                        fixture.proof_peer(),
                        rustix::io::fcntl_dupfd_cloexec(&parent.pidfd, 0).unwrap(),
                    ]
                }
                1 => {
                    changed.body[0] ^= 1;
                    vec![
                        fixture.proof_peer(),
                        rustix::io::fcntl_dupfd_cloexec(&parent.pidfd, 0).unwrap(),
                    ]
                }
                2 => vec![
                    rustix::io::fcntl_dupfd_cloexec(&parent.pidfd, 0).unwrap(),
                    fixture.proof_peer(),
                ],
                _ => vec![
                    fixture.proof_peer(),
                    rustix::io::fcntl_dupfd_cloexec(&application.pidfd, 0).unwrap(),
                ],
            };
            assert!(registry.attach_application(changed, rights).is_err());
            assert!(registry.applications.is_empty());
            assert!(registry.pending.is_none());
        }
        registry
            .attach_application(
                attach.clone(),
                vec![
                    fixture.proof_peer(),
                    rustix::io::fcntl_dupfd_cloexec(&parent.pidfd, 0).unwrap(),
                ],
            )
            .unwrap();
        assert_eq!(registry.applications.len(), 1);
        let installed = registry.pending.take().unwrap();
        registry.validate_pending(&installed).unwrap();
        assert_eq!(installed.packet.kind, RegistryKind::ApplicationInstalled);
        assert_eq!(installed.rights.len(), 1);
        let mut bytes = [0; 72];
        assert_eq!(
            rustix::io::read(&installed.rights[0], &mut bytes),
            Err(rustix::io::Errno::AGAIN)
        );
        assert!(
            registry
                .attach_application(
                    attach,
                    vec![
                        fixture.proof_peer(),
                        rustix::io::fcntl_dupfd_cloexec(&parent.pidfd, 0).unwrap()
                    ]
                )
                .is_err()
        );
        assert_eq!(registry.applications.len(), 1);
        match scenario {
            "independent" => {
                registry
                    .bind_issuer(
                        bind,
                        vec![rustix::io::fcntl_dupfd_cloexec(&identity.pidfd, 0).unwrap()],
                    )
                    .unwrap();
                assert!(matches!(registry.sessions[0], Session::Bound(_)));
                registry.sessions.clear();
                assert_eq!(registry.applications.len(), 1);
                registry.applications[0].revalidate_installation().unwrap();
                assert_eq!(
                    rustix::io::read(&installed.rights[0], &mut bytes),
                    Err(rustix::io::Errno::AGAIN),
                    "compiler binding cannot publish observation readiness"
                );
            }
            "pending_marker" => {
                if let Session::Prepared { application, .. } = &mut registry.sessions[0] {
                    *application = ApplicationAttachment::CompilerOnly;
                }
                assert!(registry.validate_pending(&installed).is_err());
            }
            "pending_retired" => {
                registry.applications[0].cancel();
                assert!(registry.validate_pending(&installed).is_err());
                assert!(
                    registry
                        .bind_issuer(
                            bind,
                            vec![rustix::io::fcntl_dupfd_cloexec(&identity.pidfd, 0).unwrap()]
                        )
                        .is_err()
                );
            }
            "prepared_expiry" => {
                if let Session::Prepared { deadline, .. } = &mut registry.sessions[0] {
                    *deadline = Instant::now();
                }
                assert!(
                    registry
                        .bind_issuer(
                            bind,
                            vec![rustix::io::fcntl_dupfd_cloexec(&identity.pidfd, 0).unwrap()]
                        )
                        .is_err()
                );
            }
            _ => {}
        }
        registry.cancel_all();
        assert!(!registry.is_drained());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !registry.is_drained() {
            assert!(Instant::now() < deadline);
            registry
                .step(|error| panic!("unexpected containment failure: {error}"))
                .unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            rustix::io::read(&installed.rights[0], &mut bytes).unwrap(),
            0
        );
        println!(
            "PASS: application registry {scenario}; cancellation drains original process custody"
        );
    }
}

fn spawn_helper(
    name: &str,
    descriptors: &[OwnedFd],
    scenario: &str,
    production: bool,
) -> ChildOwner {
    let sources: Vec<_> = descriptors
        .iter()
        .map(|fd| rustix::io::fcntl_dupfd_cloexec(fd, 240).unwrap())
        .collect();
    let raw: Vec<_> = sources.iter().map(AsRawFd::as_raw_fd).collect();
    let application_parent = scenario == "application_parent";
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            name,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_OBSERVER_TEST_SCENARIO", scenario)
        .stdin(Stdio::null());
    if production {
        command.env("FE2O3_REGISTRY_TEST_PRODUCTION", "1");
    }
    // SAFETY: only async-signal-safe credential and descriptor syscalls run in the owned child.
    unsafe {
        command.pre_exec(move || {
            if application_parent
                && (libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setresgid(1000, 1000, 1000) != 0
                    || libc::setresuid(1000, 1000, 1000) != 0)
            {
                return Err(io::Error::last_os_error());
            }
            if production && libc::geteuid() == 0 {
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
                    || libc::setgroups(0, std::ptr::null()) != 0
                    || libc::setresgid(61000, 61000, 61000) != 0
                    || libc::setresuid(61000, 61000, 61000) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                let header = [0x2008_0522_u32, 0];
                let data = [0_u32; 6];
                if libc::syscall(libc::SYS_capset, header.as_ptr(), data.as_ptr()) != 0 {
                    return Err(io::Error::last_os_error());
                }
                let limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                    || libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                libc::umask(0o077);
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
    ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap())
}

#[test]
#[ignore = "private application-registry supervisor with original issuer pidfd"]
fn application_registry_parent_helper() {
    let endpoint = Endpoint::admit(inherited(200)).unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let child = ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap());
    let identity = child_identity(&child.0);
    assert!(
        endpoint
            .send_bytes(&child.0.id().to_le_bytes(), &[identity.pidfd.as_fd()])
            .unwrap()
    );
    let deadline = Instant::now() + TIMEOUT;
    while !endpoint.closed().unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
#[ignore = "requires root and credential capabilities for authenticated registry transport"]
fn root_application_registry_transport() {
    use crate::linux::application_observation::tests::{Fixture, build_helper};
    use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorHandoffV1;
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let build = tempfile::tempdir().unwrap();
    let helper = build_helper(build.path());
    let mut fixture = Fixture::spawn(&helper, true);
    let original = fixture.registration();
    let policy = policy();
    let anchor = CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap();
    let launch = CompilerExecutionServiceLaunchManifestV1::new(
        original.compiler_handoff().launch_manifest().client(),
        anchor,
        &policy,
    );
    let binding = WorkerV3ApplicationRegistrationBindingV1::new(
        CompilerExecutionSupervisorHandoffV1::new(original.compiler_handoff().submitter(), launch)
            .unwrap(),
        original.occurrence().clone(),
        original.descriptors(),
        original.expectation(),
        original.challenge(),
    )
    .unwrap();
    let credentials = ProtectedServiceCredentialProfileV1::new(1000, 1000).unwrap();
    let (prepared, [peer, root_pidfd]) = PreparedRootCompilerObserverRegistryV1::prepare_inner(
        policy.clone(),
        credentials,
        anchor,
        false,
    )
    .unwrap();
    let application = fixture.application_identity();
    let parent = fixture.parent_identity();
    let descriptors = [
        peer,
        root_pidfd,
        bytes_file(policy.canonical_bytes()),
        bytes_file(binding.canonical_bytes()),
        fixture.compiler_peer(),
        rustix::io::fcntl_dupfd_cloexec(&application.pidfd, 0).unwrap(),
        fixture.proof_peer(),
        rustix::io::fcntl_dupfd_cloexec(&parent.pidfd, 0).unwrap(),
    ];
    let mut supervisor = spawn_helper(
        "linux::observer_channel::registry::tests::application_registry_wire_helper",
        &descriptors,
        "application_parent",
        false,
    );
    drop(descriptors);
    let supervisor_identity = LiveClientPidfdIdentityV1::admit(
        rustix::process::pidfd_open(
            rustix::process::Pid::from_raw(supervisor.0.id() as i32).unwrap(),
            rustix::process::PidfdFlags::empty(),
        )
        .unwrap(),
        ExpectedClientProcessIdentityV1::new(supervisor.0.id(), 1000, 1000).unwrap(),
    )
    .unwrap();
    let mut registry = prepared.bind(supervisor_identity).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut installed = false;
    loop {
        let result = registry.step(|error| panic!("unexpected application failure: {error}"));
        installed |= registry.applications.len() == 1;
        if let Err(error) = result {
            assert!(
                matches!(
                    error,
                    CompilerExecutionObserverErrorV1::Closed
                        | CompilerExecutionObserverErrorV1::Admission(_)
                        | CompilerExecutionObserverErrorV1::Profile(_)
                ),
                "unexpected registry error: {error}"
            );
        }
        if let Some(status) = supervisor.0.try_wait().unwrap() {
            assert!(status.success());
            registry.cancel_all();
            if registry.is_drained() {
                break;
            }
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(installed);
    println!(
        "PASS: root/supervisor two-exchange application API; installation alone cannot complete readiness"
    );
}

#[test]
#[ignore = "private application registry wire supervisor with original application descriptors"]
fn application_registry_wire_helper() {
    let peer = inherited(200);
    let root = inherited(201);
    let policy = CompilerExecutionIssuerPolicyV1::decode(&read_inherited(202)).unwrap();
    let binding = WorkerV3ApplicationRegistrationBindingV1::decode(&read_inherited(203)).unwrap();
    let compiler = inherited(204);
    let application = inherited(205);
    let proof = inherited(206);
    let parent = inherited(207);
    let mut registry = SupervisorCompilerObserverRegistryV1::admit_inner(
        peer,
        root,
        &policy,
        ProtectedServiceCredentialProfileV1::new(1000, 1000).unwrap(),
        binding
            .compiler_handoff()
            .launch_manifest()
            .external_anchor_service(),
        false,
    )
    .unwrap();
    let registered = registry
        .register_application(
            &binding,
            compiler.as_fd(),
            application.as_fd(),
            proof.as_fd(),
            parent.as_fd(),
            Instant::now() + TIMEOUT,
        )
        .unwrap();
    assert_eq!(registered.binding(), &binding);
    assert!(matches!(
        registered.await_observation(Instant::now() + Duration::from_millis(20)),
        Err(CompilerExecutionObserverErrorV1::Timeout)
    ));
}

/// Actual root/supervisor/issuer process hierarchy and observed compiler publication, with
/// test-only same-UID profiles and test keys. Does not qualify measured production launch.
pub(crate) fn exercise_registry(
    client: RetainedCompilerClientSessionV1,
    service: &ProtectedServiceAdmissionV1,
    scenario: &str,
    mut while_active: impl FnMut(),
) {
    exercise_registry_inner(client, service, scenario, &mut while_active, false);
}

pub(crate) fn exercise_production_registry(
    client: RetainedCompilerClientSessionV1,
    service: &ProtectedServiceAdmissionV1,
    mut while_active: impl FnMut(),
) {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    exercise_registry_inner(client, service, "success", &mut while_active, true);
}

fn exercise_registry_inner(
    client: RetainedCompilerClientSessionV1,
    service: &ProtectedServiceAdmissionV1,
    scenario: &str,
    while_active: &mut impl FnMut(),
    production: bool,
) {
    let policy = policy();
    let expected = client.client();
    let anchor = CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap();
    let launch = CompilerExecutionServiceLaunchManifestV1::new(
        CompilerExecutionClientProcessIdentityV1::new(expected.pid, expected.uid, expected.gid)
            .unwrap(),
        anchor,
        &policy,
    );
    let credentials = if production {
        ProtectedServiceCredentialProfileV1::new(61000, 61000).unwrap()
    } else {
        ProtectedServiceCredentialProfileV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap()
    };
    let (prepared, [peer, root_pidfd]) = PreparedRootCompilerObserverRegistryV1::prepare_inner(
        policy.clone(),
        credentials,
        anchor,
        production,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let service_root = if production {
        let root = std::fs::File::open(directory.path()).unwrap();
        rustix::fs::fchmod(&root, rustix::fs::Mode::from_raw_mode(0o700)).unwrap();
        rustix::fs::fchown(
            &root,
            Some(rustix::process::Uid::from_raw(61000)),
            Some(rustix::process::Gid::from_raw(61000)),
        )
        .unwrap();
        root.into()
    } else {
        service.try_clone_service_root().unwrap()
    };
    let mut descriptors = vec![
        peer,
        root_pidfd,
        service_root,
        rustix::io::fcntl_dupfd_cloexec(service.service_peer(), 0).unwrap(),
        rustix::io::fcntl_dupfd_cloexec(service.client_pidfd(), 0).unwrap(),
        bytes_file(policy.canonical_bytes()),
        bytes_file(launch.canonical_bytes()),
    ];
    let (profile_reader, profile_writer) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    if production {
        descriptors.push(profile_writer);
    }
    let mut supervisor = spawn_helper(
        "linux::observer_channel::registry::tests::registry_supervisor_helper",
        &descriptors,
        scenario,
        production,
    );
    drop(descriptors);
    if production {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let mut ready = [0];
            match rustix::io::read(&profile_reader, &mut ready) {
                Ok(1) if ready == *b"1" => break,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
                other => panic!("supervisor failed profile admission: {other:?}"),
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let supervisor_identity = if production {
        let pid = rustix::process::Pid::from_raw(supervisor.0.id() as i32).unwrap();
        LiveClientPidfdIdentityV1::admit(
            rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).unwrap(),
            ExpectedClientProcessIdentityV1::new(supervisor.0.id(), 61000, 61000).unwrap(),
        )
        .unwrap()
    } else {
        child_identity(&supervisor.0)
    };
    let mut registry = prepared.bind(supervisor_identity).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut active = false;
    let mut rejected = false;
    let mut expired = false;
    loop {
        let result = registry.step(|error| panic!("unexpected session failure: {error}"));
        match result {
            Err(CompilerExecutionObserverErrorV1::Protocol(
                "observer registration capacity exhausted",
            )) if scenario == "capacity" => rejected = true,
            Err(CompilerExecutionObserverErrorV1::Protocol(
                "pending registration was retired before delivery",
            )) if scenario == "expiry" => rejected = true,
            Err(CompilerExecutionObserverErrorV1::Admission(_)) if scenario == "wrong_pidfd" => {
                rejected = true
            }
            Err(CompilerExecutionObserverErrorV1::Protocol(
                "registered issuer reused or start identity changed",
            )) if scenario == "rebind" => rejected = true,
            Err(CompilerExecutionObserverErrorV1::Closed) if scenario == "registry_loss" => {
                rejected = true
            }
            // Graceful supervisor exit can race any procfs continuity read. Its eventual
            // successful exit is still required below; early containment makes that fail.
            Err(
                CompilerExecutionObserverErrorV1::Closed
                | CompilerExecutionObserverErrorV1::Admission(_)
                | CompilerExecutionObserverErrorV1::Profile(_),
            ) if matches!(scenario, "success" | "concurrent") => {}
            Err(error) => panic!("unexpected registration error in {scenario}: {error}"),
            _ => {}
        }
        if scenario == "expiry" && !expired && registry.pending.is_some() {
            for session in &mut registry.sessions {
                if let Session::Prepared { deadline, .. } = session {
                    *deadline = Instant::now();
                }
            }
            expired = true;
        }
        if registry.sessions.iter().any(|session|
            matches!(session, Session::Bound(observer) if observer.has_active_occurrence()))
        {
            active = true;
            while_active();
        }
        if supervisor.0.try_wait().unwrap().is_some() {
            registry.cancel_all();
            if registry.is_drained() {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "registry test exceeded deadline: {scenario}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(supervisor.0.wait().unwrap().success());
    if matches!(scenario, "success" | "concurrent") {
        assert!(active);
    } else {
        assert!(rejected, "negative scenario was not rejected: {scenario}");
    }
    if scenario == "registry_loss" {
        assert!(active);
    }
}

#[test]
#[ignore = "private registry supervisor helper with original compiler descriptors"]
fn registry_supervisor_helper() {
    let production = restore_test_profile();
    let peer = inherited(200);
    let root_pidfd = inherited(201);
    let service_root = inherited(202);
    let client_peer = inherited(203);
    let client_pidfd = inherited(204);
    let policy = CompilerExecutionIssuerPolicyV1::decode(&read_inherited(205)).unwrap();
    let launch = CompilerExecutionServiceLaunchManifestV1::decode(&read_inherited(206)).unwrap();
    let credentials = ProtectedServiceCredentialProfileV1::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    let mut registry = SupervisorCompilerObserverRegistryV1::admit_inner(
        peer,
        root_pidfd,
        &policy,
        credentials,
        launch.external_anchor_service(),
        production,
    )
    .unwrap();
    if production {
        let ready = inherited(207);
        assert_eq!(rustix::io::write(&ready, b"1").unwrap(), 1);
    }
    let scenario = std::env::var("FE2O3_OBSERVER_TEST_SCENARIO").unwrap();
    if scenario == "capacity" {
        let mut retained = Vec::new();
        for _ in 0..MAX_SESSIONS {
            retained.push(
                registry
                    .register(&launch, client_peer.as_fd(), client_pidfd.as_fd())
                    .unwrap(),
            );
        }
        assert!(
            registry
                .register(&launch, client_peer.as_fd(), client_pidfd.as_fd())
                .is_err()
        );
        return;
    }
    let registered = registry.register(&launch, client_peer.as_fd(), client_pidfd.as_fd());
    if scenario == "expiry" {
        assert!(registered.is_err());
        return;
    }
    let mut registered = Some(registered.unwrap());
    let mut idle = None;
    for index in 0..if scenario == "concurrent" { 2 } else { 1 } {
        let registered = registered.take().unwrap_or_else(|| {
            registry
                .register(&launch, client_peer.as_fd(), client_pidfd.as_fd())
                .unwrap()
        });
        let [observer, root] = registered.try_clone_for_launch().unwrap();
        let mut descriptors = vec![
            observer,
            root,
            rustix::io::fcntl_dupfd_cloexec(&service_root, 0).unwrap(),
            rustix::io::fcntl_dupfd_cloexec(&client_peer, 0).unwrap(),
            rustix::io::fcntl_dupfd_cloexec(&client_pidfd, 0).unwrap(),
            bytes_file(policy.canonical_bytes()),
            bytes_file(launch.canonical_bytes()),
        ];
        let (holding_reader, holding_writer) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        if scenario == "registry_loss" {
            descriptors.push(holding_writer);
        }
        let issuer_scenario = if (scenario == "concurrent" && index == 0)
            || matches!(scenario.as_str(), "rebind" | "wrong_pidfd")
        {
            "idle"
        } else if scenario == "registry_loss" {
            "holding"
        } else {
            "success"
        };
        let mut issuer = spawn_helper(
            "linux::observer_channel::issuer::tests::issuer_helper",
            &descriptors,
            issuer_scenario,
            production,
        );
        drop(descriptors);
        let identity = child_identity(&issuer.0);
        if scenario == "wrong_pidfd" {
            let mut body = Vec::new();
            encode_identity(&identity, &mut body);
            body.extend_from_slice(launch.identity().as_bytes());
            let wrong = current_identity().unwrap();
            assert!(
                registry
                    .exchange(
                        RegistryKind::Bind,
                        registered.id,
                        body,
                        &[wrong.pidfd.as_fd()],
                        Instant::now() + TIMEOUT
                    )
                    .is_err()
            );
            return;
        }
        registry
            .bind_issuer(
                &registered,
                issuer.0.id(),
                identity.pidfd.as_fd(),
                Instant::now() + TIMEOUT,
            )
            .unwrap();
        if scenario == "rebind" {
            assert!(
                registry
                    .bind_issuer(
                        &registered,
                        issuer.0.id(),
                        identity.pidfd.as_fd(),
                        Instant::now() + TIMEOUT
                    )
                    .is_err()
            );
            return;
        }
        drop(registered);
        if scenario == "registry_loss" {
            let mut ready = [0];
            assert_eq!(rustix::io::read(&holding_reader, &mut ready).unwrap(), 1);
            assert_eq!(ready, *b"1");
            drop(registry);
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(issuer.0.wait().unwrap().signal(), Some(libc::SIGKILL));
            return;
        }
        if issuer_scenario == "idle" {
            idle = Some(issuer);
        } else {
            assert!(issuer.0.wait().unwrap().success());
        }
    }
    drop(idle);
}

pub(crate) fn restore_test_profile() -> bool {
    let production = std::env::var("FE2O3_REGISTRY_TEST_PRODUCTION").as_deref() == Ok("1");
    if production {
        // SAFETY: restore the exec-reset dumpability bit in this owned test subprocess only.
        assert_eq!(unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) }, 0);
        validate_current_protected_service_profile_v1(
            ProtectedServiceCredentialProfileV1::new(61000, 61000).unwrap(),
        )
        .unwrap();
    }
    production
}
