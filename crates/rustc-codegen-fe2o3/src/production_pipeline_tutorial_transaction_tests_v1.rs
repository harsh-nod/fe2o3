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
    fn real_manifest_fixture_completes_source_and_contract_preflight() {
        let repository = test_repository();
        let manifest_bytes = fs::read(repository.join(MANIFEST_PATH)).unwrap();
        let manifest = parse_repository_document(&manifest_bytes, "tutorial manifest").unwrap();
        let manifest_object = manifest.as_object().unwrap();
        let fixture_id = "gfx942-fill-simulation";
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
        let context =
            preflight_request(&repository, canonical_document(&request).unwrap(), false).unwrap();
        assert_eq!(context.fixture_id, fixture_id);
        assert_eq!(context.target, "gfx942");
        assert!(!context.source_closure_preimage.is_empty());
        assert_eq!(
            unavailable_manifest_qualification_prerequisites_v1(&context).unwrap(),
            [
                "capability closure is not produced",
                "production capability path is not promoted",
                "proof requirement set is not complete",
                "capability-negative fixture coverage is unavailable",
            ]
        );
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
