use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    VerifiedCanonicalKernelIrModuleV12,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};

#[path = "production_issued_scope_legacy_source_v1_tests.rs"]
mod issued_scope_legacy_source_v1_tests;

#[path = "production_checked_output_formal_v1_tests.rs"]
mod checked_output_formal_v1_tests;

#[path = "production_optimized_assert_origins_v1_tests.rs"]
mod optimized_assert_origins_v1_tests;

#[path = "production_slice_view_v1_tests.rs"]
mod slice_view_v1_tests;

#[path = "production_masked_assertion_consumers_v1_tests.rs"]
mod masked_assertion_consumers_v1_tests;

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 17;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const SLICE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

#[derive(Clone, Copy)]
pub(super) enum Fixture {
    Literal(bool),
    ElidedBounds,
    Unreachable,
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn value(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn constant(ty: SemanticTypeIdV1, bits: u128, width: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, width).unwrap()),
    ))
}
fn assignment(
    local: u32,
    ty: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |tag, width, maximum, kind| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(width),
                width,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, (width * 8) as u16, width),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(kind),
        )
    };
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
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
        scalar(2, 1, 1, SemanticScalarTypeV1::Bool),
        scalar(
            3,
            4,
            u32::MAX.into(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar(
            4,
            8,
            u64::MAX.into(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([5; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                4,
                SemanticFieldsShapeV1::array(4, 0),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(false),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Slice { element: U32 },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([6; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    ),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SLICE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    ]
}
fn literal_terminator(expected: bool, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: constant(BOOL, u128::from(expected), 1),
        expected,
        message: SemanticAssertMessageV1::NullPointerDereference,
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}
fn fixture(
    kind: Fixture,
    shared: bool,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    fixture_with_blocks(kind, shared, |_, blocks| blocks)
}

fn fixture_with_blocks(
    kind: Fixture,
    shared: bool,
    transform: impl FnMut(u32, Vec<SemanticBasicBlockV1>) -> Vec<SemanticBasicBlockV1>,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    fixture_with_blocks_and_symbol(
        kind,
        shared,
        transform,
        |ordinal| format!("assert_root_{ordinal}"),
        &[],
    )
}

fn fixture_with_blocks_and_symbol(
    kind: Fixture,
    shared: bool,
    transform: impl FnMut(u32, Vec<SemanticBasicBlockV1>) -> Vec<SemanticBasicBlockV1>,
    symbol: impl FnMut(u32) -> String,
    extra_temporaries: &[SemanticTypeIdV1],
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    fixture_with_blocks_symbol_and_slices(kind, shared, transform, symbol, extra_temporaries, 1)
}

fn fixture_with_blocks_symbol_and_slices(
    kind: Fixture,
    shared: bool,
    mut transform: impl FnMut(u32, Vec<SemanticBasicBlockV1>) -> Vec<SemanticBasicBlockV1>,
    mut symbol: impl FnMut(u32) -> String,
    extra_temporaries: &[SemanticTypeIdV1],
    slice_arguments: u32,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut functions = Vec::new();
    let count = if shared { 4 } else { 1 };
    for ordinal in 0..count {
        let root = !shared || ordinal == 1 || ordinal == 3;
        let tag = 30 + ordinal as u8 * 10;
        let mut local_types = vec![(UNIT, SemanticLocalRoleV1::Return)];
        let (arguments, ownership) = if matches!(kind, Fixture::ElidedBounds) {
            assert!(!shared);
            local_types.extend(
                (0..slice_arguments)
                    .map(|argument| (SLICE_REF, SemanticLocalRoleV1::Argument(argument))),
            );
            local_types.extend([
                (U64, SemanticLocalRoleV1::Temporary),
                (U64, SemanticLocalRoleV1::Temporary),
                (BOOL, SemanticLocalRoleV1::Temporary),
            ]);
            let first = SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                0,
                Some(4),
            )
            .unwrap();
            let second = SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap();
            (
                vec![
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        SLICE_REF,
                        SemanticAbiPassModeV1::Pair { first, second },
                    ));
                    slice_arguments as usize
                ],
                vec![SemanticSourceArgumentOwnershipV1::SharedBorrow; slice_arguments as usize],
            )
        } else {
            (vec![], vec![])
        };
        local_types.extend(
            extra_temporaries
                .iter()
                .copied()
                .map(|ty| (ty, SemanticLocalRoleV1::Temporary)),
        );
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            if root {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if root {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            arguments.len() as u32,
            arguments,
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap();
        let blocks = if shared && ordinal != 0 {
            let callee = if ordinal == 1 { 2 } else { 0 };
            vec![
                block(
                    tag + 1,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(callee),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                place(0, UNIT),
                                edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(tag + 2, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            match kind {
                Fixture::Literal(expected) => vec![
                    block(tag + 1, vec![], literal_terminator(expected, 1)),
                    block(tag + 2, vec![], SemanticTerminatorKindV1::Return),
                ],
                Fixture::Unreachable => vec![
                    block(tag + 1, vec![], SemanticTerminatorKindV1::Return),
                    block(tag + 2, vec![], literal_terminator(true, 2)),
                    block(tag + 3, vec![], SemanticTerminatorKindV1::Return),
                ],
                Fixture::ElidedBounds => vec![
                    block(
                        tag + 1,
                        vec![assignment(
                            2,
                            U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: value(1, SLICE_REF),
                            },
                        )],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: value(2, U64),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    8,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                )],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                            )
                            .unwrap(),
                        },
                    ),
                    block(
                        tag + 2,
                        vec![
                            assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                            assignment(
                                4,
                                BOOL,
                                SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::LessThan,
                                    left: value(3, U64),
                                    right: value(2, U64),
                                },
                            ),
                        ],
                        SemanticTerminatorKindV1::Assert {
                            condition: value(4, BOOL),
                            expected: true,
                            message: SemanticAssertMessageV1::BoundsCheck {
                                length: value(2, U64),
                                index: value(3, U64),
                            },
                            target: edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(tag + 3, vec![], SemanticTerminatorKindV1::Return),
                ],
            }
        };
        let blocks = transform(ordinal, blocks);
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            if root {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            },
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source,
            abi,
            local_types
                .into_iter()
                .enumerate()
                .map(|(local, (ty, role))| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([tag + 10 + local as u8; 32]),
                        ty,
                        role,
                        source,
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        functions.push(if root {
            let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
            function.with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(symbol(ordinal).into_bytes()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([tag; 32]),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            ))
        } else {
            function
        });
    }
    let roots = if shared { vec![1, 3] } else { vec![0] };
    let mut fixture_types = types();
    if !matches!(kind, Fixture::ElidedBounds) {
        let required = extra_temporaries
            .iter()
            .map(|ty| ty.index() as usize + 1)
            .max()
            .unwrap_or(2)
            .max(2);
        fixture_types.truncate(required);
    }
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        fixture_types,
        vec![],
        vec![],
        vec![],
        functions,
        roots
            .iter()
            .map(|root| SemanticFunctionIdV1::from_index(*root))
            .collect(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launches = roots
        .into_iter()
        .map(|root| {
            crate::ProductionSourceLaunchRootInputV1::new(
                match root {
                    0 => "logical_0",
                    1 => "logical_1",
                    3 => "logical_3",
                    _ => unreachable!(),
                },
                [30 + root as u8 * 10; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect::<Vec<_>>();
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &launches).unwrap();
    (ssa, launch)
}
pub(super) fn materialize(
    kind: Fixture,
    shared: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture(kind, shared);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}
fn retained(owner: &ProductionPreRankedKirOwnerV1) -> usize {
    owner.retained_analysis_storage_v1()
}

#[test]
fn emitted_literal_origins_bind_exact_owner_definition_and_expected_false_edges() {
    for expected in [true, false] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = materialize(Fixture::Literal(expected), false, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        let payload = retained(&owner);
        budget.reserve_storage(payload).unwrap();
        let view = owner.assert_origins();
        assert!(std::ptr::eq(view.executable(), owner.executable()));
        assert_eq!((view.source_site_count(), view.binding_count()), (1, 1));
        let root = SemanticFunctionIdV1::from_index(0);
        let source_block = SemanticBlockIdV1::from_index(0);
        assert!(
            view.is_materialized_block(root, root, source_block, &mut budget)
                .unwrap()
        );
        let binding = view
            .assert_condition(root, root, source_block, &mut budget)
            .unwrap();
        assert_eq!(binding.expected(), expected);
        assert_eq!(binding.semantic_success().index(), 1);
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            condition_use,
            definition,
            success_edge,
            failure_edge,
        } = binding.outcome()
        else {
            panic!("non-bounds literal must retain its real use");
        };
        assert_eq!(success_edge.successor, u32::from(!expected));
        assert_eq!(failure_edge.successor, u32::from(expected));
        assert_eq!(success_edge.source, failure_edge.source);
        assert_eq!(
            condition_use,
            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                block: success_edge.source,
                operand: 0,
            }
        );
        let AssertDefinitionCoordinateV1::Result { operation, result } = definition else {
            panic!("literal must bind the actual generated constant result");
        };
        assert_eq!(result, 0);
        let graph = owner.executable().module();
        let emitted = &graph.functions[operation.block.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[operation.block.block as usize]
            .operations[operation.operation as usize];
        assert!(
            matches!(emitted.kind, OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(value))
            if value == expected)
        );
        drop(owner);
        budget.release_storage(payload).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn shared_helper_roots_alias_one_physical_assertion_without_duplicate_graph_facts() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let owner = materialize(Fixture::Literal(false), true, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    let view = owner.assert_origins();
    assert_eq!((view.source_site_count(), view.binding_count()), (2, 1));
    let helper = SemanticFunctionIdV1::from_index(0);
    let block = SemanticBlockIdV1::from_index(0);
    let one = view
        .assert_condition(
            SemanticFunctionIdV1::from_index(1),
            helper,
            block,
            &mut budget,
        )
        .unwrap();
    let other = view
        .assert_condition(
            SemanticFunctionIdV1::from_index(3),
            helper,
            block,
            &mut budget,
        )
        .unwrap();
    assert_eq!(one, other);
    assert!(matches!(
        view.assert_condition(helper, helper, block, &mut budget),
        Err(SemanticKirAssertOriginErrorV1::MissingBinding { .. })
    ));
    assert!(matches!(
        view.is_materialized_block(helper, helper, block, &mut budget),
        Err(SemanticKirAssertOriginErrorV1::InvalidBinding { .. })
    ));
    drop(owner);
    budget.release_storage(payload).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn existing_slice_bounds_rule_has_explicit_elision_without_a_fabricated_boolean() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let owner = materialize(Fixture::ElidedBounds, false, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let binding = owner
        .assert_origins()
        .assert_condition(root, root, SemanticBlockIdV1::from_index(1), &mut budget)
        .unwrap();
    let SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } =
        binding.outcome()
    else {
        panic!("existing exact slice-length guard rule must own this elision");
    };
    assert_eq!(success_edge.successor, 0);
    assert_eq!(binding.semantic_success().index(), 2);
    assert!(matches!(
        assert_origin_block_v1(owner.executable().module(), success_edge.source)
            .unwrap()
            .terminator,
        Some(Terminator::Branch { .. })
    ));
    drop(owner);
    budget.release_storage(payload).unwrap();
}

#[test]
fn unreachable_source_assert_is_absent_not_elided_and_invalid_coordinates_are_errors() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let owner = materialize(Fixture::Unreachable, false, &mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    let view = owner.assert_origins();
    let root = SemanticFunctionIdV1::from_index(0);
    assert_eq!((view.source_site_count(), view.binding_count()), (0, 0));
    assert!(
        view.is_materialized_block(root, root, SemanticBlockIdV1::from_index(0), &mut budget)
            .unwrap()
    );
    assert!(
        !view
            .is_materialized_block(root, root, SemanticBlockIdV1::from_index(1), &mut budget)
            .unwrap()
    );
    assert!(matches!(
        view.assert_condition(root, root, SemanticBlockIdV1::from_index(1), &mut budget),
        Err(SemanticKirAssertOriginErrorV1::MissingBinding { .. })
    ));
    for (owner_id, function, block) in [(1, 0, 0), (0, 1, 0), (0, 0, 3), (0, 0, u32::MAX)] {
        assert!(matches!(
            view.is_materialized_block(
                SemanticFunctionIdV1::from_index(owner_id),
                SemanticFunctionIdV1::from_index(function),
                SemanticBlockIdV1::from_index(block),
                &mut budget,
            ),
            Err(SemanticKirAssertOriginErrorV1::InvalidBinding { .. })
        ));
    }
    drop(owner);
    budget.release_storage(payload).unwrap();
}

fn malformed_seal(
    case: &str,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<SealedAssertOriginsV1> {
    // All graph/source owners are ordinarily admitted. Only private emission
    // candidates are corrupted; no public authenticated owner is fabricated.
    let (ssa, launch) = fixture(Fixture::Literal(true), false);
    let launches = launch
        .roots()
        .iter()
        .map(|root| {
            let layout = root.layout();
            RetainedRankedLaunchRootV1 {
                selected_root: root.selected_root(),
                launch_rank: root.source_rank(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }
        })
        .collect::<Vec<_>>();
    let mut emission = AssertOriginEmissionV1::new(budget);
    let (module, mut correspondence) = lower_module_with_assert_origins_v1(
        &ssa,
        ProductionSemanticKirLimitsV1::default(),
        Some(&launches),
        Some(&mut emission),
    )
    .unwrap();
    match case {
        "source-identity" => correspondence.semantic_sha256[0] ^= 1,
        "missing" => {
            emission.records.clear();
        }
        "duplicate" => {
            let span = correspondence.terminator_operation_spans[0];
            let function = &module.functions[0];
            let block = &function.body.as_ref().unwrap().blocks[0];
            emission.record(
                span,
                &function.id,
                ssa.source_semantic().functions()[0].blocks()[0]
                    .terminator()
                    .kind(),
                block.terminator.as_ref().unwrap(),
                false,
            )?;
        }
        "owner" => {
            emission.records[0].site.correspondence_owner = SemanticFunctionIdV1::from_index(99)
        }
        "function-name" => emission.records[0]
            .emitted_function
            .replace_range(0..1, "x"),
        "span" => emission.records[0].first_operation += 1,
        "condition" => {
            let PendingAssertOutcomeV1::Emitted { condition, .. } =
                &mut emission.records[0].outcome
            else {
                unreachable!()
            };
            *condition = ValueId(u32::MAX);
        }
        "failure" => {
            let PendingAssertOutcomeV1::Emitted { failure, .. } = &mut emission.records[0].outcome
            else {
                unreachable!()
            };
            *failure = BlockId(1);
        }
        "polarity" => emission.records[0].expected = false,
        "argument-range" => emission.records[0].argument_count = 1,
        "false-elision" => {
            emission.records[0].outcome = PendingAssertOutcomeV1::ElidedByExistingRule
        }
        "missing-span" => correspondence.terminator_operation_spans = Box::new([]),
        "duplicate-span" => {
            let mut spans = correspondence.terminator_operation_spans.to_vec();
            spans.push(spans[0]);
            correspondence.terminator_operation_spans = spans.into_boxed_slice();
        }
        _ => unreachable!(),
    }
    let (executable, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            emission.budget,
        )
        .unwrap();
    emission
        .budget
        .reserve_storage(storage.retained_storage())?;
    drop(module);
    emission.seal(&ssa, &correspondence, &executable)
}

#[test]
fn malformed_origin_candidates_fail_closed_and_failed_phase_cleanup_restores_floor() {
    for (case, expected) in [
        (
            "source-identity",
            "source identity differs from correspondence",
        ),
        ("missing", "missing"),
        ("duplicate", "duplicate assertion origin"),
        ("owner", "missing"),
        ("function-name", "function emission identity differs"),
        ("span", "source/span emission mismatch"),
        (
            "condition",
            "assertion condition or successor occurrence differs",
        ),
        (
            "failure",
            "assertion condition or successor occurrence differs",
        ),
        ("polarity", "source/span emission mismatch"),
        ("argument-range", "pending successor arguments are missing"),
        (
            "false-elision",
            "assertion emission rule and terminator differ",
        ),
        (
            "missing-span",
            "incomplete materialized source block roster",
        ),
        ("duplicate-span", "duplicate materialized source block"),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let error = malformed_seal(case, &mut budget).unwrap_err();
        if expected == "missing" {
            assert!(
                matches!(error, SemanticKirAssertOriginErrorV1::MissingBinding { .. }),
                "{case}: {error}"
            );
        } else {
            assert!(
                matches!(error, SemanticKirAssertOriginErrorV1::InvalidBinding { detail, .. }
                if detail == expected),
                "{case}: {error}"
            );
        }
        // Exactly the public materialization boundary's failure cleanup: all
        // inner owners have been dropped before releasing their accepted delta.
        let retained_work = budget.work();
        let peak = budget.peak_storage();
        let release = budget.storage() - FLOOR;
        assert!(release > 0 && retained_work > 11 && peak >= budget.storage());
        budget.release_storage(release).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), retained_work);
        assert_eq!(budget.peak_storage(), peak);
    }
}

#[test]
fn one_site_lookup_has_independent_exact_and_one_under_work_bounds() {
    let mut build_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut build_budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut build_work, STORAGE);
    let owner = materialize(Fixture::Literal(true), false, &mut build_budget);
    let payload = retained(&owner);
    let root = SemanticFunctionIdV1::from_index(0);
    let block = SemanticBlockIdV1::from_index(0);
    for allowance in [1, 2] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + allowance);
        work.charge_work(7).unwrap();
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + payload);
        budget.reserve_storage(FLOOR + payload).unwrap();
        let result = owner
            .assert_origins()
            .assert_condition(root, root, block, &mut budget);
        if allowance == 2 {
            result.unwrap();
            assert_eq!(budget.work(), 9);
        } else {
            assert!(
                matches!(result, Err(SemanticKirAssertOriginErrorV1::Resource(
                AssertOriginResourceV1::Work(error))) if error.actual() == 9 && error.limit() == 8)
            );
            assert_eq!(budget.work(), 8);
        }
        assert_eq!(budget.storage(), FLOOR + payload);
        assert_eq!(budget.peak_storage(), FLOOR + payload);
    }
}

#[test]
fn applicability_uses_exact_ssa_roster_with_five_work_for_one_function() {
    let mut build_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut build_budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut build_work, STORAGE);
    let owner = materialize(Fixture::Unreachable, false, &mut build_budget);
    let payload = retained(&owner);
    let root = SemanticFunctionIdV1::from_index(0);
    for allowance in [4, 5] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(3 + allowance);
        work.charge_work(3).unwrap();
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, payload + FLOOR);
        budget.reserve_storage(payload + FLOOR).unwrap();
        let result = owner.assert_origins().is_materialized_block(
            root,
            root,
            SemanticBlockIdV1::from_index(1),
            &mut budget,
        );
        if allowance == 5 {
            assert!(!result.unwrap());
            assert_eq!(budget.work(), 8);
        } else {
            assert!(
                matches!(result, Err(SemanticKirAssertOriginErrorV1::Resource(
                AssertOriginResourceV1::Work(error))) if error.actual() == 8 && error.limit() == 7)
            );
            assert_eq!(budget.work(), 7);
        }
        assert_eq!(budget.storage(), payload + FLOOR);
    }
}

#[test]
fn pending_payload_exact_bound_and_one_under_reject_before_growth_with_live_prefix() {
    let name = FunctionId::new("f");
    let source = literal_terminator(true, 1);
    let emitted = Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    };
    let span = SemanticKirTerminatorOperationSpanV1 {
        correspondence_owner: SemanticFunctionIdV1::from_index(0),
        semantic_function: SemanticFunctionIdV1::from_index(0),
        semantic_block: SemanticBlockIdV1::from_index(0),
        kernel_ir_block: BlockId(0),
        first_operation_ordinal: 0,
        operation_count: 0,
    };
    // First growth explicitly requests four rows; empty edge args own no buffer.
    // Record admission1 + growth1 + append1 + exact name copy1 = 4 work.
    let payload = 4 * std::mem::size_of::<PendingAssertOriginV1>() + 1;
    for (work_limit, storage_limit, expected) in [
        (4, payload, "ok"),
        (3, payload, "work"),
        (4, payload - 1, "storage"),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + work_limit);
        work.charge_work(5).unwrap();
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = {
            let mut producer = AssertOriginEmissionV1::new(&mut budget);
            let result = producer.record(span, &name, &source, &emitted, false);
            match expected {
                "ok" => {
                    assert_eq!(producer.records.capacity(), 4);
                    assert_eq!(producer.records[0].emitted_function.capacity(), 1);
                    assert_eq!(producer.records.len(), 1);
                }
                "work" | "storage" => assert!(producer.records.is_empty()),
                _ => unreachable!(),
            }
            result
        };
        match expected {
            "ok" => {
                result.unwrap();
                assert_eq!(budget.work(), 9);
            }
            "work" => {
                assert!(
                    matches!(result, Err(SemanticKirAssertOriginErrorV1::Resource(
                    AssertOriginResourceV1::Work(error))) if error.actual() == 9 && error.limit() == 8)
                );
                assert_eq!(budget.work(), 8);
            }
            "storage" => {
                assert!(
                    matches!(result, Err(SemanticKirAssertOriginErrorV1::Resource(
                    AssertOriginResourceV1::Storage(error))) if error.actual() == FLOOR + payload)
                );
                assert_eq!(budget.failed_storage(), Some(FLOOR + payload));
            }
            _ => unreachable!(),
        }
        let peak = budget.peak_storage();
        let live = budget.storage();
        budget.release_storage(live - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), peak);
    }
}

#[test]
fn geometric_pending_growth_charges_old_and_new_buffers_while_they_coexist() {
    let row = std::mem::size_of::<ValueId>();
    // Old capacity4 + next capacity8, not just the final capacity8.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + 12 * row);
    budget.reserve_storage(FLOOR).unwrap();
    let mut values = Vec::new();
    for id in 0..5 {
        assert_origin_push_v1(&mut values, ValueId(id), &mut budget).unwrap();
    }
    assert_eq!(values.capacity(), 8);
    assert_eq!(budget.storage(), FLOOR + 8 * row);
    assert_eq!(budget.peak_storage(), FLOOR + 12 * row);
    assert_eq!(budget.work(), (1 + 4) + (4 + 1 + 1));
    assert_origin_drop_v1(values, &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + 12 * row - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let mut values = Vec::new();
    for id in 0..4 {
        assert_origin_push_v1(&mut values, ValueId(id), &mut budget).unwrap();
    }
    assert!(
        matches!(assert_origin_push_v1(&mut values, ValueId(4), &mut budget),
        Err(SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Storage(error)))
            if error.actual() == FLOOR + 12 * row)
    );
    assert_eq!(values, [ValueId(0), ValueId(1), ValueId(2), ValueId(3)]);
    assert_eq!(budget.storage(), FLOOR + 4 * row);
    assert_origin_drop_v1(values, &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn production_materialization_origin_denials_drop_before_restoring_incoming_floor() {
    // Occurrence-capture custody is checked before the existing origin charges.
    const CAPTURE_PREFLIGHT: usize = 2;
    for (allowance, accepted, attempted) in [(0, 0, 1), (1, 1, 2), (2, 2, 3), (3, 3, 16)] {
        let (ssa, launch) = fixture(Fixture::Literal(true), false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5 + CAPTURE_PREFLIGHT + allowance);
        work.charge_work(5).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        );
        assert!(
            matches!(result, Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::AssertOrigin(SemanticKirAssertOriginErrorV1::Resource(
                AssertOriginResourceV1::Work(error)
            ))
        )) if error.actual() == 5 + CAPTURE_PREFLIGHT + attempted
            && error.limit() == 5 + CAPTURE_PREFLIGHT + allowance)
        );
        assert_eq!(budget.work(), 5 + CAPTURE_PREFLIGHT + accepted);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn source_name_spare_capacity_is_not_copied_or_charged_as_payload() {
    let mut source = String::with_capacity(4096);
    source.push('f');
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + 1);
    budget.reserve_storage(FLOOR).unwrap();
    let copied = assert_origin_copy_name_v1(&source, &mut budget).unwrap();
    assert_eq!(copied.capacity(), 1);
    assert_ne!(copied.as_ptr(), source.as_ptr());
    source.clear();
    assert_eq!(copied, "f");
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.storage(), FLOOR + 1);
    drop(copied);
    budget.release_storage(1).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn occurrence_sealer_distinguishes_same_typed_argument_values_and_definition_kinds() {
    // Structural service fixtures, not source/proof/artifact admission claims.
    for (kind, expected_definition) in [
        (
            "parameter",
            AssertDefinitionCoordinateV1::FunctionArgument {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
                argument: 0,
            },
        ),
        (
            "block-argument",
            AssertDefinitionCoordinateV1::BlockArgument {
                block: AssertBlockCoordinateV1 {
                    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
                    block: 1,
                },
                argument: 0,
            },
        ),
        (
            "checked-result",
            AssertDefinitionCoordinateV1::Result {
                operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                    block: AssertBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
                        block: 0,
                    },
                    operation: 0,
                },
                result: 1,
            },
        ),
    ] {
        let (condition, assert_id, success_id, failure_id) = match kind {
            "block-argument" => (ValueId(30), BlockId(1), BlockId(2), BlockId(3)),
            "checked-result" => (ValueId(21), BlockId(0), BlockId(1), BlockId(2)),
            _ => (ValueId(9), BlockId(0), BlockId(1), BlockId(2)),
        };
        let mut asserted = BasicBlock::new(assert_id);
        if kind == "block-argument" {
            asserted
                .parameters
                .push(ValueDef::new(condition, Type::BOOL));
        } else if kind == "checked-result" {
            asserted.operations.push(Operation::new(
                vec![
                    ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                    ValueDef::new(condition, Type::BOOL),
                ],
                OperationKind::Binary {
                    op: BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Add),
                    lhs: ValueId(9),
                    rhs: ValueId(10),
                },
            ));
        }
        asserted.terminator = Some(Terminator::ConditionalBranch {
            condition,
            then_target: success_id,
            then_arguments: vec![condition],
            else_target: failure_id,
            else_arguments: vec![],
        });
        let mut success = BasicBlock::new(success_id);
        success
            .parameters
            .push(ValueDef::new(ValueId(50), Type::BOOL));
        success.terminator = Some(Terminator::Return { values: vec![] });
        let mut failure = BasicBlock::new(failure_id);
        failure
            .operations
            .push(AmdGpuDiagnosticOperation::Trap.operation(None));
        failure.terminator = Some(Terminator::Unreachable);
        let mut blocks = Vec::new();
        if kind == "block-argument" {
            let mut entry = BasicBlock::new(BlockId(0));
            entry.terminator = Some(Terminator::Branch {
                target: assert_id,
                arguments: vec![ValueId(9)],
            });
            blocks.push(entry);
        }
        blocks.extend([asserted, success, failure]);
        let parameter = if kind == "checked-result" {
            Type::Scalar(ScalarType::U32)
        } else {
            Type::BOOL
        };
        let mut module = Module::new("origin-structural-service");
        module.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![parameter.clone(), parameter], vec![]),
            vec![ValueId(9), ValueId(10)],
            blocks,
        ));
        let pending = PendingAssertOriginV1 {
            site: SemanticKirAssertSiteV1::new(
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(assert_id.0),
            ),
            emitted_function: "f".to_owned(),
            block: assert_id,
            first_operation: 0,
            operation_count: u32::from(kind == "checked-result"),
            expected: true,
            semantic_success: SemanticBlockIdV1::from_index(success_id.0),
            argument_start: 0,
            argument_count: 1,
            outcome: PendingAssertOutcomeV1::Emitted {
                condition,
                failure: failure_id,
            },
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let graph = AssertGraphIndexV1::build(&module, true, &mut budget).unwrap();
        let coordinate = graph
            .block(
                fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
                assert_id,
                &mut budget,
            )
            .unwrap();
        let binding = seal_assert_occurrence_v1(
            &pending,
            &[condition],
            coordinate,
            &graph,
            &module,
            failure_id.0 as usize,
            &mut budget,
        )
        .unwrap();
        assert!(
            matches!(binding.outcome(), SemanticKirAssertConditionOutcomeV1::Emitted { definition, .. }
            if definition == expected_definition)
        );
        let wrong = seal_assert_occurrence_v1(
            &pending,
            &[ValueId(10)],
            coordinate,
            &graph,
            &module,
            failure_id.0 as usize,
            &mut budget,
        )
        .unwrap_err();
        assert!(matches!(
            wrong,
            SemanticKirAssertOriginErrorV1::InvalidBinding {
                detail: "assertion condition or successor occurrence differs",
                ..
            }
        ));
        graph.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[path = "production_private_array_owner_v1_tests.rs"]
mod private_array_owner_v1_tests;
