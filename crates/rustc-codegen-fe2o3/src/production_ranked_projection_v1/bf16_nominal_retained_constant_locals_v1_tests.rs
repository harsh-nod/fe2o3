//! Pure synthetic retained constants controls; no authentic source is fabricated.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;

fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn projection_function_with_locals(
    blocks: Vec<SemanticBasicBlockV1>,
    locals: Vec<SemanticLocalDeclV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(10)),
        SemanticLayoutIdentityV1::from_sha256(bytes(10)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(11)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SCALAR_TYPE,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn bytes(value: u8) -> [u8; 32] {
    [value; 32]
}
const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
fn place(index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], SCALAR_TYPE).unwrap()
}
fn assign(index: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(index),
        SemanticRvalueV1::new(SCALAR_TYPE, value),
    )))
}
fn fixture(
    statements: Vec<SemanticStatementV1>,
    count: u8,
    call_destination: Option<u32>,
) -> SemanticFunctionDeclV1 {
    let terminator = if let Some(index) = call_destination {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(index),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    } else {
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
    };
    projection_function_with_locals(
        vec![
            block(80, statements, terminator),
            block(81, vec![], SemanticTerminatorKindV1::Return),
        ],
        (0..count)
            .map(|index| {
                local(
                    100 + index,
                    SCALAR_TYPE,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                )
            })
            .collect(),
    )
}
fn mixed_fixture() -> SemanticFunctionDeclV1 {
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(10),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE).unwrap()],
        SCALAR_TYPE,
    )
    .unwrap();
    fixture(
        vec![
            assign(1, SemanticRvalueKindV1::Use(constant(7))),
            assign(
                2,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(1),
                },
            ),
            assign(
                3,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(4))),
            ),
            assign(
                4,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3))),
            ),
            assign(5, SemanticRvalueKindV1::Use(constant(9))),
            assign(5, SemanticRvalueKindV1::Use(constant(9))),
            assign(6, SemanticRvalueKindV1::Use(constant(17))),
            assign(
                7,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(6))),
            ),
            assign(
                8,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SCALAR_TYPE,
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(1_u128 << 80, 16).unwrap(),
                    ),
                ))),
            ),
            assign(9, SemanticRvalueKindV1::Use(constant(12))),
            statement(SemanticStatementKindV1::Deinitialize(place(9))),
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                projected,
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(13))),
            ))),
            assign(11, SemanticRvalueKindV1::Use(constant(21))),
            assign(
                12,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(99))),
            ),
            assign(
                13,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(13))),
            ),
            assign(99, SemanticRvalueKindV1::Use(constant(23))),
        ],
        14,
        Some(11),
    )
}
fn chain_fixture() -> SemanticFunctionDeclV1 {
    let mut statements = Vec::new();
    for index in 1..24 {
        statements.push(assign(
            index,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(index + 1))),
        ));
    }
    statements.push(assign(24, SemanticRvalueKindV1::Use(constant(37))));
    fixture(statements, 25, None)
}
fn original(function: &SemanticFunctionDeclV1) -> (Vec<Option<u64>>, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let values =
        constant_locals_with_resources_v1(function, &mut Resources::new(&mut budget, &mut owned))
            .unwrap();
    let used_work = budget.work();
    assert_eq!(owned, budget.storage());
    drop(values);
    budget.release_storage(owned).unwrap();
    // This second unchanged original call supplies independent synthetic DATA.
    // Its unmetered test allocation is outside the proposed component protocol.
    let values = constant_locals_with_resources_v1(function, &mut Resources::unmetered()).unwrap();
    (values, used_work, owned)
}
#[derive(Debug)]
struct Probe {
    ok: bool,
    phase: Phase,
    lengths: [usize; 4],
    capacities: [usize; 4],
    work: usize,
    owned: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
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
    let mut pending = RetainedConstantLocalsV1::new();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pending.prepare_into(function, &mut Resources::new(&mut budget, &mut owned))?;
        match mode {
            0 => Ok(()),
            1 => Err(Error::Unsupported("constant component callback")),
            _ => std::panic::panic_any(()),
        }
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Unsupported("constant component panic"))
        }
    };
    assert_eq!(budget.storage(), FLOOR + owned);
    let observed = Probe {
        ok: result.is_ok(),
        phase: pending.phase,
        lengths: [
            pending.definitions.len(),
            pending.states.len(),
            pending.values.len(),
            pending.path.len(),
        ],
        capacities: [
            pending.definitions.capacity(),
            pending.states.capacity(),
            pending.values.capacity(),
            pending.path.capacity(),
        ],
        work: budget.work(),
        owned,
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    // Both owner and returned owning error remain alive until after observation.
    // A caught panic payload was converted above; it is not claimed retained here.
    drop(result);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.failed_work(), observed.failed_work);
    assert_eq!(budget.failed_storage(), observed.failed_storage);
    observed
}
#[test]
fn complete_original_data_and_debits_match_after_explicit_wrapper_prefix() {
    for function in [mixed_fixture(), chain_fixture(), fixture(vec![], 1, None)] {
        let (expected, old_work, old_storage) = original(&function);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut pending = RetainedConstantLocalsV1::new();
        {
            let mut resources = Resources::new(&mut budget, &mut owned);
            pending.prepare_into(&function, &mut resources).unwrap();
            assert_eq!(
                pending.completed_for(&function, &resources).unwrap(),
                expected.as_slice()
            );
        }
        assert_eq!(pending.phase, Phase::Complete);
        assert_eq!(pending.path.len(), 0);
        assert_eq!(pending.path.capacity(), function.locals().len());
        assert_eq!(budget.work(), old_work + 32);
        assert_eq!(owned, old_storage + retained_constant_frame_v1().unwrap());
        assert_eq!(budget.storage(), owned);
        drop(pending);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn literal_alias_cycle_escape_duplicate_call_and_missing_expectations_are_independent() {
    let function = mixed_fixture();
    let (expected, _, _) = original(&function);
    assert_eq!(
        expected,
        vec![
            None,
            None,
            None,
            None,
            None,
            None,
            Some(17),
            Some(17),
            None,
            None,
            None,
            None,
            None,
            None
        ]
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedConstantLocalsV1::new();
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(&function, &mut resources).unwrap();
        assert_eq!(
            pending.completed_for(&function, &resources).unwrap(),
            expected.as_slice()
        );
    }
    assert!(matches!(
        pending.definitions[0],
        ConstantDefinitionV1::Missing
    ));
    assert!(matches!(
        pending.definitions[1],
        ConstantDefinitionV1::Invalid
    ));
    assert!(matches!(
        pending.definitions[3],
        ConstantDefinitionV1::Alias(_)
    ));
    assert!(matches!(
        pending.definitions[6],
        ConstantDefinitionV1::Direct(17)
    ));
    assert!(pending.states.iter().all(|state| *state == 2));
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn reverse_alias_chain_uses_original_iterative_path_and_preserves_all_values() {
    let function = chain_fixture();
    let (expected, _, _) = original(&function);
    assert_eq!(expected[0], None);
    assert!(expected[1..].iter().all(|value| *value == Some(37)));
    let p = probe(&function, LIMIT, LIMIT, 0);
    assert!(p.ok);
    assert_eq!(p.lengths, [25, 25, 25, 0]);
    assert_eq!(p.capacities[3], 25);
}
#[test]
fn complete_physical_owner_survives_success_returned_error_and_panic_observation() {
    let function = mixed_fixture();
    for mode in 0..3 {
        let p = probe(&function, LIMIT, LIMIT, mode);
        assert_eq!(p.ok, mode == 0);
        assert_eq!(p.phase, Phase::Complete);
        assert_eq!(p.lengths, [14, 14, 14, 0]);
        assert!(p.failed_work.is_none() && p.failed_storage.is_none());
        assert!(p.owned > 0);
    }
}
#[test]
fn every_work_cutoff_preserves_partial_owner_and_sticky_first_denial() {
    let function = chain_fixture();
    let baseline = probe(&function, LIMIT, LIMIT, 0);
    let mut observed_live_path = false;
    for limit in 0..baseline.work {
        let p = probe(&function, limit, LIMIT, 0);
        assert!(!p.ok);
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.failed_work.is_some());
        assert_eq!(p.failed_storage, None);
        if p.lengths[3] > 0 {
            observed_live_path = true;
            assert_eq!(p.lengths[..3], [25, 25, 25]);
            assert!(p.capacities[3] >= p.lengths[3]);
        }
    }
    assert!(observed_live_path);
    assert!(probe(&function, baseline.work, LIMIT, 0).ok);
}
#[test]
fn exact_and_every_short_storage_frontier_preserve_attached_candidates() {
    let function = mixed_fixture();
    let baseline = probe(&function, LIMIT, LIMIT, 0);
    let mut observed_definitions_only = false;
    for limit in FLOOR..FLOOR + baseline.owned {
        let p = probe(&function, LIMIT, limit, 0);
        assert!(!p.ok);
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.failed_storage.is_some());
        assert_eq!(p.failed_work, None);
        observed_definitions_only |= p.lengths[0] == 14 && p.lengths[1] == 0;
    }
    assert!(observed_definitions_only);
    assert!(probe(&function, LIMIT, FLOOR + baseline.owned, 0).ok);
}
#[test]
fn occupied_success_and_failed_attempts_are_terminal_without_second_debit() {
    let function = chain_fixture();
    for storage in [0, LIMIT] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, storage);
        let mut owned = 0;
        let mut pending = RetainedConstantLocalsV1::new();
        {
            let mut resources = Resources::new(&mut budget, &mut owned);
            assert_eq!(
                pending.prepare_into(&function, &mut resources).is_ok(),
                storage == LIMIT
            );
        }
        let before = (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
            owned,
        );
        let lengths = [
            pending.definitions.len(),
            pending.states.len(),
            pending.values.len(),
            pending.path.len(),
        ];
        assert!(
            pending
                .prepare_into(&function, &mut Resources::new(&mut budget, &mut owned))
                .is_err()
        );
        assert_eq!(pending.phase, Phase::Terminal);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.failed_work(),
                budget.failed_storage(),
                owned
            ),
            before
        );
        assert_eq!(
            [
                pending.definitions.len(),
                pending.states.len(),
                pending.values.len(),
                pending.path.len()
            ],
            lengths
        );
        drop(pending);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn unmetered_and_prior_denial_do_not_start_original_analysis() {
    let function = mixed_fixture();
    let mut pending = RetainedConstantLocalsV1::new();
    assert!(
        pending
            .prepare_into(&function, &mut Resources::unmetered())
            .is_err()
    );
    assert_eq!(pending.phase, Phase::Terminal);
    assert!(pending.definitions.is_empty());
    for work_denial in [false, true] {
        let mut work = Work::new(if work_denial { 0 } else { LIMIT });
        let mut budget = Budget::new(&mut work, if work_denial { LIMIT } else { 0 });
        if work_denial {
            assert!(budget.charge_work(1).is_err());
        } else {
            assert!(budget.reserve_storage(1).is_err());
        }
        let before = (
            budget.work(),
            budget.storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        let mut owned = 0;
        let mut pending = RetainedConstantLocalsV1::new();
        assert!(
            pending
                .prepare_into(&function, &mut Resources::new(&mut budget, &mut owned))
                .is_err()
        );
        assert_eq!(owned, 0);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.failed_work(),
                budget.failed_storage()
            ),
            before
        );
        assert!(pending.definitions.is_empty());
    }
}
#[test]
fn detached_equal_function_and_different_physical_ledger_refuse_completed_loan() {
    let function = mixed_fixture();
    let detached = function.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedConstantLocalsV1::new();
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        pending.prepare_into(&function, &mut resources).unwrap();
        assert!(pending.completed_for(&detached, &resources).is_err());
        assert!(pending.completed_for(&function, &resources).is_ok());
    }
    let mut other_work = Work::new(LIMIT);
    let mut other_budget = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(
        pending
            .completed_for(
                &function,
                &Resources::new(&mut other_budget, &mut other_owned)
            )
            .is_err()
    );
    assert_eq!(other_budget.work(), 0);
    assert_eq!(other_owned, 0);
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn fresh_and_sticky_denied_owners_cannot_lend_completed_values() {
    let function = mixed_fixture();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut pending = RetainedConstantLocalsV1::new();
    {
        let mut resources = Resources::new(&mut budget, &mut owned);
        assert!(pending.completed_for(&function, &resources).is_err());
        pending.prepare_into(&function, &mut resources).unwrap();
    }
    assert!(budget.charge_work(LIMIT).is_err());
    assert!(
        pending
            .completed_for(&function, &Resources::new(&mut budget, &mut owned))
            .is_err()
    );
    drop(pending);
    budget.release_storage(owned).unwrap();
}
#[test]
fn wrapper_work_denial_precedes_storage_and_old_scan_preserves_work_order() {
    let function = mixed_fixture();
    let p = probe(&function, 31, FLOOR, 0);
    assert!(!p.ok);
    assert!(p.failed_work.is_some());
    assert_eq!(p.failed_storage, None);
    assert_eq!(p.owned, 0);
    let p = probe(&function, 32, FLOOR, 0);
    assert!(!p.ok);
    assert_eq!(p.failed_work, None);
    assert!(p.failed_storage.is_some());
    assert_eq!(p.work, 32);
}
#[test]
fn additive_typed_frame_is_checked_and_original_policy_is_not_replaced() {
    let rows = typed_rows().unwrap();
    assert_eq!(rows.len(), FRAME_ROWS);
    assert_eq!(
        rows.into_iter().try_fold(0usize, usize::checked_add),
        Some(retained_constant_frame_v1().unwrap())
    );
    let source = include_str!("bf16_nominal_retained_constant_locals_v1.rs");
    let compact: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();
    let body = &compact
        [compact.find("fnprepare_attached(").unwrap()..compact.find("fnfill_attached").unwrap()];
    assert!(body.contains("+4096,"));
    let mut position = 0;
    for needle in [
        "resources.reserve_storage(",
        "fill_attached(&mutself.definitions,",
        "forblockinfunction.blocks()",
        "fill_attached(&mutself.states,",
        "fill_attached(&mutself.values,",
        "resources.reserve(&mutself.path,",
        "forindexin0..self.definitions.len()",
        "resolve_constant_iterative_with_resources_v1(",
        "Ok(())",
    ] {
        let next = body[position..].find(needle).unwrap() + position;
        position = next + needle.len();
    }
    assert!(!body.contains("resources.filled("));
    assert!(!body.contains("constant_locals_with_resources_v1("));
    assert!(!compact.contains("release_storage("));
    assert!(!compact.contains("Budget::new("));
    assert!(!compact.contains("Work::new("));
}
