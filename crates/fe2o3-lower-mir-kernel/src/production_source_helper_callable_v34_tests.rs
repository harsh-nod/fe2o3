mod helper_callable_tests_v34 {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn binding() -> SemanticNonBodyCallableBindingV1 {
        let owner = call_return_owner();
        let function = &owner.source_semantic().functions()[0];
        SemanticNonBodyCallableBindingV1::new(
            function.identity(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
        )
    }

    #[test]
    fn source_helper_defined_callable_classification_has_independent_one_work_limit() {
        let function = SemanticFunctionIdV1::from_index(19);
        let callable = SemanticCallableDeclV1::Defined { function };
        for limit in [1, 0] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_helper_defined_callee_v34(Some(&callable), &mut budget);
            if limit == 1 {
                assert_eq!(result.unwrap(), function);
            } else {
                assert!(result.is_err());
            }
            assert_eq!(budget.work(), limit);
            assert_eq!(budget.storage(), 0);
            assert!(source_helper_defined_callee_v34(Some(&callable), &mut budget).is_err());
            assert_eq!(budget.work(), limit);
        }
    }

    #[test]
    fn source_helper_intrinsic_classification_preserves_literal_operation_kind() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        for (operation, expected) in [
            (
                SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                    index_witness: SemanticTypeIdV1::from_index(1),
                    raw_index: SemanticTypeIdV1::from_index(2),
                },
                "source helper compiler intrinsic ThreadIndexGet is not interpreted",
            ),
            (
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: SemanticTypeIdV1::from_index(1),
                    raw_index: SemanticTypeIdV1::from_index(2),
                },
                "source helper compiler intrinsic ThreadIndex1d is not interpreted",
            ),
            (
                SemanticCompilerIntrinsicOperationV1::Trap,
                "source helper compiler intrinsic Trap is not interpreted",
            ),
            (
                SemanticCompilerIntrinsicOperationV1::ColdPath,
                "source helper compiler intrinsic ColdPath is not interpreted",
            ),
        ] {
            // Inert classification inputs, not an admitted intrinsic ABI or
            // execution context. A diagnostic cannot grant either authority.
            let callable = SemanticCallableDeclV1::CompilerIntrinsic {
                binding: binding(),
                operation,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([31; 32]),
            };
            assert!(
                matches!(source_helper_defined_callee_v34(Some(&callable), &mut budget),
                Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected)
            );
        }
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn source_helper_ffi_and_missing_callable_do_not_acquire_defined_invocations() {
        let callable = SemanticCallableDeclV1::DeviceFfiImport {
            binding: binding(),
            contract: SemanticDeviceFfiImportContractV1::new(
                SemanticDeviceFfiContractIdentityV1::from_sha256([32; 32]),
                SemanticLinkSymbolV1::new(b"diagnostic_only".to_vec()).unwrap(),
                SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus,
                SemanticCodeObjectVersionV1::V6,
                SemanticDeviceFfiPhysicalAbiIdentityV1::from_sha256([33; 32]),
                SemanticDeviceFfiEffectsV1::none(),
                SemanticDeviceFfiSemanticIdentityV1::from_sha256([34; 32]),
            ),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(matches!(
            source_helper_defined_callee_v34(Some(&callable), &mut budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source helper DeviceFfiImport computation is not interpreted"
            ))
        ));
        assert!(matches!(
            source_helper_defined_callee_v34(None, &mut budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source helper original callable is absent"
            ))
        ));
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.storage(), 0);
    }
}
