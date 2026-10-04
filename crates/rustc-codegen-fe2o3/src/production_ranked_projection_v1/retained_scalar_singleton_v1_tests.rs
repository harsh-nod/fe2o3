//! Synthetic component controls only; source helpers do not fabricate admission.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;
const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
// Exact helper copies follow; all fixture owners are inert model DATA.
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

fn projection_types() -> Vec<SemanticTypeDeclV1> {
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
            SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
            SemanticTypeShapeV1::Array {
                element: SCALAR_TYPE,
                length: 4,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(3)),
            SemanticLayoutIdentityV1::from_sha256(bytes(3)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    SCALAR_TYPE,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}
fn store(place: SemanticPlaceV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        place,
        constant(7),
        SemanticVolatilityV1::NonVolatile,
        None,
    )))
}
fn positive() -> SemanticFunctionDeclV1 {
    fixture(vec![store(place(1))], 4, None)
}
fn empty() -> SemanticFunctionDeclV1 {
    fixture(
        vec![assign(1, SemanticRvalueKindV1::Use(constant(7)))],
        4,
        None,
    )
}
struct OriginalMeter<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for OriginalMeter<'_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> R<()> {
        self.0.charge_work(amount).map_err(resource)
    }
    fn scalar_private_storage_v1(&self) -> R<usize> {
        Ok(self.0.storage())
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> R<()> {
        self.0.reserve_storage(amount).map_err(resource)
    }
    fn release_scalar_private_storage_v1(&mut self, amount: usize) -> R<()> {
        self.0.release_storage(amount).map_err(resource)
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> R<Option<u64>> {
        panic!("not an initializer proof")
    }
    fn is_materialized_block(&mut self, _: usize) -> R<bool> {
        panic!("not a CFG proof")
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
fn original(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
) -> (Vec<u8>, usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let flags = with_scalar_private_singletons_v1(
        types,
        function,
        &mut OriginalMeter(&mut budget),
        |flags, _| Ok(flags.to_vec()),
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (flags, budget.work(), budget.peak_storage() - FLOOR)
}
struct Probe {
    ok: bool,
    flags: Vec<u8>,
    phase: Phase,
    capacity: usize,
    work: usize,
    owned: usize,
    peak: usize,
    scan: bool,
    explicit: bool,
    denied: bool,
}
fn run(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work_limit: usize,
    storage_extra: usize,
) -> Probe {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR + storage_extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut value = RetainedScalarSingletonV1::new();
    let result = value.prepare_into(types, function, &mut Prep::new(&mut budget, &mut owned));
    if result.is_ok() {
        assert_eq!(
            value
                .completed_for(types, function, &Prep::new(&mut budget, &mut owned))
                .unwrap(),
            value.flags.as_slice()
        );
    }
    assert_eq!(budget.storage(), FLOOR + owned);
    // This copied assertion payload is test infrastructure outside the proposed
    // component protocol, never presented as a charged source-owned value.
    let probe = Probe {
        ok: result.is_ok(),
        flags: value.flags.clone(),
        phase: value.phase,
        capacity: value.flags.capacity(),
        work: budget.work(),
        owned,
        peak: budget.peak_storage(),
        scan: value.scan_invoked,
        explicit: value.explicit,
        denied: budget.failed_work().is_some() || budget.failed_storage().is_some(),
    };
    drop(result);
    drop(value);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    probe
}
#[test]
fn retained_singleton_complete_data_and_exact_original_debits_plus_frame() {
    let types = projection_types();
    let function = positive();
    let (expected, old_work, old_storage) = original(&types, &function);
    let actual = run(&types, &function, LIMIT, LIMIT);
    assert!(actual.ok && actual.scan && actual.explicit);
    assert_eq!(actual.phase, Phase::Complete);
    assert_eq!(actual.flags, expected);
    assert_eq!(
        actual.flags,
        [ELIGIBLE, ELIGIBLE | EXPLICIT, ELIGIBLE, ELIGIBLE]
    );
    assert_eq!(actual.work, old_work + 32);
    assert_eq!(actual.owned, old_storage + frame().unwrap());
    assert_eq!(actual.peak, FLOOR + actual.owned);
    assert!(!actual.denied);
}
#[test]
fn retained_singleton_original_no_candidate_scan_allocates_no_flags() {
    let types = projection_types();
    let function = empty();
    let (expected, old_work, old_storage) = original(&types, &function);
    let actual = run(&types, &function, LIMIT, LIMIT);
    assert!(expected.is_empty() && actual.ok && actual.scan && !actual.explicit);
    assert!(actual.flags.is_empty());
    assert_eq!(actual.capacity, 0);
    assert_eq!(old_storage, 0);
    assert_eq!(actual.owned, frame().unwrap());
    assert_eq!(actual.work, old_work + 32);
}
#[test]
fn retained_singleton_all_work_frontiers_preserve_partial_capacity_and_first_denial() {
    let types = projection_types();
    let function = positive();
    let full = run(&types, &function, LIMIT, LIMIT);
    let mut saw_attached = false;
    for cut in 0..full.work {
        let partial = run(&types, &function, cut, LIMIT);
        assert!(!partial.ok && partial.denied);
        assert_eq!(partial.phase, Phase::Terminal);
        assert!(partial.work <= cut);
        saw_attached |= partial.capacity > 0;
    }
    assert!(saw_attached);
    assert!(run(&types, &function, full.work, LIMIT).ok);
}
#[test]
fn retained_singleton_all_storage_frontiers_and_exact_capacity() {
    let types = projection_types();
    let function = positive();
    let full = run(&types, &function, LIMIT, LIMIT);
    for cut in 0..full.owned {
        let partial = run(&types, &function, LIMIT, cut);
        assert!(!partial.ok && partial.denied);
        assert!(partial.owned <= cut);
        assert_eq!(partial.phase, Phase::Terminal);
    }
    assert!(run(&types, &function, LIMIT, full.owned).ok);
}
#[test]
fn retained_singleton_malformed_local_and_type_keep_attached_flags() {
    let types = projection_types();
    for bad in [
        place(999),
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![],
            SemanticTypeIdV1::from_index(2),
        )
        .unwrap(),
    ] {
        let function = fixture(vec![store(bad)], 4, None);
        let partial = run(&types, &function, LIMIT, LIMIT);
        assert!(!partial.ok && !partial.denied);
        assert_eq!(partial.phase, Phase::Terminal);
        assert_eq!(partial.flags.len(), 4);
        assert!(partial.capacity >= 4);
    }
}
#[test]
fn retained_singleton_borrow_projection_and_volatile_blocks_exact_original_local() {
    let types = projection_types();
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE).unwrap()],
        SCALAR_TYPE,
    )
    .unwrap();
    let mutations = vec![
        assign(
            2,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1),
            },
        ),
        assign(
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
        ),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(1),
            constant(8),
            SemanticVolatilityV1::Volatile,
            None,
        ))),
    ];
    for item in mutations {
        let function = fixture(vec![store(place(1)), store(place(3)), item], 4, None);
        let expected = original(&types, &function).0;
        let actual = run(&types, &function, LIMIT, LIMIT);
        assert!(actual.ok);
        assert_eq!(actual.flags, expected);
        assert!(!eligible(&actual.flags, SemanticLocalIdV1::from_index(1)));
        assert!(eligible(&actual.flags, SemanticLocalIdV1::from_index(3)));
    }
}
#[test]
fn retained_singleton_retry_is_terminal_and_never_recharges_or_clears_data() {
    let types = projection_types();
    let function = positive();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedScalarSingletonV1::new();
    value
        .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let data = value.flags.clone();
    let before = (budget.work(), budget.storage(), owned);
    assert!(
        value
            .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    assert_eq!((budget.work(), budget.storage(), owned), before);
    assert_eq!(value.flags, data);
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    drop(value);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_singleton_source_types_function_and_counter_substitution_refuse() {
    let types = projection_types();
    let function = positive();
    let detached = function.clone();
    let copied = types.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedScalarSingletonV1::new();
    value
        .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    assert!(
        value
            .completed_for(&types, &detached, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    assert!(
        value
            .completed_for(&copied, &function, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    let mut substituted = owned;
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut substituted))
            .is_err()
    );
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
            .is_ok()
    );
    drop(value);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_singleton_foreign_ledger_and_released_live_floor_refuse() {
    let types = projection_types();
    let function = positive();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedScalarSingletonV1::new();
    value
        .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let mut other_owned = 0;
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut other, &mut other_owned))
            .is_err()
    );
    budget.release_storage(1).unwrap();
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    budget.reserve_storage(1).unwrap();
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
            .is_ok()
    );
    let held = owned;
    budget.release_storage(owned).unwrap();
    owned = 0;
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    budget.reserve_storage(held).unwrap();
    owned = held;
    assert!(
        value
            .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
            .is_ok()
    );
    drop(value);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_singleton_unmetered_fresh_view_and_sticky_denial_refuse() {
    let types = projection_types();
    let function = positive();
    let mut empty_owner = RetainedScalarSingletonV1::new();
    assert!(
        empty_owner
            .prepare_into(&types, &function, &mut Prep::unmetered())
            .is_err()
    );
    for storage_denial in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut value = RetainedScalarSingletonV1::new();
        assert!(
            value
                .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
                .is_err()
        );
        value
            .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        if storage_denial {
            assert!(budget.reserve_storage(LIMIT).is_err());
        } else {
            assert!(budget.charge_work(LIMIT).is_err());
        }
        assert!(
            value
                .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
                .is_err()
        );
        let before = (budget.work(), budget.storage(), owned);
        let mut fresh = RetainedScalarSingletonV1::new();
        assert!(
            fresh
                .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
                .is_err()
        );
        assert_eq!((budget.work(), budget.storage(), owned), before);
        drop(fresh);
        drop(value);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_singleton_callback_error_and_panic_leave_owner_until_explicit_drop() {
    let types = projection_types();
    let function = positive();
    for panic in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut value = RetainedScalarSingletonV1::new();
        value
            .prepare_into(&types, &function, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        let held = budget.storage();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> R<()> {
            assert_eq!(
                value
                    .completed_for(&types, &function, &Prep::new(&mut budget, &mut owned))
                    .unwrap()[1],
                ELIGIBLE | EXPLICIT
            );
            if panic {
                std::panic::panic_any(719usize);
            }
            Err(Error::Incomplete("singleton retained callback sentinel"))
        }));
        assert_eq!(budget.storage(), held);
        assert_eq!(value.flags.len(), 4);
        if panic {
            let payload = outcome.unwrap_err();
            assert_eq!(*payload.downcast::<usize>().unwrap(), 719);
        } else {
            assert!(matches!(
                outcome,
                Ok(Err(Error::Incomplete(
                    "singleton retained callback sentinel"
                )))
            ));
        }
        drop(value);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_singleton_snapshot_is_read_only_and_tracks_exact_counter_address() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let snapshot = Prep::new(&mut budget, &mut owned)
        .retained_custody_snapshot_v1()
        .unwrap();
    assert_eq!(snapshot.owned_slot, &owned as *const usize as usize);
    assert_eq!(snapshot.budget_slot, &budget as *const Budget<'_> as usize);
    assert_eq!(snapshot.owned, 0);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    assert!(Prep::unmetered().retained_custody_snapshot_v1().is_none());
}
#[test]
fn retained_singleton_required_semantic_adapter_methods_refuse() {
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
}
#[test]
fn retained_singleton_original_order_and_drop_authority_stay_visible() {
    let compact: String = include_str!("retained_scalar_singleton_v1.rs")
        .split_whitespace()
        .collect();
    let prepare = compact
        .split("fnprepare_original(")
        .nth(1)
        .unwrap()
        .split("fncheck(")
        .next()
        .unwrap();
    let mut last = 0;
    for needle in [
        "charge(&mutfacts,4)?;",
        "self.explicit=has_explicit_whole_place(",
        "if!self.explicit{returnOk(());}",
        "facts.0.reserve_storage(requested)?;",
        "self.flags.try_reserve_exact(count)",
        "facts.0.reserve_storage(excess)?;",
        "count.checked_mul(5)",
        "self.flags.push(",
        "letmutcensus=Census",
        "census.statement(",
        "census.terminator(",
    ] {
        let at = prepare
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle}"));
        assert!(at >= last, "out of order {needle}");
        last = at;
    }
    assert!(!compact.contains("Budget::new(") && !compact.contains("release_storage("));
}
