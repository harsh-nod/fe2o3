use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 17;
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const RESULT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Clone, Copy, Default)]
struct Fixture {
    roots: u32,
    shared: bool,
    intrinsic: bool,
    helper: bool,
    malformed: bool,
    administrative: usize,
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn value(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn literal() -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
    )))
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
fn call(callee: u32, destination: u32, ty: SemanticTypeIdV1) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(destination, ty),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let variant = |index| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            4,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
            SemanticBackendReprV1::scalar(tag),
            None,
            false,
            None,
            4,
            0,
            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
        )
        .unwrap()
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
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                4,
                4,
                SemanticBackendReprV1::scalar(tag),
                false,
                SemanticEnumLayoutV1::new(
                    vec![variant(0), variant(1)],
                    SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::enum_type(
                U32,
                vec![
                    SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![UNIT]).unwrap(),
                    ),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![UNIT]).unwrap(),
                    ),
                ],
            )
            .unwrap(),
        ),
    ]
}

fn abi(tag: u8, root: bool, returned: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
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
        0,
        vec![],
        SemanticAbiValueV1::new(
            returned,
            if root {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                )
            },
        ),
    )
    .unwrap()
}

fn function(
    tag: u8,
    root: bool,
    returned: SemanticTypeIdV1,
    locals: &[SemanticTypeIdV1],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
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
        abi(tag, root, returned),
        locals
            .iter()
            .enumerate()
            .map(|(ordinal, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + ordinal as u8 + 1; 32]),
                    *ty,
                    if ordinal == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn request(fixture: Fixture) -> AdmittedInertSemanticMirV1 {
    let roots = fixture.roots.max(1);
    let bodies = if fixture.shared { 1 } else { roots };
    let helper = roots + bodies;
    let intrinsic = helper + u32::from(fixture.helper);
    assert!(!(fixture.helper && fixture.intrinsic));
    let mut functions = Vec::new();
    for root in 0..roots {
        let tag = 30 + root as u8;
        let mut statements = vec![
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            );
            fixture.administrative
        ];
        if fixture.malformed {
            statements.push(assignment(
                0,
                UNIT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ));
        }
        let wrapper = function(
            tag,
            true,
            UNIT,
            &[UNIT, RESULT],
            vec![
                block(
                    tag,
                    statements,
                    call(roots + if fixture.shared { 0 } else { root }, 1, RESULT),
                ),
                block(tag + 10, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(format!("selected_wrapper_{root}").into_bytes()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([tag; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        functions.push(wrapper);
    }
    for body in 0..bodies {
        let tag = 60 + body as u8;
        let entry = if fixture.intrinsic {
            block(tag, vec![], call(intrinsic, 1, U32))
        } else if fixture.helper {
            block(tag, vec![], call(helper, 1, U32))
        } else {
            block(
                tag,
                vec![assignment(1, U32, literal())],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            )
        };
        functions.push(function(
            tag,
            false,
            RESULT,
            &[RESULT, U32, U32, U32],
            vec![
                entry,
                block(
                    tag + 10,
                    vec![
                        SemanticStatementV1::new(
                            SemanticSourceProvenanceV1::unavailable(),
                            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                                place(2, U32),
                                value(1, U32),
                                SemanticVolatilityV1::NonVolatile,
                                None,
                            )),
                        ),
                        assignment(3, U32, SemanticRvalueKindV1::Use(value(2, U32))),
                        assignment(
                            0,
                            RESULT,
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::EnumVariant(0),
                                    vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                                        UNIT,
                                        SemanticConstantValueV1::ZeroSized,
                                    ))],
                                )
                                .unwrap(),
                            ),
                        ),
                    ],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        ));
    }
    if fixture.helper {
        functions.push(function(
            100,
            false,
            U32,
            &[U32],
            vec![block(
                100,
                vec![assignment(0, U32, literal())],
                SemanticTerminatorKindV1::Return,
            )],
        ));
    }
    let mut callables: Vec<_> = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    if fixture.intrinsic {
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([120; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([120; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([120; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([120; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([120; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi(120, false, U32),
            ),
            operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X),
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([120; 32]),
        });
    }
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        (0..roots).map(SemanticFunctionIdV1::from_index).collect(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn materialize(fixture: Fixture) -> ProductionPreRankedKirOwnerV1 {
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        request(fixture),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let names: Vec<_> = (0..fixture.roots.max(1))
        .map(|root| format!("logical_{root}"))
        .collect();
    let inputs: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(root, name)| {
            crate::ProductionSourceLaunchRootInputV1::new(
                name,
                [30 + root as u8; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &inputs).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    owner
}

fn receipt(fixture: Fixture) -> ProductionMaterializedRankedModuleReceiptV1 {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let source = materialize(fixture);
    let roots = source.source_launch().roots().iter().enumerate().map(|(ordinal, root)| {
        let layout = root.layout();
        let name = format!("selected_wrapper_{ordinal}");
        let kernel = ProductionRankedKernelV1::new(&name, 0, vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: layout.grid_identity(), global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(), subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }], ProductionRankedTerminatorV1::Return,
        )]).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel(&name, kernel).unwrap(), ProductionSessionLimitsV1::default(),
        ).unwrap();
        assert!(lowering.all_mandatory_reports_are_clean());
        ProductionRankedSemanticProjectionRootV1::new(SemanticFunctionIdV1::from_index(ordinal as u32), 1, lowering,
            "genuine selected-body source/N and compiled no-global-effect ranked component; not the backend projector".to_owned(), vec![], vec![])
    }).collect();
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        source, roots,
    )
    .unwrap()
}

fn owner(fixture: Fixture) -> ProductionSemanticKirOwnerV1 {
    ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt(fixture)).unwrap()
}

fn run_census(
    owner: &ProductionSemanticKirOwnerV1,
    limit: usize,
    storage: usize,
) -> (R<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
    budget.reserve_storage(FLOOR).unwrap();
    let result = check_source(owner, &mut budget).map(|_| ());
    let scratch = budget.storage() - FLOOR;
    budget.release_storage(scratch).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), scratch)
}
