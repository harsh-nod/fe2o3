use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn source_fixture(exports: &[&str]) -> AdmittedInertSemanticMirV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let functions = exports
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            let tag = 21 + index as u8 * 20;
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256([tag + 1; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag + 2; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 3; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 4; 32]),
                source,
                SemanticFunctionAbiV1::from_rustc(
                    SemanticAbiIdentityV1::from_sha256([tag + 5; 32]),
                    SemanticLayoutIdentityV1::from_sha256([250; 32]),
                    SemanticCanonAbiV1::GpuKernel,
                    SemanticExternAbiV1::GpuKernel,
                    false,
                    false,
                    0,
                    vec![],
                    SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
                )
                .unwrap(),
                vec![SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 6; 32]),
                    unit,
                    SemanticLocalRoleV1::Return,
                    source,
                )],
                SemanticBlockIdV1::from_index(0),
                vec![
                    SemanticBasicBlockV1::new(
                        SemanticBlockIdentityV1::from_sha256([tag + 7; 32]),
                        source,
                        vec![],
                        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(symbol.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([tag + 8; 32]),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            ))
        })
        .collect();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![unit_type],
        vec![],
        vec![],
        vec![],
        functions,
        (0..exports.len())
            .map(|index| SemanticFunctionIdV1::from_index(index as u32))
            .collect(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn binding(source: &AdmittedInertSemanticMirV1, symbol: &str) -> SourceNegativeReplayBindingV1 {
    // Test-only coordinates exercise replay. They cannot construct recovered V5 custody.
    SourceNegativeReplayBindingV1 {
        request_binding: hex32([1; 32]),
        fixture_id: "gfx942-fixture".to_owned(),
        target: "gfx942".to_owned(),
        kernel_symbol: symbol.to_owned(),
        source_closure: [2; 32],
        compiler_input: [3; 32],
        sealed_result: [4; 32],
        transaction: [5; 32],
        compiler_policy: [6; 32],
        semantic_mir: sha256(source.canonical_encoding()),
        executable_kir: sha256(&kir_fixture(symbol)),
        declaration_roster: required_roster(&declaration_fixture()).unwrap(),
    }
}

fn kir_fixture(symbol: &str) -> Vec<u8> {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
        Terminator,
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("negative-admission-test");
    module.functions.push(Function::kernel_entry(
        symbol,
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        symbol,
        symbol,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    VerifiedCanonicalKernelIrV13::from_module(module)
        .unwrap()
        .canonical_bytes()
        .to_vec()
}

fn declaration_fixture() -> Value {
    serde_json::json!({
        "fixtureId": "gfx942-fixture",
        "target": "gfx942",
        "cases": [{
            "caseId": "gfx942-fixture--abi",
            "category": "abi",
            "fixtureId": "gfx942-fixture",
            "target": "gfx942",
            "fixtureBindingSha256": hex32([8; 32]),
            "mutation": { "class": "abi-layout" },
        }],
    })
}

fn required_roster(fixture: &Value) -> ResultV1<recipes::RequiredNegativeRosterV1> {
    recipes::RequiredNegativeRosterV1::from_fixture(
        &canonical_document(fixture)?,
        fixture,
        hex32([8; 32]),
    )
}

fn run_fixture(
    binding: &SourceNegativeReplayBindingV1,
    original: &[u8],
) -> ResultV1<SourceNegativeReplayReceiptV1> {
    SourceNegativeReplayReceiptV1::run(binding, original, &kir_fixture(&binding.kernel_symbol))
}

fn rebind(document: &mut Value) {
    document.as_object_mut().unwrap().remove("replayRunSha256");
    let identity = domain_sha256(RUN_DOMAIN, document).unwrap();
    document["replayRunSha256"] = Value::String(identity);
}

#[test]
fn source_generated_mutation_observes_exact_rejection_without_qualification_authority() {
    let source = source_fixture(&["first", "second"]);
    let binding = binding(&source, "second");
    let original = source.canonical_encoding().to_vec();
    let receipt = run_fixture(&binding, &original).unwrap();
    receipt.verify_document(&receipt.document).unwrap();
    assert_eq!(receipt.document["cases"][0]["mutation"]["root"], 1);
    assert_eq!(
        receipt.document["cases"][0]["diagnostic"],
        "InvalidFunctionRole"
    );
    assert_eq!(receipt.document["qualificationStatus"], "incomplete");
    assert_eq!(receipt.document["sourceRustRecompiled"], false);
    assert_eq!(receipt.document["cases"].as_array().unwrap().len(), 3);
    assert_eq!(
        receipt.document["cases"][1]["diagnostic"],
        "InvalidFunctionAbi"
    );
    assert_eq!(receipt.document["cases"][1]["mutation"]["root"], 1);
    assert_eq!(receipt.document["cases"][2]["diagnostic"], "UnknownCallee");
    assert_eq!(
        receipt.document["cases"][2]["mutation"]["function"],
        "second"
    );
    assert_eq!(receipt.document["requiredCasesSatisfied"], 0);
    assert_eq!(
        receipt.document["requiredCases"][0]["status"],
        "not-executed"
    );
    for case in receipt.document["cases"].as_array().unwrap() {
        assert_eq!(case["coverage"], "supplemental-not-declared-mutation");
    }
    assert_eq!(source.canonical_encoding(), original);
    assert_eq!(
        run_fixture(&binding, &original).unwrap().document,
        receipt.document
    );

    let error = omit_root(&source, SemanticFunctionIdV1::from_index(1))
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap_err();
    assert_eq!(
        error,
        SemanticMirErrorV1::InvalidFunctionRole {
            function: SemanticFunctionIdV1::from_index(1),
            role: SemanticFunctionRoleV1::KernelRoot,
            rooted: false,
        }
    );
}

#[test]
fn canonical_self_assertions_cannot_replace_live_negative_observation() {
    let source = source_fixture(&["first"]);
    let receipt = run_fixture(&binding(&source, "first"), source.canonical_encoding()).unwrap();
    let mutations = [
        ("/cases", serde_json::json!([])),
        (
            "/cases/0/diagnostic",
            Value::String("InvalidTypeLayout".to_owned()),
        ),
        ("/cases/0/status", Value::String("passed".to_owned())),
        ("/cases/0/mutation/root", Value::from(1)),
        ("/qualificationStatus", Value::String("complete".to_owned())),
        ("/sourceRustRecompiled", Value::Bool(true)),
        ("/requiredCasesSatisfied", Value::from(1)),
        (
            "/requiredCases/0/status",
            Value::String("passed".to_owned()),
        ),
        ("/cases/1/mutation/root", Value::from(1)),
        ("/cases/2/mutation/operation", Value::from(1)),
        (
            "/cases/2/coverage",
            Value::String("declared-mutation".to_owned()),
        ),
        (
            "/binding/recipeSourceSha256",
            Value::String(hex32([99; 32])),
        ),
    ];
    for (path, value) in mutations {
        let mut changed = receipt.document.clone();
        *changed.pointer_mut(path).unwrap() = value;
        rebind(&mut changed);
        assert_eq!(
            receipt.verify_document(&changed).unwrap_err().code(),
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            "{path}"
        );
    }
    let mut duplicate = receipt.document.clone();
    duplicate["cases"]
        .as_array_mut()
        .unwrap()
        .push(receipt.document["cases"][0].clone());
    rebind(&mut duplicate);
    assert!(receipt.verify_document(&duplicate).is_err());
}

#[test]
fn source_and_cross_run_substitutions_do_not_replay() {
    let source = source_fixture(&["first"]);
    let original = binding(&source, "first");
    let receipt = run_fixture(&original, source.canonical_encoding()).unwrap();
    let mut replacements = Vec::new();
    for index in 0..7 {
        let mut changed = original.clone();
        match index {
            0 => changed.request_binding = hex32([9; 32]),
            1 => changed.fixture_id.push_str("-other"),
            2 => changed.source_closure[0] ^= 1,
            3 => changed.compiler_input[0] ^= 1,
            4 => changed.sealed_result[0] ^= 1,
            5 => changed.transaction[0] ^= 1,
            6 => changed.compiler_policy[0] ^= 1,
            _ => unreachable!(),
        }
        replacements.push(changed);
    }
    for changed in replacements {
        let independent = run_fixture(&changed, source.canonical_encoding()).unwrap();
        assert!(independent.verify_document(&receipt.document).is_err());
    }
    let other = source_fixture(&["other"]);
    assert!(run_fixture(&original, other.canonical_encoding()).is_err());
    let independent = run_fixture(&binding(&other, "other"), other.canonical_encoding()).unwrap();
    assert!(independent.verify_document(&receipt.document).is_err());
}

#[test]
fn invalid_positive_control_and_missing_root_are_not_negative_successes() {
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    let mut corrupted = source.canonical_encoding().to_vec();
    corrupted[0] ^= 1;
    binding.semantic_mir = sha256(&corrupted);
    let error = run_fixture(&binding, &corrupted).unwrap_err();
    assert!(error.message().contains("positive control failed"));
    binding.semantic_mir = sha256(source.canonical_encoding());
    binding.kernel_symbol = "absent".to_owned();
    assert!(run_fixture(&binding, source.canonical_encoding()).is_err());
}

#[test]
fn oversized_receipt_binding_fails_closed() {
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    binding.fixture_id = "f".repeat(MAX_RECEIPT_BYTES);
    let error = run_fixture(&binding, source.canonical_encoding()).unwrap_err();
    assert!(error.message().contains("byte bound"));
}

#[test]
fn partial_source_replay_cannot_satisfy_the_full_production_negative_roster() {
    let source = source_fixture(&["first"]);
    let receipt = run_fixture(&binding(&source, "first"), source.canonical_encoding()).unwrap();
    let error = validate_production_negative_fixture_receipt_v1(
        &receipt.document,
        &canonical_document(&receipt.document).unwrap(),
    )
    .unwrap_err();
    assert_eq!(
        error.code(),
        TutorialProductionTransactionErrorCodeV1::ProtectedCompletionUnavailable
    );
    assert!(PENDING_EVIDENCE_KINDS.contains(&"negative-fixture-set"));
}

#[test]
fn malformed_or_substituted_kir_is_not_a_negative_observation() {
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    let kir = kir_fixture("first");
    let mut malformed = kir.clone();
    malformed[0] ^= 1;
    assert!(
        SourceNegativeReplayReceiptV1::run(&binding, source.canonical_encoding(), &malformed)
            .is_err()
    );
    binding.executable_kir = sha256(&malformed);
    let error =
        SourceNegativeReplayReceiptV1::run(&binding, source.canonical_encoding(), &malformed)
            .unwrap_err();
    assert!(error.message().contains("KIR positive control failed"));
    let other = kir_fixture("other");
    binding.executable_kir = sha256(&other);
    let error = SourceNegativeReplayReceiptV1::run(&binding, source.canonical_encoding(), &other)
        .unwrap_err();
    assert!(error.message().contains("lacks exact kernel"));
}

#[test]
fn colliding_unresolved_call_recipe_does_not_credit_another_diagnostic() {
    use fe2o3_kernel_ir::{Function, Signature};
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    let (_, mut module) =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(kir_fixture("first"))
            .unwrap();
    module.functions.push(Function::declaration(
        "__fe2o3_negative_unresolved_callee_v1",
        Signature::new(vec![], vec![]),
    ));
    let kir = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    binding.executable_kir = sha256(kir.canonical_bytes());
    let error = SourceNegativeReplayReceiptV1::run(
        &binding,
        source.canonical_encoding(),
        kir.canonical_bytes(),
    )
    .unwrap_err();
    assert!(
        error
            .message()
            .contains("already exists in positive control")
    );
}

#[test]
fn required_roster_rejects_cross_fixture_duplicate_and_excess_cases() {
    for (path, value) in [
        (
            "/cases/0/fixtureId",
            Value::String("another-fixture".to_owned()),
        ),
        ("/cases/0/target", Value::String("gfx950".to_owned())),
        (
            "/cases/0/fixtureBindingSha256",
            Value::String(hex32([9; 32])),
        ),
        ("/cases/0/caseId", Value::String("x".repeat(257))),
        ("/cases", serde_json::json!([])),
    ] {
        let mut fixture = declaration_fixture();
        *fixture.pointer_mut(path).unwrap() = value;
        assert!(required_roster(&fixture).is_err(), "{path}");
    }
    let mut duplicate = declaration_fixture();
    duplicate["cases"] = serde_json::json!([duplicate["cases"][0], duplicate["cases"][0]]);
    assert!(required_roster(&duplicate).is_err());
    let mut oversized = declaration_fixture();
    let case = oversized["cases"][0].clone();
    oversized["cases"] = Value::Array(
        (0..65)
            .map(|index| {
                let mut case = case.clone();
                case["caseId"] = Value::String(format!("case-{index}"));
                case
            })
            .collect(),
    );
    assert!(required_roster(&oversized).is_err());
}

#[test]
fn changed_required_case_declaration_cannot_replay_an_existing_receipt() {
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    let old = run_fixture(&binding, source.canonical_encoding()).unwrap();
    let mut declaration = declaration_fixture();
    declaration["cases"][0]["mutation"]["class"] = Value::String("different-mutation".to_owned());
    binding.declaration_roster = required_roster(&declaration).unwrap();
    let current = run_fixture(&binding, source.canonical_encoding()).unwrap();
    assert!(current.verify_document(&old.document).is_err());
}

#[test]
fn repository_required_rosters_bind_all_47_fixtures_without_claiming_766_passes() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let manifest = parse_repository_document(
        &read_repository_file(&repository, MANIFEST_PATH, MAX_JSON_BYTES).unwrap(),
        "test manifest",
    )
    .unwrap();
    let fixtures = manifest["compilerFixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 47);
    let mut count = 0;
    for fixture in fixtures {
        let fixture_id = fixture["fixtureId"].as_str().unwrap();
        let kernel = find_fixture(
            &manifest["capabilityKernels"],
            fixture_id,
            "test capability kernels",
        )
        .unwrap();
        let context = RequestContext {
            document: serde_json::json!({ "capabilityKernel": kernel }),
            canonical_bytes: vec![],
            fixture_id: fixture_id.to_owned(),
            target: fixture["target"].as_str().unwrap().to_owned(),
            kernel_symbol: kernel["kernelSymbol"].as_str().unwrap().to_owned(),
            request_binding_sha256: hex32([1; 32]),
            source_closure_preimage: vec![],
            simulator_command_sha256: hex32([2; 32]),
            hardware_command_sha256: hex32([3; 32]),
            hardware_lane: "test-only-no-execution".to_owned(),
            hardware_timeout_seconds: 1,
            repository: repository.clone(),
        };
        let digest = |field: &str| {
            let text = fixture["compilerInput"][field].as_str().unwrap();
            std::array::from_fn(|index| {
                u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap()
            })
        };
        let roster = recipes::RequiredNegativeRosterV1::load(
            &context,
            digest("sourceClosureSha256"),
            digest("contractSha256"),
        )
        .unwrap();
        let pending = roster.pending_cases();
        assert!(pending.iter().all(|case| case["status"] == "not-executed"));
        count += pending.len();
        assert!(
            recipes::RequiredNegativeRosterV1::load(&context, [0; 32], digest("contractSha256"))
                .is_err()
        );
        assert!(
            recipes::RequiredNegativeRosterV1::load(
                &context,
                digest("sourceClosureSha256"),
                [0; 32]
            )
            .is_err()
        );
    }
    assert_eq!(count, 766);
}
