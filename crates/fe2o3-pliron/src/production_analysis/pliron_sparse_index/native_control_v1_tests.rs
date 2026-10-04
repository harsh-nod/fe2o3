use super::*;
use dialect_gpu::{
    optimization_v1::BranchOp as GpuBranchOp,
    switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3, SwitchOpV3},
};
use dialect_kernel::{IndexType, ReturnOp};
use pliron::{
    builtin::types::{FunctionType, IntegerType, Signedness},
    dialect::DialectName,
    operation::verify_operation,
};

fn context() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(&mut context, &DialectName::try_new("kernel").unwrap())
        .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn census(context: &Context, function: &FuncOp) -> ProductionAnalysisInputCensusV1 {
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
    let mut census = ProductionAnalysisInputCensusV1 {
        blocks: inventory.blocks().len(),
        operations: inventory.operations().len(),
        ..ProductionAnalysisInputCensusV1::default()
    };
    for block in inventory.blocks() {
        census.block_arguments += block.deref(context).get_num_arguments();
    }
    for site in inventory.operations() {
        let raw = site.pointer().deref(context);
        census.operands += raw.get_num_operands();
        census.results += raw.get_num_results();
        census.successors += raw.get_num_successors();
    }
    census
}

#[test]
fn native_switch_sparse_merges_visit_every_original_case_and_default_tuple() {
    for cases in [0, 1, 16, 17, 64] {
        for changed in [None, Some(0), Some(cases)] {
            let context = &mut context();
            let selector_type = IntegerType::get(context, 128, Signedness::Unsigned).into();
            let index = IndexType::get(context).into();
            let signature = FunctionType::get(context, vec![selector_type], vec![]);
            let function = FuncOp::new(context, "native_switch_phi".try_into().unwrap(), signature);
            let entry = function.get_entry_block(context);
            let selector = entry.deref(context).get_argument(0);
            let join = BasicBlock::new(context, None, vec![index; 2]);
            join.insert_at_back(function.get_region(context), context);
            let seven = IndexConstantOp::new(context, 7);
            let nine = IndexConstantOp::new(context, 9);
            let edges = (0..=cases)
                .map(|ordinal| {
                    SwitchEdgeV3::new(
                        join,
                        if Some(ordinal) == changed {
                            vec![nine.result(context), seven.result(context)]
                        } else {
                            vec![seven.result(context), nine.result(context)]
                        },
                    )
                })
                .collect();
            let switch = SwitchOpV3::try_new(
                context,
                selector,
                SwitchKeyKindAttrV3::LegacyU64,
                (0..cases).map(|i| u64::MAX - i as u64).collect(),
                edges,
            )
            .unwrap();
            for operation in [
                seven.get_operation(),
                nine.get_operation(),
                switch.get_operation(),
            ] {
                operation.insert_at_back(entry, context);
            }
            ReturnOp::new(context)
                .get_operation()
                .insert_at_back(join, context);
            verify_operation(function.get_operation(), context).unwrap();
            let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
            for slot in 0..2 {
                let result = join.deref(context).get_argument(slot);
                let expected = if changed.is_some() && cases != 0 {
                    None
                } else if (slot == 0) != changed.is_some() {
                    Some(7)
                } else {
                    Some(9)
                };
                assert_eq!(
                    analysis.fact(result).constant_value(),
                    expected,
                    "C={cases}, change={changed:?}, slot={slot}"
                );
            }
            let shape = collect_sparse_index_merge_census_v1(
                context,
                &function,
                census(context, &function),
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            )
            .unwrap();
            assert_eq!(shape.argument_type_work, 4);
            assert_eq!(shape.inputs, 2 * (cases + 1));
            assert_eq!(shape.squared_inputs, 2 * (cases + 1) * (cases + 1));
        }
    }
}

#[test]
fn native_branch_uses_cached_exact_types_and_rejects_cross_type_payloads() {
    for wrong in [false, true] {
        let context = &mut context();
        let index = IndexType::get(context).into();
        let physical = IntegerType::get(context, 64, Signedness::Unsigned).into();
        let signature = FunctionType::get(context, vec![index, physical], vec![]);
        let function = FuncOp::new(context, "native_branch_phi".try_into().unwrap(), signature);
        let entry = function.get_entry_block(context);
        let join = BasicBlock::new(context, None, vec![index; 3]);
        join.insert_at_back(function.get_region(context), context);
        let value = entry.deref(context).get_argument(usize::from(wrong));
        let branch = GpuBranchOp::new(context, join, vec![value; 3]);
        branch.get_operation().insert_at_back(entry, context);
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(join, context);
        let result = analyze_pliron_sparse_indices_v1(context, &function);
        if wrong {
            assert_eq!(
                result.unwrap_err(),
                malformed("typed edge operand and block argument types differ")
            );
        } else {
            verify_operation(function.get_operation(), context).unwrap();
            let analysis = result.unwrap();
            assert_eq!(
                analysis.stable_root(context, &function, join.deref(context).get_argument(2)),
                Some(SparseStableRootV1::EntryArgument(value))
            );
        }
        let input = census(context, &function);
        let limits = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        let shape =
            collect_sparse_index_merge_census_v1(context, &function, input, limits).unwrap();
        assert_eq!(shape.argument_type_work, 9); // 1+2 for entry, 1+2+3 for join.
        assert_eq!(shape.inputs, 3);
        let bound = sparse_index_resource_upper_bound_v1(input, shape, limits).unwrap();
        for limits in [
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound() - 1,
                bound.peak_storage_upper_bound(),
            ),
            ProductionAnalysisResourceLimitsV1::new(
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound() - 1,
            ),
        ] {
            assert!(sparse_index_resource_upper_bound_v1(input, shape, limits).is_err());
        }
    }
}
