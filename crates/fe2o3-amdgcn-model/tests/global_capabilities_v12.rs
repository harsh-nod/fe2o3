use fe2o3_amdgcn_model::lower_kernel_to_llvm_ir;
use fe2o3_kernel_ir::*;

fn context_type() -> KernelContextTypeV1 {
    KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32])
}

fn source_identity() -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32])
}

fn read_module(logical: bool) -> Module {
    let element = Type::F32;
    let physical = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let capability = GlobalCapabilityTypeV1::read_only(element.clone(), context_type());
    let pointer = capability.physical_pointer_type();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = if logical {
        vec![
            Operation::kernel_context_issue(ValueId(2), context_type(), source_identity()),
            Operation::global_capability_bind(ValueId(3), capability, ValueId(2), ValueId(0)),
            Operation::global_capability_index(ValueId(4), ValueId(3), ValueId(1), None),
        ]
    } else {
        vec![
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::BOOL),
                OperationKind::Constant(Constant::Bool(false)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::BOOL),
                OperationKind::Constant(Constant::Bool(false)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
        ]
    };
    let slice = if logical { ValueId(3) } else { ValueId(0) };
    let index = if logical { ValueId(4) } else { ValueId(1) };
    let mut load_access = MemoryAccess::new(AddressSpace::Global, 4);
    load_access.volatile = true;
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::SliceLength { slice },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: index,
                rhs: ValueId(5),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Select {
                condition: ValueId(6),
                true_value: index,
                false_value: ValueId(7),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), pointer.clone()),
            OperationKind::SliceData { slice },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(9),
                offset: ValueId(8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), element.clone()),
            OperationKind::Constant(Constant::F32Bits(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), element),
            OperationKind::GuardedLoad {
                pointer: ValueId(10),
                predicate: ValueId(6),
                fallback: ValueId(11),
                access: load_access,
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("global-capability-amd-equivalence");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![physical, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

#[test]
fn verified_logical_global_erases_to_the_established_bounded_physical_llvm() {
    let logical = read_module(true);
    let physical = read_module(false);
    verify_module(&logical).unwrap();
    verify_module(&physical).unwrap();

    let logical_llvm = lower_kernel_to_llvm_ir(&logical, &KernelId::new("kernel")).unwrap();
    let physical_llvm = lower_kernel_to_llvm_ir(&physical, &KernelId::new("kernel")).unwrap();
    assert_eq!(logical_llvm, physical_llvm);
    assert!(logical_llvm.contains("ptr addrspace(1) %arg0.data, i64 %arg0.len"));
    assert!(logical_llvm.contains("icmp ult i64 %arg1"));
    assert!(logical_llvm.contains("load volatile float"));
    assert!(logical_llvm.contains("br i1"));
}
