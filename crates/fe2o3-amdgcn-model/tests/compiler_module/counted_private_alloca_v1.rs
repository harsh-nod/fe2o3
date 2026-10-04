use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::{
    LoweringErrors, bind_production_target_v1,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as native_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as native_950,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CastKind, Constant, MemoryAccess, ScalarType, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::io::Write;
use std::process::{Command, Stdio};

fn scalar(scalar: ScalarType) -> Type {
    Type::Scalar(scalar)
}

fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}

fn allocation(element: Type, count: Option<ValueId>, alignment: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(1),
            Type::pointer(
                element.clone(),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element,
            count,
            address_space: AddressSpace::Private,
            alignment,
        },
    )
}

fn module_with_count(count: Constant, element: Type, alignment: u32) -> Module {
    let mut module = Module::new("counted-private-alloca");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![returning_block(
            vec![
                constant(0, count),
                allocation(element, Some(ValueId(0)), alignment),
            ],
            vec![],
        )],
    ));
    module.kernels.push(kernel("kernel", "entry", 64));
    module
}

fn lower(module: &Module, profile: Profile) -> Result<String, LoweringErrors> {
    let bound = bind_production_target_v1(module, profile).expect("genuine verified target module");
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    const PREFIX: usize = 37;
    budget.reserve_storage(PREFIX).unwrap();
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget).unwrap();
    let retained = receipt.retained_storage();
    budget.reserve_storage(retained).unwrap();
    let before_storage = budget.storage();
    let before_work = budget.work();
    let result = match profile {
        Profile::Gfx942 => native_942(&owner),
        Profile::Gfx950 => native_950(&owner),
    };
    if let Ok(llvm) = &result {
        let identity = owner.canonical().identity();
        let digest = identity
            .digest()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let target = match profile {
            Profile::Gfx942 => "gfx942:xnack-",
            Profile::Gfx950 => "gfx950:xnack-",
        };
        assert!(llvm.contains(&format!(
            "!\"sha256:{digest}\", !\"kir-version:12\", i64 {}, !\"target:{target}\"",
            identity.canonical_length()
        )));
    }
    assert_eq!(budget.storage(), before_storage);
    assert_eq!(budget.work(), before_work);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), PREFIX);
    result
}

fn assert_refused(module: &Module, detail: &str) {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let error = lower(module, profile).unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::UnsupportedOperation));
        assert!(error.to_string().contains(detail), "{error}");
    }
}

#[test]
fn direct_unsigned_counts_preserve_the_actual_llvm_count_type_on_both_targets() {
    for (count, ty) in [
        (Constant::U8(4), "i8"),
        (Constant::U16(4), "i16"),
        (Constant::U32(4), "i32"),
        (Constant::U64(4), "i64"),
        (Constant::Index(4), "i64"),
    ] {
        let module = module_with_count(count, scalar(ScalarType::U32), 4);
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let llvm = lower(&module, profile).unwrap();
            assert_eq!(llvm.matches(" = alloca ").count(), 1);
            assert!(llvm.contains(&format!("%v1 = alloca i32, {ty} 4, align 4, addrspace(5)")));
        }
    }
}

fn first_last_memory_module() -> Module {
    let mut module = module_with_count(Constant::Index(4), scalar(ScalarType::U32), 16);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.push(constant(2, Constant::Index(0)));
    block.operations.push(constant(3, Constant::Index(3)));
    block.operations.push(constant(4, Constant::U32(42)));
    for (pointer, index, loaded) in [(5, 2, 7), (6, 3, 8)] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(pointer),
                Type::pointer(
                    scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::GetElementPointer {
                base: ValueId(1),
                offset: ValueId(index),
            },
        ));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(pointer),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(loaded), scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(pointer),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ));
    }
    module
}

fn assemble(llvm: &str) {
    let mut child = Command::new("llvm-as")
        .arg("-o")
        .arg("/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("LLVM assembler is available");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(llvm.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn counted_private_first_last_gep_store_load_assembles_without_address_space_changes() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let llvm = lower(&first_last_memory_module(), profile).unwrap();
        assert!(llvm.contains("alloca i32, i64 4, align 16, addrspace(5)"));
        assert!(llvm.contains("getelementptr i32, ptr addrspace(5) %v1, i64 0"));
        assert!(llvm.contains("getelementptr i32, ptr addrspace(5) %v1, i64 3"));
        assert_eq!(llvm.matches("store i32 42, ptr addrspace(5)").count(), 2);
        assert_eq!(llvm.matches("load i32, ptr addrspace(5)").count(), 2);
        assemble(&llvm);
    }
}

#[test]
fn dominating_constant_in_nonlexical_storage_order_does_not_hoist_the_allocation() {
    let mut module = module_with_count(Constant::Index(4), scalar(ScalarType::U32), 4);
    let mut entry = BasicBlock::new(BlockId(90));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(60),
        arguments: vec![],
    });
    let mut definition = BasicBlock::new(BlockId(60));
    definition.operations.push(constant(0, Constant::Index(4)));
    definition.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let mut use_block = BasicBlock::new(BlockId(7));
    use_block
        .operations
        .push(allocation(scalar(ScalarType::U32), Some(ValueId(0)), 4));
    use_block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions[0].body.as_mut().unwrap().blocks = vec![entry, use_block, definition];
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let llvm = lower(&module, profile).unwrap();
        let entry = llvm.find("bb90:").unwrap();
        let site = llvm.find("bb7:").unwrap();
        let allocation = llvm.find("%v1 = alloca i32, i64 4").unwrap();
        let definition = llvm.find("bb60:").unwrap();
        assert!(entry < site && site < allocation && allocation < definition);
        assert!(!llvm[entry..site].contains("alloca"));
        assemble(&llvm);
    }
}

#[test]
fn helper_counted_alloca_preserves_explicit_multi_body_anchor_absence() {
    let mut module = first_last_memory_module();
    let body = module.functions.remove(0).body.unwrap();
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        body.parameters,
        body.blocks,
    ));
    module.functions.push(void_entry("entry", &["helper"]));
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let llvm = lower(&module, profile).unwrap();
        assert!(llvm.contains("alloca i32, i64 4, align 16, addrspace(5)"));
        assert!(llvm.contains("!\"multiple_defined_bodies\""));
        assert!(!llvm.contains("llvm.pseudoprobe"));
        assemble(&llvm);
    }
}

#[test]
fn uncounted_scalar_and_pointer_allocations_keep_the_historical_spelling() {
    for (element, alignment, llvm_type) in [
        (scalar(ScalarType::U32), 4, "i32"),
        (
            Type::pointer(Type::Unit, AddressSpace::Global, AccessMode::ReadWrite),
            8,
            "ptr addrspace(1)",
        ),
    ] {
        let mut module = module_with_count(Constant::Index(4), element.clone(), alignment);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations =
            vec![allocation(element, None, alignment)];
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let llvm = lower(&module, profile).unwrap();
            assert!(llvm.contains(&format!(
                "%v1 = alloca {llvm_type}, align {alignment}, addrspace(5)"
            )));
            assert!(!llvm.contains(&format!("alloca {llvm_type}, i")));
        }
    }
}

#[test]
fn positive_byte_span_is_checked_without_claiming_device_scratch_capacity() {
    for (scalar_type, bytes) in [
        (ScalarType::U8, 1_u64),
        (ScalarType::U32, 4),
        (ScalarType::U64, 8),
    ] {
        let max_count = (i32::MAX as u64) / bytes;
        let accepted = module_with_count(
            Constant::U64(max_count),
            scalar(scalar_type),
            u32::try_from(bytes).unwrap(),
        );
        let over = module_with_count(
            Constant::U64(max_count + 1),
            scalar(scalar_type),
            u32::try_from(bytes).unwrap(),
        );
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            assert!(
                lower(&accepted, profile)
                    .unwrap()
                    .contains(&format!("i64 {max_count}, align {bytes}"))
            );
        }
        assert_refused(&over, "positive span within the 32-bit private index range");
    }
    for count in [0, u64::MAX] {
        assert_refused(
            &module_with_count(Constant::U64(count), scalar(ScalarType::U64), 8),
            "positive span within the 32-bit private index range",
        );
    }
}

#[test]
fn signed_constant_dynamic_phi_cast_and_arithmetic_counts_are_not_inferred() {
    let detail = "requires a direct unsigned integer constant";
    assert_refused(
        &module_with_count(Constant::I32(4), scalar(ScalarType::U32), 4),
        detail,
    );
    let mut dynamic = module_with_count(Constant::Index(4), scalar(ScalarType::U32), 4);
    dynamic.functions[0].signature.parameters.push(Type::INDEX);
    let body = dynamic.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(0));
    body.blocks[0].operations.remove(0);
    assert_refused(&dynamic, detail);

    let mut phi = module_with_count(Constant::Index(4), scalar(ScalarType::U32), 4);
    let body = phi.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.pop();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut target = BasicBlock::new(BlockId(1));
    target
        .parameters
        .push(ValueDef::new(ValueId(2), Type::INDEX));
    target
        .operations
        .push(allocation(scalar(ScalarType::U32), Some(ValueId(2)), 4));
    target.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(target);
    assert_refused(&phi, detail);

    let mut cast = module_with_count(Constant::U64(4), scalar(ScalarType::U32), 4);
    cast.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        constant(0, Constant::U64(4)),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: Type::INDEX,
            },
        ),
        allocation(scalar(ScalarType::U32), Some(ValueId(2)), 4),
    ];
    assert_refused(&cast, detail);
    let mut computed = module_with_count(Constant::U32(2), scalar(ScalarType::U32), 4);
    computed.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        constant(0, Constant::U32(2)),
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ),
        allocation(scalar(ScalarType::U32), Some(ValueId(2)), 4),
    ];
    assert_refused(&computed, detail);
}

#[test]
fn pointer_elements_unsupported_scalars_spaces_and_insufficient_alignment_stay_refused() {
    for element in [
        Type::pointer(Type::Unit, AddressSpace::Global, AccessMode::ReadWrite),
        scalar(ScalarType::Bool),
        scalar(ScalarType::F64),
    ] {
        let module = module_with_count(Constant::Index(4), element, 8);
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let error = lower(&module, profile).unwrap_err();
            assert!(
                error.contains(LoweringDiagnosticCode::UnsupportedType),
                "{error}"
            );
        }
    }
    assert_refused(
        &module_with_count(Constant::Index(4), scalar(ScalarType::U32), 1),
        "requires alignment 4, found 1",
    );
    let mut workgroup = module_with_count(Constant::Index(4), scalar(ScalarType::U32), 4);
    let operation = &mut workgroup.functions[0].body.as_mut().unwrap().blocks[0].operations[1];
    operation.results[0].ty = Type::pointer(
        scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let OperationKind::Alloca { address_space, .. } = &mut operation.kind else {
        unreachable!()
    };
    *address_space = AddressSpace::Workgroup;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let error = lower(&workgroup, profile).unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::UnsupportedWorkgroupMemory));
        assert!(
            error
                .to_string()
                .contains("workgroup Alloca is ambiguous; use explicit WorkgroupMemory")
        );
    }
}

#[test]
fn narrow_float_counted_allocations_reuse_existing_target_and_capability_gates() {
    for (element, capability, llvm_type) in [
        (ScalarType::F16, TargetCapability::Float16, "i16"),
        (ScalarType::Bf16, TargetCapability::BFloat16, "i16"),
    ] {
        let mut module = module_with_count(Constant::Index(4), scalar(element), 2);
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            assert!(
                lower(&module, profile)
                    .unwrap_err()
                    .contains(LoweringDiagnosticCode::UnsupportedCapability)
            );
        }
        module.required_capabilities.insert(capability);
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            assert!(
                lower(&module, profile)
                    .unwrap()
                    .contains(&format!("alloca {llvm_type}, i64 4, align 2, addrspace(5)"))
            );
        }
        assert!(lower_compiler_module_to_llvm_ir(&module).is_err());
    }
}

#[test]
fn malformed_count_result_type_is_rejected_by_real_admission_before_lowering() {
    let mut module = module_with_count(Constant::U64(4), scalar(ScalarType::U32), 4);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty = Type::INDEX;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        assert!(bind_production_target_v1(&module, profile).is_err());
    }
}
