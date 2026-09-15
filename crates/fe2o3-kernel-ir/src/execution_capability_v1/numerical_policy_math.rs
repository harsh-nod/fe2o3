//! Exact policy pairing and scalar consumers. Binding carries no FP evidence.

use super::*;
use crate::{F32MathFunction, F32MathImplementation};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NumericalPolicyMathBindingV1 {
    pub math_reference: ExecutionTypeIdentityV1,
    pub math: ExecutionTypeIdentityV1,
    pub policy_reference: ExecutionTypeIdentityV1,
    pub capability: ExecutionTypeIdentityV1,
    pub bound: ExecutionTypeIdentityV1,
    pub bound_reference: ExecutionTypeIdentityV1,
    pub policy: ExecutionTypeIdentityV1,
    pub kernel_brand: ExecutionTypeIdentityV1,
    pub mode: NumericalModeV1,
}

impl NumericalPolicyMathBindingV1 {
    pub const fn type_references(self) -> [ExecutionTypeIdentityV1; 8] {
        [
            self.math_reference,
            self.math,
            self.policy_reference,
            self.capability,
            self.bound,
            self.bound_reference,
            self.policy,
            self.kernel_brand,
        ]
    }

    pub fn is_complete(self) -> bool {
        let types = self.type_references();
        self.mode == NumericalModeV1::StrictIeee
            && types.iter().all(|ty| ty.is_complete())
            && types
                .iter()
                .enumerate()
                .all(|(index, ty)| !types[..index].contains(ty))
    }

    pub(super) fn encode(self, writer: &mut ContractWriter) {
        for ty in self.type_references() {
            writer.identity(ty);
        }
        writer.u8(0);
    }

    pub(super) fn decode(reader: &mut ContractReader<'_>) -> Option<Self> {
        let binding = Self {
            math_reference: reader.identity()?,
            math: reader.identity()?,
            policy_reference: reader.identity()?,
            capability: reader.identity()?,
            bound: reader.identity()?,
            bound_reference: reader.identity()?,
            policy: reader.identity()?,
            kernel_brand: reader.identity()?,
            mode: NumericalModeV1::StrictIeee,
        };
        (reader.u8()? == 0 && binding.is_complete()).then_some(binding)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NumericalPolicyMathOperationV1 {
    /// Source-checked KernelContext::math derivation, not numerical evidence.
    MathDerive {
        context: ExecutionTypeIdentityV1,
        binding: NumericalPolicyMathBindingV1,
    },
    Bind {
        binding: NumericalPolicyMathBindingV1,
    },
    F32 {
        binding: NumericalPolicyMathBindingV1,
        bound_reference: ExecutionTypeIdentityV1,
        element: ExecutionTypeIdentityV1,
        function: F32MathFunction,
    },
}

impl NumericalPolicyMathOperationV1 {
    pub const fn binding(self) -> NumericalPolicyMathBindingV1 {
        match self {
            Self::MathDerive { binding, .. }
            | Self::Bind { binding }
            | Self::F32 { binding, .. } => binding,
        }
    }

    pub fn type_references(self) -> Vec<ExecutionTypeIdentityV1> {
        let mut types = self.binding().type_references().to_vec();
        match self {
            Self::MathDerive { context, .. } => types.push(context),
            Self::F32 { element, .. } => types.push(element),
            Self::Bind { .. } => {}
        }
        types
    }

    pub fn is_well_formed(self) -> bool {
        let types = self.type_references();
        self.binding().is_complete()
            && types.iter().all(|ty| ty.is_complete())
            && types
                .iter()
                .enumerate()
                .all(|(index, ty)| !types[..index].contains(ty))
            && !matches!(
                self,
                Self::F32 {
                    function: F32MathFunction::Abs,
                    ..
                }
            )
            && !matches!(self, Self::F32 { binding, bound_reference, .. } if bound_reference != binding.bound_reference)
    }

    pub fn signature_matches(self, signature: ExecutionCapabilitySignatureV1) -> bool {
        let (inputs, output) = match self {
            Self::MathDerive { context, binding } => (vec![context], binding.math),
            Self::Bind { binding } => (
                vec![binding.math_reference, binding.policy_reference],
                binding.bound,
            ),
            Self::F32 {
                bound_reference,
                element,
                function,
                ..
            } => {
                let mut inputs = vec![bound_reference];
                inputs.extend(std::iter::repeat_n(element, function.arity()));
                (inputs, element)
            }
        };
        signature.arguments().eq(inputs) && signature.output() == output
    }

    pub const fn operand_count(self) -> usize {
        match self {
            Self::MathDerive { .. } => 1,
            Self::Bind { .. } => 2,
            Self::F32 { function, .. } => 1 + function.arity(),
        }
    }

    pub const fn obligations(self) -> u32 {
        use ExecutionSafetyObligationsV1 as O;
        O::LIFETIME_VALIDITY | O::NUMERICAL_POLICY | O::TARGET_SUPPORT
    }

    /// Requirements only: no conversion to a policy-free arithmetic operation.
    pub fn numerical_requirements(self) -> Option<(NumericalModeV1, F32MathImplementation)> {
        let Self::F32 { function, .. } = self else {
            return None;
        };
        self.is_well_formed().then_some((
            NumericalModeV1::StrictIeee,
            function.required_implementation(),
        ))
    }

    pub fn required_capabilities(self) -> BTreeSet<TargetCapability> {
        if matches!(self, Self::F32 { .. }) {
            BTreeSet::from([TargetCapability::Execution(
                ExecutionCapabilityRequirementV1::Numerical {
                    value_type: ScalarType::F32,
                    mode: NumericalModeV1::StrictIeee,
                },
            )])
        } else {
            BTreeSet::new()
        }
    }

    pub(super) fn encode(self, writer: &mut ContractWriter) {
        writer.u8(match self {
            Self::Bind { .. } => 0,
            Self::F32 { .. } => 1,
            Self::MathDerive { .. } => 2,
        });
        self.binding().encode(writer);
        if let Self::MathDerive { context, .. } = self {
            writer.identity(context);
        }
        if let Self::F32 {
            bound_reference,
            element,
            function,
            ..
        } = self
        {
            writer.identity(bound_reference);
            writer.identity(element);
            writer.u8(match function {
                F32MathFunction::Sqrt => 0,
                F32MathFunction::FusedMultiplyAdd => 1,
                F32MathFunction::Floor => 2,
                F32MathFunction::Ceil => 3,
                F32MathFunction::Truncate => 4,
                F32MathFunction::RoundTiesEven => 5,
                F32MathFunction::Sin => 6,
                F32MathFunction::Cos => 7,
                F32MathFunction::Exp => 8,
                F32MathFunction::Exp2 => 9,
                F32MathFunction::Ln => 10,
                F32MathFunction::Log2 => 11,
                F32MathFunction::Log10 => 12,
                F32MathFunction::Abs => 255,
            });
        }
    }

    pub(super) fn decode(reader: &mut ContractReader<'_>) -> Option<Self> {
        let tag = reader.u8()?;
        (tag <= 2).then_some(())?;
        let binding = NumericalPolicyMathBindingV1::decode(reader)?;
        let operation = if tag == 0 {
            Self::Bind { binding }
        } else if tag == 2 {
            Self::MathDerive {
                context: reader.identity()?,
                binding,
            }
        } else {
            let bound_reference = reader.identity()?;
            let element = reader.identity()?;
            let function = match reader.u8()? {
                0 => F32MathFunction::Sqrt,
                1 => F32MathFunction::FusedMultiplyAdd,
                2 => F32MathFunction::Floor,
                3 => F32MathFunction::Ceil,
                4 => F32MathFunction::Truncate,
                5 => F32MathFunction::RoundTiesEven,
                6 => F32MathFunction::Sin,
                7 => F32MathFunction::Cos,
                8 => F32MathFunction::Exp,
                9 => F32MathFunction::Exp2,
                10 => F32MathFunction::Ln,
                11 => F32MathFunction::Log2,
                12 => F32MathFunction::Log10,
                _ => return None,
            };
            Self::F32 {
                binding,
                bound_reference,
                element,
                function,
            }
        };
        operation.is_well_formed().then_some(operation)
    }
}
