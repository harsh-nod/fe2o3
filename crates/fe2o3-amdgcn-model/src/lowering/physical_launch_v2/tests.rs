use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    BarrierSemantics, Convergence, IntrinsicOperation, ValueDef, WorkgroupBarrier,
};

fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn constant(id: u32, n: u64) -> Operation {
    value(id, Type::INDEX, OperationKind::Constant(Constant::Index(n)))
}
fn branch(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn module() -> Module {
    let mut module = Module::new("physical_target");
    for name in ["first", "second"] {
        let mut function = Function::kernel_entry(
            name,
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(0)],
            vec![
                block(
                    0,
                    vec![
                        constant(1, 46080),
                        value(
                            2,
                            Type::BOOL,
                            OperationKind::Compare {
                                predicate: ComparePredicate::GreaterThanOrEqual,
                                lhs: ValueId(0),
                                rhs: ValueId(1),
                            },
                        ),
                    ],
                    branch(2, 1, 9),
                ),
                block(
                    1,
                    vec![
                        value(
                            3,
                            Type::INDEX,
                            OperationKind::Intrinsic(IntrinsicOperation::new(
                                IntrinsicKind::InvocationIndex {
                                    kind: IndexKind::Global,
                                    axis: Axis::X,
                                },
                                Type::INDEX,
                            )),
                        ),
                        constant(4, 64),
                        constant(5, 2880),
                        value(
                            6,
                            Type::INDEX,
                            OperationKind::Binary {
                                op: BinaryOp::Divide,
                                lhs: ValueId(3),
                                rhs: ValueId(4),
                            },
                        ),
                        value(
                            7,
                            Type::INDEX,
                            OperationKind::Binary {
                                op: BinaryOp::Multiply,
                                lhs: ValueId(6),
                                rhs: ValueId(5),
                            },
                        ),
                        value(
                            8,
                            Type::INDEX,
                            OperationKind::Binary {
                                op: BinaryOp::Add,
                                lhs: ValueId(7),
                                rhs: ValueId(5),
                            },
                        ),
                        value(
                            9,
                            Type::BOOL,
                            OperationKind::Compare {
                                predicate: ComparePredicate::LessThanOrEqual,
                                lhs: ValueId(8),
                                rhs: ValueId(0),
                            },
                        ),
                    ],
                    branch(9, 2, 9),
                ),
                block(
                    2,
                    vec![Operation::new(
                        vec![],
                        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                            memory_scope: SynchronizationScope::Workgroup,
                            semantics: BarrierSemantics::new(
                                MemoryOrdering::AcquireRelease,
                                [KernelAddressSpace::Workgroup],
                            ),
                            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
                        }),
                    )],
                    Terminator::Return { values: vec![] },
                ),
                block(9, vec![], Terminator::Return { values: vec![] }),
            ],
        );
        function.required_capabilities.extend([
            TargetCapability::WorkgroupBarrier,
            TargetCapability::WorkgroupMemory,
        ]);
        let mut kernel = Kernel::new(
            name,
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
        kernel.required_capabilities.extend([
            TargetCapability::WorkgroupBarrier,
            TargetCapability::WorkgroupMemory,
        ]);
        module.functions.push(function);
        module.kernels.push(kernel);
    }
    module.required_capabilities.extend([
        TargetCapability::WorkgroupBarrier,
        TargetCapability::WorkgroupMemory,
    ]);
    module
}
fn identity(module: &Module) -> ProductionSemanticAnchorKirIdentityV1 {
    let owner = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    ProductionSemanticAnchorKirIdentityV1::from_v8(&owner)
}
fn context(module: &Module, profile: Profile, grid: u32) -> CompilerPhysicalLaunchV2<'_> {
    CompilerPhysicalLaunchV2::new(
        module,
        profile,
        &module
            .kernels
            .iter()
            .map(|kernel| (kernel, [grid, 1, 1]))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn exact_physical_context_reaches_actual_target_gate_for_both_profiles_and_all_roots() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let owner = crate::bind_production_target_v1(&module(), profile).unwrap();
        let module = owner.module();
        let identity = identity(module);
        let physical = context(module, profile, 4);
        let output = lower_compiler_module_to_xnack_minus_llvm_ir_with_physical_launch_v2(
            module, identity, &physical,
        )
        .unwrap();
        assert!(output.contains("@first("));
        assert!(output.contains("@second("));
        assert_eq!(
            output
                .matches("call void asm sideeffect \"s_barrier\", \"\"()")
                .count(),
            2
        );
        let old = match profile {
            Profile::Gfx942 => {
                lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                    module, identity,
                )
            }
            Profile::Gfx950 => {
                lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                    module, identity,
                )
            }
        }
        .unwrap_err();
        assert!(old.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence));
        let extra = context(module, profile, 5);
        let error = lower_compiler_module_to_xnack_minus_llvm_ir_with_physical_launch_v2(
            module, identity, &extra,
        )
        .unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence));
    }
}

#[test]
fn physical_roster_refuses_missing_duplicate_swapped_and_equal_byte_foreign_kernels() {
    let module = module();
    let copy = module.clone();
    let profile = Profile::Gfx942;
    for rows in [
        vec![],
        vec![(&module.kernels[0], [4, 1, 1])],
        vec![
            (&module.kernels[0], [4, 1, 1]),
            (&module.kernels[0], [4, 1, 1]),
        ],
        vec![
            (&module.kernels[1], [4, 1, 1]),
            (&module.kernels[0], [4, 1, 1]),
        ],
        vec![(&copy.kernels[0], [4, 1, 1]), (&copy.kernels[1], [4, 1, 1])],
    ] {
        assert!(CompilerPhysicalLaunchV2::new(&module, profile, &rows).is_err());
    }
    let physical = context(&module, profile, 4);
    assert!(
        physical
            .check(&copy, LoweringTarget::Gfx942XnackMinusV1)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InvalidLaunchPolicy)
    );
    assert!(
        physical
            .check(&module, LoweringTarget::Gfx950XnackMinusV1)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InvalidLaunchPolicy)
    );
}

#[test]
fn physical_context_does_not_reinterpret_padded_static_or_higher_rank_as_smaller_dispatch() {
    for extent in [1, 1024] {
        let mut source = module();
        for kernel in &mut source.kernels {
            kernel.domain = LaunchDomain::D1 {
                x: LaunchExtent::Static(extent),
            };
        }
        let owner = crate::bind_production_target_v1(&source, Profile::Gfx942).unwrap();
        let module = owner.module();
        let error = lower_compiler_module_to_xnack_minus_llvm_ir_with_physical_launch_v2(
            module,
            identity(module),
            &context(module, Profile::Gfx942, 5),
        )
        .unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::UnprovenBarrierConvergence));
    }
    let mut module = module();
    for kernel in &mut module.kernels {
        kernel.domain = LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
        };
    }
    let physical = context(&module, Profile::Gfx942, 4);
    assert!(physical.entries.iter().all(|(_, entry)| entry.is_none()));
}
