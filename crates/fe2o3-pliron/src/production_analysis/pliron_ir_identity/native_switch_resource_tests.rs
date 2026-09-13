#[test]
fn native_switch_preflight_sums_callback_work_and_reuses_scratch_without_admitting_control() {
    use dialect_gpu::switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3, SwitchOpV3};
    use pliron::builtin::types::Signedness;
    let context = &mut Context::new();
    register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(context).unwrap();
    let selector_type = IntegerType::get(context, 128, Signedness::Unsigned).into();
    let index = IndexType::get(context).into();
    let signature = FunctionType::get(context, vec![selector_type, index, index], vec![]);
    let function = FuncOp::new(context, "switch_preflight".try_into().unwrap(), signature);
    let entry = function.get_entry_block(context);
    let selector = entry.deref(context).get_argument(0);
    let join = BasicBlock::new(context, None, vec![index; 2]);
    let exit = BasicBlock::new(context, None, vec![index; 2]);
    join.insert_at_back(function.get_region(context), context);
    exit.insert_at_back(function.get_region(context), context);
    for (source, target, count, first_argument) in [(entry, join, 16, 1), (join, exit, 17, 0)] {
        let a = source.deref(context).get_argument(first_argument);
        let b = source.deref(context).get_argument(first_argument + 1);
        let edges = (0..=count)
            .map(|edge| {
                SwitchEdgeV3::new(
                    target,
                    if edge % 2 == 0 {
                        vec![a, b]
                    } else {
                        vec![b, a]
                    },
                )
            })
            .collect();
        SwitchOpV3::try_new(
            context,
            selector,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..count as u64).rev().collect(),
            edges,
        )
        .unwrap()
        .get_operation()
        .insert_at_back(source, context);
    }
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(exit, context);
    let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let preflight = preflight_identity_structure_v1(context, &function, hard).unwrap();
    assert_eq!(preflight.native_switch_verification_work, 1845 + 6300);
    assert_eq!(preflight.native_switch_verification_scratch, 376);
    // Callback census is not a full generic-verifier receipt or a new allowlist.
    assert!(matches!(
        prescan(context, &function),
        Err(PlironIrIdentityErrorV1::UnsupportedOperation { .. })
    ));
    assert!(matches!(
        capture_bound_v1(context, &function, hard),
        Err(IdentityCaptureFailureV1::Unavailable { .. })
    ));
    let census = ProductionAnalysisInputCensusV1 {
        blocks: 3,
        operations: 3,
        block_arguments: 7,
        operands: 72,
        successors: 35,
        native_switch_verification_work: preflight.native_switch_verification_work,
        native_switch_verification_scratch: preflight.native_switch_verification_scratch,
        ..Default::default()
    };
    let without_callbacks = ProductionAnalysisInputCensusV1 {
        native_switch_verification_work: 0,
        native_switch_verification_scratch: 0,
        ..census
    };
    let capture = identity_capture_resource_upper_bound_v1(census, 0, 0, 0).unwrap();
    let before = identity_capture_resource_upper_bound_v1(without_callbacks, 0, 0, 0).unwrap();
    assert_eq!(capture.work_upper_bound() - before.work_upper_bound(), 8145);
    assert_eq!(
        capture.peak_storage_upper_bound() - before.peak_storage_upper_bound(),
        376
    );
    use crate::production_analysis::pliron_ranked_bounds::preflight_ranked_bounds_resource_upper_bound_v1;
    let bounds = preflight_ranked_bounds_resource_upper_bound_v1(census, hard).unwrap();
    let before = preflight_ranked_bounds_resource_upper_bound_v1(without_callbacks, hard).unwrap();
    assert_eq!(bounds.work_upper_bound() - before.work_upper_bound(), 8145);
    assert_eq!(
        bounds.peak_storage_upper_bound() - before.peak_storage_upper_bound(),
        376
    );
}
