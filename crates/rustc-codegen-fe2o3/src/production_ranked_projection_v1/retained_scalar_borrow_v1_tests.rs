//! Inert synthetic model DATA only; no admission, SSA owner or pipeline route.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;
const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const REF_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
fn bytes(value: u8) -> [u8; 32] {
    [value; 32]
}

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
fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(1)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(2)),
            SemanticLayoutIdentityV1::from_sha256(bytes(2)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SCALAR_TYPE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}
fn target() -> SemanticTargetDataLayoutV1 {
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(90)))
}
fn typed_place(index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap()
}
fn assign(index: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        typed_place(index, ty),
        SemanticRvalueV1::new(ty, value),
    )))
}
fn write(value: u128) -> SemanticStatementV1 {
    assign(
        1,
        SCALAR_TYPE,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))),
    )
}
fn borrow(kind: SemanticBorrowKindV1) -> SemanticStatementV1 {
    assign(
        2,
        REF_TYPE,
        SemanticRvalueKindV1::Borrow {
            kind,
            place: typed_place(1, SCALAR_TYPE),
        },
    )
}
fn read() -> SemanticStatementV1 {
    assign(
        3,
        SCALAR_TYPE,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                        .unwrap(),
                ],
                SCALAR_TYPE,
            )
            .unwrap(),
        )),
    )
}
fn statements() -> Vec<SemanticStatementV1> {
    vec![
        write(7),
        borrow(SemanticBorrowKindV1::Mutable),
        read(),
        read(),
    ]
}
fn fixture(rows: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    projection_function_with_locals(
        vec![
            block(
                81,
                rows,
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(82, vec![], SemanticTerminatorKindV1::Return),
        ],
        vec![
            local(10, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(11, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(12, REF_TYPE, SemanticLocalRoleV1::Temporary),
            local(13, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    )
}
struct OriginalMeter<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for OriginalMeter<'_, '_> {
    fn charge_private_array_work(&mut self, n: usize) -> R<()> {
        self.0.charge_work(n).map_err(resource)
    }
    fn scalar_private_storage_v1(&self) -> R<usize> {
        Ok(self.0.storage())
    }
    fn reserve_scalar_private_storage_v1(&mut self, n: usize) -> R<()> {
        self.0.reserve_storage(n).map_err(resource)
    }
    fn release_scalar_private_storage_v1(&mut self, n: usize) -> R<()> {
        self.0.release_storage(n).map_err(resource)
    }
    fn helper_value_ledger_v1(&self) -> R<(usize, CanonicalKernelIrWorkLedgerIdentityV1)> {
        Ok((
            self.0 as *const Budget<'_> as usize,
            self.0.work_ledger_identity_v1(),
        ))
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> R<Option<u64>> {
        panic!("not initializer facts")
    }
    fn is_materialized_block(&mut self, _: usize) -> R<bool> {
        panic!("not CFG facts")
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> R<canonical_assertion_facts_v1::ProjectedAssertionConditionV1> {
        panic!("not assertion facts")
    }
}
type LocalData = (
    Option<usize>,
    Option<(usize, usize, usize, usize)>,
    bool,
    bool,
    bool,
    bool,
);
#[derive(Debug, Eq, PartialEq)]
struct Data {
    function: usize,
    types: usize,
    type_count: usize,
    target: SemanticTargetDataLayoutV1,
    locals: Vec<LocalData>,
    starts: Vec<usize>,
    reads: Vec<Option<(usize, usize)>>,
    capacities: [usize; 3],
}
fn data(c: &ScalarPrivateBorrowsV1<'_>) -> Data {
    Data {
        function: c.function as *const _ as usize,
        types: c.types.as_ptr() as usize,
        type_count: c.types.len(),
        target: c.target,
        locals: c
            .locals
            .iter()
            .map(|r| {
                (
                    r.root_alias,
                    r.candidate.map(|v| (v.root, v.alias, v.block, v.statement)),
                    r.initialized,
                    r.ever_initialized,
                    r.alias_alive,
                    r.blocked,
                )
            })
            .collect(),
        starts: c.starts.clone(),
        reads: c
            .reads
            .iter()
            .map(|r| r.map(|r| (r.place as *const _ as usize, r.alias)))
            .collect(),
        capacities: [c.locals.capacity(), c.starts.capacity(), c.reads.capacity()],
    }
}
type Query = Result<Option<u32>, String>;
fn queries(
    census: Option<&ScalarPrivateBorrowsV1<'_>>,
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Vec<Query> {
    let mut result = Vec::new();
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, row) in body.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) = row.kind() else {
                continue;
            };
            let place = match a.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => place,
                SemanticRvalueKindV1::Load(load) => load.source(),
                _ => continue,
            };
            // Explicit fixture input, not a claim of computed provenance authority.
            result.push(match census {
                None => Ok(None),
                Some(c) => c
                    .resolve(
                        function,
                        types,
                        target(),
                        Site {
                            block,
                            statement: Some(statement),
                        },
                        place,
                        AccessKindAttr::Read,
                        None,
                        Some(LocalAllocationProvenanceV1::Private(
                            SemanticLocalIdV1::from_index(1),
                        )),
                        facts,
                    )
                    .map(|v| v.map(|v| v.index()))
                    .map_err(|e| format!("{e:?}")),
            });
        }
    }
    result
}
struct Expected {
    data: Option<Data>,
    queries: Vec<Query>,
    work: usize,
    storage: usize,
}
fn original(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    resolve: bool,
) -> Expected {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut payload = None;
    let mut observed = Vec::new();
    with_scalar_private_borrows_v1(
        types,
        function,
        target(),
        &mut OriginalMeter(&mut budget),
        |c, facts| {
            if let Some(c) = c {
                assert!(c.ledger == facts.helper_value_ledger_v1()?);
                assert_eq!(c.live_floor, facts.scalar_private_storage_v1()?);
                payload = Some(data(c));
            }
            if resolve {
                observed = queries(c, types, function, facts);
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    Expected {
        data: payload,
        queries: observed,
        work: budget.work(),
        storage: budget.peak_storage() - FLOOR,
    }
}
fn original_error(types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> String {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let error = with_scalar_private_borrows_v1(
        types,
        function,
        target(),
        &mut OriginalMeter(&mut budget),
        |_, _| Ok(()),
    )
    .unwrap_err();
    assert_eq!(budget.storage(), 0);
    format!("{error:?}")
}
struct Probe {
    ok: bool,
    error: Option<String>,
    phase: Phase,
    data: Option<Data>,
    queries: Vec<Query>,
    work: usize,
    owned: usize,
    capacities: [usize; 3],
    denied: bool,
    found: bool,
}
fn run(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work_limit: usize,
    storage: usize,
    resolve: bool,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR + storage);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut owner = RetainedScalarBorrowsV1::new();
    let result = owner.prepare_into(
        types,
        function,
        target(),
        &mut Prep::new(&mut budget, &mut owned),
    );
    let mut observed = Vec::new();
    if result.is_ok() {
        let census = owner
            .completed_for(
                types,
                function,
                target(),
                &Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        if let Some(c) = census {
            assert!(
                c.ledger
                    == (
                        &budget as *const Budget<'_> as usize,
                        budget.work_ledger_identity_v1()
                    )
            );
            assert_eq!(c.live_floor, budget.storage());
        }
        if resolve {
            observed = queries(
                census,
                types,
                function,
                &mut Meter(&mut Prep::new(&mut budget, &mut owned)),
            );
        }
    }
    assert_eq!(budget.storage(), FLOOR + owned);
    let probe = Probe {
        ok: result.is_ok(),
        error: result.as_ref().err().map(|e| format!("{e:?}")),
        phase: owner.phase,
        data: owner.census.as_ref().map(data),
        queries: observed,
        work: budget.work(),
        owned,
        capacities: owner.census.as_ref().map_or([0; 3], |c| {
            [c.locals.capacity(), c.starts.capacity(), c.reads.capacity()]
        }),
        denied: budget.failed_work().is_some() || budget.failed_storage().is_some(),
        found: owner.found,
    };
    drop(result);
    drop(owner);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    probe
}
fn compare(types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> Probe {
    let old = original(types, function, true);
    let actual = run(types, function, LIMIT, LIMIT, true);
    assert!(actual.ok);
    assert_eq!(actual.phase, Phase::Complete);
    assert_eq!(actual.data, old.data);
    assert_eq!(actual.queries, old.queries);
    assert_eq!(actual.work, old.work + 32);
    assert_eq!(actual.owned, old.storage + frame().unwrap());
    actual
}
#[test]
fn retained_borrow_complete_original_data_and_resolve_results() {
    let types = types();
    let function = fixture(statements());
    let actual = compare(&types, &function);
    assert_eq!(actual.queries, [Ok(Some(1)), Ok(Some(1))]);
    assert!(actual.found && !actual.denied);
    let d = actual.data.unwrap();
    assert_eq!(d.starts, [0, 4, 4]);
    assert_eq!(d.locals.len(), 4);
    assert_eq!(d.locals[1], (Some(2), None, true, true, false, false));
    assert_eq!(
        d.locals[2],
        (None, Some((1, 2, 0, 1)), false, false, true, false)
    );
    assert!(d.reads[0].is_none() && d.reads[1].is_none());
    assert!(d.reads[2].is_some() && d.reads[3].is_some());
}
#[test]
fn retained_borrow_no_candidate_preserves_linear_scan_without_census_allocation() {
    let types = types();
    let function = fixture(vec![write(7), borrow(SemanticBorrowKindV1::Shared), read()]);
    let actual = compare(&types, &function);
    assert!(!actual.found && actual.data.is_none());
    assert_eq!(actual.capacities, [0; 3]);
    assert_eq!(actual.owned, frame().unwrap());
    assert_eq!(actual.queries, [Ok(None)]);
}
#[test]
fn retained_borrow_all_work_frontiers_keep_each_attached_partial_vector() {
    let types = types();
    let function = fixture(statements());
    let full = run(&types, &function, LIMIT, LIMIT, false);
    let mut seen = [false; 3];
    for limit in 0..full.work {
        let p = run(&types, &function, limit, LIMIT, false);
        assert!(!p.ok && p.denied);
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.work <= limit);
        for i in 0..3 {
            seen[i] |= p.capacities[i] > 0;
        }
    }
    assert_eq!(seen, [true; 3]);
    assert!(run(&types, &function, full.work, LIMIT, false).ok);
}
#[test]
fn retained_borrow_all_storage_frontiers_keep_original_allocation_order() {
    let types = types();
    let function = fixture(statements());
    let full = run(&types, &function, LIMIT, LIMIT, false);
    let mut locals_only = false;
    let mut locals_starts = false;
    for limit in 0..full.owned {
        let p = run(&types, &function, LIMIT, limit, false);
        assert!(!p.ok && p.denied);
        assert_eq!(p.phase, Phase::Terminal);
        assert!(p.owned <= limit);
        locals_only |= p.capacities[0] > 0 && p.capacities[1] == 0;
        locals_starts |= p.capacities[0] > 0 && p.capacities[1] > 0 && p.capacities[2] == 0;
    }
    assert!(locals_only && locals_starts);
    assert!(run(&types, &function, LIMIT, full.owned, false).ok);
}
#[test]
fn retained_borrow_malformed_source_preserves_discovery_or_full_partial_census() {
    let types = types();
    let good = fixture(statements());
    let before = run(&[], &good, LIMIT, LIMIT, false);
    assert!(!before.ok && !before.denied && before.data.is_none());
    assert_eq!(before.error, Some(original_error(&[], &good)));
    let mut rows = statements();
    rows[2] = assign(
        3,
        SCALAR_TYPE,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(typed_place(999, SCALAR_TYPE))),
    );
    let bad = fixture(rows);
    let partial = run(&types, &bad, LIMIT, LIMIT, false);
    assert!(!partial.ok && !partial.denied);
    assert!(partial.capacities.into_iter().all(|v| v > 0));
    assert_eq!(partial.phase, Phase::Terminal);
    assert_eq!(partial.error, Some(original_error(&types, &bad)));
}
#[test]
fn retained_borrow_lifetime_restart_escape_and_postborrow_writes_match_original() {
    let types = types();
    for fault in [
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(2),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(2),
        )),
        assign(
            2,
            REF_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(typed_place(2, REF_TYPE))),
        ),
        write(9),
    ] {
        let mut rows = statements();
        rows.insert(2, fault);
        let function = fixture(rows);
        let p = compare(&types, &function);
        assert!(p.queries.iter().all(|q| matches!(q, Ok(None))));
    }
    for sequence in [vec![false], vec![true], vec![false, true]] {
        let mut rows = vec![write(7)];
        for live in sequence {
            rows.push(statement(if live {
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1))
            } else {
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1))
            }));
        }
        rows.extend([
            write(9),
            borrow(SemanticBorrowKindV1::Mutable),
            read(),
            read(),
        ]);
        let function = fixture(rows);
        let p = compare(&types, &function);
        assert_eq!(p.queries, [Ok(None), Ok(None)]);
    }
}
#[test]
fn retained_borrow_repeated_preborrow_writes_and_candidate_overlap_match_original() {
    let types = types();
    let mut rows = statements();
    rows.insert(1, write(9));
    assert_eq!(
        compare(&types, &fixture(rows)).queries,
        [Ok(Some(1)), Ok(Some(1))]
    );
    let mut rows = statements();
    rows.insert(2, borrow(SemanticBorrowKindV1::Mutable));
    assert_eq!(
        compare(&types, &fixture(rows)).queries,
        [Ok(None), Ok(None)]
    );
}
#[test]
fn retained_borrow_explicit_loads_keep_volatile_and_atomic_exclusions() {
    let types = types();
    for case in 0..3 {
        let mut rows = statements();
        rows[2] = assign(
            3,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(2),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            SCALAR_TYPE,
                        )
                        .unwrap(),
                    ],
                    SCALAR_TYPE,
                )
                .unwrap(),
                if case == 1 {
                    SemanticVolatilityV1::Volatile
                } else {
                    SemanticVolatilityV1::NonVolatile
                },
                if case == 2 {
                    Some(SemanticAtomicAccessV1::new(
                        SemanticAtomicOrderingV1::Relaxed,
                        SemanticAtomicScopeV1::SingleThread,
                    ))
                } else {
                    None
                },
            )),
        );
        let actual = compare(&types, &fixture(rows));
        assert_eq!(
            actual.queries,
            if case == 0 {
                vec![Ok(Some(1)), Ok(Some(1))]
            } else {
                vec![Ok(None), Ok(None)]
            }
        );
    }
}
#[test]
fn retained_borrow_call_operands_and_cross_block_reads_do_not_gain_waivers() {
    let types = types();
    let source = fixture(statements());
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![SemanticOperandV1::Copy(typed_place(2, REF_TYPE))],
            Some(SemanticCallDestinationV1::new(
                typed_place(0, SCALAR_TYPE),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let called = projection_function_with_locals(
        vec![
            block(81, statements(), call),
            block(82, vec![], SemanticTerminatorKindV1::Return),
        ],
        source.locals().to_vec(),
    );
    assert_eq!(compare(&types, &called).queries, [Ok(None), Ok(None)]);
    let crossed = projection_function_with_locals(
        vec![
            block(
                81,
                statements(),
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(82, vec![read()], SemanticTerminatorKindV1::Return),
        ],
        source.locals().to_vec(),
    );
    assert_eq!(
        compare(&types, &crossed).queries,
        [Ok(None), Ok(None), Ok(None)]
    );
}
#[test]
fn retained_borrow_exact_place_identity_target_and_private_provenance_stay_required() {
    let types = types();
    let function = fixture(statements());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut owner = RetainedScalarBorrowsV1::new();
    owner
        .prepare_into(
            &types,
            &function,
            target(),
            &mut Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    let c = owner
        .completed_for(
            &types,
            &function,
            target(),
            &Prep::new(&mut budget, &mut owned),
        )
        .unwrap()
        .unwrap();
    let read = c.reads[2].unwrap();
    let foreign = read.place.clone();
    let site = Site {
        block: 0,
        statement: Some(2),
    };
    let provenance = Some(LocalAllocationProvenanceV1::Private(
        SemanticLocalIdV1::from_index(1),
    ));
    let mut prep = Prep::new(&mut budget, &mut owned);
    let mut meter = Meter(&mut prep);
    assert!(
        c.resolve(
            &function,
            &types,
            target(),
            site,
            &foreign,
            AccessKindAttr::Read,
            None,
            provenance,
            &mut meter
        )
        .unwrap()
        .is_none()
    );
    assert!(
        c.resolve(
            &function,
            &types,
            target(),
            site,
            read.place,
            AccessKindAttr::Read,
            None,
            None,
            &mut meter
        )
        .is_err()
    );
    assert!(
        c.resolve(
            &function,
            &types,
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(91))),
            site,
            read.place,
            AccessKindAttr::Read,
            None,
            provenance,
            &mut meter
        )
        .is_err()
    );
    assert!(
        c.resolve(
            &function,
            &types,
            target(),
            site,
            read.place,
            AccessKindAttr::Write,
            None,
            provenance,
            &mut meter
        )
        .unwrap()
        .is_none()
    );
    assert!(
        c.resolve(
            &function,
            &types,
            target(),
            site,
            read.place,
            AccessKindAttr::Read,
            Some(SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::SingleThread
            )),
            provenance,
            &mut meter
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(
        c.resolve(
            &function,
            &types,
            target(),
            site,
            read.place,
            AccessKindAttr::Read,
            None,
            provenance,
            &mut meter
        )
        .unwrap(),
        Some(SemanticLocalIdV1::from_index(1))
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_borrow_one_shot_and_source_counter_substitution_refuse() {
    let types = types();
    let function = fixture(statements());
    let detached = function.clone();
    let copied = types.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut owner = RetainedScalarBorrowsV1::new();
    owner
        .prepare_into(
            &types,
            &function,
            target(),
            &mut Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    assert!(
        owner
            .completed_for(
                &types,
                &detached,
                target(),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert!(
        owner
            .completed_for(
                &copied,
                &function,
                target(),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(
                    91
                ))),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    let mut other_owned = owned;
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                target(),
                &Prep::new(&mut budget, &mut other_owned)
            )
            .is_err()
    );
    let before = (budget.work(), budget.storage(), owned);
    let saved = owner.census.as_ref().map(data);
    assert!(
        owner
            .prepare_into(
                &types,
                &function,
                target(),
                &mut Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    assert_eq!(owner.census.as_ref().map(data), saved);
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                target(),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_borrow_foreign_ledger_and_coupled_live_credit_rollback_refuse() {
    let types = types();
    let function = fixture(statements());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut owner = RetainedScalarBorrowsV1::new();
    owner
        .prepare_into(
            &types,
            &function,
            target(),
            &mut Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                target(),
                &Prep::new(&mut other, &mut other_owned)
            )
            .is_err()
    );
    let held = owned;
    budget.release_storage(1).unwrap();
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                target(),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    budget.reserve_storage(1).unwrap();
    budget.release_storage(owned).unwrap();
    owned = 0;
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                target(),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    budget.reserve_storage(held).unwrap();
    owned = held;
    assert!(
        owner
            .completed_for(
                &types,
                &function,
                target(),
                &Prep::new(&mut budget, &mut owned)
            )
            .is_ok()
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_borrow_incomplete_unmetered_and_sticky_denials_refuse() {
    let types = types();
    let function = fixture(statements());
    let mut empty = RetainedScalarBorrowsV1::new();
    assert!(
        empty
            .prepare_into(&types, &function, target(), &mut Prep::unmetered())
            .is_err()
    );
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut owner = RetainedScalarBorrowsV1::new();
        assert!(
            owner
                .completed_for(
                    &types,
                    &function,
                    target(),
                    &Prep::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        owner
            .prepare_into(
                &types,
                &function,
                target(),
                &mut Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        if storage {
            assert!(budget.reserve_storage(LIMIT).is_err());
        } else {
            assert!(budget.charge_work(LIMIT).is_err());
        }
        assert!(
            owner
                .completed_for(
                    &types,
                    &function,
                    target(),
                    &Prep::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        let before = (budget.work(), budget.storage(), owned);
        let mut fresh = RetainedScalarBorrowsV1::new();
        assert!(
            fresh
                .prepare_into(
                    &types,
                    &function,
                    target(),
                    &mut Prep::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert_eq!(before, (budget.work(), budget.storage(), owned));
        drop(fresh);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_borrow_callback_error_or_panic_keeps_complete_census_until_owner_drop() {
    let types = types();
    let function = fixture(statements());
    for panic in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut owner = RetainedScalarBorrowsV1::new();
        owner
            .prepare_into(
                &types,
                &function,
                target(),
                &mut Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        let held = budget.storage();
        let before = owner.census.as_ref().map(data);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> R<()> {
            assert!(
                owner
                    .completed_for(
                        &types,
                        &function,
                        target(),
                        &Prep::new(&mut budget, &mut owned)
                    )
                    .unwrap()
                    .is_some()
            );
            if panic {
                std::panic::panic_any(823usize);
            }
            Err(Error::Incomplete("retained borrow callback sentinel"))
        }));
        assert_eq!(budget.storage(), held);
        assert_eq!(owner.census.as_ref().map(data), before);
        if panic {
            assert_eq!(*outcome.unwrap_err().downcast::<usize>().unwrap(), 823);
        } else {
            assert!(matches!(
                outcome,
                Ok(Err(Error::Incomplete("retained borrow callback sentinel")))
            ));
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_borrow_semantic_adapter_and_refund_methods_refuse() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut prep = Prep::new(&mut budget, &mut owned);
    let mut meter = Meter(&mut prep);
    assert!(meter.private_array_initializer_count(0, 0).is_err());
    assert!(meter.is_materialized_block(0).is_err());
    assert!(
        meter
            .condition(0, true, SemanticBlockIdV1::from_index(0))
            .is_err()
    );
    assert!(meter.release_scalar_private_storage_v1(0).is_err());
    assert_eq!((budget.work(), budget.storage(), owned), (0, 0, 0));
}
#[test]
fn retained_borrow_original_operation_order_and_no_refund_stay_visible() {
    let compact: String = include_str!("retained_scalar_borrow_v1.rs")
        .split_whitespace()
        .collect();
    let discovery = compact
        .split("fnprepare_original(")
        .nth(1)
        .unwrap()
        .split("fncheck(")
        .next()
        .unwrap();
    let mut previous = 0;
    for needle in [
        "charge(facts,6)?;",
        "self.scan_invoked=true;",
        "self.found|=candidate(",
        "if!self.found{returnOk(());}",
        "self.census=Some(",
        "build_into(",
    ] {
        let at = discovery.find(needle).unwrap();
        assert!(at >= previous);
        previous = at;
    }
    let build = compact
        .split("fnbuild_into<")
        .nth(1)
        .unwrap()
        .split("fnframe(")
        .next()
        .unwrap();
    previous = 0;
    for needle in [
        "reserve_scalar_private_storage_v1(",
        "allocate_into(&mutcensus.locals,",
        "allocate_into(&mutcensus.starts,",
        "allocate_into(&mutcensus.reads,",
        "census.ledger=facts.helper_value_ledger_v1()?;",
        "ifletSome(c)=candidate(",
        "letmutscan=Scan",
        "scan.statement(value.kind())?;",
        "scan.terminator(",
        "census.live_floor=",
    ] {
        let at = build.find(needle).unwrap();
        assert!(at >= previous);
        previous = at;
    }
    assert!(!compact.contains("Budget::new(") && !compact.contains(".release_storage("));
}
