// Origin/geometry equations only. Authentic descriptor authority is tested by
// the original-source full consumer, not by this inert physical model.
fn descriptor_local_geometry_module_v29(fault: usize) -> Module {
    let mut module = literal_array_module(1, 0);
    let function = &mut module.functions[0];
    let generic = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite);
    let global = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Global, AccessMode::ReadWrite);
    function.signature.parameters.push(if fault == 3 { generic.clone() } else { global.clone() });
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(100));
    let block = &mut body.blocks[0];
    if fault == 1 {
        let OperationKind::GetElementPointer { offset, .. } = &mut block.operations[8].kind else { unreachable!() };
        *offset = ValueId(0);
    }
    if fault == 2 {
        block.operations[7].kind = OperationKind::Constant(Constant::Index(1));
    }
    let mut external = ValueId(100);
    let mut external_type = if fault == 3 { generic.clone() } else { global };
    if fault == 4 {
        block.operations.extend([
            Operation::effect_free(ValueDef::new(ValueId(110), generic.clone()), OperationKind::Cast {
                kind: CastKind::PointerToGeneric, value: ARRAY, to: generic.clone(),
            }),
            Operation::effect_free(ValueDef::new(ValueId(111), generic.clone()), OperationKind::Cast {
                kind: CastKind::PointerToGeneric, value: ValueId(100), to: generic.clone(),
            }),
            Operation::effect_free(ValueDef::new(ValueId(112), Type::BOOL), OperationKind::Constant(Constant::Bool(true))),
            Operation::effect_free(ValueDef::new(ValueId(113), generic.clone()), OperationKind::Select {
                condition: ValueId(112), true_value: ValueId(110), false_value: ValueId(111),
            }),
        ]);
        external = ValueId(113);
        external_type = generic;
    }
    block.operations.push(Operation::effect_free(ValueDef::new(ValueId(101), external_type.clone()),
        OperationKind::GetElementPointer { base: external, offset: ValueId(0) }));
    let Type::Pointer(pointer) = &external_type else { unreachable!() };
    block.operations.push(Operation::effect_free(ValueDef::new(ValueId(102), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: if fault == 5 { UNUSED } else { ValueId(101) },
            access: MemoryAccess::new(if fault == 5 { AddressSpace::Private } else { pointer.address_space }, 4),
        }));
    module
}

fn descriptor_local_geometry_v29(fault: usize, work_limit: usize, storage_limit: usize)
    -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize)
{
    let module = descriptor_local_geometry_module_v29(fault);
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut setup = ArgumentBudgetV1::new(&mut setup_work, LIMIT);
    let (owner, receipt) = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        &module, ProductionSemanticKirLimitsV1::default().storage_layout_limits(), &mut setup,
    ).unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut setup).unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let slots = [literal_array_slot(1)];
    let accesses = [SourceAddressAccessV29 { block: BlockId(0), operation: 5, slot: 0 },
        SourceAddressAccessV29 { block: BlockId(0), operation: 6, slot: 0 }];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_inventory(&inventory, CanonicalKirFunctionCoordinateV1(0),
            &slots, &accesses, budget)?.solve(&slots, &accesses, &[], budget)?;
        assert_eq!(graph.exact(ARRAY, budget)?, Some(0));
        assert_eq!(graph.exact(ValueId(101), budget)?, None);
        drop(graph);
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn descriptor_local_equations_keep_unused_private_unknown_and_mixed_geps_closed() {
    for fault in 1..=5 {
        let positive = descriptor_local_geometry_v29(0, LIMIT, LIMIT).0;
        assert!(positive.is_ok(), "{positive:?}");
        let error = descriptor_local_geometry_v29(fault, LIMIT, LIMIT).0.unwrap_err();
        assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
            function: 0, block: None, statement: None,
            detail: "source raw address differs from its actual formation or memory history",
        }), "fault={fault}: {error:?}");
    }
}

#[test]
fn descriptor_local_equations_preserve_exact_and_short_resource_cleanup() {
    let (positive, work, peak) = descriptor_local_geometry_v29(0, LIMIT, LIMIT);
    assert!(positive.is_ok(), "{positive:?}");
    assert!(descriptor_local_geometry_v29(0, work, peak).0.is_ok());
    for (work, storage, is_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let error = descriptor_local_geometry_v29(0, work, storage).0.unwrap_err();
        assert!(matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))) && is_work
            || matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))) && !is_work,
            "{error:?}");
    }
}
