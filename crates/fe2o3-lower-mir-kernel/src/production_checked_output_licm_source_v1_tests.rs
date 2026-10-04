// Genuine constructed semantic source, never detached canonical substitutions.
use super::*;
#[path = "production_checked_output_cross_block_forwarding_v1_tests.rs"]
mod cross_block_forwarding;
#[path = "production_checked_output_induction_refinement_internal_v1_tests.rs"]
mod induction_refinement;
#[path = "production_checked_output_loop_unroll_source_v1_tests.rs"]
mod loop_unroll;
#[path = "production_checked_output_refined_forwarding_v1_tests.rs"]
mod refined_forwarding;
use crate::{
    ProductionOwnedLoopPreheadersContinuationV1 as DirectPreheaders,
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1 as ErasedPreheaders,
};

#[path = "production_checked_output_licm_direct_v1_tests.rs"]
mod direct;
#[path = "production_checked_output_licm_erased_v1_tests.rs"]
mod erased;
#[path = "production_checked_output_licm_resource_v1_tests.rs"]
mod resources;
#[path = "production_checked_output_licm_sim_v1_tests.rs"]
mod sim;

fn jump(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}
fn switch(
    operand: SemanticOperandV1,
    bits: u128,
    target: u32,
    otherwise: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: operand,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                bits,
                edge(SemanticEdgeRoleV1::SwitchValue, target),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
        )
        .unwrap(),
    }
}
fn binary(
    local: u32,
    ty: SemanticTypeIdV1,
    operation: SemanticBinaryOpV1,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
) -> SemanticStatementV1 {
    assignment(
        local,
        ty,
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        },
    )
}
fn store(bits: u128) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(1, U32),
            constant(U32, bits, 4),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    )
}
fn call(callee: u32, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(callee),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(0, UNIT),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn source(shared: bool, mutation: bool) -> ProductionPreRankedKirOwnerV1 {
    source_with(shared, mutation, |_| {})
}

fn source_with(
    shared: bool,
    mutation: bool,
    transform: impl FnOnce(&mut Vec<SemanticFunctionDeclV1>),
) -> ProductionPreRankedKirOwnerV1 {
    let (seed, _) = fixture_with_blocks_and_symbol(
        Fixture::Literal(true),
        shared,
        |_, blocks| blocks,
        |ordinal| {
            if shared {
                format!("licm_unit_{ordinal}")
            } else {
                "private_array_relation".to_owned()
            }
        },
        &[U32, U32],
    );
    let semantic = seed.source_semantic();
    let mut functions = semantic
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, function)| {
            let root = function.role() == SemanticFunctionRoleV1::KernelRoot;
            let tag = 30 + ordinal as u8 * 10;
            let mut locals = function.locals().to_vec();
            let abi = if root {
                for (offset, ty) in [U32, U32, BOOL, U32, BOOL].into_iter().enumerate() {
                    locals.push(SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([tag + 13 + offset as u8; 32]),
                        ty,
                        if offset == 0 {
                            SemanticLocalRoleV1::Argument(0)
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                        function.source(),
                    ));
                }
                SemanticFunctionAbiV1::from_rustc(
                    function.abi().identity(),
                    semantic.target().identity(),
                    SemanticCanonAbiV1::GpuKernel,
                    SemanticExternAbiV1::GpuKernel,
                    false,
                    false,
                    1,
                    vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        U32,
                        SemanticAbiPassModeV1::Direct(
                            SemanticAbiValueAttributesV1::new(
                                SemanticAbiRegularAttributesV1::new(
                                    false, None, false, false, false, true,
                                ),
                                SemanticAbiExtensionV1::None,
                                0,
                                None,
                            )
                            .unwrap(),
                        ),
                    ))],
                    SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
                )
                .unwrap()
                .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
                .unwrap()
            } else {
                function.abi().clone()
            };
            let read = assignment(2, U32, SemanticRvalueKindV1::Use(value(1, U32)));
            let blocks = if root && mutation {
                let mut blocks = vec![
                    block(
                        tag + 1,
                        vec![assignment(
                            4,
                            U32,
                            SemanticRvalueKindV1::Use(constant(U32, 0, 4)),
                        )],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: value(3, U32),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![
                                    SemanticSwitchTargetV1::new(
                                        0,
                                        edge(SemanticEdgeRoleV1::SwitchValue, 2),
                                    ),
                                    SemanticSwitchTargetV1::new(
                                        u32::MAX.into(),
                                        edge(SemanticEdgeRoleV1::SwitchValue, 5),
                                    ),
                                ],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                            )
                            .unwrap(),
                        },
                    ),
                    block(
                        tag + 2,
                        vec![assignment(
                            4,
                            U32,
                            SemanticRvalueKindV1::Use(constant(U32, 1, 4)),
                        )],
                        jump(2),
                    ),
                    block(
                        tag + 3,
                        vec![binary(
                            5,
                            BOOL,
                            SemanticBinaryOpV1::LessThan,
                            value(4, U32),
                            constant(U32, 3, 4),
                        )],
                        switch(value(5, BOOL), 1, 3, 5),
                    ),
                    block(
                        tag + 4,
                        vec![
                            binary(
                                6,
                                U32,
                                SemanticBinaryOpV1::BitAnd,
                                value(3, U32),
                                constant(U32, 1, 4),
                            ),
                            binary(
                                7,
                                BOOL,
                                SemanticBinaryOpV1::Equal,
                                value(6, U32),
                                constant(U32, 0, 4),
                            ),
                            store(99),
                        ],
                        switch(value(7, BOOL), 1, 4, 5),
                    ),
                    block(
                        tag + 5,
                        vec![
                            read,
                            binary(
                                4,
                                U32,
                                SemanticBinaryOpV1::Add,
                                value(4, U32),
                                constant(U32, 1, 4),
                            ),
                        ],
                        jump(2),
                    ),
                    block(
                        tag + 6,
                        vec![],
                        if shared {
                            call(0, 6)
                        } else {
                            SemanticTerminatorKindV1::Return
                        },
                    ),
                ];
                if shared {
                    blocks.push(block(
                        tag + 7,
                        vec![],
                        if ordinal == 1 {
                            call(2, 7)
                        } else {
                            SemanticTerminatorKindV1::Return
                        },
                    ));
                    if ordinal == 1 {
                        blocks.push(block(tag + 8, vec![], SemanticTerminatorKindV1::Return));
                    }
                }
                blocks
            } else if root {
                let mut blocks = vec![
                    block(
                        tag + 1,
                        vec![binary(
                            5,
                            BOOL,
                            SemanticBinaryOpV1::Equal,
                            value(3, U32),
                            constant(U32, 0, 4),
                        )],
                        switch(value(5, BOOL), 1, 1, 1),
                    ),
                    block(
                        tag + 2,
                        vec![],
                        if shared {
                            call(0, 2)
                        } else {
                            SemanticTerminatorKindV1::Return
                        },
                    ),
                ];
                if shared {
                    blocks.push(block(
                        tag + 3,
                        vec![],
                        if ordinal == 1 {
                            call(2, 3)
                        } else {
                            SemanticTerminatorKindV1::Return
                        },
                    ));
                    if ordinal == 1 {
                        blocks.push(block(tag + 4, vec![], SemanticTerminatorKindV1::Return));
                    }
                }
                blocks
            } else {
                // Both genuine helper definitions must survive import before the
                // checked UnitLocal deletion stage counts their three call sites.
                vec![
                    block(
                        tag + 1,
                        vec![store(if ordinal == 0 { 77 } else { 88 }), read],
                        jump(1),
                    ),
                    block(tag + 2, vec![], SemanticTerminatorKindV1::Return),
                ]
            };
            let rebuilt = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                abi,
                locals,
                function.entry(),
                blocks,
            )
            .unwrap();
            if root {
                rebuilt.with_kernel_entry(function.kernel_entry().unwrap().clone())
            } else {
                rebuilt
            }
        })
        .collect();
    transform(&mut functions);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launches: Vec<_> = ssa
        .source_semantic()
        .roots()
        .iter()
        .map(|root| {
            crate::ProductionSourceLaunchRootInputV1::new(
                match root.index() {
                    0 => "logical_0",
                    1 => "logical_1",
                    3 => "logical_3",
                    _ => unreachable!(),
                },
                [30 + root.index() as u8 * 10; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &launches).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        owner.source_launch().roots().len(),
        if shared { 2 } else { 1 }
    );
    if shared {
        assert_eq!(
            owner.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::UnitLocal
        );
        assert_eq!(owner.semantic_ssa().source_semantic().functions().len(), 4);
        let bodies = || {
            owner
                .executable()
                .module()
                .functions
                .iter()
                .filter_map(|function| function.body.as_ref())
        };
        assert_eq!(
            bodies()
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
                .count(),
            3
        );
        for bits in [77, 88] {
            assert_eq!(
                bodies()
                    .flat_map(|body| &body.blocks)
                    .flat_map(|block| &block.operations)
                    .filter(|operation| operation.kind
                        == OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(bits)))
                    .count(),
                1
            );
        }
    }
    owner
}

fn actual_mutation(
    before: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    after: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    rows: &[fe2o3_kernel_analysis::CanonicalKirLicmOriginV1],
) {
    let operations = |owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12| {
        owner
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .map(|b| b.blocks.iter().map(|b| b.operations.len()).sum::<usize>())
            .sum::<usize>()
    };
    assert_ne!(
        before.canonical().canonical_bytes(),
        after.canonical().canonical_bytes()
    );
    assert_eq!(rows.len(), operations(before));
    assert_eq!(rows.len(), operations(after));
    let operation = |coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1| {
        &before.module().functions[coordinate.block.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[coordinate.block.block as usize]
            .operations[coordinate.operation as usize]
    };
    assert!(rows.iter().any(|row| row.hoist.is_some()
        && matches!(
            operation(row.input).kind,
            OperationKind::Binary {
                op: fe2o3_kernel_ir::BinaryOp::BitAnd,
                ..
            }
        )));
    assert!(rows.iter().any(|row| row.hoist.is_some()
        && matches!(operation(row.input).kind, OperationKind::Compare { .. })));
    assert!(rows.iter().any(|row| row.hoist.is_none()
        && row.input.operation != row.output.operation
        && matches!(operation(row.input).kind, OperationKind::Store { .. })));
}
