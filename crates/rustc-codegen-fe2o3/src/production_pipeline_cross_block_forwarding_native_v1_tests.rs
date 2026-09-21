//! Constructed genuine source owners, distinct from ordinary-rustc qualification.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
use crate::production_ranked_projection_v1::{
    with_backend_forwarding_erased_prefix_v1, with_backend_licm_direct_prefix_v1,
    with_backend_licm_erased_prefix_v1,
};
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirOperationCoordinateV1 as Coordinate, Operation, OperationKind,
};
use std::mem::size_of_val;
fn with_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        with_backend_forwarding_erased_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Erased(owner), budget)
        });
    } else {
        with_backend_licm_direct_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Direct(owner), budget)
        });
    }
}
fn operation(owner: &Graph, site: Coordinate) -> &Operation {
    &owner.module().functions[site.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[site.block.block as usize]
        .operations[site.operation as usize]
}
fn shape(value: &PreparedCrossBlockForwardingNativeOutputV1, erased: bool, selected: usize) {
    let before = value.licm_input();
    let after = value.output();
    let mut count = 0;
    let operations = before
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .map(|b| b.operations.len())
        .sum::<usize>();
    assert_eq!(value.origins().len(), operations);
    for source in value.origins() {
        let row = source.canonical_origin();
        assert_eq!(row.input, row.output);
        let old = operation(before, row.input);
        let new = operation(after, row.output);
        if let Some(store) = row.store {
            count += 1;
            assert_ne!(store.block, row.input.block);
            let OperationKind::Load { pointer, access } = old.kind else {
                panic!("actual retained input Load")
            };
            let OperationKind::Store {
                pointer: stored_pointer,
                value,
                access: stored_access,
            } = operation(before, store).kind
            else {
                panic!("actual initializing Store")
            };
            assert_eq!((pointer, access), (stored_pointer, stored_access));
            assert_eq!(access.address_space, AddressSpace::Private);
            assert_eq!(
                new.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: value,
                    rhs: value
                }
            );
            assert_eq!(new.results, old.results);
            let (function, block, statement) = source
                .original_source_statement()
                .expect("original source Load occurrence");
            assert!(function.index() < 2);
            assert_eq!((block.index(), statement), (if erased { 7 } else { 5 }, 0));
        } else {
            assert_eq!(old, new);
        }
    }
    assert_eq!(
        count, selected,
        "actual positive must survive every existing stage"
    );
    assert_eq!(before.module().kernels, after.module().kernels);
    assert_eq!(after.module().kernels.len(), 2);
    assert_eq!(
        before.canonical().canonical_bytes() == after.canonical().canonical_bytes(),
        selected == 0
    );
    assert!(!std::ptr::eq(before, after));
    assert_eq!(value.limits(), Limits::default());
    assert!(!value.grants_artifact_or_launch_authority());
    assert_eq!(value.prefix_execution.policy_version(), 7);
    assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
    if let Forwarded::Erased(owner) = &value.owner {
        let source = owner.prefix().prefix().prefix().prefix().prefix().prefix();
        assert_eq!(
            source
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .functions()
                .len(),
            3
        );
        assert_eq!(
            (
                source.erased_source().deleted_call_count(),
                source.erased_source().deleted_function_count()
            ),
            (2, 1)
        );
        assert_eq!(owner.kernels().len(), 2);
    }
}
fn release(
    value: PreparedCrossBlockForwardingNativeOutputV1,
    receipt: CrossBlockForwardingNativeStorageV1,
    budget: &mut Budget<'_>,
) {
    drop(value);
    budget.release_storage(receipt.retained_storage()).unwrap();
}
#[test]
fn cross_block_forwarding_native_actual_direct_and_unit_local_two_loads_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, true, |prefix, budget| {
                let original = match &prefix {
                    Prefix6::Direct(v) => v.source_semantic_kir().pre_ranked_executable().unwrap(),
                    Prefix6::Erased(v) => v.original_source().executable(),
                }
                .canonical()
                .canonical_bytes()
                .as_ptr();
                let sibling = vec![0x73u8; 43];
                let sibling_bytes = size_of_val(&sibling) + sibling.capacity();
                budget.reserve_storage(sibling_bytes).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let (value, receipt) = prepare(prefix, profile, Limits::default(), budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                assert_eq!(
                    value
                        .original()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    original
                );
                shape(&value, erased, 2);
                value.verify_equivalence(budget).unwrap();
                let (text, bytes) = lower_native(value.output(), profile, budget).unwrap();
                budget.reserve_storage(bytes).unwrap();
                assert_eq!(text, value.llvm_ir());
                drop(text);
                budget.release_storage(bytes).unwrap();
                let (old, bytes) = lower_native(value.licm_input(), profile, budget).unwrap();
                budget.reserve_storage(bytes).unwrap();
                assert_ne!(old, value.llvm_ir());
                assert!(matches!(
                    check_native_text(value.output(), profile, &old, budget),
                    Err(ProductionPipelineError::LicmNativeStage(
                        LicmNativeStageErrorV1::CrossBlockForwarding(
                            CrossBlockForwardingNativeStageErrorV1::Mismatch(
                                "exact cross-block private-forwarding native LLVM"
                            )
                        )
                    ))
                ));
                drop(old);
                budget.release_storage(bytes).unwrap();
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x73; 43]);
                release(value, receipt, budget);
                assert_eq!(budget.storage(), floor);
                drop(sibling);
                budget.release_storage(sibling_bytes).unwrap();
            });
        }
    }
}
#[test]
fn cross_block_forwarding_native_original_unit_local_global_clobber_is_not_searched_through() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_backend_licm_erased_prefix_v1(profile, true, |prefix, _, budget| {
            let (value, receipt) =
                prepare(Prefix6::Erased(prefix), profile, Limits::default(), budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                value
                    .licm_input()
                    .module()
                    .functions
                    .iter()
                    .filter_map(|f| f.body.as_ref())
                    .flat_map(|b| &b.blocks)
                    .flat_map(|b| &b.operations)
                    .filter(|op| matches!(op.kind, OperationKind::Load { .. }))
                    .count(),
                2
            );
            shape(&value, true, 0);
            value.verify_equivalence(budget).unwrap();
            release(value, receipt, budget);
        });
    }
}
#[test]
fn cross_block_forwarding_native_genuine_noops_are_exact_both_routes_and_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, false, |prefix, budget| {
                let floor = budget.storage();
                let (value, receipt) = prepare(prefix, profile, Limits::default(), budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                shape(&value, erased, 0);
                value.verify_equivalence(budget).unwrap();
                release(value, receipt, budget);
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn cross_block_forwarding_native_replay_refuses_changed_text_profile_floor_and_historical_witness()
{
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let (mut first, receipt) =
            prepare(prefix, Profile::Gfx942, Limits::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let original = first.llvm.remove(0);
        first
            .llvm
            .insert(0, if original == ';' { ' ' } else { ';' });
        assert!(matches!(
            first.verify_equivalence(budget),
            Err(ProductionPipelineError::LicmNativeStage(
                LicmNativeStageErrorV1::CrossBlockForwarding(
                    CrossBlockForwardingNativeStageErrorV1::Mismatch(
                        "exact cross-block private-forwarding native LLVM"
                    )
                )
            ))
        ));
        first.llvm.remove(0);
        first.llvm.insert(0, original);
        first.profile = Profile::Gfx950;
        assert!(matches!(
            first.verify_equivalence(budget),
            Err(ProductionPipelineError::TargetLowering(_))
        ));
        first.profile = Profile::Gfx942;
        first.retained_floor += 1;
        let before = budget.work();
        assert!(matches!(
            first.verify_equivalence(budget),
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                P7Error::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!((budget.work(), budget.storage()), (before, floor));
        first.retained_floor -= 1;
        with_prefix(false, Profile::Gfx950, true, |prefix, other| {
            let (mut second, receipt) =
                prepare(prefix, Profile::Gfx950, Limits::default(), other).unwrap();
            other.reserve_storage(receipt.retained_storage()).unwrap();
            assert_ne!(
                first.prefix_execution.canonical_bytes(),
                second.prefix_execution.canonical_bytes()
            );
            assert_eq!(
                first.prefix_execution.retained_storage(),
                second.prefix_execution.retained_storage()
            );
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            assert!(matches!(
                first.verify_equivalence(budget),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Execution("Policy7 complete execution transcript")
                ))
            ));
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            second.verify_equivalence(other).unwrap();
            release(second, receipt, other);
        });
        first.verify_equivalence(budget).unwrap();
        assert_eq!(budget.storage(), floor);
        release(first, receipt, budget);
    });
}
fn measured(
    erased: bool,
    profile: Profile,
    mutation: bool,
    w: usize,
    p: usize,
) -> (
    Result<()>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
    usize,
    usize,
) {
    let mut observation = None;
    with_prefix(erased, profile, mutation, |prefix, outer| {
        let sibling = vec![0x61u8; 47];
        let floor = outer.storage() + size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(w);
        work.charge_work(17).unwrap();
        let (result, used, peak, failed_storage, length, capacity) = {
            let mut budget = Budget::new(&mut work, p);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let mut length = 0;
            let mut capacity = 0;
            let result =
                prepare(prefix, profile, Limits::default(), &mut budget).map(|(value, receipt)| {
                    length = value.llvm_ir().len();
                    capacity = value.llvm.capacity();
                    shape(&value, erased, if mutation { 2 } else { 0 });
                    assert!(receipt.retained_storage() > 0);
                    drop(value);
                });
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x61; 47]);
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
                length,
                capacity,
            )
        };
        observation = Some((
            result,
            used,
            peak,
            work.failed_work(),
            failed_storage,
            length,
            capacity,
        ));
    });
    observation.unwrap()
}
#[test]
fn cross_block_forwarding_native_exact_resources_and_typed_final_text_work_denial() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let full = measured(erased, profile, mutation, 1_000_000_000, 1 << 30);
                full.0.unwrap();
                let exact = measured(erased, profile, mutation, full.1, full.2);
                exact.0.unwrap();
                assert_eq!(
                    (exact.1, exact.2, exact.3, exact.4, exact.5, exact.6),
                    (full.1, full.2, None, None, full.5, full.6)
                );
                let short = measured(erased, profile, mutation, full.1 - 1, full.2);
                match short.0 {
                    Err(ProductionPipelineError::LicmNativeStage(
                        LicmNativeStageErrorV1::CrossBlockForwarding(
                            CrossBlockForwardingNativeStageErrorV1::Resource(Resource::Work(error)),
                        ),
                    )) => assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1)),
                    other => panic!("exact final native text charge: {other:?}"),
                }
                assert_eq!(
                    (short.1, short.2, short.3, short.4),
                    (full.1 - (2 * full.5 + 1), full.2, Some(full.1), None)
                );
                let short = measured(erased, profile, mutation, 1_000_000_000, full.2 - 1);
                match short.0 {
                    Err(ProductionPipelineError::PrivateCellNativeStage(
                        PrivateCellNativeStageErrorV1::Resource(Resource::Storage(error)),
                    )) => assert_eq!((error.actual(), error.limit()), (full.2, full.2 - 1)),
                    other => panic!("exact final native scratch Storage refusal: {other:?}"),
                }
                let owner_header = if erased {
                    size_of::<ErasedForwarded>()
                } else {
                    size_of::<DirectForwarded>()
                };
                let candidate_header = size_of::<PreparedCrossBlockForwardingNativeOutputV1>()
                    .checked_sub(owner_header)
                    .and_then(|bytes| bytes.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
                    .unwrap();
                // Final replay repeats the first native scratch request with the
                // actual LLVM backing and completed candidate headers retained.
                assert_eq!(
                    short.2,
                    full.2
                        .checked_sub(candidate_header.checked_add(full.6).unwrap())
                        .unwrap()
                );
                assert_eq!(
                    short.1,
                    full.1
                        .checked_sub(full.5.checked_mul(2).unwrap().checked_add(1).unwrap())
                        .unwrap()
                );
                assert_eq!((short.3, short.4), (None, Some(full.2)));
            }
        }
    }
}
#[test]
fn cross_block_forwarding_native_partial_failure_drops_actual_owner_before_floor_cleanup() {
    use std::{cell::Cell, rc::Rc};
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for erased in [false, true] {
        for panic in [false, true] {
            with_prefix(erased, Profile::Gfx942, true, |prefix, budget| {
                let sibling = vec![0x43u8; 43];
                let bytes = size_of_val(&sibling) + sibling.capacity();
                budget.reserve_storage(bytes).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let dropped = Rc::new(Cell::new(false));
                let result: Result<()> = scoped(floor, budget, |budget| {
                    let (owner, receipt) =
                        prepare(prefix, Profile::Gfx942, Limits::default(), budget)?;
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    shape(&owner, erased, 2);
                    let _live = (owner, Dropped(dropped.clone()));
                    if panic {
                        std::panic::panic_any(73u32);
                    }
                    Err(mismatch("injected with actual forwarded native owner"))
                });
                if panic {
                    assert!(matches!(
                        result,
                        Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                            P7Error::Panicked
                        ))
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionPipelineError::LicmNativeStage(
                            LicmNativeStageErrorV1::CrossBlockForwarding(
                                CrossBlockForwardingNativeStageErrorV1::Mismatch(
                                    "injected with actual forwarded native owner"
                                )
                            )
                        ))
                    ));
                }
                assert!(dropped.get());
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert_eq!(sibling, [0x43; 43]);
                drop(sibling);
                budget.release_storage(bytes).unwrap();
            });
        }
    }
}

use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationConflictAssessmentV1, SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationExecutionV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};
#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 262_144 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete source forwarding observation bound".into(),
            });
        }
        self.0.push(event.clone());
        Ok(())
    }
}
fn simulate(
    owner: &Graph,
    request: &SimulationRequestV1,
) -> (SimulationExecutionV1, Vec<SimulationEventV1>) {
    let wire = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(wire.identity(), owner.canonical().identity());
    let module =
        AdmittedSimulationModuleV1::admit_v12(wire, SimulationLimitsV1::default()).unwrap();
    let mut events = Events::default();
    let value = module
        .simulate_observed_with_sink(
            request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    (value, events.0)
}
fn source_launch(
    value: &PreparedCrossBlockForwardingNativeOutputV1,
    ordinal: usize,
) -> ([u64; 3], [u32; 3]) {
    let (roster, semantic) = match &value.owner {
        Forwarded::Direct(owner) => {
            let source = owner
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir();
            (
                source.source_launch_roster().unwrap(),
                source.semantic().semantic(),
            )
        }
        Forwarded::Erased(owner) => {
            let source = owner
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source();
            (
                source.source_launch(),
                source.semantic_ssa().source_semantic(),
            )
        }
    };
    assert_eq!(
        roster.semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    assert!(!roster.grants_artifact_or_launch_authority());
    assert_eq!(roster.roots().len(), value.output().module().kernels.len());
    let row = roster.roots()[ordinal];
    let source = &semantic.functions()[row.selected_root().index() as usize];
    assert_eq!(source.identity(), row.semantic_root_identity());
    let entry = source.kernel_entry().unwrap();
    assert_eq!(
        *entry.kernel_binding_identity().as_bytes(),
        row.kernel_binding()
    );
    let kernel = &value.output().module().kernels[ordinal];
    assert_eq!(
        kernel.id.as_str().as_bytes(),
        entry.export_symbol().as_bytes()
    );
    assert_eq!(kernel.domain.rank(), row.source_rank());
    assert_eq!(&value.licm_input().module().kernels[ordinal], kernel);
    let mut original = value.original().unwrap().module().kernels[ordinal].clone();
    original.required_capabilities.insert(match value.profile {
        Profile::Gfx942 => fe2o3_kernel_ir::gfx942_xnack_minus_target_capability(),
        Profile::Gfx950 => fe2o3_kernel_ir::gfx950_xnack_minus_target_capability(),
    });
    original
        .required_capabilities
        .insert(fe2o3_kernel_ir::TargetCapability::WaveWidth(
            fe2o3_kernel_ir::WaveWidth::Wave64,
        ));
    assert_eq!(&original, kernel);
    let layout = row.layout();
    assert!(layout.full_physical_workgroups());
    let grid = layout.global_extents();
    let group = kernel.workgroup_size.unwrap();
    let group = [group.x, group.y, group.z];
    assert_eq!(group.map(u64::from), layout.workgroup_extents());
    for axis in 0..3 {
        assert_ne!(grid[axis], 0);
        assert_ne!(group[axis], 0);
        assert_eq!(grid[axis] % u64::from(group[axis]), 0);
    }
    for (axis, extent) in kernel.domain.extents().enumerate() {
        if let fe2o3_kernel_ir::LaunchExtent::Static(n) = extent {
            assert_eq!(grid[axis], u64::from(n));
        }
    }
    (grid, group)
}
fn remaining_events(
    value: &PreparedCrossBlockForwardingNativeOutputV1,
    events: &[SimulationEventV1],
) -> (Vec<SimulationEventV1>, usize) {
    let mut retained = Vec::new();
    let mut removed = 0;
    for (index, event) in events.iter().enumerate() {
        if let EventKind::MemoryRead {
            allocation,
            offset,
            bytes,
        } = event.kind
        {
            let body = value.licm_input().module().functions[event.site.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let block = body
                .blocks
                .iter()
                .position(|b| b.id == event.site.block)
                .unwrap();
            let selected = value
                .origins()
                .iter()
                .map(|row| row.canonical_origin())
                .find(|row| {
                    row.store.is_some()
                        && row.input.block.function.0 as usize == event.site.function_ordinal
                        && row.input.block.block as usize == block
                        && Some(row.input.operation) == event.site.operation
                });
            if let Some(row) = selected {
                let store = row.store.unwrap();
                assert_eq!((offset, bytes), (0, 4));
                assert_eq!(row.input, row.output);
                assert!(
                    events[..index]
                        .iter()
                        .any(|prior| prior.invocation == event.invocation
                            && prior.site.block == body.blocks[store.block.block as usize].id
                            && prior.site.operation == Some(store.operation)
                            && prior.kind
                                == (EventKind::MemoryWrite {
                                    allocation,
                                    offset,
                                    bytes
                                }))
                );
                assert!(events[..index].iter().any(|prior|prior.invocation==event.invocation&&matches!(prior.kind,
                    EventKind::AllocationCreated{allocation:a,address_space:AddressSpace::Private,bytes:4} if a==allocation)));
                removed += 1;
                continue;
            }
        }
        retained.push(event.clone());
    }
    (retained, removed)
}
#[test]
fn cross_block_forwarding_native_sim_preserves_cpu_outputs_and_every_unselected_event() {
    let mut comparisons = 0;
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_prefix(erased, profile, mutation, |prefix, budget| {
                    let (value, receipt) =
                        prepare(prefix, profile, Limits::default(), budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    shape(&value, erased, if mutation { 2 } else { 0 });
                    for (ordinal, kernel) in value.output().module().kernels.iter().enumerate() {
                        let (grid, group) = source_launch(&value, ordinal);
                        let invocations = grid.into_iter().product::<u64>();
                        let controls: &[u32] = if erased && !mutation {
                            &[0]
                        } else {
                            &[0, 1, 2, u32::MAX]
                        };
                        let bounds: &[u64] = if mutation { &[0, 1, 3] } else { &[0] };
                        for &control in controls {
                            for &bound in bounds {
                                let mut args = Vec::new();
                                if erased {
                                    args.push(SimulationArgumentV1::Buffer(
                                        BufferArgumentV1::from_scalars(
                                            fe2o3_kernel_ir::AccessMode::ReadWrite,
                                            4,
                                            &[ScalarBitsV1::u32(123)],
                                            SimulationTargetV1::amdgpu_64(),
                                        )
                                        .unwrap(),
                                    ));
                                }
                                if !erased || mutation {
                                    args.push(SimulationArgumentV1::Scalar(ScalarBitsV1::u32(
                                        control,
                                    )));
                                }
                                if mutation {
                                    args.push(SimulationArgumentV1::Scalar(
                                        ScalarBitsV1::new(
                                            fe2o3_kernel_ir::ScalarType::U64,
                                            bound.into(),
                                            SimulationTargetV1::amdgpu_64(),
                                        )
                                        .unwrap(),
                                    ));
                                }
                                let request =
                                    SimulationRequestV1::new(kernel.id.as_str(), grid, group, args);
                                let unchanged = request.clone();
                                for _ in 0..2 {
                                    let original = simulate(value.original().unwrap(), &request);
                                    let before = simulate(value.licm_input(), &request);
                                    let after = simulate(value.output(), &request);
                                    assert_eq!(original.0.arguments(), before.0.arguments());
                                    assert_eq!(before.0.arguments(), after.0.arguments());
                                    assert_eq!(
                                        original.0.shared_buffers(),
                                        after.0.shared_buffers()
                                    );
                                    let (expected, removed) = remaining_events(&value, &before.1);
                                    assert_eq!(
                                        removed,
                                        if mutation && control & 1 == 0 {
                                            bound as usize * invocations as usize
                                        } else {
                                            0
                                        }
                                    );
                                    assert_eq!(expected, after.1);
                                    for (execution, events) in [&original, &before, &after] {
                                        assert_eq!(execution.invocations_executed(), invocations);
                                        assert!(!execution.grants_execution_authority());
                                        assert!(matches!(
                                            execution.conflict_assessment(),
                                            SimulationConflictAssessmentV1::NoConflictsObserved
                                        ));
                                        assert!(matches!(
                                            execution.race_assessment(),
                                            SimulationRaceAssessmentV1::NoRacesObserved { .. }
                                        ));
                                        if erased {
                                            let expected = if !mutation || bound == 0 {
                                                7
                                            } else {
                                                control & 1
                                            };
                                            let buffer = execution.buffer(0).unwrap();
                                            assert_eq!(buffer.bytes(), expected.to_le_bytes());
                                            assert!(buffer.initialized().iter().all(|b| *b));
                                            let globals: Vec<_> = events
                                                .iter()
                                                .filter_map(|event| match event.kind {
                                                    EventKind::AllocationPreexisting {
                                                        allocation,
                                                        address_space: AddressSpace::Global,
                                                        ..
                                                    } => Some(allocation),
                                                    _ => None,
                                                })
                                                .collect();
                                            assert_eq!(globals.len(), 1);
                                            assert_eq!(events.iter().filter(|event|matches!(event.kind,EventKind::MemoryWrite{allocation,offset:0,bytes:4} if allocation==globals[0])).count(),1+if mutation{bound as usize}else{0});
                                        }
                                    }
                                    comparisons += 1;
                                }
                                assert_eq!(request, unchanged);
                            }
                        }
                    }
                    release(value, receipt, budget);
                });
            }
        }
    }
    assert_eq!(
        comparisons, 232,
        "696 real simulations, including original source and actual final graph"
    );
}
