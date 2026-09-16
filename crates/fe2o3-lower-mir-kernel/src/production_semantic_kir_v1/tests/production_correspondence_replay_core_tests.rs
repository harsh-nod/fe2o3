use super::*;

#[derive(Clone, Copy)]
pub(super) enum Expected {
    Accepted,
    CorrespondenceMismatch,
}

fn source_and_output() -> (
    ProductionSemanticSsaOwnerV1,
    Module,
    SemanticKirCorrespondenceV1,
) {
    let source = ProductionSemanticSsaOwnerV1::try_new(
        helper_closure_semantic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let (module, correspondence) =
        lower_module(&source, ProductionSemanticKirLimitsV1::default(), None).unwrap();
    assert_eq!(source.source_semantic().functions().len(), 2);
    assert_eq!(
        source.source_semantic().roots(),
        [SemanticFunctionIdV1::from_index(0)]
    );
    assert_eq!((module.functions.len(), module.kernels.len()), (2, 1));
    assert_eq!(correspondence.lowered_functions.len(), 2);
    assert_eq!(correspondence.blocks.len(), 3);
    assert_eq!(correspondence.terminator_operation_spans.len(), 3);
    assert!(correspondence.parameter_bindings.is_empty());
    (source, module, correspondence)
}

pub(super) fn check_both(
    source: &ProductionSemanticSsaOwnerV1,
    module: &Module,
    roots: &[SemanticFunctionIdV1],
    max_blocks: usize,
    correspondence: &SemanticKirCorrespondenceV1,
    expected: Expected,
) {
    // The private core's precondition is established by a real replay on the
    // same immutable owner. No fabricated proof token or skip mode is exposed.
    source.verify_replay().unwrap();
    let limits = ProductionSemanticKirLimitsV1 {
        max_blocks,
        ..ProductionSemanticKirLimitsV1::default()
    };
    let core = validate_semantic_kir_correspondence_after_source_replay_v1(
        source,
        module,
        roots,
        limits,
        correspondence,
    );
    let wrapper =
        validate_semantic_kir_correspondence(source, module, roots, limits, correspondence);
    for result in [core, wrapper] {
        match expected {
            Expected::Accepted => result.unwrap(),
            Expected::CorrespondenceMismatch => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            )),
        }
    }
}

#[test]
fn correspondence_replay_and_core_accept_the_same_actual_helper_output() {
    let (source, module, correspondence) = source_and_output();
    assert!(source.occurrences_v1().is_none());
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &correspondence,
        Expected::Accepted,
    );
}

#[test]
fn captured_source_does_not_change_the_legacy_correspondence_wrapper() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    };
    let (mut source, module, correspondence) = source_and_output();
    let identity = source.identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16 * 1024 * 1024);
    let receipt = source
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 0);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(source.identity(), identity);
    let capture = source.occurrences_v1().unwrap();
    assert_eq!(capture.function_count(), 2);
    assert!(std::ptr::eq(
        capture
            .function(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .owner(),
        &source
    ));
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &correspondence,
        Expected::Accepted,
    );
    let mut changed = correspondence.clone();
    changed.semantic_sha256[0] ^= 1;
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &changed,
        Expected::CorrespondenceMismatch,
    );
    drop(source);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn correspondence_core_preserves_exact_structural_rejections() {
    let (source, module, correspondence) = source_and_output();
    let roots = source.source_semantic().roots();
    let limits = ProductionSemanticKirLimitsV1::default();
    for mutation in 0..8 {
        let mut changed = correspondence.clone();
        let mut graph = module.clone();
        let foreign = [SemanticFunctionIdV1::from_index(1)];
        let expected_roots = match mutation {
            0 => &[][..],
            1 => &foreign,
            _ => roots,
        };
        match mutation {
            0 | 1 => {}
            2 => changed.function_count += 1,
            3 => graph.kernels[0].entry = FunctionId::new("foreign_entry"),
            4 => {
                changed.lowered_functions =
                    changed.lowered_functions[..1].to_vec().into_boxed_slice()
            }
            5 => changed.blocks[0].semantic_block = SemanticBlockIdV1::from_index(99),
            6 => changed.terminator_operation_spans[0].operation_count += 1,
            7 => {
                changed.parameter_bindings = vec![SemanticKirParameterBindingV1 {
                    correspondence_owner: SemanticFunctionIdV1::from_index(0),
                    semantic_function: SemanticFunctionIdV1::from_index(0),
                    semantic_local: SemanticLocalIdV1::from_index(0),
                    kernel_ir_value: ValueId(0),
                }]
                .into_boxed_slice()
            }
            _ => unreachable!(),
        }
        check_both(
            &source,
            &graph,
            expected_roots,
            limits.max_blocks,
            &changed,
            Expected::CorrespondenceMismatch,
        );
    }
    // The closure rejects three required blocks against a limit of two.
    // Both correspondence entries preserve their existing mismatch mapping.
    let semantic = source.source_semantic();
    let selected_body = semantic
        .select_kernel_body_for_root_v1(roots[0])
        .unwrap()
        .body();
    let mut closure_budget = ReachableClosureBudgetV1::new(2);
    assert!(matches!(
        reachable_defined_closure_v1(
            semantic,
            selected_body,
            correspondence.function_count,
            &mut closure_budget,
        ),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Blocks,
            actual: 3,
            limit: 2,
        })
    ));
    check_both(
        &source,
        &module,
        roots,
        2,
        &correspondence,
        Expected::CorrespondenceMismatch,
    );
    check_both(
        &source,
        &module,
        roots,
        3,
        &correspondence,
        Expected::Accepted,
    );
}

#[test]
fn correspondence_core_rejects_a_different_genuinely_admitted_source_owner() {
    let (_source, module, correspondence) = source_and_output();
    let foreign = ProductionSemanticSsaOwnerV1::try_new(
        noop_semantic_owner(&["foreign_root"]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    check_both(
        &foreign,
        &module,
        foreign.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &correspondence,
        Expected::CorrespondenceMismatch,
    );
}
