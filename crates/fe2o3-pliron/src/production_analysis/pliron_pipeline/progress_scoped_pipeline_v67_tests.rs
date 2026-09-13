#[cfg(test)]
mod progress_scoped_pipeline_v67_tests {
    use super::*;
    use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
    use dialect_kernel::{
        BranchArgsOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexType, ReturnOp,
        register_dialect,
    };
    use pliron::{
        basic_block::BasicBlock,
        builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
        dialect::DialectName,
        op::Op,
        operation::verify_operation,
        r#type::TypeHandle,
    };

    #[test]
    fn scoped_progress_reuses_the_actual_pipeline_verification_on_many_argument_uses() {
        let context = &mut Context::new();
        register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let index: TypeHandle = IndexType::get(context).into();
        let function_type = FunctionType::get(context, vec![index; 2], vec![]);
        let function = FuncOp::new(
            context,
            "scoped_progress_arguments".try_into().unwrap(),
            function_type,
        );
        let mut blocks = vec![function.get_entry_block(context)];
        for ordinal in 1..32 {
            let block = BasicBlock::new(
                context,
                Some(format!("b{ordinal}").try_into().unwrap()),
                vec![index; 2],
            );
            block.insert_at_back(function.get_region(context), context);
            blocks.push(block);
        }
        for (ordinal, block) in blocks.iter().copied().enumerate() {
            let lhs = block.deref(context).get_argument(0);
            let rhs = block.deref(context).get_argument(1);
            for _ in 0..16 {
                IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, lhs, rhs)
                    .get_operation()
                    .insert_at_back(block, context);
            }
            if let Some(next) = blocks.get(ordinal + 1) {
                BranchArgsOp::new(context, vec![lhs, rhs], *next)
                    .get_operation()
                    .insert_at_back(block, context);
            } else {
                ReturnOp::new(context)
                    .get_operation()
                    .insert_at_back(block, context);
            }
        }
        verify_operation(function.get_operation(), context).unwrap();
        let census = LivePlironStructuralIdentityProviderV1::new(context, &function)
            .capture_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .ok()
            .unwrap()
            .input_census;
        assert_eq!(census.blocks, 32);
        assert_eq!(census.operations, 544);
        assert_eq!(census.operands, 1_086);
        assert_eq!(census.results, 512);
        assert_eq!(census.block_arguments, 64);
        assert_eq!(census.successors, 31);
        assert_eq!(census.ownership_contracts, 0);
        assert_eq!(census.ranked_accesses, 0);
        // 1,086 operand visits times the old global 544-operation bound.
        assert_eq!(census.operands * census.operations, 590_784);
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .unwrap_or_else(|error| panic!("verified sparse argument graph: {error:?}"));
        assert!(report.is_clean());
        assert!(report.semantics().is_clean());
        assert!(report.preservation().is_exact_identity());
        assert_eq!(report.preservation().certificates().len(), 9);
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}
