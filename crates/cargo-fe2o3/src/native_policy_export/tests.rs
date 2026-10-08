use super::*;
use crate::pinned_executable_test_directory::TestDirectory;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::os::unix::fs::{PermissionsExt, symlink};

fn request(dir: &Path) -> Request {
    Request {
        crate_name: "selected_kernel".into(),
        source: "src/lib.rs".into(),
        output: dir.join("inputs").to_str().unwrap().into(),
    }
}
fn account() -> Owned {
    Owned::new(Work::new(100_000_000), 16 * 1024 * 1024)
}
fn observations() -> OriginalObservations {
    // Descriptive format tests only; not an original compiler owner fixture.
    OriginalObservations {
        handoff: ([1; 32], 9),
        carriage: [2; 32],
        artifact: [3; 32],
        copy_classification: fe2o3_verifier::NativeCopyProgramClassificationV1::Unsupported,
    }
}

#[test]
fn native_policy_export_cli_is_build_only_exact_and_not_an_application_argument() {
    let args = |parts: &[&str]| parts.iter().map(OsString::from).collect::<Vec<_>>();
    let input = args(&[
        "--locked",
        FLAG,
        "selected_kernel",
        "./src/lib.rs",
        "/private/new",
        "--lib",
    ]);
    let (forwarded, selected) = parse("build", &input).unwrap();
    assert_eq!(forwarded, args(&["--locked", "--lib"]));
    assert_eq!(selected.unwrap().source, "./src/lib.rs");
    for command in ["run", "check", "test"] {
        assert!(parse(command, &input).is_err());
    }
    let mut duplicate = input.clone();
    duplicate.extend_from_slice(&input[1..5]);
    assert!(parse("build", &duplicate).is_err());
    for bad in [
        args(&[FLAG]),
        args(&[FLAG, "kernel", "src/lib.rs"]),
        args(&[FLAG, "kernel", "src/lib.rs", "relative"]),
    ] {
        assert!(parse("build", &bad).is_err());
    }
    let trailing = args(&["--", FLAG, "k", "source", "/output"]);
    let (forwarded, selected) = parse("run", &trailing).unwrap();
    assert_eq!(forwarded, trailing);
    assert!(selected.is_none());
}

#[test]
fn native_policy_export_selection_preserves_exact_producer_and_clears_default_environment() {
    let dir = TestDirectory::new();
    let request = request(dir.path());
    assert!(
        request
            .matches(
                &ProducerIdentity::from_codegen("selected_kernel", Some(Path::new("src/lib.rs")))
                    .unwrap()
            )
            .unwrap()
    );
    for (name, source) in [
        ("different", "src/lib.rs"),
        ("selected_kernel", "./src/lib.rs"),
    ] {
        assert!(
            !request
                .matches(&ProducerIdentity::from_codegen(name, Some(Path::new(source))).unwrap())
                .unwrap()
        );
    }
    let mut command = Command::new("unused");
    command.env(ENV, "ambient");
    configure(&mut command, None).unwrap();
    assert!(
        command
            .get_envs()
            .any(|(name, value)| name == ENV && value.is_none())
    );
    configure(&mut command, Some(&request)).unwrap();
    let encoded = command
        .get_envs()
        .find(|(name, _)| *name == ENV)
        .unwrap()
        .1
        .unwrap();
    let decoded = decode(encoded).unwrap();
    assert_eq!(decoded.source, request.source);
    assert_eq!(decoded.output, request.output);
    assert!(decode(std::ffi::OsStr::new("{\"unknown\":1}")).is_err());
    assert!(decode(std::ffi::OsStr::new(&"x".repeat(REQUEST_MAX + 1))).is_err());
    assert!(reject_ambient(None).is_ok());
    for value in ["", "1", encoded.to_str().unwrap()] {
        assert!(reject_ambient(Some(std::ffi::OsStr::new(value))).is_err());
    }
}

#[test]
fn native_policy_export_publishes_exact_inert_bundle_on_original_account() {
    let dir = TestDirectory::new();
    let request = request(dir.path());
    let parent = request.preflight().unwrap();
    let source = b"format-test source, not semantic evidence";
    let roster = b"format-test roster, not accepted policy";
    account().with_budget(|budget| {
        budget.reserve_storage(source.len() + roster.len()).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let storage = budget.storage_account_identity_v1();
        request
            .publish(
                &request.producer().unwrap(),
                source,
                roster,
                observations(),
                budget,
            )
            .unwrap();
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage_account_identity_v1(), storage);
    });
    parent.require_bundle().unwrap();
    assert_eq!(
        std::fs::read(Path::new(&request.output).join(NAMES[0])).unwrap(),
        source
    );
    assert_eq!(
        std::fs::read(Path::new(&request.output).join(NAMES[1])).unwrap(),
        roster
    );
    let raw = std::fs::read(Path::new(&request.output).join(NAMES[2])).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(manifest["grants_authority"], false);
    assert_eq!(manifest["independently_approved"], false);
    assert_eq!(
        manifest["final_f_copy_observation"]["classification"],
        "unsupported"
    );
    assert_eq!(
        manifest["final_f_copy_observation"]["machine_accepted"],
        false
    );
    assert_eq!(manifest["source_spelling"], "src/lib.rs");
    assert_eq!(
        manifest["source_packet"]["sha256"],
        crate::hex_encode(&Sha256::digest(source))
    );
    assert_eq!(std::fs::read_dir(&request.output).unwrap().count(), 3);
    assert!(request.preflight().is_err());
}

#[test]
fn copied_shape_diagnostic_cannot_mark_a_machine_or_policy_accepted() {
    // Format-only observation, not a fabricated recovered compiler owner.
    let dir = TestDirectory::new();
    let request = request(dir.path());
    let mut observations = observations();
    observations.copy_classification =
        fe2o3_verifier::NativeCopyProgramClassificationV1::GuardedU32 {
            load: [17, 14],
            store: [17, 15],
        };
    account().with_budget(|budget| {
        budget.reserve_storage(2).unwrap();
        request
            .publish(
                &request.producer().unwrap(),
                b"s",
                b"r",
                observations,
                budget,
            )
            .unwrap();
    });
    let raw = std::fs::read(Path::new(&request.output).join(NAMES[2])).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let copy = &manifest["final_f_copy_observation"];
    assert_eq!(copy["classification"], "guarded-u32-copy-final-f-v1");
    assert_eq!(copy["load"], serde_json::json!([17, 14]));
    assert_eq!(copy["store"], serde_json::json!([17, 15]));
    assert_eq!(copy["machine_accepted"], false);
    assert_eq!(manifest["grants_authority"], false);
    assert_eq!(manifest["independently_approved"], false);
    assert_eq!(std::fs::read_dir(&request.output).unwrap().count(), 3);
}

#[test]
fn native_policy_export_refuses_budget_wrong_producer_shared_parent_and_aliases_before_output() {
    let dir = TestDirectory::new();
    let request = request(dir.path());
    for (work, storage) in [(0, 16 * 1024 * 1024), (100_000_000, 1)] {
        Owned::new(Work::new(work), storage).with_budget(|budget| {
            assert!(
                request
                    .publish(
                        &request.producer().unwrap(),
                        b"a",
                        b"b",
                        observations(),
                        budget
                    )
                    .is_err()
            );
        });
    }
    account().with_budget(|budget| {
        budget.reserve_storage(2).unwrap();
        let other =
            ProducerIdentity::from_codegen("different", Some(Path::new("src/lib.rs"))).unwrap();
        assert!(
            request
                .publish(&other, b"a", b"b", observations(), budget)
                .is_err()
        );
    });
    assert!(!Path::new(&request.output).exists());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(request.preflight().is_err());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(dir.path(), &request.output).unwrap();
    assert!(request.preflight().is_err());
}

#[test]
fn native_policy_export_refusal_cleans_only_original_stage_and_never_replaces_destination() {
    let dir = TestDirectory::new();
    let request = request(dir.path());
    let parent = request.open_parent(true).unwrap();
    assert!(
        publish_directory(&parent, [b"a", b"b", b"c"], || {
            assert!(!Path::new(&request.output).exists());
            Err(error("injected refusal"))
        })
        .is_err()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    assert!(
        publish_directory(&parent, [b"a", b"b", b"c"], || {
            std::fs::create_dir(&request.output)?;
            std::fs::write(Path::new(&request.output).join("other"), b"preserve")
        })
        .is_err()
    );
    assert_eq!(
        std::fs::read(Path::new(&request.output).join("other")).unwrap(),
        b"preserve"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn native_policy_export_does_not_delete_substituted_staging_names_or_records() {
    let dir = TestDirectory::new();
    let request = request(dir.path());
    let parent = request.open_parent(true).unwrap();
    let retained = dir.path().join("retained-stage");
    assert!(
        publish_directory(&parent, [b"a", b"b", b"c"], || {
            let stage = std::fs::read_dir(dir.path())?.next().unwrap()?.path();
            std::fs::rename(&stage, &retained)?;
            std::fs::create_dir(&stage)?;
            std::fs::write(stage.join("unrelated"), b"preserve")?;
            std::fs::remove_file(retained.join(NAMES[0]))?;
            std::fs::write(retained.join(NAMES[0]), b"replacement")?;
            Err(error("injected substitution"))
        })
        .is_err()
    );
    assert_eq!(
        std::fs::read(retained.join(NAMES[0])).unwrap(),
        b"replacement"
    );
    let replacement = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b".fe2o3-native-inputs-")
        })
        .unwrap();
    assert_eq!(
        std::fs::read(replacement.join("unrelated")).unwrap(),
        b"preserve"
    );
    assert!(!Path::new(&request.output).exists());
}
