//! Filesystem/publication controls with explicit synthetic immutability, not root approval.
use super::*;
use crate::compiler_runtime_bundle::tests::Fixture;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV3 as IssuerPolicy,
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
};

fn profile() -> Profile {
    let mut work = Work::new(8_000_000);
    let mut budget = Budget::new(&mut work, 8_000_000);
    let (policy, charge) = IssuerPolicy::new(
        1,
        Measurement::new([1; 32], 123).unwrap(),
        Measurement::new([2; 32], 456).unwrap(),
        SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[4; 32]).verifying_key().to_bytes(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (profile, charge) = Profile::new(
        1000,
        1000,
        Service::new(1001, 1001).unwrap(),
        policy,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    profile
}

fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
fn owner() -> (u32, u32) {
    (
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
}
fn synthetic_immutable(_: &File) -> Result<()> {
    Ok(())
}
const SYNTHETIC: Immutability = Immutability {
    install: synthetic_immutable,
    verify: synthetic_immutable,
};

struct Offline {
    _temporary: tempfile::TempDir,
    path: PathBuf,
    profile: Profile,
    bundle: Fixture,
}
impl Offline {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("offline");
        fs::create_dir(&path).unwrap();
        chmod(&path, 0o700);
        for name in [
            "etc",
            "etc/fe2o3",
            "etc/fe2o3/compiler-execution",
            "opt",
            "opt/fe2o3",
        ] {
            fs::create_dir(path.join(name)).unwrap();
            chmod(&path.join(name), 0o755);
        }
        let profile = profile();
        fs::write(path.join(PROFILE_PATH), profile.canonical_bytes()).unwrap();
        chmod(&path.join(PROFILE_PATH), 0o444);
        let mut bundle = Fixture::new();
        bundle.bind_profile(*profile.identity().as_bytes(), 60001, 60002);
        Self {
            _temporary: temporary,
            path,
            profile,
            bundle,
        }
    }
    fn install(
        &self,
        hook: &mut impl FnMut(Stage) -> Result<()>,
    ) -> Result<InstalledCompilerRuntimeDeploymentV1> {
        install_using(
            self.bundle.verify().unwrap(),
            &self.path,
            owner(),
            SYNTHETIC,
            hook,
        )
    }
    fn published(&self) -> (bool, bool) {
        (
            self.path.join(RUNTIME_PARENT).join(RUNTIME_NAME).exists(),
            self.path.join(APPROVAL_PARENT).join(APPROVAL_NAME).exists(),
        )
    }
}

#[test]
fn initial_install_has_exact_code_records_modes_and_unchanged_profile() {
    let fixture = Offline::new();
    let result = fixture.install(&mut |_| Ok(())).unwrap();
    assert_eq!(result.file_count(), 6);
    let (policy, manifest) = fixture.bundle.pins();
    assert_eq!(result.policy_sha256(), policy);
    assert_eq!(result.manifest_sha256(), manifest);
    assert_eq!(fixture.published(), (true, true));
    let verified = fixture.bundle.verify().unwrap();
    for entry in verified.manifest().entries() {
        let path = fixture
            .path
            .join(RUNTIME_PARENT)
            .join(RUNTIME_NAME)
            .join(entry.path);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
            entry.role.protected_mode()
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(fs::read(path).unwrap())),
            entry.sha256
        );
    }
    assert_eq!(
        fs::read(fixture.path.join(PROFILE_PATH)).unwrap(),
        fixture.profile.canonical_bytes()
    );
    assert_eq!(
        fs::read_dir(fixture.path.join(RUNTIME_PARENT))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        fs::read_dir(fixture.path.join(APPROVAL_PARENT))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn initial_install_never_replaces_or_reuses_any_destination() {
    for name in [
        format!("{RUNTIME_PARENT}/{RUNTIME_NAME}"),
        format!("{APPROVAL_PARENT}/{APPROVAL_NAME}"),
    ] {
        for kind in 0..3 {
            let fixture = Offline::new();
            let destination = fixture.path.join(&name);
            match kind {
                0 => fs::create_dir(&destination).unwrap(),
                1 => fs::write(&destination, b"preserve").unwrap(),
                _ => symlink("missing", &destination).unwrap(),
            }
            let before = fs::symlink_metadata(&destination).unwrap();
            assert_eq!(
                fixture.install(&mut |_| Ok(())).unwrap_err().kind(),
                Kind::InvalidInventory
            );
            assert_eq!(
                fs::symlink_metadata(destination).unwrap().file_type(),
                before.file_type()
            );
        }
    }
    let fixture = Offline::new();
    fixture.install(&mut |_| Ok(())).unwrap();
    assert_eq!(
        fixture.install(&mut |_| Ok(())).unwrap_err().kind(),
        Kind::InvalidInventory
    );
}

#[test]
fn profile_binding_and_separate_helper_credentials_precede_mutation() {
    for (identity, uid, gid) in [
        ([9; 32], 60001, 60002),
        (*profile().identity().as_bytes(), 1000, 60002),
        (*profile().identity().as_bytes(), 1001, 60002),
        (*profile().identity().as_bytes(), 60001, 1000),
        (*profile().identity().as_bytes(), 60001, 1001),
    ] {
        let mut fixture = Offline::new();
        fixture.bundle.bind_profile(identity, uid, gid);
        assert_eq!(
            fixture.install(&mut |_| Ok(())).unwrap_err().kind(),
            Kind::ContentMismatch
        );
        assert_eq!(fixture.published(), (false, false));
        assert_eq!(
            fs::read_dir(fixture.path.join(RUNTIME_PARENT))
                .unwrap()
                .count(),
            0
        );
    }
}

#[test]
fn protected_parents_and_profile_reject_links_modes_and_missing_paths() {
    for case in 0..6 {
        let fixture = Offline::new();
        let profile = fixture.path.join(PROFILE_PATH);
        match case {
            0 => chmod(&fixture.path, 0o755),
            1 => chmod(&fixture.path.join("opt"), 0o777),
            2 => chmod(&profile, 0o644),
            3 => fs::hard_link(&profile, fixture.path.join("alias")).unwrap(),
            4 => fs::remove_file(&profile).unwrap(),
            _ => {
                fs::rename(&profile, fixture.path.join("profile")).unwrap();
                symlink("../../../profile", &profile).unwrap();
            }
        }
        assert!(fixture.install(&mut |_| Ok(())).is_err());
        assert_eq!(fixture.published(), (false, false));
    }
}

#[test]
fn every_interruption_reports_retained_paths_without_rollback() {
    for stop in [
        Stage::Staging,
        Stage::Prepared,
        Stage::RuntimePublished,
        Stage::ApprovalPublished,
    ] {
        let fixture = Offline::new();
        let error = fixture
            .install(&mut |stage| {
                if stage == stop {
                    Err(invalid(Kind::InjectedFailure, "interrupted installation"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.kind(), Kind::IncompleteRuntimeInstallation);
        assert!(error.to_string().contains(&format!("{stop:?}")));
        assert!(error.to_string().contains("no rollback performed"));
        assert_eq!(
            fixture.published(),
            match stop {
                Stage::Staging | Stage::Prepared => (false, false),
                Stage::RuntimePublished => (true, false),
                Stage::ApprovalPublished => (true, true),
            }
        );
    }
}

#[test]
fn profile_replacement_and_parent_displacement_refuse_publication() {
    for case in 0..3 {
        let fixture = Offline::new();
        let error = fixture
            .install(&mut |stage| {
                if stage == Stage::Prepared {
                    match case {
                        0 => {
                            fs::remove_file(fixture.path.join(PROFILE_PATH)).unwrap();
                            fs::write(
                                fixture.path.join(PROFILE_PATH),
                                fixture.profile.canonical_bytes(),
                            )
                            .unwrap();
                            chmod(&fixture.path.join(PROFILE_PATH), 0o444);
                        }
                        1 => {
                            fs::rename(
                                fixture.path.join(RUNTIME_PARENT),
                                fixture.path.join("displaced"),
                            )
                            .unwrap();
                            fs::create_dir(fixture.path.join(RUNTIME_PARENT)).unwrap();
                        }
                        _ => {
                            fs::rename(&fixture.path, fixture.path.with_extension("displaced"))
                                .unwrap();
                            fs::create_dir(&fixture.path).unwrap();
                            chmod(&fixture.path, 0o700);
                        }
                    }
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.kind(), Kind::IncompleteRuntimeInstallation);
        assert_eq!(fixture.published(), (false, false));
    }
}

#[test]
fn publication_race_never_overwrites_new_destination() {
    let fixture = Offline::new();
    let destination = fixture.path.join(APPROVAL_PARENT).join(APPROVAL_NAME);
    let error = fixture
        .install(&mut |stage| {
            if stage == Stage::RuntimePublished {
                fs::write(&destination, b"preserve").unwrap();
            }
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("RuntimePublished"));
    assert_eq!(fs::read(destination).unwrap(), b"preserve");
}

#[test]
fn completed_roster_and_file_metadata_are_checked_before_success() {
    for case in 0..3 {
        let fixture = Offline::new();
        let error = fixture
            .install(&mut |stage| {
                if stage == Stage::ApprovalPublished {
                    let runtime = fixture.path.join(RUNTIME_PARENT).join(RUNTIME_NAME);
                    match case {
                        0 => fs::write(runtime.join("extra"), b"extra").unwrap(),
                        1 => chmod(&runtime.join("bin/rustc"), 0o755),
                        _ => {
                            fs::remove_file(runtime.join("bin/rustc")).unwrap();
                            fs::write(runtime.join("bin/rustc"), b"inert rustc").unwrap();
                            chmod(&runtime.join("bin/rustc"), 0o555);
                        }
                    }
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.to_string().contains("ApprovalPublished"));
        assert_eq!(error.kind(), Kind::IncompleteRuntimeInstallation);
    }
}

#[test]
fn staged_directory_displacement_refuses_before_rename() {
    for (stop, parent, prefix) in [
        (Stage::Prepared, RUNTIME_PARENT, ".compiler-runtime-"),
        (
            Stage::RuntimePublished,
            APPROVAL_PARENT,
            ".compiler-approval-",
        ),
    ] {
        let fixture = Offline::new();
        let error = fixture
            .install(&mut |stage| {
                if stage == stop {
                    let path = fs::read_dir(fixture.path.join(parent))
                        .unwrap()
                        .map(|e| e.unwrap().path())
                        .find(|p| p.file_name().unwrap().to_str().unwrap().starts_with(prefix))
                        .unwrap();
                    fs::rename(&path, path.with_extension("displaced")).unwrap();
                    fs::create_dir(&path).unwrap();
                    chmod(&path, 0o755);
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.kind(), Kind::IncompleteRuntimeInstallation);
        assert_eq!(
            fixture.published(),
            (stop == Stage::RuntimePublished, false)
        );
    }
}

#[test]
fn lost_immutability_is_refused_without_repair_at_each_publication_boundary() {
    use std::cell::Cell;
    thread_local! {
        static INSTALLS: Cell<usize> = const { Cell::new(0) };
        static VERIFICATIONS: Cell<usize> = const { Cell::new(0) };
        static REFUSE_AT: Cell<usize> = const { Cell::new(0) };
    }
    fn install(_: &File) -> Result<()> {
        INSTALLS.set(INSTALLS.get() + 1);
        Ok(())
    }
    fn verify(_: &File) -> Result<()> {
        VERIFICATIONS.set(VERIFICATIONS.get() + 1);
        if VERIFICATIONS.get() == REFUSE_AT.get() {
            Err(invalid(Kind::InvalidMetadata, "lost immutable flag"))
        } else {
            Ok(())
        }
    }
    // Eight copy-time checks; each tree observation checks retained and named files.
    for (at, published) in [(9, (false, false)), (25, (true, false)), (41, (true, true))] {
        INSTALLS.set(0);
        VERIFICATIONS.set(0);
        REFUSE_AT.set(at);
        let fixture = Offline::new();
        let error = install_using(
            fixture.bundle.verify().unwrap(),
            &fixture.path,
            owner(),
            Immutability { install, verify },
            &mut |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), Kind::IncompleteRuntimeInstallation);
        assert_eq!(fixture.published(), published);
        assert_eq!(
            INSTALLS.get(),
            8,
            "verification must never reinstall immutable flags"
        );
    }
}

#[test]
fn immutable_operation_failure_never_publishes_policy() {
    fn deny(_: &File) -> Result<()> {
        Err(invalid(
            Kind::InvalidMetadata,
            "immutable operation refused",
        ))
    }
    let fixture = Offline::new();
    let error = install_using(
        fixture.bundle.verify().unwrap(),
        &fixture.path,
        owner(),
        Immutability {
            install: deny,
            verify: synthetic_immutable,
        },
        &mut |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), Kind::IncompleteRuntimeInstallation);
    assert_eq!(fixture.published(), (false, false));
}

#[test]
fn native_cleanup_preserves_filesystem_flags_unknown_to_rustix() {
    let extents = 0x0008_0000;
    let flags = IFlags::from_bits_retain(extents | IFlags::IMMUTABLE.bits());
    assert_eq!(disposable_flags(flags).bits(), extents);
}

fn disposable_flags(flags: IFlags) -> IFlags {
    // Bitflags' `!` truncates unknown flags, including ext4's EXTENTS bit.
    IFlags::from_bits_retain(flags.bits() & !IFlags::IMMUTABLE.bits())
}

#[test]
#[ignore = "requires root with CAP_LINUX_IMMUTABLE in an isolated disposable filesystem; synthetic code is not compiler qualification"]
fn native_immutable_directory_publication_and_no_overwrite() {
    assert_eq!(owner(), (0, 0));
    assert_eq!(
        std::env::var("FE2O3_RUNTIME_INSTALL_NATIVE").as_deref(),
        Ok("isolated-disposable-root")
    );
    let fixture = Offline::new();
    let source = fixture.bundle.verify().unwrap();
    let mut paths: Vec<_> = source
        .manifest()
        .entries()
        .map(|e| {
            fixture
                .path
                .join(RUNTIME_PARENT)
                .join(RUNTIME_NAME)
                .join(e.path)
        })
        .collect();
    paths.extend(["policy-v2", "compiler-runtime-manifest-v1"].map(|name| {
        fixture
            .path
            .join(APPROVAL_PARENT)
            .join(APPROVAL_NAME)
            .join(name)
    }));
    install_compiler_runtime_deployment_v1(source, &fixture.path).unwrap();
    let files: Vec<_> = paths.iter().map(|path| File::open(path).unwrap()).collect();
    for file in &files {
        require_immutable(file).unwrap();
    }
    for path in &paths {
        assert!(fs::remove_file(path).is_err());
    }
    assert_eq!(
        install_compiler_runtime_deployment_v1(fixture.bundle.verify().unwrap(), &fixture.path)
            .unwrap_err()
            .kind(),
        Kind::InvalidInventory
    );
    // Only this test's retained, freshly created inodes are made disposable.
    for file in &files {
        let flags = ioctl_getflags(file).unwrap();
        ioctl_setflags(file, disposable_flags(flags)).unwrap();
    }
}
