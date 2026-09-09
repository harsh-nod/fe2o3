use super::super::resource_tests::{
    checked_execution_view_tests::transparent_result_wrapper_owner,
    guarded_value_tests::admitted_global_load_owner, helper_closure_semantic_owner_with_calls,
    noop_ranked_root, noop_semantic_owner,
};
use super::*;
use fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

fn lower(source: ProductionSemanticMirOwnerV1) -> ProductionSemanticKirOwnerV1 {
    ProductionSemanticKirOwnerV1::try_lower(source, ProductionSemanticKirLimitsV1::default())
        .unwrap()
}

fn reports(
    owner: &ProductionSemanticKirOwnerV1,
) -> Vec<(SemanticFunctionIdV1, SemanticU32InductionNoOverflowReportV1)> {
    owner
        .semantic()
        .semantic()
        .roots()
        .iter()
        .map(|&root| {
            (
                root,
                analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
                    owner.semantic().semantic(),
                    owner.semantic_ssa().execution_expansion(),
                    root,
                    SemanticU32InductionAnalysisLimitsV1::default(),
                )
                .unwrap(),
            )
        })
        .collect()
}

fn evidence(
    owner: &ProductionSemanticKirOwnerV1,
) -> InertCanonicalMirToKirCorrespondenceEvidenceV6 {
    let reports = reports(owner);
    InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(
        owner,
        &reports
            .iter()
            .map(|(root, report)| (*root, report))
            .collect::<Vec<_>>(),
        SemanticU32InductionAnalysisLimitsV1::default(),
    )
    .unwrap()
}

fn replay<'a>(
    e: &'a InertCanonicalMirToKirCorrespondenceEvidenceV6,
    owner: &'a ProductionSemanticKirOwnerV1,
) -> Result<ReplayedMirToKirCorrespondenceV6<'a>> {
    e.verify_replay(owner, SemanticU32InductionAnalysisLimitsV1::default())
}

#[test]
fn repeated_calls_compose_distinct_source_instances_with_exact_kir_spans() {
    let owner = lower(helper_closure_semantic_owner_with_calls(2));
    let e = evidence(&owner);
    let decoded =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(e.canonical_bytes()).unwrap();
    assert_eq!(e, decoded);
    let checked = replay(&decoded, &owner).unwrap();
    assert!(std::ptr::eq(checked.owner(), &owner));
    assert_eq!(checked.induction_reports(), reports(&owner));
    assert!(!e.grants_authority());
    assert!(!checked.grants_authority());
    assert_eq!(e.execution_correspondence(), owner.correspondence());
    let view = &e.expansion().roots()[0];
    assert_eq!(view.instances().len(), 3);
    assert_eq!(
        view.instances()[1].function(),
        view.instances()[2].function()
    );
    assert_ne!(
        view.instances()[1].call_block(),
        view.instances()[2].call_block()
    );
    for row in e.execution_correspondence().blocks() {
        let origin = &view.block_origins()[row.semantic_block().index() as usize];
        let instance = &view.instances()[origin.instance().index() as usize];
        assert_eq!(origin.function(), instance.function());
    }
    assert!(matches!(
        e.roots()[0].induction(),
        MirToKirInductionEvidenceV6::Expanded(_)
    ));
    let original =
        analyze_semantic_u32_induction_no_overflow_v1(owner.semantic().semantic(), view.root())
            .unwrap();
    assert!(
        InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(
            &owner,
            &[(view.root(), &original)],
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .is_err()
    );
    assert!(
        crate::InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(&owner, &original)
            .is_err()
    );
    assert!(
        crate::InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(&owner, &original)
            .is_err()
    );
    assert!(
        InertCanonicalSemanticU32InductionEvidenceV1::from_report(&reports(&owner)[0].1).is_err()
    );
}

fn rebuild_function(
    original: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap();
    match original.kernel_entry() {
        Some(entry) => function.with_kernel_entry(entry.clone()),
        None => function,
    }
}

fn call_block(
    source: SemanticSourceProvenanceV1,
    identity: u8,
    callee: u32,
    target: u32,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([identity; 32]),
        source,
        vec![],
        SemanticTerminatorV1::new(
            source,
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new(
                    SemanticFunctionIdV1::from_index(callee),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(0),
                            vec![],
                            SemanticTypeIdV1::from_index(0),
                        )
                        .unwrap(),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(target),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap()
}

fn nested_source() -> ProductionSemanticMirOwnerV1 {
    let baseline = helper_closure_semantic_owner_with_calls(2);
    let source = baseline.semantic();
    let helper = &source.functions()[1];
    let parent = rebuild_function(
        helper,
        vec![
            call_block(helper.source(), 217, 2, 1),
            helper.blocks()[0].clone(),
        ],
    );
    let leaf = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([221; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([222; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([224; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([225; 32]),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        helper.entry(),
        helper.blocks().to_vec(),
    )
    .unwrap();
    admit_like(
        source,
        vec![source.functions()[0].clone(), parent, leaf],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
}

fn admit_like(
    source: &AdmittedInertSemanticMirV1,
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> ProductionSemanticMirOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        roots,
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn nested_repeated_calls_retain_parent_and_original_function_identity() {
    let owner = lower(nested_source());
    let e = evidence(&owner);
    replay(&e, &owner).unwrap();
    let view = &e.expansion().roots()[0];
    assert_eq!(view.instances().len(), 5);
    let leaves = view
        .instances()
        .iter()
        .filter(|i| i.depth() == 2)
        .collect::<Vec<_>>();
    assert_eq!(leaves.len(), 2);
    assert_ne!(leaves[0].parent(), leaves[1].parent());
    assert_eq!(leaves[0].function_identity(), leaves[1].function_identity());
}

#[test]
fn result_wrapper_keeps_physical_root_selected_body_and_identity_view_distinct() {
    let owner = lower(transparent_result_wrapper_owner());
    let e = evidence(&owner);
    replay(&e, &owner).unwrap();
    let root = &e.roots()[0];
    assert_ne!(root.root(), root.source_body());
    assert_eq!(root.root(), SemanticFunctionIdV1::from_index(0));
    assert_eq!(root.source_body(), SemanticFunctionIdV1::from_index(1));
    assert!(matches!(
        root.induction(),
        MirToKirInductionEvidenceV6::Original(_)
    ));
    assert_eq!(
        owner.module().kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
}

fn mixed_roots_owner() -> ProductionSemanticKirOwnerV1 {
    let baseline = helper_closure_semantic_owner_with_calls(2);
    let plain = noop_semantic_owner(&["plain_root"]);
    let source = baseline.semantic();
    let root = &source.functions()[0];
    let called = rebuild_function(
        root,
        vec![
            call_block(root.source(), 208, 2, 1),
            call_block(root.source(), 209, 2, 2),
            root.blocks()[2].clone(),
        ],
    );
    let source = admit_like(
        source,
        vec![
            plain.semantic().functions()[0].clone(),
            called,
            source.functions()[1].clone(),
        ],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    );
    let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(source,
        vec![noop_ranked_root(SemanticFunctionIdV1::from_index(0), "plain_root"),
            noop_ranked_root(SemanticFunctionIdV1::from_index(1), "helper_root")],
    ).unwrap();
    ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn complete_mixed_roster_replays_each_root_with_its_own_coordinate_policy() {
    let owner = mixed_roots_owner();
    let e = evidence(&owner);
    assert!(owner.retains_mandatory_generic_checks());
    assert_eq!(replay(&e, &owner).unwrap().induction_reports().len(), 2);
    assert!(matches!(
        e.roots()[0].induction(),
        MirToKirInductionEvidenceV6::Original(_)
    ));
    assert!(matches!(
        e.roots()[1].induction(),
        MirToKirInductionEvidenceV6::Expanded(_)
    ));
    let reports = reports(&owner);
    let a = (reports[0].0, &reports[0].1);
    let b = (reports[1].0, &reports[1].1);
    for bad in [vec![], vec![a], vec![a, a], vec![b, a], vec![a, b, b]] {
        assert!(matches!(
            InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(
                &owner,
                &bad,
                SemanticU32InductionAnalysisLimitsV1::default(),
            ),
            Err(ProductionCorrespondenceEvidenceErrorV6::RootRoster)
        ));
    }
}

fn reject_mutation(
    owner: &ProductionSemanticKirOwnerV1,
    mutate: impl FnOnce(&mut InertCanonicalMirToKirCorrespondenceEvidenceV6),
) {
    let mut changed = evidence(owner);
    mutate(&mut changed);
    assert!(
        replay(&changed, owner).is_err(),
        "stale retained bytes must reject"
    );
    if let Ok(bytes) = encode(&changed) {
        let decoded = InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&bytes).unwrap();
        assert!(
            replay(&decoded, owner).is_err(),
            "canonical inert mutation must not authenticate"
        );
    }
}

#[test]
fn canonical_statement_terminator_synthetic_and_parameter_mutations_require_live_replay() {
    let owner = lower(helper_closure_semantic_owner_with_calls(2));
    assert!(!owner.correspondence.statement_operation_spans.is_empty());
    reject_mutation(&owner, |e| {
        e.correspondence.statement_operation_spans[0].operation_count += 1
    });
    reject_mutation(&owner, |e| {
        e.correspondence.statement_operation_spans[0].first_operation_ordinal += 1
    });
    reject_mutation(&owner, |e| {
        e.correspondence.statement_operation_spans =
            e.correspondence.statement_operation_spans[1..].into()
    });
    reject_mutation(&owner, |e| {
        e.correspondence.terminator_operation_spans[0].operation_count += 1
    });
    reject_mutation(&owner, |e| {
        e.correspondence.synthetic_operation_spans = vec![SemanticKirSyntheticOperationSpanV1 {
            correspondence_owner: e.roots[0].root,
            semantic_function: e.roots[0].source_body,
            rule: SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage,
            kernel_ir_block: e.correspondence.blocks[0].kernel_ir_block,
            first_operation_ordinal: 0,
            operation_count: 1,
        }]
        .into_boxed_slice();
    });
    reject_mutation(&owner, |e| e.correspondence.function_count += 1);
    reject_mutation(&owner, |e| e.functions[0].kernel_ir_function_ordinal += 1);
    let scalar = admitted_global_load_owner();
    assert!(!scalar.correspondence.parameter_bindings.is_empty());
    replay(&evidence(&scalar), &scalar).unwrap();
    reject_mutation(&scalar, |e| {
        e.correspondence.parameter_bindings[0].kernel_ir_value.0 += 1
    });
}

#[test]
fn original_owner_kir_ssa_and_lowering_limit_substitutions_reject() {
    let mut owner = lower(helper_closure_semantic_owner_with_calls(2));
    let e = evidence(&owner);
    let other = lower(helper_closure_semantic_owner_with_calls(1));
    assert!(replay(&e, &other).is_err());
    reject_mutation(&owner, |e| e.semantic_ssa_identity[0] ^= 1);
    reject_mutation(&owner, |e| e.lowering_limits.max_operations -= 1);
    let canonical = std::mem::replace(&mut owner.canonical_kernel_ir, other.canonical_kernel_ir);
    assert!(replay(&e, &owner).is_err());
    owner.canonical_kernel_ir = canonical;
    let original = owner.module.clone();
    owner.module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::Unreachable);
    assert!(replay(&e, &owner).is_err());
    owner.module = original;
    replay(&e, &owner).unwrap();
}

#[test]
fn report_substitution_and_truncation_never_release_decoded_facts() {
    let owner = lower(helper_closure_semantic_owner_with_calls(2));
    let other = lower(helper_closure_semantic_owner_with_calls(1));
    let other_reports = reports(&other);
    assert!(
        InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(
            &owner,
            &[(other_reports[0].0, &other_reports[0].1)],
            SemanticU32InductionAnalysisLimitsV1::default(),
        )
        .is_err()
    );
    let e = evidence(&owner);
    for len in [0, 8, 20, e.canonical_bytes().len() - 1] {
        assert!(
            InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&e.canonical_bytes()[..len])
                .is_err()
        );
    }
    let mut trailing = e.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&trailing).is_err());
}

#[test]
fn deterministic_zero_exact_and_exhausted_analysis_budgets() {
    let owner = lower(helper_closure_semantic_owner_with_calls(2));
    let reports = reports(&owner);
    let cost = reports[0].1.work_units();
    assert!(cost > 0);
    let refs = [(reports[0].0, &reports[0].1)];
    let exact = SemanticU32InductionAnalysisLimitsV1::new(cost, 0);
    let e = InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(&owner, &refs, exact)
        .unwrap();
    e.verify_replay(&owner, exact).unwrap();
    for work in [0, cost - 1] {
        let limits = SemanticU32InductionAnalysisLimitsV1::new(work, 0);
        assert!(
            InertCanonicalMirToKirCorrespondenceEvidenceV6::from_live_owner(&owner, &refs, limits)
                .is_err()
        );
        assert!(e.verify_replay(&owner, limits).is_err());
    }
    let mut budget = ReplayBudget { remaining: cost };
    budget.charge(cost).unwrap();
    assert!(budget.charge(1).is_err());
    assert_eq!(
        budget
            .analysis_limits(SemanticU32InductionAnalysisLimitsV1::default())
            .work_units(),
        0
    );
    assert!(
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&vec![
            0;
            MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V6
                + 1
        ])
        .is_err()
    );
}

#[test]
fn call_map_and_source_identity_changes_cannot_be_hidden_by_rebinding_induction_hashes() {
    let owner = lower(helper_closure_semantic_owner_with_calls(2));
    let mut changed = evidence(&owner);
    let view = &changed.expansion.roots()[0];
    // Frozen expansion V1: header 152, root prefix 116, each call instance 64.
    let first_instance = 152 + 116;
    let first_local = first_instance + view.instances().len() * 64;
    let original = changed.expansion.canonical_bytes();
    assert_eq!(
        &original[first_local + 12..first_local + 44],
        view.local_identity(SemanticLocalIdV1::from_index(0))
            .unwrap()
    );
    let mut bad_call = original.to_vec();
    bad_call[first_instance + 64 + 36..first_instance + 64 + 40]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(InertCanonicalSemanticCallExpansionEvidenceV1::decode(&bad_call).is_err());

    let mut bytes = original.to_vec();
    bytes[first_local + 12] ^= 0x40;
    changed.expansion = InertCanonicalSemanticCallExpansionEvidenceV1::decode(&bytes).unwrap();
    let MirToKirInductionEvidenceV6::Expanded(induction) = &changed.roots[0].induction else {
        panic!("expanded fixture");
    };
    let mut bytes = induction.canonical_bytes().to_vec();
    // V2's expansion-evidence identity immediately follows the original source hash.
    bytes[52..84].copy_from_slice(changed.expansion.identity());
    changed.roots[0].induction = MirToKirInductionEvidenceV6::Expanded(
        InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes).unwrap(),
    );
    let rebound =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&encode(&changed).unwrap()).unwrap();
    assert!(matches!(
        replay(&rebound, &owner),
        Err(ProductionCorrespondenceEvidenceErrorV6::Expansion(_))
    ));
}

#[test]
fn induction_work_substitution_is_inert_even_when_every_identity_matches() {
    let owner = lower(helper_closure_semantic_owner_with_calls(2));
    let mut changed = evidence(&owner);
    let MirToKirInductionEvidenceV6::Expanded(induction) = &changed.roots[0].induction else {
        panic!("expanded fixture");
    };
    let mut bytes = induction.canonical_bytes().to_vec();
    // Header, five identities, root/function IDs, then examined count and work.
    let work_offset = 20 + 5 * 32 + 2 * 4 + 4;
    assert_eq!(
        u64::from_le_bytes(bytes[work_offset..work_offset + 8].try_into().unwrap()),
        induction.work_units()
    );
    bytes[work_offset..work_offset + 8]
        .copy_from_slice(&(induction.work_units() + 1).to_le_bytes());
    changed.roots[0].induction = MirToKirInductionEvidenceV6::Expanded(
        InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes).unwrap(),
    );
    let changed =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&encode(&changed).unwrap()).unwrap();
    assert!(matches!(
        replay(&changed, &owner),
        Err(ProductionCorrespondenceEvidenceErrorV6::InductionV2(_))
    ));
}

#[test]
fn cross_root_ownership_and_root_report_omissions_reject() {
    let owner = mixed_roots_owner();
    reject_mutation(&owner, |e| e.roots.swap(0, 1));
    reject_mutation(&owner, |e| {
        let mut roots = std::mem::take(&mut e.roots).into_vec();
        roots.truncate(1);
        e.roots = roots.into_boxed_slice();
    });
    reject_mutation(&owner, |e| {
        e.correspondence.blocks[0].correspondence_owner = e.roots[1].root
    });
    reject_mutation(&owner, |e| {
        e.functions[0].record.correspondence_owner = e.roots[1].root
    });
    reject_mutation(&owner, |e| e.roots[0].source_body = e.roots[1].source_body);
}

fn repeated_loop_helper_owner() -> ProductionSemanticKirOwnerV1 {
    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    let baseline = helper_closure_semantic_owner_with_calls(2);
    let original = baseline.semantic();
    let helper = &original.functions()[1];
    let source = helper.source();
    let scalar = |tag, shape, bytes, maximum| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, (bytes * 8) as u16, bytes),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            shape,
        )
    };
    let types = vec![
        original.types()[0].clone(),
        scalar(
            31,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
            4,
            u128::from(u32::MAX),
        ),
        scalar(
            33,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            1,
            1,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([35; 32]),
            SemanticLayoutIdentityV1::from_sha256([36; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![0, 4],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, BOOL]).unwrap()),
        ),
    ];
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let copy = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let constant = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    };
    let field = |index, ty| {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap(),
                ],
                ty,
            )
            .unwrap(),
        )
    };
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let block = |tag, statements, kind| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, kind),
        )
        .unwrap()
    };
    let root = rebuild_function(
        &original.functions()[0],
        (0..2u8)
            .map(|index| {
                block(
                    208 + index,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(1),
                            vec![constant(u128::from(index) + 8)],
                            Some(SemanticCallDestinationV1::new(
                                place(0, UNIT),
                                edge(SemanticEdgeRoleV1::CallReturn, u32::from(index) + 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                )
            })
            .chain(std::iter::once(original.functions()[0].blocks()[2].clone()))
            .collect(),
    );
    // Same checked i=0; i<bound; i+=1 topology and pair layout as the MIR V2 fixture.
    let blocks = vec![
        block(
            230,
            vec![assign(2, U32, SemanticRvalueKindV1::Use(constant(0)))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            231,
            vec![assign(
                3,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: copy(2, U32),
                    right: copy(1, U32),
                },
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(3, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            232,
            vec![assign(
                4,
                PAIR,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    copy(2, U32),
                    constant(1),
                )),
            )],
            SemanticTerminatorKindV1::Assert {
                condition: field(1, BOOL),
                expected: false,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: copy(2, U32),
                    right: constant(1),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            233,
            vec![assign(2, U32, SemanticRvalueKindV1::Use(field(0, U32)))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(234, vec![], SemanticTerminatorKindV1::Return),
    ];
    let helper = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([216; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                U32,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                ),
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        [
            (UNIT, SemanticLocalRoleV1::Return),
            (U32, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Temporary),
            (BOOL, SemanticLocalRoleV1::Temporary),
            (PAIR, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([217 + index as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new(
        original.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    lower(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
    )
}

#[test]
fn helper_loop_certificates_replay_and_reject_omission_or_cross_instance_binding() {
    let owner = repeated_loop_helper_owner();
    let e = evidence(&owner);
    let decoded =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(e.canonical_bytes()).unwrap();
    let checked = replay(&decoded, &owner).unwrap();
    assert_eq!(checked.induction_reports(), reports(&owner));
    let report = &checked.induction_reports()[0].1;
    assert_eq!(report.certificates().len(), 2);
    let view = owner
        .semantic_ssa()
        .execution_view_for_root(e.roots()[0].root())
        .unwrap();
    let instances = report
        .certificates()
        .iter()
        .map(|certificate| {
            let bound = view.local_origins()[certificate.bound().local().index() as usize];
            assert_eq!(bound.function(), SemanticFunctionIdV1::from_index(1));
            assert_eq!(bound.local(), SemanticLocalIdV1::from_index(1));
            assert_eq!(
                view.block_origins()[certificate.guard().block().block().index() as usize]
                    .instance(),
                bound.instance()
            );
            bound.instance()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(instances.len(), 2);
    let MirToKirInductionEvidenceV6::Expanded(induction) = e.roots()[0].induction() else {
        panic!("expanded loop fixture");
    };
    let original = induction.canonical_bytes();
    // Frozen V2: 204-byte header, certificate count at 200, 72-byte place bindings.
    const HEADER: usize = 204;
    assert_eq!(
        u32::from_le_bytes(original[200..HEADER].try_into().unwrap()),
        2
    );
    assert_eq!((original.len() - HEADER) % 2, 0);
    let certificate_size = (original.len() - HEADER) / 2;
    let mut omitted = original[..HEADER + certificate_size].to_vec();
    let length = omitted.len() as u32;
    omitted[16..20].copy_from_slice(&length.to_le_bytes());
    omitted[200..HEADER].copy_from_slice(&1u32.to_le_bytes());

    let mut substituted = original.to_vec();
    let bound_offset = HEADER + 2 * 72;
    let other_bound = bound_offset + certificate_size;
    assert_ne!(
        &original[bound_offset..bound_offset + 4],
        &original[other_bound..other_bound + 4]
    );
    substituted[bound_offset..bound_offset + 72]
        .copy_from_slice(&original[other_bound..other_bound + 72]);
    for bytes in [omitted, substituted] {
        let mut changed =
            InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(e.canonical_bytes()).unwrap();
        changed.roots[0].induction = MirToKirInductionEvidenceV6::Expanded(
            InertCanonicalSemanticU32InductionEvidenceV2::decode(&bytes).unwrap(),
        );
        let changed =
            InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(&encode(&changed).unwrap())
                .unwrap();
        assert!(matches!(
            replay(&changed, &owner),
            Err(ProductionCorrespondenceEvidenceErrorV6::InductionV2(
                SemanticU32InductionEvidenceErrorV2::ReportMismatch,
            ))
        ));
    }
}

#[test]
fn shared_replay_budget_exhausts_on_later_root_after_first_root_succeeds() {
    let owner = mixed_roots_owner();
    let e = evidence(&owner);
    let expected = reports(&owner);
    let first_cost = expected[0].1.work_units();
    let second_cost = expected[1].1.work_units();
    assert!(first_cost > 0 && second_cost > 0);
    let total = first_cost.checked_add(second_cost).unwrap();
    let limits = SemanticU32InductionAnalysisLimitsV1::default();
    let source = owner.semantic().semantic();
    let expansion = owner.semantic_ssa().execution_expansion();
    let MirToKirInductionEvidenceV6::Original(first) = e.roots()[0].induction() else {
        panic!("call-free first root");
    };
    let MirToKirInductionEvidenceV6::Expanded(second) = e.roots()[1].induction() else {
        panic!("expanded second root");
    };
    assert_eq!(
        second
            .verify_replay(
                source,
                expansion,
                e.expansion(),
                SemanticU32InductionAnalysisLimitsV1::new(second_cost, 0)
            )
            .unwrap(),
        expected[1].1,
    );
    assert!(preflight_replay(&owner, limits).unwrap().remaining >= total);

    // Exercise the actual limiter and root replay APIs with a small allowance;
    // this does not exhaust the public V6 entry point's fixed 64M-work cap.
    for remaining in [total, total - 1] {
        let mut budget = ReplayBudget { remaining };
        let report = analyze_expanded_semantic_u32_induction_no_overflow_with_limits_v1(
            source,
            expansion,
            e.roots()[0].root(),
            budget.analysis_limits(limits),
        )
        .unwrap();
        assert_eq!(report, expected[0].1);
        assert_eq!(
            &InertCanonicalSemanticU32InductionEvidenceV1::from_report(&report).unwrap(),
            first,
        );
        budget.charge(report.work_units()).unwrap();
        assert_eq!(budget.remaining, remaining - first_cost);
        let result = second.verify_replay(
            source,
            expansion,
            e.expansion(),
            budget.analysis_limits(limits),
        );
        if remaining == total {
            let report = result.unwrap();
            assert_eq!(report, expected[1].1);
            budget.charge(report.work_units()).unwrap();
            assert_eq!(budget.remaining, 0);
        } else {
            assert!(
                matches!(result, Err(SemanticU32InductionEvidenceErrorV2::Analysis(
                fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1::WorkLimit { limit, .. },
            )) if limit == second_cost - 1)
            );
            assert_eq!(budget.remaining, second_cost - 1);
        }
    }
}
