use super::*;
use fe2o3_kernel_ir::{
    BasicBlock as KirBlock, CanonicalKernelIrWorkBudgetV1 as Work, ExecutionOperationV15 as E,
    ExecutionRoleV15 as R, Function, MemoryAccess, Signature, StorageFieldV1, StorageLayoutIdV1,
    StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1 as S, StorageProjectionV1 as P,
    ValueDef,
};

pub(super) const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};
pub(super) const AMPLE: usize = 1_000_000_000;
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PanicAt {
    None,
    Import,
    Extract,
}
std::thread_local! {
    pub(super) static PANIC: std::cell::Cell<PanicAt> = const { std::cell::Cell::new(PanicAt::None) };
    static PAYLOAD_PANICS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    pub(super) static CLEANUP: std::cell::Cell<CleanupObservation> = const {
        std::cell::Cell::new(CleanupObservation {
            started: false, storage: 0, poisoned: None, drops: 0,
        })
    };
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CleanupObservation {
    pub(super) started: bool,
    pub(super) storage: usize,
    pub(super) poisoned: Option<bool>,
    pub(super) drops: usize,
}
pub(super) fn cleanup_start(storage: usize, poisoned: Option<bool>) {
    CLEANUP.with(|slot| {
        slot.set(CleanupObservation {
            started: true,
            storage,
            poisoned,
            drops: 0,
        })
    });
}
pub(super) fn nested_panic_at(point: PanicAt, panics: usize) {
    CLEANUP.with(|slot| slot.set(CleanupObservation::default()));
    PAYLOAD_PANICS.with(|slot| slot.set(Some(panics)));
    PANIC.with(|slot| slot.set(point));
}
pub(super) fn clear_panic() {
    PANIC.with(|slot| slot.set(PanicAt::None));
    PAYLOAD_PANICS.with(|slot| slot.set(None));
}
struct NestedPayload(usize);
impl Drop for NestedPayload {
    fn drop(&mut self) {
        // The snapshot is taken immediately before cleanup. The helper has no
        // ledger access or refund path between that snapshot and these drops.
        CLEANUP.with(|slot| {
            let mut observed = slot.get();
            observed.drops += 1;
            slot.set(observed);
        });
        if self.0 != 0 {
            std::panic::panic_any(NestedPayload(self.0 - 1));
        }
    }
}
pub(super) fn panic_at(point: PanicAt) {
    PANIC.with(|slot| {
        if slot.get() == point {
            slot.set(PanicAt::None);
            if let Some(panics) = PAYLOAD_PANICS.with(|slot| slot.take()) {
                std::panic::panic_any(NestedPayload(panics));
            }
            panic!("injected V18 bridge unwind");
        }
    });
}
pub(super) fn scalar() -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    }
}
pub(super) fn owner(source: &Module) -> VerifiedCanonicalKernelIrModuleV18 {
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        source,
        LIMITS,
        &mut budget,
    )
    .unwrap()
    .0
}
fn address(row: u32) -> Type {
    Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(row)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}
pub(super) fn fixture() -> Module {
    let mut source = Module::new("storage-bridge");
    source.storage_layouts = vec![
        scalar(),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                    StorageFieldV1 {
                        offset: 8,
                        layout: StorageLayoutIdV1(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ];
    let memory = MemoryAccess::new(AddressSpace::Private, 8);
    let mut block = KirBlock::new(BlockId(0));
    block.operations = vec![
        KirOperation::new(
            vec![ValueDef::new(ValueId(0), address(1))],
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U64))],
            OperationKind::Constant(Constant::U64(11)),
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(2), address(0))],
            OperationKind::Storage(S::Project {
                base: ValueId(0),
                step: P::Field(1),
            }),
        ),
        KirOperation::new(
            vec![],
            OperationKind::Storage(S::WriteValue {
                address: ValueId(2),
                value: ValueId(1),
                access: memory,
            }),
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U64))],
            OperationKind::Storage(S::ReadValue {
                address: ValueId(2),
                access: memory,
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    source.functions.push(Function::definition(
        "f",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U64)]),
        vec![],
        vec![block],
    ));
    source
}
pub(super) fn live_value(graph: &KirPlironGraphV18<'_>, id: u32) -> Value {
    *graph
        .origins
        .values
        .iter()
        .find(|(_, source)| **source == ValueId(id))
        .unwrap()
        .0
}
fn commit(
    graph: &mut KirPlironGraphV18<'_>,
    transaction: crate::graph_analysis_v1::CheckedOperationGraphMutationV1,
) {
    graph
        .session
        .commit_checked_operation_graph_mutation_v1(transaction, true)
        .unwrap();
    graph.epoch = graph.session.operation_graph_epochs[&graph.root.identity];
}

#[test]
fn storage_v18_real_private_graph_roundtrip_retains_table_roles_and_coordinates() {
    let input = owner(&fixture());
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    budget.reserve_storage(19).unwrap();
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), 19);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    let (output, report, retained) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert_eq!(
        output.module().storage_layouts,
        input.module().storage_layouts
    );
    assert_eq!(
        output.module().functions[0].role,
        input.module().functions[0].role
    );
    assert_eq!(report.correspondence, graph.correspondence);
    assert_eq!(report.input, report.output);
    assert_eq!(report.table, graph.table_identity());
    assert!(retained.retained_storage() > 0);
    assert_eq!(budget.storage(), floor);
    assert!(std::ptr::eq(
        output.verified_storage_module_ref_v1().module(),
        output.module()
    ));
}

#[test]
fn storage_v18_ordinary_live_mutation_changes_module_not_table_identity() {
    let input = owner(&fixture());
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let transaction = graph
        .session
        .begin_checked_operation_graph_mutation_v1(&graph.root)
        .unwrap();
    let pointer = graph
        .origins
        .values
        .iter()
        .find_map(|(live, source)| {
            (*source == ValueId(1))
                .then(|| live.defining_op())
                .flatten()
        })
        .unwrap();
    let constant = Operation::get_op::<PlironConstantOp>(pointer, &graph.session.context).unwrap();
    constant.set_attr_gpu_constant_value(
        &graph.session.context,
        constant_to_pliron(&graph.session.context, &Constant::U64(23)).unwrap(),
    );
    commit(&mut graph, transaction);
    let (output, report, _) = graph.extract_canonical_v18(LIMITS, &mut budget).unwrap();
    assert_ne!(output.identity(), input.identity());
    assert_eq!(report.table, graph.table_identity());
    assert_eq!(
        output.module().storage_layouts,
        input.module().storage_layouts
    );
    assert_eq!(
        output.module().functions[0].body.as_ref().unwrap().blocks[0].operations[1].kind,
        OperationKind::Constant(Constant::U64(23))
    );
    assert!(matches!(
        graph.extract_canonical_v18_o0(LIMITS, &mut budget),
        Err(KirBridgeErrorV18::Bridge(
            KirBridgeErrorV1::NonExactRoundTrip
        ))
    ));
}

#[test]
fn storage_v18_live_storage_operand_remapping_is_not_source_replay() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            2,
            KirOperation::new(
                vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U64))],
                OperationKind::Constant(Constant::U64(31)),
            ),
        );
    let input = owner(&module);
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let target = *graph
        .origins
        .preserved_operations
        .iter()
        .find(|(_, kind)| matches!(kind, OperationKind::Storage(S::WriteValue { .. })))
        .unwrap()
        .0;
    let replacement = live_value(&graph, 4);
    let transaction = graph
        .session
        .begin_checked_operation_graph_mutation_v1(&graph.root)
        .unwrap();
    Operation::replace_operand(target, &graph.session.context, 1, replacement);
    commit(&mut graph, transaction);
    let (output, _, _) = graph.extract_canonical_v18(LIMITS, &mut budget).unwrap();
    assert!(
        output.module().functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .iter()
            .any(|op| matches!(
                op.kind,
                OperationKind::Storage(S::WriteValue {
                    value: ValueId(4),
                    ..
                })
            ))
    );
}

#[test]
fn storage_v18_same_table_type_uniquing_never_substitutes_owner_custody() {
    let left = owner(&fixture());
    let mut changed = fixture();
    changed.id = fe2o3_kernel_ir::ModuleId::new("other");
    let right = owner(&changed);
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let a = storage_v18::ProfileV18::new(&left, &mut budget).unwrap();
    let b = storage_v18::ProfileV18::new(&right, &mut budget).unwrap();
    assert_eq!(a.key(), b.key());
    assert!(a.validate_module(right.module()).is_err());
    let context = Context::new();
    let ty = a.to_pliron(&context, &address(0)).unwrap();
    assert_eq!(ty, b.to_pliron(&context, &address(0)).unwrap());
    assert_eq!(b.decode_type(&context, ty, 0).unwrap(), address(0));
    let mut incompatible = fixture();
    incompatible.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I64);
    incompatible.functions.clear();
    let incompatible = owner(&incompatible);
    let c = storage_v18::ProfileV18::new(&incompatible, &mut budget).unwrap();
    assert!(c.decode_type(&context, ty, 0).is_err());
    let hostile = dialect_gpu::storage_types_v18::StorageObjectTypeV18::get(
        &context,
        dialect_gpu::storage_types_v18::StorageTableKeyAttrV18::new(
            *a.key().digest(),
            a.key().encoded_length(),
        ),
        dialect_gpu::storage_types_v18::StorageOrdinalAttrV18(u32::MAX),
    );
    assert!(a.decode_type(&context, hostile.into(), 0).is_err());
}

#[test]
fn storage_v18_foreign_ledger_missing_reservation_and_epoch_are_refused_without_cleanup() {
    let input = owner(&fixture());
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    budget.reserve_storage(17).unwrap();
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    assert!(graph.extract_canonical_v18(LIMITS, &mut budget).is_err());
    assert_eq!(budget.storage(), 17);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let mut foreign_work = Work::new(AMPLE);
    let mut foreign = Budget::new(&mut foreign_work, AMPLE);
    foreign.reserve_storage(budget.storage()).unwrap();
    let before = foreign.storage();
    assert!(graph.extract_canonical_v18(LIMITS, &mut foreign).is_err());
    assert_eq!(foreign.storage(), before);
    assert_eq!(foreign.work(), 0);
    graph.epoch = graph.epoch.checked_next().unwrap();
    let before = budget.storage();
    assert!(graph.extract_canonical_v18(LIMITS, &mut budget).is_err());
    assert_eq!(budget.storage(), before);
}

#[test]
fn storage_v18_fixed_descriptor_missing_origin_and_foreign_coordinate_are_refused() {
    use dialect_gpu::storage_operations_v18::StorageOpV18;
    for case in 0..4 {
        let input = owner(&fixture());
        let mut work = Work::new(AMPLE);
        let mut budget = Budget::new(&mut work, AMPLE);
        let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let target = *graph
            .origins
            .preserved_operations
            .iter()
            .find(|(_, kind)| matches!(kind, OperationKind::Storage(S::Project { .. })))
            .unwrap()
            .0;
        if case == 1 {
            graph.origins.preserved_operations.remove(&target);
        } else if case == 2 {
            graph.coordinates.insert(
                target,
                KirBridgeCoordinateV1::Operation {
                    function: 0,
                    block: 0,
                    operation: 1,
                },
            );
        } else if case == 3 {
            let pointer = live_value(&graph, 1).defining_op().unwrap();
            let constant =
                Operation::get_op::<PlironConstantOp>(pointer, &graph.session.context).unwrap();
            constant.set_attr_gpu_constant_value(
                &graph.session.context,
                constant_to_pliron(&graph.session.context, &Constant::U64(99)).unwrap(),
            );
        } else {
            let transaction = graph
                .session
                .begin_checked_operation_graph_mutation_v1(&graph.root)
                .unwrap();
            let operation =
                Operation::get_op::<StorageOpV18>(target, &graph.session.context).unwrap();
            operation.set_attr_gpu_storage_selector_v18(
                &graph.session.context,
                dialect_gpu::storage_types_v18::StorageOrdinalAttrV18(0),
            );
            commit(&mut graph, transaction);
        }
        assert!(graph.extract_canonical_v18(LIMITS, &mut budget).is_err());
        if case == 3 {
            assert!(graph.session.poisoned);
        }
    }
}

#[test]
fn storage_v18_execution_roles_roundtrip_through_actual_graph_and_legacy_stays_closed() {
    let mut module = Module::new("execution");
    module.storage_layouts.push(scalar());
    let mut block = KirBlock::new(BlockId(0));
    block.operations = vec![
        KirOperation::new(
            vec![ValueDef::new(ValueId(0), Type::Execution(R::Context))],
            OperationKind::Execution(E::ContextIssue),
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(1), Type::Execution(R::Workgroup))],
            OperationKind::Execution(E::WorkgroupDerive {
                context: ValueId(0),
            }),
        ),
        KirOperation::new(
            vec![],
            OperationKind::Execution(E::ScopeEnd {
                workgroup: ValueId(1),
                discarded: vec![],
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(fe2o3_kernel_ir::Kernel::new(
        "entry",
        "entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Static(64),
        },
    ));
    let input = owner(&module);
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (output, _, _) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    for profile in [
        KirBridgeTypeProfileV12::Legacy,
        KirBridgeTypeProfileV12::V12,
    ] {
        assert!(
            profile
                .preflight_type(&Type::Execution(R::Context))
                .is_err()
        );
        assert!(profile.preflight_type(&address(0)).is_err());
        assert!(preflight_with_profile_v12(input.module(), profile).is_err());
    }
}

#[test]
fn storage_v18_execution_and_ordered_remappers_preserve_fixed_fields_and_live_positions() {
    for execution in [
        E::ContextIssue,
        E::WorkgroupDerive {
            context: ValueId(8),
        },
        E::ScopeEnd {
            workgroup: ValueId(8),
            discarded: vec![ValueId(9), ValueId(10)],
        },
        E::MaskedTileLoadU32 {
            workgroup: ValueId(8),
            input: ValueId(9),
            base: ValueId(10),
            lanes: 3,
            elements: 2,
        },
        E::TileIntoFragmentU32 {
            tile: ValueId(8),
            lanes: 3,
            elements: 2,
        },
        E::FragmentIntoPartsU32 {
            fragment: ValueId(8),
            lanes: 3,
            elements: 2,
        },
    ] {
        let kind = OperationKind::Execution(execution);
        let expected: Vec<_> = (0..kind.operands().len())
            .map(|i| ValueId(40 + i as u32))
            .collect();
        let mapped = storage_v18::remap(&kind, expected.clone()).unwrap();
        assert_eq!(mapped.operands(), expected);
        assert_eq!(
            storage_v18::preserved_kind(&mapped).unwrap(),
            PreservedOperationKindAttr::ExecutionV18
        );
        assert!(preserved_operation_kind(&mapped).is_err());
    }
    let invalid = OperationKind::Execution(E::ScopeEnd {
        workgroup: ValueId(0),
        discarded: vec![ValueId(1), ValueId(2)],
    });
    assert!(storage_v18::remap(&invalid, vec![ValueId(9), ValueId(3), ValueId(2)]).is_err());
}

#[test]
fn storage_v18_ordered_regions_and_programs_use_actual_live_operands() {
    use fe2o3_kernel_ir::*;
    let identity = AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]);
    let mut words = [0; 16];
    words[..3].copy_from_slice(&[0x85, 0x133, 0x19d]);
    let program = Gfx942U32ProgramV1::from_descriptors(3, words).unwrap();
    let inputs = [ValueId(0), ValueId(1), ValueId(2)];
    let kinds = [
        OperationKind::Gfx942OrderedRegion(
            Gfx942OrderedRegionV1::new(
                identity,
                Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
                inputs,
            )
            .unwrap(),
        ),
        OperationKind::Gfx942OrderedProgram(
            Gfx942OrderedProgramV1::new(
                identity,
                Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
                inputs,
                program,
            )
            .unwrap(),
        ),
    ];
    for kind in kinds {
        let op = KirOperation::new(
            vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))],
            kind.clone(),
        );
        let capabilities = op.required_capabilities();
        let mut block = KirBlock::new(BlockId(0));
        block.operations.push(op);
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(4)],
        });
        let mut function = Function::internal_helper(
            "ordered",
            Signature::new(
                vec![Type::Scalar(ScalarType::U32); 4],
                vec![Type::Scalar(ScalarType::U32)],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            vec![block],
        );
        function.required_capabilities = capabilities.clone();
        let mut module = Module::new("ordered");
        module.storage_layouts.push(scalar());
        module.required_capabilities = capabilities;
        module.functions.push(function);
        let input = owner(&module);
        let mut work = Work::new(AMPLE);
        let mut budget = Budget::new(&mut work, AMPLE);
        let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (same, _, _) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
        assert_eq!(same.canonical_bytes(), input.canonical_bytes());
        let target = *graph
            .origins
            .preserved_operations
            .iter()
            .find(|(_, original)| **original == kind)
            .unwrap()
            .0;
        let replacement = live_value(&graph, 3);
        let transaction = graph
            .session
            .begin_checked_operation_graph_mutation_v1(&graph.root)
            .unwrap();
        pliron::operation::Operation::replace_operand(
            target,
            &graph.session.context,
            1,
            replacement,
        );
        commit(&mut graph, transaction);
        let (output, report, _) = graph.extract_canonical_v18(LIMITS, &mut budget).unwrap();
        let actual =
            &output.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind;
        assert_eq!(actual.operands(), vec![ValueId(0), ValueId(3), ValueId(2)]);
        match (&kind, actual) {
            (OperationKind::Gfx942OrderedRegion(a), OperationKind::Gfx942OrderedRegion(b)) => {
                assert_eq!(a.source(), b.source());
                assert_eq!(a.registers(), b.registers());
            }
            (OperationKind::Gfx942OrderedProgram(a), OperationKind::Gfx942OrderedProgram(b)) => {
                assert_eq!(a.source(), b.source());
                assert_eq!(a.registers(), b.registers());
                assert_eq!(a.program(), b.program());
            }
            _ => panic!("ordered operation family changed"),
        }
        assert_ne!(report.input, report.output);
        assert_eq!(report.table, graph.table_identity());
    }
}

#[test]
fn storage_v18_execution_tile_base_is_remapped_from_the_live_operand() {
    use fe2o3_kernel_ir::{Kernel, LaunchDomain, LaunchExtent};
    let mut block = KirBlock::new(BlockId(0));
    block.operations = vec![
        KirOperation::new(
            vec![ValueDef::new(ValueId(10), Type::Execution(R::Context))],
            OperationKind::Execution(E::ContextIssue),
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(11), Type::Execution(R::Workgroup))],
            OperationKind::Execution(E::WorkgroupDerive {
                context: ValueId(10),
            }),
        ),
        KirOperation::new(
            vec![ValueDef::new(
                ValueId(12),
                Type::Execution(R::MaskedTileU32 {
                    lanes: 3,
                    elements: 2,
                }),
            )],
            OperationKind::Execution(E::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes: 3,
                elements: 2,
            }),
        ),
        KirOperation::new(
            vec![],
            OperationKind::Execution(E::ScopeEnd {
                workgroup: ValueId(11),
                discarded: vec![ValueId(12)],
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("execution-remap");
    module.storage_layouts.push(scalar());
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    let input = owner(&module);
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let target = *graph
        .origins
        .preserved_operations
        .iter()
        .find(|(_, kind)| matches!(kind, OperationKind::Execution(E::MaskedTileLoadU32 { .. })))
        .unwrap()
        .0;
    let replacement = live_value(&graph, 2);
    let transaction = graph
        .session
        .begin_checked_operation_graph_mutation_v1(&graph.root)
        .unwrap();
    Operation::replace_operand(target, &graph.session.context, 2, replacement);
    commit(&mut graph, transaction);
    let (output, report, _) = graph.extract_canonical_v18(LIMITS, &mut budget).unwrap();
    assert!(matches!(
        output.module().functions[0].body.as_ref().unwrap().blocks[0].operations[2].kind,
        OperationKind::Execution(E::MaskedTileLoadU32 {
            base: ValueId(2),
            lanes: 3,
            elements: 2,
            ..
        })
    ));
    assert_ne!(report.input, report.output);
}

#[test]
fn storage_v18_direct_and_niche_construction_remain_distinct_from_active_reads() {
    use fe2o3_kernel_ir::{StorageVariantEncodingV1 as Encoding, StorageVariantV1 as Variant};
    for niche in [false, true] {
        for selected in [0, 1] {
            let mut module = Module::new("enum");
            let field = StorageFieldV1 {
                offset: 0,
                layout: StorageLayoutIdV1(0),
            };
            module.storage_layouts = vec![
                scalar(),
                StorageLayoutV1 {
                    size: 8,
                    alignment: 8,
                    kind: StorageLayoutKindV1::Record(vec![field].into_boxed_slice()),
                },
                StorageLayoutV1 {
                    size: if niche { 8 } else { 16 },
                    alignment: 8,
                    kind: StorageLayoutKindV1::Variants {
                        encoding: if niche {
                            Encoding::Niche {
                                tag: field,
                                untagged_variant: 0,
                                first_niche_variant: 1,
                                last_niche_variant: 1,
                                niche_start: u64::MAX as u128,
                            }
                        } else {
                            Encoding::Direct {
                                tag: StorageFieldV1 { offset: 8, ..field },
                            }
                        },
                        variants: [3_u128, 9]
                            .into_iter()
                            .enumerate()
                            .map(|(i, bits)| Variant {
                                discriminant: if niche { i as u128 } else { 91 + i as u128 },
                                direct_tag_bits: (!niche).then_some(bits),
                                uninhabited: false,
                                layout: StorageLayoutIdV1(1),
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    },
                },
            ];
            let wo = Type::pointer(
                Type::StorageObject(StorageLayoutIdV1(1)),
                AddressSpace::Private,
                AccessMode::WriteOnly,
            );
            let memory = MemoryAccess::new(AddressSpace::Private, 8);
            let mut block = KirBlock::new(BlockId(0));
            block.operations = vec![
                KirOperation::new(
                    vec![ValueDef::new(ValueId(0), address(2))],
                    OperationKind::Alloca {
                        element: Type::StorageObject(StorageLayoutIdV1(2)),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 8,
                    },
                ),
                KirOperation::new(
                    vec![ValueDef::new(ValueId(1), wo)],
                    OperationKind::Storage(S::Project {
                        base: ValueId(0),
                        step: P::VariantForWrite { index: selected },
                    }),
                ),
                KirOperation::new(
                    vec![],
                    OperationKind::Storage(S::SetDiscriminant {
                        address: ValueId(0),
                        variant: selected,
                        access: memory,
                    }),
                ),
                KirOperation::new(
                    vec![ValueDef::new(ValueId(2), address(1))],
                    OperationKind::Storage(S::Project {
                        base: ValueId(0),
                        step: P::Variant {
                            index: selected,
                            access: memory,
                        },
                    }),
                ),
            ];
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::internal_helper(
                "enum",
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
            let input = owner(&module);
            let mut work = Work::new(AMPLE);
            let mut budget = Budget::new(&mut work, AMPLE);
            let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let (output, _, _) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
            assert_eq!(output.canonical_bytes(), input.canonical_bytes());
            // This transports structural state, not initialized runtime bytes.
            assert_eq!(output.module().storage_layouts, module.storage_layouts);
        }
    }
}

fn discriminant_fixture(niche: bool) -> Module {
    use fe2o3_kernel_ir::{StorageVariantEncodingV1 as Encoding, StorageVariantV1 as Variant};
    let mut module = Module::new("logical-discriminant");
    let field = StorageFieldV1 { offset: 0, layout: StorageLayoutIdV1(0) };
    module.storage_layouts = vec![scalar(), StorageLayoutV1 {
        size: 8, alignment: 8, kind: StorageLayoutKindV1::Record(vec![field].into_boxed_slice()),
    }, StorageLayoutV1 {
        size: 16, alignment: 8, kind: StorageLayoutKindV1::Variants {
            encoding: if niche { Encoding::Niche { tag: field, untagged_variant: 0,
                first_niche_variant: 1, last_niche_variant: 2, niche_start: u64::MAX as u128 } }
                else { Encoding::Direct { tag: StorageFieldV1 { offset: 8, ..field } } },
            variants: [255, 1_u128 << 100, u128::MAX].into_iter().enumerate().map(|(i, discriminant)| Variant {
                discriminant: if niche { i as u128 } else { discriminant },
                direct_tag_bits: (!niche).then_some([3, 17, 250][i]), uninhabited: false,
                layout: StorageLayoutIdV1(1),
            }).collect::<Vec<_>>().into_boxed_slice(),
        },
    }];
    let mut block = KirBlock::new(BlockId(0));
    block.operations.push(KirOperation::new(vec![ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U128))],
        OperationKind::Storage(S::ReadDiscriminant { address: ValueId(0), access: MemoryAccess::new(AddressSpace::Private, 8) })));
    block.terminator = Some(Terminator::Return { values: vec![ValueId(2)] });
    module.functions.push(Function::internal_helper("read", Signature::new(vec![address(2), address(2)],
        vec![Type::Scalar(ScalarType::U128)]), vec![ValueId(0), ValueId(1)], vec![block]));
    module
}

#[test]
fn storage_v18_discriminant_roundtrip_preserves_the_actual_logical_table() {
    for niche in [false, true] {
        let input = owner(&discriminant_fixture(niche));
        let mut work = Work::new(AMPLE);
        let mut budget = Budget::new(&mut work, AMPLE);
        let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        let (output, _, _) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
        assert_eq!(output.canonical_bytes(), input.canonical_bytes());
        assert_eq!(output.module().storage_layouts, input.module().storage_layouts);
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn storage_v18_discriminant_extraction_uses_actual_live_pointer_not_original_replay() {
    let input = owner(&discriminant_fixture(false));
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let target = *graph.origins.preserved_operations.iter()
        .find(|(_, kind)| matches!(kind, OperationKind::Storage(S::ReadDiscriminant { .. }))).unwrap().0;
    let replacement = live_value(&graph, 1);
    let transaction = graph.session.begin_checked_operation_graph_mutation_v1(&graph.root).unwrap();
    Operation::replace_operand(target, &graph.session.context, 0, replacement);
    commit(&mut graph, transaction);
    let (output, _, _) = graph.extract_canonical_v18(LIMITS, &mut budget).unwrap();
    assert!(matches!(output.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind,
        OperationKind::Storage(S::ReadDiscriminant { address: ValueId(1), .. })));
    assert_ne!(output.canonical_bytes(), input.canonical_bytes());
}

#[test]
fn storage_v18_array_projection_and_whole_object_copy_roundtrip_without_component_cells() {
    let mut module = Module::new("array-copy");
    module.storage_layouts = vec![
        scalar(),
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 2,
                stride: 8,
            },
        },
    ];
    let mut block = KirBlock::new(BlockId(0));
    for id in [0, 1] {
        block.operations.push(KirOperation::new(
            vec![ValueDef::new(ValueId(id), address(1))],
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ));
    }
    block.operations.extend([
        KirOperation::new(
            vec![ValueDef::new(ValueId(2), Type::INDEX)],
            OperationKind::Constant(Constant::Index(1)),
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(3), address(0))],
            OperationKind::Storage(S::Project {
                base: ValueId(0),
                step: P::ArrayIndex(ValueId(2)),
            }),
        ),
        KirOperation::new(
            vec![],
            OperationKind::Storage(S::CopyObject {
                source: ValueId(0),
                destination: ValueId(1),
                source_access: MemoryAccess::new(AddressSpace::Private, 8),
                destination_access: MemoryAccess::new(AddressSpace::Private, 8),
                overlap: fe2o3_kernel_ir::StorageCopyOverlapV1::MayOverlap,
            }),
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::internal_helper(
        "copy",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let input = owner(&module);
    let mut work = Work::new(AMPLE);
    let mut budget = Budget::new(&mut work, AMPLE);
    let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (output, _, _) = graph.extract_canonical_v18_o0(LIMITS, &mut budget).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert_eq!(
        output.module().storage_layouts,
        input.module().storage_layouts
    );
    let operations = &output.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert_eq!(operations.len(), 5);
    assert!(matches!(
        operations[4].kind,
        OperationKind::Storage(S::CopyObject {
            source: ValueId(0),
            destination: ValueId(1),
            ..
        })
    ));
}

#[test]
fn storage_v18_facades_contain_four_finite_payload_attempts_and_poison_before_drop() {
    for panics in 0..=3 {
        let input = owner(&fixture());
        let mut work = Work::new(AMPLE);
        let mut budget = Budget::new(&mut work, AMPLE);
        budget.reserve_storage(19).unwrap();
        nested_panic_at(PanicAt::Import, panics);
        assert!(matches!(
            KirPlironGraphV18::import(&input, &mut budget),
            Err(KirBridgeErrorV18::Bridge(
                KirBridgeErrorV1::UpstreamPanicked
            ))
        ));
        let observed = CLEANUP.with(|slot| slot.get());
        assert!(observed.started);
        assert_eq!(observed.drops, panics + 1);
        assert_eq!(observed.poisoned, None);
        assert!(observed.storage > 19);
        assert_eq!(budget.storage(), 19);

        let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        nested_panic_at(PanicAt::Extract, panics);
        assert!(matches!(
            graph.extract_canonical_v18(LIMITS, &mut budget),
            Err(KirBridgeErrorV18::Bridge(
                KirBridgeErrorV1::UpstreamPanicked
            ))
        ));
        let observed = CLEANUP.with(|slot| slot.get());
        assert!(observed.started);
        assert_eq!(observed.drops, panics + 1);
        assert_eq!(observed.poisoned, Some(true));
        assert!(observed.storage > floor);
        assert_eq!(budget.storage(), floor);
        assert!(graph.session.poisoned);
        let accepted = budget.work();
        assert!(graph.extract_canonical_v18(LIMITS, &mut budget).is_err());
        assert_eq!(CLEANUP.with(|slot| slot.get()), observed);
        assert_eq!(budget.work(), accepted);
        assert_eq!(budget.storage(), floor);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn storage_v18_payload_cleanup_exhaustion_aborts_isolated_worker() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_V18_BOUNDED_PANIC_CLEANUP_CHILD";
    if let Some(mode) = std::env::var_os(CHILD) {
        let input = owner(&fixture());
        let mut work = Work::new(AMPLE);
        let mut budget = Budget::new(&mut work, AMPLE);
        if mode == "import" {
            nested_panic_at(PanicAt::Import, 4);
            let _ = KirPlironGraphV18::import(&input, &mut budget);
        } else {
            assert_eq!(mode, "extract");
            let (mut graph, receipt) = KirPlironGraphV18::import(&input, &mut budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            nested_panic_at(PanicAt::Extract, 4);
            let _ = graph.extract_canonical_v18(LIMITS, &mut budget);
        }
        panic!("four panicking destructor attempts must terminate the worker");
    }
    let module = module_path!().split_once("::").unwrap().1;
    let test = format!("{module}::storage_v18_payload_cleanup_exhaustion_aborts_isolated_worker");
    for mode in ["import", "extract"] {
        let child = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "ulimit -c 0 || exit 125; exec \"$@\"",
                "v18-panic-child",
            ])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        assert_eq!(
            child.status.signal(),
            Some(6),
            "{mode}: status {:?}, stdout {}, stderr {}",
            child.status,
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
}
