//! Synthetic inert record/filesystem controls, not a protected compiler release.
//! The private engine substitutes ONLY profile filesystem ownership for rootless tests.
use super::*;
use crate::{compiler_runtime_bundle::tests::Fixture, encode_sha256_lower_hex_v1 as encode};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Issuer,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
};

fn owner() -> (u32, u32) {
    (
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
}
fn mode(path: &Path, value: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(value)).unwrap();
}
fn write(path: &Path, bytes: &[u8], value: u32) {
    if path.exists() {
        mode(path, 0o600);
    }
    fs::write(path, bytes).unwrap();
    mode(path, value);
}
fn profile() -> Profile {
    let mut work = Work::new(8_000_000);
    let mut b = Budget::new(&mut work, 8_000_000);
    let (issuer, charge) = Issuer::new(
        1,
        Measurement::new([1; 32], 10).unwrap(),
        Measurement::new([2; 32], 11).unwrap(),
        SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[4; 32]).verifying_key().to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    Profile::new(
        1000,
        1000,
        Service::new(1001, 1001).unwrap(),
        issuer,
        &mut b,
    )
    .unwrap()
    .0
}

struct Package {
    temporary: tempfile::TempDir,
    inputs: Fixture,
    recipe: PathBuf,
    profile_root: PathBuf,
    output_parent: PathBuf,
}
impl Package {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let inputs = Fixture::new();
        let recipe = temporary.path().join("recipe");
        let profile_root = temporary.path().join("profile-root");
        let output_parent = temporary.path().join("output-parent");
        for path in [&profile_root, &output_parent] {
            fs::create_dir(path).unwrap();
            mode(path, 0o700);
        }
        for path in ["etc", "etc/fe2o3", "etc/fe2o3/compiler-execution"] {
            fs::create_dir(profile_root.join(path)).unwrap();
            mode(&profile_root.join(path), 0o755);
        }
        write(
            &profile_root.join(PROFILE),
            profile().canonical_bytes(),
            0o444,
        );
        let result = Self {
            temporary,
            inputs,
            recipe,
            profile_root,
            output_parent,
        };
        write(&result.recipe, result.text().as_bytes(), 0o444);
        result
    }
    fn text(&self) -> String {
        let closure = self.inputs.manifest.compiler_closure();
        let mut result = format!("{HEADER}\n");
        for (key, value) in [
            ("cargo_sha256", closure.cargo_executable_sha256()),
            (
                "trampoline_sha256",
                closure.cargo_binding_trampoline_sha256(),
            ),
            (
                "wrapper_sha256",
                closure.cargo_fe2o3_binding_wrapper_sha256(),
            ),
            ("rustc_sha256", closure.rustc_executable_sha256()),
            ("rustc_tree_sha256", closure.rustc_runtime_tree_sha256()),
            ("backend_sha256", closure.codegen_backend_sha256()),
            (
                "proof_runtime_identity",
                *self.inputs.manifest.proof_runtime_identity(),
            ),
        ] {
            result.push_str(&format!("{key}={}\n", encode(value)));
        }
        result.push_str("helper_uid=60001\nhelper_gid=60002\n");
        for entry in self.inputs.manifest.entries() {
            let role = match entry.role {
                Role::Rustc => "rustc",
                Role::CodegenBackend => "backend",
                Role::Fe2o3ProcMacro => "proc-macro",
                Role::ElfInterpreter => "interpreter",
                Role::ProofExecutorHelper => "proof-helper",
                Role::SharedLibrary => "shared-library",
            };
            result.push_str(&format!(
                "{role} {} {} {}\n",
                entry.length,
                encode(entry.sha256),
                entry.path
            ));
        }
        result
    }
    fn source(&self) -> PathBuf {
        self.inputs.bundle.join("runtime")
    }
    fn destination(&self, name: &str) -> PathBuf {
        self.output_parent.join(name)
    }
    fn quota(&self) -> CompilerRuntimePackageQuotaV1 {
        CompilerRuntimePackagePlanV1 {
            manifest: self.inputs.manifest.clone(),
            helper_uid: 60001,
            helper_gid: 60002,
        }
        .quota()
        .unwrap()
    }
    fn read(&self) -> Result<CompilerRuntimePackagePlanV1> {
        let mut w = Work::new(CompilerRuntimePackagePlanV1::READ_WORK);
        let mut b = Budget::new(&mut w, CompilerRuntimePackagePlanV1::READ_STORAGE);
        CompilerRuntimePackagePlanV1::read(&self.recipe, &mut b)
    }
    fn run(
        &self,
        name: &str,
        hook: &mut impl FnMut(Step) -> Result<()>,
    ) -> Result<PackagedCompilerRuntimeDeploymentV1> {
        let quota = self.quota();
        let total = CompilerRuntimePackagePlanV1::READ_WORK + quota.work();
        let peak = CompilerRuntimePackagePlanV1::STORAGE + quota.storage();
        let mut work = Work::new(total);
        let mut b = Budget::new(&mut work, peak);
        let plan = CompilerRuntimePackagePlanV1::read(&self.recipe, &mut b)?;
        b.reserve_storage(CompilerRuntimePackagePlanV1::STORAGE)?;
        let ledger = b.work_ledger_identity_v1();
        let result = package_using(
            &plan,
            &self.source(),
            &self.profile_root,
            &self.destination(name),
            owner(),
            &mut b,
            hook,
        );
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.storage(), CompilerRuntimePackagePlanV1::STORAGE);
        if result.is_ok() {
            assert_eq!(b.work(), total);
            assert_eq!(b.peak_storage(), peak);
            assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
        }
        result
    }
}

#[test]
fn deterministic_measured_package_roundtrips_existing_verifier_without_authority() {
    let f = Package::new();
    let first = f.run("first", &mut |_| Ok(())).unwrap();
    let second = f.run("second", &mut |_| Ok(())).unwrap();
    assert_eq!(first.policy_sha256(), second.policy_sha256());
    assert_eq!(first.manifest_sha256(), second.manifest_sha256());
    assert_eq!(first.file_count(), 6);
    let verified = verify_compiler_runtime_deployment_v1(
        &f.destination("first"),
        first.policy_sha256(),
        first.manifest_sha256(),
    )
    .unwrap();
    assert_eq!(verified.manifest(), &f.inputs.manifest);
    assert_eq!(
        verified.policy().client_profile_identity(),
        profile().identity().as_bytes()
    );
    assert_eq!(verified.policy().proof_helper_uid(), 60001);
    assert_eq!(verified.policy().proof_helper_gid(), 60002);
    assert!(!verified.policy().grants_authority());
    for entry in verified.manifest().entries() {
        let path = f.destination("first").join("runtime").join(entry.path);
        assert_eq!(
            fs::metadata(&path).unwrap().mode() & 0o7777,
            entry.role.protected_mode()
        );
        assert_eq!(
            fs::read(path).unwrap(),
            fs::read(f.source().join(entry.path)).unwrap()
        );
    }
    assert_eq!(
        fs::read(f.profile_root.join(PROFILE)).unwrap(),
        profile().canonical_bytes()
    );
}

#[test]
#[ignore = "requires root UID/GID and native filesystem operations; synthetic inert records only"]
fn native_public_package_requires_root_owned_profile_and_roundtrips() {
    assert_eq!(
        std::env::var("FE2O3_RUNTIME_PACKAGE_NATIVE")
            .expect("explicit isolated native runner marker"),
        "isolated-disposable-root",
    );
    assert_eq!(
        owner(),
        (0, 0),
        "run explicitly as root; never skip success"
    );
    let f = Package::new();
    let q = f.quota();
    let floor = CompilerRuntimePackagePlanV1::STORAGE;
    let mut work = Work::new(CompilerRuntimePackagePlanV1::READ_WORK + 2 * q.work());
    let mut budget = Budget::new(&mut work, floor + q.storage());
    let plan = CompilerRuntimePackagePlanV1::read(&f.recipe, &mut budget).unwrap();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = package_compiler_runtime_deployment_v1(
        &plan,
        &f.source(),
        &f.profile_root,
        &f.destination("public"),
        &mut budget,
    )
    .unwrap();
    let verified = verify_compiler_runtime_deployment_v1(
        &f.destination("public"),
        result.policy_sha256(),
        result.manifest_sha256(),
    )
    .unwrap();
    assert_eq!(verified.manifest(), &f.inputs.manifest);
    assert_eq!(
        verified.policy().client_profile_identity(),
        profile().identity().as_bytes()
    );
    assert_eq!(
        budget.work(),
        CompilerRuntimePackagePlanV1::READ_WORK + q.work()
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + q.storage());
    drop(verified);

    // Mutate only this test's private profile object. The public entry point must
    // refuse its wrong owner before creating a second bundle, on the same account.
    let profile_file = File::open(f.profile_root.join(PROFILE)).unwrap();
    rustix::fs::fchown(
        &profile_file,
        Some(rustix::process::Uid::from_raw(60003)),
        None,
    )
    .unwrap();
    let result = package_compiler_runtime_deployment_v1(
        &plan,
        &f.source(),
        &f.profile_root,
        &f.destination("wrong-owner"),
        &mut budget,
    );
    assert!(
        matches!(result, Err(CompilerRuntimePackageErrorV1::Deployment(ref e)) if e.kind() == Kind::InvalidMetadata)
    );
    assert!(!f.destination("wrong-owner").exists());
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        budget.work(),
        CompilerRuntimePackagePlanV1::READ_WORK + 2 * q.work() - PROFILE_WORK - POLICY_WORK
    );
    assert_eq!(budget.peak_storage(), floor + q.storage());
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn recipe_rejects_noncanonical_numbers_hashes_paths_roles_and_inventory() {
    let f = Package::new();
    let original = f.text();
    let rustc = f
        .inputs
        .manifest
        .compiler_closure()
        .rustc_executable_sha256();
    let cases = [
        original.replace(HEADER, "other"),
        original.replace("helper_uid=60001", "helper_uid=060001"),
        original.replace("helper_gid=60002", "helper_gid=0"),
        original.replace("helper_gid=60002", "helper_gid=4294967296"),
        original.replace("helper_uid=60001", "helper_uid=18446744073709551616"),
        original.replace("proof-helper ", "unknown "),
        original.replace("bin/helper", "../helper"),
        original.replace("bin/helper", "/bin/helper"),
        original.replace("bin/helper", "bin//helper"),
        original.replace("bin/helper", "bin/./helper"),
        original.replace("bin/rustc", "bin/helper"),
        original.replace("bin/rustc", "bin/helper/child"),
        original.replace("bin/helper", "z-last"),
        original.replace("proof-helper ", "shared-library "),
        original.replace(&encode(rustc), &"AB".repeat(32)),
        original.replace(&encode(rustc), &"00".repeat(32)),
        original.replace("proof-helper 12 ", "proof-helper 0 "),
        original.replace('\n', "\r\n"),
        original.trim_end().to_owned(),
        format!("{original}\n"),
    ];
    for (index, text) in cases.iter().enumerate() {
        write(&f.recipe, text.as_bytes(), 0o444);
        assert!(f.read().is_err(), "recipe mutation {index}");
    }
    write(&f.recipe, &[b'x'; MAX_RECIPE + 1], 0o444);
    assert!(f.read().is_err());
    write(&f.recipe, &[], 0o444);
    assert!(f.read().is_err());
}

#[test]
fn recipe_origin_rejects_links_and_wrong_mode() {
    for case in 0..3 {
        let f = Package::new();
        match case {
            0 => mode(&f.recipe, 0o644),
            1 => fs::hard_link(&f.recipe, f.temporary.path().join("alias")).unwrap(),
            _ => {
                let moved = f.temporary.path().join("moved");
                fs::rename(&f.recipe, &moved).unwrap();
                symlink(moved, &f.recipe).unwrap();
            }
        }
        assert!(f.read().is_err());
    }
}

#[test]
fn recipe_retains_existing_manifest_count_bound() {
    let f = Package::new();
    let original_count = f.inputs.manifest.entries().len();
    for extra in [MAX_FILES - original_count, MAX_FILES - original_count + 1] {
        let mut text = f.text();
        for i in 0..extra {
            text.push_str(&format!(
                "shared-library 1 {} z/lib-{i:03}\n",
                "ab".repeat(32)
            ));
        }
        write(&f.recipe, text.as_bytes(), 0o444);
        let result = f.read();
        if extra + original_count == MAX_FILES {
            assert_eq!(result.unwrap().manifest.entries().len(), MAX_FILES);
        } else {
            assert!(result.is_err());
        }
    }
}

#[test]
fn source_modes_links_types_and_exact_lengths_refuse() {
    for case in 0..6 {
        let f = Package::new();
        let path = f.source().join("lib/libc.so");
        match case {
            0 => mode(&path, 0o644),
            1 => fs::hard_link(&path, f.temporary.path().join("alias")).unwrap(),
            2 => {
                let moved = f.temporary.path().join("moved");
                fs::rename(&path, &moved).unwrap();
                symlink(moved, &path).unwrap();
            }
            3 => {
                fs::remove_file(&path).unwrap();
                fs::create_dir(&path).unwrap();
            }
            4 => mode(&f.source().join("lib"), 0o755),
            _ => write(&path, b"short", 0o444),
        }
        assert!(f.run("result", &mut |_| Ok(())).is_err());
        assert!(!f.destination("result").exists());
    }
}

#[test]
fn wrong_measured_digest_leaves_explicitly_unapproved_nonreusable_output() {
    let f = Package::new();
    let path = f.source().join("lib/libc.so");
    let mut bytes = fs::read(&path).unwrap();
    bytes[CHUNK] ^= 1;
    write(&path, &bytes, 0o444);
    let error = f.run("result", &mut |_| Ok(())).unwrap_err();
    assert!(
        matches!(error, CompilerRuntimePackageErrorV1::Deployment(ref e) if e.kind() == Kind::ContentMismatch)
    );
    assert!(error.to_string().contains("unapproved partial package"));
    assert!(f.destination("result").exists());
    bytes[CHUNK] ^= 1;
    write(&path, &bytes, 0o444);
    assert!(f.run("result", &mut |_| Ok(())).is_err());
}

#[test]
fn syntactically_valid_expected_digest_does_not_approve_different_bytes() {
    let f = Package::new();
    let original = fs::read(f.source().join("lib/libc.so")).unwrap();
    let expected = f
        .inputs
        .manifest
        .entries()
        .find(|e| e.path == "lib/libc.so")
        .unwrap()
        .sha256;
    write(
        &f.recipe,
        f.text()
            .replace(&encode(expected), &"ab".repeat(32))
            .as_bytes(),
        0o444,
    );
    let error = f.run("result", &mut |_| Ok(())).unwrap_err();
    assert!(
        matches!(error, CompilerRuntimePackageErrorV1::Deployment(ref e) if e.kind() == Kind::ContentMismatch)
    );
    assert_eq!(fs::read(f.source().join("lib/libc.so")).unwrap(), original);
}

#[test]
fn paths_and_output_parent_refuse_before_copying() {
    let f = Package::new();
    for path in [
        "relative",
        "/",
        "/tmp/../other",
        "/tmp/./other",
        "/tmp//other",
        "/tmp/other/",
    ] {
        assert!(canonical_path(Path::new(path)).is_err(), "{path}");
    }
    let plan = f.read().unwrap();
    let q = plan.quota().unwrap();
    for destination in [
        f.source().join("nested-output"),
        f.profile_root.join("nested-output"),
        f.destination("result"),
    ] {
        mode(&f.output_parent, 0o755);
        let mut w = Work::new(q.work());
        let mut b = Budget::new(&mut w, CompilerRuntimePackagePlanV1::STORAGE + q.storage());
        b.reserve_storage(CompilerRuntimePackagePlanV1::STORAGE)
            .unwrap();
        assert!(
            package_using(
                &plan,
                &f.source(),
                &f.profile_root,
                &destination,
                owner(),
                &mut b,
                &mut |_| Ok(())
            )
            .is_err()
        );
        assert!(!destination.exists());
    }
}

#[test]
fn existing_destination_including_dangling_link_is_never_reused() {
    for case in 0..3 {
        let f = Package::new();
        let path = f.destination("result");
        match case {
            0 => fs::create_dir(&path).unwrap(),
            1 => write(&path, b"preserve", 0o444),
            _ => symlink("missing", &path).unwrap(),
        }
        let before = fs::symlink_metadata(&path).unwrap();
        assert!(f.run("result", &mut |_| Ok(())).is_err());
        let after = fs::symlink_metadata(&path).unwrap();
        assert_eq!(
            (before.dev(), before.ino(), before.mode()),
            (after.dev(), after.ino(), after.mode())
        );
    }
}

#[test]
fn profile_credentials_and_protection_fail_before_output() {
    for case in 0..5 {
        let f = Package::new();
        match case {
            0 => write(
                &f.recipe,
                f.text()
                    .replace("helper_uid=60001", "helper_uid=1000")
                    .as_bytes(),
                0o444,
            ),
            1 => write(
                &f.recipe,
                f.text()
                    .replace("helper_gid=60002", "helper_gid=1001")
                    .as_bytes(),
                0o444,
            ),
            2 => mode(&f.profile_root.join(PROFILE), 0o644),
            3 => {
                let mut bytes = profile().canonical_bytes().to_vec();
                bytes[0] ^= 1;
                write(&f.profile_root.join(PROFILE), &bytes, 0o444);
            }
            _ => fs::hard_link(
                f.profile_root.join(PROFILE),
                f.temporary.path().join("profile-alias"),
            )
            .unwrap(),
        }
        assert!(f.run("result", &mut |_| Ok(())).is_err());
        assert!(!f.destination("result").exists());
    }
}

#[test]
fn original_sources_profile_and_path_custody_survive_until_final_verification() {
    for checkpoint in [Step::Copied, Step::Verified] {
        for case in 0..4 {
            let f = Package::new();
            let mut reached = false;
            let result = f.run("result", &mut |step| {
                if step == checkpoint {
                    reached = true;
                    let path = match case {
                        0 => f.source().join("bin/rustc"),
                        1 => f.profile_root.join(PROFILE),
                        2 => f.source().join("lib"),
                        _ => f.source(),
                    };
                    if case < 2 {
                        let bytes = fs::read(&path).unwrap();
                        let original_mode = fs::metadata(&path).unwrap().mode() & 0o7777;
                        let retained_profile = if case == 1 {
                            let displaced = f.temporary.path().join("displaced-profile");
                            fs::rename(&path, &displaced).unwrap();
                            Some(displaced)
                        } else {
                            fs::remove_file(&path).unwrap();
                            None
                        };
                        write(&path, &bytes, original_mode);
                        if let Some(displaced) = retained_profile {
                            let retained = fs::metadata(displaced).unwrap();
                            let replacement = fs::metadata(&path).unwrap();
                            assert_eq!(retained.nlink(), 1);
                            assert_eq!(replacement.nlink(), 1);
                            assert_ne!(
                                (retained.dev(), retained.ino()),
                                (replacement.dev(), replacement.ino())
                            );
                        }
                    } else {
                        fs::rename(&path, f.temporary.path().join("displaced")).unwrap();
                        fs::create_dir(&path).unwrap();
                        mode(&path, 0o700);
                    }
                }
                Ok(())
            });
            assert!(reached, "original custody checkpoint was not reached");
            let expected = match case {
                0 => Kind::InvalidMetadata, // The retained original lost its link.
                2 => Kind::Io,              // The substituted directory lacks the original files.
                _ => Kind::InputChanged,
            };
            assert!(
                matches!(&result, Err(CompilerRuntimePackageErrorV1::Deployment(error)) if error.kind() == expected),
                "original custody case {case}: {result:?}"
            );
        }
    }
}

#[test]
fn completed_verifier_is_not_stale_after_a_late_output_mutation() {
    for case in 0..8 {
        let f = Package::new();
        let mut reached = false;
        let result = f.run("result", &mut |step| {
            if step == Step::Verified {
                reached = true;
                let root = f.destination("result");
                match case {
                    0 => write(&root.join("runtime/lib/extra"), b"extra", 0o444),
                    1 => {
                        let file = root.join("runtime/bin/rustc");
                        let bytes = fs::read(&file).unwrap();
                        fs::remove_file(&file).unwrap();
                        write(&file, &bytes, 0o555);
                    }
                    2 => {
                        fs::rename(&root, root.with_extension("old")).unwrap();
                        fs::create_dir(&root).unwrap();
                        mode(&root, 0o700);
                    }
                    3 => {
                        let file = root.join("runtime/bin/rustc");
                        let original = fs::metadata(&file).unwrap();
                        let mut bytes = fs::read(&file).unwrap();
                        bytes[0] ^= 1;
                        write(&file, &bytes, 0o555);
                        let changed = fs::metadata(&file).unwrap();
                        assert_eq!(
                            (original.dev(), original.ino()),
                            (changed.dev(), changed.ino())
                        );
                    }
                    4 | 5 => {
                        let file = root.join(if case == 4 {
                            "policy-v2"
                        } else {
                            "compiler-runtime-manifest-v1"
                        });
                        let bytes = fs::read(&file).unwrap();
                        fs::remove_file(&file).unwrap();
                        write(&file, &bytes, 0o444);
                    }
                    6 => {
                        let directory = root.join("runtime/lib");
                        fs::rename(&directory, directory.with_extension("old")).unwrap();
                        fs::create_dir(&directory).unwrap();
                        mode(&directory, 0o700);
                    }
                    _ => {
                        fs::rename(&f.output_parent, f.output_parent.with_extension("old"))
                            .unwrap();
                        fs::create_dir(&f.output_parent).unwrap();
                        mode(&f.output_parent, 0o700);
                    }
                }
            }
            Ok(())
        });
        assert!(
            reached,
            "existing verifier must have succeeded before mutation"
        );
        assert!(
            matches!(&result, Err(CompilerRuntimePackageErrorV1::Deployment(error)) if error.kind() == Kind::InputChanged),
            "late output mutation {case}: {result:?}"
        );
    }
}

#[test]
fn exact_and_one_short_quotes_preserve_original_floor_and_denial_history() {
    const OUTER: usize = 23;
    for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
        let f = Package::new();
        let q = f.quota();
        let work_limit =
            CompilerRuntimePackagePlanV1::READ_WORK + q.work() - usize::from(work_short);
        let storage_limit = OUTER + CompilerRuntimePackagePlanV1::STORAGE + q.storage()
            - usize::from(storage_short);
        let mut w = Work::new(work_limit);
        let mut b = Budget::new(&mut w, storage_limit);
        b.reserve_storage(OUTER).unwrap();
        let plan = CompilerRuntimePackagePlanV1::read(&f.recipe, &mut b).unwrap();
        b.reserve_storage(CompilerRuntimePackagePlanV1::STORAGE)
            .unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = package_using(
            &plan,
            &f.source(),
            &f.profile_root,
            &f.destination("result"),
            owner(),
            &mut b,
            &mut |_| Ok(()),
        );
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.storage(), OUTER + CompilerRuntimePackagePlanV1::STORAGE);
        if work_short {
            assert!(matches!(
                result,
                Err(CompilerRuntimePackageErrorV1::Resource(Resource::Work(_)))
            ));
            assert_eq!(b.failed_work(), Some(work_limit + 1));
        } else if storage_short {
            assert!(matches!(
                result,
                Err(CompilerRuntimePackageErrorV1::Resource(Resource::Storage(
                    _
                )))
            ));
            assert_eq!(b.failed_storage(), Some(storage_limit + 1));
        } else {
            assert!(result.is_ok());
            assert_eq!(b.work(), work_limit);
            assert_eq!(b.peak_storage(), storage_limit);
        }
        assert_eq!(
            f.destination("result").exists(),
            !work_short && !storage_short
        );
    }
}

#[test]
fn reading_and_plan_floor_refuse_underfunding_without_output() {
    let f = Package::new();
    for (work, storage) in [
        (
            CompilerRuntimePackagePlanV1::READ_WORK - 1,
            CompilerRuntimePackagePlanV1::READ_STORAGE,
        ),
        (
            CompilerRuntimePackagePlanV1::READ_WORK,
            CompilerRuntimePackagePlanV1::READ_STORAGE - 1,
        ),
    ] {
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, storage);
        assert!(CompilerRuntimePackagePlanV1::read(&f.recipe, &mut b).is_err());
        assert_eq!(b.storage(), 0);
        assert!(b.failed_work().is_some() || b.failed_storage().is_some());
    }
    let plan = f.read().unwrap();
    let q = plan.quota().unwrap();
    let mut w = Work::new(q.work());
    let mut b = Budget::new(&mut w, q.storage());
    let result = package_using(
        &plan,
        &f.source(),
        &f.profile_root,
        &f.destination("result"),
        owner(),
        &mut b,
        &mut |_| Ok(()),
    );
    assert!(matches!(
        result,
        Err(CompilerRuntimePackageErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(b.work(), 8);
    assert_eq!(b.storage(), 0);
    assert!(!f.destination("result").exists());
}

fn references(id: (u64, u64)) -> usize {
    let mut count = 0;
    for (i, entry) in fs::read_dir("/proc/self/fd").unwrap().enumerate() {
        assert!(i < 4096);
        if let Ok(metadata) = fs::metadata(entry.unwrap().path()) {
            count += usize::from((metadata.dev(), metadata.ino()) == id);
        }
    }
    count
}

#[test]
fn failure_and_unwind_drop_sources_without_refunding_account_history() {
    for unwind in [false, true] {
        for stop in [Step::Copied, Step::Verified] {
            let f = Package::new();
            let q = f.quota();
            let limit = CompilerRuntimePackagePlanV1::READ_WORK + q.work();
            let storage = CompilerRuntimePackagePlanV1::STORAGE + q.storage();
            let mut w = Work::new(limit);
            let mut b = Budget::new(&mut w, storage);
            let plan = CompilerRuntimePackagePlanV1::read(&f.recipe, &mut b).unwrap();
            b.reserve_storage(CompilerRuntimePackagePlanV1::STORAGE)
                .unwrap();
            assert!(b.charge_work(limit).is_err());
            assert!(b.reserve_storage(storage).is_err());
            let denials = (b.failed_work(), b.failed_storage());
            let ledger = b.work_ledger_identity_v1();
            let ids: Vec<_> = f
                .inputs
                .manifest
                .entries()
                .map(|e| {
                    let m = fs::metadata(f.source().join(e.path)).unwrap();
                    ((m.dev(), m.ino()), references((m.dev(), m.ino())))
                })
                .collect();
            let mut reached = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                package_using(
                    &plan,
                    &f.source(),
                    &f.profile_root,
                    &f.destination("result"),
                    owner(),
                    &mut b,
                    &mut |step| {
                        if step == stop {
                            reached = true;
                            if unwind {
                                panic!("package checkpoint");
                            }
                            return Err(invalid(Kind::InjectedFailure, "package checkpoint").into());
                        }
                        Ok(())
                    },
                )
            }));
            assert!(reached);
            if unwind {
                assert_eq!(
                    result.err().unwrap().downcast_ref::<&str>(),
                    Some(&"package checkpoint")
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(b.storage(), CompilerRuntimePackagePlanV1::STORAGE);
            assert_eq!(b.work(), limit);
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!((b.failed_work(), b.failed_storage()), denials);
            for (id, count) in ids {
                assert_eq!(references(id), count);
            }
        }
    }
}
