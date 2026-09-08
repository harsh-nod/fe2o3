use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    ProductionTargetLaunchEvidenceV13, lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1,
};
use fe2o3_kernel_ir::{
    Module, OperationKind, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13,
};

mod catalog_fixture {
    #![allow(dead_code, unnameable_test_items)]

    include!("../../fe2o3-kernel-ir/tests/execution_capability_catalog_v13.rs");

    fn atomic(kind: ExecutionAtomicKindV1) -> ExecutionCapabilityOperationV1 {
        use ExecutionAtomicKindV1 as Kind;

        let element = id(0x15);
        let (operand, replacement, result, success, failure) = match kind {
            Kind::BindGlobalView => (None, None, id(0x31), None, None),
            Kind::BindGlobalLocation => (Some(id(0x14)), None, id(0x31), None, None),
            Kind::Load => (
                None,
                None,
                element,
                Some(ExecutionMemoryOrderingV1::Acquire),
                None,
            ),
            Kind::Store => (
                Some(element),
                None,
                id(0x44),
                Some(ExecutionMemoryOrderingV1::Release),
                None,
            ),
            Kind::FetchAdd => (
                Some(element),
                None,
                element,
                Some(ExecutionMemoryOrderingV1::AcquireRelease),
                None,
            ),
            Kind::CompareExchange => (
                Some(element),
                Some(element),
                element,
                Some(ExecutionMemoryOrderingV1::AcquireRelease),
                Some(ExecutionMemoryOrderingV1::Acquire),
            ),
        };
        ExecutionCapabilityOperationV1::Atomic {
            kind,
            authority: id(0x11),
            location_input: id(0x30),
            location: id(0x31),
            element,
            operand,
            replacement,
            result,
            value_type: ScalarType::U32,
            address_space: ExecutionMemoryAddressSpaceV1::Global,
            scope: ExecutionMemoryScopeV1::Workgroup,
            success,
            failure,
        }
    }

    fn operations() -> Vec<ExecutionCapabilityOperationV1> {
        let mut operations = catalog();
        operations.retain(|operation| {
            !matches!(operation, ExecutionCapabilityOperationV1::Atomic { .. })
        });
        for operation in &mut operations {
            if let ExecutionCapabilityOperationV1::RawMemoryBind {
                space,
                access,
                index_space,
                atomic_scope,
                ..
            } = operation
            {
                *space = ExecutionMemoryAddressSpaceV1::Global;
                *access = ExecutionMemoryAccessV1::ExclusiveReadWrite;
                *index_space = None;
                *atomic_scope = None;
            }
        }
        operations.extend([
            atomic(ExecutionAtomicKindV1::BindGlobalView),
            atomic(ExecutionAtomicKindV1::BindGlobalLocation),
            atomic(ExecutionAtomicKindV1::Load),
            atomic(ExecutionAtomicKindV1::Store),
            atomic(ExecutionAtomicKindV1::FetchAdd),
            atomic(ExecutionAtomicKindV1::CompareExchange),
        ]);

        for kind in [
            ExecutionCollectiveKindV1::InclusiveScanSum,
            ExecutionCollectiveKindV1::ExclusiveScanSum,
        ] {
            let mut workgroup = catalog()
                .into_iter()
                .find(|operation| {
                    matches!(
                        operation,
                        ExecutionCapabilityOperationV1::WorkgroupCollective { .. }
                    )
                })
                .unwrap();
            let ExecutionCapabilityOperationV1::WorkgroupCollective {
                kind: operation_kind,
                ..
            } = &mut workgroup
            else {
                unreachable!()
            };
            *operation_kind = kind;
            operations.push(workgroup);

            let mut subgroup = catalog()
                .into_iter()
                .find(|operation| {
                    matches!(
                        operation,
                        ExecutionCapabilityOperationV1::SubgroupCollective { .. }
                    )
                })
                .unwrap();
            let ExecutionCapabilityOperationV1::SubgroupCollective {
                kind: operation_kind,
                ..
            } = &mut subgroup
            else {
                unreachable!()
            };
            *operation_kind = kind;
            operations.push(subgroup);
        }
        operations
    }

    fn finish_builder(mut builder: CatalogModuleBuilder, name: &str) -> Module {
        builder.block.terminator = Some(Terminator::Return { values: vec![] });
        let parameters = vec![
            Type::Scalar(ScalarType::U32),
            Type::INDEX,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
        ];
        let mut function = Function::kernel_entry(
            "entry",
            Signature::new(parameters, vec![]),
            (0..4).map(ValueId).collect(),
            vec![builder.block],
        );
        function.required_capabilities = builder.requirements.clone();
        let mut module = Module::new(format!("v13-lowering-{name}"));
        module.required_capabilities = builder.requirements;
        module.functions.push(function);
        module.kernels.push(Kernel::new(
            "entry_kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        module
    }

    fn private_load_module(operation: ExecutionCapabilityOperationV1) -> Module {
        let ExecutionCapabilityOperationV1::MemoryLoad { view, element, .. } = operation else {
            unreachable!()
        };
        let mut builder = CatalogModuleBuilder::new();
        let input = builder.private_view(view, element);
        builder.emit(
            ExecutionCapabilityOperationV1::MemoryLoad {
                view,
                workgroup: None,
                index: id(0x14),
                option: id(0x41),
                element,
                layout: layout(),
                space: ExecutionMemoryAddressSpaceV1::Private,
                access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
            },
            vec![input, ValueId(1)],
            EPOCH_BEFORE,
        );
        finish_builder(builder, "private-load")
    }

    fn workgroup_load_module() -> Module {
        let publish = catalog()
            .into_iter()
            .find(|operation| {
                matches!(
                    operation,
                    ExecutionCapabilityOperationV1::WorkgroupMemoryPublish { .. }
                )
            })
            .unwrap();
        let ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
            output_view,
            transition,
            element,
            ..
        } = publish
        else {
            unreachable!()
        };
        let mut builder = CatalogModuleBuilder::new();
        let publish = ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
            input_workgroup: id(0x11),
            input_view: id(0x3d),
            output_view,
            transition,
            element,
            layout: layout(),
        };
        let (operands, epoch) = builder.target_operands(&publish);
        let published = builder.emit(publish, operands, epoch);
        builder.emit(
            ExecutionCapabilityOperationV1::MemoryLoad {
                view: output_view,
                workgroup: Some(transition),
                index: id(0x14),
                option: id(0x41),
                element,
                layout: layout(),
                space: ExecutionMemoryAddressSpaceV1::Workgroup,
                access: ExecutionMemoryAccessV1::ReadOnly,
            },
            vec![published[1], published[0], ValueId(1)],
            EPOCH_AFTER,
        );
        finish_builder(builder, "workgroup-load")
    }

    pub(super) fn lowering_cases() -> Vec<(String, Module)> {
        operations()
            .into_iter()
            .enumerate()
            .map(|(ordinal, operation)| {
                let case_name = match &operation {
                    ExecutionCapabilityOperationV1::Atomic { kind, .. } => {
                        format!("Atomic::{kind:?}")
                    }
                    ExecutionCapabilityOperationV1::WorkgroupCollective { kind, .. } => {
                        format!("WorkgroupCollective::{kind:?}")
                    }
                    ExecutionCapabilityOperationV1::SubgroupCollective { kind, .. } => {
                        format!("SubgroupCollective::{kind:?}")
                    }
                    _ => name(&operation).to_owned(),
                };
                let mut module = match operation.clone() {
                    ExecutionCapabilityOperationV1::LdsAllocate { .. } => catalog()
                        .into_iter()
                        .find(|candidate| {
                            matches!(
                                candidate,
                                ExecutionCapabilityOperationV1::LdsInitializeByInvocation { .. }
                            )
                        })
                        .map(|fixture| module_for(fixture, ordinal as u8))
                        .unwrap(),
                    ExecutionCapabilityOperationV1::PrivateMemoryAllocate { .. }
                    | ExecutionCapabilityOperationV1::MemoryLoad { .. } => catalog()
                        .into_iter()
                        .find(|candidate| {
                            matches!(candidate, ExecutionCapabilityOperationV1::MemoryLoad { .. })
                        })
                        .map(private_load_module)
                        .unwrap(),
                    ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate { .. }
                    | ExecutionCapabilityOperationV1::WorkgroupMemoryPublish { .. } => {
                        workgroup_load_module()
                    }
                    other => module_for(other, ordinal as u8),
                };
                module.kernels[0].id = KernelId::new("entry_kernel");
                module.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
                module
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
                module.functions[0]
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
                module.kernels[0]
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
                let function = &mut module.functions[0];
                function.signature.parameters.truncate(4);
                function.body.as_mut().unwrap().parameters.truncate(4);
                (case_name, module)
            })
            .collect()
    }

    pub(super) fn hostile_cases() -> Vec<(String, Module)> {
        operations()
            .into_iter()
            .enumerate()
            .map(|(ordinal, operation)| {
                let case_name = name(&operation).to_owned();
                (case_name, module_for(operation, ordinal as u8))
            })
            .collect()
    }
}

fn target_operation_mut(module: &mut Module) -> &mut fe2o3_kernel_ir::Operation {
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .last_mut()
        .unwrap()
}

#[test]
fn every_v13_operation_family_and_subkind_lowers_for_both_profiles() {
    let cases = catalog_fixture::lowering_cases();
    assert_eq!(cases.len(), 32);
    for (case_name, module) in cases {
        let owner = VerifiedCanonicalKernelIrV13::from_module(module)
            .unwrap_or_else(|error| panic!("{case_name}: invalid fixture: {error}"));
        let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 23)
            .unwrap_or_else(|error| panic!("{case_name}: invalid launch evidence: {error}"));
        for profile in [
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionAmdTargetProfileV1::Gfx950,
        ] {
            let lowered =
                lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 23, &evidence, profile)
                    .unwrap_or_else(|error| panic!("{case_name} {profile:?}: {error}"));
            assert_ne!(
                lowered.capability_closure_identity(),
                [0; 32],
                "{case_name}"
            );
            assert!(
                !lowered.has_complete_operational_translation_derivation(),
                "{case_name} {profile:?} unexpectedly claimed complete operational translation"
            );
            assert!(
                lowered.structured_derivation().is_none(),
                "{case_name} {profile:?} returned a partial structured derivation"
            );
            assert!(
                !lowered.unsupported_operational_translation().is_empty(),
                "{case_name} {profile:?} did not report unsupported source operations"
            );
            assert!(
                lowered
                    .unsupported_operational_translation()
                    .iter()
                    .all(|operation| operation.function_symbol() == "entry"),
                "{case_name} {profile:?} lost source location custody"
            );
        }
    }
}

#[test]
fn every_v13_operation_rejects_operand_and_result_substitution() {
    for (case_name, module) in catalog_fixture::hostile_cases() {
        let mut operand_substitution = module.clone();
        let OperationKind::ExecutionCapability(contract) =
            &mut target_operation_mut(&mut operand_substitution).kind
        else {
            unreachable!()
        };
        contract.operands.pop();
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(operand_substitution).is_err(),
            "{case_name}: operand substitution was admitted"
        );

        let mut result_substitution = module;
        let target = target_operation_mut(&mut result_substitution);
        if target.results.pop().is_none() {
            target
                .results
                .push(ValueDef::new(ValueId(u32::MAX), Type::Unit));
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(result_substitution).is_err(),
            "{case_name}: result substitution was admitted"
        );
    }
}
