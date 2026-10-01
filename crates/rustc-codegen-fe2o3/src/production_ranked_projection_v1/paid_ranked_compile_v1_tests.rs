//! Real typed recipe/compiler-selection controls, not source-owner admission.
use super::*;
use std::cell::Cell;

fn construction() -> ProductionConstructionV1 {
    use fe2o3_pliron::{
        ProductionRankedOperationV1 as Op, ProductionRankedValueIdV1 as Id,
        ProductionRankedValueV1 as Value,
    };
    let view = Id::new(0);
    let index = Id::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "checked",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                Op::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1; 3],
                    workgroup_extents: [1; 3],
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
                    value: 0,
                },
                Op::Access {
                    kind: AccessKindAttr::Read,
                    view: Value::Local(view),
                    indices: vec![Value::Local(index)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    ProductionConstructionV1::ranked_kernel("selected_root", kernel).unwrap()
}

#[test]
fn exact_single_ordinary_shape_only() {
    assert!(require_single_ordinary(1, 0).is_ok());
    for (roots, references) in [(0, 0), (2, 0), (1, 1), (usize::MAX, 0), (1, usize::MAX)] {
        assert!(matches!(
            require_single_ordinary(roots, references),
            Err(ProductionRankedProjectionErrorV1::Unsupported(_))
        ));
    }
}

#[test]
fn legacy_selection_is_the_original_gfx942_engine() {
    let original = compile_ranked_kernel_for_gfx942_lowering_v1(
        construction(),
        ProductionSessionLimitsV1::default(),
        vec![1],
    )
    .unwrap();
    let mut selection = Selection::Legacy;
    assert!(selection.is_legacy());
    let selected = selection
        .compile(construction(), vec![1], "diagnostic".into(), vec![])
        .unwrap();
    assert_eq!(original.kernel(), selected.kernel());
    assert_eq!(
        original.exact_graph_identity(),
        selected.exact_graph_identity()
    );
}

#[test]
fn selected_compiler_receives_exact_construction_and_coherence_and_is_once_only() {
    let calls = Cell::new(0);
    let mut once = Some(
        |construction: ProductionConstructionV1, coherent: Vec<u64>| {
            calls.set(calls.get() + 1);
            assert_eq!(coherent, vec![1]);
            compile_ranked_kernel_for_gfx942_lowering_v1(
                construction,
                ProductionSessionLimitsV1::default(),
                coherent,
            )
        },
    );
    let mut callback = |construction, coherent| once.take().map(|f| f(construction, coherent));
    let mut selection = Selection::Paid(&mut callback);
    assert!(!selection.is_legacy());
    let selected = selection
        .compile(construction(), vec![1], "first".into(), vec![])
        .unwrap();
    assert!(selected.all_mandatory_reports_are_clean());
    assert!(matches!(
        selection.compile(construction(), vec![1], "second".into(), vec![]),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "paid ordinary compiler was already consumed"
        ))
    ));
    assert_eq!(calls.get(), 1);
}

#[test]
fn selected_typed_refusal_retains_diagnostics_and_never_falls_back() {
    let calls = Cell::new(0);
    let mut callback = |construction, coherent| {
        calls.set(calls.get() + 1);
        Some(fe2o3_pliron::compile_ranked_kernel_for_gfx942_lowering_with_analysis_and_snapshot_allowances_v1(
            construction, ProductionSessionLimitsV1::default(), coherent,
            fe2o3_pliron::ProductionRankedAnalysisAllowanceV1::new(0, 0).unwrap(),
            fe2o3_pliron::ProductionRankedSnapshotAllowanceV1::production_hard_ceiling(),
        ))
    };
    let mut selection = Selection::Paid(&mut callback);
    match selection.compile(
        construction(),
        vec![1],
        "retained diagnostic".into(),
        vec![],
    ) {
        Err(ProductionRankedProjectionErrorV1::Compile {
            error,
            ranked_ir,
            access_sources,
        }) => {
            assert!(matches!(
                *error,
                ProductionRankedCompileErrorV1::Session(
                    ProductionSessionErrorV1::AnalysisResourceLimit { .. }
                )
            ));
            assert_eq!(ranked_ir, "retained diagnostic");
            assert!(access_sources.is_empty());
        }
        _ => panic!("original typed analysis refusal"),
    }
    assert_eq!(calls.get(), 1);
}

#[test]
fn consumed_callback_stays_consumed_after_unwind() {
    let calls = Cell::new(0);
    let mut once = Some(
        |_: ProductionConstructionV1, _: Vec<u64>| -> CompileResult {
            calls.set(calls.get() + 1);
            panic!("controlled selected compiler panic");
        },
    );
    let mut callback = |construction, coherent| once.take().map(|f| f(construction, coherent));
    let mut selection = Selection::Paid(&mut callback);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        selection.compile(construction(), vec![1], "panic".into(), vec![])
    }));
    assert!(result.is_err());
    assert!(matches!(
        selection.compile(construction(), vec![1], "no retry".into(), vec![]),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "paid ordinary compiler was already consumed"
        ))
    ));
    assert_eq!(calls.get(), 1);
}
