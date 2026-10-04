use super::*;
use fe2o3_runtime_protocol::*;

struct OwnedChild(std::process::Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().unwrap();
    }
}

struct ApplicationFixture {
    _lock: MutexGuard<'static, ()>,
    child: OwnedChild,
    binding: WorkerV3ApplicationRegistrationBindingV1,
    rights: [OwnedFd; 4],
    _app_proof: OwnedFd,
}

fn application_fixture(
    supervisor: &ProtectedIssuerSupervisorV1,
    image: &Fixture,
) -> ApplicationFixture {
    let (lock, child, launch) = live_launch();
    let child = OwnedChild(child);
    let handoff = CompilerExecutionSupervisorHandoffV1::new(
        launch.submitter(),
        CompilerExecutionServiceLaunchManifestV1::new(
            launch.client(),
            supervisor.external_anchor_service(),
            supervisor.policy(),
        ),
    )
    .unwrap();
    let (compiler, app_pidfd) = launch.into_test_descriptors();
    let (app_proof, proof) = proof_pair();
    let stat = rustix::fs::fstat(&app_proof).unwrap();
    let occurrence = WorkerV3ApplicationOccurrenceV1::new(
        WorkerV3ApplicationIdentityV1::from_sealed_static_elf_v1(&image.bytes).unwrap(),
        [10; 32],
        &[
            WorkerV3ApplicationInputOccurrenceV1::new(1, [1; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(2, [2; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::new(3, [3; 32]).unwrap(),
            WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
                4,
                stat.st_dev,
                stat.st_ino,
                stat.st_mode,
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"transport fixture only").unwrap(),
        &occurrence,
    );
    let binding = WorkerV3ApplicationRegistrationBindingV1::new(
        handoff,
        occurrence,
        WorkerV3ApplicationRegistrationDescriptorsV1::new(210, 211, 212, 213).unwrap(),
        expectation,
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([11; 32]).unwrap(),
    )
    .unwrap();
    let cargo_pidfd = rustix::process::pidfd_open(
        rustix::process::getpid(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    ApplicationFixture {
        _lock: lock,
        child,
        binding,
        rights: [compiler, app_pidfd, proof, cargo_pidfd],
        _app_proof: app_proof,
    }
}

fn proof_pair() -> (OwnedFd, OwnedFd) {
    let pair = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    for fd in [&pair.0, &pair.1] {
        rustix::net::sockopt::set_socket_passcred(fd, true).unwrap();
        bind(fd, &rustix::net::SocketAddrUnix::new_unnamed()).unwrap();
    }
    pair
}

fn send(control: &OwnedFd, payload: &[u8], rights: &[std::os::fd::BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(6))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        sendmsg(
            control,
            &[IoSlice::new(payload)],
            &mut ancillary,
            SendFlags::NOSIGNAL
        )
        .unwrap(),
        payload.len()
    );
}

#[test]
fn application_handoff_admits_parent_and_child_then_detects_child_exit() {
    let image = Fixture::new("application-handoff");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let mut fixture = application_fixture(&supervisor, &image);
    let (sender, receiver) = seqpacket_pair();
    send(
        &sender,
        fixture.binding.canonical_bytes(),
        &fixture.rights.each_ref().map(AsFd::as_fd),
    );
    let accepted = supervisor
        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    assert_eq!(accepted.binding(), &fixture.binding);
    accepted.revalidate(&supervisor).unwrap();
    fixture.child.0.kill().unwrap();
    fixture.child.0.wait().unwrap();
    assert!(accepted.revalidate(&supervisor).is_err());
}

#[test]
fn application_handoff_rejects_wrong_count_swapped_and_aliased_roles() {
    let image = Fixture::new("application-roles");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    for count in [0, 1, 2, 3, 5, 6] {
        let (sender, receiver) = seqpacket_pair();
        let candidates = [
            rights[0], rights[1], rights[2], rights[3], rights[0], rights[1],
        ];
        send(
            &sender,
            fixture.binding.canonical_bytes(),
            &candidates[..count],
        );
        assert!(
            matches!(
                supervisor
                    .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
                Err(ProtectedApplicationHandoffErrorV1::Handoff(
                    ProtectedIssuerHandoffErrorV1::MalformedTransfer
                ))
            ),
            "rights {count}"
        );
    }
    for left in 0..4 {
        for right in 0..4 {
            if left == right {
                continue;
            }
            for swap in [false, true] {
                let mut changed = rights;
                if swap {
                    changed.swap(left, right);
                } else {
                    changed[left] = changed[right];
                }
                let (sender, receiver) = seqpacket_pair();
                send(&sender, fixture.binding.canonical_bytes(), &changed);
                assert!(
                    supervisor
                        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
                        .is_err(),
                    "left {left} right {right} swap {swap}"
                );
            }
        }
    }
}

#[test]
fn compiler_and_application_profiles_cannot_be_interchanged() {
    let image = Fixture::new("application-profiles");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(matches!(
        supervisor.accept_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedIssuerHandoffErrorV1::MalformedTransfer)
    ));
    let (sender, receiver) = seqpacket_pair();
    send(
        &sender,
        fixture.binding.compiler_handoff().canonical_bytes(),
        &rights[..2],
    );
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::MalformedTransfer
        ))
    ));
}

#[test]
fn application_handoff_rejects_corruption_truncation_and_unclaimed_proof_flags() {
    let image = Fixture::new("application-corruption");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let rights = fixture.rights.each_ref().map(AsFd::as_fd);
    for length in [1, 839, 841] {
        let (sender, receiver) = seqpacket_pair();
        let mut bytes = fixture.binding.canonical_bytes().to_vec();
        bytes.resize(length, 0);
        send(&sender, &bytes, &rights);
        assert!(
            supervisor
                .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1))
                .is_err()
        );
    }
    for offset in [0, 24, 208, 512, 528, 724, 808, 839] {
        let (sender, receiver) = seqpacket_pair();
        let mut bytes = *fixture.binding.canonical_bytes();
        bytes[offset] ^= 1;
        send(&sender, &bytes, &rights);
        assert!(matches!(
            supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
            Err(ProtectedApplicationHandoffErrorV1::Binding(_))
        ));
    }
    let (sender, receiver) = seqpacket_pair();
    rustix::net::sockopt::set_socket_passcred(&fixture.rights[2], false).unwrap();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::ProofPeer(_))
    ));
}

#[test]
fn application_handoff_revalidates_original_proof_flags_and_parent_relation() {
    let image = Fixture::new("application-continuity");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let (sender, receiver) = seqpacket_pair();
    send(
        &sender,
        fixture.binding.canonical_bytes(),
        &fixture.rights.each_ref().map(AsFd::as_fd),
    );
    let accepted = supervisor
        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    rustix::fs::fcntl_setfl(&fixture.rights[2], OFlags::empty()).unwrap();
    assert!(accepted.revalidate(&supervisor).is_err());
}

#[test]
fn application_handoff_timeout_and_same_uid_production_admission_fail_closed() {
    let image = Fixture::new("application-timeout");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let (_sender, receiver) = seqpacket_pair();
    assert!(matches!(
        supervisor.accept_application_handoff(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::ClientAndSupervisorUidMatch
        ))
    ));
    let (_sender, receiver) = seqpacket_pair();
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_millis(10)),
        Err(ProtectedApplicationHandoffErrorV1::Handoff(
            ProtectedIssuerHandoffErrorV1::Timeout
        ))
    ));
}

#[test]
fn same_cargo_foreign_pair_remains_provisional_until_root_observation() {
    let image = Fixture::new("application-foreign-pair");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let (_foreign_app, foreign_peer) = proof_pair();
    let mut rights = fixture.rights.each_ref().map(AsFd::as_fd);
    rights[2] = foreign_peer.as_fd();
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    // Only root can inspect slot 4 and prove the reverse-address association. This owner
    // deliberately has no launch/readiness API, and production dispatch cannot consume it.
    let provisional = supervisor
        .accept_application_handoff_inner::<false>(receiver, Duration::from_secs(2))
        .unwrap();
    provisional.revalidate(&supervisor).unwrap();
}

#[test]
fn application_side_endpoint_is_not_admitted_as_its_counterpart() {
    let image = Fixture::new("application-wrong-side");
    let Some(supervisor) = bound_supervisor(&image) else {
        return;
    };
    let fixture = application_fixture(&supervisor, &image);
    let mut rights = fixture.rights.each_ref().map(AsFd::as_fd);
    rights[2] = fixture._app_proof.as_fd();
    let (sender, receiver) = seqpacket_pair();
    send(&sender, fixture.binding.canonical_bytes(), &rights);
    assert!(matches!(
        supervisor.accept_application_handoff_inner::<false>(receiver, Duration::from_secs(1)),
        Err(ProtectedApplicationHandoffErrorV1::ProofPeer(
            "received app endpoint instead of counterpart"
        ))
    ));
}
