//! Helper-role admission and legacy refusal using the private synthetic tree.
use super::*;

#[test]
fn each_helper_credential_must_be_separate_from_both_bound_profile_services() {
    let tree = Tree::new();
    let anchor = tree.profile.external_anchor_service();
    for (uid, gid, message) in [
        (
            tree.profile.supervisor_uid(),
            1002,
            "proof helper UID aliases a V3 profile service",
        ),
        (
            anchor.uid(),
            1002,
            "proof helper UID aliases a V3 profile service",
        ),
        (
            1002,
            tree.profile.supervisor_gid(),
            "proof helper GID aliases a V3 profile service",
        ),
        (
            1002,
            anchor.gid(),
            "proof helper GID aliases a V3 profile service",
        ),
    ] {
        let policy = CompilerApprovalPolicyV2::new(
            tree.policy.compiler_closure(),
            *tree.profile.identity().as_bytes(),
            *tree.policy.runtime_manifest_identity(),
            1,
            uid,
            gid,
            |_| Ok::<_, Resource>(()),
        )
        .unwrap();
        assert!(!policy.grants_authority());
        tree.write(POLICY_REL, policy.canonical_bytes());
        let (result, usage) = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b));
        assert!(matches!(failure(result), Error::Mismatch(actual) if actual == message));
        assert_eq!(usage.work, LOAD_WORK);
        assert_eq!(usage.live, FLOOR);
    }
    tree.write(POLICY_REL, tree.policy.canonical_bytes());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let owner = tree.retained(&mut budget);
    assert_eq!(owner.policy().proof_helper_uid(), 1002);
    assert_eq!(owner.policy().proof_helper_gid(), 1002);
    tree.revalidate(&owner, fixture_immutable, &mut budget)
        .unwrap();
}

#[test]
fn invalid_helper_credentials_in_root_policy_bytes_never_create_an_owner() {
    use fe2o3_build_authority::COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V2 as DOMAIN;
    use sha2::{Digest, Sha256};
    let tree = Tree::new();
    for (offset, expected) in [
        (20, CompilerApprovalPolicyErrorV2::InvalidProofHelperUid),
        (24, CompilerApprovalPolicyErrorV2::InvalidProofHelperGid),
    ] {
        for value in [0_u32, u32::MAX] {
            let mut bytes = *tree.policy.canonical_bytes();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            let mut hash = Sha256::new();
            hash.update(DOMAIN);
            hash.update(320_u64.to_le_bytes());
            hash.update(&bytes[..320]);
            bytes[320..].copy_from_slice(&hash.finalize());
            tree.write(POLICY_REL, &bytes);
            let (result, usage) = observe(LOAD_WORK, LIMIT, |b| tree.load(fixture_immutable, b));
            assert!(matches!(failure(result), Error::Codec(actual) if actual == expected));
            assert_eq!(usage.work, IO_WORK + POLICY_WORK);
            assert_eq!(usage.live, FLOOR);
        }
    }
}

#[test]
fn legacy_policy_path_and_v1_bytes_never_supply_production_approval() {
    use fe2o3_build_authority::{CompilerApprovalPolicyErrorV1, CompilerApprovalPolicyV1};
    let tree = Tree::new();
    let legacy = CompilerApprovalPolicyV1::new(
        tree.policy.compiler_closure(),
        *tree.profile.identity().as_bytes(),
        *tree.policy.runtime_manifest_identity(),
        1,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap();
    let old_path = "etc/fe2o3/build-authority/policy-v1";
    for bytes in [legacy.canonical_bytes(), tree.policy.canonical_bytes()] {
        tree.write(old_path, bytes);
        if tree.path(POLICY_REL).exists() {
            fs::remove_file(tree.path(POLICY_REL)).unwrap();
        }
        assert!(matches!(
            failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
            Error::Capability(CapabilityError::Io {
                errno: libc::ENOENT,
                ..
            })
        ));
    }
    tree.write(POLICY_REL, legacy.canonical_bytes());
    assert!(matches!(
        failure(observe(LIMIT, LIMIT, |b| tree.load(fixture_immutable, b)).0),
        Error::Codec(CompilerApprovalPolicyErrorV2::Framing(
            CompilerApprovalPolicyErrorV1::Header
        ))
    ));
}

#[test]
fn changing_only_helper_credentials_invalidates_the_retained_policy_origin() {
    for (uid, gid) in [(1003, 1002), (1002, 1003)] {
        let tree = Tree::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let owner = tree.retained(&mut budget);
        let changed = CompilerApprovalPolicyV2::new(
            tree.policy.compiler_closure(),
            *tree.profile.identity().as_bytes(),
            *tree.policy.runtime_manifest_identity(),
            1,
            uid,
            gid,
            |_| Ok::<_, Resource>(()),
        )
        .unwrap();
        assert_eq!(
            changed.client_profile_identity(),
            tree.policy.client_profile_identity()
        );
        assert_ne!(changed.identity(), tree.policy.identity());
        tree.write(POLICY_REL, changed.canonical_bytes());
        assert!(
            tree.revalidate(&owner, fixture_immutable, &mut budget)
                .is_err()
        );
    }
}
