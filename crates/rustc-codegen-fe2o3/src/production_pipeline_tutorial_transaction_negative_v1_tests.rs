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
    }
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
    let receipt = SourceNegativeReplayReceiptV1::run(&binding, &original).unwrap();
    receipt.verify_document(&receipt.document).unwrap();
    assert_eq!(receipt.document["cases"][0]["mutation"]["root"], 1);
    assert_eq!(
        receipt.document["cases"][0]["diagnostic"],
        "InvalidFunctionRole"
    );
    assert_eq!(receipt.document["qualificationStatus"], "incomplete");
    assert_eq!(receipt.document["sourceRustRecompiled"], false);
    assert_eq!(source.canonical_encoding(), original);
    assert_eq!(
        SourceNegativeReplayReceiptV1::run(&binding, &original)
            .unwrap()
            .document,
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
    let receipt =
        SourceNegativeReplayReceiptV1::run(&binding(&source, "first"), source.canonical_encoding())
            .unwrap();
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
    let receipt =
        SourceNegativeReplayReceiptV1::run(&original, source.canonical_encoding()).unwrap();
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
        let independent =
            SourceNegativeReplayReceiptV1::run(&changed, source.canonical_encoding()).unwrap();
        assert!(independent.verify_document(&receipt.document).is_err());
    }
    let other = source_fixture(&["other"]);
    assert!(SourceNegativeReplayReceiptV1::run(&original, other.canonical_encoding()).is_err());
    let independent =
        SourceNegativeReplayReceiptV1::run(&binding(&other, "other"), other.canonical_encoding())
            .unwrap();
    assert!(independent.verify_document(&receipt.document).is_err());
}

#[test]
fn invalid_positive_control_and_missing_root_are_not_negative_successes() {
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    let mut corrupted = source.canonical_encoding().to_vec();
    corrupted[0] ^= 1;
    binding.semantic_mir = sha256(&corrupted);
    let error = SourceNegativeReplayReceiptV1::run(&binding, &corrupted).unwrap_err();
    assert!(error.message().contains("positive control failed"));
    binding.semantic_mir = sha256(source.canonical_encoding());
    binding.kernel_symbol = "absent".to_owned();
    assert!(SourceNegativeReplayReceiptV1::run(&binding, source.canonical_encoding()).is_err());
}

#[test]
fn oversized_receipt_binding_fails_closed() {
    let source = source_fixture(&["first"]);
    let mut binding = binding(&source, "first");
    binding.fixture_id = "f".repeat(MAX_RECEIPT_BYTES);
    let error =
        SourceNegativeReplayReceiptV1::run(&binding, source.canonical_encoding()).unwrap_err();
    assert!(error.message().contains("byte bound"));
}

#[test]
fn partial_source_replay_cannot_satisfy_the_full_production_negative_roster() {
    let source = source_fixture(&["first"]);
    let receipt =
        SourceNegativeReplayReceiptV1::run(&binding(&source, "first"), source.canonical_encoding())
            .unwrap();
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
