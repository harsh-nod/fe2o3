#[test]
fn each_release_family_requires_its_exact_child_entry() {
    for family in [ReleaseFamily::LegacyV3, ReleaseFamily::NativeV4] {
        let mut record = contract();
        record.family = family;
        for entry in [INTERNAL_CHILD_ARG, INTERNAL_CHILD_ARG_V4] {
            record.argv[1] = entry.as_bytes().to_vec();
            assert_eq!(record.encode().is_ok(), entry == family.child_arg());
            assert_eq!(
                validate_fields(family, &record.argv, &record.environment).is_ok(),
                entry == family.child_arg()
            );
        }
    }
}

#[test]
fn native_release_v4_preserves_roster_and_refuses_cross_family_readers() {
    let legacy = contract();
    let old_bytes = legacy.encode().unwrap();
    let mut native = legacy.clone();
    native.family = ReleaseFamily::NativeV4;
    native.argv[1] = INTERNAL_CHILD_ARG_V4.as_bytes().to_vec();
    let bytes = native.encode().unwrap();
    assert_eq!(&bytes[..8], b"F2AURL4\0");
    assert_eq!(&bytes[8..10], &4_u16.to_le_bytes());
    assert_eq!(&bytes[16..24], &[0; 8]);
    assert_eq!(native.descriptors, legacy.descriptors);
    assert_eq!(CLIENT_PROFILE_FD, 203);
    assert_eq!(
        [CONTRACT_FD, CONTROL_FD, LAUNCHER_IMAGE_FD, CWD_FD],
        [187, 188, 189, 190]
    );
    let (decoded, identity) = ReleaseContract::decode_for(&bytes, ReleaseFamily::NativeV4).unwrap();
    assert_eq!(decoded, native);
    assert_eq!(decoded.encode().unwrap(), bytes);
    assert_eq!(
        identity,
        domain_hash(
            b"FE2O3/PROTECTED-AUTHORITY-RELEASE-CONTRACT/V4\0",
            &[&bytes[..bytes.len() - 32]]
        )
    );
    assert!(ReleaseContract::decode(&bytes).is_err());
    assert!(ReleaseContract::decode_for(&old_bytes, ReleaseFamily::NativeV4).is_err());
    assert_eq!(ReleaseContract::decode(&old_bytes).unwrap().0, legacy);
}

#[test]
fn native_release_rehashed_unknown_version_and_reserved_bytes_reject() {
    let mut native = contract();
    native.family = ReleaseFamily::NativeV4;
    native.argv[1] = INTERNAL_CHILD_ARG_V4.as_bytes().to_vec();
    let bytes = native.encode().unwrap();
    for offset in [0, 7, 8, 9, 10, 12, 16, 23] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        let end = changed.len() - 32;
        let digest = domain_hash(native.family.contract_domain(), &[&changed[..end]]);
        changed[end..].copy_from_slice(&digest);
        assert!(
            ReleaseContract::decode_for(&changed, ReleaseFamily::NativeV4).is_err(),
            "offset {offset}"
        );
    }
    for offset in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        assert!(
            ReleaseContract::decode_for(&changed, ReleaseFamily::NativeV4).is_err(),
            "byte {offset}"
        );
    }
}

#[test]
fn native_release_exact_profile_and_child_family_bind_all_handshake_stages() {
    let legacy = contract();
    let mut native = legacy.clone();
    native.family = ReleaseFamily::NativeV4;
    native.argv[1] = INTERNAL_CHILD_ARG_V4.as_bytes().to_vec();
    let bytes = native.encode().unwrap();
    let (_, identity) = ReleaseContract::decode_for(&bytes, native.family).unwrap();
    let grant = grant_identity(&native, &identity, 789, 987);
    assert_ne!(grant, grant_identity(&legacy, &identity, 789, 987));
    assert_ne!(
        accept_identity(&native.attempt, &grant),
        accept_identity_for(native.family, &native.attempt, &grant)
    );
    assert_eq!(native.family.ready_magic(), b"F2AURDY4");
    assert_eq!(native.family.grant_magic(), b"F2AUGRT4");
    assert_eq!(native.family.accept_magic(), b"F2AUACC4");
    assert!(validate_child_observation(&native, &observation(&native)).is_ok());
    let mut wrong_profile = observation(&native);
    wrong_profile.compiler_execution_profile_identity[0] ^= 1;
    assert!(validate_child_observation(&native, &wrong_profile).is_err());
    let mut old_entry = observation(&native);
    old_entry.argv[1] = INTERNAL_CHILD_ARG.as_bytes().to_vec();
    assert!(validate_child_observation(&native, &old_entry).is_err());
}

#[test]
fn profile_family_is_generation_semantic_input_even_for_equal_identity_bytes() {
    let mut legacy = Vec::new();
    let mut native = Vec::new();
    CompilerExecutionProfileIdentity::V1([7; 32]).append_semantic_configuration(&mut legacy);
    CompilerExecutionProfileIdentity::V3([7; 32]).append_semantic_configuration(&mut native);
    let mut expected = b"fe2o3-compiler-execution-client-profile-v1\0".to_vec();
    expected.extend_from_slice(&[7; 32]);
    assert_eq!(legacy, expected);
    assert_ne!(native, legacy);
}
