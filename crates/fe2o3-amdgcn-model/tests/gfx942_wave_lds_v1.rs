use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, lower_kernel_to_gfx942_llvm_ir,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

const KERNEL: &str = "gfx942_wave_lds_v1_hw";

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-wave-lds-v1-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn emit(block: &mut BasicBlock, next: &mut u32, ty: Type, kind: OperationKind) -> ValueId {
    let id = ValueId(*next);
    *next += 1;
    block
        .operations
        .push(Operation::effect_free(ValueDef::new(id, ty), kind));
    id
}

fn store(block: &mut BasicBlock, pointer: ValueId, value: ValueId, space: AddressSpace) {
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer,
            value,
            access: MemoryAccess::new(space, 4),
        },
    ));
}

fn barrier(block: &mut BasicBlock) {
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    ));
}

fn load(block: &mut BasicBlock, next: &mut u32, pointer: ValueId, space: AddressSpace) -> ValueId {
    emit(
        block,
        next,
        Type::Scalar(ScalarType::U32),
        OperationKind::Load {
            pointer,
            access: MemoryAccess::new(space, 4),
        },
    )
}

fn gep(
    block: &mut BasicBlock,
    next: &mut u32,
    base: ValueId,
    offset: ValueId,
    space: AddressSpace,
    access: AccessMode,
) -> ValueId {
    emit(
        block,
        next,
        Type::pointer(Type::Scalar(ScalarType::U32), space, access),
        OperationKind::GetElementPointer { base, offset },
    )
}

fn constant_u32(block: &mut BasicBlock, next: &mut u32, value: u32) -> ValueId {
    emit(
        block,
        next,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(value)),
    )
}

fn constant_index(block: &mut BasicBlock, next: &mut u32, value: u32) -> ValueId {
    emit(
        block,
        next,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(u64::from(value))),
    )
}

fn slice_data(
    block: &mut BasicBlock,
    next: &mut u32,
    slice: ValueId,
    access: AccessMode,
) -> ValueId {
    emit(
        block,
        next,
        Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Global, access),
        OperationKind::SliceData { slice },
    )
}

fn active_wave_lds_module() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let readonly = Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let readwrite = Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    let mut next = 4;

    let values = slice_data(&mut block, &mut next, ValueId(0), AccessMode::ReadOnly);
    let active_flags = slice_data(&mut block, &mut next, ValueId(1), AccessMode::ReadOnly);
    let wave_output = slice_data(&mut block, &mut next, ValueId(2), AccessMode::ReadWrite);
    let workgroup_output = slice_data(&mut block, &mut next, ValueId(3), AccessMode::ReadWrite);

    let rank = emit(
        &mut block,
        &mut next,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    );
    let active_ptr = gep(
        &mut block,
        &mut next,
        active_flags,
        rank,
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let active = load(&mut block, &mut next, active_ptr, AddressSpace::Global);
    let value_ptr = gep(
        &mut block,
        &mut next,
        values,
        rank,
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let value = load(&mut block, &mut next, value_ptr, AddressSpace::Global);
    let zero = constant_u32(&mut block, &mut next, 0);
    let predicate = emit(
        &mut block,
        &mut next,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::NotEqual,
            lhs: active,
            rhs: zero,
        },
    );
    let _mask = emit(
        &mut block,
        &mut next,
        Type::Scalar(ScalarType::U64),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::Ballot { predicate },
            WaveWidth::Wave64,
        )),
    );
    let masked = emit(
        &mut block,
        &mut next,
        u32_ty.clone(),
        OperationKind::Select {
            condition: predicate,
            true_value: value,
            false_value: zero,
        },
    );
    let lane = emit(
        &mut block,
        &mut next,
        u32_ty.clone(),
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::LaneId,
            WaveWidth::Wave64,
        )),
    );
    let mut wave_sum = masked;
    for offset in [32, 16, 8, 4, 2, 1] {
        let offset = constant_u32(&mut block, &mut next, offset);
        let source = emit(
            &mut block,
            &mut next,
            u32_ty.clone(),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: lane,
                rhs: offset,
            },
        );
        let peer = emit(
            &mut block,
            &mut next,
            u32_ty.clone(),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ShuffleIndex {
                    value: wave_sum,
                    source_lane: source,
                    tile_width: 64,
                },
                WaveWidth::Wave64,
            )),
        );
        wave_sum = emit(
            &mut block,
            &mut next,
            u32_ty.clone(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: wave_sum,
                rhs: peer,
            },
        );
    }
    let wave_out = gep(
        &mut block,
        &mut next,
        wave_output,
        rank,
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    store(&mut block, wave_out, wave_sum, AddressSpace::Global);

    let scratch = emit(
        &mut block,
        &mut next,
        Type::pointer(
            u32_ty.clone(),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: u32_ty.clone(),
            extent: WorkgroupMemoryExtent::Static(256),
            alignment: 4,
        }),
    );
    let scratch_slot = gep(
        &mut block,
        &mut next,
        scratch,
        rank,
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    store(&mut block, scratch_slot, masked, AddressSpace::Workgroup);
    barrier(&mut block);

    let mut offset = 128;
    while offset != 0 {
        let offset_value = constant_index(&mut block, &mut next, offset);
        let participates = emit(
            &mut block,
            &mut next,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: rank,
                rhs: offset_value,
            },
        );
        let pair = emit(
            &mut block,
            &mut next,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: rank,
                rhs: offset_value,
            },
        );
        let zero_index = constant_index(&mut block, &mut next, 0);
        let safe_pair = emit(
            &mut block,
            &mut next,
            Type::INDEX,
            OperationKind::Select {
                condition: participates,
                true_value: pair,
                false_value: zero_index,
            },
        );
        let lhs_ptr = gep(
            &mut block,
            &mut next,
            scratch,
            rank,
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let rhs_ptr = gep(
            &mut block,
            &mut next,
            scratch,
            safe_pair,
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let lhs = load(&mut block, &mut next, lhs_ptr, AddressSpace::Workgroup);
        let rhs = load(&mut block, &mut next, rhs_ptr, AddressSpace::Workgroup);
        let sum = emit(
            &mut block,
            &mut next,
            u32_ty.clone(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
            },
        );
        let next_value = emit(
            &mut block,
            &mut next,
            u32_ty.clone(),
            OperationKind::Select {
                condition: participates,
                true_value: sum,
                false_value: lhs,
            },
        );
        barrier(&mut block);
        store(&mut block, lhs_ptr, next_value, AddressSpace::Workgroup);
        barrier(&mut block);
        offset >>= 1;
    }

    let zero_index = constant_index(&mut block, &mut next, 0);
    let result_ptr = gep(
        &mut block,
        &mut next,
        scratch,
        zero_index,
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let workgroup_sum = load(&mut block, &mut next, result_ptr, AddressSpace::Workgroup);
    let workgroup_out = gep(
        &mut block,
        &mut next,
        workgroup_output,
        rank,
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    store(
        &mut block,
        workgroup_out,
        workgroup_sum,
        AddressSpace::Global,
    );
    barrier(&mut block);
    block.terminator = Some(Terminator::Return { values: Vec::new() });

    let mut function = Function::kernel_entry(
        "gfx942_wave_lds_v1_hw_impl",
        Signature::new(
            vec![readonly.clone(), readonly, readwrite.clone(), readwrite],
            Vec::new(),
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function
        .required_capabilities
        .extend(function.derived_capabilities());
    let mut kernel = Kernel::new(
        KERNEL,
        "gfx942_wave_lds_v1_hw_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(256),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
    let mut module = Module::new("tests::gfx942_wave_lds_v1");
    module
        .required_capabilities
        .extend(function.derived_capabilities());
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

#[test]
fn verified_ir_and_llvm_preserve_the_exact_wave_lds_shape() {
    let module = active_wave_lds_module();
    verify_module(&module).expect("wave/LDS hardware module verifies");
    let llvm = lower_kernel_to_llvm_ir(&module, &KernelId::new(KERNEL)).unwrap();
    assert_eq!(llvm.matches("call i64 @llvm.amdgcn.ballot.i64").count(), 1);
    assert_eq!(llvm.matches("call i32 @llvm.amdgcn.ds.bpermute").count(), 6);
    assert_eq!(
        llvm.matches("call void @llvm.amdgcn.s.barrier()").count(),
        18
    );
    assert!(llvm.contains("addrspace(3) global [256 x i32] undef, align 4"));
    assert!(llvm.contains("\"amdgpu-flat-work-group-size\"=\"256,256\""));
    assert!(llvm.contains("\"target-features\"=\"-wavefrontsize32,+wavefrontsize64\""));
}

#[test]
fn exact_target_binding_is_accepted_only_by_the_xnack_minus_lowerer() {
    let mut module = active_wave_lds_module();
    let exact_target = gfx942_xnack_minus_target_capability();
    module.required_capabilities.insert(exact_target.clone());
    module.kernels[0]
        .required_capabilities
        .insert(exact_target.clone());
    module.functions[0]
        .required_capabilities
        .insert(exact_target);
    verify_module(&module).expect("exact target-bound module verifies");

    let generic = lower_kernel_to_gfx942_llvm_ir(&module, &KernelId::new(KERNEL)).unwrap_err();
    assert_eq!(
        generic.diagnostics()[0].code,
        LoweringDiagnosticCode::UnsupportedCapability
    );

    let exact = lower_kernel_to_gfx942_xnack_minus_llvm_ir(&module, &KernelId::new(KERNEL))
        .expect("exact gfx942:xnack- lowerer accepts the retained binding");
    assert!(exact.contains("\"target-cpu\"=\"gfx942\""));
    assert!(exact.contains("-wavefrontsize32,+wavefrontsize64,-xnack"));

    let baseline = lower_kernel_to_llvm_ir(&module, &KernelId::new(KERNEL)).unwrap_err();
    assert_eq!(
        baseline.diagnostics()[0].code,
        LoweringDiagnosticCode::UnsupportedCapability
    );
    assert!(
        baseline.diagnostics()[0]
            .message
            .contains("fe2o3.amdgpu.target")
    );
}

#[test]
fn invalid_lds_alignment_fails_before_llvm() {
    let mut module = active_wave_lds_module();
    let operation = module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find(|operation| matches!(operation.kind, OperationKind::WorkgroupMemory(_)))
        .unwrap();
    let OperationKind::WorkgroupMemory(memory) = &mut operation.kind else {
        unreachable!()
    };
    memory.alignment = 2;
    verify_module(&module).expect("generic IR remains structurally valid");
    let error = lower_kernel_to_llvm_ir(&module, &KernelId::new(KERNEL)).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        LoweringDiagnosticCode::UnsupportedWorkgroupMemory
    );
    assert!(
        error.diagnostics()[0]
            .message
            .contains("requires alignment 4")
    );
}

#[test]
#[ignore = "requires ROCm LLVM, HIP, and a gfx942:xnack- GPU"]
fn gfx942_xnack_minus_hardware_executes_masked_wave_and_lds_reductions() {
    let directory = TemporaryDirectory::new();
    let input = directory.join("wave_lds.ll");
    let assembly = directory.join("wave_lds.s");
    let hsaco = directory.join("wave_lds.hsaco");
    let runner = directory.join("wave_lds_runner");
    let module = active_wave_lds_module();
    verify_module(&module).unwrap();
    let llvm = lower_kernel_to_llvm_ir(&module, &KernelId::new(KERNEL)).unwrap();
    fs::write(&input, llvm).unwrap();

    let clang = PathBuf::from("/opt/rocm/llvm/bin/clang");
    let compile = Command::new(&clang)
        .args([
            "--target=amdgcn-amd-amdhsa",
            "-mcpu=gfx942:xnack-",
            "-nogpulib",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&hsaco)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "clang/LLD failed:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let assemble = Command::new(&clang)
        .args([
            "--target=amdgcn-amd-amdhsa",
            "-mcpu=gfx942:xnack-",
            "-nogpulib",
            "-S",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&assembly)
        .output()
        .unwrap();
    assert!(assemble.status.success());
    let assembly = fs::read_to_string(&assembly).unwrap();
    assert!(assembly.contains("ds_bpermute_b32"));
    assert!(assembly.contains("s_barrier"));
    assert!(assembly.contains("ds_write_b32"));
    assert!(assembly.contains("ds_read_b32"));

    let readobj = Command::new("/opt/rocm/llvm/bin/llvm-readobj")
        .args(["--file-headers", "--notes"])
        .arg(&hsaco)
        .output()
        .unwrap();
    assert!(readobj.status.success());
    let report = String::from_utf8(readobj.stdout).unwrap();
    assert!(report.contains("EF_AMDGPU_FEATURE_XNACK_OFF_V4"));
    assert!(report.contains(".group_segment_fixed_size: 1024"));

    let hipcc = Command::new("/opt/rocm/bin/hipcc")
        .arg("-O2")
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/gfx942_wave_lds_v1_runner.cpp"),
        )
        .arg("-o")
        .arg(&runner)
        .output()
        .unwrap();
    assert!(
        hipcc.status.success(),
        "hipcc failed:\n{}",
        String::from_utf8_lossy(&hipcc.stderr)
    );
    let execution = Command::new(&runner)
        .arg(&hsaco)
        .env("HSA_XNACK", "0")
        .output()
        .unwrap();
    assert!(
        execution.status.success(),
        "hardware runner failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
    assert!(String::from_utf8_lossy(&execution.stdout).contains("PASS gfx942 wave/LDS V1"));
}
