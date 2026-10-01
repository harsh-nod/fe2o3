use super::super::{
    ProductionPlironSessionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1 as Op, ProductionRankedTerminatorV1 as Term,
    ProductionRankedValueIdV1 as Id, ProductionRankedValueV1 as Value, ProductionSessionErrorV1,
    compile_ranked_kernel_for_lowering_v1,
};
use super::*;
use dialect_kernel::AccessKindAttr;

// Same original typed read fixture as production/session_v1_tests.rs.
// This is a real ranked constructor/verifier control, not source or GPU evidence.
fn construction(index_value: u64) -> ProductionConstructionV1 {
    let view = Id::new(0);
    let index = Id::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "checked",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                Op::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                Op::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                Op::IndexConstant {
                    result: index,
                    value: index_value,
                },
                Op::Access {
                    kind: AccessKindAttr::Read,
                    view: Value::Local(view),
                    indices: vec![Value::Local(index)],
                },
            ],
            Term::Return,
        )],
    )
    .expect("original typed ranked recipe");
    ProductionConstructionV1::ranked_kernel("allowance_root", kernel)
        .expect("original closed construction")
}

fn ceiling() -> ProductionRankedAnalysisAllowanceV1 {
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    ProductionRankedAnalysisAllowanceV1::new(limits.max_work(), limits.max_peak_storage())
        .expect("existing analysis ceiling")
}

fn compile(
    allowance: ProductionRankedAnalysisAllowanceV1,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    compile_ranked_kernel_for_lowering_with_analysis_allowance_v1(
        construction(0),
        ProductionSessionLimitsV1::default(),
        allowance,
    )
}

fn resource_refusal(
    result: Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1>,
) {
    assert!(matches!(
        result,
        Err(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::AnalysisResourceLimit { .. }
        ))
    ));
}

// These are analysis-account bounds only, not constructor/context heap bounds.
// Both original occurrence rosters consume the prefix before the two-run receipt.
fn admitted_analysis_envelope() -> (usize, usize) {
    let input = compile(ceiling()).expect("real mandatory pipeline");
    let bindings = input._session.ownership_binding_resources;
    let work = bindings
        .work_upper_bound()
        .checked_add(input.production_analysis_work_upper_bound_v1())
        .unwrap();
    let storage = bindings
        .retained_storage_upper_bound()
        .checked_add(input.production_analysis_peak_storage_upper_bound_v1())
        .unwrap();
    assert!(work > 0 && storage > 0);
    (work, storage)
    // The owned lowering input (including its session) is dropped here.
}

#[test]
fn zero_allowance_is_a_valid_refusing_configuration() {
    let a = ProductionRankedAnalysisAllowanceV1::new(0, 0).unwrap();
    assert_eq!(a.max_work(), 0);
    assert_eq!(a.max_peak_storage(), 0);
}

#[test]
fn exact_existing_hard_ceilings_are_admitted_without_widening() {
    let a = ceiling();
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    assert_eq!(a.max_work(), limits.max_work());
    assert_eq!(a.max_peak_storage(), limits.max_peak_storage());
    assert_eq!(a.limits, limits);
}

#[test]
fn one_above_work_ceiling_is_refused() {
    let a = ceiling();
    assert_eq!(
        ProductionRankedAnalysisAllowanceV1::new(
            a.max_work().checked_add(1).unwrap(),
            a.max_peak_storage()
        ),
        Err(ProductionRankedAnalysisAllowanceErrorV1::WorkAboveHardCeiling)
    );
}

#[test]
fn one_above_storage_ceiling_is_refused() {
    let a = ceiling();
    assert_eq!(
        ProductionRankedAnalysisAllowanceV1::new(
            a.max_work(),
            a.max_peak_storage().checked_add(1).unwrap()
        ),
        Err(ProductionRankedAnalysisAllowanceErrorV1::StorageAboveHardCeiling)
    );
}

#[test]
fn maximal_scalar_inputs_do_not_overflow_or_widen_policy() {
    assert_eq!(
        ProductionRankedAnalysisAllowanceV1::new(usize::MAX, usize::MAX),
        Err(ProductionRankedAnalysisAllowanceErrorV1::WorkAboveHardCeiling)
    );
    assert_eq!(
        ProductionRankedAnalysisAllowanceV1::new(0, usize::MAX),
        Err(ProductionRankedAnalysisAllowanceErrorV1::StorageAboveHardCeiling)
    );
}

#[test]
fn caller_allowance_is_retained_by_the_real_lowering_owner() {
    let a = ceiling();
    let input = compile(a).expect("real verified lowering input");
    assert_eq!(input._session.analysis_resource_limits(), a.limits);
    assert_eq!(
        input._session.limits(),
        ProductionSessionLimitsV1::default()
    );
    assert!(input._session.atomic_target.is_none());
    assert_eq!(input.kernel().function_name(), "checked");
    assert!(input.all_mandatory_reports_are_clean());
    assert!(!input.grants_artifact_or_launch_authority());
    assert!(!input.grants_compiler_refinement_authority());
    // Private inspection proves the returned object still owns the actual session.
    assert_eq!(input._session.manifest().registration_order().len(), 3);
}

#[test]
fn unchanged_default_route_keeps_its_hard_ceiling_and_recipe() {
    let original = compile_ranked_kernel_for_lowering_v1(
        construction(0),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    let selected = compile(ceiling()).unwrap();
    assert_eq!(
        original._session.analysis_resource_limits(),
        ceiling().limits
    );
    assert_eq!(original.kernel(), selected.kernel());
    assert!(original.all_mandatory_reports_are_clean());
    assert!(selected.all_mandatory_reports_are_clean());
    let direct =
        ProductionPlironSessionV1::new_ranked_v1(ProductionSessionLimitsV1::default()).unwrap();
    assert_eq!(direct.analysis_resource_limits(), ceiling().limits);
}

#[test]
fn zero_work_reaches_a_real_analysis_resource_refusal() {
    resource_refusal(compile(
        ProductionRankedAnalysisAllowanceV1::new(0, ceiling().max_peak_storage()).unwrap(),
    ));
}

#[test]
fn zero_storage_reaches_a_real_analysis_resource_refusal() {
    resource_refusal(compile(
        ProductionRankedAnalysisAllowanceV1::new(ceiling().max_work(), 0).unwrap(),
    ));
}

#[test]
fn exact_occurrence_plus_two_run_envelope_is_usable() {
    let (work, storage) = admitted_analysis_envelope();
    let a = ProductionRankedAnalysisAllowanceV1::new(work, storage).unwrap();
    let input = compile(a).expect("exact inherited analysis allowance");
    assert_eq!(input._session.analysis_resource_limits(), a.limits);
    assert!(input.all_mandatory_reports_are_clean());
}

#[test]
fn one_short_cumulative_analysis_work_refuses() {
    let (work, storage) = admitted_analysis_envelope();
    resource_refusal(compile(
        ProductionRankedAnalysisAllowanceV1::new(work - 1, storage).unwrap(),
    ));
}

#[test]
fn one_short_overlapping_analysis_storage_refuses() {
    let (work, storage) = admitted_analysis_envelope();
    resource_refusal(compile(
        ProductionRankedAnalysisAllowanceV1::new(work, storage - 1).unwrap(),
    ));
}

#[test]
fn allowance_does_not_bypass_real_memory_bounds_checks() {
    let result = compile_ranked_kernel_for_lowering_with_analysis_allowance_v1(
        construction(1),
        ProductionSessionLimitsV1::default(),
        ceiling(),
    );
    assert!(matches!(
        result,
        Err(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::RankedBounds(_)
        ))
    ));
}

#[test]
fn gfx942_companion_uses_the_original_closed_target_context() {
    let a = ceiling();
    let input = compile_ranked_kernel_for_gfx942_lowering_with_analysis_allowance_v1(
        construction(0),
        ProductionSessionLimitsV1::default(),
        [],
        a,
    )
    .expect("same gfx942 target pipeline");
    assert_eq!(input._session.analysis_resource_limits(), a.limits);
    assert!(input._session.atomic_target.is_some());
    assert!(input.all_mandatory_reports_are_clean());
    assert!(!input.grants_artifact_or_launch_authority());
}
