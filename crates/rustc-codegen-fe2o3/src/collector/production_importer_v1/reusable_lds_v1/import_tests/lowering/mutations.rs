use fe2o3_kernel_ir::{
    DiagnosticCode, ExecutionSafetyObligationsV1, Module, OperationKind, verify_module,
};

pub(super) fn check(module: &Module, function: usize, block: usize, operation: usize) {
    for change in 0..6 {
        let mut changed = module.clone();
        let op = &mut changed.functions[function].body.as_mut().unwrap().blocks[block].operations
            [operation];
        let OperationKind::ExecutionCapability(contract) = &mut op.kind else {
            unreachable!()
        };
        match change {
            0 => contract.source.occurrence = None,
            1 => contract.operands.clear(),
            2 => contract.operands[0] = op.results[0].id,
            3 => contract.epoch_before.as_mut().unwrap()[0] ^= 1,
            4 => contract.workgroup_brand.as_mut().unwrap()[0] ^= 1,
            5 => {
                contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                    contract.obligations.bits() & !ExecutionSafetyObligationsV1::ALIASING_VALIDITY,
                )
            }
            _ => unreachable!(),
        }
        let error = verify_module(&changed)
            .expect_err("the actual source-derived conversion must reject this custody mutation");
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidExecutionCapability),
            "mutation {change} rejected for the wrong reason: {error:?}",
        );
    }
}
