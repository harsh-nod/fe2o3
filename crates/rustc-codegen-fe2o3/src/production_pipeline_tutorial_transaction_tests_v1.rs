mod tests {
    use super::*;
    use fe2o3_compiler_ffi::{InertCompilerStageOutputReceiptV5, ProductionCompilerOutputStageV5};
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
        Terminator,
    };

    #[test]
    fn stale_request_binding_has_stable_diagnostic() {
        let request = serde_json::json!({
            "candidate": {},
            "capabilityKernel": {},
            "fixture": {},
            "hardwareReceiptChallenge": {},
            "manifest": {},
            "productionTransaction": {},
            "requestBindingSha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "roadmapIssue": ROADMAP_ISSUE,
            "schema": REQUEST_SCHEMA,
            "simulatorEvidence": {},
        });
        let bytes = canonical_document(&request).unwrap();
        let directory = std::env::current_dir().unwrap();
        let error = preflight_request(&directory, bytes, false).unwrap_err();
        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::RequestBinding
        );
        assert!(error.to_string().starts_with("FE2O3-TUTORIAL-TXN-004:"));
    }

    #[test]
    fn wrong_target_is_rejected_before_any_compiler_work() {
        let mut request = minimal_bound_request("sm_90");
        bind_request(&mut request);
        let bytes = canonical_document(&request).unwrap();
        let directory = std::env::current_dir().unwrap();
        let error = preflight_request(&directory, bytes, false).unwrap_err();
        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::WrongTarget
        );
    }

    #[test]
    fn missing_evidence_roster_is_rejected() {
        let error = validate_local_evidence_roster(&BTreeMap::new()).unwrap_err();
        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::MissingEvidence
        );
    }

    #[test]
    fn final_record_negative_fixture_claims_remain_explicitly_incomplete() {
        let apparently_complete = serde_json::json!({
            "cases": [{
                "diagnosticCode": "FE2O3-NEG-0001",
                "status": "passed",
            }],
            "setSha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "status": "passed",
        });
        let error = validate_production_negative_fixture_receipt_v1(
            &apparently_complete,
            b"self-consistent-but-untyped-negative-evidence",
        )
        .unwrap_err();

        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable
        );
        assert_eq!(
            error.message(),
            PRODUCTION_NEGATIVE_FIXTURE_RECEIPT_INCOMPLETE_V1
        );
    }

    #[test]
    fn genuine_stage_receipt_accepts_only_exact_artifact_bytes() {
        let artifact = b"genuine-object";
        let receipt = InertCompilerStageOutputReceiptV5::from_stage_output(
            ProductionCompilerOutputStageV5::Object,
            artifact,
        )
        .unwrap();
        require_exact_measurement(
            receipt.output_sha256(),
            receipt.output_bytes(),
            artifact,
            "object/HSACO",
        )
        .unwrap();
        let error = require_exact_measurement(
            receipt.output_sha256(),
            receipt.output_bytes(),
            b"substituted-object",
            "object/HSACO",
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::ArtifactMismatch
        );
    }

    #[test]
    fn stale_source_preimage_cannot_match_admitted_identity() {
        let root = std::env::temp_dir().join(format!(
            "fe2o3-tutorial-source-{}-{}",
            std::process::id(),
            STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("package/src/lib.rs");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"pub fn kernel() {}\n").unwrap();
        let first = source_closure_preimage(&root, &root.join("package")).unwrap();
        fs::write(&source, b"pub fn kernel() { panic!() }\n").unwrap();
        let second = source_closure_preimage(&root, &root.join("package")).unwrap();
        assert_ne!(hex_sha256(&first), hex_sha256(&second));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn real_migration_manifest_is_admitted_without_requiring_promotion() {
        let fixture_id = "gfx942-fill-simulation";
        let snapshot = current_migration_fixture(&test_repository(), fixture_id);
        let repository = snapshot.path().canonicalize().unwrap();
        let manifest_bytes = fs::read(repository.join(MANIFEST_PATH)).unwrap();
        let manifest = parse_repository_document(&manifest_bytes, "tutorial manifest").unwrap();
        let manifest_object = manifest.as_object().unwrap();
        let fixture = find_fixture(
            manifest_object.get("compilerFixtures").unwrap(),
            fixture_id,
            "compiler fixtures",
        )
        .unwrap();
        let kernel = find_fixture(
            manifest_object.get("capabilityKernels").unwrap(),
            fixture_id,
            "capability kernels",
        )
        .unwrap();
        let matrix = fixture.get("matrix").unwrap().as_object().unwrap();
        let mut corpus = manifest_object.clone();
        corpus.remove("baseline");
        let simulator_payload = b"simulator-evidence";
        let mut request = serde_json::json!({
            "candidate": {
                "compilerCommit": "1111111111111111111111111111111111111111",
                "compilerTree": "2222222222222222222222222222222222222222",
                "worktreeClean": true,
            },
            "capabilityKernel": {
                "capabilityClosure": kernel.get("capabilityClosure").unwrap(),
                "kernelSymbol": kernel.get("kernelSymbol").unwrap(),
                "lessonIds": kernel.get("lessonIds").unwrap(),
                "requiredProperties": kernel.get("requiredProperties").unwrap(),
            },
            "fixture": {
                "compilerInput": fixture.get("compilerInput").unwrap(),
                "fixtureId": fixture_id,
                "hardwareCommand": {
                    "arguments": matrix.get("runnerArguments").unwrap(),
                    "environment": matrix.get("environment").unwrap(),
                    "executable": matrix.get("runnerPath").unwrap(),
                    "timeoutSeconds": 1200,
                    "workingDirectory": ".",
                },
                "hardwareLane": "mi300x",
                "hardwareReservation": "test-reservation",
                "target": fixture.get("target").unwrap(),
            },
            "hardwareReceiptChallenge": {
                "nonce": "3333333333333333333333333333333333333333333333333333333333333333",
                "reservationIdentity": "test-reservation",
                "schema": "fe2o3-tutorial-hardware-receipt-challenge-v1",
                "transportSchema": "fe2o3-tutorial-hardware-archive-v1",
            },
            "manifest": {
                "corpusContractSha256": domain_sha256(CORPUS_DOMAIN, &Value::Object(corpus)).unwrap(),
                "path": MANIFEST_PATH,
                "rawSha256": hex_sha256(&manifest_bytes),
            },
            "productionTransaction": {
                "allowsFallback": false,
                "allowsPipelineSelection": false,
                "pipelineEntry": PIPELINE_ENTRY,
                "policyVersion": 4,
            },
            "requestBindingSha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "roadmapIssue": ROADMAP_ISSUE,
            "schema": REQUEST_SCHEMA,
            "simulatorEvidence": object_reference(simulator_payload),
        });
        bind_request(&mut request);
        let request_bytes = canonical_document(&request).unwrap();
        let context = preflight_request(&repository, request_bytes.clone(), false).unwrap();
        assert_eq!(context.fixture_id, fixture_id);
        assert_eq!(context.target, "gfx942");
        assert!(!context.source_closure_preimage.is_empty());
        assert_eq!(kernel["productionCapabilityPath"]["status"], "legacy-only");
        assert_eq!(kernel["proofRequirements"]["status"], "missing");

        let input = &fixture["compilerInput"];
        for (relative, message) in [
            (
                input["packageManifest"].as_str().unwrap(),
                "package manifest or Cargo.lock changed after fixture admission",
            ),
            (
                input["cargoLockPath"].as_str().unwrap(),
                "package manifest or Cargo.lock changed after fixture admission",
            ),
            (
                input["sourcePaths"][0].as_str().unwrap(),
                "Rust source closure changed after fixture admission",
            ),
        ] {
            let path = repository.join(relative);
            let original = fs::read(&path).unwrap();
            let mut changed = original.clone();
            changed.push(b'\n');
            fs::write(&path, changed).unwrap();
            let error = preflight_request(&repository, request_bytes.clone(), false).unwrap_err();
            assert_eq!(
                error.code(),
                TutorialProductionTransactionErrorCodeV1::SourceMismatch
            );
            assert_eq!(error.message(), message);
            fs::write(path, original).unwrap();
        }
    }

    fn current_migration_fixture(
        source: &Path,
        fixture_id: &str,
    ) -> crate::test_temp_dir::TestTempDir {
        let snapshot = crate::test_temp_dir::TestTempDir::create("fe2o3-tutorial-migration");
        let repository = snapshot.path().canonicalize().unwrap();
        let mut manifest = parse_repository_document(
            &fs::read(source.join(MANIFEST_PATH)).unwrap(),
            "tutorial manifest",
        )
        .unwrap();
        let fixture = manifest["compilerFixtures"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|fixture| fixture["fixtureId"].as_str() == Some(fixture_id))
            .unwrap();
        let input = &mut fixture["compilerInput"];
        let package = PathBuf::from(input["packageManifest"].as_str().unwrap());
        let lock = PathBuf::from(input["cargoLockPath"].as_str().unwrap());
        let mut files = Vec::new();
        collect_rust_sources(&source.join(package.parent().unwrap()), &mut files).unwrap();
        files.extend([source.join(&package), source.join(&lock)]);
        for file in files {
            let destination = repository.join(file.strip_prefix(source).unwrap());
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(file, destination).unwrap();
        }

        // Match the input refresh tool without rewriting historical evidence or promotion state.
        input["packageManifestSha256"] =
            hex_sha256(&fs::read(repository.join(&package)).unwrap()).into();
        input["cargoLockSha256"] = hex_sha256(&fs::read(repository.join(&lock)).unwrap()).into();
        input["sourceClosureSha256"] = hex_sha256(
            &source_closure_preimage(&repository, &repository.join(package.parent().unwrap()))
                .unwrap(),
        )
        .into();
        let mut contract_input = input.as_object().unwrap().clone();
        contract_input.remove("contractSha256");
        let contract = serde_json::json!({
            "compilerInput": contract_input,
            "fixtureId": fixture["fixtureId"],
            "matrix": fixture["matrix"],
            "target": fixture["target"],
        });
        fixture["compilerInput"]["contractSha256"] = domain_sha256(FIXTURE_INPUT_DOMAIN, &contract)
            .unwrap()
            .into();
        let path = repository.join(MANIFEST_PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, canonical_document(&manifest).unwrap()).unwrap();
        snapshot
    }

    #[test]
    fn requested_fixture_drives_one_exact_protected_cargo_build() {
        use std::os::unix::fs::PermissionsExt;

        let repository = test_repository();
        let manifest_bytes = fs::read(repository.join(MANIFEST_PATH)).unwrap();
        let manifest = parse_repository_document(&manifest_bytes, "tutorial manifest").unwrap();
        let fixture = find_fixture(
            manifest
                .as_object()
                .unwrap()
                .get("compilerFixtures")
                .unwrap(),
            "gfx942-fill-simulation",
            "compiler fixtures",
        )
        .unwrap();
        let document = serde_json::json!({
            "fixture": {
                "compilerInput": fixture.get("compilerInput").unwrap(),
            },
        });
        let root = std::env::temp_dir().join(format!(
            "fe2o3-tutorial-protected-build-test-{}-{}",
            std::process::id(),
            STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let log = root.join("invocation.log");
        let executable = root.join("cargo-fe2o3");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf 'target=%s\\n' \"$FE2O3_TARGET\" > '{}'\nfor argument do printf 'arg=%s\\n' \"$argument\" >> '{}'; done\nexit 37\n",
                log.display(),
                log.display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();

        let error =
            run_protected_fixture_build_v1(&executable, &repository, "gfx942", &document, &root)
                .unwrap_err();
        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable
        );
        assert!(error.message().contains("exit status: 37"));
        let invocation = fs::read_to_string(log).unwrap();
        let expected_manifest = repository.join(
            fixture
                .get("compilerInput")
                .unwrap()
                .get("packageManifest")
                .unwrap()
                .as_str()
                .unwrap(),
        );
        for expected in [
            "target=gfx942".to_owned(),
            "arg=authority".to_owned(),
            "arg=release".to_owned(),
            "arg=build".to_owned(),
            "arg=--locked".to_owned(),
            format!("arg={}", expected_manifest.display()),
            "arg=--lib".to_owned(),
        ] {
            assert!(
                invocation.lines().any(|line| line == expected),
                "{expected}"
            );
        }
        assert_eq!(
            invocation
                .lines()
                .filter(|line| *line == "arg=build")
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repository_manifest_parser_rejects_duplicate_keys() {
        let error =
            parse_repository_document(b"{\"schema\":1,\"schema\":1}\n", "manifest").unwrap_err();
        assert_eq!(
            error.code(),
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson
        );
        assert!(error.message().contains("duplicate JSON key"));
    }

    #[test]
    fn semantic_kir_identity_is_not_an_archive_object_hash() {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("tutorial-identity-test");
        module.functions.push(Function::kernel_entry(
            "kernel",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "kernel",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        let kir = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();

        assert_ne!(sha256(kir.canonical_bytes()), *kir.identity().digest());
        for typed_identity in [
            "finalOptimizedKirSha256",
            "loweringIdentitySha256",
            "sourceMirToKirRefinementSha256",
            "machineRefinementSha256",
            "proofObligationSetSha256",
            "proofEvidenceSha256",
        ] {
            assert!(
                PRODUCTION_EVIDENCE_JOINS
                    .iter()
                    .all(|(claim, _)| *claim != typed_identity),
                "{typed_identity} must not be treated as a raw archive hash"
            );
        }
    }

    #[test]
    fn exact_compiler_evidence_package_rejects_mutation_and_omission() {
        let expected = test_compiler_evidence_package();
        validate_exact_evidence_package_v1(&expected, &expected).unwrap();

        let mut mutated = test_compiler_evidence_package();
        mutated.get_mut("artifact").unwrap()[0] ^= 1;
        assert_eq!(
            validate_exact_evidence_package_v1(&expected, &mutated)
                .unwrap_err()
                .code(),
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch
        );

        let mut omitted = test_compiler_evidence_package();
        omitted.remove("compiler-policy");
        assert_eq!(
            validate_exact_evidence_package_v1(&expected, &omitted)
                .unwrap_err()
                .code(),
            TutorialProductionTransactionErrorCodeV1::MissingEvidence
        );
    }

    #[test]
    fn pre_hardware_references_exclude_hardware_owned_identity() {
        let objects = test_compiler_evidence_package();
        let simulator = object_reference(b"external simulator observation");
        let mut references = objects
            .iter()
            .map(|(kind, payload)| (kind.clone(), object_reference(payload)))
            .collect::<Map<_, _>>();
        references.insert("simulator".to_owned(), simulator.clone());
        validate_evidence_references_v1(&Value::Object(references.clone()), &objects, &simulator)
            .unwrap();
        for hardware_owned in PENDING_EVIDENCE_KINDS {
            assert!(!references.contains_key(*hardware_owned));
        }

        references.remove("proof-checker");
        assert_eq!(
            validate_evidence_references_v1(&Value::Object(references), &objects, &simulator)
                .unwrap_err()
                .code(),
            TutorialProductionTransactionErrorCodeV1::MissingEvidence
        );
    }

    #[test]
    fn transaction_surface_has_no_caller_assembly_authority_or_stub() {
        let transaction = include_str!("production_pipeline_tutorial_transaction_v1.rs");
        let pipeline = include_str!("production_pipeline.rs");
        assert!(
            transaction
                .contains("pub fn prepare_tutorial_capability_qualification_transaction_v1(")
        );
        assert!(!transaction.contains("produce_tutorial_capability_qualification_transaction_v1"));
        assert!(!transaction.contains("_hardware_trust_policy"));
        assert!(pipeline.contains("prepare_tutorial_capability_qualification_transaction_v1"));
        assert!(!pipeline.contains("produce_tutorial_capability_qualification_transaction_v1"));
        assert!(!transaction.contains(concat!("FE2O3-TUTORIAL-TXN-", "015")));
        assert!(!transaction.contains(concat!("TutorialProductionTransaction", "AssemblyV1")));
        assert!(!pipeline.contains(concat!(
            "assemble_tutorial_capability_qualification_",
            "transaction_v1"
        )));
    }

    #[test]
    fn compiler_only_prepare_publishes_exact_export_without_hardware() {
        let parent = std::env::temp_dir().join(format!(
            "fe2o3-tutorial-prepare-test-{}-{}",
            std::process::id(),
            STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&parent).unwrap();
        let output = parent.join("prepared");
        let objects = test_compiler_evidence_package();
        let envelope = b"{\"schema\":\"test-pre-hardware\"}\n";

        publish_transaction_export(&output, PRE_HARDWARE_RESULT_NAME, envelope, &objects, None)
            .unwrap();

        assert_eq!(
            fs::read(output.join(PRE_HARDWARE_RESULT_NAME)).unwrap(),
            envelope
        );
        assert!(!output.join("hardware-archive-v1.zip").exists());
        for payload in objects.values() {
            let reference = object_reference(payload);
            assert_eq!(
                fs::read(output.join(reference["path"].as_str().unwrap())).unwrap(),
                *payload
            );
        }
        fs::remove_dir_all(parent).unwrap();
    }

    fn test_compiler_evidence_package() -> BTreeMap<String, Vec<u8>> {
        COMPILER_EVIDENCE_KINDS
            .iter()
            .map(|kind| (kind.to_string(), format!("exact:{kind}\n").into_bytes()))
            .collect()
    }

    fn minimal_bound_request(target: &str) -> Value {
        serde_json::json!({
            "candidate": {},
            "capabilityKernel": {
                "capabilityClosure": {},
                "kernelSymbol": "kernel",
                "lessonIds": ["lesson"],
                "requiredProperties": ["functional-refinement"],
            },
            "fixture": {
                "compilerInput": {},
                "fixtureId": "fixture",
                "hardwareCommand": {},
                "hardwareLane": "mi350",
                "hardwareReservation": "reservation",
                "target": target,
            },
            "hardwareReceiptChallenge": {},
            "manifest": {},
            "productionTransaction": {
                "allowsFallback": false,
                "allowsPipelineSelection": false,
                "pipelineEntry": PIPELINE_ENTRY,
                "policyVersion": 4,
            },
            "requestBindingSha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "roadmapIssue": ROADMAP_ISSUE,
            "schema": REQUEST_SCHEMA,
            "simulatorEvidence": {},
        })
    }

    fn bind_request(request: &mut Value) {
        let object = request.as_object_mut().unwrap();
        object.remove("requestBindingSha256");
        let binding = domain_sha256(REQUEST_DOMAIN, request).unwrap();
        request
            .as_object_mut()
            .unwrap()
            .insert("requestBindingSha256".to_owned(), Value::String(binding));
    }

    fn test_repository() -> PathBuf {
        let source = Path::new(file!());
        if source.is_absolute() {
            return source
                .ancestors()
                .find(|path| path.join(MANIFEST_PATH).is_file())
                .unwrap()
                .to_path_buf();
        }
        let current = std::env::current_dir().unwrap();
        current
            .ancestors()
            .map(|ancestor| ancestor.join(source))
            .find(|candidate| candidate.is_file())
            .and_then(|source| {
                source
                    .ancestors()
                    .find(|path| path.join(MANIFEST_PATH).is_file())
                    .map(Path::to_path_buf)
            })
            .unwrap()
    }
}
