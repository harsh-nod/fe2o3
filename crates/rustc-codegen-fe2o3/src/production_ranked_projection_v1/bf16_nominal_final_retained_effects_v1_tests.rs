//! Synthetic copy/accounting controls only, not forged actual source evidence.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_call_routing_v1::NominalCallVisitorV1;
use crate::production_ranked_projection_v1::{
    AccessKindAttr, AllocationContractV1, ProjectedReadValueV1, ProjectedReadViewAccessV1,
    ProjectedReadViewV1, ProjectedTransposeWorkgroupEffectV1, SemanticGfx950LdsTransposeFormatV1,
    SemanticLocalIdV1, SemanticSourceProvenanceV1, SemanticTypeIdV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const LIMIT: usize = 2 * 1024 * 1024;
const FLOOR: usize = 37;
struct SyntheticConsumer<'a, 'w>(&'a mut Budget<'w>);
impl NominalCapabilityConsumerV1 for SyntheticConsumer<'_, '_> {
    fn charge_work_v1(&mut self, n: usize) -> Result<()> {
        self.0.charge_work(n).map_err(Error::Resource)
    }
    fn reserve_storage_v1(&mut self, n: usize) -> Result<()> {
        self.0.reserve_storage(n).map_err(Error::Resource)
    }
    fn release_storage_v1(&mut self, n: usize) -> Result<()> {
        self.0.release_storage(n).map_err(Error::Resource)
    }
    fn ledger_v1(&self) -> NominalCapabilityLedgerV1 {
        NominalCapabilityLedgerV1 {
            slot: self.0 as *const Budget<'_> as usize,
            identity: self.0.work_ledger_identity_v1(),
            work: self.0.work(),
            storage: self.0.storage(),
            peak: self.0.peak_storage(),
            denied_work: self.0.failed_work().is_some(),
            denied_storage: self.0.failed_storage().is_some(),
        }
    }
    fn with_nominal_call_v1(
        &mut self,
        _: usize,
        _: &SemanticDirectCallV1,
        _: SemanticSourceProvenanceV1,
        _: &mut NominalCallVisitorV1<'_>,
    ) -> Result<()> {
        Err(Error::Unavailable(
            "synthetic retained test has no nominal source",
        ))
    }
}
fn run() -> NominalCapabilityRunV1 {
    NominalCapabilityRunV1 {
        query_visits: [1, 1, 1],
        authenticated_visits: [1, 1, 1],
        reached_blocks: [2, 2, 2],
        array_destination_has_origin: false,
    }
}
fn effect() -> ProjectedCapabilityTerminatorEffectsV1 {
    let allocation = AllocationContractV1 {
        allocation_origin: 11,
        noalias_class: 13,
        writable: false,
        singleton_object: false,
    };
    ProjectedCapabilityTerminatorEffectsV1 {
        layout: Some(ProductionRankedOperationV1::TensorLayout {
            contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                .with_zero_filled_predicate_inputs(),
            convergence: TensorConvergenceAttr::UniformSubgroup,
            active_lanes: 64,
            binding: None,
        }),
        global_read: Some(allocation),
        transpose_workgroup: Some(ProjectedTransposeWorkgroupEffectV1 {
            access: AccessKindAttr::Write,
            format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
        }),
        read_view: Some(ProjectedReadViewAccessV1 {
            view: ProjectedReadViewV1 {
                root: 17,
                element: SemanticTypeIdV1::from_index(3),
                allocation,
                rows: ProjectedReadValueV1::Constant(19),
                columns: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(5)),
            },
            row: ProjectedReadValueV1::Constant(7),
            column: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(9)),
        }),
    }
}
fn unsupported() -> ProjectedCapabilityTerminatorEffectsV1 {
    ProjectedCapabilityTerminatorEffectsV1 {
        layout: Some(ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [1; 3],
            workgroup_extents: [1; 3],
            subgroup_size: 64,
            full_physical_workgroups: true,
        }),
        ..ProjectedCapabilityTerminatorEffectsV1::default()
    }
}
#[test]
fn fixed_copy_preserves_all_four_effect_fields_without_alias_or_enum_clone() {
    let mut original = effect();
    let copied = copy_fixed_effect(&original).unwrap();
    assert_eq!(copied, original);
    original.global_read = None;
    original.read_view = None;
    assert!(copied.global_read.is_some());
    assert!(copied.read_view.is_some());
    assert!(copied.transpose_workgroup.is_some());
    assert!(matches!(
        copied.layout,
        Some(ProductionRankedOperationV1::TensorLayout { .. })
    ));
    assert_eq!(
        copy_fixed_effect(&ProjectedCapabilityTerminatorEffectsV1::default()).unwrap(),
        ProjectedCapabilityTerminatorEffectsV1::default()
    );
    assert!(copy_fixed_effect(&unsupported()).is_err());
}
#[test]
fn closed_shape_arithmetic_and_exact_allocation_capacity_refuse() {
    assert!(retained_scope_storage::<()>(0, 0).is_err());
    assert!(retained_scope_storage::<()>(33, 0).is_err());
    assert!(retained_scope_storage::<()>(32, 0).is_ok());
    assert!(retained_scope_storage::<()>(2, usize::MAX).is_err());
    assert_eq!(
        require_exact_capacity(3, 2),
        Err(Resource::Allocation.into())
    );
    assert_eq!(
        require_exact_capacity(1, 2),
        Err(Resource::Allocation.into())
    );
    assert_eq!(require_exact_capacity(2, 2), Ok(()));
}
#[test]
fn actual_internal_capture_shape_preserves_default_rows_and_order() {
    let source = [effect(), ProjectedCapabilityTerminatorEffectsV1::default()];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    with_retained_scope(&mut budget, 2, |budget, protected| {
        let mut retained = RetainedEffects::new(2, budget)?;
        assert_eq!(retained.rows.capacity(), 2);
        assert!(retained.completed.is_none());
        retained.capture(&source, run(), &mut SyntheticConsumer(budget), protected)?;
        assert_eq!(retained.rows.as_slice(), &source);
        assert_eq!(retained.completed, Some(run()));
        protected.require_completed(budget)?;
        drop(retained);
        Ok(())
    })
    .unwrap();
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        budget.work(),
        SCOPE_WORK
            + RETAIN_INITIALIZE_WORK
            + 2 * size_of::<ProjectedCapabilityTerminatorEffectsV1>()
            + RETAIN_CAPTURE_WORK
            + 2 * RETAIN_ROW_WORK
    );
}
#[test]
fn missing_wrong_shape_repeated_or_incomplete_capture_is_not_completion() {
    for mode in 0..4 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = with_retained_scope(&mut budget, 2, |budget, protected| {
            let mut retained = RetainedEffects::new(2, budget)?;
            let rows = [effect(), ProjectedCapabilityTerminatorEffectsV1::default()];
            let mut summary = run();
            if mode == 0 {
                summary.query_visits[1] = 0;
            }
            if mode == 1 {
                summary.array_destination_has_origin = true;
            }
            let slice = if mode == 2 { &rows[..1] } else { &rows[..] };
            retained.capture(slice, summary, &mut SyntheticConsumer(budget), protected)?;
            assert_eq!(mode, 3);
            retained.capture(&rows, summary, &mut SyntheticConsumer(budget), protected)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn partial_copy_and_denied_work_drop_before_outer_refund() {
    for denied in [false, true] {
        let paid = SCOPE_WORK
            + RETAIN_INITIALIZE_WORK
            + 2 * size_of::<ProjectedCapabilityTerminatorEffectsV1>()
            + RETAIN_CAPTURE_WORK
            + RETAIN_ROW_WORK;
        let mut work = Work::new(if denied { paid } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = with_retained_scope(&mut budget, 2, |budget, protected| {
            let mut retained = RetainedEffects::new(2, budget)?;
            let rows = [effect(), if denied { effect() } else { unsupported() }];
            retained.capture(&rows, run(), &mut SyntheticConsumer(budget), protected)
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.failed_work().is_some(), denied);
        if denied {
            assert_eq!(budget.work(), paid);
        }
    }
}
#[test]
fn exact_scope_work_and_storage_boundaries_never_enter_on_shortfall() {
    for mode in 0..3 {
        let entered = Cell::new(false);
        let body = |budget: &mut Budget<'_>, protected: Checkpoint| {
            protected.require_completed(budget)?;
            entered.set(true);
            Ok(())
        };
        let frame = retained_scope_storage::<()>(2, size_of_val(&body)).unwrap();
        let mut work = Work::new(SCOPE_WORK - usize::from(mode == 1));
        let mut budget = Budget::new(&mut work, FLOOR + frame - usize::from(mode == 2));
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_retained_scope(&mut budget, 2, body);
        assert_eq!(entered.get(), mode == 0);
        assert_eq!(result.is_ok(), mode == 0);
        assert_eq!(budget.storage(), FLOOR);
        if mode == 0 {
            assert_eq!(budget.peak_storage(), FLOOR + frame);
        }
    }
}
#[test]
fn successful_error_and_panic_callbacks_preserve_surplus_after_table_drop() {
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_retained_scope(&mut budget, 2, |budget, protected| {
            let retained = RetainedEffects::new(2, budget)?;
            protected.require_completed(budget)?;
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match mode {
                0 => {
                    drop(retained);
                    Ok(())
                }
                1 => Err(Error::Unavailable("synthetic retained refusal")),
                _ => panic!("synthetic retained unwind"),
            }
        });
        assert_eq!(budget.storage(), FLOOR + 23);
        assert_eq!(
            result,
            match mode {
                0 => Ok(()),
                1 => Err(Error::Unavailable("synthetic retained refusal")),
                _ => Err(Error::CallbackPanicked),
            }
        );
    }
}
#[test]
fn ignored_sticky_denials_cannot_be_reported_as_success() {
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = with_retained_scope(&mut budget, 2, |budget, _| {
            let retained = RetainedEffects::new(2, budget)?;
            if storage {
                let _ = budget.reserve_storage(LIMIT + 1);
            } else {
                let _ = budget.charge_work(LIMIT + 1);
            }
            drop(retained);
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.failed_storage().is_some(), storage);
        assert_eq!(budget.failed_work().is_some(), !storage);
    }
}
#[test]
fn floor_erosion_and_replaced_ledger_are_not_repaired() {
    let mut work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(19).unwrap();
    let identity = foreign.work_ledger_identity_v1();
    assert_eq!(
        with_retained_scope(&mut budget, 2, |budget, _| {
            let retained = RetainedEffects::new(2, budget)?;
            std::mem::swap(budget, &mut foreign);
            drop(retained);
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), 19);
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let before = Cell::new(0);
    assert_eq!(
        with_retained_scope(&mut other, 2, |budget, _| {
            let retained = RetainedEffects::new(2, budget)?;
            before.set(budget.storage());
            budget.release_storage(1)?;
            drop(retained);
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert_eq!(other.storage(), before.get() - 1);
}
#[test]
fn consumer_guard_rejects_foreign_identity_and_sticky_denials_before_copy() {
    let mut work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    let protected = Checkpoint::take(&budget);
    assert_eq!(
        require_consumer_custody(protected, &SyntheticConsumer(&mut foreign)),
        Err(Resource::Accounting.into())
    );
    let _ = budget.charge_work(LIMIT + 1);
    assert_eq!(
        require_consumer_custody(protected, &SyntheticConsumer(&mut budget)),
        Err(Resource::Accounting.into())
    );
}
#[test]
fn preexisting_denial_refuses_without_allocating_or_entering() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, LIMIT);
    let _ = budget.charge_work(1);
    let entered = Cell::new(false);
    let before = (
        budget.storage(),
        budget.peak_storage(),
        budget.work(),
        budget.failed_work(),
    );
    assert_eq!(
        with_retained_scope(&mut budget, 2, |_, _| {
            entered.set(true);
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert!(!entered.get());
    assert_eq!(
        (
            budget.storage(),
            budget.peak_storage(),
            budget.work(),
            budget.failed_work()
        ),
        before
    );
}
#[test]
fn noncopy_panic_payload_is_destroyed_before_return() {
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let marker = dropped.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert_eq!(
        with_retained_scope::<(), _>(&mut budget, 2, move |budget, _| {
            let _retained = RetainedEffects::new(2, budget)?;
            std::panic::panic_any(Payload(marker));
        }),
        Err(Error::CallbackPanicked)
    );
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(budget.storage(), 0);
}
