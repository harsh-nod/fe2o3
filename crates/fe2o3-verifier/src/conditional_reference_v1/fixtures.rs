//! Descriptive component fixtures. No source request, signature receipt or proof.
use super::*;
use crate::portable_reference_v1::extraction::ReferenceWorkV1;
use crate::portable_reference_v1::signature::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExternAbiV1, SemanticFunctionSafetyV1, SemanticMutabilityV1,
};
use fe2o3_pliron::{ProductionRankedBlockV1 as Block, ProductionRankedTerminatorV1 as Term};

pub(super) fn identity(tag: u8) -> ReferenceFunctionIdentityV1 {
    ReferenceFunctionIdentityV1 {
        def_path_hash: [tag; 16],
        function_sha256: [tag; 32],
        item_definition_sha256: [tag + 1; 32],
        monomorphization_sha256: [tag + 2; 32],
        generic_type_arguments_sha256: [tag + 3; 32],
        const_generic_arguments_sha256: [tag + 4; 32],
        rustc_mir_body_sha256: [tag + 5; 32],
    }
}

pub(super) struct Fixture {
    pub kernel: ReferenceFunctionIdentityV1,
    pub reference: ReferenceFunctionIdentityV1,
    pub signature: ReferenceLogicalSignaturePreimageV1,
    pub ir: ReferenceEffectIrV1,
    pub writes: Vec<ReferenceOutputWriteV1>,
}

pub(super) fn local(local: u32) -> ReferencePlaceV1 {
    ReferencePlaceV1 {
        local,
        projection: Box::default(),
    }
}

fn copy(local_id: u32) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(local(local_id))
}

pub(super) fn point() -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
}

pub(super) fn cpu_load() -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::InputLoad {
        reference_argument: 1,
        index: Box::new(point()),
    }
}

pub(super) fn constant() -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::F32,
        bits: 0x422a_0000,
    })
}

pub(super) fn gpu_constant() -> Expr {
    Expr::Constant {
        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        bits: 0x422a_0000,
    }
}

impl Fixture {
    pub fn new() -> Self {
        let scalar = ReferenceScalarTypeV1::F32;
        let shared = ReferenceSignatureInputV1::Reference {
            region: ReferenceRegionV1::Erased,
            mutability: SemanticMutabilityV1::Immutable,
            pointee: ReferencePointeeV1::Slice(scalar),
        };
        let signature = ReferenceLogicalSignaturePreimageV1::new(
            vec![
                shared,
                ReferenceSignatureInputV1::NominalOutput {
                    carrier: ReferenceCarrierV1::DisjointSlice,
                    element: scalar,
                },
            ]
            .into_boxed_slice(),
            vec![
                ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::Usize),
                shared,
                ReferenceSignatureInputV1::Reference {
                    region: ReferenceRegionV1::Erased,
                    mutability: SemanticMutabilityV1::Mutable,
                    pointee: ReferencePointeeV1::Scalar(scalar),
                },
            ]
            .into_boxed_slice(),
            ReferenceReturnShapeV1::Unit,
            SemanticExternAbiV1::Rust,
            SemanticFunctionSafetyV1::Safe,
            false,
        )
        .unwrap();
        let derived = signature.derive_relations_v1().unwrap();
        let relations = (0..derived.len())
            .map(|raw| derived.relation_at_raw_argument_v1(raw as u32).unwrap())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let ir = ReferenceEffectIrV1 {
            argument_count: 3,
            local_count: 6,
            relations,
            blocks: vec![
                ReferenceBlockV1 {
                    block: 0,
                    assignments: vec![
                        ReferenceAssignmentV1 {
                            statement: 0,
                            destination: local(4),
                            value: ReferenceValueV1::InputLength {
                                reference_argument: 1,
                            },
                        },
                        ReferenceAssignmentV1 {
                            statement: 1,
                            destination: local(5),
                            value: ReferenceValueV1::Binary {
                                operation: ReferenceBinaryOpV1::LessThan,
                                lhs: copy(1),
                                rhs: copy(4),
                                checked: false,
                            },
                        },
                    ]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Assert {
                        condition: copy(5),
                        expected: true,
                        success: 1,
                        bounds_check: Some(ReferenceBoundsCheckV1 {
                            index: copy(1),
                            length: copy(4),
                        }),
                    },
                },
                ReferenceBlockV1 {
                    block: 1,
                    assignments: vec![ReferenceAssignmentV1 {
                        statement: 0,
                        destination: ReferencePlaceV1 {
                            local: 3,
                            projection: vec![ReferencePlaceProjectionV1::Dereference]
                                .into_boxed_slice(),
                        },
                        value: ReferenceValueV1::Use(ReferenceOperandV1::Copy(ReferencePlaceV1 {
                            local: 2,
                            projection: vec![
                                ReferencePlaceProjectionV1::Dereference,
                                ReferencePlaceProjectionV1::Index(1),
                            ]
                            .into_boxed_slice(),
                        })),
                    }]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Return,
                },
            ]
            .into_boxed_slice(),
            loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        };
        let mut fixture = Self {
            kernel: identity(10),
            reference: identity(20),
            signature,
            ir,
            writes: vec![],
        };
        fixture.refresh();
        fixture
    }

    pub fn refresh(&mut self) {
        struct Descriptive;
        impl ReferenceWorkV1 for Descriptive {
            fn charge(&self, _: usize) -> Result<(), ReferenceBindingErrorV1> {
                Ok(())
            }
        }
        self.writes = self.ir.observable_output_writes_v1(&Descriptive).unwrap();
        self.ir.observable_output_effects = self.writes.clone().into_boxed_slice();
    }

    pub fn input(&self) -> ConditionalReferenceInputV1<'_> {
        ConditionalReferenceInputV1 {
            kernel: &self.kernel,
            reference: &self.reference,
            replay: ReferenceReplayInputV1 {
                signature_preimage: &self.signature,
                effect_ir: &self.ir,
                effect_ir_sha256: self.ir.canonical_sha256_v1(),
                observable_output_writes: &self.writes,
            },
        }
    }
}

pub(super) fn kernel() -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new(
        "component_only",
        2,
        vec![Block::new(
            vec![
                Op::InvocationIndex {
                    result: ProductionRankedValueIdV1::new(0),
                    dimension: 0,
                    launch_extent: 64,
                },
                Op::ViewInSpace {
                    result: ProductionRankedValueIdV1::new(1),
                    element_width: 32,
                    writable: false,
                    shape: vec![dialect_kernel::DYNAMIC_EXTENT],
                    dynamic_extents: vec![Value::Argument(0)],
                    allocation_origin: 1,
                    noalias_class: 1,
                    memory_space: dialect_kernel::MemorySpaceAttr::Global,
                },
                Op::Access {
                    kind: dialect_kernel::AccessKindAttr::Read,
                    view: Value::Local(ProductionRankedValueIdV1::new(1)),
                    indices: vec![Value::Local(ProductionRankedValueIdV1::new(0))],
                },
            ],
            Term::Return,
        )],
    )
    .unwrap()
}

pub(super) fn gpu_load() -> Expr {
    Expr::Load(ProductionSemanticLoadV2 {
        block: 0,
        operation: 2,
        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        allocation_origin: 1,
        view: Value::Local(ProductionRankedValueIdV1::new(1)),
        indices: vec![Value::Local(ProductionRankedValueIdV1::new(0))].into_boxed_slice(),
    })
}
