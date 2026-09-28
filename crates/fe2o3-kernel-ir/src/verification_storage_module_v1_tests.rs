use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CastKind,
    Constant, DiagnosticCode, Function, MemoryAccess, Operation, OperationKind, ScalarType,
    Signature, StorageCopyOverlapV1, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1,
    StorageLayoutLimitsV1, StorageLayoutV1, StorageOperationV1, StorageProjectionV1, Terminator,
    Type, ValueDef, ValueId, VerificationErrors, WorkgroupMemory, WorkgroupMemoryExtent,
    check_module_storage_v1, verify_module_ref,
};

pub(super) const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

pub(super) fn checked(module: &Module) -> StructurallyCheckedModuleStorageV1<'_> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let checked = check_module_storage_v1(module, LIMITS, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    checked
}

fn word() -> Type {
    Type::Scalar(ScalarType::U64)
}

fn object(id: u32) -> Type {
    Type::StorageObject(StorageLayoutIdV1(id))
}

fn address(id: u32, space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(object(id), space, access)
}

fn private(id: u32) -> Type {
    address(id, AddressSpace::Private, AccessMode::ReadWrite)
}

fn rows() -> Vec<StorageLayoutV1> {
    vec![
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
        },
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
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(0),
                length: 2,
                stride: 8,
            },
        },
    ]
}

fn operation(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}

fn storage(id: u32, ty: Type, kind: StorageOperationV1) -> Operation {
    operation(id, ty, OperationKind::Storage(kind))
}

fn one_block(
    parameters: Vec<Type>,
    results: Vec<Type>,
    operations: Vec<Operation>,
    returned: Vec<ValueId>,
) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: returned });
    let values = (0..parameters.len()).map(|i| ValueId(i as u32)).collect();
    let mut module = Module::new("storage");
    module.storage_layouts = rows();
    module.functions.push(Function::definition(
        "f",
        Signature::new(parameters, results),
        values,
        vec![block],
    ));
    module
}

pub(super) fn allocation(space: AddressSpace) -> Module {
    let access = MemoryAccess::new(space, 8);
    let allocation = if space == AddressSpace::Workgroup {
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: object(1),
            extent: WorkgroupMemoryExtent::Static(1),
            alignment: 8,
        })
    } else {
        OperationKind::Alloca {
            element: object(1),
            count: None,
            address_space: space,
            alignment: 8,
        }
    };
    one_block(
        vec![],
        vec![word()],
        vec![
            operation(0, address(1, space, AccessMode::ReadWrite), allocation),
            operation(1, word(), OperationKind::Constant(Constant::U64(7))),
            storage(
                2,
                address(0, space, AccessMode::ReadWrite),
                StorageOperationV1::Project {
                    base: ValueId(0),
                    step: StorageProjectionV1::Field(0),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::WriteValue {
                    address: ValueId(2),
                    value: ValueId(1),
                    access,
                }),
            ),
            storage(
                3,
                word(),
                StorageOperationV1::ReadValue {
                    address: ValueId(2),
                    access,
                },
            ),
        ],
        vec![ValueId(3)],
    )
}

fn body(module: &mut Module) -> &mut crate::FunctionBody {
    module.functions[0].body.as_mut().unwrap()
}

pub(super) fn inspect(module: &Module) -> Result<VerifiedStorageKernelIrModuleV1<'_>, Error> {
    let storage = checked(module);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    let result = verify_storage_module_ref_with_budget_v1(storage, None, &mut budget);
    assert_eq!(budget.storage(), 19);
    result
}

fn accepts(module: &Module) {
    let token = inspect(module).unwrap();
    assert!(std::ptr::eq(token.module(), module));
    assert!(std::ptr::eq(token.storage().module(), module));
    assert!(std::ptr::eq(
        token.storage().layouts().rows(),
        module.storage_layouts.as_slice()
    ));
}

fn rejects(module: &Module, code: DiagnosticCode) -> VerificationErrors {
    match inspect(module).unwrap_err() {
        Error::Verification(errors) => {
            assert!(errors.contains(code), "{errors:?}");
            errors
        }
        Error::Resource(error) => panic!("unexpected resource error: {error}"),
    }
}

fn flow() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(1));
    left.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(1)],
    });
    let mut right = BasicBlock::new(BlockId(2));
    right.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(2)],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.parameters = vec![ValueDef::new(ValueId(3), private(0))];
    merge.operations.push(operation(
        4,
        word(),
        OperationKind::Call {
            callee: "read_helper".into(),
            arguments: vec![ValueId(3)],
        },
    ));
    merge.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut helper = BasicBlock::new(BlockId(0));
    helper.operations.push(storage(
        1,
        word(),
        StorageOperationV1::ReadValue {
            address: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        },
    ));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut module = Module::new("storage");
    module.storage_layouts = rows();
    module.functions.push(Function::definition(
        "f",
        Signature::new(
            vec![Type::Scalar(ScalarType::Bool), private(0), private(0)],
            vec![word()],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, left, right, merge],
    ));
    module.functions.push(Function::definition(
        "read_helper",
        Signature::new(vec![private(0)], vec![word()]),
        vec![ValueId(0)],
        vec![helper],
    ));
    module
}

#[test]
fn storage_module_allocations_bind_private_and_workgroup_rows() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let module = allocation(space);
        accepts(&module);
        assert!(verify_module_ref(&module).is_err());
    }
}

#[test]
fn storage_module_reuses_cfg_block_arguments_and_defined_call_checks() {
    accepts(&flow());
    let mut missing = flow();
    body(&mut missing).blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(99),
        arguments: vec![ValueId(1)],
    });
    rejects(&missing, DiagnosticCode::InvalidBranchTarget);
    let mut arity = flow();
    body(&mut arity).blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    rejects(&arity, DiagnosticCode::BranchArgumentCount);
    let mut wrong_row = flow();
    wrong_row.functions[0].signature.parameters[1] = private(1);
    rejects(&wrong_row, DiagnosticCode::BranchArgumentType);
    let mut wrong_call = flow();
    wrong_call.functions[1].signature.parameters[0] = private(1);
    rejects(&wrong_call, DiagnosticCode::TypeMismatch);
}

#[test]
fn storage_module_keeps_undefined_dominance_duplicates_and_result_checks() {
    let mut undefined = allocation(AddressSpace::Private);
    body(&mut undefined).blocks[0].operations[4].kind =
        OperationKind::Storage(StorageOperationV1::ReadValue {
            address: ValueId(99),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        });
    rejects(&undefined, DiagnosticCode::UndefinedValue);
    let mut dominance = flow();
    body(&mut dominance).blocks[1].operations.push(operation(
        8,
        private(0),
        OperationKind::Select {
            condition: ValueId(0),
            true_value: ValueId(1),
            false_value: ValueId(2),
        },
    ));
    body(&mut dominance).blocks[3].operations[0].kind = OperationKind::Call {
        callee: "read_helper".into(),
        arguments: vec![ValueId(8)],
    };
    rejects(&dominance, DiagnosticCode::NonDominatingUse);
    let mut duplicate = allocation(AddressSpace::Private);
    body(&mut duplicate).blocks[0].operations[1].results[0].id = ValueId(0);
    rejects(&duplicate, DiagnosticCode::DuplicateValue);
    let mut wrong_result = allocation(AddressSpace::Private);
    body(&mut wrong_result).blocks[0].operations[4].results[0].ty = Type::Scalar(ScalarType::U32);
    rejects(&wrong_result, DiagnosticCode::TypeMismatch);
    let mut no_result = allocation(AddressSpace::Private);
    body(&mut no_result).blocks[0].operations[4].results.clear();
    rejects(&no_result, DiagnosticCode::ResultArity);
}

#[test]
fn storage_module_forbids_direct_storage_ssa_in_every_definition_roster() {
    let parameter = one_block(vec![object(0)], vec![], vec![], vec![]);
    rejects(&parameter, DiagnosticCode::InvalidOperandType);
    let signature = one_block(vec![], vec![object(0)], vec![], vec![]);
    rejects(&signature, DiagnosticCode::InvalidOperandType);
    let mut block = flow();
    body(&mut block).blocks[3].parameters[0].ty = object(0);
    rejects(&block, DiagnosticCode::InvalidOperandType);
    let mut result = allocation(AddressSpace::Private);
    body(&mut result).blocks[0].operations[4].results[0].ty = object(0);
    rejects(&result, DiagnosticCode::InvalidOperandType);
}

#[test]
fn storage_module_unknown_ids_and_foreign_same_name_owners_do_not_unify() {
    let valid = allocation(AddressSpace::Private);
    let mut foreign = valid.clone();
    foreign.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I64);
    assert_eq!(valid.id, foreign.id);
    accepts(&valid);
    rejects(&foreign, DiagnosticCode::TypeMismatch);
    let mut unknown = one_block(vec![private(99)], vec![], vec![], vec![]);
    rejects(&unknown, DiagnosticCode::InvalidOperandType);
    unknown.storage_layouts.clear();
    rejects(&unknown, DiagnosticCode::InvalidOperandType);
    // No API accepts a Module and an independently selected table/view pair.
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let token =
        verify_storage_module_ref_with_budget_v1(checked(&valid), None, &mut budget).unwrap();
    assert!(!std::ptr::eq(token.module(), &foreign));
}

#[test]
fn storage_module_legacy_refuses_raw_types_and_operations_even_with_empty_table() {
    let mut raw_type = one_block(vec![private(0)], vec![], vec![], vec![]);
    raw_type.storage_layouts.clear();
    assert!(
        verify_module_ref(&raw_type)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );
    let mut raw_operation = one_block(
        vec![Type::pointer(
            word(),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        )],
        vec![word()],
        vec![storage(
            1,
            word(),
            StorageOperationV1::ReadValue {
                address: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        )],
        vec![ValueId(1)],
    );
    raw_operation.storage_layouts.clear();
    assert!(
        verify_module_ref(&raw_operation)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidSemanticOperation)
    );
    rejects(&raw_operation, DiagnosticCode::InvalidOperandType);
}

#[test]
fn storage_module_allocation_rejects_wrong_ids_alignment_space_and_rights() {
    for (element, alignment, space, result) in [
        (object(99), 8, AddressSpace::Private, private(99)),
        (object(1), 4, AddressSpace::Private, private(1)),
        (object(1), 12, AddressSpace::Private, private(1)),
        (
            object(1),
            8,
            AddressSpace::Global,
            address(1, AddressSpace::Global, AccessMode::ReadWrite),
        ),
        (
            object(1),
            8,
            AddressSpace::Private,
            address(1, AddressSpace::Private, AccessMode::ReadOnly),
        ),
        (object(1), 8, AddressSpace::Private, private(0)),
        (
            private(0),
            8,
            AddressSpace::Private,
            Type::pointer(private(0), AddressSpace::Private, AccessMode::ReadWrite),
        ),
    ] {
        let module = one_block(
            vec![],
            vec![],
            vec![operation(
                0,
                result,
                OperationKind::Alloca {
                    element,
                    count: None,
                    address_space: space,
                    alignment,
                },
            )],
            vec![],
        );
        assert!(matches!(inspect(&module), Err(Error::Verification(_))));
    }
    for alignment in [0, 4, 12] {
        let mut module = allocation(AddressSpace::Workgroup);
        if let OperationKind::WorkgroupMemory(memory) =
            &mut body(&mut module).blocks[0].operations[0].kind
        {
            memory.alignment = alignment;
        }
        rejects(&module, DiagnosticCode::InvalidAlignment);
    }
}

#[test]
fn storage_module_generic_memory_and_numeric_casts_cannot_bridge_layouts() {
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    for (result, kind) in [
        (
            Some(object(0)),
            OperationKind::Load {
                pointer: ValueId(0),
                access,
            },
        ),
        (
            None,
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access,
            },
        ),
        (
            Some(private(0)),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(2),
            },
        ),
        (
            Some(word()),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: word(),
            },
        ),
    ] {
        let results = result
            .into_iter()
            .map(|ty| ValueDef::new(ValueId(3), ty))
            .collect();
        let module = one_block(
            vec![private(0), word(), Type::Scalar(ScalarType::Index)],
            vec![],
            vec![Operation::new(results, kind)],
            vec![],
        );
        rejects(&module, DiagnosticCode::InvalidMemoryAccess);
    }
    let nested = Type::pointer(private(0), AddressSpace::Private, AccessMode::ReadWrite);
    let load = one_block(
        vec![nested],
        vec![],
        vec![operation(
            1,
            private(0),
            OperationKind::Load {
                pointer: ValueId(0),
                access,
            },
        )],
        vec![],
    );
    rejects(&load, DiagnosticCode::InvalidMemoryAccess);
}

#[test]
fn storage_module_pointer_access_restriction_preserves_exact_layout_and_space() {
    let target = address(0, AddressSpace::Private, AccessMode::ReadOnly);
    let build = |source: Type, target: Type| {
        one_block(
            vec![source],
            vec![],
            vec![operation(
                1,
                target.clone(),
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: ValueId(0),
                    to: target,
                },
            )],
            vec![],
        )
    };
    accepts(&build(private(0), target.clone()));
    for (source, target) in [
        (
            private(0),
            address(1, AddressSpace::Private, AccessMode::ReadOnly),
        ),
        (
            private(0),
            address(0, AddressSpace::Workgroup, AccessMode::ReadOnly),
        ),
        (target, private(0)),
    ] {
        rejects(&build(source, target), DiagnosticCode::InvalidCast);
    }
}

#[test]
fn storage_module_copy_checks_both_local_layouts_and_access_endpoints() {
    let copy = |source, destination, source_access, destination_access| {
        one_block(
            vec![source, destination],
            vec![],
            vec![Operation::new(
                vec![],
                OperationKind::Storage(StorageOperationV1::CopyObject {
                    source: ValueId(0),
                    destination: ValueId(1),
                    source_access,
                    destination_access,
                    overlap: StorageCopyOverlapV1::MayOverlap,
                }),
            )],
            vec![],
        )
    };
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    accepts(&copy(private(1), private(1), access, access));
    rejects(
        &copy(private(0), private(1), access, access),
        DiagnosticCode::TypeMismatch,
    );
    rejects(
        &copy(
            private(1),
            address(1, AddressSpace::Private, AccessMode::ReadOnly),
            access,
            access,
        ),
        DiagnosticCode::InvalidMemoryAccess,
    );
    rejects(
        &copy(
            private(1),
            private(1),
            MemoryAccess::new(AddressSpace::Global, 8),
            access,
        ),
        DiagnosticCode::InvalidMemoryAccess,
    );
}

#[test]
fn storage_module_array_projection_checks_index_type_and_child_identity() {
    let mut module = one_block(
        vec![private(2), Type::Scalar(ScalarType::Index)],
        vec![],
        vec![storage(
            2,
            private(0),
            StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::ArrayIndex(ValueId(1)),
            },
        )],
        vec![],
    );
    accepts(&module);
    body(&mut module).blocks[0].operations[0].results[0].ty = private(1);
    rejects(&module, DiagnosticCode::TypeMismatch);
    body(&mut module).blocks[0].operations[0].results[0].ty = private(0);
    module.functions[0].signature.parameters[1] = Type::Scalar(ScalarType::Bool);
    rejects(&module, DiagnosticCode::TypeMismatch);
}

#[test]
fn storage_module_structural_layout_failure_precedes_the_verifier_token() {
    let mut module = allocation(AddressSpace::Private);
    module.storage_layouts[0].alignment = 3;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(19).unwrap();
    assert!(check_module_storage_v1(&module, LIMITS, &mut budget).is_err());
    assert_eq!(budget.storage(), 19);
}

#[test]
fn storage_module_shared_preflight_bounds_nested_type_depth_before_verification() {
    for (depth, accepted) in [(64, true), (65, false)] {
        let ty = (0..depth).fold(object(0), |ty, _| {
            Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite)
        });
        let mut module = Module::new("module");
        module.storage_layouts = rows();
        module.functions.push(Function::external_import(
            "function",
            Signature::new(vec![ty], vec![]),
        ));
        if accepted {
            accepts(&module);
        } else {
            rejects(&module, DiagnosticCode::ResourceLimit);
        }
    }
}
