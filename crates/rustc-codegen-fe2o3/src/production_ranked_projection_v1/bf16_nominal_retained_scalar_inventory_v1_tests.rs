//! Synthetic retained scalar controls; no constructor grants source authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;

fn payload_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn payload_targets(cases: &[(u128, u32)], otherwise: u32) -> SemanticSwitchTargetsV1 {
    SemanticSwitchTargetsV1::new(
        cases
            .iter()
            .map(|(value, target)| {
                SemanticSwitchTargetV1::new(
                    *value,
                    payload_edge(SemanticEdgeRoleV1::SwitchValue, *target),
                )
            })
            .collect(),
        payload_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
    )
    .unwrap()
}

pub(super) fn payload_fixture(
    cases: &[(u128, u32)],
    otherwise: u32,
    second_definition: bool,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let enumeration = SemanticTypeIdV1::from_index(2);
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([202; 32]),
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([204; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([206; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Enum {
                discriminant: scalar,
                variants: (0..3)
                    .map(|variant| {
                        SemanticEnumVariantV1::new(
                            variant,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        )
                    })
                    .collect(),
            },
        ),
    ];
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, ty, kind| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    };
    let define = || {
        assign(
            1,
            enumeration,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                .unwrap(),
        )
    };
    let mut statements = vec![define()];
    if second_definition {
        statements.push(define());
    }
    statements.push(assign(
        2,
        scalar,
        SemanticRvalueKindV1::Discriminant(place(1, enumeration)),
    ));
    let mut blocks = vec![
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([207; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, scalar)),
                    targets: payload_targets(cases, otherwise),
                },
            ),
        )
        .unwrap(),
    ];
    for tag in [208, 209, 210] {
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        );
    }
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([211; 32]),
        SemanticLayoutIdentityV1::from_sha256([212; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([213; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([214; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([215; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([216; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([217; 32]),
        source,
        abi,
        [unit, enumeration, scalar]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([218 + index as u8; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn fixture() -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    let (_, base) = payload_fixture(&[(0, 2), (1, 3)], 2, false);
    let source = SemanticSourceProvenanceV1::unavailable();
    let option = SemanticTypeIdV1::from_index(2);
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        Vec::new(),
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), Vec::new(), option).unwrap(),
            edge(SemanticEdgeRoleV1::CallReturn, 1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([40; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
        source,
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(45, Vec::new(), SemanticTerminatorKindV1::Call(call)),
            block(
                46,
                vec![base.blocks()[0].statements()[1].clone()],
                base.blocks()[0].terminator().kind().clone(),
            ),
            block(47, Vec::new(), SemanticTerminatorKindV1::Return),
            block(48, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let callables = vec![SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([50; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
            source,
            base.abi().clone(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: option,
            index_witness: option,
            element: option,
            raw_index: option,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([55; 32]),
    }];
    (function, callables)
}

#[derive(Debug)]
struct Probe {
    result: Result<()>,
    phase: Phase,
    counts_len: usize,
    blocks_len: usize,
    candidate_len: usize,
    candidate_capacity: usize,
    work: usize,
    live: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 11;
fn probe(
    function: &SemanticFunctionDeclV1,
    work_limit: usize,
    storage_limit: usize,
    mode: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut pending = RetainedScalarInventoryV1::new();
    // Both pending and owning error slot outlive the callback/catch observation.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(function, &mut resources)?;
        let data = pending.completed_for(function, &resources)?;
        assert_eq!(data.counts.len(), function.locals().len());
        match mode {
            0 => Ok(()),
            1 => Err(Error::Unsupported("scalar component callback")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Unsupported("scalar component panic"))
        }
    };
    assert_eq!(budget.storage(), FLOOR + owned);
    let observed = Probe {
        result,
        phase: pending.phase,
        counts_len: pending.inventory.counts.len(),
        blocks_len: pending.inventory.blocks.len(),
        candidate_len: pending.block_candidate.len(),
        candidate_capacity: pending.block_candidate.capacity(),
        work: budget.work(),
        live: budget.storage(),
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    // This is component custody, not a genuine checked-source postflight.
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
fn exact_data(left: &AssertionDefinitionInventoryV1, right: &AssertionDefinitionInventoryV1) {
    assert_eq!(left.counts, right.counts);
    assert_eq!(left.blocks, right.blocks);
    assert_eq!(left.assignments, right.assignments);
    assert_eq!(left.address_escaped, right.address_escaped);
}
fn oracle(function: &SemanticFunctionDeclV1) -> (AssertionDefinitionInventoryV1, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let data = assertion_definition_inventory_with_resources_v1(
        function,
        &mut Resources::new(&mut budget, &mut owned),
    )
    .unwrap();
    let work = budget.work();
    assert_eq!(budget.storage(), owned);
    drop(data);
    budget.release_storage(owned).unwrap();
    // DATA comes from a second unchanged original invocation, not candidate output.
    // This unmetered synthetic oracle is outside the proposed component protocol.
    let data =
        assertion_definition_inventory_with_resources_v1(function, &mut Resources::unmetered())
            .unwrap();
    (data, work, owned)
}
fn varied(
    function: &SemanticFunctionDeclV1,
    repeat: usize,
    malformed: bool,
) -> SemanticFunctionDeclV1 {
    let scalar = SemanticTypeIdV1::from_index(1);
    let source = function.source();
    let repeated = function.blocks()[0].statements()[0].clone();
    let mut blocks = Vec::new();
    for (index, block) in function.blocks().iter().enumerate() {
        let mut statements = block.statements().to_vec();
        if index == 0 {
            statements.extend(std::iter::repeat_n(repeated.clone(), repeat));
        }
        if index == 1 && malformed {
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(99), vec![], scalar)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        scalar,
                        SemanticRvalueKindV1::Discriminant(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], scalar)
                                .unwrap(),
                        ),
                    ),
                )),
            ));
        }
        blocks.push(
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap(),
        );
    }
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        source,
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}
#[test]
fn complete_data_and_original_debits_are_preserved_after_explicit_prefix() {
    for second in [false, true] {
        let (_, f) = payload_fixture(&[(0, 2), (1, 3)], 2, second);
        let (expected, old_work, old_storage) = oracle(&f);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut pending = RetainedScalarInventoryV1::new();
        {
            let mut resources = Resources::new(&mut budget, &mut owned);
            pending.prepare_into(&f, &mut resources).unwrap();
            exact_data(pending.completed_for(&f, &resources).unwrap(), &expected);
        }
        assert_eq!(budget.work(), old_work + 32);
        assert_eq!(owned, old_storage + retained_scalar_frame_v1().unwrap());
        assert_eq!(pending.block_candidate.capacity(), 0);
        assert_eq!(pending.phase, Phase::Complete);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn saturation_and_duplicate_assignment_data_match_original() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let f = varied(&f, 260, false);
    let (expected, old_work, old_storage) = oracle(&f);
    assert_eq!(expected.counts[1], u8::MAX);
    assert_eq!(expected.assignments[1], None);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedScalarInventoryV1::new();
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(&f, &mut resources).unwrap();
        exact_data(pending.completed_for(&f, &resources).unwrap(), &expected);
    }
    assert_eq!(budget.work(), old_work + 32);
    assert_eq!(owned, old_storage + retained_scalar_frame_v1().unwrap());
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn complete_owner_survives_component_success_error_and_panic() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    for mode in 0..3 {
        let p = probe(&f, LIMIT, LIMIT, mode);
        assert_eq!(p.result.is_ok(), mode == 0);
        assert_eq!(p.phase, Phase::Complete);
        assert_eq!(p.counts_len, f.locals().len());
        assert_eq!(p.blocks_len, f.blocks().len());
        assert!(p.failed_work.is_none() && p.failed_storage.is_none());
    }
}
#[test]
fn malformed_original_error_retains_all_attached_partial_tables() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let f = varied(&f, 0, true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let original = assertion_definition_inventory_with_resources_v1(
        &f,
        &mut Resources::new(&mut budget, &mut owned),
    );
    assert!(matches!(
        original,
        Err(Error::Unsupported(
            "an assertion proof assignment is outside the semantic local table"
        ))
    ));
    budget.release_storage(owned).unwrap();
    let p = probe(&f, LIMIT, LIMIT, 0);
    assert!(matches!(
        p.result,
        Err(Error::Unsupported(
            "an assertion proof assignment is outside the semantic local table"
        ))
    ));
    assert_eq!(p.phase, Phase::Terminal);
    assert_eq!(p.counts_len, f.locals().len());
    assert_eq!(p.blocks_len, 1);
    assert_eq!(p.candidate_len, 0);
    assert!(p.candidate_capacity > 0);
    assert!(p.failed_work.is_none() && p.failed_storage.is_none());
}
#[test]
fn exact_limits_and_one_short_keep_partial_custody_and_sticky_denial() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let baseline = probe(&f, LIMIT, LIMIT, 0);
    assert!(baseline.result.is_ok());
    let exact = probe(&f, baseline.work, baseline.live, 0);
    assert!(exact.result.is_ok());
    for (w, s) in [
        (baseline.work - 1, baseline.live),
        (baseline.work, baseline.live - 1),
    ] {
        let p = probe(&f, w, s, 0);
        assert!(p.result.is_err());
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.failed_work.is_some() || p.failed_storage.is_some());
        assert!(p.counts_len > 0);
    }
}
#[test]
fn each_work_boundary_preserves_partial_block_candidate_without_refunding() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let full = probe(&f, LIMIT, LIMIT, 0);
    let mut saw_candidate = false;
    let mut saw_completed_block = false;
    for limit in 0..full.work {
        let p = probe(&f, limit, LIMIT, 0);
        assert!(p.result.is_err());
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.failed_work.is_some());
        saw_candidate |= p.candidate_len > 0;
        saw_completed_block |= p.blocks_len > 0;
    }
    assert!(saw_candidate && saw_completed_block);
}
#[test]
fn initial_storage_refusal_and_array_boundary_do_not_replace_owned_tables() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let header = retained_scalar_frame_v1().unwrap();
    let p = probe(&f, LIMIT, FLOOR + header - 1, 0);
    assert!(p.result.is_err());
    assert_eq!(p.live, FLOOR);
    assert_eq!(p.counts_len, 0);
    assert!(p.failed_storage.is_some());
    let old_header = size_of::<AssertionDefinitionInventoryV1>() + size_of::<Vec<usize>>() + 4096;
    let p = probe(&f, LIMIT, FLOOR + header + old_header + f.locals().len(), 0);
    assert!(p.result.is_err());
    assert_eq!(p.counts_len, f.locals().len());
    assert_eq!(p.blocks_len, 0);
    assert!(p.failed_storage.is_some());
}
#[test]
fn occupied_retry_is_terminal_without_debits_or_payload_replacement() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedScalarInventoryV1::new();
    pending
        .prepare_into(&f, &mut Resources::new(&mut budget, &mut owned))
        .unwrap();
    let before = (
        budget.work(),
        owned,
        pending.inventory.counts.as_ptr(),
        pending.inventory.blocks.as_ptr(),
    );
    assert!(
        pending
            .prepare_into(&f, &mut Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    assert_eq!(
        before,
        (
            budget.work(),
            owned,
            pending.inventory.counts.as_ptr(),
            pending.inventory.blocks.as_ptr()
        )
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(
        pending
            .completed_for(&f, &Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn detached_source_wrong_ledger_and_sticky_denial_cannot_lend_completed_data() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let (_, detached) = payload_fixture(&[(0, 2)], 2, false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedScalarInventoryV1::new();
    pending
        .prepare_into(&f, &mut Resources::new(&mut budget, &mut owned))
        .unwrap();
    assert!(
        pending
            .completed_for(&detached, &Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    let mut other_work = Work::new(LIMIT);
    let mut other_budget = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(
        pending
            .completed_for(&f, &Resources::new(&mut other_budget, &mut other_owned))
            .is_err()
    );
    assert_eq!(other_owned, 0);
    assert!(budget.charge_work(LIMIT).is_err());
    assert!(
        pending
            .completed_for(&f, &Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    let before = (budget.work(), owned);
    let mut fresh = RetainedScalarInventoryV1::new();
    assert!(
        fresh
            .prepare_into(&f, &mut Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    assert_eq!(before, (budget.work(), owned));
    drop(fresh);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn unmetered_component_is_refused_and_frame_is_explicit_checked_sum() {
    let (_, f) = payload_fixture(&[(0, 2)], 2, false);
    let mut pending = RetainedScalarInventoryV1::new();
    assert!(
        pending
            .prepare_into(&f, &mut Resources::unmetered())
            .is_err()
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(pending.inventory.counts.is_empty());
    let rows = typed_rows().unwrap();
    assert_eq!(rows.len(), FRAME_ROWS);
    assert!(rows.iter().all(|row| *row > 0));
    assert_eq!(
        retained_scalar_frame_v1().unwrap(),
        rows.into_iter().sum::<usize>()
    );
}
#[test]
fn original_order_and_candidate_move_after_fallible_admission_are_visible() {
    let source = include_str!("bf16_nominal_retained_scalar_inventory_v1.rs");
    let start = source.find("fn prepare_attached(").unwrap();
    let end = source[start..].find("\n}\n\nfn fill_attached").unwrap() + start;
    let body: String = source[start..end]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut position = 0;
    for needle in [
        "resources.reserve_storage(",
        "fill_attached(&mutself.inventory.counts,",
        "fill_attached(&mutself.inventory.assignments,",
        "fill_attached(&mutself.inventory.address_escaped,",
        "resources.reserve(&mutself.inventory.blocks,",
        "resources.work(4)?;",
        "resources.reserve(&mutself.block_candidate,",
        "resources.work(16)?;",
        "visit_statement_definition_places(",
        "resources.sort_unique_indices(&mutself.block_candidate)?;",
        "resources.work(1)?;",
        "resources.reserve(&mutself.inventory.blocks,1)?;",
        "self.inventory.blocks.push(std::mem::take(&mutself.block_candidate));",
    ] {
        position += body[position..].find(needle).unwrap() + needle.len();
    }
    assert!(!body.contains("Budget::new("));
    assert!(!body.contains("unmetered("));
    assert!(!body.contains("resources.push("));
}

#[test]
fn actual_call_destination_definition_matches_original() {
    let (f, _) = fixture();
    let (expected, old_work, old_storage) = oracle(&f);
    assert_eq!(expected.counts[1], 1);
    assert_eq!(expected.blocks[0], [1]);
    assert_eq!(expected.assignments[1], None);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedScalarInventoryV1::new();
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(&f, &mut resources).unwrap();
        exact_data(pending.completed_for(&f, &resources).unwrap(), &expected);
    }
    assert_eq!(budget.work(), old_work + 32);
    assert_eq!(owned, old_storage + retained_scalar_frame_v1().unwrap());
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn mutable_borrow_escape_and_shared_non_escape_match_original() {
    for kind in [SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Shared] {
        let (_, base) = payload_fixture(&[(0, 2)], 2, false);
        let scalar = SemanticTypeIdV1::from_index(1);
        let source = base.source();
        let mut blocks = base.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], scalar).unwrap(),
                SemanticRvalueV1::new(
                    scalar,
                    SemanticRvalueKindV1::Borrow {
                        kind,
                        place: SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(1),
                            vec![],
                            scalar,
                        )
                        .unwrap(),
                    },
                ),
            )),
        ));
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            source,
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        let f = SemanticFunctionDeclV1::new(
            base.identity(),
            base.role(),
            base.item_definition_identity(),
            base.monomorphization_identity(),
            base.generic_type_arguments_identity(),
            base.const_generic_arguments_identity(),
            source,
            base.abi().clone(),
            base.locals().to_vec(),
            base.entry(),
            blocks,
        )
        .unwrap();
        let (expected, old_work, old_storage) = oracle(&f);
        assert_eq!(
            expected.address_escaped[1],
            kind == SemanticBorrowKindV1::Mutable
        );
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut pending = RetainedScalarInventoryV1::new();
        {
            let mut resources = Resources::new(&mut budget, &mut owned);
            pending.prepare_into(&f, &mut resources).unwrap();
            exact_data(pending.completed_for(&f, &resources).unwrap(), &expected);
        }
        assert_eq!(budget.work(), old_work + 32);
        assert_eq!(owned, old_storage + retained_scalar_frame_v1().unwrap());
        drop(pending);
        budget.release_storage(owned).unwrap();
    }
}
