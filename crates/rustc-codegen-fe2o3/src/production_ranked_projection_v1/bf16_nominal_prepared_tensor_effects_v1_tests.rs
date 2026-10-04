//! Inert ownership/mapping/resource controls. No admitted owner is fabricated.
use super::*;
use crate::production_ranked_projection_v1::{
    AccessKindAttr, AllocationContractV1, ProjectedReadValueV1, ProjectedReadViewAccessV1,
    ProjectedReadViewV1, ProjectedTransposeWorkgroupEffectV1, SemanticGfx950LdsTransposeFormatV1,
    SemanticLocalIdV1, SemanticTypeIdV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const LIMIT: usize = 2 * 1024 * 1024;
const FLOOR: usize = 37;

fn layout() -> ProductionRankedOperationV1 {
    let roots =
        std::array::from_fn::<_, 6, _>(|i| DigestV1::from_untrusted_bytes([i as u8 + 1; 32]));
    ProductionRankedOperationV1::TensorLayout {
        contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
            .with_zero_filled_predicate_inputs(),
        convergence: TensorConvergenceAttr::UniformSubgroup,
        active_lanes: 64,
        binding: Some(
            ProductionCooperativeTensorBindingV1::new(
                roots[0], roots[1], roots[2], roots[3], roots[4], roots[5], 4,
            )
            .unwrap(),
        ),
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
        layout: Some(layout()),
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
fn compose_for_test(
    rows: &[ProjectedCapabilityTerminatorEffectsV1],
    block: usize,
    operation: &ProductionRankedOperationV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    with_composed_scope(rows.len(), budget, |budget| {
        let output = compose_rows(rows, block, operation, budget)?;
        drop(output);
        Ok(())
    })
}

#[test]
fn composition_inserts_only_selected_layout_and_preserves_full_original_rows() {
    let original = [
        effect(),
        ProjectedCapabilityTerminatorEffectsV1::default(),
        effect(),
    ];
    let before = original.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    with_composed_scope(3, &mut budget, |budget| {
        let output = compose_rows(&original, 1, &layout(), budget)?;
        assert_eq!(output.len(), 3);
        assert_eq!(output.capacity(), 3);
        assert_ne!(output.as_ptr(), original.as_ptr());
        assert_eq!(output[0], original[0]);
        assert_eq!(output[2], original[2]);
        assert_eq!(output[1].layout, Some(layout()));
        assert_eq!(output[1].global_read, None);
        assert_eq!(output[1].transpose_workgroup, None);
        assert_eq!(output[1].read_view, None);
        assert_eq!(original, before);
        drop(output);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn all_four_destination_collisions_refuse_without_overwriting_original() {
    for field in 0..4 {
        let mut row = ProjectedCapabilityTerminatorEffectsV1::default();
        let mut nonempty = effect();
        match field {
            0 => row.layout = nonempty.layout.take(),
            1 => row.global_read = nonempty.global_read,
            2 => row.transpose_workgroup = nonempty.transpose_workgroup,
            _ => row.read_view = nonempty.read_view,
        }
        let original = [row];
        let before = original.clone();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        assert!(matches!(
            compose_for_test(&original, 0, &layout(), &mut budget),
            Err(Error::Unavailable(
                "composed nominal source slot is not the empty actual Defined-call effect"
            ))
        ));
        assert_eq!(original, before);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn repeated_insertion_refuses_and_keeps_first_complete_output() {
    let original = [ProjectedCapabilityTerminatorEffectsV1::default()];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    with_composed_scope(1, &mut budget, |budget| {
        let first = compose_rows(&original, 0, &layout(), budget)?;
        let before = first.clone();
        assert!(compose_rows(&first, 0, &layout(), budget).is_err());
        assert_eq!(first, before);
        drop(first);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn missing_slot_empty_oversized_and_wrong_layouts_refuse() {
    assert!(compose_storage::<()>(0, 0).is_err());
    assert!(compose_storage::<()>(33, 0).is_err());
    assert!(compose_storage::<()>(32, 0).is_ok());
    assert!(matches!(
        compose_storage::<()>(1, usize::MAX),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    let original = [ProjectedCapabilityTerminatorEffectsV1::default()];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(compose_for_test(&original, 1, &layout(), &mut budget).is_err());
    for variant in 0..4 {
        let mut bad = layout();
        if let ProductionRankedOperationV1::TensorLayout {
            active_lanes,
            binding,
            contract,
            ..
        } = &mut bad
        {
            match variant {
                0 => *active_lanes = 32,
                1 => *binding = None,
                2 => *contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                _ => {}
            }
        }
        if variant == 3 {
            bad = ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: 1,
                global_extents: [1; 3],
                workgroup_extents: [1; 3],
                subgroup_size: 64,
                full_physical_workgroups: true,
            };
        }
        assert!(compose_for_test(&original, 0, &bad, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn unsupported_other_row_cannot_be_silently_omitted_after_partial_copy() {
    let mut bad = effect();
    bad.layout = Some(ProductionRankedOperationV1::ExecutionLayout {
        grid_identity: 1,
        global_extents: [1; 3],
        workgroup_extents: [1; 3],
        subgroup_size: 64,
        full_physical_workgroups: true,
    });
    let original = [
        effect(),
        ProjectedCapabilityTerminatorEffectsV1::default(),
        bad,
    ];
    let before = original.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(compose_for_test(&original, 1, &layout(), &mut budget).is_err());
    assert_eq!(original, before);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn exact_prepaid_copy_work_and_one_short_are_distinguished() {
    let amount = COMPOSE_SCOPE_WORK + 64 + 2 * COMPOSE_ROW_WORK;
    for short in [false, true] {
        let original = [effect(), ProjectedCapabilityTerminatorEffectsV1::default()];
        let mut work = Work::new(amount - usize::from(short));
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = compose_for_test(&original, 1, &layout(), &mut budget);
        assert_eq!(result.is_ok(), !short);
        assert_eq!(budget.failed_work().is_some(), short);
        assert_eq!(budget.storage(), 0);
        assert_eq!(
            budget.work(),
            if short { COMPOSE_SCOPE_WORK } else { amount }
        );
    }
}

#[test]
fn exact_frame_and_one_short_never_enter_the_body_on_shortfall() {
    for short in [false, true] {
        let entered = Cell::new(false);
        let body = |_: &mut Budget<'_>| {
            entered.set(true);
            Ok(())
        };
        let frame = compose_storage::<()>(2, size_of_val(&body)).unwrap();
        let mut work = Work::new(COMPOSE_SCOPE_WORK);
        let mut budget = Budget::new(&mut work, FLOOR + frame - usize::from(short));
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_composed_scope(2, &mut budget, body);
        assert_eq!(result.is_ok(), !short);
        assert_eq!(entered.get(), !short);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage().is_some(), short);
    }
}

#[test]
fn callbacks_error_panic_and_surplus_keep_original_account_custody() {
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = with_composed_scope(1, &mut budget, |budget| {
            let output = compose_rows(
                &[ProjectedCapabilityTerminatorEffectsV1::default()],
                0,
                &layout(),
                budget,
            )?;
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match mode {
                0 => {
                    drop(output);
                    Ok(())
                }
                1 => Err(Error::Unavailable("composed callback refusal")),
                _ => panic!("composed callback unwind"),
            }
        });
        assert_eq!(
            result,
            match mode {
                0 => Ok(()),
                1 => Err(Error::Unavailable("composed callback refusal")),
                _ => Err(Error::CallbackPanicked),
            }
        );
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + 23);
    }
}

#[test]
fn ignored_denial_and_preexisting_denial_never_become_success() {
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        assert_eq!(
            with_composed_scope(1, &mut budget, |budget| {
                if storage {
                    let _ = budget.reserve_storage(LIMIT + 1);
                } else {
                    let _ = budget.charge_work(LIMIT + 1);
                }
                Ok(())
            }),
            Err(Resource::Accounting.into())
        );
        assert_eq!(budget.storage(), 0);
        let entered = Cell::new(false);
        let before = (budget.work(), budget.storage(), budget.peak_storage());
        assert_eq!(
            with_composed_scope(1, &mut budget, |_| {
                entered.set(true);
                Ok(())
            }),
            Err(Resource::Accounting.into())
        );
        assert!(!entered.get());
        assert_eq!(
            before,
            (budget.work(), budget.storage(), budget.peak_storage())
        );
    }
}

#[test]
fn undercut_and_foreign_account_refuse_without_repair() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let remaining = Cell::new(0);
    assert_eq!(
        with_composed_scope(1, &mut budget, |budget| {
            budget.release_storage(1)?;
            remaining.set(budget.storage());
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert_eq!(budget.storage(), remaining.get());

    let mut original_work = Work::new(LIMIT);
    let mut replacement_work = Work::new(LIMIT);
    let mut original = Budget::new(&mut original_work, LIMIT);
    let mut replacement = Budget::new(&mut replacement_work, LIMIT);
    replacement.reserve_storage(19).unwrap();
    let identity = replacement.work_ledger_identity_v1();
    assert_eq!(
        with_composed_scope(1, &mut original, |budget| {
            std::mem::swap(budget, &mut replacement);
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert!(original.work_ledger_identity_v1() == identity);
    assert_eq!(original.storage(), 19);
}
