//! Exact original observer oracle over inert model DATA, not admitted ownership.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
const SCRATCH: usize = 97;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}

fn declaration(index: usize, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    let layout = match &shape {
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap()
        }
        SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_) => {
            let (size, second) = if index == 4 {
                (24, 8)
            } else if index >= 10 {
                (1u64 << (index - 6), 1u64 << (index - 7))
            } else {
                (16, 8)
            };
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                8,
                SemanticAggregateLayoutV1::new(vec![0, second], vec![]).unwrap(),
            )
            .unwrap()
        }
        _ => SemanticTypeLayoutV1::new(Some(if index == 7 { 16 } else { 8 }), 8).unwrap(),
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([index as u8 + 40; 32]),
        layout,
        shape,
    )
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let pointer = |pointee, kind, mutability, metadata| {
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(ty(pointee), kind, mutability, 5, 64, metadata)
                .unwrap(),
        )
    };
    let tuple = |fields: Vec<u32>| {
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(fields.into_iter().map(ty).collect()).unwrap(),
        )
    };
    let scalar = SemanticScalarTypeV1::Integer {
        signed: false,
        bits: 32,
    };
    vec![
        SemanticTypeShapeV1::Scalar(scalar),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        tuple(vec![0, 1]),
        tuple(vec![1, 1]),
        tuple(vec![0, 2]),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
        ),
        pointer(
            0,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
        pointer(
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::SliceLength,
        ),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                scalar,
                vec![SemanticScalarValidityRangeV1::new(1, u32::MAX as u128)],
            )
            .unwrap(),
        ),
        pointer(
            8,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, shape)| declaration(index, shape))
    .collect()
}

fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 100; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), terminator),
    )
    .unwrap()
}

fn function_with_locals(
    local_types: &[u32],
    blocks: Vec<SemanticBasicBlockV1>,
    role: SemanticFunctionRoleV1,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty(local_types[0]), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([203; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([204; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([205; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([206; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([207; 32]),
        source(),
        abi,
        local_types
            .iter()
            .enumerate()
            .map(|(index, &kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([index as u8 + 60; 32]),
                    ty(kind),
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn function_with_end(
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    function_with_locals(
        &[0, 0, 1, 1, 2, 0, 3, 4, 9, 8, 5, 6, 7, 8],
        vec![block(0, statements, terminator)],
        SemanticFunctionRoleV1::KernelRoot,
    )
}

fn function(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    function_with_end(statements, SemanticTerminatorKindV1::Return)
}

fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}

fn place(index: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], ty(kind)).unwrap()
}

fn projected(index: u32, projections: &[(SemanticProjectionKindV1, u32)]) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local(index),
        projections
            .iter()
            .map(|&(kind, result)| SemanticProjectionV1::new(kind, ty(result)).unwrap())
            .collect(),
        ty(projections.last().unwrap().1),
    )
    .unwrap()
}

fn dereference(index: u32, kind: u32) -> SemanticPlaceV1 {
    projected(index, &[(SemanticProjectionKindV1::Dereference, kind)])
}

struct OriginalMeter<'s, 'w>(&'s mut Budget<'w>);
impl BorrowWork for OriginalMeter<'_, '_> {
    fn work(&mut self, units: usize) -> AnalysisResult<()> {
        self.0
            .charge_work(units)
            .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)
    }
}
fn site(index: u32) -> SemanticTransparentBorrowSiteV1 {
    adapter::shared_primitive_v29::retained_test_site_v1(2, index)
}
fn rows(reads: &Reads) -> Vec<(u32, u32, u32, u32, u32, usize)> {
    reads
        .rows
        .iter()
        .map(|row| {
            (
                row.borrow.test_coordinates().0,
                row.borrow.test_coordinates().1,
                row.source,
                row.block,
                row.statement,
                row.place,
            )
        })
        .collect()
}
fn original(
    cap: usize,
    limit: usize,
    place: &SemanticPlaceV1,
) -> (
    Vec<(u32, u32, u32, u32, u32, usize)>,
    String,
    usize,
    Option<usize>,
) {
    let mut reads = Reads {
        rows: Vec::with_capacity(cap),
        cap,
    };
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut meter = OriginalMeter(&mut budget);
    let error = (0..=cap)
        .find_map(|index| {
            reads
                .read(site(index as u32), 1, (3, index as u32), place, &mut meter)
                .err()
        })
        .unwrap();
    (
        rows(&reads),
        format!("{error:?}"),
        budget.work(),
        budget.failed_work(),
    )
}
#[test]
fn retained_observer_delegates_exact_original_rows_work_and_cap_refusal() {
    let function = function(vec![]);
    let types = types();
    let place = dereference(2, 0);
    for cap in [0, 1, 3, 7] {
        for limit in 0..=cap + 1 {
            let expected = original(cap, limit, &place);
            let mut owner = RetainedSharedObserverV1::new();
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let mut failure = None;
            owner
                .prepare_into(
                    &function,
                    &types,
                    SCRATCH,
                    cap,
                    &mut budget,
                    &mut owned,
                    &mut failure,
                )
                .unwrap();
            let pointer = owner.reads.rows.as_ptr();
            let capacity = owner.reads.rows.capacity();
            let error = (0..=cap)
                .find_map(|index| {
                    owner
                        .record(
                            &function,
                            &types,
                            site(index as u32),
                            1,
                            (3, index as u32),
                            &place,
                            &mut budget,
                            &owned,
                            &mut failure,
                        )
                        .err()
                })
                .unwrap();
            assert_eq!(owner.test_rows(), expected.0);
            assert_eq!(format!("{error:?}"), expected.1);
            assert_eq!(budget.work(), expected.2);
            assert_eq!(budget.failed_work(), expected.3);
            assert_eq!(owner.reads.rows.as_ptr(), pointer);
            assert_eq!(owner.reads.rows.capacity(), capacity);
            assert_eq!(owner.phase, Phase::Terminal);
            assert_eq!(budget.storage(), FLOOR + owned);
            if limit <= cap {
                assert!(matches!(failure, Some(Resource::Work(_))));
            } else {
                assert!(failure.is_none());
            } // original cap error is not fabricated Resource
            assert!(
                owner
                    .postflight_for(&function, &types, &budget, &owned, &failure)
                    .is_err()
            );
            drop(owner);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
#[test]
fn retained_observer_every_storage_cut_keeps_rows_and_original_pair_coupled() {
    let function = function(vec![]);
    let types = types();
    let mut complete = RetainedSharedObserverV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut failure = None;
    complete
        .prepare_into(
            &function,
            &types,
            SCRATCH,
            3,
            &mut budget,
            &mut owned,
            &mut failure,
        )
        .unwrap();
    let needed = budget.storage();
    drop(complete);
    budget.release_storage(owned).unwrap();
    for limit in 0..needed {
        let mut owner = RetainedSharedObserverV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        let mut owned = 0;
        let mut failure = None;
        let error = owner
            .prepare_into(
                &function,
                &types,
                SCRATCH,
                3,
                &mut budget,
                &mut owned,
                &mut failure,
            )
            .unwrap_err();
        assert!(matches!(error, Resource::Storage(_)));
        assert_eq!(failure, Some(error));
        assert_eq!(budget.storage(), owned);
        assert!(owner.reads.rows.is_empty());
        assert_eq!(owner.phase, Phase::Terminal);
        assert_eq!(owner.scratch, SCRATCH);
        assert_eq!(owner.reads.cap, 3);
        let pointer = owner.reads.rows.as_ptr();
        let capacity = owner.reads.rows.capacity();
        let checkpoint = (
            budget.work(),
            budget.storage(),
            owned,
            budget.failed_storage(),
        );
        assert!(
            owner
                .prepare_into(
                    &function,
                    &types,
                    0,
                    0,
                    &mut budget,
                    &mut owned,
                    &mut failure
                )
                .is_err()
        );
        assert_eq!(
            checkpoint,
            (
                budget.work(),
                budget.storage(),
                owned,
                budget.failed_storage()
            )
        );
        assert_eq!(owner.reads.rows.as_ptr(), pointer);
        assert_eq!(owner.reads.rows.capacity(), capacity);
        assert_eq!(owner.scratch, SCRATCH);
        assert_eq!(owner.reads.cap, 3);
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_observer_successful_preparation_has_exact_observed_capacity_credits() {
    let function = function(vec![]);
    let types = types();
    for cap in [0, 1, 3, 7] {
        let mut owner = RetainedSharedObserverV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut failure = None;
        owner
            .prepare_into(
                &function,
                &types,
                SCRATCH,
                cap,
                &mut budget,
                &mut owned,
                &mut failure,
            )
            .unwrap();
        assert_eq!(
            owned,
            frame().unwrap() + SCRATCH + owner.reads.rows.capacity() * size_of::<Read>()
        );
        assert_eq!(owner.reads.cap, cap);
        assert!(owner.reads.rows.capacity() >= cap);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), FLOOR + owned);
        assert!(
            owner
                .postflight_for(&function, &types, &budget, &owned, &failure)
                .is_ok()
        );
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
#[test]
fn retained_observer_exact_function_types_counter_slot_and_ledger_are_bound() {
    let function = function(vec![]);
    let other_function = function.clone();
    let types = types();
    let other_types = types.clone();
    let mut owner = RetainedSharedObserverV1::new();
    let mut work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let mut owned = 0;
    let mut failure = None;
    owner
        .prepare_into(
            &function,
            &types,
            SCRATCH,
            3,
            &mut budget,
            &mut owned,
            &mut failure,
        )
        .unwrap();
    assert!(
        owner
            .postflight_for(&other_function, &types, &budget, &owned, &failure)
            .is_err()
    );
    assert!(
        owner
            .postflight_for(&function, &other_types, &budget, &owned, &failure)
            .is_err()
    );
    let counterfeit = owned;
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &counterfeit, &failure)
            .is_err()
    );
    other.reserve_storage(budget.storage()).unwrap();
    std::mem::swap(&mut budget, &mut other);
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned, &failure)
            .is_err()
    );
    std::mem::swap(&mut budget, &mut other);
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned, &failure)
            .is_ok()
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_observer_record_checkpoints_growth_and_detects_held_floor_rollback() {
    let function = function(vec![]);
    let types = types();
    let place = dereference(2, 0);
    let mut owner = RetainedSharedObserverV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut failure = None;
    owner
        .prepare_into(
            &function,
            &types,
            SCRATCH,
            3,
            &mut budget,
            &mut owned,
            &mut failure,
        )
        .unwrap();
    budget.reserve_storage(7).unwrap();
    owned += 7;
    owner
        .record(
            &function,
            &types,
            site(0),
            1,
            (3, 0),
            &place,
            &mut budget,
            &owned,
            &mut failure,
        )
        .unwrap();
    budget.release_storage(7).unwrap();
    owned -= 7;
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned, &failure)
            .is_err()
    );
    budget.reserve_storage(7).unwrap();
    owned += 7;
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned, &failure)
            .is_ok()
    );
    assert!(budget.charge_work(LIMIT + 1).is_err());
    assert!(
        owner
            .postflight_for(&function, &types, &budget, &owned, &failure)
            .is_err()
    );
    drop(owner);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_observer_terminal_entry_never_appends_or_changes_first_failure() {
    let function = function(vec![]);
    let types = types();
    let place = dereference(2, 0);
    for resource in [false, true] {
        let mut owner = RetainedSharedObserverV1::new();
        let mut work = Work::new(if resource { 0 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut failure = None;
        owner
            .prepare_into(
                &function,
                &types,
                SCRATCH,
                if resource { 2 } else { 0 },
                &mut budget,
                &mut owned,
                &mut failure,
            )
            .unwrap();
        assert!(
            owner
                .record(
                    &function,
                    &types,
                    site(0),
                    1,
                    (3, 0),
                    &place,
                    &mut budget,
                    &owned,
                    &mut failure
                )
                .is_err()
        );
        let checkpoint = snapshot(&budget, &owned);
        let first = failure;
        let pointer = owner.test_capacity();
        let rows = owner.test_rows();
        for _ in 0..3 {
            assert!(
                owner
                    .record(
                        &function,
                        &types,
                        site(1),
                        5,
                        (4, 1),
                        &place,
                        &mut budget,
                        &owned,
                        &mut failure
                    )
                    .is_err()
            );
            assert!(checkpoint == snapshot(&budget, &owned));
            assert_eq!(first, failure);
            assert_eq!(owner.test_capacity(), pointer);
            assert_eq!(owner.test_rows(), rows);
        }
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_observer_arithmetic_and_existing_denials_are_fail_closed_without_allocation() {
    let function = function(vec![]);
    let types = types();
    for variant in 0..3 {
        let mut owner = RetainedSharedObserverV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut failure = None;
        if variant == 1 {
            assert!(budget.charge_work(LIMIT + 1).is_err());
        }
        if variant == 2 {
            assert!(budget.reserve_storage(LIMIT + 1).is_err());
        }
        let error = owner
            .prepare_into(
                &function,
                &types,
                SCRATCH,
                usize::MAX,
                &mut budget,
                &mut owned,
                &mut failure,
            )
            .unwrap_err();
        assert_eq!(
            error,
            if variant == 0 {
                Resource::Arithmetic
            } else {
                Resource::Accounting
            }
        );
        assert_eq!(failure, Some(error));
        assert_eq!(owner.reads.rows.capacity(), 0);
        assert_eq!(owner.phase, Phase::Terminal);
        drop(owner);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_observer_rows_survive_outer_caller_error_or_unwind_until_explicit_drop() {
    let function = function(vec![]);
    let types = types();
    let place = dereference(2, 0);
    for unwind in [false, true] {
        let mut owner = RetainedSharedObserverV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut failure = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner
                .prepare_into(
                    &function,
                    &types,
                    SCRATCH,
                    3,
                    &mut budget,
                    &mut owned,
                    &mut failure,
                )
                .unwrap();
            owner
                .record(
                    &function,
                    &types,
                    site(0),
                    1,
                    (3, 0),
                    &place,
                    &mut budget,
                    &owned,
                    &mut failure,
                )
                .unwrap();
            if unwind {
                panic!("inert caller unwind");
            }
            Err::<(), &'static str>("inert caller error")
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(_))));
        }
        assert_eq!(owner.reads.rows.len(), 1);
        assert_eq!(budget.storage(), owned);
        assert_eq!(owner.test_rows()[0].5, &place as *const _ as usize);
        drop(owner);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_observer_source_delegation_keeps_original_read_and_allocation_order() {
    let source: String = include_str!("shared_primitive_reads_retained_observer_v1.rs")
        .split_whitespace()
        .collect::<String>()
        .replace(",)", ")");
    let parent: String = include_str!("shared_primitive_reads_v1.rs")
        .split_whitespace()
        .collect::<String>()
        .replace(",)", ")");
    let prep = source
        .split("fnprepare_into")
        .nth(1)
        .unwrap()
        .split("fncheck")
        .next()
        .unwrap();
    let scratch = prep.find("reserve(budget,owned,failure,scratch)").unwrap();
    let row = prep
        .find("reserve(budget,owned,failure,row_bytes)")
        .unwrap();
    let allocation = prep
        .find("self.reads.rows.try_reserve_exact(units)")
        .unwrap();
    let observed = prep.find("letactual=self.reads.rows.capacity()").unwrap();
    assert!(scratch < row && row < allocation && allocation < observed);
    assert!(source.contains("self.reads.read(borrow,source,site,place,&mutmeter)"));
    assert!(parent.contains("meter.work(1)?;ifself.rows.len()==self.cap"));
    assert!(!source.contains("release_storage("));
    assert!(!source.contains("Budget::new("));
}
