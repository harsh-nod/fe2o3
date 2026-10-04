use super::*;
use crate::production_analysis::pliron_invocation_trace::{
    PlironTraceFailureV1, trace_pliron_invocations_with_inputs_v1,
};
use crate::{SparseIndexFactV1, analyze_pliron_sparse_indices_v1};
use pliron::{op::Op, operation::verify_operation, parsable::parse_from_str};

fn parse(text: &str) -> (Context, FuncOp) {
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    let op = parse_from_str(Operation::top_level_parser(), &mut context, text).unwrap();
    verify_operation(op, &context).unwrap();
    (context, FuncOp::from_operation(op))
}

fn duplicate(identical: bool) -> (Context, FuncOp) {
    parse(&format!(
        r#"
builtin.func @boolean_phi: builtin.function <(builtin.integer i1) -> ()>
{{
  ^entry(b: builtin.integer i1):
    zero = kernel.index_constant () [] [kernel_index_value: kernel.index_value 0]: <() -> (kernel.index)>;
    nine = kernel.index_constant () [] [kernel_index_value: kernel.index_value 9]: <() -> (kernel.index)>;
    gpu.cond_branch (b, zero, {other}) [^join, ^join] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 1]]: <(builtin.integer i1, kernel.index, kernel.index) -> ()>
  ^join(i: kernel.index):
    kernel.return () [] []: <() -> ()>
}}
"#,
        other = if identical { "zero" } else { "nine" }
    ))
}

fn layout() -> PlironExecutionLayoutV1 {
    PlironExecutionLayoutV1 {
        grid: 0,
        global_extents: [64, 1, 1],
        workgroup_extents: [64, 1, 1],
        subgroup_size: 64,
        execution_domain: dialect_gpu::ExecutionDomainAttr::FullPhysicalWorkgroups,
    }
}

#[test]
fn boolean_duplicate_edges_keep_distinct_sparse_phi_alternatives() {
    let (context, function) = duplicate(false);
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let entry = inventory.blocks()[0];
    let phi = inventory.blocks()[1].deref(&context).get_argument(0);
    assert_eq!(sparse.fact(phi), SparseIndexFactV1::Unknown);
    assert_eq!(
        sparse.fact(entry.deref(&context).get_argument(0)),
        SparseIndexFactV1::Unknown
    );
    let mut work = 0;
    let topology = build_subgroup_phi_selection_v1(&context, entry, &inventory, &mut work).unwrap();
    assert_eq!(topology.predecessors[1], [0, 0]);
    assert!(
        matches!(topology.selectors[0], SubgroupSelectorV1::Boolean(condition)
        if condition == entry.deref(&context).get_argument(0))
    );
}

#[test]
fn boolean_identical_incoming_numeric_value_is_preserved_not_invented() {
    let (context, function) = duplicate(true);
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let phi = inventory.blocks()[1].deref(&context).get_argument(0);
    let SparseIndexFactV1::Affine(value) = sparse.fact(phi) else {
        panic!("equal edge operands retain their actual constant");
    };
    assert_eq!(value.constant_term(), 0);
}

#[test]
fn boolean_unknown_condition_remains_located_unresolved_concrete_trace() {
    for identical in [true, false] {
        let (context, function) = duplicate(identical);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
        assert!(matches!(
            trace_pliron_invocations_with_inputs_v1(&context, &inventory, &sparse, None),
            Err(PlironTraceFailureV1::UnresolvedBranch { block: 0 })
        ));
    }
}

#[test]
fn boolean_root_selection_retains_existing_uniform_parameter_classification() {
    let (context, function) = duplicate(false);
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let uniformity =
        analyze_pliron_subgroup_uniformity(&context, &function, &inventory, layout(), &sparse)
            .unwrap();
    let condition = inventory.blocks()[0].deref(&context).get_argument(0);
    let phi = inventory.blocks()[1].deref(&context).get_argument(0);
    assert_eq!(
        uniformity.fact(condition),
        SubgroupValueUniformityV1::Uniform
    );
    assert_eq!(uniformity.fact(phi), SubgroupValueUniformityV1::Uniform);
}

#[test]
fn boolean_classifier_does_not_upgrade_unknown_or_varying_facts() {
    let (context, function) = duplicate(false);
    let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let entry = function.get_entry_block(&context);
    let condition = entry.deref(&context).get_argument(0);
    // Unit-test the existing fact lattice, not a new producer or admission API.
    for fact in [
        SubgroupValueUniformityV1::Uniform,
        SubgroupValueUniformityV1::Unknown,
        SubgroupValueUniformityV1::Varying,
    ] {
        let uniformity = PlironSubgroupUniformityV1 {
            facts: HashMap::from([(condition, fact)]),
        };
        let mut work = 0;
        assert_eq!(
            classify_subgroup_selector_v1(
                &context,
                entry,
                layout(),
                &sparse,
                &uniformity,
                SubgroupSelectorV1::Boolean(condition),
                &mut work
            )
            .unwrap(),
            fact
        );
        assert_eq!(work, 2);
    }
}

#[test]
fn boolean_selector_classification_has_exact_two_step_boundary() {
    let (context, function) = duplicate(false);
    let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let entry = function.get_entry_block(&context);
    let condition = entry.deref(&context).get_argument(0);
    let uniformity = PlironSubgroupUniformityV1 {
        facts: HashMap::new(),
    };
    let mut exact = MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 - 2;
    assert_eq!(
        classify_subgroup_selector_v1(
            &context,
            entry,
            layout(),
            &sparse,
            &uniformity,
            SubgroupSelectorV1::Boolean(condition),
            &mut exact
        )
        .unwrap(),
        SubgroupValueUniformityV1::Unknown
    );
    assert_eq!(exact, MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1);
    let mut short = MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 - 1;
    assert!(matches!(
        classify_subgroup_selector_v1(
            &context,
            entry,
            layout(),
            &sparse,
            &uniformity,
            SubgroupSelectorV1::Boolean(condition),
            &mut short
        ),
        Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded)
    ));
}

#[test]
fn boolean_loop_phi_is_carried_but_not_certified_as_finite() {
    let (context, function) = parse(
        r#"
builtin.func @boolean_loop: builtin.function <(builtin.integer i1) -> ()>
{
  ^entry(b: builtin.integer i1):
    kernel.br_args (b) [^header] []: <(builtin.integer i1) -> ()>
  ^header(c: builtin.integer i1):
    gpu.cond_branch (c, c) [^header, ^exit] [operand_segment_sizes: builtin.operand_segment_sizes [1, 1, 0]]: <(builtin.integer i1, builtin.integer i1) -> ()>
  ^exit():
    kernel.return () [] []: <() -> ()>
}
"#,
    );
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    let uniformity =
        analyze_pliron_subgroup_uniformity(&context, &function, &inventory, layout(), &sparse)
            .unwrap();
    assert_eq!(
        uniformity.fact(inventory.blocks()[1].deref(&context).get_argument(0)),
        SubgroupValueUniformityV1::Uniform
    );
    let progress = crate::production_analysis::pliron_progress::run_pliron_progress_check_v1(
        &context, &function,
    );
    assert!(
        !progress.is_clean(),
        "Boolean control supplies no finite-loop induction: {progress:?}"
    );
    assert!(progress.certificates().is_empty());
    // The concrete numeric-only binder declines the incoming Bool payload before
    // the header. It must not reinterpret it as an index to reach the loop.
    assert!(matches!(
        trace_pliron_invocations_with_inputs_v1(&context, &inventory, &sparse, None),
        Err(PlironTraceFailureV1::UnresolvedBranch { block: 0 })
    ));
}
