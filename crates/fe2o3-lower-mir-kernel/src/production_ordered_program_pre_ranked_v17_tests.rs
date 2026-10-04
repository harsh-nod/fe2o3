//! Genuine ordinary-owner path from inert fixture data. These tests qualify
//! lowering/custody, not rustc authentication, source provenance, or hardware.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U8: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy, Default)]
enum Flow {
    #[default]
    Straight,
    Conditional,
    Backedge,
}

struct Fixture {
    program: SemanticGfx942U32ProgramV32,
    registers: [u8; 5],
    old_marker: bool,
    consume_result: bool,
    dynamic_register: bool,
    projected_input: bool,
    projected_destination: bool,
    omit_argument: bool,
    flow: Flow,
    prefix: Option<SemanticRvalueKindV1>,
    workgroup: [u32; 3],
    maximum_workgroup: Option<[u32; 3]>,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            program: bitselect_program(),
            registers: [32, 33, 34, 35, 36],
            old_marker: false,
            consume_result: false,
            dynamic_register: false,
            projected_input: false,
            projected_destination: false,
            omit_argument: false,
            flow: Flow::Straight,
            prefix: None,
            workgroup: [64, 1, 1],
            maximum_workgroup: Some([64, 1, 1]),
        }
    }
}

// Independent descriptor literals for:
// xor scratch,input0,input1; and scratch,scratch,input2; xor out,input1,scratch.
// These are abstract role descriptors, not AMD machine words.
fn bitselect_program() -> SemanticGfx942U32ProgramV32 {
    let mut descriptors = [0; 16];
    descriptors[..3].copy_from_slice(&[0x0085, 0x0133, 0x019d]);
    SemanticGfx942U32ProgramV32::from_descriptors(3, descriptors).unwrap()
}

fn bounded_program(count: u8) -> SemanticGfx942U32ProgramV32 {
    let mut descriptors = [0; 16];
    if count == 1 {
        // mov out,input0
        descriptors[0] = 0x0008;
    } else {
        assert_eq!(count, 16);
        // mov scratch,input0; fourteen retained self moves; mov out,scratch.
        descriptors[1..15].fill(0x0030);
        descriptors[15] = 0x0038;
    }
    SemanticGfx942U32ProgramV32::from_descriptors(count, descriptors).unwrap()
}

fn provenance() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn function_identity() -> SemanticFunctionIdentityV1 {
    SemanticFunctionIdentityV1::from_sha256([10; 32])
}
fn source() -> SemanticOrderedProgramSourceV32 {
    SemanticOrderedProgramSourceV32::new([30; 32], function_identity(), [31; 32], [32; 32]).unwrap()
}
fn place(index: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap()
}
fn input(index: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(index, U32))
}
fn projected_place() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap()
}
fn literal(value: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U8,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 1).unwrap()),
    ))
}
fn edge(role: SemanticEdgeRoleV1, index: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(index))
}
fn assignment(kind: SemanticRvalueKindV1) -> SemanticAssignmentV1 {
    SemanticAssignmentV1::new(place(5, U32), SemanticRvalueV1::new(U32, kind))
}
fn statement(kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(assignment(kind)),
    )
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([70 + index; 32]),
        provenance(),
        statements,
        SemanticTerminatorV1::new(provenance(), kind),
    )
    .unwrap()
}
fn scalar(bits: u16, tag: u8) -> SemanticTypeDeclV1 {
    let bytes = u64::from(bits / 8);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }),
    )
}
fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        scalar(32, 1),
        scalar(8, 3),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
    ]
}
fn abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}
fn intrinsic(
    operation: SemanticCompilerIntrinsicOperationV1,
    arguments: Vec<SemanticAbiValueV1>,
    tag: u8,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag + 1; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag + 2; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 3; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 4; 32]),
            provenance(),
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([tag + 5; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 6; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                arguments,
                abi_value(U32),
            )
            .unwrap(),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag + 7; 32]),
    }
}

impl Fixture {
    fn request(&self) -> InertSemanticMirRequestV1 {
        let pointer = SemanticTypeIdV1::from_index(3);
        let pointer_argument = self.projected_input || self.projected_destination;
        let first_argument_type = if pointer_argument { pointer } else { U32 };
        let mut fixture_types = types();
        if pointer_argument {
            fixture_types.push(
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([7; 32]),
                    SemanticLayoutIdentityV1::from_sha256([8; 32]),
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(8),
                        8,
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::pointer(1, 8, 8),
                            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                        )),
                        false,
                    )
                    .unwrap(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new_with_kind(
                            U32,
                            SemanticPointerKindV1::Raw,
                            if self.projected_destination {
                                SemanticMutabilityV1::Mutable
                            } else {
                                SemanticMutabilityV1::Immutable
                            },
                            1,
                            64,
                            SemanticPointerMetadataV1::None,
                        )
                        .unwrap(),
                    ),
                )
                .with_rustc_abi_properties(
                    SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                        Some(
                            SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1)
                                .unwrap(),
                        ),
                        None,
                    ),
                ),
            );
        }
        let mut arguments = vec![abi_value(U32); 3];
        arguments.extend(vec![abi_value(U8); 5]);
        let mut callables = vec![
            SemanticCallableDeclV1::defined(ROOT),
            intrinsic(
                SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(self.program),
                arguments,
                20,
            ),
        ];
        if self.old_marker {
            callables.push(intrinsic(
                SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(
                    SemanticGfx942InlineU32V30::new(SemanticGfx942InlineInstructionV30::VMovB32, 1)
                        .unwrap(),
                ),
                vec![abi_value(U32)],
                100,
            ));
        }
        let mut arguments = vec![
            input(if self.old_marker { 5 } else { 1 }),
            input(2),
            input(3),
        ];
        arguments.extend(self.registers.into_iter().map(literal));
        if self.projected_input && !self.old_marker {
            arguments[0] = SemanticOperandV1::Copy(projected_place());
        }
        if self.projected_destination {
            arguments[0] = input(2);
        }
        if self.dynamic_register {
            arguments[3] = SemanticOperandV1::Copy(place(6, U8));
        }
        if self.omit_argument {
            arguments.pop();
        }
        let region_index =
            u32::from(self.old_marker) + u32::from(matches!(self.flow, Flow::Conditional));
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            arguments,
            Some(SemanticCallDestinationV1::new(
                if self.projected_destination && !self.old_marker {
                    projected_place()
                } else {
                    place(4, U32)
                },
                edge(SemanticEdgeRoleV1::CallReturn, region_index + 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap()
        .with_ordered_program_source_v32(source());
        let mut blocks = vec![];
        if matches!(self.flow, Flow::Conditional) {
            blocks.push(block(
                0,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: input(1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, region_index + 1),
                    )
                    .unwrap(),
                },
            ));
        }
        if self.old_marker {
            let old = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(2),
                vec![if self.projected_input {
                    SemanticOperandV1::Copy(projected_place())
                } else if self.projected_destination {
                    input(2)
                } else {
                    input(1)
                }],
                Some(SemanticCallDestinationV1::new(
                    if self.projected_destination {
                        projected_place()
                    } else {
                        place(5, U32)
                    },
                    edge(SemanticEdgeRoleV1::CallReturn, region_index),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap()
            .with_inline_assembly_source_v30(
                SemanticInlineAssemblySourceV30::new(
                    [130; 32],
                    function_identity(),
                    [131; 32],
                    [132; 32],
                )
                .unwrap(),
            );
            blocks.push(block(
                blocks.len() as u8,
                vec![],
                SemanticTerminatorKindV1::Call(old),
            ));
        }
        let mut prefix = self
            .prefix
            .clone()
            .map(statement)
            .into_iter()
            .collect::<Vec<_>>();
        if self.dynamic_register {
            prefix.push(SemanticStatementV1::new(
                provenance(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(6, U8),
                    SemanticRvalueV1::new(U8, SemanticRvalueKindV1::Use(literal(32))),
                )),
            ));
        }
        blocks.push(block(
            blocks.len() as u8,
            prefix,
            SemanticTerminatorKindV1::Call(call),
        ));
        let after = if self.consume_result {
            vec![statement(SemanticRvalueKindV1::Use(input(4)))]
        } else {
            vec![]
        };
        let terminator = if matches!(self.flow, Flow::Backedge) {
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0))
        } else {
            SemanticTerminatorKindV1::Return
        };
        blocks.push(block(blocks.len() as u8, after, terminator));
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([60; 32]),
            SemanticLayoutIdentityV1::from_sha256([61; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            3,
            vec![
                SemanticAbiArgumentV1::source(abi_value(first_argument_type)),
                SemanticAbiArgumentV1::source(abi_value(U32)),
                SemanticAbiArgumentV1::source(abi_value(U32)),
            ],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let locals = [
            (UNIT, SemanticLocalRoleV1::Return),
            (first_argument_type, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Argument(1)),
            (U32, SemanticLocalRoleV1::Argument(2)),
            (U32, SemanticLocalRoleV1::Temporary),
            (U32, SemanticLocalRoleV1::Temporary),
            (U8, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([80 + index as u8; 32]),
                ty,
                role,
                provenance(),
            )
        })
        .collect();
        let root = SemanticFunctionDeclV1::new(
            function_identity(),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([11; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([12; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([13; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([14; 32]),
            provenance(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"ordered_program_root".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([15; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new(self.workgroup).unwrap()),
                        self.maximum_workgroup
                            .map(|size| SemanticWorkgroupDimensionsV1::new(size).unwrap()),
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            fixture_types,
            vec![],
            vec![],
            vec![],
            vec![root],
            callables,
            vec![ROOT],
        )
        .unwrap()
    }

    fn owners(
        &self,
    ) -> (
        ProductionSemanticSsaOwnerV1,
        crate::ProductionSourceLaunchRosterV1,
    ) {
        let admitted = self
            .request()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V32);
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(
            ssa.source_semantic(),
            &[crate::ProductionSourceLaunchRootInputV1::new(
                "ordered_program_root",
                [15; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some(self.workgroup), [1, 1, 1]),
            )],
        )
        .unwrap();
        (ssa, launch)
    }

    fn materialize(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionOrderedProgramPreRankedKirOwnerV17,
        ProductionOrderedProgramPreRankedErrorV17,
    > {
        let (ssa, launch) = self.owners();
        ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )
    }
}

#[test]
fn ordered_program_retains_exact_payload_correspondence_and_unused_result() {
    for consume_result in [false, true] {
        for old_marker in [false, true] {
            let fixture = Fixture {
                consume_result,
                old_marker,
                ..Fixture::default()
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
            budget.reserve_storage(17).unwrap();
            let owner = fixture.materialize(&mut budget).unwrap();
            assert_eq!(budget.storage(), 17);
            assert!(!owner.grants_artifact_or_launch_authority());
            owner.semantic_ssa().verify_replay().unwrap();
            assert_eq!(
                owner.correspondence().semantic_sha256(),
                owner
                    .semantic_ssa()
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes()
            );
            assert_eq!(owner.correspondence().function_count(), 1);
            assert_eq!(owner.correspondence().parameter_bindings().len(), 3);
            assert_eq!(owner.correspondence().call_returns.len(), 1);
            assert!(owner.correspondence().call_result_components.is_empty());
            assert_eq!(
                owner.call_correspondence_storage(),
                CallReturnBufferV1::bytes(1, 0).unwrap()
            );
            let operations = owner.executable().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .collect::<Vec<_>>();
            let regions = operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Gfx942OrderedProgram(region) => Some((*operation, region)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [(operation, region)] = regions.as_slice() else {
                panic!("region count: {}", regions.len());
            };
            assert_eq!(region.registers().scratch(), 32);
            assert_eq!(region.registers().output(), 33);
            assert_eq!(region.registers().inputs(), [34, 35, 36]);
            assert_eq!(region.registers().vgpr_high_water(), 37);
            assert_eq!(region.program().count(), fixture.program.count());
            assert_eq!(
                region.program().descriptors(),
                fixture.program.descriptors()
            );
            assert_eq!(region.source().frontend_unit, source().frontend_unit());
            assert_eq!(region.source().function, *source().function().as_bytes());
            assert_eq!(region.source().contract, source().contract());
            assert_eq!(region.source().statement, source().statement());
            assert_eq!(operation.results.len(), 1);
            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
            let block_id = SemanticBlockIdV1::from_index(u32::from(old_marker));
            let span = owner
                .correspondence()
                .terminator_operation_spans()
                .iter()
                .find(|span| span.semantic_block() == block_id)
                .unwrap();
            assert_eq!(span.semantic_function(), ROOT);
            assert_eq!(span.operation_count(), 1);
            assert_eq!(
                operations
                    .iter()
                    .filter(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
                    .count(),
                usize::from(old_marker)
            );
            // Complete NoMemory is independent of the nonmovable ordered-unit
            // effect. It must not be mistaken for a movable scalar program.
            assert!(operation.has_complete_effect_summary());
            let mut work2 = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
            let mut budget2 =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work2, 32 * 1024 * 1024);
            let repeated = fixture.materialize(&mut budget2).unwrap();
            assert_eq!(
                owner.executable().canonical_bytes(),
                repeated.executable().canonical_bytes()
            );
            assert_eq!(owner.correspondence(), repeated.correspondence());
        }
    }
}

#[test]
fn ordered_program_rejects_alias_out_of_range_nonliteral_and_wrong_arity_before_lowering() {
    for fixture in [
        Fixture {
            registers: [32, 32, 34, 35, 36],
            ..Fixture::default()
        },
        Fixture {
            registers: [64, 33, 34, 35, 36],
            ..Fixture::default()
        },
        Fixture {
            dynamic_register: true,
            ..Fixture::default()
        },
        Fixture {
            omit_argument: true,
            ..Fixture::default()
        },
    ] {
        assert!(
            fixture
                .request()
                .admit_current_production(SemanticMirLimitsV1::default())
                .is_err()
        );
    }
}

#[test]
fn ordered_program_rejects_conditional_backedge_launch_and_trapping_prefix() {
    for fixture in [
        Fixture {
            flow: Flow::Conditional,
            ..Fixture::default()
        },
        Fixture {
            flow: Flow::Backedge,
            ..Fixture::default()
        },
        Fixture {
            workgroup: [32, 1, 1],
            ..Fixture::default()
        },
        Fixture {
            prefix: Some(SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Divide,
                left: input(1),
                right: input(2),
            }),
            ..Fixture::default()
        },
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        budget.reserve_storage(17).unwrap();
        assert!(fixture.materialize(&mut budget).is_err());
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn ordered_program_prefix_is_closed_to_direct_scalar_nontrapping_forms() {
    let types = types();
    for kind in [
        SemanticRvalueKindV1::Use(input(1)),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::Not,
            operand: input(1),
        },
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitXor,
            left: input(1),
            right: input(2),
        },
    ] {
        assert!(ordered_program_scalar_prefix_assignment_v32(
            &types,
            &assignment(kind)
        ));
    }
    for operation in [
        SemanticBinaryOpV1::Add,
        SemanticBinaryOpV1::Subtract,
        SemanticBinaryOpV1::Multiply,
        SemanticBinaryOpV1::Divide,
        SemanticBinaryOpV1::Remainder,
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
        SemanticBinaryOpV1::Offset,
    ] {
        assert!(!ordered_program_scalar_prefix_assignment_v32(
            &types,
            &assignment(SemanticRvalueKindV1::Binary {
                operation,
                left: input(1),
                right: input(2)
            })
        ));
    }
    for kind in [
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(1, U32),
        },
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place: place(1, U32),
        },
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Float,
            operand: input(1),
        },
        SemanticRvalueKindV1::Length(place(1, U32)),
    ] {
        assert!(!ordered_program_scalar_prefix_assignment_v32(
            &types,
            &assignment(kind)
        ));
    }
}

#[test]
fn ordered_program_rejects_projected_data_from_an_admitted_semantic_owner() {
    for (fixture, expected) in [
        (
            Fixture {
                projected_input: true,
                ..Fixture::default()
            },
            "ordered program inputs must be direct scalar locals or constants",
        ),
        (
            Fixture {
                projected_destination: true,
                ..Fixture::default()
            },
            "ordered program result must be a direct scalar local",
        ),
        (
            Fixture {
                projected_input: true,
                old_marker: true,
                ..Fixture::default()
            },
            "prefix marker inputs and destination must be direct scalar locals or constants",
        ),
        (
            Fixture {
                projected_destination: true,
                old_marker: true,
                ..Fixture::default()
            },
            "prefix marker inputs and destination must be direct scalar locals or constants",
        ),
    ] {
        // Construction goes through genuine public semantic/SSA owner admission.
        // This is inert shape evidence, not an actual source-authenticated pointer.
        let (ssa, launch) = fixture.owners();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        budget.reserve_storage(17).unwrap();
        let error = ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionOrderedProgramPreRankedErrorV17::Lowering(
                ProductionSemanticKirErrorV1::Unsupported {
                    detail,
                    ..
                }
            ) if detail == expected
        ));
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn ordered_program_rejects_substituted_launch_and_source_limits() {
    let (ssa, _) = Fixture::default().owners();
    let (_, launch) = Fixture {
        registers: [0, 1, 2, 3, 4],
        ..Fixture::default()
    }
    .owners();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
    budget.reserve_storage(17).unwrap();
    assert!(
        ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 17);
    for limits in [
        ProductionSemanticKirLimitsV1 {
            max_blocks: 1,
            ..ProductionSemanticKirLimitsV1::default()
        },
        ProductionSemanticKirLimitsV1 {
            max_operations: 0,
            ..ProductionSemanticKirLimitsV1::default()
        },
    ] {
        let (ssa, launch) = Fixture::default().owners();
        assert!(
            ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
                ssa,
                launch,
                limits,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn ordered_program_budget_failures_restore_floor_without_forgiving_work() {
    let fixture = Fixture::default();
    for maximum_work in [0, 7, 8, 32, 64, 128, 512, 2048] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(maximum_work);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        budget.reserve_storage(17).unwrap();
        assert!(fixture.materialize(&mut budget).is_err());
        assert_eq!(budget.storage(), 17);
        assert!(work.work() <= maximum_work);
        assert!(work.failed_work().is_some());
    }
    for maximum_storage in [17, 18, 32, 64, 128, 512, 1024] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, maximum_storage);
        budget.reserve_storage(17).unwrap();
        assert!(fixture.materialize(&mut budget).is_err());
        assert_eq!(budget.storage(), 17);
        assert!(work.work() > 0);
    }
}

#[path = "production_ordered_program_inspection_v1_tests.rs"]
mod inspection_tests;

#[test]
fn ordered_program_retains_one_three_and_sixteen_steps_without_normalization() {
    for program in [bounded_program(1), bitselect_program(), bounded_program(16)] {
        for consume_result in [false, true] {
            let fixture = Fixture {
                program,
                consume_result,
                ..Fixture::default()
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
            budget.reserve_storage(17).unwrap();
            let owner = fixture.materialize(&mut budget).unwrap();
            assert_eq!(budget.storage(), 17);
            assert_eq!(
                owner.semantic_ssa().source_semantic().wire_version(),
                SemanticMirWireVersionV1::V32
            );
            let module = owner.executable().module();
            let programs = module.functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Gfx942OrderedProgram(payload) => Some((operation, payload)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [(operation, payload)] = programs.as_slice() else {
                panic!("expected one atomic program");
            };
            assert_eq!(payload.program().count(), program.count());
            assert_eq!(payload.program().descriptors(), program.descriptors());
            assert_eq!(payload.program().packed_words(), program.packed_words());
            assert!(
                fixture
                    .request()
                    .admit_exact_v30(SemanticMirLimitsV1::default())
                    .is_err()
            );
            assert!(
                fixture
                    .request()
                    .admit_exact_v31(SemanticMirLimitsV1::default())
                    .is_err()
            );
            assert_eq!(operation.results.len(), 1);
            assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::U32));
            assert!(fe2o3_kernel_ir::encode_module_v12(module).is_err());
            assert!(fe2o3_kernel_ir::encode_module_v16(module).is_err());
            assert!(
                fe2o3_kernel_ir::decode_module_v16(owner.executable().canonical_bytes()).is_err()
            );
        }
    }
}

#[test]
fn ordered_program_same_output_dead_step_edit_still_changes_canonical_identity() {
    // Both programs leave out unchanged by the fourth step, but the authored
    // instruction count and complete descriptor array must remain distinct.
    let original = bitselect_program();
    let edited = SemanticGfx942U32ProgramV32::from_descriptors(
        4,
        *original.descriptors(), // Active descriptor0 is mov scratch,input0.
    )
    .unwrap();
    let materialize = |program| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        Fixture {
            program,
            ..Fixture::default()
        }
        .materialize(&mut budget)
        .unwrap()
    };
    let before = materialize(original);
    let after = materialize(edited);
    assert_ne!(
        before.semantic_ssa().source_semantic().semantic_sha256(),
        after.semantic_ssa().source_semantic().semantic_sha256()
    );
    assert_ne!(
        before.executable().identity(),
        after.executable().identity()
    );
    assert_ne!(
        before.executable().canonical_bytes(),
        after.executable().canonical_bytes()
    );
    // This inert fixture deliberately uses the same four source references.
    // Actual rustc occurrence custody must separately bind the authored edit;
    // these tests do not claim authenticated source identity.
}

#[test]
fn ordered_program_materializes_each_closed_opcode_with_exact_descriptor() {
    // Independent one-step descriptor words: mov out,input0 and each binary
    // operation out,input0,input1. No compiler-generated preferred opcode list.
    for descriptor in [0x0008, 0x0089, 0x008a, 0x008b, 0x008c, 0x008d] {
        let mut descriptors = [0; 16];
        descriptors[0] = descriptor;
        let fixture = Fixture {
            program: SemanticGfx942U32ProgramV32::from_descriptors(1, descriptors).unwrap(),
            ..Fixture::default()
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        let owner = fixture.materialize(&mut budget).unwrap();
        let body = owner.executable().module().functions[0]
            .body
            .as_ref()
            .unwrap();
        let actual = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                OperationKind::Gfx942OrderedProgram(payload) => Some(payload),
                _ => None,
            })
            .unwrap();
        assert_eq!(actual.program().count(), 1);
        assert_eq!(actual.program().descriptors(), &descriptors);
    }
}

#[test]
fn ordered_program_source_maximum_is_exact_not_inferred_from_required_or_roster() {
    for maximum_workgroup in [None, Some([128, 1, 1])] {
        let fixture = Fixture {
            maximum_workgroup,
            ..Fixture::default()
        };
        // Inert semantic/SSA and ordinary required-launch agreement can admit
        // these shapes; the NEW closed source-program owner must refuse them.
        let (ssa, launch) = fixture.owners();
        assert_eq!(launch.roots()[0].layout().workgroup_extents(), [64, 1, 1]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        budget.reserve_storage(17).unwrap();
        let error = ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap_err();
        assert!(matches!(error,
            ProductionOrderedProgramPreRankedErrorV17::Lowering(
                ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            ) if detail == "ordered program source requires exact required and maximum 64x1x1 bounds"
        ));
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 24);
    }
}

// Reuse the existing inert semantic fixture; no duplicated test module.
#[path = "production_ordered_composition_v1_tests.rs"]
mod composition;
