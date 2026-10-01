use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_transition_v18;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, MemoryAccess, Module, Operation as Instruction, Signature, StorageLayoutLimitsV1,
    Terminator, UnaryOp, ValueDef, ValueId,
};

const LIMIT: usize = 256 * 1024 * 1024;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 0,
    edges: 0,
    containment_depth: 0,
    object_bytes: 0,
};

#[path = "mixed_optimizer_typed_prefix_v49_tests.rs"]
mod prefix_tests;

fn block(id: u32, operations: Vec<Instruction>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}

fn fixture() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let branch = |target| Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    };
    let store = |pointer, value| {
        Instruction::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(pointer),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    };
    let alloca = |value| {
        Instruction::effect_free(
            ValueDef::new(ValueId(value), pointer.clone()),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        )
    };
    let mut module = Module::new("typed-allocation-event-bridge");
    module.functions.push(Function::internal_helper(
        "allocation_bridge",
        Signature::new(vec![scalar.clone(), Type::BOOL], vec![scalar.clone()]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                0,
                vec![Instruction::effect_free(
                    ValueDef::new(ValueId(9), scalar.clone()),
                    OperationKind::Constant(Constant::U32(99)),
                )],
                branch(1),
            ),
            block(
                1,
                vec![alloca(2), alloca(3), store(2, 0), store(3, 0)],
                branch(2),
            ),
            block(
                2,
                vec![],
                Terminator::ConditionalBranch {
                    condition: ValueId(1),
                    then_target: BlockId(3),
                    then_arguments: vec![],
                    else_target: BlockId(4),
                    else_arguments: vec![],
                },
            ),
            block(
                3,
                vec![
                    Instruction::effect_free(
                        ValueDef::new(ValueId(10), scalar.clone()),
                        OperationKind::Unary {
                            op: UnaryOp::Not,
                            operand: ValueId(0),
                        },
                    ),
                    Instruction::effect_free(
                        ValueDef::new(ValueId(11), scalar.clone()),
                        OperationKind::Binary {
                            op: BinaryOp::BitXor,
                            lhs: ValueId(0),
                            rhs: ValueId(0),
                        },
                    ),
                    Instruction::effect_free(
                        ValueDef::new(ValueId(12), scalar.clone()),
                        OperationKind::Binary {
                            op: BinaryOp::BitXor,
                            lhs: ValueId(0),
                            rhs: ValueId(0),
                        },
                    ),
                    store(2, 10),
                    store(3, 11),
                    store(3, 12),
                ],
                branch(2),
            ),
            block(
                4,
                vec![Instruction::effect_free(
                    ValueDef::new(ValueId(13), scalar),
                    OperationKind::Load {
                        pointer: ValueId(2),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    },
                )],
                Terminator::Return {
                    values: vec![ValueId(13)],
                },
            ),
        ],
    ));
    module
}

fn with_chain(consume: impl FnOnce(&Prefix<'_, '_, '_, '_>, &Licm<'_>, &Inventory<'_>, usize)) {
    with_chain_module(&fixture(), consume);
}

fn with_chain_module(
    module: &Module,
    consume: impl FnOnce(&Prefix<'_, '_, '_, '_>, &Licm<'_>, &Inventory<'_>, usize),
) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, retained) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    let optimized =
        fe2o3_pliron::optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&owner, LAYOUTS, &mut budget)
            .unwrap();
    budget
        .reserve_storage(optimized.storage().retained_storage())
        .unwrap();
    assert_eq!(optimized.execution().policy_version(), 11);
    let (original, retained) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    let (prefix, retained) = Inventory::derive_v18(optimized.owner(), &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    assert!(original.operations().len() > prefix.operations().len());
    let (checked, retained) = check_canonical_kir_transition_v18(
        &original,
        &prefix,
        optimized.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    let licm =
        fe2o3_kernel_opt::prepare_owned_licm_v18(optimized.owner(), LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(licm.retained_storage()).unwrap();
    let (pair, retained) = licm.replay_against(optimized.owner(), &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    assert!(pair.origins().iter().any(|row| row.hoist.is_some()));
    let (output, retained) = Inventory::derive_v18(pair.output(), &mut budget).unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    consume(&checked, &pair, &output, budget.storage());
}

#[test]
fn typed_licm_bridge_uses_actual_operations_and_preserves_both_checked_results() {
    use fe2o3_kernel_ir::{CheckedBinaryOperator, FormalIndexWidth};
    let mut module = fixture();
    let body = &mut module.functions[0].body.as_mut().unwrap().blocks[3];
    body.operations.insert(
        0,
        Instruction::new(
            vec![
                ValueDef::new(ValueId(14), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(15), Type::BOOL),
            ],
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ),
    );
    body.operations.push(Instruction::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(2),
            value: ValueId(14),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    ));
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(15),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    with_chain_module(&module, |prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        let generate = |out: &mut Writer<'_, '_>| -> Result<()> {
            let limits = fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                max_boundaries: 4096,
            };
            let (input_physical, storage) =
                fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                    prefix.output(),
                    limits,
                    out.budget,
                )?;
            out.budget.reserve_storage(storage.retained_storage())?;
            let (output_physical, storage) =
                fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                    output, limits, out.budget,
                )?;
            out.budget.reserve_storage(storage.retained_storage())?;
            let input_contracts = target_view_contracts_v38::TargetByteViewContractsV38::derive(
                prefix.output(),
                FormalIndexWidth::Bits64,
                out,
            )?;
            let output_contracts = target_view_contracts_v38::TargetByteViewContractsV38::derive(
                output,
                FormalIndexWidth::Bits64,
                out,
            )?;
            let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
            bridge.emit_typed_licm_v48(
                &input_physical,
                &output_physical,
                &input_contracts,
                &output_contracts,
                FormalIndexWidth::Bits64,
                1,
                out,
            )
        };
        let measured = run(floor, LIMIT, LIMIT, generate);
        let text = measured.0.unwrap();
        let mut checked = 0;
        for (ordinal, row) in prefix.output().operations().iter().enumerate() {
            if matches!(
                row.operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    ..
                }
            ) {
                assert!(licm.origins()[ordinal].hoist.is_some());
                assert_eq!(row.results.len(), 2);
                for result in row.results.clone() {
                    assert!(text.contains(&format!("evaluated.state.values[{result}]")));
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 1);
        assert!(text.contains("let evaluated = byte_operation_0_"));
        assert!(text.contains("evaluated.state.memory == probe.memory"));
        assert!(text.contains("typed_allocation_environment_1_v48(before) == typed_allocation_environment_2_v48(after)"));
        assert!(
            text.contains("returned: if result.state.valid { result.returned } else { seq![] }")
        );
        assert!(text.contains("else if result.state.pc == -2 { -2 }"));
        assert!(text.contains("typed_licm_input_defined_0_v48(before, little_endian, fuel)"));
        assert!(!text.contains("Seq<int>"));
        assert!(!text.contains("spec_fn("));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
        let exact = run(floor, measured.1, measured.2, generate);
        assert_eq!(exact.0.unwrap(), text);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        for work in [true, false] {
            let work_limit = measured.1 - usize::from(work);
            let storage_limit = measured.2 - usize::from(!work);
            match run(floor, work_limit, storage_limit, generate)
                .0
                .err()
                .unwrap()
            {
                Error::Resource(Resource::Work(error)) if work => {
                    assert_eq!(error.limit(), work_limit);
                    assert_eq!(error.actual(), measured.1);
                }
                Error::Resource(Resource::Storage(error)) if !work => {
                    assert_eq!(error.limit(), storage_limit);
                    assert_eq!(error.actual(), measured.2);
                }
                other => panic!("typed LICM exact resource boundary: {other:?}"),
            }
        }
    });
}

// Test-only canonical origin resolver. No original source allocation or spill
// authority is constructed by this fixture.
struct Origins<'a, 'owner> {
    inventory: &'a Inventory<'owner>,
    duplicate: bool,
}

impl ByteAllocationResolverV30 for Origins<'_, '_> {
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        if !std::ptr::eq(owner, self.inventory.owner()) {
            return Err(mismatch());
        }
        out.budget.charge_work(1)?;
        Ok(())
    }
    fn site(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<ByteAllocationSiteV30> {
        self.check_owner(self.inventory.owner(), out)?;
        out.budget
            .charge_work(self.inventory.operations().len() + 1)?;
        let first = self
            .inventory
            .operations()
            .iter()
            .find(|row| matches!(row.operation.kind, OperationKind::Alloca { .. }))
            .unwrap();
        if !matches!(
            self.inventory.operations()[operation_index(self.inventory, operation)?]
                .operation
                .kind,
            OperationKind::Alloca { .. }
        ) {
            return Err(mismatch());
        }
        Ok(ByteAllocationSiteV30 {
            original: if self.duplicate {
                first.coordinate
            } else {
                operation
            },
            physical_root_owner: 0,
        })
    }
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
        budget.reserve_storage(floor + crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)?;
        let mut out = Writer::new(&mut budget)?;
        body(&mut out)?;
        out.finish()
    })();
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn typed_allocation_bridge_preserves_all_three_exact_register_environments() {
    with_chain(|prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        let text =
            run(floor, LIMIT, LIMIT, |out| {
                let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
                assert_eq!(bridge.rows.iter().flatten().count(), 2);
                for side in [
                    AllocationSideV48::Original,
                    AllocationSideV48::Prefix,
                    AllocationSideV48::Relocated,
                ] {
                    let view = bridge.view(side, out)?;
                    view.check_owner(bridge.inventory(side).owner(), out)?;
                    for operation in bridge.inventory(side).operations() {
                        if matches!(operation.operation.kind, OperationKind::Alloca { .. }) {
                            let site = view.site(operation.coordinate, out)?;
                            assert_eq!(site.physical_root_owner, 0);
                            assert!(
                                prefix.input().operations().iter().any(|row| row.coordinate
                                    == site.original
                                    && matches!(row.operation.kind, OperationKind::Alloca { .. }))
                            );
                        }
                    }
                }
                bridge.emit_transport([71, 72, 73], out)?;
                for row in bridge.rows.iter().flatten() {
                    for definition in row.definitions {
                        assert!(out.text.contains(&format!("s.values[{definition}]")));
                    }
                }
                Ok(())
            })
            .0
            .unwrap();
        assert!(text.contains("typed_original_allocation_registers_71_v48"));
        assert!(text.contains("typed_original_map_view_exact_73_v48"));
        assert!(text.contains("before == after"));
        assert!(text.contains("MemoryOperationEffectV30::TagRead { .. } => false"));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
        assert!(!text.contains("Seq<int>"));
    });
}

#[test]
fn typed_allocation_bridge_refuses_duplicate_sites_wrong_side_and_stale_custody() {
    with_chain(|prefix, licm, output, floor| {
        let duplicate = Origins {
            inventory: prefix.input(),
            duplicate: true,
        };
        assert!(matches!(
            run(floor, LIMIT, LIMIT, |out| {
                AllocationBridgeV48::derive(prefix, licm, output, &duplicate, out).map(|_| ())
            })
            .0,
            Err(Error::Statement(_))
        ));
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        for foreign in [false, true] {
            run(floor, LIMIT, LIMIT, |out| {
                let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
                let view = bridge.view(AllocationSideV48::Original, out)?;
                assert!(matches!(
                    view.check_owner(prefix.output().owner(), out),
                    Err(Error::Statement(_))
                ));
                view.check_owner(prefix.input().owner(), out)?;
                assert!(bridge.emit_transport([1, 1, 2], out).is_err());
                if foreign {
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, LIMIT);
                    budget.reserve_storage(out.budget.storage())?;
                    let mut other = Writer::new(&mut budget)?;
                    let before = (other.budget.work(), other.budget.storage());
                    assert!(matches!(
                        bridge.view(AllocationSideV48::Original, &mut other),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    assert_eq!((other.budget.work(), other.budget.storage()), before);
                } else {
                    out.budget
                        .release_storage(out.budget.storage() - bridge.required + 1)?;
                    assert!(matches!(
                        bridge.view(AllocationSideV48::Original, out),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    out.budget.reserve_storage(1)?;
                }
                let before = (out.budget.work(), out.budget.storage());
                assert!(matches!(
                    bridge.view(AllocationSideV48::Relocated, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!((out.budget.work(), out.budget.storage()), before);
                Ok(())
            })
            .0
            .unwrap();
        }
    });
}

#[test]
fn typed_allocation_bridge_complete_emission_has_exact_and_one_short_resources() {
    with_chain(|prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        let generate = |out: &mut Writer<'_, '_>| {
            AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?
                .emit_transport([41, 42, 43], out)
        };
        let measured = run(floor, LIMIT, LIMIT, generate);
        let expected = measured.0.unwrap();
        let exact = run(floor, measured.1, measured.2, generate);
        assert_eq!(exact.0.unwrap(), expected);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        for work in [true, false] {
            let limit_work = measured.1 - usize::from(work);
            let limit_storage = measured.2 - usize::from(!work);
            match run(floor, limit_work, limit_storage, generate)
                .0
                .err()
                .unwrap()
            {
                Error::Resource(Resource::Work(error)) if work => {
                    assert_eq!(error.limit(), limit_work);
                    assert_eq!(error.actual(), measured.1);
                }
                Error::Resource(Resource::Storage(error)) if !work => {
                    assert_eq!(error.limit(), limit_storage);
                    assert_eq!(error.actual(), measured.2);
                }
                other => panic!("wrong exact resource boundary: {other:?}"),
            }
        }
    });
}
