use super::tests::{STORAGE, WORK, memory, owner};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn native_effects_without_ranked_accesses_cannot_pass_empty_consumers() {
    let (owner, _) = owner(&memory());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let projection = Projection::import(&owner, &mut budget).unwrap();
    projection.with_function(0, &mut budget, |context, function, row, budget| {
        let input = NativeTraceInputV1::derive(&owner, context, row, 0, projection.epoch(), budget).unwrap();
        let census = Census { blocks: 1, operations: 3, operands: 3, results: 1, block_arguments: 1, attributes: 6, max_operation_arity: 2, ..Census::default() };
        let mut manager = Manager::new_with_resource_contract(function, census, Bound::zero(), 0, Limits::production_hard_ceiling()).unwrap();
        manager.prepare_native_exact_trace_v1(context, function, &input, budget).unwrap();
        assert!(manager.has_successful_exact_trace_v1());
        assert!(manager.native_obligations_v1().unwrap().provenance_and_alias);
        let before = manager.resource_upper_bound().work_upper_bound();
        manager.prepare_provenance_alias(context, function);
        assert!(matches!(manager.provenance_alias(), Err(super::super::pliron_provenance_alias::PlironProvenanceFailureV1::NativeObligations)));
        manager.prepare_memory_order(context, function);
        assert!(matches!(manager.memory_order(), Err(super::super::pliron_analysis_manager::PlironMemoryOrderAnalysisFailureV1::MemoryOrder(super::super::pliron_memory_order::PlironMemoryOrderFailureV1::NativeObligations))));
        assert!(!super::super::pliron_race::run_pliron_ranked_race_check_with_analyses_v1(context, function, &mut manager).is_clean());
        assert!(!super::super::pliron_workgroup_memory::run_pliron_workgroup_memory_check_with_analyses_v1(context, function, &mut manager).is_clean());
        assert!(!super::super::pliron_hierarchical_ownership::run_pliron_hierarchical_ownership_check_with_analyses_v1(context, function, &mut manager).is_clean());
        assert_eq!(manager.resource_upper_bound().work_upper_bound() - before, 5);
        Ok(())
    }).unwrap();
}

#[test]
fn ordinary_manager_keeps_the_native_terminator_refusal() {
    let (owner, _) = owner(&memory());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let projection = Projection::import(&owner, &mut budget).unwrap();
    projection
        .with_function(0, &mut budget, |context, function, _, _| {
            let mut manager = Manager::new(function);
            manager.prepare_exact_trace(context, function);
            assert!(!manager.has_successful_exact_trace_v1());
            assert!(matches!(
                manager.exact_trace(),
                Err(TraceFailure::UnsupportedTerminator { block: 0 })
            ));
            assert!(!manager.has_native_obligations_v1());
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_manager_cannot_reuse_success_for_another_root_instance() {
    let mut module = memory();
    module.kernels.push(fe2o3_kernel_ir::Kernel::new(
        "other",
        "entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(2),
        },
    ));
    let (owner, _) = owner(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let projection = Projection::import(&owner, &mut budget).unwrap();
    projection
        .with_function(0, &mut budget, |context, function, row, budget| {
            let first =
                NativeTraceInputV1::derive(&owner, context, row, 0, projection.epoch(), budget)
                    .unwrap();
            let second =
                NativeTraceInputV1::derive(&owner, context, row, 1, projection.epoch(), budget)
                    .unwrap();
            let census = Census {
                blocks: 1,
                operations: 3,
                operands: 3,
                results: 1,
                block_arguments: 1,
                attributes: 6,
                max_operation_arity: 2,
                ..Census::default()
            };
            let mut manager = Manager::new_with_resource_contract(
                function,
                census,
                Bound::zero(),
                0,
                Limits::production_hard_ceiling(),
            )
            .unwrap();
            manager
                .prepare_native_exact_trace_v1(context, function, &first, budget)
                .unwrap();
            assert!(matches!(
                manager.prepare_native_exact_trace_v1(context, function, &second, budget),
                Err(TraceFailure::Native {
                    reason: NativeTraceRefusalV1::Context,
                    ..
                })
            ));
            assert_eq!(manager.exact_trace().unwrap().len(), 1);
            Ok(())
        })
        .unwrap();
}
