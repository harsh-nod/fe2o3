use fe2o3_kernel_ir::{
    DiagnosticCode, ExecutionCapabilityOperationV1 as E, ExecutionCapabilitySignatureV1,
    ExecutionCapabilitySourceOccurrenceV1, ExecutionSafetyObligationsV1, ExecutionTypeIdentityV1,
    Module, OperationKind, verify_module,
};

pub(super) fn check(module: &Module, function: usize, block: usize, operation: usize) {
    verify_module(module).expect("unmutated actual source-derived borrowed allocation must verify");
    for change in 0..11 {
        let mut changed = module.clone();
        let op = &mut changed.functions[function].body.as_mut().unwrap().blocks[block].operations
            [operation];
        let OperationKind::ExecutionCapability(contract) = &mut op.kind else {
            unreachable!()
        };
        let E::LdsAllocateBorrowed {
            workgroup_reference,
            workgroup,
            lds,
            element,
            layout,
            elements,
        } = &mut contract.operation
        else {
            panic!("actual source must use op30, not the owned-input allocation");
        };
        match change {
            0 => *workgroup_reference = *workgroup,
            1 => *workgroup = ExecutionTypeIdentityV1::new([0xf1; 32]),
            2 => {
                contract.signature =
                    ExecutionCapabilitySignatureV1::new(&[*workgroup], *lds).unwrap()
            }
            3 => contract.operands.clear(),
            4 => contract.operands[0] = op.results[0].id,
            5 => contract.epoch_before.as_mut().unwrap()[0] ^= 1,
            6 => contract.workgroup_brand.as_mut().unwrap()[0] ^= 1,
            7 => {
                contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                    contract.obligations.bits() & !ExecutionSafetyObligationsV1::LIFETIME_VALIDITY,
                )
            }
            8 => {
                contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                    contract.obligations.bits() & !ExecutionSafetyObligationsV1::ALIASING_VALIDITY,
                )
            }
            9 => {
                let occurrence = contract.source.occurrence.unwrap();
                contract.source.occurrence =
                    ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                        [0xf2; 32],
                        occurrence.expansion_identity(),
                        occurrence.expanded_root_identity(),
                        occurrence.caller_instance(),
                        occurrence.expanded_block(),
                    );
            }
            10 => {
                // Recasting the source reference as old op2 must not change the
                // real owned SSA operand into a reference-flavored capability.
                contract.operation = E::LdsAllocate {
                    workgroup: *workgroup_reference,
                    lds: *lds,
                    element: *element,
                    layout: *layout,
                    elements: *elements,
                };
                contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                    fe2o3_kernel_ir::required_execution_obligations_v1(&contract.operation),
                );
            }
            _ => unreachable!(),
        }
        let errors = verify_module(&changed)
            .expect_err("actual borrowed allocation custody mutation must reject");
        assert!(
            errors
                .diagnostics()
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidExecutionCapability),
            "allocation mutation {change} rejected for the wrong reason: {errors:?}"
        );
    }
}
