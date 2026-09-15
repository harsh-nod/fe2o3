//! Validated source obligations, deliberately without a bare-FP conversion.

use fe2o3_kernel_ir::{F32MathFunction, F32MathImplementation, NumericalModeV1};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionIdentityV1, SemanticKernelCapabilityProvenanceV1, SemanticMutabilityV1,
    SemanticPointerKindV1, SemanticPointerMetadataV1, SemanticScalarTypeV1, SemanticTypeDeclV1,
    SemanticTypeIdV1, SemanticTypeIdentityV1, SemanticTypeShapeV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PolicyMathTypesV1 {
    pub bound_reference: SemanticTypeIdV1,
    pub bound: SemanticTypeIdV1,
    pub math_reference: SemanticTypeIdV1,
    pub math: SemanticTypeIdV1,
    pub policy_reference: SemanticTypeIdV1,
    pub capability: SemanticTypeIdV1,
    pub element: SemanticTypeIdV1,
}

impl PolicyMathTypesV1 {
    pub(super) fn new(ids: [SemanticTypeIdV1; 7]) -> Self {
        let [
            bound_reference,
            bound,
            math_reference,
            math,
            policy_reference,
            capability,
            element,
        ] = ids;
        Self {
            bound_reference,
            bound,
            math_reference,
            math,
            policy_reference,
            capability,
            element,
        }
    }

    pub(crate) fn all(self) -> [SemanticTypeIdV1; 7] {
        [
            self.bound_reference,
            self.bound,
            self.math_reference,
            self.math,
            self.policy_reference,
            self.capability,
            self.element,
        ]
    }
}

/// Issuance and constructor custody must still dominate this consumer in SSA.
/// Strict numerical semantics and the operation-specific target refinement
/// remain obligations, even after these source/type checks succeed.
#[must_use = "retain the policy, provenance, and numerical requirements in semantic MIR"]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PolicyMathSourceContractV1 {
    types: PolicyMathTypesV1,
    policy: SemanticTypeIdentityV1,
    kernel_brand: SemanticTypeIdentityV1,
    function: F32MathFunction,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
}

impl PolicyMathSourceContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        ids: PolicyMathTypesV1,
        policy: SemanticTypeIdentityV1,
        kernel_brand: SemanticTypeIdentityV1,
        function: F32MathFunction,
        provenance: SemanticKernelCapabilityProvenanceV1,
        source_identity: SemanticFunctionIdentityV1,
        types: &[SemanticTypeDeclV1],
    ) -> Result<Self, &'static str> {
        if policy.as_bytes() == &[0; 32]
            || kernel_brand.as_bytes() == &[0; 32]
            || kernel_brand == policy
            || source_identity.as_bytes() == &[0; 32]
            || function == F32MathFunction::Abs
        {
            return Err("policy math has incomplete source, brand, policy, or function");
        }
        let all = ids.all();
        let mut identities = vec![policy, kernel_brand];
        for (index, id) in all.iter().enumerate() {
            let ty = types
                .get(id.index() as usize)
                .ok_or("policy math type is missing")?;
            if all[..index].contains(id)
                || ty.identity().as_bytes() == &[0; 32]
                || identities.contains(&ty.identity())
                || types
                    .iter()
                    .filter(|other| other.identity() == ty.identity())
                    .count()
                    != 1
                || ty.layout().is_uninhabited()
            {
                return Err("policy math type identities must be exact and distinct");
            }
            identities.push(ty.identity());
        }
        for (reference, pointee) in [
            (ids.bound_reference, ids.bound),
            (ids.math_reference, ids.math),
            (ids.policy_reference, ids.capability),
        ] {
            if !matches!(types[reference.index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
                if pointer.pointee() == pointee
                    && pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                    && pointer.metadata() == SemanticPointerMetadataV1::None)
            {
                return Err("policy math must preserve exact shared reference edges");
            }
        }
        let SemanticTypeShapeV1::Aggregate(bound) = types[ids.bound.index() as usize].shape()
        else {
            return Err("policy math wrapper must remain an aggregate");
        };
        let [math, policy_reference, phantom] = bound.fields() else {
            return Err("policy math wrapper must retain both references and its marker");
        };
        if *math != ids.math_reference
            || *policy_reference != ids.policy_reference
            || types.get(phantom.index() as usize).is_none_or(|ty| {
                ty.layout().size_bytes() != Some(0) || ty.layout().is_uninhabited()
            })
        {
            return Err("policy math wrapper has substituted or reordered fields");
        }
        for id in [ids.math, ids.capability] {
            let ty = &types[id.index() as usize];
            if !matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
                || ty.layout().size_bytes() != Some(0)
                || ty.layout().alignment_bytes() != 1
            {
                return Err("policy math capability must retain its exact inhabited ZST type");
            }
        }
        if !matches!(
            types[ids.element.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
        ) || types[ids.element.index() as usize].layout().size_bytes() != Some(4)
        {
            return Err("policy math element must be FP32");
        }
        Ok(Self {
            types: ids,
            policy,
            kernel_brand,
            function,
            provenance,
            source_identity,
        })
    }

    pub(crate) fn types(self) -> PolicyMathTypesV1 {
        self.types
    }
    pub(crate) fn policy(self) -> SemanticTypeIdentityV1 {
        self.policy
    }
    pub(crate) fn kernel_brand(self) -> SemanticTypeIdentityV1 {
        self.kernel_brand
    }
    pub(crate) fn function(self) -> F32MathFunction {
        self.function
    }
    pub(crate) fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub(crate) fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }

    pub(crate) fn arguments(self) -> Vec<SemanticTypeIdV1> {
        let mut arguments = vec![self.types.bound_reference];
        arguments.extend(std::iter::repeat_n(
            self.types.element,
            self.function.arity(),
        ));
        arguments
    }

    /// Required behavior, never evidence that the backend has implemented it.
    pub(crate) fn numerical_requirements(self) -> (NumericalModeV1, F32MathImplementation) {
        (
            NumericalModeV1::StrictIeee,
            self.function.required_implementation(),
        )
    }
}

#[cfg(test)]
#[path = "contract/tests.rs"]
mod tests;
