use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::*;

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 128 * 1024 * 1024;
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const INDEX: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const FLOAT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const READ: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const WRITE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

#[derive(Clone, Copy)]
enum Case {
    Vecadd,
    SelectedFailure,
    SelectedSuccess,
    DeadOutput,
    Abort,
    ExtraRead,
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn value(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn boolean(value: bool) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        BOOL,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(u128::from(value), 1).unwrap()),
    ))
}
fn assignment(
    destination: SemanticPlaceV1,
    ty: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([100 + index; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}
fn attributes(reference: bool, mutable: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            reference,
            if reference && !mutable {
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
            } else {
                None
            },
            reference,
            reference && !mutable,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        reference.then_some(4),
    )
    .unwrap()
}
fn pair(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Pair {
            first: attributes(true, ty == WRITE),
            second: attributes(false, false),
        },
    )
}
fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let integer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let empty = || {
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
        .unwrap()
    };
    let mut types = vec![declaration(1, empty(), SemanticTypeShapeV1::Unit)];
    for (tag, width, primitive, maximum, scalar) in [
        (
            2,
            1,
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            1u128,
            SemanticScalarTypeV1::Bool,
        ),
        (
            3,
            8,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            u64::MAX.into(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
        ),
        (
            4,
            4,
            SemanticBackendPrimitiveV1::float(32, 4),
            u32::MAX.into(),
            SemanticScalarTypeV1::Float { bits: 32 },
        ),
    ] {
        types.push(declaration(
            tag,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(width),
                width,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    primitive,
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(scalar),
        ));
    }
    types.push(declaration(
        5,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    types.push(declaration(
        6,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(integer),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![INDEX, MARKER]).unwrap()),
    ));
    types.push(declaration(
        7,
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
        SemanticTypeShapeV1::Slice { element: FLOAT },
    ));
    for (tag, mutable) in [(8, false), (9, true)] {
        types.push(
            declaration(
                tag,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(16),
                    8,
                    SemanticBackendReprV1::scalar_pair(
                        SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                        ),
                        integer,
                    ),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SLICE,
                        SemanticPointerKindV1::Reference,
                        if mutable {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
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
                            if mutable {
                                SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                            } else {
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                            },
                            0,
                            4,
                        )
                        .unwrap(),
                    ),
                    None,
                ),
            ),
        );
    }
    types
}

// Genuine admitted semantic MIR for the shared vecadd body's control/memory
// shape: global invocation index, output guard, two input bounds assertions,
// two f32 reads and one f32-add store. This is a semantic-source component test,
// not a rustc collector or GPU qualification. The mutable-slice parameter models
// the lowered DisjointSlice view; it does not claim to test its intrinsic import.
fn semantic_vecadd(
    case: Case,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    let index_place = |slice: u32| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(slice),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(5)),
                    FLOAT,
                )
                .unwrap(),
            ],
            FLOAT,
        )
        .unwrap()
    };
    let length = |destination, slice, ty| {
        assignment(
            place(destination, INDEX),
            INDEX,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: value(slice, ty),
            },
        )
    };
    let compare = |destination, length| {
        assignment(
            place(destination, BOOL),
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: value(5, INDEX),
                right: value(length, INDEX),
            },
        )
    };
    let assertion = |condition, length, target| SemanticTerminatorKindV1::Assert {
        condition: value(condition, BOOL),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: value(length, INDEX),
            index: value(5, INDEX),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let raw_index = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), INDEX).unwrap()],
        INDEX,
    )
    .unwrap();
    let mut first_check = vec![length(8, 1, READ), compare(9, 8)];
    if matches!(case, Case::SelectedFailure | Case::SelectedSuccess) {
        first_check[1] = assignment(
            place(9, BOOL),
            BOOL,
            SemanticRvalueKindV1::Use(boolean(matches!(case, Case::SelectedSuccess))),
        );
    }
    let mut computation = vec![
        assignment(
            place(13, FLOAT),
            FLOAT,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(index_place(2))),
        ),
        assignment(
            place(14, FLOAT),
            FLOAT,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: value(10, FLOAT),
                right: value(13, FLOAT),
            },
        ),
        assignment(
            index_place(3),
            FLOAT,
            SemanticRvalueKindV1::Use(value(14, FLOAT)),
        ),
    ];
    if matches!(case, Case::ExtraRead) {
        computation.push(assignment(
            place(15, FLOAT),
            FLOAT,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(index_place(1))),
        ));
    }
    let blocks = vec![
        block(
            0,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(1),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        place(4, WITNESS),
                        edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        block(
            1,
            vec![
                assignment(
                    place(5, INDEX),
                    INDEX,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(raw_index)),
                ),
                length(6, 3, WRITE),
                if matches!(case, Case::DeadOutput) {
                    assignment(
                        place(7, BOOL),
                        BOOL,
                        SemanticRvalueKindV1::Use(boolean(false)),
                    )
                } else {
                    compare(7, 6)
                },
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: value(7, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 2),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                )
                .unwrap(),
            },
        ),
        block(
            2,
            first_check,
            if matches!(case, Case::Abort) {
                SemanticTerminatorKindV1::Abort
            } else {
                assertion(9, 8, 3)
            },
        ),
        block(
            3,
            vec![
                assignment(
                    place(10, FLOAT),
                    FLOAT,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(index_place(1))),
                ),
                length(11, 2, READ),
                if matches!(case, Case::SelectedSuccess) {
                    assignment(
                        place(12, BOOL),
                        BOOL,
                        SemanticRvalueKindV1::Use(boolean(true)),
                    )
                } else {
                    compare(12, 11)
                },
            ],
            assertion(12, 11, 4),
        ),
        block(
            4,
            computation,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5)),
        ),
        block(5, vec![], SemanticTerminatorKindV1::Return),
    ];
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        vec![
            SemanticAbiArgumentV1::source(pair(READ)),
            SemanticAbiArgumentV1::source(pair(READ)),
            SemanticAbiArgumentV1::source(pair(WRITE)),
        ],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ])
    .unwrap();
    let locals = [
        UNIT, READ, READ, WRITE, WITNESS, INDEX, INDEX, BOOL, INDEX, BOOL, FLOAT, INDEX, BOOL,
        FLOAT, FLOAT, FLOAT,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([30 + index as u8; 32]),
            ty,
            match index {
                0 => SemanticLocalRoleV1::Return,
                1..=3 => SemanticLocalRoleV1::Argument(index as u32 - 1),
                _ => SemanticLocalRoleV1::Temporary,
            },
            SemanticSourceProvenanceV1::unavailable(),
        )
    })
    .collect();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([20; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([20; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"scalar_vecadd".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([20; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let intrinsic = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([21; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([21; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([21; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([21; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([21; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([21; 32]),
                SemanticLayoutIdentityV1::from_sha256([21; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![],
                SemanticAbiValueV1::new(
                    WITNESS,
                    SemanticAbiPassModeV1::Direct(attributes(false, false)),
                ),
            )
            .unwrap(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: WITNESS,
            raw_index: INDEX,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([21; 32]),
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            intrinsic,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "scalar_vecadd",
            [20; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch)
}

fn with_vecadd(
    case: Case,
    check: impl FnOnce(
        &mut ProductionSourceOutputOccurrencesV1<'_, '_>,
        &crate::CheckedOutputFormalMemoryAnalysisPolicy3V1<'_>,
        &mut Budget<'_>,
    ),
) {
    let (ssa, launch) = semantic_vecadd(case);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(17).unwrap();
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let source_storage = source.retained_analysis_storage_v1();
    budget.reserve_storage(source_storage).unwrap();
    let target = dialect_amdgcn::bind_production_target_v1(
        source.executable().module(),
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let (bound, bound_storage) =
        Owner::from_module_ref_with_verification_budget_v12(target.module(), &mut budget).unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    drop(target);
    let coordinates_storage = {
        let (coordinates, coordinates_storage) =
            dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                source.executable(),
                &bound,
                fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
                &mut budget,
            )
            .unwrap();
        budget
            .reserve_storage(coordinates_storage.retained_storage())
            .unwrap();
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
                .unwrap();
        let checked_storage = checked.storage().retained_storage();
        budget.reserve_storage(checked_storage).unwrap();
        let (mut view, view_storage) = derive_source_output_occurrences_policy3_v1(
            &source,
            &coordinates,
            &checked,
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(view_storage.retained_storage())
            .unwrap();
        let formal = crate::analyze_checked_output_formal_memory_policy3_v1(&checked).unwrap();
        let floor = budget.storage();
        check(&mut view, &formal, &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(formal);
        drop(view);
        budget
            .release_storage(view_storage.retained_storage())
            .unwrap();
        drop(checked);
        budget.release_storage(checked_storage).unwrap();
        coordinates_storage
    };
    budget
        .release_storage(coordinates_storage.retained_storage())
        .unwrap();
    drop(bound);
    budget
        .release_storage(bound_storage.retained_storage())
        .unwrap();
    drop(source);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), 17);
}

#[test]
fn policy3_semantic_vecadd_reads_write_and_pending_assertions_have_exact_coverage() {
    with_vecadd(Case::Vecadd, |view, formal, budget| {
        let floor = budget.storage();
        let (census, storage) =
            derive_scalar_output_obligations_policy3_v1(view, formal, budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(!census.grants_authority());
        assert!(std::ptr::eq(census.source_output(), view));
        assert!(std::ptr::eq(census.formal(), formal));
        assert_eq!(census.memory().len(), 3);
        assert_eq!(formal.kernels()[0].accesses().len(), 3);
        let mut allocations = Vec::new();
        for row in census.memory() {
            let access = row.formal_access().unwrap();
            assert!(row.executable());
            assert!(row.output().is_some());
            assert_eq!(access.byte_width(), 4);
            allocations.push((access.allocation().parameter_index(), access.kind()));
            assert_eq!(row.source_site().0, SemanticFunctionIdV1::from_index(0));
        }
        assert_eq!(
            allocations,
            vec![
                (0, FormalMemoryAccessKind::Read),
                (1, FormalMemoryAccessKind::Read),
                (2, FormalMemoryAccessKind::Write)
            ]
        );
        assert_eq!(census.assertions().len(), 2);
        for assertion in census.assertions() {
            assert!(matches!(
                assertion.binding().outcome(),
                SemanticKirOptimizedAssertOutcomeV1::Conditional { .. }
            ));
            view.assertion_arguments(assertion.binding(), budget)
                .unwrap();
        }
        assert_eq!(census.traps().len(), 1);
        assert_eq!(census.traps()[0].failure_edges(), 2);
        assert!(census.traps()[0].executable());
        drop(census);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn policy3_extra_semantic_read_is_covered_instead_of_assuming_three_effects() {
    with_vecadd(Case::ExtraRead, |view, formal, budget| {
        let (census, _) =
            derive_scalar_output_obligations_policy3_v1(view, formal, budget).unwrap();
        assert_eq!(census.memory().len(), 4);
        assert_eq!(formal.kernels()[0].accesses().len(), 4);
    });
}

#[test]
fn policy3_selected_success_and_dead_memory_keep_distinct_checked_outcomes() {
    with_vecadd(Case::SelectedSuccess, |view, formal, budget| {
        let (census, _) =
            derive_scalar_output_obligations_policy3_v1(view, formal, budget).unwrap();
        assert_eq!(census.memory().len(), 3);
        assert_eq!(census.assertions().len(), 2);
        for assertion in census.assertions() {
            assert!(matches!(
                assertion.binding().outcome(),
                SemanticKirOptimizedAssertOutcomeV1::SelectedSuccess { .. }
            ));
        }
        assert_eq!(census.traps().len(), 1);
        assert!(!census.traps()[0].executable());
    });
    with_vecadd(Case::DeadOutput, |view, formal, budget| {
        let (census, _) =
            derive_scalar_output_obligations_policy3_v1(view, formal, budget).unwrap();
        assert!(formal.kernels()[0].accesses().is_empty());
        assert_eq!(census.memory().len(), 3);
        for access in census.memory() {
            assert!(!access.executable());
            assert!(access.formal_access().is_none());
        }
        for assertion in census.assertions() {
            assert!(matches!(
                assertion.binding().outcome(),
                SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable { .. }
            ));
        }
    });
}

#[test]
fn policy3_selected_assertion_failure_rejects_despite_complete_formal_memory() {
    with_vecadd(Case::SelectedFailure, |view, formal, budget| {
        assert!(matches!(
            derive_scalar_output_obligations_policy3_v1(view, formal, budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "scalar output selects source assertion failure"
            ))
        ));
    });
}

#[test]
fn policy3_abort_cannot_use_another_assertions_trap_coverage() {
    with_vecadd(Case::Abort, |view, formal, budget| {
        assert!(matches!(
            derive_scalar_output_obligations_policy3_v1(view, formal, budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "scalar trap predecessor lacks an exact assertion obligation"
            ))
        ));
    });
}

#[test]
fn policy3_equal_bytes_foreign_formal_owner_is_not_substitutable() {
    with_vecadd(Case::Vecadd, |view, _, budget| {
        let other =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(view.bound(), budget)
                .unwrap();
        let other_storage = other.storage().retained_storage();
        budget.reserve_storage(other_storage).unwrap();
        assert_eq!(
            other.owner().canonical().canonical_bytes(),
            view.output().canonical().canonical_bytes()
        );
        let foreign = crate::analyze_checked_output_formal_memory_policy3_v1(&other).unwrap();
        assert!(matches!(
            derive_scalar_output_obligations_policy3_v1(view, &foreign, budget),
            Err(ProductionSourceOutputErrorV1::InputCustody)
        ));
        drop(foreign);
        drop(other);
        budget.release_storage(other_storage).unwrap();
    });
}

#[test]
fn policy3_missing_assertion_alias_cannot_hide_a_pending_obligation() {
    with_vecadd(Case::Vecadd, |view, formal, budget| {
        view.assertions.aliases.pop().unwrap();
        assert!(matches!(
            derive_scalar_output_obligations_policy3_v1(view, formal, budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "scalar assertion reverse coverage is incomplete"
            ))
        ));
    });
}

#[test]
fn policy3_private_effects_are_refused_even_when_formal_would_skip_them() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let operation = Operation::new(
        vec![ValueDef::new(
            ValueId(1),
            Type::pointer(Type::F32, AddressSpace::Private, AccessMode::ReadWrite),
        )],
        OperationKind::Alloca {
            element: Type::F32,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    );
    assert!(scalar_output_opcode_v1(&operation, &mut budget).is_err());
}

#[test]
fn policy3_census_resource_failures_and_unwind_restore_the_incoming_floor() {
    with_vecadd(Case::Vecadd, |view, formal, original| {
        let floor = original.storage();
        let before = original.work();
        let (census, _) =
            derive_scalar_output_obligations_policy3_v1(view, formal, original).unwrap();
        drop(census);
        let required = original.work() - before;
        for limit in [0, 1, 7, required / 2, required - 1] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            assert!(
                derive_scalar_output_obligations_policy3_v1(view, formal, &mut budget).is_err()
            );
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() <= limit);
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        assert!(derive_scalar_output_obligations_policy3_v1(view, formal, &mut budget).is_err());
        assert_eq!(budget.storage(), floor);
    });
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(17).unwrap();
    let result: ScalarOutputResultV1<()> = scalar_output_scope_v1(&mut budget, |budget| {
        scalar_output_reserve_v1(budget, 123)?;
        scalar_output_work_v1(budget, 9)?;
        panic!("injected census unwind");
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOutputErrorV1::Panicked)
    ));
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 9);
    assert_eq!(budget.peak_storage(), 140);
}
