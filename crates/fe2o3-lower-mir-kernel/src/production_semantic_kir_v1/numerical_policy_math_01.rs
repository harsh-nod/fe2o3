use fe2o3_kernel_ir::{NumericalPolicyMathBindingV1, NumericalPolicyMathOperationV1};
use fe2o3_mir_model::semantic_mir_v1::SemanticNumericalPolicyMathContractV1;

mod numerical_policy_math_occurrences_01 {
    use super::*;
    include!("numerical_policy_math_01/occurrences.rs");
}

mod numerical_policy_math_custody_01 {
    use super::*;
    include!("numerical_policy_math_01/custody.rs");
}

include!("numerical_policy_math_01/production.rs");
include!("numerical_policy_math_01/bridge.rs");

#[cfg(test)]
mod numerical_policy_math_ssa_tests {
    include!("numerical_policy_math_01/ssa_tests.rs");
}

// This is an inert type/operation adapter. Its inputs must come from the checked
// source occurrence; it neither discovers nor issues authority from type facts.
#[derive(Clone, Debug)]
struct NumericalPolicyMathLoweringV1 {
    binding: NumericalPolicyMathBindingV1,
    element: ExecutionTypeIdentityV1,
    function: F32MathFunction,
    provenance: ExecutionCapabilityProvenanceV1,
}

impl NumericalPolicyMathLoweringV1 {
    fn new(
        types: &[SemanticTypeDeclV1],
        contract: SemanticNumericalPolicyMathContractV1,
        context: &KernelContextTypeV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let ids = contract.types();
        let source = contract.provenance();
        if context.kernel_marker() != source.kernel_marker().as_bytes()
            || context.target() != source.target_brand().as_bytes()
            || context.launch() != source.launch_brand().as_bytes()
            || execution_scalar_v1(types, ids.element)? != ScalarType::F32
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        for (reference, pointee) in [
            (ids.bound_reference, ids.bound),
            (ids.math_reference, ids.math),
            (ids.policy_reference, ids.capability),
        ] {
            if !numerical_policy_math_shared_type_v1(types, reference, pointee) {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "policy math lost an exact shared reference edge",
                ));
            }
        }
        let Some(SemanticTypeShapeV1::Aggregate(wrapper)) = types
            .get(ids.bound.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let [math_reference, policy_reference, marker] = wrapper.fields() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if *math_reference != ids.math_reference
            || *policy_reference != ids.policy_reference
            || [ids.math, ids.capability, *marker].iter().any(|id| {
                !types.get(id.index() as usize).is_some_and(|ty| {
                    ty.layout().size_bytes() == Some(0) && !ty.layout().is_uninhabited()
                })
            })
        {
            return Err(unsupported(
                0,
                None,
                None,
                "policy math constructor fields or retained referents changed",
            ));
        }
        let binding = NumericalPolicyMathBindingV1 {
            math_reference: execution_type_identity_v1(types, ids.math_reference)?,
            math: execution_type_identity_v1(types, ids.math)?,
            policy_reference: execution_type_identity_v1(types, ids.policy_reference)?,
            capability: execution_type_identity_v1(types, ids.capability)?,
            bound: execution_type_identity_v1(types, ids.bound)?,
            bound_reference: execution_type_identity_v1(types, ids.bound_reference)?,
            policy: ExecutionTypeIdentityV1::new(*contract.policy().as_bytes()),
            kernel_brand: ExecutionTypeIdentityV1::new(*contract.kernel_brand().as_bytes()),
            mode: fe2o3_kernel_ir::NumericalModeV1::StrictIeee,
        };
        let lowering = Self {
            binding,
            element: execution_type_identity_v1(types, ids.element)?,
            function: lower_f32_math_function(contract.function()),
            provenance: ExecutionCapabilityProvenanceV1 {
                root: context.root().clone(),
                kernel_binding: *source.kernel_binding().as_bytes(),
                frontend_unit: *source.frontend_unit().as_bytes(),
                kernel_marker: *source.kernel_marker().as_bytes(),
                target_brand: *source.target_brand().as_bytes(),
                launch_brand: *source.launch_brand().as_bytes(),
                issuance: *source.issuance().as_bytes(),
            },
        };
        if !lowering.consumer().is_well_formed() {
            return Err(unsupported(
                0,
                None,
                None,
                "policy math substituted a nominal type identity",
            ));
        }
        Ok(lowering)
    }

    fn consumer(&self) -> NumericalPolicyMathOperationV1 {
        NumericalPolicyMathOperationV1::F32 {
            binding: self.binding,
            bound_reference: self.binding.bound_reference,
            element: self.element,
            function: self.function,
        }
    }

    fn operation(
        &self,
        operation: NumericalPolicyMathOperationV1,
        source: ExecutionCapabilitySourceV1,
        operands: &[(ValueId, Type)],
    ) -> Result<(ExecutionCapabilityOpV1, Type), ProductionSemanticKirErrorV1> {
        if operation.binding() != self.binding || !operation.is_well_formed() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let (inputs, output, expected) = match operation {
            NumericalPolicyMathOperationV1::MathDerive { context, binding } => (
                vec![context],
                binding.math,
                vec![Type::KernelContext(KernelContextTypeV1::new(
                    self.provenance.root.clone(),
                    self.provenance.kernel_marker,
                    self.provenance.target_brand,
                    self.provenance.launch_brand,
                ))],
            ),
            NumericalPolicyMathOperationV1::Bind { binding } => (
                vec![binding.math_reference, binding.policy_reference],
                binding.bound,
                vec![
                    numerical_policy_math_capability_type_v1(
                        binding.math,
                        ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding),
                        &self.provenance,
                    ),
                    numerical_policy_math_capability_type_v1(
                        binding.capability,
                        ExecutionCapabilityRoleV1::NumericalPolicy {
                            policy: binding.policy,
                            mode: binding.mode,
                        },
                        &self.provenance,
                    ),
                ],
            ),
            NumericalPolicyMathOperationV1::F32 {
                binding,
                bound_reference,
                element,
                function,
            } => {
                if operation != self.consumer() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let mut inputs = vec![bound_reference];
                inputs.extend(std::iter::repeat_n(element, function.arity()));
                let mut expected = vec![numerical_policy_math_capability_type_v1(
                    binding.bound,
                    ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding),
                    &self.provenance,
                )];
                expected.extend(std::iter::repeat_n(Type::F32, function.arity()));
                (inputs, element, expected)
            }
        };
        if operands.len() != expected.len() || operands.iter().map(|(_, ty)| ty).ne(expected.iter())
        {
            return Err(unsupported(
                0,
                Some(source.block),
                None,
                "policy math requires exact typed authority and scalar SSA operands",
            ));
        }
        let result = numerical_policy_math_result_type_v1(operation, &self.provenance)?;
        let contract = ExecutionCapabilityOpV1 {
            operands: operands.iter().map(|(value, _)| *value).collect(),
            operation: ExecutionCapabilityOperationV1::NumericalPolicyMath(operation),
            signature: ExecutionCapabilitySignatureV1::new(&inputs, output)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            provenance: self.provenance.clone(),
            workgroup_brand: None,
            epoch_before: None,
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(operation.obligations()),
            source,
        };
        if !contract.is_complete() {
            return Err(unsupported(
                0,
                Some(contract.source.block),
                None,
                "policy math source contract is incomplete",
            ));
        }
        Ok((contract, result))
    }
}

fn numerical_policy_math_shared_type_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    matches!(
        types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer))
            if pointer.pointee() == pointee
                && pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Immutable
                && pointer.address_space() == 0
                && pointer.pointer_width_bits() == 64
                && pointer.metadata() == SemanticPointerMetadataV1::None
    )
}

fn numerical_policy_math_capability_type_v1(
    source_type: ExecutionTypeIdentityV1,
    role: ExecutionCapabilityRoleV1,
    provenance: &ExecutionCapabilityProvenanceV1,
) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance.clone(),
        workgroup_brand: None,
        epoch: None,
        role,
    })
}

fn numerical_policy_math_result_type_v1(
    operation: NumericalPolicyMathOperationV1,
    provenance: &ExecutionCapabilityProvenanceV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    if !operation.is_well_formed() || !provenance.is_complete() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(match operation {
        NumericalPolicyMathOperationV1::MathDerive { binding, .. } => {
            numerical_policy_math_capability_type_v1(
                binding.math,
                ExecutionCapabilityRoleV1::NumericalPolicyMathSource(binding),
                provenance,
            )
        }
        NumericalPolicyMathOperationV1::Bind { binding } => {
            numerical_policy_math_capability_type_v1(
                binding.bound,
                ExecutionCapabilityRoleV1::NumericalPolicyMathBound(binding),
                provenance,
            )
        }
        NumericalPolicyMathOperationV1::F32 { .. } => Type::F32,
    })
}

#[cfg(test)]
mod numerical_policy_math_lowering_tests {
    use super::*;
    include!("numerical_policy_math_01_tests.rs");
    include!("numerical_policy_math_01/transport_tests.rs");
}
