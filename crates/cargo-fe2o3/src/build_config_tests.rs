use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cargo-fe2o3-build-config-v2-{}-{}",
            std::process::id(),
            SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write_manifest(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn provider(path: &Path, byte_len: u64, sha256: [u8; 32]) -> Value {
    serde_json::json!({
        "byte_len": byte_len,
        "kind": "llvm-bitcode",
        "path": path,
        "sha256": hex(&sha256),
    })
}

#[test]
fn providers_reject_aggregate_limits_before_any_payload_io() {
    let scratch = ScratchDirectory::new();
    let missing = scratch.0.join("missing-provider");
    let cap = fe2o3_hsaco_finalize::MAX_WORKER_TOTAL_INPUT_BYTES as u64;
    for lengths in [
        vec![cap + 1],
        vec![cap, 1],
        vec![u64::MAX],
        vec![1, u64::MAX],
    ] {
        let values: Vec<_> = lengths
            .iter()
            .enumerate()
            .map(|(i, len)| provider(&missing, *len, [i as u8; 32]))
            .collect();
        assert!(matches!(
            prepare_providers(&Value::Array(values)),
            Err(BuildConfigError::Invalid(reason)) if reason.contains("total input limit")
        ));
    }
    for lengths in [vec![cap], vec![cap - 1, 1]] {
        let values: Vec<_> = lengths
            .iter()
            .enumerate()
            .map(|(i, len)| provider(&missing, *len, [i as u8; 32]))
            .collect();
        assert!(matches!(
            prepare_providers(&Value::Array(values)),
            Err(BuildConfigError::Io { kind: "provider", error, .. })
                if error.kind() == std::io::ErrorKind::NotFound
        ));
    }
}

#[test]
fn providers_validate_later_metadata_before_opening_earlier_inputs() {
    let scratch = ScratchDirectory::new();
    let missing = scratch.0.join("missing-provider");
    for mutation in 0..4 {
        let first = provider(&missing, 1, [1; 32]);
        let mut second = provider(&missing, 1, [2; 32]);
        match mutation {
            0 => second["sha256"] = first["sha256"].clone(),
            1 => second["kind"] = Value::from("unsupported"),
            2 => second["byte_len"] = Value::from(0),
            _ => {
                second.as_object_mut().unwrap().remove("path");
            }
        }
        assert!(matches!(
            prepare_providers(&serde_json::json!([first, second])),
            Err(BuildConfigError::Invalid(_))
        ));
    }
}

#[test]
fn providers_keep_exact_content_checks_and_bound_reads_by_declared_size() {
    let scratch = ScratchDirectory::new();
    let path = scratch.0.join("provider.bc");
    let bytes = b"provider fixture";
    fs::write(&path, bytes).unwrap();
    let exact = provider(&path, bytes.len() as u64, Sha256::digest(bytes).into());
    let inputs = prepare_providers(&serde_json::json!([exact.clone()])).unwrap();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].bytes(), bytes);
    assert!(
        prepare_providers(&serde_json::json!([]))
            .unwrap()
            .is_empty()
    );

    let mut oversized_file = exact.clone();
    oversized_file["byte_len"] = Value::from(bytes.len() - 1);
    assert!(matches!(
        prepare_providers(&serde_json::json!([oversized_file])),
        Err(BuildConfigError::Invalid(reason)) if reason.contains("regular file containing")
    ));
    let mut short_file = exact.clone();
    short_file["byte_len"] = Value::from(bytes.len() + 1);
    let mut wrong_digest = exact;
    wrong_digest["sha256"] = Value::from(hex(&[0; 32]));
    for rejected in [short_file, wrong_digest] {
        assert!(matches!(
            prepare_providers(&serde_json::json!([rejected])),
            Err(BuildConfigError::Protocol(_))
        ));
    }
}

fn executable_measurement() -> (PathBuf, [u8; 32], u64) {
    let path = std::env::current_exe().unwrap();
    let bytes = fs::read(&path).unwrap();
    let byte_len = bytes.len() as u64;
    let sha256 = Sha256::digest(bytes).into();
    (path, sha256, byte_len)
}

fn complete_manifest(scratch: &ScratchDirectory, version: u8) -> Value {
    let (worker, sha256, byte_len) = executable_measurement();
    let mut root = serde_json::json!({
        "candidate_output_max_bytes": 1048576,
        "format": if version == 1 {
            PRODUCTION_BUILD_CONFIG_FORMAT_V1
        } else {
            PRODUCTION_BUILD_CONFIG_FORMAT_V2
        },
        "limits": {
            "stderr_bytes": 4096,
            "stdout_bytes": 4096,
            "timeout_ms": 1000
        },
        "link_options": [
            {"name": "code-object-version", "value": "5"},
            {"name": "opt-level", "value": "2"},
            {"name": "strip-debug", "value": "false"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": [],
        "units": [{
            "crate_name": "kernel",
            "source": "src/lib.rs",
            "working_directory": scratch.0.to_str().unwrap()
        }],
        "worker": {
            "byte_len": byte_len,
            "llvm_build_identity": "llvm-build-v1",
            "path": worker.to_str().unwrap(),
            "sha256": hex(&sha256),
            "worker_build_identity": "worker-build-v1"
        }
    });
    if version == 2 {
        root.as_object_mut().unwrap().insert(
            "observation".to_owned(),
            serde_json::json!({"kind": SOURCE_ISA_SUMMARY_OBSERVATION_KIND_V1}),
        );
    }
    root
}

fn native_fixture(scratch: &ScratchDirectory, version: u8) -> (PathBuf, BuildConfigIdentity) {
    let mut manifest = complete_manifest(scratch, version);
    let worker = Path::new("/bin/true");
    let worker_bytes = fs::read(worker).unwrap();
    manifest["worker"]["path"] = serde_json::json!(worker);
    manifest["worker"]["byte_len"] = Value::from(worker_bytes.len());
    manifest["worker"]["sha256"] = Value::from(hex(&Sha256::digest(&worker_bytes)));
    let payload = b"native configuration provider";
    let path = scratch.0.join("provider.bc");
    fs::write(&path, payload).unwrap();
    manifest["providers"] = serde_json::json!([provider(
        &path,
        payload.len() as u64,
        Sha256::digest(payload).into()
    )]);
    let path = scratch.write_manifest(&format!("native-v{version}.json"), &manifest);
    let parsed = if version == 1 {
        prepare_production_manifest_v1(&path)
    } else {
        prepare_production_manifest_v2(&path)
    }
    .unwrap();
    (path, parsed.identity())
}

pub(crate) fn native_recipe_for_test(
    b: &mut Budget<'_>,
) -> native::PreparedNativeProductionBuildConfig {
    let scratch = ScratchDirectory::new();
    let (path, expected) = native_fixture(&scratch, 1);
    native::PreparedNativeProductionBuildConfig::from_manifest(
        &path,
        ProductionBuildConfigVersion::V1,
        expected.as_bytes(),
        b,
    )
    .unwrap()
}

#[test]
fn native_configuration_preserves_both_schema_identities_and_exact_account_limits() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use native::PreparedNativeProductionBuildConfig as Native;
    let scratch = ScratchDirectory::new();
    for version in [1, 2] {
        let (path, expected) = native_fixture(&scratch, version);
        let schema = if version == 1 {
            ProductionBuildConfigVersion::V1
        } else {
            ProductionBuildConfigVersion::V2(ProductionSourceIsaObservationKindV1::Summary)
        };
        let mut limits = (30_000_000, 30_000_000);
        for case in 0..4 {
            let mut work = Work::new(limits.0 - usize::from(case == 2));
            let mut b = Budget::new(&mut work, limits.1 - usize::from(case == 3));
            b.charge_work(19).unwrap();
            b.reserve_storage(37).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let result = Native::from_manifest(&path, schema, expected.as_bytes(), &mut b);
            if case < 2 {
                assert_eq!(result.unwrap().identity(), expected);
            } else {
                assert!(matches!(result, Err(BuildConfigError::Resource(_))));
            }
            if case == 0 {
                limits = (b.work(), b.peak_storage());
            }
            if case == 3 {
                assert!(b.failed_storage().is_some());
            }
            assert!(b.storage() >= 37);
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn native_recipe_rejects_foreign_ledgers_and_retired_input_storage() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use native::PreparedNativeProductionBuildConfig as Native;
    let scratch = ScratchDirectory::new();
    let (path, expected) = native_fixture(&scratch, 1);
    for case in 0..4 {
        let mut work = Work::new(30_000_000);
        let mut b = Budget::new(&mut work, 30_000_000);
        b.reserve_storage(37).unwrap();
        let recipe = Native::from_manifest(
            &path,
            ProductionBuildConfigVersion::V1,
            expected.as_bytes(),
            &mut b,
        )
        .unwrap();
        let paid = b.storage();
        if case == 1 {
            let mut other_work = Work::new(30_000_000);
            let mut other = Budget::new(&mut other_work, 30_000_000);
            other.reserve_storage(paid).unwrap();
            assert!(matches!(
                recipe.into_worker_parts(&mut other),
                Err(Resource::Accounting)
            ));
            assert_eq!(b.storage(), paid);
        } else if case == 2 {
            b.release_storage(1).unwrap();
            assert!(matches!(
                recipe.into_worker_parts(&mut b),
                Err(Resource::Accounting)
            ));
            assert_eq!(b.storage(), paid - 1);
        } else if case == 3 {
            let mut relocated = Box::new(b);
            assert!(matches!(
                recipe.into_worker_parts(&mut relocated),
                Err(Resource::Accounting)
            ));
            assert_eq!(relocated.storage(), paid);
        } else {
            let (_, providers, options, _, _, storage) = recipe.into_worker_parts(&mut b).unwrap();
            assert_eq!(providers[0].bytes(), b"native configuration provider");
            assert_eq!(options.len(), REQUIRED_OPTIONS.len());
            assert_eq!(storage + 37, paid);
            assert_eq!(b.storage(), paid);
        }
    }
}

#[test]
fn native_configuration_checks_expected_identity_and_keeps_terminal_reservations() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use native::PreparedNativeProductionBuildConfig as Native;
    let scratch = ScratchDirectory::new();
    let (path, expected) = native_fixture(&scratch, 1);
    let mut wrong = *expected.as_bytes();
    wrong[0] ^= 1;
    let mut work = Work::new(30_000_000);
    let mut b = Budget::new(&mut work, 30_000_000);
    b.reserve_storage(37).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert!(
        matches!(Native::from_manifest(&path, ProductionBuildConfigVersion::V1, &wrong, &mut b),
        Err(BuildConfigError::Invalid(reason)) if reason.contains("expected identity"))
    );
    let after_failure = b.storage();
    assert!(after_failure > 37);
    assert!(matches!(
        Native::from_manifest(
            &path,
            ProductionBuildConfigVersion::V2(ProductionSourceIsaObservationKindV1::Summary),
            expected.as_bytes(),
            &mut b
        ),
        Err(BuildConfigError::Invalid(_))
    ));
    assert!(b.storage() > after_failure);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn native_configuration_resource_denials_precede_their_io_and_parse_operations() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use native::PreparedNativeProductionBuildConfig as Native;
    let scratch = ScratchDirectory::new();
    let missing = scratch.0.join("missing");
    let mut work = Work::new(0);
    let mut b = Budget::new(&mut work, 30_000_000);
    assert!(matches!(
        Native::from_manifest(&missing, ProductionBuildConfigVersion::V1, &[0; 32], &mut b),
        Err(BuildConfigError::Resource(Resource::Work(_)))
    ));

    let malformed = scratch.0.join("malformed.json");
    fs::write(&malformed, b"{").unwrap();
    let mut work = Work::new(30_000_000);
    let mut b = Budget::new(&mut work, 30_000_000);
    assert!(matches!(
        Native::from_manifest(
            &malformed,
            ProductionBuildConfigVersion::V1,
            &[0; 32],
            &mut b
        ),
        Err(BuildConfigError::Json(_))
    ));
    let exact_work = b.work();
    let paid = b.storage();
    let mut work = Work::new(exact_work - 1);
    let mut b = Budget::new(&mut work, paid);
    assert!(matches!(
        Native::from_manifest(
            &malformed,
            ProductionBuildConfigVersion::V1,
            &[0; 32],
            &mut b
        ),
        Err(BuildConfigError::Resource(Resource::Work(_)))
    ));
    let mut work = Work::new(30_000_000);
    let mut b = Budget::new(&mut work, 0);
    assert!(matches!(
        prepare_providers_on_account(
            &serde_json::json!([provider(&missing, 1, [0; 32])]),
            Some(&mut b)
        ),
        Err(BuildConfigError::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn native_configuration_quote_overflow_keeps_the_original_account() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let mut work = Work::new(100);
    let mut b = Budget::new(&mut work, 100);
    b.charge_work(7).unwrap();
    b.reserve_storage(37).unwrap();
    let ledger = b.work_ledger_identity_v1();
    for result in [
        native::prepay_read(usize::MAX, &mut b),
        native::prepay_manifest(usize::MAX, &mut b),
        native::prepay_providers(usize::MAX, 0, &mut b),
        native::prepay_providers(0, usize::MAX, &mut b),
    ] {
        assert_eq!(result, Err(Resource::Arithmetic));
    }
    assert_eq!((b.work(), b.storage()), (7, 37));
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn production_v1_schema_identity_and_inert_observer_behavior_are_frozen() {
    assert_eq!(
        PRODUCTION_BUILD_CONFIG_FORMAT_V1,
        "fe2o3-production-build-config-v1"
    );
    assert_eq!(PRODUCTION_CONFIG_PROFILE_ID_V1, "production-v1");
    assert_eq!(
        PRODUCTION_CONFIG_IDENTITY_DOMAIN_V1,
        b"fe2o3-build-config-transitive-v1"
    );
    assert_eq!(
        ROOT_KEYS_V1,
        [
            "candidate_output_max_bytes",
            "format",
            "limits",
            "link_options",
            "providers",
            "units",
            "worker",
        ]
    );
    assert!(!ProductionBuildConfigVersion::V1.source_isa_summary_enabled());

    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::from_parts([0x11; 32], 123),
        "worker-build-v1",
        "llvm-build-v1",
    )
    .unwrap();
    let identity = transitive_identity_from_measurement(
        PRODUCTION_CONFIG_IDENTITY_DOMAIN_V1,
        PRODUCTION_CONFIG_PROFILE_ID_V1,
        br#"{"format":"frozen-v1"}"#,
        &measurement,
        &[],
    );
    assert_eq!(
        identity.to_hex(),
        "6a8e515a9a85bc48b67ce8cc8af892c8325aab699457ea5d1c2fea2459e8213c"
    );
}

#[test]
fn production_v2_observation_is_exact_and_has_a_distinct_identity_domain() {
    assert_ne!(
        PRODUCTION_CONFIG_IDENTITY_DOMAIN_V1,
        PRODUCTION_CONFIG_IDENTITY_DOMAIN_V2
    );
    assert!(
        ProductionBuildConfigVersion::V2(ProductionSourceIsaObservationKindV1::Summary)
            .source_isa_summary_enabled()
    );
    assert!(
        parse_source_isa_observation(&serde_json::json!({"kind": "source-isa-summary-v1"})).is_ok()
    );
    assert_eq!(
        parse_source_isa_observation(&serde_json::json!({"kind": "source-isa-characteristic-v1"}))
            .unwrap(),
        ProductionSourceIsaObservationKindV1::Characteristic
    );
    for rejected in [
        serde_json::json!({"kind": "source-isa-summary-v2"}),
        serde_json::json!({"kind": "source-isa-summary-v1", "output": "stderr"}),
        serde_json::json!({}),
    ] {
        assert!(parse_source_isa_observation(&rejected).is_err());
    }
}

#[test]
fn characteristic_observation_rejects_multi_unit_configuration() {
    let scratch = ScratchDirectory::new();
    let mut manifest = complete_manifest(&scratch, 2);
    manifest["observation"] =
        serde_json::json!({"kind": SOURCE_ISA_CHARACTERISTIC_OBSERVATION_KIND_V1});
    manifest["units"] = serde_json::json!([
        {
            "crate_name": "kernel",
            "source": "src/lib.rs",
            "working_directory": scratch.0.to_str().unwrap()
        },
        {
            "crate_name": "kernel_two",
            "source": "src/lib.rs",
            "working_directory": scratch.0.to_str().unwrap()
        }
    ]);
    let path = scratch.write_manifest("characteristic-multi-unit.json", &manifest);
    assert!(matches!(
        prepare_production_manifest_v2(&path),
        Err(BuildConfigError::Invalid(message))
            if message == "source-isa-characteristic-v1 requires exactly one configured unit"
    ));
}

#[test]
fn complete_v2_manifest_binds_identity_units_and_observer_policy() {
    let scratch = ScratchDirectory::new();
    let path = scratch.write_manifest("v2.json", &complete_manifest(&scratch, 2));
    let config = prepare_production_manifest_v2(&path).unwrap();
    assert_eq!(
        config.config_environment_name(),
        PRODUCTION_BUILD_CONFIG_V2_ENV
    );
    assert_eq!(
        config.expected_identity_environment_name(),
        PRODUCTION_BUILD_EXPECTED_ID_V2_ENV
    );

    let source = Path::new("src/lib.rs");
    let expected = config
        .source_isa_unit_identity("kernel", source, &scratch.0)
        .unwrap();
    let policy = config.source_isa_observer_policy().unwrap().unwrap();
    assert_eq!(policy.config_identity(), config.identity());
    assert_eq!(policy.selected_units(), &[expected]);
    assert_eq!(
        expected,
        source_isa_unit_identity(
            config.identity(),
            "kernel",
            "src/lib.rs",
            scratch.0.to_str().unwrap()
        )
    );
    assert_ne!(
        expected,
        source_isa_unit_identity(
            config.identity(),
            "kernel-mutated",
            "src/lib.rs",
            scratch.0.to_str().unwrap()
        )
    );
    assert_ne!(
        expected,
        source_isa_unit_identity(
            config.identity(),
            "kernel",
            "src/other.rs",
            scratch.0.to_str().unwrap()
        )
    );
    assert_ne!(
        expected,
        source_isa_unit_identity(config.identity(), "kernel", "src/lib.rs", "/other")
    );
    assert!(
        config
            .source_isa_unit_identity("kernel-mutated", source, &scratch.0)
            .is_none()
    );
    assert!(
        config
            .source_isa_unit_identity("kernel", Path::new("src/other.rs"), &scratch.0)
            .is_none()
    );
    assert!(
        config
            .source_isa_unit_identity("kernel", source, Path::new("/other"))
            .is_none()
    );
}

#[test]
fn v1_manifest_retains_no_observer_policy() {
    let scratch = ScratchDirectory::new();
    let path = scratch.write_manifest("v1.json", &complete_manifest(&scratch, 1));
    let config = prepare_production_manifest_v1(&path).unwrap();
    assert_eq!(
        config.config_environment_name(),
        PRODUCTION_BUILD_CONFIG_ENV
    );
    assert!(config.source_isa_observer_policy().unwrap().is_none());
    assert!(
        config
            .source_isa_unit_identity("kernel", Path::new("src/lib.rs"), &scratch.0)
            .is_none()
    );
}

#[test]
fn expected_identity_namespaces_reject_orphans_wrong_versions_and_dual_values() {
    let scratch = ScratchDirectory::new();
    let v2_path = scratch.write_manifest("v2.json", &complete_manifest(&scratch, 2));
    let v1_path = scratch.write_manifest("v1.json", &complete_manifest(&scratch, 1));
    let v2 = prepare_production_manifest_v2(&v2_path).unwrap();
    let v1 = prepare_production_manifest_v1(&v1_path).unwrap();
    let v2_identity = v2.identity().to_hex();
    let v1_identity = v1.identity().to_hex();
    assert_ne!(v1_identity, v2_identity);
    let wrong = OsStr::new("0000000000000000000000000000000000000000000000000000000000000000");

    assert!(
        validate_expected_build_config_identity_values(
            Some(&v2),
            None,
            Some(OsStr::new(&v2_identity))
        )
        .is_ok()
    );
    assert!(
        validate_expected_build_config_identity_values(
            Some(&v1),
            Some(OsStr::new(&v1_identity)),
            None
        )
        .is_ok()
    );
    for result in [
        validate_expected_build_config_identity_values(Some(&v2), None, None),
        validate_expected_build_config_identity_values(Some(&v2), Some(wrong), None),
        validate_expected_build_config_identity_values(Some(&v2), None, Some(wrong)),
        validate_expected_build_config_identity_values(
            Some(&v2),
            Some(OsStr::new(&v1_identity)),
            Some(OsStr::new(&v2_identity)),
        ),
        validate_expected_build_config_identity_values(Some(&v1), None, None),
        validate_expected_build_config_identity_values(Some(&v1), None, Some(wrong)),
        validate_expected_build_config_identity_values(Some(&v1), Some(wrong), None),
        validate_expected_build_config_identity_values(
            Some(&v1),
            Some(OsStr::new(&v1_identity)),
            Some(OsStr::new(&v2_identity)),
        ),
        validate_expected_build_config_identity_values(None, Some(wrong), None),
        validate_expected_build_config_identity_values(None, None, Some(wrong)),
        validate_expected_build_config_identity_values(None, Some(wrong), Some(wrong)),
    ] {
        assert!(result.is_err());
    }
    assert!(validate_expected_build_config_identity_values(None, None, None).is_ok());
}

#[test]
fn broker_identity_rejects_schema_substitution_omission_and_environment_only_agreement() {
    let scratch = ScratchDirectory::new();
    let v1_path = scratch.write_manifest("v1.json", &complete_manifest(&scratch, 1));
    let v2_path = scratch.write_manifest("v2.json", &complete_manifest(&scratch, 2));
    let v1 = prepare_production_manifest_v1(&v1_path).unwrap();
    let v2 = prepare_production_manifest_v2(&v2_path).unwrap();
    assert_ne!(v1.identity(), v2.identity());
    assert!(validate_brokered_build_config_identity(None, None).is_ok());
    for (config, other) in [(&v1, &v2), (&v2, &v1)] {
        let identity = *config.identity().as_bytes();
        validate_brokered_build_config_identity(Some(config), Some(identity)).unwrap();
        let environment_identity = config.identity().to_hex();
        let (expected_v1, expected_v2) = match config.version {
            ProductionBuildConfigVersion::V1 => (Some(OsStr::new(&environment_identity)), None),
            ProductionBuildConfigVersion::V2(_) => (None, Some(OsStr::new(&environment_identity))),
        };
        validate_expected_build_config_identity_values(Some(config), expected_v1, expected_v2)
            .unwrap();
        let mut changed = identity;
        changed[0] ^= 1;
        for result in [
            validate_brokered_build_config_identity(Some(config), None),
            validate_brokered_build_config_identity(None, Some(identity)),
            validate_brokered_build_config_identity(Some(config), Some(changed)),
            validate_brokered_build_config_identity(
                Some(config),
                Some(*other.identity().as_bytes()),
            ),
        ] {
            assert!(matches!(result, Err(BuildConfigError::Invalid(message))
                if message == "production build configuration differs from the authenticated broker binding"));
        }
    }
}

#[test]
fn v2_hostile_schema_matrix_is_rejected_before_worker_admission() {
    let scratch = ScratchDirectory::new();
    let valid = complete_manifest(&scratch, 2);
    let mut hostile = Vec::new();

    let mut missing_observation = valid.clone();
    missing_observation
        .as_object_mut()
        .unwrap()
        .remove("observation");
    hostile.push(missing_observation);

    let mut extra_root = valid.clone();
    extra_root
        .as_object_mut()
        .unwrap()
        .insert("output".to_owned(), Value::Null);
    hostile.push(extra_root);

    for observation in [
        Value::Null,
        serde_json::json!({}),
        serde_json::json!({"kind": "source-isa-summary-v2"}),
        serde_json::json!({"kind": SOURCE_ISA_SUMMARY_OBSERVATION_KIND_V1, "output": "stderr"}),
    ] {
        let mut value = valid.clone();
        value
            .as_object_mut()
            .unwrap()
            .insert("observation".to_owned(), observation);
        hostile.push(value);
    }

    for (index, value) in hostile.iter().enumerate() {
        let path = scratch.write_manifest(&format!("hostile-{index}.json"), value);
        assert!(
            prepare_production_manifest_v2(&path).is_err(),
            "case {index}"
        );
    }

    let mut v1_with_observation = complete_manifest(&scratch, 1);
    v1_with_observation.as_object_mut().unwrap().insert(
        "observation".to_owned(),
        serde_json::json!({"kind": SOURCE_ISA_SUMMARY_OBSERVATION_KIND_V1}),
    );
    let path = scratch.write_manifest("v1-with-observation.json", &v1_with_observation);
    assert!(prepare_production_manifest_v1(&path).is_err());
}
