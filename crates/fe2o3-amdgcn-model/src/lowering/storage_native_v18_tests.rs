use super::*;
pub(crate) use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
pub(crate) use fe2o3_kernel_ir::{
    AddressSpace as Space, CanonicalKernelIrWorkBudgetV1 as Work, MemoryAccess,
    StorageFieldV1 as Field, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind,
    StorageLayoutLimitsV1, StorageLayoutV1 as Row, StorageOperationV1 as Storage, StoragePointerV1,
    StorageProjectionV1 as Projection, StorageVariantEncodingV1 as Encoding,
    StorageVariantV1 as Variant, ValueDef, WorkgroupMemory,
};

pub(crate) fn scalar() -> Row {
    Row {
        size: 4,
        alignment: 4,
        kind: Kind::Scalar(ScalarType::U32),
    }
}
pub(crate) fn object(row: u32) -> Type {
    Type::StorageObject(Id(row))
}
pub(crate) fn address(row: u32, space: Space) -> Type {
    Type::pointer(object(row), space, AccessMode::ReadWrite)
}
pub(crate) fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}
pub(crate) fn constant(id: u32, bits: u32) -> Operation {
    value(
        id,
        Type::Scalar(ScalarType::U32),
        OperationKind::Constant(Constant::U32(bits)),
    )
}
pub(crate) fn allocate(id: u32, row: u32, space: Space, alignment: u32) -> Operation {
    value(
        id,
        address(row, space),
        if space == Space::Workgroup {
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: object(row),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment,
            })
        } else {
            OperationKind::Alloca {
                element: object(row),
                count: None,
                address_space: space,
                alignment,
            }
        },
    )
}
pub(crate) fn project(id: u32, row: u32, base: u32, step: Projection, space: Space) -> Operation {
    value(
        id,
        address(row, space),
        OperationKind::Storage(Storage::Project {
            base: ValueId(base),
            step,
        }),
    )
}
pub(crate) fn write(address: u32, input: u32, space: Space, alignment: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Storage(Storage::WriteValue {
            address: ValueId(address),
            value: ValueId(input),
            access: MemoryAccess::new(space, alignment),
        }),
    )
}
pub(crate) fn read(id: u32, address: u32, ty: Type, space: Space, alignment: u32) -> Operation {
    value(
        id,
        ty,
        OperationKind::Storage(Storage::ReadValue {
            address: ValueId(address),
            access: MemoryAccess::new(space, alignment),
        }),
    )
}
pub(crate) fn module(rows: Vec<Row>, operations: Vec<Operation>, profile: Profile) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("storage_target");
    module.storage_layouts = rows;
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    bind(&mut module, profile);
    module
}
pub(crate) fn bind(module: &mut Module, profile: Profile) {
    let mut required = BTreeSet::from([
        TargetCapability::WaveWidth(WaveWidth::Wave64),
        match profile {
            Profile::Gfx942 => fe2o3_kernel_ir::gfx942_xnack_minus_target_capability(),
            Profile::Gfx950 => fe2o3_kernel_ir::gfx950_xnack_minus_target_capability(),
        },
    ]);
    for f in &module.functions {
        if let Some(body) = &f.body {
            for b in &body.blocks {
                for op in &b.operations {
                    required.extend(op.required_capabilities());
                }
            }
        }
    }
    module.required_capabilities = required.clone();
    module.kernels[0].required_capabilities = required.clone();
    module.functions[0].required_capabilities = required;
}
pub(crate) fn with_owner<T>(module: &Module, f: impl FnOnce(&Owner, &mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    budget.reserve_storage(17).unwrap();
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v18(
        module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .expect("actual canonical V18 admission");
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    let result = f(&owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 17);
    result
}
pub(crate) fn emit(module: &Module, profile: Profile) -> String {
    let text = with_owner(module, |owner, budget| {
        lower_canonical_storage_module_v18(owner, profile, budget)
            .expect("actual shared target emitter")
    });
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("llvm-as")
        .args(["-", "-o", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("llvm-as is required by AMDGPU model tests");
    let written = child.stdin.take().unwrap().write_all(text.as_bytes());
    let output = child.wait_with_output().expect("wait for llvm-as");
    assert!(
        written.is_ok() && output.status.success(),
        "LLVM rejected emitted storage IR: {}\n{text}",
        String::from_utf8_lossy(&output.stderr)
    );
    text
}

#[test]
fn actual_v18_owner_emits_both_exact_worker_profiles_and_not_legacy() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let module = module(
            vec![scalar()],
            vec![allocate(0, 0, Space::Private, 4)],
            profile,
        );
        let text = emit(&module, profile);
        assert!(text.contains("%v0 = alloca [4 x i8], align 4, addrspace(5)"));
        assert!(text.contains(fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1));
        assert!(text.contains(profile.cpu()));
        assert!(lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(&module).is_err());
        assert!(v12_preflight::reject_unsupported_v12_module(&module).is_err());
    }
}

#[test]
fn storage_context_and_anchors_refuse_foreign_equal_bytes_owner() {
    let module = module(vec![scalar()], vec![], Profile::Gfx942);
    with_owner(&module, |first, _| {
        with_owner(&module, |second, budget| {
            assert_eq!(first.canonical_bytes(), second.canonical_bytes());
            let meter = MeterV18::new(budget).unwrap();
            let context = StorageEmissionContextV18 {
                owner: first,
                target: LoweringTarget::Gfx942XnackMinusV1,
                meter: &meter,
                root_roles: None,
            };
            assert!(
                context
                    .check_current(second.module(), context.target)
                    .is_err()
            );
            assert!(
                SemanticAnchorInputV1::Storage(first)
                    .validate(second.module())
                    .is_err()
            );
            assert!(
                context
                    .check_current(first.module(), LoweringTarget::Gfx950XnackMinusV1)
                    .is_err()
            );
        })
    });
}

#[test]
fn exact_target_capability_disagreement_refuses_before_output() {
    let module = module(vec![scalar()], vec![], Profile::Gfx942);
    with_owner(&module, |owner, budget| {
        assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx950, budget).is_err())
    });
}

#[test]
fn unused_unsupported_layout_and_external_signature_are_not_skipped() {
    let mut module = module(
        vec![Row {
            size: 8,
            alignment: 8,
            kind: Kind::Scalar(ScalarType::F64),
        }],
        vec![],
        Profile::Gfx942,
    );
    with_owner(&module, |owner, budget| {
        assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx942, budget).is_err())
    });
    module.storage_layouts = vec![scalar()];
    let mut declaration = Function::definition(
        "foreign",
        Signature::new(vec![address(0, Space::Global)], vec![]),
        vec![],
        vec![],
    );
    declaration.body = None;
    declaration.role = FunctionRole::ExternalImport;
    module.functions.push(declaration);
    with_owner(&module, |owner, budget| {
        assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx942, budget).is_err())
    });
}

#[test]
fn zero_sized_object_has_an_explicit_target_refusal() {
    let module = module(
        vec![Row {
            size: 0,
            alignment: 1,
            kind: Kind::Record(vec![].into_boxed_slice()),
        }],
        vec![],
        Profile::Gfx942,
    );
    with_owner(&module, |owner, budget| {
        assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx942, budget).is_err())
    });
}

#[test]
fn authenticated_dynamic_lds_minimum_is_not_silently_erased_by_the_wire() {
    let mut allocation = allocate(0, 0, Space::Workgroup, 4);
    let OperationKind::WorkgroupMemory(memory) = &mut allocation.kind else {
        unreachable!()
    };
    memory.extent = WorkgroupMemoryExtent::DynamicAtLeast(1);
    let module = module(vec![scalar()], vec![allocation], Profile::Gfx942);
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    let result = Owner::from_module_ref_with_verification_budget_v18(
        &module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Encode(
                fe2o3_kernel_ir::KernelIrEncodeError::UnsupportedInVersion {
                    version: 18,
                    feature: "authenticated dynamic workgroup-memory extent",
                }
            )
        )
    ));
}
