use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1 as Capability,
    ExecutionCapabilityProvenanceV1, ExecutionCapabilityRoleV1 as Role,
    ExecutionCapabilitySignatureV1, ExecutionCapabilitySourceV1, ExecutionCapabilityTypeV1,
    ExecutionCollectiveKindV1 as Collective, ExecutionElementLayoutV1, ExecutionLdsStateV1,
    ExecutionSafetyObligationsV1, ExecutionTypeIdentityV1, Kernel, KernelContextSourceIdentityV1,
    KernelContextTypeV1, LaunchDomain, LaunchExtent, VerifiedCanonicalKernelIrV13,
    required_execution_obligations_v1,
};

fn id(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn capability_type(source: u8, epoch: u8, role: Role) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: id(source),
        provenance: provenance(),
        workgroup_brand: Some([9; 32]),
        epoch: Some([epoch; 32]),
        role,
    })
}

fn capability(
    operation: Capability,
    operands: &[u32],
    arguments: &[u8],
    output: u8,
    source: u8,
    after: Option<u8>,
    results: Vec<ValueDef>,
) -> Operation {
    Operation::new(
        results,
        OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operands: operands.iter().copied().map(ValueId).collect(),
            signature: ExecutionCapabilitySignatureV1::new(
                &arguments.iter().copied().map(id).collect::<Vec<_>>(),
                id(output),
            )
            .unwrap(),
            provenance: provenance(),
            workgroup_brand: Some([9; 32]),
            epoch_before: Some([10; 32]),
            epoch_after: after.map(|epoch| [epoch; 32]),
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [20; 32],
                operation: [source; 32],
                block: 0,
                occurrence: None,
            },
            operation,
        }),
    )
}

fn admit(mut module: Module) -> Module {
    let requirements = module.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .flat_map(Operation::required_capabilities)
        .collect::<BTreeSet<_>>();
    module.functions[0].required_capabilities = requirements.clone();
    module.kernels[0].required_capabilities = requirements.clone();
    module.required_capabilities = requirements;
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    module
}

// These inert KIR fixtures test analysis, not production source or machine custody.
fn collective_module(scope: SynchronizationScope, kind: Collective, scalar: ScalarType) -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let lds_role = Role::Lds {
        element: id(6),
        layout,
        elements: 64,
        state: ExecutionLdsStateV1::Uninitialized,
    };
    let p = provenance();
    let mut block = returning(0);
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(1),
            KernelContextTypeV1::new("entry", p.kernel_marker, p.target_brand, p.launch_brand),
            KernelContextSourceIdentityV1::new([21; 32], [22; 32], [23; 32], [24; 32]),
        ),
        capability(
            Capability::WorkgroupDerive {
                context: id(1),
                workgroup: id(2),
            },
            &[1],
            &[1],
            2,
            30,
            None,
            vec![ValueDef::new(
                ValueId(2),
                capability_type(2, 10, Role::Workgroup),
            )],
        ),
    ];
    match scope {
        SynchronizationScope::Subgroup => {
            block.operations.push(capability(
                Capability::SubgroupDerive {
                    workgroup: id(2),
                    subgroup: id(3),
                    width: 64,
                },
                &[2],
                &[2],
                3,
                31,
                None,
                vec![ValueDef::new(
                    ValueId(3),
                    capability_type(3, 10, Role::Subgroup { width: 64 }),
                )],
            ));
            block.operations.push(capability(
                Capability::SubgroupCollective {
                    kind,
                    subgroup_reference: id(4),
                    subgroup: id(3),
                    epoch: id(5),
                    element: id(6),
                    value_type: scalar,
                    width: 64,
                },
                &[3, 0],
                &[4, 5, 6],
                6,
                32,
                None,
                vec![ValueDef::new(ValueId(6), Type::Scalar(scalar))],
            ));
        }
        SynchronizationScope::Workgroup => {
            block.operations.push(capability(
                Capability::LdsAllocate {
                    workgroup: id(2),
                    lds: id(3),
                    element: id(6),
                    layout,
                    elements: 64,
                },
                &[2],
                &[2],
                3,
                31,
                None,
                vec![ValueDef::new(
                    ValueId(3),
                    capability_type(3, 10, lds_role.clone()),
                )],
            ));
            block.operations.push(capability(
                Capability::WorkgroupCollective {
                    kind,
                    input_workgroup: id(2),
                    scratch: id(3),
                    element: id(6),
                    transition: id(7),
                    value_type: scalar,
                    layout,
                    elements: 64,
                },
                &[2, 3, 0],
                &[2, 3, 6],
                7,
                32,
                Some(11),
                vec![
                    ValueDef::new(ValueId(4), capability_type(7, 11, Role::Workgroup)),
                    ValueDef::new(ValueId(5), capability_type(7, 11, lds_role)),
                    ValueDef::new(ValueId(6), Type::Scalar(scalar)),
                ],
            ));
        }
        _ => panic!("fixture supports only exact workgroup or subgroup collectives"),
    }
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(scalar)], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut kernel = Kernel::new(
        "collective",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("collective_uniformity");
    module.functions.push(entry);
    module.kernels.push(kernel);
    admit(module)
}

#[test]
fn raw_shuffle_requires_full_width_and_uniform_selector() {
    let variations = [
        Variation::GridUniform,
        Variation::WorkgroupUniform,
        Variation::SubgroupUniform,
        Variation::Varying,
    ];
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile in [1, 16, width.lanes()] {
            for scalar in [ScalarType::I32, ScalarType::U32] {
                let mut block = returning(0);
                block.operations = vec![
                    constant(2, Constant::U32(tile - 1)),
                    result(
                        3,
                        Type::Scalar(ScalarType::U32),
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            lhs: ValueId(1),
                            rhs: ValueId(2),
                        },
                    ),
                    result(
                        4,
                        Type::Scalar(scalar),
                        OperationKind::Wave(WaveOperation::full(
                            WaveOperationKind::ShuffleIndex {
                                value: ValueId(0),
                                source_lane: ValueId(3),
                                tile_width: tile,
                            },
                            width,
                        )),
                    ),
                ];
                let function = checked_function(
                    vec![Type::Scalar(scalar), Type::Scalar(ScalarType::U32)],
                    vec![block],
                );
                for value in variations {
                    for selector in variations {
                        let expected = if value.is_uniform_for(SynchronizationScope::Subgroup) {
                            value
                        } else if tile == width.lanes()
                            && selector.is_uniform_for(SynchronizationScope::Subgroup)
                        {
                            Variation::SubgroupUniform
                        } else {
                            Variation::Varying
                        };
                        let report = analyze_function_with_contract(
                            &function,
                            &[value, selector],
                            &BTreeSet::new(),
                            &BTreeSet::new(),
                            None,
                        );
                        assert_eq!(
                            report.value(ValueId(4)),
                            expected,
                            "{width:?}/{tile}/{scalar:?}/{value:?}/{selector:?}"
                        );
                        assert!(report.diagnostics().is_empty());
                    }
                }
            }
        }
    }
}

fn branch_and_barrier(
    block: &mut BasicBlock,
    condition: ValueId,
    scope: SynchronizationScope,
) -> Vec<BasicBlock> {
    block.terminator = Some(Terminator::ConditionalBranch {
        condition,
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut conditional = returning(1);
    conditional.operations.push(Operation::new(
        vec![],
        OperationKind::Barrier(Barrier {
            execution_scope: scope,
            memory_scope: scope,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
        }),
    ));
    vec![conditional, returning(2)]
}

#[test]
fn raw_partial_shuffle_rejects_physical_subgroup_convergence() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        let mut block = returning(0);
        block.operations = vec![
            result(
                0,
                Type::Scalar(ScalarType::U32),
                OperationKind::Wave(WaveOperation::full(WaveOperationKind::LaneId, width)),
            ),
            constant(1, Constant::U32(0)),
            result(
                2,
                Type::Scalar(ScalarType::U32),
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::ShuffleIndex {
                        value: ValueId(0),
                        source_lane: ValueId(1),
                        tile_width: 16,
                    },
                    width,
                )),
            ),
            constant(3, Constant::U32(16)),
            result(
                4,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(2),
                    rhs: ValueId(3),
                },
            ),
        ];
        let tails = branch_and_barrier(&mut block, ValueId(4), SynchronizationScope::Subgroup);
        let mut blocks = vec![block];
        blocks.extend(tails);
        let report = analyze_function(&checked_function(vec![], blocks));
        assert_eq!(report.value(ValueId(2)), Variation::Varying);
        assert_divergent(&report, SynchronizationScope::Subgroup);
    }
}

#[test]
fn typed_collectives_distinguish_float_reduction_and_all_scan_results() {
    for scope in [
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
    ] {
        for kind in [
            Collective::ReduceSum,
            Collective::InclusiveScanSum,
            Collective::ExclusiveScanSum,
        ] {
            for scalar in [ScalarType::U32, ScalarType::I32, ScalarType::F32] {
                let module = collective_module(scope, kind, scalar);
                for input in [
                    Variation::GridUniform,
                    Variation::WorkgroupUniform,
                    Variation::SubgroupUniform,
                    Variation::Varying,
                ] {
                    let report = analyze_function_with_contract(
                        &module.functions[0],
                        &[input],
                        &BTreeSet::new(),
                        &BTreeSet::new(),
                        None,
                    );
                    let expected = if kind != Collective::ReduceSum {
                        Variation::Varying
                    } else if scope == SynchronizationScope::Workgroup {
                        Variation::WorkgroupUniform
                    } else if scalar.is_float() {
                        input
                    } else {
                        Variation::SubgroupUniform
                    };
                    assert_eq!(
                        report.value(ValueId(6)),
                        expected,
                        "{scope:?}/{kind:?}/{scalar:?}/{input:?}"
                    );
                    if scope == SynchronizationScope::Workgroup {
                        assert_eq!(report.value(ValueId(4)), Variation::WorkgroupUniform);
                        assert_eq!(report.value(ValueId(5)), Variation::WorkgroupUniform);
                    }
                    assert!(
                        report.diagnostics().is_empty(),
                        "{:?}",
                        report.diagnostics()
                    );
                }
            }
        }
    }
}

fn assert_divergent(report: &AnalysisReport, scope: SynchronizationScope) {
    assert_eq!(report.block_control(BlockId(1)), Variation::Varying);
    assert!(
        matches!(report.diagnostics(), [Diagnostic::DivergentBarrier {
        block: BlockId(1), operation_index: 0, execution_scope, control: Variation::Varying,
    }] if *execution_scope == scope),
        "{:?}",
        report.diagnostics()
    );
}

#[test]
fn typed_float_subgroup_sum_nan_payload_rejects_convergence() {
    let mut module = collective_module(
        SynchronizationScope::Subgroup,
        Collective::ReduceSum,
        ScalarType::F32,
    );
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let mut collective = block.operations.pop().unwrap();
    let OperationKind::ExecutionCapability(contract) = &mut collective.kind else {
        unreachable!()
    };
    assert_eq!(contract.operands, [ValueId(3), ValueId(0)]);
    contract.operands[1] = ValueId(13);
    block.operations.extend([
        result(
            10,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::LaneId,
                WaveWidth::Wave64,
            )),
        ),
        constant(11, Constant::U32(0x7fc0_0100)),
        result(
            12,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
        result(
            13,
            Type::F32,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(12),
                to: Type::F32,
            },
        ),
        collective,
        result(
            14,
            Type::Scalar(ScalarType::U32),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(6),
                to: Type::Scalar(ScalarType::U32),
            },
        ),
        result(
            15,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(14),
                rhs: ValueId(11),
            },
        ),
    ]);
    let tails = branch_and_barrier(block, ValueId(15), SynchronizationScope::Subgroup);
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .extend(tails);
    let module = admit(module);
    let report = analyze_kernel_entry(&module, &module.functions[0]);
    assert_eq!(report.value(ValueId(6)), Variation::Varying);
    assert_divergent(&report, SynchronizationScope::Subgroup);
}

#[test]
fn typed_uniform_nonzero_scans_reject_result_dependent_convergence() {
    for scope in [
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
    ] {
        for kind in [Collective::InclusiveScanSum, Collective::ExclusiveScanSum] {
            for scalar in [ScalarType::U32, ScalarType::I32, ScalarType::F32] {
                let mut module = collective_module(scope, kind, scalar);
                let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
                let mut collective = block.operations.pop().unwrap();
                let OperationKind::ExecutionCapability(contract) = &mut collective.kind else {
                    unreachable!()
                };
                *contract.operands.last_mut().unwrap() = ValueId(10);
                let one = match scalar {
                    ScalarType::U32 => Constant::U32(1),
                    ScalarType::I32 => Constant::I32(1),
                    ScalarType::F32 => Constant::F32Bits(1.0_f32.to_bits()),
                    _ => unreachable!(),
                };
                block.operations.extend([
                    constant(10, one),
                    collective,
                    result(
                        11,
                        Type::BOOL,
                        OperationKind::Compare {
                            predicate: ComparePredicate::Equal,
                            lhs: ValueId(6),
                            rhs: ValueId(10),
                        },
                    ),
                ]);
                let tails = branch_and_barrier(block, ValueId(11), scope);
                module.functions[0]
                    .body
                    .as_mut()
                    .unwrap()
                    .blocks
                    .extend(tails);
                let module = admit(module);
                let report = analyze_kernel_entry(&module, &module.functions[0]);
                assert_eq!(report.value(ValueId(10)), Variation::GridUniform);
                assert_eq!(report.value(ValueId(6)), Variation::Varying);
                assert_divergent(&report, scope);
            }
        }
    }
}

#[test]
fn workgroup_scan_capability_result_roster_cannot_be_relabelled() {
    for kind in [Collective::InclusiveScanSum, Collective::ExclusiveScanSum] {
        let original = collective_module(SynchronizationScope::Workgroup, kind, ScalarType::F32);
        for other in [1, 2] {
            let mut changed = original.clone();
            let collective = changed.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .last_mut()
                .unwrap();
            assert_eq!(collective.results.len(), 3);
            collective.results.swap(0, other);
            assert!(VerifiedCanonicalKernelIrV13::from_module(changed).is_err());
        }
    }
}
