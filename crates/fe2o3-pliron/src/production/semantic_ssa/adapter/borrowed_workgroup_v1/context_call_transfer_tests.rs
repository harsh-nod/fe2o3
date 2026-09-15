use super::super::super::super::context_call_transfers::CheckedTransfers;
use super::*;
use super::super::super::super::ordered_context_loans;

#[path = "context_scc_owner_tests.rs"]
mod scc_owner_tests;

#[test]
fn scc_and_dfs_full_execution_classifier_retain_exact_sites() {
    for (change, expected) in [
        (Change::None, 2),
        (Change::MoveArgument, 2),
        (Change::ByValueHelper, 0),
        (Change::OrdinaryCopy, 0),
        (Change::LateSharedUse, 0),
        (Change::DeadOwner, 0),
        (Change::Backedge, 0),
    ] {
        let semantic = admitted(change);
        let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
        expansion.verify_replay(&semantic).unwrap();
        let view = expansion.root(semantic.roots()[0]).unwrap();
        let before = view.body().clone();
        let old = ordered_context_loans::with_dfs_reference(|| {
            execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK)
        }).unwrap();
        let new = execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK).unwrap();
        assert_eq!(old, new);
        assert_eq!(new.len(), expected);
        assert_eq!(view.body(), &before);
    }
}

// Canonical, replayed component fixture only. The owned aggregate initializer
// below is NOT a compiler-issued Context or an authenticated production root.
#[derive(Clone, Copy, Default)]
enum Change {
    #[default]
    None,
    ByValueHelper,
    OrdinaryCopy,
    LateSharedUse,
    DeadOwner,
    Backedge,
    MoveArgument,
}

fn abi(
    input: u32,
    output: u32,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticFunctionAbiV1 {
    // This fixture records type4 as frozen shared and type2 as mutable/unpin=false.
    assert!(matches!(input, 2 | 4));
    let frozen_shared = input == 4;
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            frozen_shared,
            frozen_shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            frozen_shared,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([181; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(input)],
        ty(output),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(input),
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(ty(output), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap()
}

fn terminal(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    contract: SemanticExecutionCapabilityContractV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

fn admitted(change: Change) -> AdmittedInertSemanticMirV1 {
    let seed = crate::production::semantic_ssa::tests::admitted_helper_semantic();
    let original = &seed.functions()[0];
    let mut types = Fixture::new().types;
    types[0] = SemanticTypeDeclV1::new(
        types[0].identity(),
        types[0].layout_identity(),
        seed.types()[0].layout().clone(),
        SemanticTypeShapeV1::Unit,
    );
    for (index, kind) in [
        (
            2,
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
        ),
        (
            4,
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        ),
    ] {
        types[index] = types[index].clone().with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_rustc_layout_is_noundef(true)
                .with_scalar_pointee_info(
                    Some(SemanticAbiPointeeInfoV1::new(kind, 0, 1).unwrap()),
                    None,
                ),
        );
    }
    let mut entry_statements = vec![
        assign(
            1,
            1,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, vec![]).unwrap(),
        ),
        borrow(2, 2, place(1, 1), SemanticBorrowKindV1::Mutable),
    ];
    if matches!(change, Change::DeadOwner) {
        entry_statements.push(statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )));
    }
    let operand = if matches!(change, Change::MoveArgument) {
        SemanticOperandV1::Move(place(2, 2))
    } else {
        SemanticOperandV1::Copy(place(2, 2))
    };
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        vec![
            local(0, 0, SemanticLocalRoleV1::Return),
            local(1, 1, SemanticLocalRoleV1::Temporary),
            local(2, 2, SemanticLocalRoleV1::Temporary),
            local(3, 0, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(0, vec![], jump(1)),
            block(1, entry_statements, call(1, vec![operand], place(3, 0), 2)),
            block(
                2,
                vec![statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(1),
                ))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut helper_statements = vec![];
    let shared_parent = if matches!(change, Change::OrdinaryCopy) {
        helper_statements.push(alias(5, 1, 2));
        5
    } else {
        1
    };
    helper_statements.push(borrow(
        2,
        4,
        deref(shared_parent),
        SemanticBorrowKindV1::Shared,
    ));
    let mut helper_blocks = vec![
        block(
            0,
            helper_statements,
            call(
                4,
                vec![SemanticOperandV1::Copy(place(2, 4))],
                place(3, 5),
                1,
            ),
        ),
        block(
            1,
            vec![],
            call(
                2,
                vec![SemanticOperandV1::Copy(place(1, 2))],
                place(4, 0),
                2,
            ),
        ),
        block(2, vec![], SemanticTerminatorKindV1::Return),
    ];
    if matches!(change, Change::LateSharedUse) {
        helper_blocks[2] = block(
            2,
            vec![],
            call(
                4,
                vec![SemanticOperandV1::Copy(place(2, 4))],
                place(3, 5),
                3,
            ),
        );
        helper_blocks.push(block(3, vec![], SemanticTerminatorKindV1::Return));
    }
    if matches!(change, Change::Backedge) {
        helper_blocks[2] = block(2, vec![], fork(3, 0));
        helper_blocks.push(block(3, vec![], SemanticTerminatorKindV1::Return));
    }
    let helper = function(
        150,
        abi(
            2,
            0,
            if matches!(change, Change::ByValueHelper) {
                SemanticSourceArgumentOwnershipV1::ByValue
            } else {
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            },
        ),
        vec![
            local(0, 0, SemanticLocalRoleV1::Return),
            local(1, 2, SemanticLocalRoleV1::Argument(0)),
            local(2, 4, SemanticLocalRoleV1::Temporary),
            local(3, 5, SemanticLocalRoleV1::Temporary),
            local(4, 0, SemanticLocalRoleV1::Temporary),
            local(5, 2, SemanticLocalRoleV1::Temporary),
        ],
        helper_blocks,
    );
    let nested = function(
        151,
        abi(2, 0, SemanticSourceArgumentOwnershipV1::UniqueBorrow),
        vec![
            local(0, 0, SemanticLocalRoleV1::Return),
            local(1, 2, SemanticLocalRoleV1::Argument(0)),
            local(2, 3, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                0,
                vec![],
                call(
                    3,
                    vec![SemanticOperandV1::Copy(place(1, 2))],
                    place(2, 3),
                    1,
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(2)),
        terminal(
            170,
            abi(2, 3, SemanticSourceArgumentOwnershipV1::UniqueBorrow),
            contract(&context_callable(
                170,
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                2,
            ))
            .unwrap(),
        ),
        terminal(
            172,
            abi(4, 5, SemanticSourceArgumentOwnershipV1::SharedBorrow),
            contract(&policy_callable(172)).unwrap(),
        ),
    ];
    InertSemanticMirRequestV1::new_with_callables(
        seed.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper, nested],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn copy_candidates(view: &SemanticExpandedRootV1) -> Vec<SemanticBorrowCandidateV1> {
    let mut result = vec![];
    for (b, origin) in view.block_origins().iter().enumerate() {
        for (s, item) in origin.statements().iter().enumerate() {
            if !matches!(
                item,
                SemanticExpandedStatementOriginV1::ParameterTransfer { .. }
            ) {
                continue;
            }
            let SemanticStatementKindV1::Assign(a) = view.body().blocks()[b].statements()[s].kind()
            else {
                panic!()
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)) = a.value().kind() else {
                continue;
            };
            result.push(SemanticBorrowCandidateV1 {
                site: site(b as u32, s as u32),
                source_local: p.local().index(),
                source_type: ty(1),
                source_reference: Some(p.local().index()),
                value_alias: true, source_kind: SemanticBorrowCandidateSourceV1::Direct,
                valid: true,
                consumers: 1,
                intrinsic_consumer: false,
            });
        }
    }
    result
}

fn budget(work: usize) -> Budget {
    Budget {
        remaining: work,
        limit: work,
        profile: FlowWorkProfile::default(),
    }
}

#[test]
fn measured_context_two_copy_parameter_transfers_promote_original_owner() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    expansion.verify_replay(&semantic).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let original = view.body().clone();
    let candidates = copy_candidates(view);
    assert_eq!(candidates.len(), 2);
    let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
    for candidate in &candidates {
        assert!(
            transfers
                .accepts(view.body(), *candidate, &mut budget(96))
                .unwrap()
        );
    }
    let sites = execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK).unwrap();
    assert_eq!(
        sites.len(),
        2,
        "owned mutable borrow and nested shared reborrow"
    );
    assert!(sites.contains(&site(1, 1)));
    assert!(typed_direct_sites(view.body(), semantic.types(), semantic.callables()).is_empty());
    assert!(
        candidates.iter().all(|c| !sites.contains(&c.site)),
        "frame copies stay assignments"
    );
    let (input, _, _) = semantic_function_ssa_input_with_event_origins_v1(
        view.body(),
        Some(semantic.types()),
        semantic.callables(),
        &sites,
        |_, _, _| {},
    );
    let (unclassified, _, _) = semantic_function_ssa_input_v1(
        view.body(),
        Some(semantic.types()),
        semantic.callables(),
        &BTreeSet::new(),
    );
    assert!(input.promotable()[1]);
    assert!(!unclassified.promotable()[1]);
    assert_eq!(
        input.blocks(),
        unclassified.blocks(),
        "all Copy/Move/Kill events retained"
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert!(plan.promoted_variables().contains(&SsaVariableIdV1::new(1)));
    assert_eq!(plan.resolved_events(SsaBlockIdV1::new(1)).unwrap().iter().filter(|(_, event)|
        matches!(event, SsaResolvedEventV1::Define { variable, .. } if variable.get() == 1)).count(), 1);
    assert_eq!(
        view.body(),
        &original,
        "no source, frame, or lifetime event rewritten"
    );
}

#[test]
fn measured_context_copy_receipt_rejects_wrong_site_local_type_and_body() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
    let candidate = copy_candidates(view)[0];
    let mut changed = candidate;
    changed.site.statement -= 1;
    assert!(
        !transfers
            .accepts(view.body(), changed, &mut budget(96))
            .unwrap()
    );
    changed = candidate;
    changed.source_reference = Some(0);
    assert!(
        !transfers
            .accepts(view.body(), changed, &mut budget(96))
            .unwrap()
    );
    changed = candidate;
    changed.source_type = ty(3);
    assert!(
        !transfers
            .accepts(view.body(), changed, &mut budget(96))
            .unwrap()
    );
    changed = candidate;
    changed.value_alias = false;
    assert!(
        !transfers
            .accepts(view.body(), changed, &mut budget(96))
            .unwrap()
    );
    assert!(
        !transfers
            .accepts(&view.body().clone(), candidate, &mut budget(96))
            .unwrap()
    );
}

#[test]
fn measured_context_copy_receipt_is_source_and_expansion_bound() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let other = admitted(Change::MoveArgument);
    assert!(matches!(
        CheckedTransfers::new(&other, &expansion, view),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    let other_expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    assert!(matches!(
        CheckedTransfers::new(&semantic, &other_expansion, view),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn measured_context_copy_check_charges_fixed_work_before_any_acceptance() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
    let candidate = copy_candidates(view)[0];
    let mut exact = budget(96);
    assert!(
        transfers
            .accepts(view.body(), candidate, &mut exact)
            .unwrap()
    );
    assert_eq!(exact.remaining, 0);
    assert!(matches!(
        transfers.accepts(view.body(), candidate, &mut budget(95))
            .map_err(flow_work_profile_v1::original_error_for_test),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: 96,
            limit: 95
        })
    ));
}

#[test]
fn measured_context_by_value_helper_does_not_authorize_exclusive_copy() {
    let semantic = admitted(Change::ByValueHelper);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    assert!(
        execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn measured_context_original_move_argument_remains_a_move() {
    let semantic = admitted(Change::MoveArgument);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    assert_eq!(
        copy_candidates(view).len(),
        1,
        "only the nested call still copies"
    );
    let sites = execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK).unwrap();
    assert_eq!(sites.len(), 2);
    assert!(sites.contains(&site(1, 1)));
}

#[test]
fn measured_context_source_copy_remains_rejected_with_valid_frame_receipts() {
    let semantic = admitted(Change::OrdinaryCopy);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    assert_eq!(copy_candidates(view).len(), 2);
    assert!(
        execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn measured_context_late_shared_use_still_overlaps_exclusive_descendant() {
    let semantic = admitted(Change::LateSharedUse);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    assert!(
        execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn measured_context_frame_receipts_do_not_waive_owner_death_or_backedges() {
    for change in [Change::DeadOwner, Change::Backedge] {
        let semantic = admitted(change);
        let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
        let view = expansion.root(semantic.roots()[0]).unwrap();
        assert!(
            execution_sites(&semantic, &expansion, view, MAX_FLOW_WORK)
                .unwrap()
                .is_empty()
        );
    }
}
