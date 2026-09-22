use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn local(id: u32) -> Value {
    Value::Local(Id::new(id))
}
fn memory_kernel(index: u64) -> Kernel {
    Kernel::new(
        "recipe_replay",
        0,
        vec![Block::new(
            vec![
                Op::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                Op::ViewInSpace {
                    result: Id::new(0),
                    element_width: 32,
                    writable: false,
                    shape: vec![16],
                    dynamic_extents: vec![],
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                Op::IndexConstant {
                    result: Id::new(1),
                    value: index,
                },
                Op::Access {
                    kind: AccessKindAttr::Read,
                    view: local(0),
                    indices: vec![local(1)],
                },
            ],
            Term::Return,
        )],
    )
    .unwrap()
}

#[test]
fn decoded_canonical_recipe_reenters_the_unchanged_complete_ranked_pipeline() {
    let source = memory_kernel(3);
    let decoded = tests::roundtrip(&source);
    let original = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("recipe_module", source).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    let reconstructed = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("recipe_module", decoded.kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(original.all_mandatory_reports_are_clean());
    assert!(reconstructed.all_mandatory_reports_are_clean());
    assert_eq!(reconstructed.kernel(), original.kernel());
    assert_eq!(
        reconstructed.exact_graph_identity(),
        original.exact_graph_identity()
    );
    assert_eq!(
        reconstructed.production_pipeline_report().pass_order(),
        &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    );
    assert!(
        reconstructed
            .retained_policy_checked_refinement_staging()
            .is_empty()
    );
    assert!(reconstructed.pass_preservation_report().is_exact_identity());
}

#[test]
fn decoded_out_of_bounds_recipe_is_not_promoted_by_syntax_or_constructor_success() {
    let decoded = tests::roundtrip(&memory_kernel(16));
    let error = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("refused_recipe", decoded.kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBounds(_))
    ));
}

#[test]
fn zero_and_arbitrary_decoded_receipt_claims_never_synthesize_imported_staging() {
    for identity in [0, 0x5a] {
        let request = variant_tests::proof(identity);
        let source = Kernel::new(
            "unstaged_request",
            0,
            vec![Block::new(
                vec![
                    Op::SemanticConstant {
                        result: Id::new(0),
                        value: 1,
                    },
                    Op::SemanticConstant {
                        result: Id::new(1),
                        value: 1,
                    },
                    Op::RequireAuthenticatedReferenceEquivalent {
                        actual: local(0),
                        expected: local(1),
                        proof: request,
                    },
                ],
                Term::Return,
            )],
        )
        .unwrap();
        let decoded = tests::roundtrip(&source);
        let Op::RequireAuthenticatedReferenceEquivalent { proof, .. } =
            &decoded.kernel.blocks[0].operations[2]
        else {
            panic!("request shape");
        };
        assert_eq!(*proof, request);
        assert_eq!(
            proof.receipt_identity().digest(),
            variant_tests::digest(identity)
        );
        let error = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("unstaged_module", decoded.kernel).unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedRecipe(
                ProductionRankedKernelErrorV1::Materialization(
                    "policy-checked functional-refinement staging was not retained"
                )
            ))
        ));
    }
}

fn wire_expression(payload: &[u8], nodes: u32) -> Vec<u8> {
    let mut bytes = tests::literal_empty();
    bytes.truncate(57);
    bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
    bytes[32..36].copy_from_slice(&nodes.to_le_bytes());
    bytes[53..57].copy_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&[31, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(payload);
    bytes.extend_from_slice(&[1, 0]); // Exact bitvector numerical contract.
    bytes.extend_from_slice(&[11, 0, 0, 0]);
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    bytes
}
fn scan_refuses(bytes: &[u8], field: &'static str, actual: usize, limit: usize) {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 8 * tests::FIXTURE_FLOOR);
    budget.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
    assert!(matches!(read_ranked_recipe_v1(bytes, &mut budget),
        Err(E::Limit { field: got, actual: a, limit: l }) if (got, a, l) == (field, actual, limit)));
    assert_eq!(budget.storage(), tests::FIXTURE_FLOOR);
}
#[test]
fn actual_expression_depth_and_node_counts_refuse_before_typed_construction() {
    let mut unary = Vec::new();
    for _ in 0..128 {
        unary.extend_from_slice(&[4, 0, 1, 0, 1, 0]);
    }
    unary.extend_from_slice(&[1, 0, 0, 0, 0, 0, 1, 0]);
    scan_refuses(&wire_expression(&unary, 129), "expression depth", 129, 128);
    fn tree(depth: usize, bytes: &mut Vec<u8>) {
        if depth == 0 {
            bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 1, 0]);
        } else {
            bytes.extend_from_slice(&[5, 0, 1, 0, 1, 0, 1, 0]);
            tree(depth - 1, bytes);
            tree(depth - 1, bytes);
        }
    }
    let mut large = vec![4, 0, 1, 0, 1, 0, 4, 0, 1, 0, 1, 0];
    tree(12, &mut large); // 8191 tree nodes + two unary roots.
    scan_refuses(
        &wire_expression(&large, 8193),
        "expression nodes",
        8193,
        8192,
    );
}

#[test]
fn oversized_nested_count_refuses_without_allocating_or_trusting_enclosing_counts() {
    let mut bytes = tests::literal_empty();
    bytes.truncate(57);
    bytes[28..32].copy_from_slice(&1u32.to_le_bytes());
    bytes[53..57].copy_from_slice(&1u32.to_le_bytes());
    // DeterministicJoin result0 and impossible length; the header/body are otherwise complete.
    bytes.extend_from_slice(&[11, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&[11, 0, 0, 0]);
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    scan_refuses(
        &bytes,
        "values",
        u32::MAX as usize,
        MAX_DETERMINISTIC_JOIN_INPUTS_V1,
    );
}

#[test]
fn decoder_identity_and_full_bytes_are_both_checked_not_just_a_recipe_digest() {
    let first = memory_kernel(3);
    let second = memory_kernel(4);
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 8 * tests::FIXTURE_FLOOR);
    budget.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
    let (a, ar) = encode_ranked_recipe_v1(&first, &mut budget).unwrap();
    budget.reserve_storage(ar.retained_storage()).unwrap();
    let (b, br) = encode_ranked_recipe_v1(&second, &mut budget).unwrap();
    budget.reserve_storage(br.retained_storage()).unwrap();
    let (view, vr) = read_ranked_recipe_v1(a.canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(vr.retained_storage()).unwrap();
    let forged = RankedRecipeRefV1 {
        bytes: b.canonical_bytes(),
        shape: view.shape,
        identity: view.identity,
    };
    budget
        .reserve_storage(size_of::<RankedRecipeRefV1<'_>>())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        materialize_ranked_recipe_v1(&forged, &mut budget),
        Err(E::NonCanonical)
    ));
    assert_eq!(budget.storage(), floor);
}

// Model-only semantic owner, adapted mechanically from the existing Middle V5 fixture.
// It is not rustc capture or source-proof custody.
mod middle_model {
    use crate::{ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ShellLimits};
    use dialect_mir::pliron::MirProductionPlironLimitsV1;
    use fe2o3_mir_model::semantic_mir_v1::*;
    fn bytes(value: u8) -> [u8; 32] {
        [value; 32]
    }
    fn unit_type() -> SemanticTypeDeclV1 {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(4)),
            SemanticLayoutIdentityV1::from_sha256(bytes(4)),
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
        )
    }

    fn semantic_function() -> SemanticFunctionDeclV1 {
        let type_id = SemanticTypeIdV1::from_index(0);
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(2)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(10)),
            SemanticSourceProvenanceV1::unavailable(),
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            )],
            SemanticTerminatorV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTerminatorKindV1::Return,
            ),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(2)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(2)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(2)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(3)),
                type_id,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            )],
            SemanticBlockIdV1::from_index(0),
            vec![block],
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"middle_end_evidence_test".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(bytes(5)),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        None,
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ))
    }

    pub(super) fn semantic_owner() -> ProductionSemanticMirOwnerV1 {
        let admitted = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
            vec![unit_type()],
            vec![],
            vec![],
            vec![],
            vec![semantic_function()],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::new(
                ShellLimits::default(),
                MirProductionPlironLimitsV1::default(),
            ),
        )
        .unwrap()
    }
}

#[test]
fn complete_middle_v5_evidence_is_identical_after_inert_recipe_reconstruction() {
    let kernel = memory_kernel(3);
    let decoded = tests::roundtrip(&kernel);
    let original = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("middle_recipe", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    let reconstructed = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("middle_recipe", decoded.kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    let semantic = middle_model::semantic_owner();
    // Caller diagnostic text is held identical; neither this string nor the
    // model semantic owner is claimed to be genuine source/execution evidence.
    let diagnostic = "func @recipe_replay {\n  kernel.return\n}\n";
    let expected =
        crate::ProductionMiddleEndEvidenceV5::try_new(&semantic, &original, diagnostic).unwrap();
    let actual =
        crate::ProductionMiddleEndEvidenceV5::try_new(&semantic, &reconstructed, diagnostic)
            .unwrap();
    assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
    let replay =
        crate::InertProductionMiddleEndEvidenceV5::decode(actual.canonical_bytes()).unwrap();
    assert_eq!(replay.canonical_bytes(), expected.canonical_bytes());
}

#[test]
fn unbound_request_keeps_its_exact_compile_refusal_after_decode() {
    let kernel = Kernel::new(
        "unbound_request",
        0,
        vec![Block::new(
            vec![
                Op::SemanticConstant {
                    result: Id::new(0),
                    value: 1,
                },
                Op::RequestAuthenticatedReferenceEquivalent {
                    actual: local(0),
                    expected: local(0),
                    subjects: variant_tests::subjects(),
                },
            ],
            Term::Return,
        )],
    )
    .unwrap();
    let decoded = tests::roundtrip(&kernel);
    let error = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("unbound_module", decoded.kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedRecipe(
            ProductionRankedKernelErrorV1::Materialization(
                "unbound functional-refinement request cannot be materialized"
            )
        ))
    ));
}
