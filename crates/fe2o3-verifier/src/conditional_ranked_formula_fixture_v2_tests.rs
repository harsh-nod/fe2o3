//! Inert codec/statement components, never a source request or proof fixture.
use super::*;
use crate::portable_reference_v1::{codec::NativeCpuAssociationV1, signature::*, *};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExternAbiV1, SemanticFunctionSafetyV1, SemanticMutabilityV1,
};

pub(super) fn digest(n: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([n; 32])
}

fn identity(n: u8) -> ReferenceFunctionIdentityV1 {
    ReferenceFunctionIdentityV1 {
        def_path_hash: [n; 16],
        function_sha256: [n; 32],
        item_definition_sha256: [n + 1; 32],
        monomorphization_sha256: [n + 2; 32],
        generic_type_arguments_sha256: [n + 3; 32],
        const_generic_arguments_sha256: [n + 4; 32],
        rustc_mir_body_sha256: [n + 5; 32],
    }
}

pub(super) fn constant(bits: u128) -> ReferenceConstantV1 {
    ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::F32,
        bits,
    }
}

pub(super) struct Fixture {
    pub signature: ReferenceLogicalSignaturePreimageV1,
    pub ir: ReferenceEffectIrV1,
    pub kernel: ReferenceFunctionIdentityV1,
    pub reference: ReferenceFunctionIdentityV1,
}
impl Fixture {
    pub fn new() -> Self {
        let scalar = ReferenceScalarTypeV1::F32;
        let signature = ReferenceLogicalSignaturePreimageV1::new(
            vec![ReferenceSignatureInputV1::NominalOutput {
                carrier: ReferenceCarrierV1::DisjointSlice,
                element: scalar,
            }]
            .into_boxed_slice(),
            vec![
                ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::Usize),
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
        let value = ReferenceValueV1::Use(ReferenceOperandV1::Constant(constant(0x422a_0000)));
        let write = ReferenceOutputWriteV1 {
            argument: 0,
            block: 0,
            statement: 0,
            coordinate: ReferenceOutputCoordinateV1::LogicalPoint(
                vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice(),
            ),
            guard: ReferencePathPredicateV1::unconditional_v1(),
            rhs: ReferenceEffectExpressionV1::Constant(constant(0x422a_0000)),
            value: value.clone(),
        };
        Self {
            signature,
            kernel: identity(11),
            reference: identity(21),
            ir: ReferenceEffectIrV1 {
                argument_count: 2,
                local_count: 3,
                relations,
                blocks: vec![ReferenceBlockV1 {
                    block: 0,
                    assignments: vec![ReferenceAssignmentV1 {
                        statement: 0,
                        destination: ReferencePlaceV1 {
                            local: 2,
                            projection: vec![ReferencePlaceProjectionV1::Dereference]
                                .into_boxed_slice(),
                        },
                        value,
                    }]
                    .into_boxed_slice(),
                    terminator: ReferenceTerminatorV1::Return,
                }]
                .into_boxed_slice(),
                loop_summaries: Box::default(),
                observable_output_effects: vec![write].into_boxed_slice(),
            },
        }
    }

    pub fn input(&self) -> NativeCpuInputV1<'_> {
        NativeCpuInputV1 {
            association: NativeCpuAssociationV1 {
                semantic_mir_sha256: [31; 32],
                semantic_root: 0,
                registration_path: "component::registration",
                logical_kernel_name: "component",
            },
            kernel: &self.kernel,
            reference: &self.reference,
            replay: ReferenceReplayInputV1 {
                signature_preimage: &self.signature,
                effect_ir: &self.ir,
                effect_ir_sha256: self.ir.canonical_sha256_v1(),
                observable_output_writes: &self.ir.observable_output_effects,
            },
        }
    }

    pub fn change_operand(&mut self) {
        let changed = constant(0x3f80_0000);
        let value = ReferenceValueV1::Use(ReferenceOperandV1::Constant(changed.clone()));
        self.ir.blocks[0].assignments[0].value = value.clone();
        self.ir.observable_output_effects[0].value = value;
        self.ir.observable_output_effects[0].rhs = ReferenceEffectExpressionV1::Constant(changed);
    }
}
