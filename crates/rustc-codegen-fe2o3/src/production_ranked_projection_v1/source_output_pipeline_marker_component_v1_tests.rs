// Structured V12 graph coverage only: no semantic source owner or NativeSource admission.
mod marker_component_tests {
    use super::*;
    use fe2o3_kernel_analysis::{
        check_canonical_kir_transition_v1, transport_kernel_ir_contract_catalog_v1,
    };
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, Constant, Function,
        KernelIrPipelineContractDefinitionV1 as Contract,
        KernelIrPipelineStorageBindingV1 as Binding, Module, Operation, OperationKind, ScalarType,
        Signature, TargetCapability, Terminator, Type, ValueDef, ValueId,
        VerificationContractKeyV12, VerificationContractOperationV12, WorkgroupMemory,
        WorkgroupMemoryExtent, WorkgroupPipelineEventKindV12 as Event,
    };

    fn marker_module() -> Module {
        let mut block = BasicBlock::new(BlockId(812));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(111), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(42)),
        ));
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(900),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 4,
            }),
        ));
        for kind in [
            Event::Stage,
            Event::Commit,
            Event::Wait,
            Event::Consume,
            Event::Discard,
            Event::Release,
        ] {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::VerificationContract(
                    VerificationContractOperationV12::WorkgroupPipelineEvent {
                        contract: VerificationContractKeyV12::new(0),
                        kind,
                        storage: ValueId(900),
                        epoch: ValueId(444),
                    },
                ),
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut function = Function::internal_helper(
            "marker_component",
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(444)],
            vec![block],
        );
        function
            .required_capabilities
            .insert(TargetCapability::WorkgroupMemory);
        let mut module = Module::new("marker-component-not-source-qualified");
        module
            .required_capabilities
            .insert(TargetCapability::WorkgroupMemory);
        module.functions.push(function);
        module
    }

    #[test]
    fn structural_six_marker_graph_preserves_checked_placements_and_missing_binding_rejection() {
        let module = marker_module();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let (input_owner, input_receipt) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(input_receipt.retained_storage())
            .unwrap();
        let checked_output =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_v1(&input_owner, &mut budget)
                .unwrap();
        let checked_storage = checked_output.storage();
        budget
            .reserve_storage(checked_storage.retained_storage())
            .unwrap();
        assert_ne!(input_owner.module(), checked_output.owner().module());
        assert_eq!(
            input_owner.canonical().canonical_bytes(),
            checked_output.native_input_audit_bytes()
        );
        assert!(!checked_output.grants_authority());

        let (input, input_storage) = Inventory::derive(&input_owner, &mut budget).unwrap();
        budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let (output, output_storage) =
            Inventory::derive(checked_output.owner(), &mut budget).unwrap();
        budget
            .reserve_storage(output_storage.retained_storage())
            .unwrap();
        let (transition, transition_storage) = check_canonical_kir_transition_v1(
            &input,
            &output,
            checked_output.occurrences().candidate(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(transition_storage.retained_storage())
            .unwrap();
        let (catalog, catalog_storage) = Catalog::from_rows_with_budget(
            [1; 32],
            &[Contract {
                key: 0,
                semantic_pipeline_type: 9,
                semantic_payload_type: 3,
                buffers: 2,
                elements: 32,
                prefetch_distance: 1,
                packed_bits: 32,
                source_size_bytes: 4,
                source_alignment_bytes: 4,
            }],
            &[Binding {
                function: 0,
                storage: 900,
                key: 0,
                block: 0,
                operation: 1,
            }],
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(catalog_storage.retained_storage())
            .unwrap();
        let (input_catalog, input_catalog_storage) =
            check_kernel_ir_contract_catalog_v1(&input, &catalog, &mut budget).unwrap();
        budget
            .reserve_storage(input_catalog_storage.retained_storage())
            .unwrap();
        assert_eq!(input_catalog.marker_count(), 6);
        let (transported, transported_storage) =
            transport_kernel_ir_contract_catalog_v1(&transition, &input_catalog, &mut budget)
                .unwrap();
        budget
            .reserve_storage(transported_storage.retained_storage())
            .unwrap();
        assert!(!transported.grants_authority());
        assert_eq!(transported.allocations().len(), 1);
        assert_eq!(transported.markers().len(), 6);
        assert_eq!(transported.catalog().bindings()[0].operation, 0);
        for (ordinal, row) in transported.markers().iter().enumerate() {
            assert_eq!(row.input.operation, u32::try_from(ordinal + 2).unwrap());
            let actual = row.output.expect("all six physical markers remain");
            assert_eq!(actual.operation, u32::try_from(ordinal + 1).unwrap());
        }
        check_actual_catalog(
            checked_output.owner(),
            transported.catalog(),
            6,
            &mut budget,
        );

        let (missing, missing_storage) = Catalog::from_rows_with_budget(
            *transported.catalog().semantic_source(),
            transported.catalog().definitions(),
            &[],
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(missing_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let result = check_kernel_ir_contract_catalog_v1(&output, &missing, &mut budget);
        assert!(
            matches!(
                &result,
                Err(BindingError::Invalid("marker storage has no binding"))
            ),
            "{result:?}"
        );
        assert_eq!(budget.storage(), floor);
        drop(result);
        drop(missing);
        budget
            .release_storage(missing_storage.retained_storage())
            .unwrap();
        drop(transported);
        budget
            .release_storage(transported_storage.retained_storage())
            .unwrap();
        drop(input_catalog);
        budget
            .release_storage(input_catalog_storage.retained_storage())
            .unwrap();
        drop(catalog);
        budget
            .release_storage(catalog_storage.retained_storage())
            .unwrap();
        drop(transition);
        budget
            .release_storage(transition_storage.retained_storage())
            .unwrap();
        drop(output);
        budget
            .release_storage(output_storage.retained_storage())
            .unwrap();
        drop(input);
        budget
            .release_storage(input_storage.retained_storage())
            .unwrap();
        drop(checked_output);
        budget
            .release_storage(checked_storage.retained_storage())
            .unwrap();
        drop(input_owner);
        budget
            .release_storage(input_receipt.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), PREFIX);
    }
}
