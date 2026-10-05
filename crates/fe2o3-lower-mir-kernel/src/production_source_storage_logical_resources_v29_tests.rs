use super::layout_tests::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticProjectionKindV1, SemanticProjectionV1};

fn logical_resource(error: Error) -> ArgumentResourceV1 {
    match error {
        Error::ArgumentCorrespondenceResource(error) => error,
        other => panic!("expected resource denial, got {other:?}"),
    }
}

fn logical_state_header() -> usize {
    size_of::<SourceStorageStateV29<'_, '_>>()
        + size_of::<Result<SourceStorageStateV29<'_, '_>, Error>>()
}

#[derive(Clone, Copy, Debug)]
enum LogicalOperation {
    State,
    Root,
    Field,
    Copy,
    Join,
}

impl LogicalOperation {
    fn work(self) -> usize {
        match self {
            Self::State | Self::Root => 1,
            // Original owner1; projection visit1; typed step1; empty copied
            // path vector3; path push2 plus its first capacity-four vector3.
            Self::Field => 1 + 1 + 1 + 3 + 2 + 3,
            // Source constructor1, empty facts/bytes/enums vectors3 each.
            Self::Copy => 1 + 3 + 3 + 3,
            // Source constructor1; one order row(vector3,push2); root1;
            // two empty proofs(check1,vector3), intersection vector3;
            // replacement(check1,proof4,vector3); empty byte vector3.
            Self::Join => 1 + 3 + 2 + 1 + 4 + 4 + 3 + (1 + 4 + 3) + 3,
        }
    }

    fn peak(self) -> usize {
        let place = size_of::<SourceStorageSubobjectV29<'_, '_>>();
        match self {
            Self::State | Self::Copy => logical_state_header(),
            Self::Root => place,
            Self::Field => 2 * place + 4 * size_of::<SubobjectStep>(),
            Self::Join => {
                let proof = size_of::<InitializationProofV29>()
                    + size_of::<Result<InitializationProofV29, Error>>();
                logical_state_header()
                    + size_of::<Vec<(usize, usize)>>()
                    + size_of::<(usize, usize)>()
                    + place
                    + 4 * proof
                    + 2 * size_of::<Vec<InitializationFact<'_, '_>>>()
                    + size_of::<InitializationFact<'_, '_>>()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogicalBoundary {
    Exact,
    WorkShort,
    StorageShort,
}

fn logical_boundary(operation: LogicalOperation, boundary: LogicalBoundary) {
    const WORK: usize = 2_000_000;
    const STORAGE: usize = 64 * 1024 * 1024;
    const FLOOR: usize = 73;
    let owner = owner();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[], budget).unwrap();
            assert!(layouts.rows(instances.owner(), budget).unwrap().is_empty());
            assert!(layouts.row_for(&owner, TWO_ZST, budget).is_err());
            let state = SourceStorageStateV29::new_source(
                &layouts,
                instances,
                instances.root(),
                local_for(TWO_ZST),
                budget,
            )
            .unwrap();
            assert_eq!(state.physical_root, None);
            assert!(state.facts.is_empty());
            let allowed_work = operation.work() - usize::from(boundary == LogicalBoundary::WorkShort);
            let allowed_storage = operation.peak() - usize::from(boundary == LogicalBoundary::StorageShort);
            budget.charge_work(WORK - budget.work() - allowed_work).unwrap();
            let pressure = STORAGE - budget.storage() - allowed_storage;
            budget.reserve_storage(pressure).unwrap();
            let before_work = budget.work();
            let before_storage = budget.storage();
            let result = match operation {
                LogicalOperation::State => SourceStorageStateV29::new_source(
                    &layouts,
                    instances,
                    instances.root(),
                    local_for(TWO_ZST),
                    budget,
                ).map(drop),
                LogicalOperation::Root => layouts.source_root_subobject(&owner, TWO_ZST, budget).map(drop),
                LogicalOperation::Field => layouts.source_projected_subobject(
                    &owner,
                    TWO_ZST,
                    &[SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), UNIT).unwrap()],
                    budget,
                ).map(drop),
                LogicalOperation::Copy => state.copy(budget).map(drop),
                LogicalOperation::Join => state.join(&state, budget).map(drop),
            };
            let first = result.err().map(logical_resource);
            match boundary {
                LogicalBoundary::Exact => {
                    assert_eq!(first, None, "{operation:?}");
                    assert_eq!(budget.work(), before_work + operation.work());
                    assert_eq!(budget.peak_storage(), before_storage + operation.peak());
                }
                LogicalBoundary::WorkShort => {
                    assert!(matches!(first, Some(ArgumentResourceV1::Work(error))
                        if error.actual() == before_work + operation.work() && error.limit() == WORK), "{operation:?}: {first:?}");
                    assert!(budget.work() < before_work + operation.work());
                }
                LogicalBoundary::StorageShort => {
                    assert!(matches!(first, Some(ArgumentResourceV1::Storage(error))
                        if error.actual() == before_storage + operation.peak() && error.limit() == STORAGE), "{operation:?}: {first:?}");
                    assert!(budget.peak_storage() < before_storage + operation.peak());
                }
            }
            assert_eq!(layouts.lease.failure.resource(), first);
            if let Some(first) = first {
                let counters = (budget.work(), budget.storage(), budget.peak_storage());
                assert_eq!(logical_resource(layouts.lease.work(0, budget).unwrap_err()), first);
                assert_eq!(logical_resource(layouts.lease.reserve(usize::MAX, budget).unwrap_err()), first);
                assert_eq!(counters, (budget.work(), budget.storage(), budget.peak_storage()));
            }
            drop(state);
            assert_eq!(layouts.release(budget).err().map(logical_resource), first);
            assert_eq!(budget.storage(), floor + pressure);
            budget.release_storage(pressure).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    ).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn logical_boundaries(operation: LogicalOperation) {
    for boundary in [
        LogicalBoundary::Exact,
        LogicalBoundary::WorkShort,
        LogicalBoundary::StorageShort,
    ] {
        logical_boundary(operation, boundary);
    }
}

#[test]
fn source_only_state_constructor_has_independent_exact_and_short_limits() {
    logical_boundaries(LogicalOperation::State);
}

#[test]
fn source_only_root_geometry_has_independent_exact_and_short_limits() {
    logical_boundaries(LogicalOperation::Root);
}

#[test]
fn source_only_zero_width_projection_has_independent_exact_and_short_limits() {
    logical_boundaries(LogicalOperation::Field);
}

#[test]
fn source_only_copy_has_independent_exact_and_short_limits() {
    logical_boundaries(LogicalOperation::Copy);
}

#[test]
fn source_only_live_join_has_independent_exact_and_short_limits() {
    logical_boundaries(LogicalOperation::Join);
}

#[test]
fn source_only_zero_width_facts_are_logical_and_preserve_siblings() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[], budget).unwrap();
            let mut state = SourceStorageStateV29::new_source(
                &layouts,
                instances,
                instances.root(),
                local_for(TWO_ZST),
                budget,
            )
            .unwrap();
            let root = state.root_subobject(budget).unwrap();
            let left = state
                .projected_subobject(
                    &[
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), UNIT)
                            .unwrap(),
                    ],
                    budget,
                )
                .unwrap();
            let right = state
                .projected_subobject(
                    &[
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), UNIT)
                            .unwrap(),
                    ],
                    budget,
                )
                .unwrap();
            assert_eq!(left.range.length(), 0);
            assert_eq!(right.range, left.range);
            assert_ne!(left.path, right.path);
            assert!(!state.is_initialized(&root, budget).unwrap());
            assert!(!state.is_initialized(&left, budget).unwrap());
            state.initialize(&left, budget).unwrap();
            assert!(state.is_initialized(&left, budget).unwrap());
            assert!(!state.is_initialized(&right, budget).unwrap());
            assert!(!state.is_initialized(&root, budget).unwrap());
            state.initialize(&right, budget).unwrap();
            assert!(state.is_initialized(&root, budget).unwrap());
            let valid = state.copy(budget).unwrap();
            state.deinitialize(&left, budget).unwrap();
            assert!(!state.is_initialized(&left, budget).unwrap());
            assert!(state.is_initialized(&right, budget).unwrap());
            for joined in [
                state.join(&valid, budget).unwrap(),
                valid.join(&state, budget).unwrap(),
            ] {
                assert!(!joined.is_initialized(&left, budget).unwrap());
                assert!(joined.is_initialized(&right, budget).unwrap());
                joined.discard(budget).unwrap();
            }
            state
                .copy_subobject_from(&left, &valid, &left, budget)
                .unwrap();
            assert!(state.is_initialized(&root, budget).unwrap());
            assert!(layouts.root_subobject(&owner, TWO_ZST, budget).is_err());
            assert!(
                SourceStorageStateV29::new(
                    &layouts,
                    instances,
                    instances.root(),
                    local_for(TWO_ZST),
                    budget
                )
                .is_err()
            );
            drop((state, valid, root, left, right));
            layouts.release(budget).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn source_only_geometry_rejects_foreign_owner_unsized_and_changed_projection() {
    let owner = owner();
    let foreign = owner_with(types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
    assert!(
        layouts
            .source_root_subobject(&foreign, TWO_ZST, &mut budget)
            .is_err()
    );
    assert!(
        layouts
            .source_root_subobject(&owner, SLICE, &mut budget)
            .is_err()
    );
    assert!(
        layouts
            .source_projected_subobject(
                &owner,
                TWO_ZST,
                &[SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), WORD).unwrap()],
                &mut budget
            )
            .is_err()
    );
    assert!(
        layouts
            .source_projected_subobject(
                &owner,
                TWO_ZST,
                &[SemanticProjectionV1::new(SemanticProjectionKindV1::Field(2), UNIT).unwrap()],
                &mut budget
            )
            .is_err()
    );
    assert!(
        layouts
            .source_projected_subobject(
                &owner,
                ARRAY,
                &[SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(0)),
                    WORD
                )
                .unwrap()],
                &mut budget
            )
            .is_err()
    );
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}
