use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::lower_compiler_module_to_gfx942_llvm_ir;
use fe2o3_kernel_ir::*;

fn returning_block(operations: Vec<Operation>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn copy(
    source: ValueId,
    destination: ValueId,
    count: ValueId,
    element: MemoryElementType,
) -> Operation {
    Operation::new(
        vec![],
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
            source,
            destination,
            count,
            element,
            source_address_space: AddressSpace::Global,
            destination_address_space: AddressSpace::Global,
            layout: element.expected_layout(),
            contract: CopyNonOverlappingContract::supported_rust(),
        }),
    )
}

fn memory_module() -> Module {
    let element = MemoryElementType::Scalar(ScalarType::U32);
    let input = Type::slice(
        element.ir_type(),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let output = Type::slice(
        element.ir_type(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let source_pointer = Type::pointer(
        element.ir_type(),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let destination_pointer = Type::pointer(
        element.ir_type(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), source_pointer),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), destination_pointer),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::I64)),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
                pointer: ValueId(3),
                origin: ValueId(3),
                kind: PointerDistanceKind::Signed,
                unit: PointerDistanceUnit::Elements,
                element,
                address_space: AddressSpace::Global,
                layout: element.expected_layout(),
                contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
                pointer: ValueId(3),
                origin: ValueId(3),
                kind: PointerDistanceKind::Unsigned,
                unit: PointerDistanceUnit::Bytes,
                element,
                address_space: AddressSpace::Global,
                layout: element.expected_layout(),
                contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Unsigned),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), element.ir_type()),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(3),
                element,
                address_space: AddressSpace::Global,
                layout: element.expected_layout(),
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(4),
                value: ValueId(7),
                element,
                address_space: AddressSpace::Global,
                layout: element.expected_layout(),
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
        ),
        copy(ValueId(3), ValueId(4), ValueId(2), element),
    ];
    let entry = Function::kernel_entry(
        "memory_entry",
        Signature::new(vec![input, output, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![returning_block(operations)],
    );

    let zst = MemoryElementType::Unit;
    let zst_source = Type::pointer(Type::Unit, AddressSpace::Global, AccessMode::ReadOnly);
    let zst_destination = Type::pointer(Type::Unit, AddressSpace::Global, AccessMode::ReadWrite);
    let mut zst_helper = Function::internal_helper(
        "zst_copy",
        Signature::new(vec![zst_source, zst_destination, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![returning_block(vec![
            Operation::new(
                vec![],
                OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                    pointer: ValueId(0),
                    element: zst,
                    address_space: AddressSpace::Global,
                    layout: zst.expected_layout(),
                    contract: VolatileAccessContract::zero_sized_aligned_no_access(),
                }),
            ),
            copy(ValueId(0), ValueId(1), ValueId(2), zst),
        ])],
    );
    zst_helper
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));

    let mut kernel = Kernel::new(
        "memory",
        "memory_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::memory_intrinsics");
    module.functions = vec![entry, zst_helper];
    module.kernels = vec![kernel];
    module
}

#[test]
fn gfx942_llvm_retains_distance_volatile_and_copy_semantics() {
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&memory_module()).unwrap();

    assert!(llvm.contains("sub i64 %memory.0.2.pointer, %memory.0.2.origin"));
    assert!(llvm.contains("sdiv exact i64 %memory.0.2.bytes, 4"));
    assert!(llvm.contains("sub nuw i64 %memory.0.3.pointer, %memory.0.3.origin"));
    assert!(llvm.contains("udiv exact i64 %memory.0.3.bytes, 1"));
    assert!(llvm.contains("load volatile i32, ptr addrspace(1)"));
    assert!(llvm.contains("store volatile i32"));
    assert!(llvm.contains("%memory.0.6.bytes = mul nuw i64 %arg2, 4"));
    assert!(llvm.contains(
        "declare void @llvm.memcpy.p1.p1.i64(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i64, i1 immarg)"
    ));
    assert!(llvm.contains("call void @llvm.memcpy.p1.p1.i64(ptr addrspace(1) align 4"));

    let zst_body = llvm
        .split("define internal void @zst_copy")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(!zst_body.contains("memcpy"));
    assert!(!zst_body.contains("mul"));
    assert!(!zst_body.contains("load volatile"));
}

#[test]
fn gfx942_rejects_unrepresentable_address_spaces() {
    let mut module = memory_module();
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[2];
    let OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
        address_space,
        ..
    }) = &mut operation.kind
    else {
        unreachable!()
    };
    *address_space = AddressSpace::Generic;

    let errors = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap_err();
    assert!(errors.to_string().contains("address space"));
}

#[test]
fn gfx942_rejects_contracts_that_do_not_justify_llvm_flags() {
    let mut wrong_order = memory_module();
    let operation = &mut wrong_order.functions[0].body.as_mut().unwrap().blocks[0].operations[3];
    let OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
        contract, ..
    }) = &mut operation.kind
    else {
        unreachable!()
    };
    *contract = PointerDistanceContract::supported_rust(PointerDistanceKind::Signed);
    let errors = lower_compiler_module_to_gfx942_llvm_ir(&wrong_order).unwrap_err();
    assert!(errors.to_string().contains("kind-specific ordering"));

    let mut wrong_volatile_range = memory_module();
    let operation = &mut wrong_volatile_range.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks[0]
        .operations[4];
    let OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad { contract, .. }) =
        &mut operation.kind
    else {
        unreachable!()
    };
    *contract = VolatileAccessContract::rust_allocation_store();
    let errors = lower_compiler_module_to_gfx942_llvm_ir(&wrong_volatile_range).unwrap_err();
    assert!(errors.to_string().contains("readable initialized-element"));

    let mut missing_external_load_isolation = memory_module();
    let operation = &mut missing_external_load_isolation.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks[0]
        .operations[4];
    let OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad { contract, .. }) =
        &mut operation.kind
    else {
        unreachable!()
    };
    *contract = VolatileAccessContract::external_mmio_load();
    contract.external_effect = VolatileExternalEffectContract::NotExternal;
    let errors =
        lower_compiler_module_to_gfx942_llvm_ir(&missing_external_load_isolation).unwrap_err();
    assert!(
        errors
            .to_string()
            .contains("external side-effect isolation")
    );

    let mut missing_external_store_isolation = memory_module();
    let operation = &mut missing_external_store_isolation.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks[0]
        .operations[5];
    let OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
        contract, ..
    }) = &mut operation.kind
    else {
        unreachable!()
    };
    *contract = VolatileAccessContract::external_mmio_store();
    contract.external_effect = VolatileExternalEffectContract::NotExternal;
    let errors =
        lower_compiler_module_to_gfx942_llvm_ir(&missing_external_store_isolation).unwrap_err();
    assert!(
        errors
            .to_string()
            .contains("external side-effect isolation")
    );
}

#[test]
#[ignore = "requires the ROCm LLVM toolchain with gfx942 support"]
fn rocm_compiles_memory_intrinsics_for_gfx942() {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-memory-intrinsics-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let input = directory.join("memory.ll");
    let output = directory.join("memory.hsaco");
    fs::write(
        &input,
        lower_compiler_module_to_gfx942_llvm_ir(&memory_module()).unwrap(),
    )
    .unwrap();

    let clang = std::env::var_os("FE2O3_ROCM_CLANG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/rocm/llvm/bin/clang"));
    let result = Command::new(clang)
        .args([
            "--target=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-nogpulib",
            "-O2",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "clang failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fs::metadata(&output).unwrap().len() > 64);
    fs::remove_dir_all(directory).unwrap();
}
