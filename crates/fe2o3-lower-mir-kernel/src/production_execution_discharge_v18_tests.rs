use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, BorrowedKernelIrVerificationErrorV1,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, ExecutionOperationV15 as Execution,
    ExecutionRoleV15 as Role, Function, Kernel, KernelIrDecodeError, KernelIrEncodeError,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, StorageCopyOverlapV1, StorageFieldV1, StorageLayoutErrorV1, StorageLayoutIdV1,
    StorageLayoutKindV1, StorageLayoutProblemV1, StorageLayoutV1, StorageOperationV1,
    StoragePointerV1, StorageProjectionV1, StorageVariantEncodingV1, StorageVariantV1, Terminator,
    Type, ValueDef, ValueId, WorkgroupMemory, WorkgroupMemoryExtent,
};

const LIMIT: usize = 10_000_000;
const FLOOR: usize = 19;
const PRIOR: usize = 11;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

fn execution(result: Option<(u32, Role)>, kind: Execution) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, role)| ValueDef::new(ValueId(id), Type::Execution(role)))
            .collect(),
        OperationKind::Execution(kind),
    )
}
fn context() -> Operation {
    execution(Some((10, Role::Context)), Execution::ContextIssue)
}
fn derive(id: u32) -> Operation {
    execution(
        Some((id, Role::Workgroup)),
        Execution::WorkgroupDerive {
            context: ValueId(10),
        },
    )
}
fn end(id: u32, discarded: &[u32]) -> Operation {
    execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(id),
            discarded: discarded.iter().copied().map(ValueId).collect(),
        },
    )
}
fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}
fn constant(id: u32, bits: u32) -> Operation {
    value(
        id,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(bits)),
    )
}
fn storage(result: Option<(u32, Type)>, kind: StorageOperationV1) -> Operation {
    Operation::new(
        result
            .into_iter()
            .map(|(id, ty)| ValueDef::new(ValueId(id), ty))
            .collect(),
        OperationKind::Storage(kind),
    )
}
fn module(operations: Vec<Operation>) -> Module {
    let mut module = Module::new("storage-execution-discharge");
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
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}
fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
fn ordinary() -> Module {
    module(vec![
        constant(20, 3),
        context(),
        derive(11),
        constant(21, 5),
        end(11, &[]),
        derive(100),
        constant(22, 7),
        end(100, &[]),
        constant(23, 9),
    ])
}
fn layout_rows() -> Vec<StorageLayoutV1> {
    let field = |offset| StorageFieldV1 {
        offset,
        layout: StorageLayoutIdV1(0),
    };
    vec![
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(vec![field(0), field(8)].into_boxed_slice()),
        },
        StorageLayoutV1 {
            size: 24,
            alignment: 8,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct { tag: field(16) },
                variants: vec![StorageVariantV1 {
                    discriminant: 91,
                    direct_tag_bits: Some(3),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(1),
                }]
                .into_boxed_slice(),
            },
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(3),
                value_space: AddressSpace::Private,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(Vec::new().into_boxed_slice()),
        },
    ]
}
fn stored(space: AddressSpace) -> Module {
    let address =
        |row, access| Type::pointer(Type::StorageObject(StorageLayoutIdV1(row)), space, access);
    let allocate = |id, row| {
        value(
            id,
            address(row, AccessMode::ReadWrite),
            if space == AddressSpace::Workgroup {
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element: Type::StorageObject(StorageLayoutIdV1(row)),
                    extent: WorkgroupMemoryExtent::Static(1),
                    alignment: 8,
                })
            } else {
                OperationKind::Alloca {
                    element: Type::StorageObject(StorageLayoutIdV1(row)),
                    count: None,
                    address_space: space,
                    alignment: 8,
                }
            },
        )
    };
    let access = MemoryAccess::new(space, 8);
    let mut module = module(vec![
        constant(20, 3),
        context(),
        allocate(900, 1),
        derive(11),
        value(
            901,
            Type::Scalar(ScalarType::U64),
            OperationKind::Constant(Constant::U64(23)),
        ),
        storage(
            Some((902, address(0, AccessMode::ReadWrite))),
            StorageOperationV1::Project {
                base: ValueId(900),
                step: StorageProjectionV1::Field(1),
            },
        ),
        storage(
            None,
            StorageOperationV1::WriteValue {
                address: ValueId(902),
                value: ValueId(901),
                access,
            },
        ),
        storage(
            Some((903, Type::Scalar(ScalarType::U64))),
            StorageOperationV1::ReadValue {
                address: ValueId(902),
                access,
            },
        ),
        allocate(904, 1),
        storage(
            None,
            StorageOperationV1::CopyObject {
                source: ValueId(900),
                destination: ValueId(904),
                source_access: access,
                destination_access: access,
                overlap: StorageCopyOverlapV1::MayOverlap,
            },
        ),
        end(11, &[]),
        derive(100),
        allocate(905, 2),
        storage(
            Some((906, address(1, AccessMode::WriteOnly))),
            StorageOperationV1::Project {
                base: ValueId(905),
                step: StorageProjectionV1::VariantForWrite { index: 0 },
            },
        ),
        storage(
            Some((907, address(0, AccessMode::WriteOnly))),
            StorageOperationV1::Project {
                base: ValueId(906),
                step: StorageProjectionV1::Field(0),
            },
        ),
        storage(
            None,
            StorageOperationV1::WriteValue {
                address: ValueId(907),
                value: ValueId(903),
                access,
            },
        ),
        storage(
            None,
            StorageOperationV1::SetDiscriminant {
                address: ValueId(905),
                variant: 0,
                access,
            },
        ),
        storage(
            Some((908, address(1, AccessMode::ReadWrite))),
            StorageOperationV1::Project {
                base: ValueId(905),
                step: StorageProjectionV1::Variant { index: 0, access },
            },
        ),
        storage(
            Some((909, address(0, AccessMode::ReadWrite))),
            StorageOperationV1::Project {
                base: ValueId(908),
                step: StorageProjectionV1::Field(0),
            },
        ),
        storage(
            Some((910, Type::Scalar(ScalarType::U64))),
            StorageOperationV1::ReadValue {
                address: ValueId(909),
                access,
            },
        ),
        end(100, &[]),
        constant(22, 9),
    ]);
    module.storage_layouts = layout_rows();
    module
}
fn branched() -> Module {
    let mut module = stored(AddressSpace::Private);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].operations.truncate(10);
    blocks[0].operations.push(value(
        50,
        Type::BOOL,
        OperationKind::Constant(Constant::Bool(true)),
    ));
    blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(50),
        then_target: BlockId(4_000_000_000),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    for id in [4_000_000_000, 3] {
        blocks.push(BasicBlock {
            id: BlockId(id),
            parameters: vec![],
            operations: vec![end(11, &[])],
            terminator: Some(Terminator::Return { values: vec![] }),
        });
    }
    let mut second = self::module(vec![context()]);
    second.functions[0].id = "context-only".into();
    second.kernels[0].id = "second".into();
    second.kernels[0].entry = "context-only".into();
    module.functions.extend(second.functions);
    module.kernels.extend(second.kernels);
    module.functions.insert(
        1,
        Function::external_import(
            "unchanged",
            Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
        ),
    );
    module
}
fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            LAYOUTS,
            &mut budget,
        )
        .expect("fixture must pass real V18 structural/CFG/lifecycle admission");
    assert_eq!(budget.storage(), FLOOR);
    (owner, receipt.retained_storage())
}

struct Run {
    result: Result<
        (
            ProductionExecutionDischargeV18,
            ProductionExecutionDischargeStorageV29,
        ),
        DischargeError,
    >,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn run(
    input: &VerifiedCanonicalKernelIrModuleV18,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
    limits: StorageLayoutLimitsV1,
) -> Run {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = ProductionExecutionDischargeV18::try_discharge(input, limits, &mut budget);
    assert_eq!(
        budget.storage(),
        floor,
        "all result paths restore paid input custody"
    );
    let (used, peak, failed_storage) = (
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    Run {
        result,
        work: used,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}
fn canonical_resource(error: &CanonicalKernelIrReplayAdmissionErrorV18) -> Option<ResourceError> {
    match error {
        CanonicalKernelIrReplayAdmissionErrorV18::Resource(error)
        | CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::Resource(error))
        | CanonicalKernelIrReplayAdmissionErrorV18::Layout(StorageLayoutErrorV1::Resource(error))
        | CanonicalKernelIrReplayAdmissionErrorV18::Verification(
            BorrowedKernelIrVerificationErrorV1::Resource(error),
        ) => Some(*error),
        CanonicalKernelIrReplayAdmissionErrorV18::Encode(KernelIrEncodeError::WorkLimit(error))
        | CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::WorkLimit(error))
        | CanonicalKernelIrReplayAdmissionErrorV18::Decode(KernelIrDecodeError::Encode(
            KernelIrEncodeError::WorkLimit(error),
        )) => Some(ResourceError::Work(*error)),
        _ => None,
    }
}
fn resource(error: &DischargeError) -> Option<ResourceError> {
    match error {
        DischargeError::Input(error) | DischargeError::Output(error) => canonical_resource(error),
        DischargeError::Resource(error) => Some(*error),
        _ => None,
    }
}

#[test]
fn v18_lifecycle_successor_preserves_real_storage_graphs_and_exact_erasure_rosters() {
    for module in [
        ordinary(),
        stored(AddressSpace::Private),
        stored(AddressSpace::Workgroup),
        branched(),
    ] {
        let (input, input_bytes) = admit(&module);
        let identity = *input.identity();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + input_bytes).unwrap();
        let floor = budget.storage();
        let (output, receipt) =
            ProductionExecutionDischargeV18::try_discharge(&input, LAYOUTS, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(output.input_identity(), &identity);
        assert_ne!(output.output().identity(), &identity);
        assert_eq!(
            output.output().module().storage_layouts,
            module.storage_layouts
        );
        let expected: Vec<_> = input
            .module()
            .functions
            .iter()
            .enumerate()
            .flat_map(|(function, f)| {
                f.body.iter().flat_map(move |body| {
                    body.blocks.iter().enumerate().flat_map(move |(block, b)| {
                        b.operations
                            .iter()
                            .enumerate()
                            .filter_map(move |(operation, op)| {
                                matches!(op.kind, OperationKind::Execution(_))
                                    .then_some((function, block, operation))
                            })
                    })
                })
            })
            .collect();
        assert_eq!(
            output
                .erased_operations()
                .iter()
                .map(|row| (
                    row.function_ordinal(),
                    row.block_ordinal(),
                    row.operation_ordinal()
                ))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(expected.len(), 5);
        for (before, after) in input
            .module()
            .functions
            .iter()
            .zip(&output.output().module().functions)
        {
            if let (Some(before), Some(after)) = (&before.body, &after.body) {
                for (before, after) in before.blocks.iter().zip(&after.blocks) {
                    assert_eq!(
                        before
                            .operations
                            .iter()
                            .filter(|op| !matches!(op.kind, OperationKind::Execution(_)))
                            .collect::<Vec<_>>(),
                        after.operations.iter().collect::<Vec<_>>()
                    );
                    assert_eq!(before.terminator, after.terminator);
                }
            }
        }
        let (inventory, inventory_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(
                output.output(),
                &mut budget,
            )
            .unwrap();
        budget
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        assert!(inventory.belongs_to(output.output()));
        assert_eq!(inventory.identity_v18(), *output.output().identity());
        drop(inventory);
        budget
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        // Independently admitted output establishes the receipt component, not
        // a bound inferred from the wrapper's reported retained amount.
        let (checked, checked_bytes) = admit(output.output().module());
        assert_eq!(checked.identity(), output.output().identity());
        assert_eq!(
            receipt.retained_storage(),
            checked_bytes + size_of::<ProductionExecutionDischargeV18>()
                - size_of::<VerifiedCanonicalKernelIrModuleV18>()
                + 5 * size_of::<ProductionExecutionErasureV29>()
        );
        drop(checked);
        drop(output);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(*input.identity(), identity);
        drop(input);
        budget.release_storage(input_bytes).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn v18_erasure_replay_rejects_layout_only_and_retained_graph_changes_after_real_admission() {
    let (input, input_bytes) = admit(&stored(AddressSpace::Private));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + input_bytes).unwrap();
    let (correct, receipt) =
        ProductionExecutionDischargeV18::try_discharge(&input, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let original = correct.output().module();
    for fault in 0..11 {
        let floor = budget.storage();
        replay(
            &input,
            correct.output(),
            correct.erased_operations(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        let mut changed = original.clone();
        match fault {
            0 => changed.storage_layouts[4].size = 16,
            1 => changed.storage_layouts[4].alignment = 4,
            2 => changed.storage_layouts[4].kind = StorageLayoutKindV1::Scalar(ScalarType::U64),
            3 => changed.storage_layouts.swap(3, 4),
            4 => {
                let StorageLayoutKindV1::Variants { variants, .. } =
                    &mut changed.storage_layouts[2].kind
                else {
                    unreachable!()
                };
                variants[0].direct_tag_bits = Some(5);
            }
            5 => {
                let OperationKind::Constant(Constant::U32(bits)) =
                    &mut operations(&mut changed)[0].kind
                else {
                    unreachable!()
                };
                *bits += 1;
            }
            6 => {
                let OperationKind::Storage(StorageOperationV1::Project { step, .. }) =
                    &mut operations(&mut changed)[3].kind
                else {
                    unreachable!()
                };
                *step = StorageProjectionV1::Field(0);
            }
            7 => {
                let OperationKind::Storage(StorageOperationV1::WriteValue { access, .. }) =
                    &mut operations(&mut changed)[4].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
            }
            8 => {
                let OperationKind::Storage(StorageOperationV1::CopyObject { overlap, .. }) =
                    &mut operations(&mut changed)[7].kind
                else {
                    unreachable!()
                };
                *overlap = StorageCopyOverlapV1::NonOverlapping;
            }
            9 => changed.kernels[0].id = "different-public-name".into(),
            10 => {
                operations(&mut changed)[0].results[0].ty = Type::Scalar(ScalarType::I32);
                operations(&mut changed)[0].kind = OperationKind::Constant(Constant::I32(3));
            }
            _ => unreachable!(),
        }
        let (hostile, hostile_bytes) = admit(&changed);
        budget.reserve_storage(hostile_bytes).unwrap();
        assert!(
            matches!(
                replay(&input, &hostile, correct.erased_operations(), &mut budget),
                Err(DischargeError::ReplayMismatch)
            ),
            "fault {fault}"
        );
        drop(hostile);
        budget.release_storage(hostile_bytes).unwrap();
        assert_eq!(budget.storage(), floor);
        replay(
            &input,
            correct.output(),
            correct.erased_operations(),
            &mut budget,
        )
        .unwrap();
    }
    for fault in 0..5 {
        let mut rows = correct.erased_operations().to_vec();
        match fault {
            0 => rows[0].function_ordinal += 1,
            1 => rows[0].block_ordinal += 1,
            2 => rows[0].operation_ordinal += 1,
            3 => rows.swap(0, 1),
            4 => rows[0].kind = crate::ProductionExecutionErasureKindV29::ScopeEnd,
            _ => unreachable!(),
        }
        let floor = budget.storage();
        assert!(matches!(
            replay(&input, correct.output(), &rows, &mut budget),
            Err(DischargeError::ReplayMismatch)
        ));
        assert_eq!(budget.storage(), floor);
        replay(
            &input,
            correct.output(),
            correct.erased_operations(),
            &mut budget,
        )
        .unwrap();
    }
    drop(correct);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_bytes).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn v18_erasure_replay_preserves_original_branch_successor_order() {
    let (input, input_bytes) = admit(&branched());
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + input_bytes).unwrap();
    let (correct, receipt) =
        ProductionExecutionDischargeV18::try_discharge(&input, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    replay(
        &input,
        correct.output(),
        correct.erased_operations(),
        &mut budget,
    )
    .unwrap();
    let mut changed = correct.output().module().clone();
    let Some(Terminator::ConditionalBranch {
        then_target,
        else_target,
        ..
    }) = &mut changed.functions[0].body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    std::mem::swap(then_target, else_target);
    let (hostile, hostile_bytes) = admit(&changed);
    budget.reserve_storage(hostile_bytes).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        replay(&input, &hostile, correct.erased_operations(), &mut budget),
        Err(DischargeError::ReplayMismatch)
    ));
    assert_eq!(budget.storage(), floor);
    drop(hostile);
    budget.release_storage(hostile_bytes).unwrap();
    replay(
        &input,
        correct.output(),
        correct.erased_operations(),
        &mut budget,
    )
    .unwrap();
    drop(correct);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(input);
    budget.release_storage(input_bytes).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn v18_no_execution_has_independent_exact_work_and_temporary_header_premises() {
    let mut module = Module::new("no-execution");
    module.storage_layouts = layout_rows();
    let (input, bytes) = admit(&module);
    type Outcome = Result<
        (
            ProductionExecutionDischargeV18,
            ProductionExecutionDischargeStorageV29,
        ),
        DischargeError,
    >;
    let headers = size_of::<DischargeScope<'_, '_>>()
        + 2 * size_of::<Result<DischargeScope<'_, '_>, ResourceError>>()
        + 3 * size_of::<Outcome>()
        + 2 * size_of::<Result<(), ResourceError>>();
    assert_eq!(DischargeScope::headers().unwrap(), headers);
    let floor = FLOOR + bytes;
    let work = 1 + input.canonical_bytes().len();
    let result = run(&input, floor, work, floor + headers, LAYOUTS);
    assert!(matches!(result.result, Err(DischargeError::NoExecution)));
    assert_eq!((result.work, result.peak), (work, floor + headers));
    assert!(matches!(
        run(&input, floor, work - 1, floor + headers, LAYOUTS).result,
        Err(DischargeError::Resource(ResourceError::Work(_)))
    ));
    assert!(matches!(
        run(&input, floor, work, floor + headers - 1, LAYOUTS).result,
        Err(DischargeError::Resource(ResourceError::Storage(_)))
    ));
    let zero = run(&input, floor, 0, floor, LAYOUTS);
    assert!(matches!(
        zero.result,
        Err(DischargeError::Resource(ResourceError::Work(_)))
    ));
    assert_eq!((zero.work, zero.peak), (0, floor));
}

#[test]
fn v18_lifecycle_exact_resource_boundaries_caps_and_first_denials() {
    let (input, bytes) = admit(&stored(AddressSpace::Private));
    let floor = FLOOR + bytes;
    let measured = run(&input, floor, LIMIT, LIMIT, LAYOUTS);
    let (output, receipt) = measured.result.unwrap();
    let retained = receipt.retained_storage();
    drop(output);
    let exact = run(&input, floor, measured.work, measured.peak, LAYOUTS);
    assert_eq!(
        exact.result.as_ref().unwrap().1.retained_storage(),
        retained
    );
    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
    drop(exact);
    for _ in 0..3 {
        let failed = run(&input, floor, measured.work - 1, measured.peak, LAYOUTS);
        assert!(matches!(
            resource(failed.result.as_ref().unwrap_err()),
            Some(ResourceError::Work(_))
        ));
        assert!(failed.failed_work.is_some());
        let failed = run(&input, floor, measured.work, measured.peak - 1, LAYOUTS);
        assert!(matches!(
            resource(failed.result.as_ref().unwrap_err()),
            Some(ResourceError::Storage(_))
        ));
        assert!(failed.failed_storage.is_some());
    }
    let exact_layouts = StorageLayoutLimitsV1 {
        rows: 5,
        edges: 5,
        containment_depth: 3,
        object_bytes: 24,
    };
    assert!(
        run(&input, floor, LIMIT, LIMIT, exact_layouts)
            .result
            .is_ok()
    );
    for (limits, problem) in [
        (
            StorageLayoutLimitsV1 {
                rows: 4,
                ..exact_layouts
            },
            StorageLayoutProblemV1::Rows,
        ),
        (
            StorageLayoutLimitsV1 {
                edges: 4,
                ..exact_layouts
            },
            StorageLayoutProblemV1::Edges,
        ),
        (
            StorageLayoutLimitsV1 {
                containment_depth: 2,
                ..exact_layouts
            },
            StorageLayoutProblemV1::Depth,
        ),
        (
            StorageLayoutLimitsV1 {
                object_bytes: 23,
                ..exact_layouts
            },
            StorageLayoutProblemV1::Size,
        ),
    ] {
        assert!(matches!(run(&input, floor, LIMIT, LIMIT, limits).result,
            Err(DischargeError::Output(CanonicalKernelIrReplayAdmissionErrorV18::Layout(StorageLayoutErrorV1::Invalid { problem: actual, .. }))) if actual == problem));
    }
    let mut work = Work::new(PRIOR + 20);
    work.charge_work(PRIOR).unwrap();
    {
        let mut budget = Budget::new(&mut work, floor + 4096);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let error = ProductionExecutionDischargeV18::try_discharge(&input, LAYOUTS, &mut budget)
            .unwrap_err();
        assert!(matches!(resource(&error), Some(ResourceError::Work(_))));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

struct ArmGuard;
impl ArmGuard {
    fn new(point: faults::Point, fault: faults::Fault) -> Self {
        faults::ARMED.with(|armed| assert!(armed.replace(Some((point, fault))).is_none()));
        faults::SEEN.set(None);
        Self
    }
}
impl Drop for ArmGuard {
    fn drop(&mut self) {
        faults::ARMED.with(|armed| {
            armed.borrow_mut().take();
        });
    }
}
fn observed(
    input: &VerifiedCanonicalKernelIrModuleV18,
    floor: usize,
    point: faults::Point,
) -> faults::History {
    let _arm = ArmGuard::new(point, faults::Fault::Observe);
    let result = run(input, floor, LIMIT, LIMIT, LAYOUTS);
    assert!(result.result.is_ok());
    faults::SEEN.get().expect("checkpoint must be reached")
}

#[test]
fn v18_lifecycle_preserves_nested_decode_work_and_output_admission_error_phases() {
    let (input, bytes) = admit(&ordinary());
    let floor = FLOOR + bytes;
    let copied = observed(&input, floor, faults::Point::InputTransfer);
    let error = run(&input, floor, copied.0 - 1, LIMIT, LAYOUTS)
        .result
        .unwrap_err();
    assert!(
        matches!(
            error,
            DischargeError::Input(CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(_))
            ))
        ),
        "last copy work must remain a nested Input decode/encode error: {error:?}"
    );
    let emitted = observed(&input, floor, faults::Point::Erased);
    let error = run(&input, floor, emitted.0 + 1, LIMIT, LAYOUTS)
        .result
        .unwrap_err();
    assert!(
        matches!(error, DischargeError::Output(_)),
        "fresh output phase: {error:?}"
    );
    assert!(matches!(resource(&error), Some(ResourceError::Work(_))));
}

#[test]
fn v18_lifecycle_candidate_and_output_transfer_denials_release_only_owned_bytes() {
    let (input, bytes) = admit(&stored(AddressSpace::Private));
    let identity = *input.identity();
    let floor = FLOOR + bytes;
    for point in [faults::Point::InputTransfer, faults::Point::OutputTransfer] {
        let _arm = ArmGuard::new(point, faults::Fault::Deny);
        let result = run(&input, floor, LIMIT, LIMIT, LAYOUTS);
        assert!(matches!(
            result.result,
            Err(DischargeError::Resource(ResourceError::Storage(_)))
        ));
        assert_eq!(result.failed_storage, Some(LIMIT + 1));
        let seen = faults::SEEN.get().expect("transfer point");
        assert_eq!(result.work, seen.0);
        assert_eq!(result.peak, seen.2);
        assert_eq!(*input.identity(), identity);
    }
}

#[test]
fn v18_lifecycle_unwind_drops_candidates_before_floor_refund_and_recovers() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Probe(Arc<AtomicUsize>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (input, bytes) = admit(&stored(AddressSpace::Private));
    let identity = *input.identity();
    let floor = FLOOR + bytes;
    for point in [
        faults::Point::InputTransfer,
        faults::Point::InputAdopted,
        faults::Point::Erased,
        faults::Point::OutputTransfer,
        faults::Point::OutputAdopted,
        faults::Point::Replayed,
    ] {
        let drops = Arc::new(AtomicUsize::new(0));
        let _arm = ArmGuard::new(point, faults::Fault::Panic(Box::new(Probe(drops.clone()))));
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ProductionExecutionDischargeV18::try_discharge(&input, LAYOUTS, &mut budget)
        }));
        assert!(result.is_err(), "{point:?}");
        assert_eq!(budget.storage(), floor);
        let seen = faults::SEEN.get().expect("panic point");
        assert_eq!(
            (
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (seen.0, seen.2, seen.3)
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(result);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(*input.identity(), identity);
        let (output, receipt) =
            ProductionExecutionDischargeV18::try_discharge(&input, LAYOUTS, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(output);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn v18_tiles_fragments_and_descendant_disposal_require_their_existing_lowering() {
    for consumption in 0..3 {
        let mut module = module(vec![
            context(),
            derive(11),
            execution(
                Some((
                    12,
                    Role::MaskedTileU32 {
                        lanes: 64,
                        elements: 1,
                    },
                )),
                Execution::MaskedTileLoadU32 {
                    workgroup: ValueId(11),
                    input: ValueId(0),
                    base: ValueId(1),
                    lanes: 64,
                    elements: 1,
                },
            ),
        ]);
        if consumption == 0 {
            operations(&mut module).push(end(11, &[12]));
        } else {
            operations(&mut module).push(execution(
                Some((
                    13,
                    Role::LaneFragmentU32 {
                        lanes: 64,
                        elements: 1,
                    },
                )),
                Execution::TileIntoFragmentU32 {
                    tile: ValueId(12),
                    lanes: 64,
                    elements: 1,
                },
            ));
            if consumption == 1 {
                operations(&mut module).push(end(11, &[13]));
            } else {
                operations(&mut module).push(Operation::new(
                    vec![
                        ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                        ValueDef::new(ValueId(21), Type::BOOL),
                    ],
                    OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                        fragment: ValueId(13),
                        lanes: 64,
                        elements: 1,
                    }),
                ));
                operations(&mut module).push(end(11, &[]));
            }
        }
        module.storage_layouts = layout_rows();
        let (input, bytes) = admit(&module);
        assert!(matches!(
            run(&input, FLOOR + bytes, LIMIT, LIMIT, LAYOUTS).result,
            Err(DischargeError::UnsupportedExecution)
        ));
    }
}
