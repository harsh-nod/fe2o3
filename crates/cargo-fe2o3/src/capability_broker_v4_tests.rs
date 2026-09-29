use crate::authority_release::profile::tests as profile_fixtures;

fn native_test_closure() -> fe2o3_build_authority::CompilerClosureV2 {
    fe2o3_build_authority::CompilerClosureV2::new(
        [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32],
    )
    .unwrap()
}

fn native_test_binding(identity: [u8; 32]) -> CapabilityBindingV3 {
    CapabilityBindingV3::new_protected_v4(
        CapabilityProfileV1::Ordinary,
        Some([7; 32]),
        native_test_closure(),
        [8; 32],
        identity,
    )
    .unwrap()
}

fn native_test_route(binding: CapabilityBindingV3) -> BrokerRouteV3 {
    BrokerRouteV3 {
        endpoint: "12".repeat(32),
        secret: [9; 32],
        binding,
        peer: BrokerPeerIdentityV2 {
            uid: 1000,
            pid: 123,
            start_time_ticks: 456,
            device: 7,
            inode: 8,
            mode: 0o100500,
            executable_sha256: [10; 32],
        },
    }
}

fn native_test_roster(profile: File) -> Vec<OwnedFd> {
    let full = rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::SEAL;
    let backend = profile_fixtures::image(b"synthetic broker image; no compiler authority", full);
    let backend = File::open(format!("/proc/self/fd/{}", backend.as_raw_fd())).unwrap();
    let artifact = File::from(
        rustix::fs::open(
            "/",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    let closure = CompilerClosureCapabilityV1::create(native_test_closure())
        .unwrap()
        .try_clone_for_transfer()
        .unwrap();
    vec![
        backend.into(),
        artifact.into(),
        closure.into(),
        profile.into(),
    ]
}

#[test]
fn native_v4_route_exact_readers_and_fields_preserve_legacy_shape() {
    let binding = native_test_binding([11; 32]);
    let native = native_test_route(binding);
    let encoded = native.encode();
    assert_eq!(encoded.split(':').count(), 18);
    assert!(encoded.starts_with("fe2o3-capability-route-v4:"));
    assert!(encoded.ends_with(&format!(":profile-v3:{}", hex(&[11; 32]))));
    assert_eq!(BrokerRouteV3::parse_v4(&encoded).unwrap(), native);
    assert!(BrokerRouteV3::parse(&encoded).is_err());
    let legacy = native_test_route(CapabilityBindingV3 {
        native_profile_identity: None,
        ..binding
    });
    let old = legacy.encode();
    assert_eq!(old.split(':').count(), 16);
    assert!(old.starts_with("fe2o3-capability-route-v3:"));
    assert_eq!(BrokerRouteV3::parse(&old).unwrap(), legacy);
    assert!(BrokerRouteV3::parse_v4(&old).is_err());
    for (index, replacement) in [
        (0, "fe2o3-capability-route-v5"),
        (5, "-"),
        (16, "profile-v1"),
        (16, "profile-v2"),
        (16, "profile-v4"),
        (17, "00"),
    ] {
        let mut fields = encoded.split(':').collect::<Vec<_>>();
        fields[index] = replacement;
        assert!(BrokerRouteV3::parse_v4(&fields.join(":")).is_err());
    }
    assert!(BrokerRouteV3::parse_v4(&format!("{encoded}:extra")).is_err());
    assert!(
        CapabilityBindingV3::new_protected_v4(
            CapabilityProfileV1::Ordinary,
            None,
            native_test_closure(),
            [8; 32],
            [11; 32]
        )
        .is_err()
    );
}

#[test]
fn native_v4_request_authentication_covers_family_profile_config_and_session() {
    let binding = native_test_binding([11; 32]);
    let old = CapabilityBindingV3 {
        native_profile_identity: None,
        ..binding
    };
    let session = BuildSession::from_bytes([12; 16]);
    let challenge = [13; 32];
    let secret = [14; 32];
    let native_request = request_bytes(session, binding, challenge, &secret);
    let old_request = request_bytes(session, old, challenge, &secret);
    assert_eq!(native_request.len(), REQUEST_BYTES + 33);
    assert_eq!(old_request.len(), REQUEST_BYTES);
    assert_eq!(&native_request[..REQUEST_MAGIC_V4.len()], REQUEST_MAGIC_V4);
    let family_offset = REQUEST_BYTES - 2 * 32;
    assert_eq!(native_request[family_offset], 3);
    assert_eq!(
        &native_request[family_offset + 1..family_offset + 33],
        &[11; 32]
    );
    let (_, native_auth) =
        authenticate_request(&native_request, session, binding, &secret).unwrap();
    assert!(authenticate_request(&old_request, session, binding, &secret).is_err());
    assert!(authenticate_request(&native_request, session, old, &secret).is_err());
    assert!(authenticate_request(&old_request, session, old, &secret).is_ok());
    for changed in [
        CapabilityBindingV3 {
            native_profile_identity: Some([15; 32]),
            ..binding
        },
        CapabilityBindingV3 {
            config_identity: Some([16; 32]),
            ..binding
        },
        CapabilityBindingV3 {
            retained_object_binding_sha256: [17; 32],
            ..binding
        },
    ] {
        let request = request_bytes(session, changed, challenge, &secret);
        assert!(authenticate_request(&request, session, binding, &secret).is_err());
    }
    for tag in [0, 1, 2, 4, 255] {
        let mut changed = native_request.clone();
        changed[family_offset] = tag;
        let auth_offset = changed.len() - REQUEST_AUTH_BYTES;
        let auth = keyed_digest(REQUEST_AUTH_DOMAIN_V4, &secret, &[&changed[..auth_offset]]);
        changed[auth_offset..].copy_from_slice(&auth);
        assert!(authenticate_request(&changed, session, binding, &secret).is_err());
    }
    assert_ne!(
        response_bytes_for(binding, &secret, challenge, native_auth),
        response_bytes(&secret, challenge, native_auth)
    );
    assert!(
        authenticate_request(
            &native_request,
            BuildSession::from_bytes([18; 16]),
            binding,
            &secret
        )
        .is_err()
    );
}

#[test]
fn legacy_request_record_remains_byte_for_byte_frozen() {
    let binding = CapabilityBindingV3::new_protected(
        CapabilityProfileV1::Ordinary,
        Some([7; 32]),
        native_test_closure(),
        [8; 32],
    )
    .unwrap();
    let session = BuildSession::from_bytes([12; 16]);
    let mut expected = b"FE2O3-CARGO-CAPABILITY-BROKER-V3\0".to_vec();
    expected.extend_from_slice(session.as_bytes());
    expected.push(1);
    expected.extend_from_slice(&[7; 32]);
    expected.push(1);
    expected.extend_from_slice(&native_test_closure().identity_sha256());
    expected.extend_from_slice(&[4; 32]);
    expected.extend_from_slice(&[8; 32]);
    expected.extend_from_slice(&[13; 32]);
    let auth = keyed_digest(
        b"FE2O3/CAPABILITY-BROKER/REQUEST-AUTH/V3\0",
        &[14; 32],
        &[&expected],
    );
    expected.extend_from_slice(&auth);
    assert_eq!(
        request_bytes(session, binding, [13; 32], &[14; 32]),
        expected
    );
    assert_eq!(binding.descriptor_count(), 4);
}

#[test]
fn native_v4_descriptor_decode_requires_exact_profile_object_and_identity() {
    let source = profile_fixtures::native_profile(7);
    let identity = *source.profile().identity().as_bytes();
    let binding = native_test_binding(identity);
    let roster = || {
        native_test_roster(
            source
                .try_clone_for_transfer()
                .unwrap()
                .file()
                .try_clone()
                .unwrap(),
        )
    };
    let account = || {
        let account = profile_fixtures::account(100_000_000, 16_000_000);
        account.lock().unwrap().with_budget(|budget| budget.reserve_storage(
            6 * fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3::FILE_STORAGE
        ).unwrap());
        account
    };
    let decoded =
        decode_received_descriptors_with_account(roster(), binding, Some(account())).unwrap();
    assert!(decoded.compiler_execution_profile.is_none());
    assert_eq!(
        decoded
            .compiler_execution_profile_v3
            .as_ref()
            .unwrap()
            .profile()
            .identity()
            .as_bytes(),
        &identity
    );
    // Structural decode by itself grants no authenticated route or invocation.
    assert!(decoded.authenticated_client_profile_v3_identity().is_none());
    assert!(decoded.invocation_authority.is_none());
    assert!(decode_received_descriptors(roster(), binding).is_err());
    let old = CapabilityBindingV3 {
        native_profile_identity: None,
        ..binding
    };
    assert!(decode_received_descriptors(roster(), old).is_err());
    let wrong = native_test_binding([19; 32]);
    assert!(decode_received_descriptors_with_account(roster(), wrong, Some(account())).is_err());
    let copied_old = native_test_roster(profile_fixtures::legacy_profile_file(7));
    assert!(
        decode_received_descriptors_with_account(copied_old, binding, Some(account())).is_err()
    );
    let old_roster = native_test_roster(profile_fixtures::legacy_profile_file(7));
    let legacy = decode_received_descriptors(old_roster, old).unwrap();
    assert!(legacy.compiler_execution_profile.is_some());
    assert!(legacy.compiler_execution_profile_v3.is_none());
    let mut missing = roster();
    missing.pop();
    assert!(decode_received_descriptors_with_account(missing, binding, Some(account())).is_err());
    let mut extra = roster();
    extra.push(extra[0].try_clone().unwrap());
    assert!(decode_received_descriptors_with_account(extra, binding, Some(account())).is_err());
    let mut duplicate = roster();
    duplicate[0] = duplicate[3].try_clone().unwrap();
    assert!(decode_received_descriptors_with_account(duplicate, binding, Some(account())).is_err());
    let mutable = profile_fixtures::image(
        source.profile().canonical_bytes(),
        rustix::fs::SealFlags::empty(),
    );
    assert!(
        decode_received_descriptors_with_account(
            native_test_roster(mutable),
            binding,
            Some(account())
        )
        .is_err()
    );
}

fn open_aliases(file: &File) -> usize {
    let expected = file.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| fs::metadata(entry.path()).ok())
        .filter(|metadata| (metadata.dev(), metadata.ino()) == (expected.dev(), expected.ino()))
        .count()
}

#[test]
fn native_v4_response_failure_owns_and_closes_all_disclosed_fds() {
    let binding = native_test_binding([11; 32]);
    let payload = profile_fixtures::legacy_profile_file(7);
    let before = open_aliases(&payload);
    for (count, wrong_auth) in [(0, false), (3, false), (4, true), (5, false)] {
        let (sender, receiver) = UnixStream::pair().unwrap();
        let mut response = response_bytes_for(binding, &[14; 32], [13; 32], [12; 32]);
        if wrong_auth {
            response[1] ^= 1;
        }
        let descriptors = vec![payload.as_fd(); count];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if count != 0 {
            assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
        }
        assert_eq!(
            sendmsg(
                &sender,
                &[IoSlice::new(&response)],
                &mut ancillary,
                SendFlags::NOSIGNAL
            )
            .unwrap(),
            RESPONSE_BYTES
        );
        assert!(receive_response(&receiver, binding, &[14; 32], [13; 32], [12; 32]).is_err());
        assert_eq!(open_aliases(&payload), before);
    }
}

#[test]
fn native_invocation_stream_and_original_account_survive_profile_transfer() {
    let account = profile_fixtures::account(100_000_000, 16_000_000);
    account.lock().unwrap().with_budget(|budget| budget.reserve_storage(
        6 * fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV3::FILE_STORAGE
    ).unwrap());
    let (stream, mut peer) = UnixStream::pair().unwrap();
    let authority = BrokeredInvocationAuthorityV1::from_authenticated_stream_with_account(
        stream,
        Some(Arc::clone(&account)),
    )
    .unwrap();
    assert!(Arc::ptr_eq(
        authority.profile_account.as_ref().unwrap(),
        &account
    ));
    peer.set_read_timeout(Some(Duration::from_millis(10)))
        .unwrap();
    let mut byte = [0];
    let err = peer.read(&mut byte).unwrap_err();
    assert!(matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    drop(authority);
    assert_eq!(peer.read(&mut byte).unwrap(), 0);
}
