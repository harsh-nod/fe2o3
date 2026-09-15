use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13;

#[path = "../../../fe2o3-kernel-ir/tests/support/numerical_policy_v13.rs"]
mod fixture;

#[test]
fn numerical_policy_direct_used_result_guard_precedes_erasure() {
    for used in [false, true] {
        let module = if used {
            fixture::used_policy_module()
        } else {
            fixture::module()
        };
        VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let function = &module.functions[0];
        let body = function.body.as_ref().unwrap();
        let operation = &body.blocks[0].operations[1];
        let OperationKind::ExecutionCapability(contract) = &operation.kind else {
            unreachable!()
        };
        let types = value_types(function);
        let used_values = body
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .operations
                    .iter()
                    .flat_map(Operation::operands)
                    .chain(block.terminator.iter().flat_map(Terminator::operands))
            })
            .collect();
        let mut lowered_types = types.clone();
        let mut aliases = BTreeMap::new();
        let mut next_value = next_value_id(&module, function).unwrap();
        let original_next_value = next_value;
        let mut wave_width = None;
        let mut output = Vec::new();
        let mut helpers = Vec::new();
        // Exercise the issuance arm itself, before the outer block-parameter rejection.
        let result = lower_operation(
            &module,
            operation,
            contract,
            &types,
            &used_values,
            None,
            &BTreeMap::new(),
            &mut lowered_types,
            &mut aliases,
            &mut next_value,
            &mut wave_width,
            &mut output,
            &mut helpers,
            0,
            body.blocks[0].id,
            1,
        );
        if used {
            let errors = result.unwrap_err();
            let [diagnostic] = errors.diagnostics() else {
                panic!("expected only the numerical consumer guard");
            };
            assert_eq!(diagnostic.code, LoweringDiagnosticCode::IncompleteOperation);
            assert_eq!(
                diagnostic.message,
                "V13 numerical-policy use requires an exact consumer adapter; issuance does not discharge numerical refinement"
            );
            assert!(aliases.is_empty());
        } else {
            result.unwrap();
            assert_eq!(
                aliases,
                BTreeMap::from([(ValueId(1), ExecutionAliasV1::Erased)])
            );
        }
        assert!(output.is_empty());
        assert!(helpers.is_empty());
        assert_eq!(lowered_types, types);
        assert_eq!(next_value, original_next_value);
        assert_eq!(wave_width, None);
    }
}
