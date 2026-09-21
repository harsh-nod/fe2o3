use super::super::*;
use crate::production_rustc_intrinsic_v1::ProductionRustcIntrinsicOperationV1;
use crate::production_safe_core_shift_v1::{NormalizedCallV1, SafeCoreShiftV1};
use crate::production_semantic_body_v1::ProductionSemanticNormalizedRustcIntrinsicRecipeV1 as BodyRecipe;
use crate::rustc_semantic_adapter_v1::CanonicalFunctionIdentitiesV1;
use crate::rustc_semantic_plan_v1::NormalizedRustcIntrinsicRecipeV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use std::mem::{align_of, size_of, size_of_val};

// Exact old field roster/order and variant roster, without repr changes.
#[derive(Clone, Copy, Debug)]
enum OldCall<'tcx> {
    Rustc(ProductionRustcIntrinsicOperationV1),
    SafeCoreShift(SafeCoreShiftV1<'tcx>),
}
#[derive(Clone, Copy, Debug)]
struct OldPlan<'tcx> {
    caller: SemanticFunctionIdV1,
    block: u32,
    operation: OldCall<'tcx>,
    element_type: Ty<'tcx>,
    instance: Instance<'tcx>,
    identities: CanonicalFunctionIdentitiesV1,
}
#[derive(Clone, Copy, Debug)]
struct OldBody<'tcx> {
    caller: SemanticFunctionIdV1,
    rustc_block: u32,
    expected_callee: Instance<'tcx>,
    expected_element_type: Ty<'tcx>,
    operation: OldCall<'tcx>,
}

#[test]
fn primitive_from_retained_enum_and_both_recipe_layouts_equal_frozen_old_rosters() {
    assert!(size_of::<CheckedPrimitiveFromV1<'_>>() <= size_of::<SafeCoreShiftV1<'_>>());
    assert!(align_of::<CheckedPrimitiveFromV1<'_>>() <= align_of::<SafeCoreShiftV1<'_>>());
    assert_eq!(size_of::<NormalizedCallV1<'_>>(), size_of::<OldCall<'_>>());
    assert_eq!(
        align_of::<NormalizedCallV1<'_>>(),
        align_of::<OldCall<'_>>()
    );
    assert_eq!(
        size_of::<NormalizedRustcIntrinsicRecipeV1<'_>>(),
        size_of::<OldPlan<'_>>()
    );
    assert_eq!(
        align_of::<NormalizedRustcIntrinsicRecipeV1<'_>>(),
        align_of::<OldPlan<'_>>()
    );
    assert_eq!(size_of::<BodyRecipe<'_>>(), size_of::<OldBody<'_>>());
    assert_eq!(align_of::<BodyRecipe<'_>>(), align_of::<OldBody<'_>>());
}

pub(super) fn check_actual<'tcx>(
    tcx: TyCtxt<'tcx>,
    checked: CheckedPrimitiveFromV1<'tcx>,
    shift: SafeCoreShiftV1<'tcx>,
) {
    use crate::rustc_semantic_adapter_v1::canonical_function_identities_v1;
    let intrinsic = ProductionRustcIntrinsicOperationV1::FabsF32;
    let old_calls = [OldCall::Rustc(intrinsic), OldCall::SafeCoreShift(shift)];
    for value in old_calls {
        match value {
            OldCall::Rustc(op) => assert_eq!(op, intrinsic),
            OldCall::SafeCoreShift(s) => assert!(s.same_producers(shift)),
        }
    }
    let old_plan = OldPlan {
        caller: SemanticFunctionIdV1::from_index(0),
        block: 0,
        operation: old_calls[0],
        element_type: checked.output_type(),
        instance: checked.instance,
        identities: canonical_function_identities_v1(tcx, checked.instance),
    };
    let old_body = OldBody {
        caller: old_plan.caller,
        rustc_block: old_plan.block,
        expected_callee: old_plan.instance,
        expected_element_type: old_plan.element_type,
        operation: old_plan.operation,
    };
    assert_eq!(old_body.caller, old_plan.caller);
    assert_eq!(old_body.rustc_block, old_plan.block);
    assert_eq!(old_body.expected_callee, checked.instance);
    assert_eq!(old_body.expected_element_type, checked.output_type());
    assert_eq!(
        old_plan.identities.function(),
        canonical_function_identities_v1(tcx, checked.instance).function()
    );
    assert!(matches!(old_body.operation, OldCall::Rustc(_)));
    let from = NormalizedCallV1::CheckedPrimitiveFrom(checked);
    assert_eq!(from.statement_count(), 1);
    assert_eq!(from.operation_tag(), 7);
    let left = NormalizedCallV1::SafeCoreShift(shift);
    assert_eq!(left.statement_count(), 3);
    assert_eq!(left.operation_tag(), 4);
    for calls in [
        vec![left, NormalizedCallV1::Rustc(intrinsic)],
        vec![from],
        vec![left, from, NormalizedCallV1::Rustc(intrinsic)],
    ] {
        let mut plan = Vec::new();
        for (index, operation) in calls.iter().copied().enumerate() {
            plan.push(NormalizedRustcIntrinsicRecipeV1 {
                caller: old_plan.caller,
                block: index as u32,
                operation,
                element_type: checked.output_type(),
                instance: checked.instance,
                identities: old_plan.identities,
            });
        }
        let plan_capacity = plan.capacity();
        let plan_bytes = size_of_val(&plan)
            .checked_add(
                plan_capacity
                    .checked_mul(size_of::<NormalizedRustcIntrinsicRecipeV1<'_>>())
                    .unwrap(),
            )
            .unwrap();
        assert!(plan_capacity >= calls.len());
        let boxed = plan.into_boxed_slice();
        let boxed_bytes = size_of_val(&boxed)
            .checked_add(std::mem::size_of_val(boxed.as_ref()))
            .unwrap();
        assert_eq!(boxed.len(), calls.len());
        let imported = boxed
            .iter()
            .map(|r| BodyRecipe::new(r.caller, r.block, r.instance, r.element_type, r.operation))
            .collect::<Vec<_>>();
        let import_bytes = size_of_val(&imported)
            .checked_add(
                imported
                    .capacity()
                    .checked_mul(size_of::<BodyRecipe<'_>>())
                    .unwrap(),
            )
            .unwrap();
        let bindings = imported.as_slice();
        assert_eq!(bindings.len(), boxed.len());
        let map = imported.iter().map(Some).collect::<Vec<_>>();
        let map_bytes = size_of_val(&map)
            .checked_add(
                map.capacity()
                    .checked_mul(size_of::<Option<&BodyRecipe<'_>>>())
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(map.len(), calls.len());
        assert!(
            map.iter()
                .zip(bindings)
                .all(|(a, b)| std::ptr::eq(a.unwrap(), b))
        );
        let coexistence = boxed_bytes
            .checked_add(import_bytes)
            .and_then(|n| n.checked_add(map_bytes))
            .unwrap();
        assert!(coexistence >= boxed_bytes);
        println!(
            "primitive-from retained stage=plan-vec rows={} capacity={} element={} bytes={plan_bytes}; stage=plan-box bytes={boxed_bytes}; stage=import-vec capacity={} element={} bytes={import_bytes}; stage=body-borrowed-map capacity={} bytes={map_bytes}; coexistence={coexistence}",
            calls.len(),
            plan_capacity,
            size_of::<NormalizedRustcIntrinsicRecipeV1<'_>>(),
            imported.capacity(),
            size_of::<BodyRecipe<'_>>(),
            map.capacity()
        );
    }
}
