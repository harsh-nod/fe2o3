//! Real filesystem/sealed-copy tests using inert bytes, not runnable deployments.
use super::*;
use crate::{tests::Fixture, *};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    panic::{AssertUnwindSafe, catch_unwind},
};

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TARGET: &str = COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1;

#[test]
fn manifest_bytes_match_independently_pinned_v1_and_v3_goldens() {
    // Synthetic lengths/digests test only the canonical grammar and exact roster.
    // These are not installable bundles or content/authority endorsements.
    for (profile, golden) in [
        (
            Profile::V1,
            include_bytes!("../tests/fixtures/install-manifest-v1.txt").as_slice(),
        ),
        (
            Profile::V3,
            include_bytes!("../tests/fixtures/install-manifest-v3.txt").as_slice(),
        ),
    ] {
        let entries: Vec<_> = profile
            .files()
            .iter()
            .map(|&spec| ManifestEntryV1 {
                spec,
                byte_len: 17,
                sha256: [0; 32],
            })
            .collect();
        assert_eq!(
            serialize_manifest(profile, COMMIT, TARGET, &entries),
            golden
        );
        assert_eq!(
            parse_manifest(profile, golden, COMMIT).unwrap().entries,
            entries
        );
    }
}

fn owner() -> (u32, u32) {
    (
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
}
fn parent() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn native() -> (Fixture, CompilerExecutionManifestGenerationV1) {
    let fixture = Fixture::for_profile(Profile::V3);
    let report =
        generate_compiler_execution_install_manifest_v3(fixture.root.path(), COMMIT, TARGET)
            .unwrap();
    (fixture, report)
}
fn verify(fixture: &Fixture, digest: [u8; 32]) -> VerifiedCompilerExecutionDeploymentV3 {
    verify_compiler_execution_deployment_v3(fixture.root.path(), &lower_hex(&digest), COMMIT)
        .unwrap()
}

#[test]
fn versioned_inventory_and_manifest_refuse_both_crossings() {
    for profile in [Profile::V1, Profile::V3] {
        let fixture = Fixture::for_profile(profile);
        let report = generate_manifest(profile, fixture.root.path(), COMMIT, TARGET).unwrap();
        let bytes = fs::read(fixture.root.path().join(profile.manifest().source)).unwrap();
        assert_eq!(report.byte_len(), bytes.len() as u64);
        let parsed = parse_manifest(profile, &bytes, COMMIT).unwrap();
        assert_eq!(parsed.entries.len(), profile.files().len());
        assert_eq!(
            serialize_manifest(profile, COMMIT, TARGET, &parsed.entries),
            bytes
        );
        let other = if profile == Profile::V1 {
            Profile::V3
        } else {
            Profile::V1
        };
        assert_eq!(
            parse_manifest(other, &bytes, COMMIT).err().unwrap().kind(),
            DeploymentVerificationErrorKindV1::InvalidManifest
        );
        assert!(
            verify_deployment(
                other,
                fixture.root.path(),
                &lower_hex(&report.sha256()),
                COMMIT
            )
            .is_err()
        );
        let sources: Vec<_> = profile.files().iter().map(|s| s.source).collect();
        let mut sorted = sources.clone();
        sorted.sort_unstable();
        assert_eq!(sources, sorted);
    }
    let (fixture, report) = native();
    assert_eq!(verify(&fixture, report.sha256()).file_count(), 12);
    assert!(
        verify_compiler_execution_deployment_v1(
            fixture.root.path(),
            &lower_hex(&report.sha256()),
            COMMIT
        )
        .is_err()
    );
    assert!(
        !Profile::V3
            .files()
            .iter()
            .any(|s| s.source.ends_with("client-check"))
    );
}

#[test]
fn native_refuses_old_header_even_with_fresh_external_hash_and_native_name() {
    let (fixture, _) = native();
    let path = fixture
        .root
        .path()
        .join(COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V3);
    let bytes =
        fs::read_to_string(&path)
            .unwrap()
            .replacen(Profile::V3.header(), Profile::V1.header(), 1);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, bytes.as_bytes()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    let digest: [u8; 32] = Sha256::digest(bytes.as_bytes()).into();
    assert_eq!(
        verify_compiler_execution_deployment_v3(fixture.root.path(), &lower_hex(&digest), COMMIT)
            .unwrap_err()
            .kind(),
        DeploymentVerificationErrorKindV1::InvalidManifest
    );
}

#[test]
fn native_rejects_old_build_info_and_extra_legacy_image() {
    let old =
        format!("schema_version=1\ngit_commit={COMMIT}\nsource_date_epoch=1\ntarget={TARGET}\n");
    validate_build_info(Profile::V1, old.as_bytes(), COMMIT, TARGET).unwrap();
    assert!(validate_build_info(Profile::V3, old.as_bytes(), COMMIT, TARGET).is_err());
    let (fixture, report) = native();
    let path = fixture
        .root
        .path()
        .join("usr/libexec/fe2o3/fe2o3-compiler-execution-client-check");
    fs::write(&path, b"old client").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o555)).unwrap();
    assert_eq!(
        verify_compiler_execution_deployment_v3(
            fixture.root.path(),
            &lower_hex(&report.sha256()),
            COMMIT
        )
        .unwrap_err()
        .kind(),
        DeploymentVerificationErrorKindV1::InvalidInventory
    );
}

#[test]
fn native_install_retains_exact_sources_and_reacquires_without_v1_conversion() {
    let (fixture, report) = native();
    let verified = verify(&fixture, report.sha256());
    assert_eq!(verified.sealed_source_file_count(), 13);
    let expected = fs::read(
        fixture
            .root
            .path()
            .join("systemd/fe2o3-compiler-execution.service"),
    )
    .unwrap();
    drop(fixture);
    let parent = parent();
    let installed = install_for_owner(verified.inner, parent.path(), owner()).unwrap();
    assert_eq!(
        installed.root_name,
        compiler_execution_install_root_name_v3(report.sha256())
    );
    assert_ne!(
        installed.root_name,
        compiler_execution_install_root_name_v1(report.sha256())
    );
    let unit = parent
        .path()
        .join(&installed.root_name)
        .join("usr/lib/systemd/system/fe2o3-compiler-execution.service");
    assert_eq!(fs::read(&unit).unwrap(), expected);
    revalidate_installed(&installed, owner()).unwrap();
    let (fixture, again) = native();
    assert_eq!(again, report);
    let reacquired = install_for_owner(
        verify(&fixture, again.sha256()).inner,
        parent.path(),
        owner(),
    )
    .unwrap();
    assert_eq!(
        reacquired.publication,
        CompilerExecutionInstalledRootPublicationV1::Reacquired
    );
    fs::set_permissions(&unit, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(&unit, b"substituted unit").unwrap();
    fs::set_permissions(&unit, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(revalidate_installed(&installed, owner()).is_err());
    assert!(
        install_for_owner(
            verify(&fixture, again.sha256()).inner,
            parent.path(),
            owner()
        )
        .is_err()
    );
}

#[test]
fn native_uses_same_prepublication_cleanup_and_postpublication_reacquisition() {
    for point in installation_fault_points(Profile::V3) {
        let (fixture, report) = native();
        let parent = parent();
        let mut hooks = InjectInstallationFaultV1 {
            point,
            fired: false,
        };
        let error = install_for_owner_with_hooks(
            verify(&fixture, report.sha256()).inner,
            parent.path(),
            owner(),
            &mut hooks,
        )
        .unwrap_err();
        assert!(hooks.fired, "{point:?}");
        let published = installation_fault_is_after_publication_for_test_v1(point);
        assert_eq!(
            fs::read_dir(parent.path()).unwrap().count(),
            usize::from(published),
            "{point:?}"
        );
        if published {
            assert_eq!(
                error.kind(),
                DeploymentVerificationErrorKindV1::PublicationAmbiguous
            );
        }
        let installed = install_for_owner(
            verify(&fixture, report.sha256()).inner,
            parent.path(),
            owner(),
        )
        .unwrap();
        assert_eq!(
            installed.publication,
            if published {
                CompilerExecutionInstalledRootPublicationV1::Reacquired
            } else {
                CompilerExecutionInstalledRootPublicationV1::Created
            }
        );
        revalidate_installed(&installed, owner()).unwrap();
    }
}

#[test]
fn native_unwind_cleans_only_its_unpublished_staging() {
    struct Unwind;
    impl InstallationHooksV1 for Unwind {
        fn checkpoint(
            &mut self,
            point: InstallationFaultPointV1,
        ) -> Result<(), DeploymentVerificationErrorV1> {
            if point == InstallationFaultPointV1::FileWritten(2) {
                panic!("staged unwind");
            }
            Ok(())
        }
    }
    let (fixture, report) = native();
    let parent = parent();
    assert!(
        catch_unwind(AssertUnwindSafe(|| install_for_owner_with_hooks(
            verify(&fixture, report.sha256()).inner,
            parent.path(),
            owner(),
            &mut Unwind
        )))
        .is_err()
    );
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn native_parent_replacement_preserves_foreign_directory() {
    for trigger in [
        InstallationFaultPointV1::FileWritten(0),
        InstallationFaultPointV1::RootRenamed,
    ] {
        let (fixture, report) = native();
        let outer = parent();
        let original = outer.path().join("parent");
        let displaced = outer.path().join("displaced");
        fs::create_dir(&original).unwrap();
        fs::set_permissions(&original, fs::Permissions::from_mode(0o700)).unwrap();
        let mut hooks = ReplaceInstallParentPathV1 {
            original: original.clone(),
            displaced: displaced.clone(),
            trigger,
            fired: false,
        };
        assert!(
            install_for_owner_with_hooks(
                verify(&fixture, report.sha256()).inner,
                &original,
                owner(),
                &mut hooks
            )
            .is_err()
        );
        assert!(hooks.fired);
        assert!(original.is_dir());
        // The injected replacement is never removed or used to publish this root.
        assert!(
            !original
                .join(compiler_execution_install_root_name_v3(report.sha256()))
                .exists()
        );
        assert_eq!(
            fs::read_dir(&displaced).unwrap().count(),
            usize::from(trigger == InstallationFaultPointV1::RootRenamed)
        );
    }
}

#[test]
fn recovery_never_deletes_other_family_or_mixed_crash_leftovers() {
    for profile in [Profile::V1, Profile::V3] {
        let parent = parent();
        let other = if profile == Profile::V1 {
            Profile::V3
        } else {
            Profile::V1
        };
        let staging = parent
            .path()
            .join(format!("{}{}", profile.staging_prefix(), "0".repeat(32)));
        let foreign = parent
            .path()
            .join(format!("{}{}", other.staging_prefix(), "1".repeat(32)));
        for path in [&staging, &foreign] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(path.join("partial"), b"retain").unwrap();
        }
        let digest = lower_hex(&[7; 32]);
        assert!(
            recover_install_parent_for_owner(profile, parent.path(), &digest, owner()).is_err()
        );
        assert!(staging.join("partial").exists());
        assert!(foreign.join("partial").exists());
        fs::remove_dir_all(&foreign).unwrap();
        let foreign_root = parent.path().join(install_root_name(other, [7; 32]));
        fs::create_dir(&foreign_root).unwrap();
        assert!(
            recover_install_parent_for_owner(profile, parent.path(), &digest, owner()).is_err()
        );
        assert!(staging.join("partial").exists());
        fs::remove_dir(foreign_root).unwrap();
        assert_eq!(
            recover_install_parent_for_owner(profile, parent.path(), &digest, owner()).unwrap(),
            CompilerExecutionInstallRecoveryV1::Recovered
        );
        assert_eq!(
            recover_install_parent_for_owner(profile, parent.path(), &digest, owner()).unwrap(),
            CompilerExecutionInstallRecoveryV1::AlreadyClean
        );
    }
}
