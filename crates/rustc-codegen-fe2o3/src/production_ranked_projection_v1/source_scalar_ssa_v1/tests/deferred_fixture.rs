//! Consumer coupling fixtures. Private projection receipts are test inputs,
//! not replacements for the production source-authentication producer.
use super::*;
use fixtures::{ROOT, U32, Shape, assignment, block, constant, place};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1};

pub(super) fn rebuild(
    original: &SemanticFunctionDeclV1,
    types: Vec<SemanticTypeDeclV1>,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
    callables: Vec<SemanticCallableDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([56; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(request, Default::default()).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(semantic, Default::default()).unwrap();
    owner.verify_replay().unwrap();
    owner
}

pub(super) fn discriminant_owner() -> ProductionSemanticSsaOwnerV1 {
    let seed = fixtures::owner(Shape::IntegerCondition);
    let original = &seed.source_semantic().functions()[0];
    let mut types = seed.source_semantic().types().to_vec();
    let enumeration = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([160; 32]),
        SemanticLayoutIdentityV1::from_sha256([161; 32]),
        SemanticTypeLayoutV1::enum_layout(
            8,
            4,
            SemanticEnumLayoutV1::new(
                (0..2)
                    .map(|index| {
                        SemanticEnumVariantLayoutV1::from_rustc(
                            index,
                            8,
                            4,
                            SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
                            SemanticBackendReprV1::memory(true),
                            None,
                            false,
                            None,
                            4,
                            100 + u64::from(index),
                            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
                        )
                        .unwrap()
                    })
                    .collect(),
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 32, 4),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            (0..2)
                .map(|index| {
                    SemanticEnumVariantV1::new(
                        index,
                        SemanticAggregateTypeV1::new(vec![U32]).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    ));
    let mut locals = original.locals().to_vec();
    let value = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([162; 32]),
        enumeration,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let mut blocks = original.blocks().to_vec();
    blocks[0] = block(
        20,
        vec![
            assignment(
                value,
                enumeration,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(1),
                        vec![constant(5)],
                    )
                    .unwrap(),
                ),
            ),
            assignment(
                1,
                U32,
                SemanticRvalueKindV1::Discriminant(place(value, enumeration)),
            ),
        ],
        blocks[0].terminator().kind().clone(),
    );
    rebuild(
        original,
        types,
        locals,
        blocks,
        vec![SemanticCallableDeclV1::defined(ROOT)],
    )
}

pub(super) fn empty_intrinsic(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
) -> IntrinsicProjectionV1 {
    IntrinsicProjectionV1 {
        index_values: vec![],
        ordinary_index_values: vec![],
        local_contracts: ProjectionLocalContractsV1 {
            checked_references: CheckedReferencesV1 {
                origins: vec![],
                option_dominance: SemanticOptionDominanceV1::analyze(function, &[]).unwrap(),
                enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(function, types)
                    .unwrap(),
            },
            allocations: vec![],
            allocation_provenance: vec![],
        },
        guarded_accesses: vec![],
        option_predicates: vec![],
        direct_switch_predicates: vec![],
        deterministic_switches: vec![],
        uniform_inductions: vec![],
        tensor_layouts: vec![],
        capability_read_effects: vec![],
        transpose_workgroup_effects: vec![],
        read_view_effects: vec![],
        direct_read_effects: vec![],
        direct_write_effects: vec![],
        global_uses: Default::default(),
        global_views: vec![],
        pipeline_effects: vec![],
        generated_terminator_effects: vec![],
        extent_argument_count: 0,
    }
}

pub(super) fn fact_intrinsic(
    owner: &ProductionSemanticSsaOwnerV1,
    changed_extent: bool,
) -> IntrinsicProjectionV1 {
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let mut intrinsic = empty_intrinsic(function, owner.source_semantic().types());
    let (block, statement) = function
        .blocks()
        .iter()
        .enumerate()
        .find_map(|(b, block)| {
            block
                .statements()
                .iter()
                .enumerate()
                .find_map(|(s, statement)| {
                    matches!(statement.kind(), SemanticStatementKindV1::Assign(a)
                if matches!(a.value().kind(), SemanticRvalueKindV1::Discriminant(_)))
                    .then_some((b, s))
                })
        })
        .unwrap();
    intrinsic
        .global_uses
        .discriminants
        .insert((block, statement), 0);
    intrinsic
        .direct_read_effects
        .push(Some(GuardedRankedAccessV1 {
            view: ProductionRankedValueIdV1::new(1),
            indices: vec![ranked(0)],
            checked_success: None,
            comparisons: vec![(
                ranked(0),
                ProductionRankedValueV1::Argument(u32::from(changed_extent)),
            )],
            access: AccessKindAttr::Read,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: None,
        }));
    intrinsic
}

pub(super) fn ranked(id: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id))
}

pub(super) fn write(block: usize) -> RankedGpuWriteV2 {
    RankedGpuWriteV2 {
        block,
        operation: 0,
        allocation_origin: 3,
        view: ranked(3),
        indices: vec![ranked(0)],
        value: Err(AMBIGUOUS),
    }
}

pub(super) fn source(
    owner: &ProductionSemanticSsaOwnerV1,
    block: usize,
) -> ProjectedAccessSourceV1 {
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let (site, _) = fixtures::use_site(function);
    ProjectedAccessSourceV1 {
        block,
        operation: 0,
        access: AccessKindAttr::Write,
        memory_space: MemorySpaceAttr::Global,
        source: function.blocks()[site.block().index() as usize].statements()
            [site.statement().unwrap() as usize]
            .source(),
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block: site.block().index() as usize,
            statement: site.statement().map(|s| s as usize),
        }),
    }
}

pub(super) fn kernel(guarded: bool) -> ProductionRankedKernelV1 {
    let mut operations = vec![ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 1024,
    }];
    for index in 0..3 {
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(index + 1),
            element_width: 32,
            writable: index == 2,
            shape: vec![if index == 2 { 1024 } else { DYNAMIC_EXTENT }],
            dynamic_extents: if index == 2 {
                vec![]
            } else {
                vec![ProductionRankedValueV1::Argument(index)]
            },
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: u64::from(index + 1),
            noalias_class: u64::from(index + 1),
        });
    }
    let store = || {
        ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view: ranked(3),
                indices: vec![ranked(0)],
            }],
            ProductionRankedTerminatorV1::Return,
        )
    };
    ProductionRankedKernelV1::new(
        "deferred_source_guard",
        2,
        vec![
            ProductionRankedBlockV1::new(
                operations,
                if guarded {
                    ProductionRankedTerminatorV1::IndexLessThan {
                        lhs: ranked(0),
                        rhs: ProductionRankedValueV1::Argument(0),
                        true_block: 1,
                        false_block: 2,
                    }
                } else {
                    ProductionRankedTerminatorV1::Branch { target: 1 }
                },
            ),
            store(),
            store(),
        ],
    )
    .unwrap()
}

pub(super) fn cpu_ir() -> crate::reference_effect_v1::ReferenceEffectIrV1 {
    use crate::reference_effect_v1::*;
    let place = |local| ReferencePlaceV1 {
        local,
        projection: Box::default(),
    };
    let operand = |local| ReferenceOperandV1::Copy(place(local));
    ReferenceEffectIrV1 {
        argument_count: 4,
        local_count: 6,
        relations: vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 0,
                element: ReferenceScalarTypeV1::U32,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 1,
                element: ReferenceScalarTypeV1::U32,
            },
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                argument: 2,
                element: ReferenceScalarTypeV1::U32,
            },
        ]
        .into_boxed_slice(),
        blocks: vec![
            ReferenceBlockV1 {
                block: 0,
                assignments: vec![
                    ReferenceAssignmentV1 {
                        statement: 0,
                        destination: place(4),
                        value: ReferenceValueV1::InputLength {
                            reference_argument: 1,
                        },
                    },
                    ReferenceAssignmentV1 {
                        statement: 1,
                        destination: place(5),
                        value: ReferenceValueV1::Binary {
                            operation: ReferenceBinaryOpV1::LessThan,
                            lhs: operand(0),
                            rhs: operand(4),
                            checked: false,
                        },
                    },
                ]
                .into_boxed_slice(),
                terminator: ReferenceTerminatorV1::Assert {
                    condition: operand(5),
                    expected: true,
                    success: 1,
                    bounds_check: Some(ReferenceBoundsCheckV1 {
                        index: operand(0),
                        length: operand(4),
                    }),
                },
            },
            ReferenceBlockV1 {
                block: 1,
                assignments: Box::default(),
                terminator: ReferenceTerminatorV1::Return,
            },
        ]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    }
}
