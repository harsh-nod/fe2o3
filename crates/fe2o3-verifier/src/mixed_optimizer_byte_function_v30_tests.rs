use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Function as KirFunction, MemoryAccess, Module, Operation as KirOperation, Signature,
    StorageLayoutLimitsV1, ValueDef, ValueId,
};

const LIMIT: usize = 100_000_000;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

include!("mixed_optimizer_index_byte_operations_v37_tests.rs");
include!("mixed_optimizer_byte_views_v38_tests.rs");
include!("mixed_optimizer_private_byte_obligations_v38_tests.rs");

#[test]
fn byte_function_integer_switch_reuses_exact_signed_constant_bits() {
    use fe2o3_kernel_ir::{Constant, IntegerSwitchCase};
    for (scalar, value, bits) in [
        (ScalarType::I8, Constant::I8(-1), 255u128),
        (ScalarType::I16, Constant::I16(-1), u16::MAX as u128),
        (ScalarType::I32, Constant::I32(-1), u32::MAX as u128),
        (ScalarType::I64, Constant::I64(-1), u64::MAX as u128),
    ] {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.terminator = Some(Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: vec![IntegerSwitchCase {
                value,
                target: BlockId(1),
                arguments: vec![],
            }],
            default_target: BlockId(2),
            default_arguments: vec![],
        });
        let mut selected = BasicBlock::new(BlockId(1));
        selected.terminator = Some(Terminator::Return { values: vec![] });
        let mut otherwise = BasicBlock::new(BlockId(2));
        otherwise.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("byte-integer-switch-signed-raw-bits");
        module.functions.push(KirFunction::internal_helper(
            "switch",
            Signature::new(vec![Type::Scalar(scalar)], vec![]),
            vec![ValueId(0)],
            vec![entry, selected, otherwise],
        ));
        with_inventory(&module, |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let emit = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    FormalIndexWidth::Bits64,
                    &allocations,
                    out,
                )?
                .emit(38, out)
            };
            let measured = run(floor, LIMIT, LIMIT, emit);
            let text = measured.0.unwrap();
            assert!(text.contains(&format!("if selector == {bits} ")));
            assert!(!text.contains("if selector == -1"));
            assert!(text.contains("pc: 1"));
            assert!(text.contains("pc: 2"));
            assert_eq!(run(floor, measured.1, measured.2, emit).0.unwrap(), text);
            assert!(matches!(
                run(floor, measured.1 - 1, measured.2, emit).0,
                Err(Error::Resource(_))
            ));
            assert!(matches!(
                run(floor, measured.1, measured.2 - 1, emit).0,
                Err(Error::Resource(_))
            ));
        });
    }
}

#[test]
fn byte_function_private_generic_cast_preserves_tag_without_allocating_or_ending_lifetime() {
    let private = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let restricted = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    let generic = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        KirOperation::effect_free(
            ValueDef::new(ValueId(1), restricted.clone()),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
                value: ValueId(0),
                to: restricted,
            },
        ),
        KirOperation::effect_free(
            ValueDef::new(ValueId(2), generic.clone()),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::PointerToGeneric,
                value: ValueId(1),
                to: generic.clone(),
            },
        ),
    ];
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut module = Module::new("exact-private-generic-cast");
    module.functions.push(KirFunction::internal_helper(
        "cast",
        Signature::new(vec![private], vec![generic]),
        vec![ValueId(0)],
        vec![entry],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let (result, _, _) = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(71, out)
        });
        let text = result.unwrap();
        assert!(text.contains("byte_pointer_type_v30(p, 0, 8)"));
        assert!(text.contains("MemoryOperationEffectV30::Pure"));
        assert!(!text.contains("byte_allocate_v30("));
        assert!(!text.contains("byte_end_lifetime_v30("));
        assert!(!text.contains("byte_pop_frame_v30("));
    });
}

// No authored allocation rows: this test resolver admits only modules without
// Alloca. Genuine retained SourceSlots/Alloca coverage lives with that consumer.
struct NoAllocations<'a>(&'a Owner);

impl ByteAllocationResolverV30 for NoAllocations<'_> {
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        if std::ptr::eq(self.0, owner) {
            Ok(())
        } else {
            Err(mismatch())
        }
    }

    fn site(&self, _: Operation, _: &mut Writer<'_, '_>) -> Result<ByteAllocationSiteV30> {
        Err(Error::Statement("test resolver refuses every allocation"))
    }
}

fn pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )
}

fn memory_module(functions: usize) -> Module {
    let mut module = Module::new("tagged-byte-operation-census");
    for ordinal in 0..functions {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations = vec![
            KirOperation::effect_free(
                ValueDef::new(ValueId(3), pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(0),
                    offset: ValueId(1),
                },
            ),
            KirOperation::effect_free(
                ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
                OperationKind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            KirOperation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        entry.terminator = Some(Terminator::Return {
            values: vec![ValueId(4)],
        });
        module.functions.push(KirFunction::internal_helper(
            format!("memory{ordinal}"),
            Signature::new(
                vec![pointer(), Type::INDEX, Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![entry],
        ));
    }
    module
}

fn with_inventory(module: &Module, run: impl FnOnce(&Inventory<'_>, &Physical<'_, '_>, usize)) {
    with_inventory_byte_limit(module, 4096, run);
}

fn with_inventory_byte_limit(
    module: &Module,
    max_boundaries: usize,
    run: impl FnOnce(&Inventory<'_>, &Physical<'_, '_>, usize),
) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, stored) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(stored.retained_storage()).unwrap();
    let (inventory, retained) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    let (physical, retained) = fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
        &inventory,
        fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 { max_boundaries },
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    run(&inventory, &physical, budget.storage());
}

fn run(
    floor: usize,
    work: usize,
    storage: usize,
    body: impl FnOnce(&mut Writer<'_, '_>) -> Result<()>,
) -> (Result<String>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        budget.reserve_storage(floor + super::super::super::SOURCE_LIMIT)?;
        let mut out = Writer::new(&mut budget)?;
        body(&mut out)?;
        out.finish()
    })();
    let actual = (budget.work(), budget.peak_storage());
    budget.release_storage(budget.storage()).unwrap();
    (result, actual.0, actual.1)
}

#[test]
fn byte_function_consumes_all_operations_and_preserves_exact_global_observations() {
    with_inventory(&memory_module(2), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(1),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?;
            assert_eq!(model.operation_range(out)?, 3..6);
            assert!(std::ptr::eq(
                model.operation(4, out)?,
                &inventory.operations()[4]
            ));
            assert!(model.operation(2, out).is_err());
            model.emit(19, out)
        })
        .0
        .unwrap();
        assert!(text.contains("s.values.len() != 10 || s.pc != 1"));
        assert!(text.contains("function: 1, block: 0, operation: 1"));
        assert!(text.contains("MemoryOperationEffectV30::Read { address: s.values[8], width: 4, alignment: 4, value: values[9] }"));
        assert!(text.contains("MemoryOperationEffectV30::Write { address: s.values[5], width: 4, alignment: 4, value: s.values[7] }"));
        assert!(text.contains("byte_scalar_range_initialized_v37(s.memory, p, 4)"));
        assert!(text.contains("byte_state_memory_well_formed_v30(s)"));
        assert!(text.contains("p.byte_offset + i * 4 < memory_value_modulus_v30(8)"));
        assert!(text.contains("m.next_operation == 4 && m.observations.len() == 1"));
        assert!(text.contains("m.observations.len() == 3 { byte_control_19_1_v30"));
        assert!(text.contains("observations: m.observations.push(result.observation)"));
        assert!(!text.contains("byte_end_frame_v30"));
        assert!(!text.contains("spec_fn"));
    });
}

#[test]
fn byte_function_edges_read_immutable_preedge_values_in_operand_order() {
    let mut module = Module::new("tagged-byte-parallel-edge");
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0), ValueId(1)],
    });
    let mut loop_block = BasicBlock::new(BlockId(1));
    loop_block.parameters = vec![
        ValueDef::new(ValueId(3), pointer()),
        ValueDef::new(ValueId(4), pointer()),
    ];
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(4), ValueId(3)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(KirFunction::internal_helper(
        "swap",
        Signature::new(vec![pointer(), pointer(), Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, loop_block, exit],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(2, out)
        })
        .0
        .unwrap();
        assert!(text.contains("let values = preedge.update(3, preedge[4]).update(4, preedge[3])"));
        assert!(text.contains("MemoryValueV30::Scalar(0) | MemoryValueV30::Scalar(1)"));
        assert!(
            text.contains("m.next_operation == -1 && m.state.pc == 1 && m.observations.len() == 0")
        );
    });
}

#[test]
fn byte_function_unused_and_unreachable_unsupported_operations_refuse_before_text() {
    for unreachable in [false, true] {
        let mut module = memory_module(1);
        let body = module.functions[0].body.as_mut().unwrap();
        let operation = KirOperation::effect_free(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(2),
            },
        );
        if unreachable {
            let mut dead = BasicBlock::new(BlockId(7));
            dead.operations.push(operation);
            dead.terminator = Some(Terminator::Unreachable);
            body.blocks.push(dead);
        } else {
            body.blocks[0].operations.push(operation);
        }
        with_inventory(&module, |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let result = run(floor, LIMIT, LIMIT, |out| {
                let result = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    FormalIndexWidth::Bits64,
                    &allocations,
                    out,
                );
                assert!(out.text.is_empty());
                result.map(|_| ())
            })
            .0;
            assert!(matches!(result, Err(Error::Statement(_))));
        });
    }
}

#[test]
fn byte_function_volatile_and_pointer_payloads_refuse_before_text() {
    for pointer_payload in [false, true] {
        let mut module = memory_module(1);
        let function = &mut module.functions[0];
        if pointer_payload {
            let payload = pointer();
            let address =
                Type::pointer(payload.clone(), AddressSpace::Global, AccessMode::ReadWrite);
            function.signature = Signature::new(vec![address, payload], vec![]);
            let body = function.body.as_mut().unwrap();
            body.parameters = vec![ValueId(0), ValueId(1)];
            body.blocks[0].operations = vec![KirOperation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 8),
                },
            )];
            body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
        } else if let OperationKind::Load { access, .. } =
            &mut function.body.as_mut().unwrap().blocks[0].operations[1].kind
        {
            access.volatile = true;
        }
        with_inventory(&module, |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let result = run(floor, LIMIT, LIMIT, |out| {
                let result = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    FormalIndexWidth::Bits64,
                    &allocations,
                    out,
                );
                assert!(out.text.is_empty());
                result.map(|_| ())
            })
            .0;
            assert!(matches!(result, Err(Error::Statement(_))));
        });
    }
}

#[test]
fn byte_function_foreign_owner_and_unknown_index_width_refuse_before_text() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        with_inventory(&memory_module(1), |foreign, _, _| {
            for (owner, width) in [
                (foreign.owner(), FormalIndexWidth::Bits64),
                (inventory.owner(), FormalIndexWidth::Unknown),
            ] {
                let allocations = NoAllocations(owner);
                let result = run(floor, LIMIT, LIMIT, |out| {
                    let result = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        width,
                        &allocations,
                        out,
                    );
                    assert!(out.text.is_empty());
                    result.map(|_| ())
                })
                .0;
                assert!(matches!(result, Err(Error::Statement(_))));
            }
        });
    });
}

#[test]
fn byte_function_derivation_has_independent_function_local_resource_oracle() {
    for functions in [1, 8, 32] {
        with_inventory(&memory_module(functions), |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let derive = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function((functions - 1) as u32),
                    FormalIndexWidth::Bits64,
                    &allocations,
                    out,
                )
                .map(|_| ())
            };
            // Owner1 + shape/analysis owner4 + definitions5 + three fixed
            // dispatch/probe/parser frames42 + physical census19 per operation,
            // five pointer operands8 + two effects2 + return block9.
            const WORK: usize =
                1 + 4 + 5 + 3 * (4 + 6 + 6 + 2 + 24 + 19) + 5 * 8 + 2 * 2 + 5 + 3 + 1;
            let expected_storage = floor
                + super::super::super::SOURCE_LIMIT
                + headers::<NoAllocations<'_>>()
                + 3 * size_of::<ByteOperationV30<'_, '_>>();
            let exact = run(floor, WORK, expected_storage, derive);
            assert!(exact.0.unwrap().is_empty());
            assert_eq!((exact.1, exact.2), (WORK, expected_storage));
            assert!(matches!(
                run(floor, WORK - 1, expected_storage, derive).0,
                Err(Error::Resource(_))
            ));
            assert!(matches!(
                run(floor, WORK, expected_storage - 1, derive).0,
                Err(Error::Resource(_))
            ));
        });
    }
}

#[test]
fn byte_function_emission_retains_resolver_and_budget_custody() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        run(floor, LIMIT, LIMIT, |out| {
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?;
            let paid = out.budget.storage();
            out.budget.release_storage(1)?;
            assert!(matches!(
                model.emit(0, out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(out.text.is_empty());
            out.budget.reserve_storage(1)?;
            assert_eq!(out.budget.storage(), paid);
            let work = out.budget.work();
            assert!(matches!(
                model.emit(0, out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                model.operation_range(out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                model.operation(0, out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(out.budget.work(), work);
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn byte_function_funded_foreign_ledger_poison_is_retained_after_original_restore() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        run(floor, LIMIT, LIMIT, |out| {
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?;
            assert_eq!(model.operation_range(out)?, 0..3);
            let paid = out.budget.storage();
            let mut work = Work::new(LIMIT);
            let mut foreign = Budget::new(&mut work, LIMIT);
            foreign.reserve_storage(paid)?;
            let mut foreign_out = Writer::new(&mut foreign)?;
            let foreign_work = foreign_out.budget.work();
            assert!(matches!(
                model.operation_range(&mut foreign_out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(foreign_out.budget.work(), foreign_work);
            assert!(foreign_out.text.is_empty());
            let original_work = out.budget.work();
            assert!(matches!(
                model.emit(0, out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(out.budget.work(), original_work);
            assert_eq!(out.budget.storage(), paid);
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn byte_function_full_emission_has_independent_work_and_exact_capacity_boundary() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let emit = |out: &mut Writer<'_, '_>| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                FormalIndexWidth::Bits64,
                &allocations,
                out,
            )?
            .emit(0, out)
        };
        let measured = run(floor, LIMIT, LIMIT, emit);
        let text = measured.0.unwrap();
        // Derivation246; two owner checks2, scalar-plan scan3, operation scan6,
        // pointer emission3, block frame2, ordered operation/observation scans6,
        // return operands2, micro begin1/step6/finish1, dispatcher1; text bytes.
        let work = 246 + 2 + 3 + 6 + 3 + 2 + 6 + 2 + 1 + 6 + 1 + 1 + text.len();
        let storage = floor
            + super::super::super::SOURCE_LIMIT
            + headers::<NoAllocations<'_>>()
            + 3 * size_of::<ByteOperationV30<'_, '_>>();
        assert_eq!((measured.1, measured.2), (work, storage));
        assert_eq!(run(floor, work, storage, emit).0.unwrap(), text);
        assert!(matches!(
            run(floor, work - 1, storage, emit).0,
            Err(Error::Resource(_))
        ));
        assert!(matches!(
            run(floor, work, storage - 1, emit).0,
            Err(Error::Resource(_))
        ));
    });
}

#[test]
fn byte_function_header_oracle_accounts_for_retained_plan_and_coexisting_helper_frames() {
    type R<'a> = NoAllocations<'a>;
    type FunctionFields<'a, 'b> = (
        &'a Inventory<'b>,
        &'a Physical<'a, 'b>,
        &'a R<'a>,
        Function,
        Vec<ByteOperationV30<'a, 'b>>,
        FormalIndexWidth,
        usize,
        usize,
        Ledger,
        Cell<Option<Resource>>,
    );
    assert_eq!(
        size_of::<ByteFunctionV30<'_, '_, R<'_>>>(),
        size_of::<FunctionFields<'_, '_>>()
    );
    let pointer = 2 * size_of::<PointerByteOperationV30>()
        + 2 * size_of::<Result<Option<PointerByteOperationV30>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<ByteMemoryContextNamesV30<'_>>()
        + size_of::<PointerByteEffectV30>()
        + size_of::<Range<usize>>()
        + size_of::<FormalIndexWidth>()
        + size_of::<(
            Option<usize>,
            [usize; 18],
            [bool; 4],
            [&(); 18],
            [Result<usize>; 5],
        )>();
    let private = 2 * size_of::<AllocaByteOperationV30>()
        + 2 * size_of::<Result<Option<AllocaByteOperationV30>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<ByteAllocationSiteV30>()
        + size_of::<FormalIndexWidth>()
        + size_of::<([usize; 16], [&(); 12], [Result<()>; 3])>();
    let control = size_of::<(
        [Range<usize>; 4],
        [usize; 20],
        [&(); 20],
        [Result<()>; 5],
        std::slice::Iter<'static, fe2o3_kernel_analysis::CanonicalKirUseRefV1>,
        std::slice::Iter<'static, fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>,
        std::iter::Enumerate<std::ops::Range<usize>>,
        Option<usize>,
    )>();
    let storage = 2 * size_of::<StorageByteOperationV37>()
        + 2 * size_of::<Result<Option<StorageByteOperationV37>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<ByteMemoryContextNamesV30<'_>>()
        + size_of::<PointerByteEffectV30>()
        + size_of::<([usize; 20], [&Type; 8], [Result<()>; 4])>();
    let index = 2 * size_of::<IndexByteOperationV37>()
        + 2 * size_of::<Result<Option<IndexByteOperationV37>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<(
            FormalIndexWidth,
            [&(); 5],
            [usize; 4],
            fe2o3_kernel_ir::IntrinsicKind,
            fe2o3_kernel_ir::IndexKind,
        )>();
    let model = size_of::<ByteFunctionV30<'_, '_, R<'_>>>()
        + 2 * size_of::<Result<ByteFunctionV30<'_, '_, R<'_>>>>()
        + size_of::<ByteOperationV30<'_, '_>>()
        + 2 * size_of::<Result<Option<ByteOperationV30<'_, '_>>>>()
        + size_of::<(Result<()>, Option<Resource>)>()
        + size_of::<(
            [&Inventory<'_>; 3],
            [&R<'_>; 2],
            &mut Writer<'_, '_>,
            [usize; 32],
            [Range<usize>; 5],
            [Result<()>; 6],
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryContextNamesV30<'static>,
            PointerByteEffectV30,
            [&Type; 4],
            [&str; 8],
            [Option<usize>; 3],
        )>();
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateByteObligationV38 as Obligation,
        CanonicalKirPrivateByteOperationKindV38 as Kind,
        CanonicalKirPrivateByteOperationV38 as Fact,
    };
    let physical = size_of::<(
        &Physical<'_, '_>,
        &Fact,
        &OperationKind,
        &ByteOperationV30<'_, '_>,
    )>() + 2 * size_of::<physical::Guard>()
        + size_of::<Result<physical::Guard>>()
        + size_of::<(Kind, Obligation, [usize; 2], [bool; 2], [Result<()>; 2])>();
    assert_eq!(
        super::super::pointer_byte_operations_v30::headers(),
        pointer
    );
    assert_eq!(
        super::super::private_byte_operations_v30::headers(),
        private
    );
    assert_eq!(control::headers(), control);
    assert_eq!(
        super::super::storage_byte_operations_v37::headers(),
        storage
    );
    assert_eq!(super::super::index_byte_operations_v37::headers(), index);
    assert_eq!(physical::headers(), physical);
    assert_eq!(
        headers::<R<'_>>(),
        pointer + private + storage + index + control + physical + model
    );
}

fn pointer_storage_module_v37(nonoverlapping: bool, copies: usize) -> Module {
    use fe2o3_kernel_ir::{
        StorageCopyOverlapV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind,
        StorageLayoutV1 as Layout, StorageOperationV1 as Storage, StoragePointerV1,
    };
    let mut module = Module::new("pointer-relocation-snapshot");
    module.storage_layouts = vec![
        Layout {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        Layout {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: AddressSpace::Private,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
    ];
    let holder = Type::pointer(
        Type::StorageObject(Id(1)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let value = Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let memory = MemoryAccess::new(AddressSpace::Private, 8);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(KirOperation::new(
        vec![],
        OperationKind::Storage(Storage::WriteValue {
            address: ValueId(0),
            value: ValueId(1),
            access: memory,
        }),
    ));
    for _ in 0..copies {
        entry.operations.push(KirOperation::new(
            vec![],
            OperationKind::Storage(Storage::CopyObject {
                source: ValueId(0),
                destination: ValueId(2),
                source_access: memory,
                destination_access: memory,
                overlap: if nonoverlapping {
                    StorageCopyOverlapV1::NonOverlapping
                } else {
                    StorageCopyOverlapV1::MayOverlap
                },
            }),
        ));
    }
    entry.operations.push(KirOperation::new(
        vec![ValueDef::new(ValueId(3), value.clone())],
        OperationKind::Storage(Storage::ReadValue {
            address: ValueId(2),
            access: memory,
        }),
    ));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    module.functions.push(KirFunction::internal_helper(
        "pointer-storage",
        Signature::new(vec![holder.clone(), value.clone(), holder], vec![value]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry],
    ));
    module
}

#[test]
fn byte_function_typed_pointer_storage_uses_layout_width_not_index_width() {
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
        with_inventory(
            &pointer_storage_module_v37(false, 1),
            |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let text = run(floor, LIMIT, LIMIT, |out| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        width,
                        &allocations,
                        out,
                    )?
                    .emit(17, out)
                })
                .0
                .unwrap();
                assert!(text.contains("byte_pointer_store_v37(s.memory, p, 8, v, little_endian)"));
                assert!(
                    text.contains("byte_pointer_load_valid_v37(s.memory, p, 8, little_endian)")
                );
                assert!(text.contains("byte_pointer_load_v37(s.memory, p, 8, little_endian)"));
                assert!(text.contains("byte_pointer_type_v30(v, 0, 8)"));
                assert!(!text.contains("byte_store_v30(s.memory, p, 8, v, little_endian)"));
            },
        );
    }
}

#[test]
fn byte_function_copy_observation_preserves_ordered_snapshot_and_overlap_contract() {
    for nonoverlapping in [false, true] {
        with_inventory(
            &pointer_storage_module_v37(nonoverlapping, 1),
            |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let text = run(floor, LIMIT, LIMIT, |out| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        FormalIndexWidth::Bits64,
                        &allocations,
                        out,
                    )?
                    .emit(17, out)
                })
                .0
                .unwrap();
                assert!(text.contains(&format!("byte_copy_object_valid_v37(s.memory, source, destination, 8, 8, 8, {nonoverlapping})")));
                assert!(text.contains("byte_copy_object_v37(s.memory, source, destination, 8)"));
                assert!(text.contains("MemoryOperationEffectV30::Copy { source: s.values[0], destination: s.values[2], width: 8"));
                assert_eq!(text.matches("MemoryOperationEffectV30::Copy").count(), 1);
                assert_eq!(text.matches("MemoryOperationEffectV30::Read").count(), 1);
                assert_eq!(text.matches("MemoryOperationEffectV30::Write").count(), 1);
            },
        );
    }
}

#[test]
fn byte_function_typed_pointer_copy_has_exact_and_one_short_resources() {
    with_inventory(
        &pointer_storage_module_v37(false, 1),
        |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let emit = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    FormalIndexWidth::Bits64,
                    &allocations,
                    out,
                )?
                .emit(17, out)
            };
            let (text, work, peak) = run(floor, LIMIT, LIMIT, emit);
            let text = text.unwrap();
            let (exact, used, storage) = run(floor, work, peak, emit);
            assert_eq!(exact.unwrap(), text);
            assert_eq!((used, storage), (work, peak));
            assert!(run(floor, work - 1, peak, emit).0.is_err());
            assert!(run(floor, work, peak - 1, emit).0.is_err());
        },
    );
}
