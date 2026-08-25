//! Compiler-private bijection between independently extracted write effects.
//!
//! A successful match establishes per-effect partial correctness for the
//! modeled writes. It deliberately does not establish total output coverage.

use std::fmt;

use crate::reference_effect_v1::{
    ReferenceOutputCoordinateV1, ReferenceOutputWriteV1, ReferencePathPredicateV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompilerExtractedGpuOutputEffectV1 {
    pub(crate) output_argument: u32,
    pub(crate) block: u32,
    pub(crate) operation: u32,
    pub(crate) coordinate: ReferenceOutputCoordinateV1,
    pub(crate) guard: ReferencePathPredicateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceEffectPairV1 {
    pub(crate) reference_block: u32,
    pub(crate) reference_statement: u32,
    pub(crate) gpu_block: u32,
    pub(crate) gpu_operation: u32,
}

/// Enforces a complete one-to-one correspondence between static observable
/// output-write events in the supported source subset.
///
/// Both inputs must be independently extracted. In particular, callers may
/// not populate the reference coordinate or guard from the GPU effect. RHS
/// equivalence is established later from two independently translated typed
/// expressions; it is deliberately not part of write-site pairing.
/// `gpu.guard` is the normalized logical path predicate. It may exclude only
/// the exact memory-bounds selection predicate already discharged by the
/// mandatory ranked bounds pass. Every other GPU path condition must be
/// independently normalized and matched; an adapter that cannot distinguish
/// those predicates must reject the source.
pub(crate) fn establish_reference_effect_bijection_v1(
    reference: &[ReferenceOutputWriteV1],
    gpu: &[CompilerExtractedGpuOutputEffectV1],
) -> Result<Box<[ReferenceEffectPairV1]>, ReferenceEffectBijectionErrorV1> {
    let mut used_gpu = vec![false; gpu.len()];
    let mut pairs = Vec::with_capacity(reference.len());
    for reference_effect in reference {
        let exact = gpu
            .iter()
            .enumerate()
            .filter(|(index, gpu_effect)| {
                !used_gpu[*index] && same_site_v1(reference_effect, gpu_effect)
            })
            .collect::<Vec<_>>();
        if exact.len() > 1 {
            return Err(ReferenceEffectBijectionErrorV1::AmbiguousGpuOutput {
                output_argument: reference_effect.argument,
                reference_block: reference_effect.block,
                reference_statement: reference_effect.statement,
            });
        }
        if let Some((index, gpu_effect)) = exact.first().copied() {
            used_gpu[index] = true;
            pairs.push(ReferenceEffectPairV1 {
                reference_block: reference_effect.block,
                reference_statement: reference_effect.statement,
                gpu_block: gpu_effect.block,
                gpu_operation: gpu_effect.operation,
            });
            continue;
        }

        let candidate = gpu.iter().enumerate().find(|(index, gpu_effect)| {
            !used_gpu[*index] && gpu_effect.output_argument == reference_effect.argument
        });
        let Some((_, candidate)) = candidate else {
            return Err(ReferenceEffectBijectionErrorV1::MissingGpuOutput {
                output_argument: reference_effect.argument,
                reference_block: reference_effect.block,
                reference_statement: reference_effect.statement,
            });
        };
        if candidate.coordinate != reference_effect.coordinate {
            return Err(ReferenceEffectBijectionErrorV1::CoordinateMismatch {
                output_argument: reference_effect.argument,
                reference_block: reference_effect.block,
                reference_statement: reference_effect.statement,
                gpu_block: candidate.block,
                gpu_operation: candidate.operation,
            });
        }
        if candidate.guard != reference_effect.guard {
            return Err(ReferenceEffectBijectionErrorV1::GuardMismatch {
                output_argument: reference_effect.argument,
                reference_block: reference_effect.block,
                reference_statement: reference_effect.statement,
                gpu_block: candidate.block,
                gpu_operation: candidate.operation,
            });
        }
        return Err(ReferenceEffectBijectionErrorV1::GuardMismatch {
            output_argument: reference_effect.argument,
            reference_block: reference_effect.block,
            reference_statement: reference_effect.statement,
            gpu_block: candidate.block,
            gpu_operation: candidate.operation,
        });
    }
    if let Some((_, extra)) = gpu.iter().enumerate().find(|(index, _)| !used_gpu[*index]) {
        return Err(ReferenceEffectBijectionErrorV1::ExtraGpuOutput {
            output_argument: extra.output_argument,
            gpu_block: extra.block,
            gpu_operation: extra.operation,
        });
    }
    Ok(pairs.into_boxed_slice())
}

fn same_site_v1(
    reference: &ReferenceOutputWriteV1,
    gpu: &CompilerExtractedGpuOutputEffectV1,
) -> bool {
    reference.argument == gpu.output_argument
        && reference.coordinate == gpu.coordinate
        && reference.guard == gpu.guard
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceEffectBijectionErrorV1 {
    MissingGpuOutput {
        output_argument: u32,
        reference_block: u32,
        reference_statement: u32,
    },
    AmbiguousGpuOutput {
        output_argument: u32,
        reference_block: u32,
        reference_statement: u32,
    },
    ExtraGpuOutput {
        output_argument: u32,
        gpu_block: u32,
        gpu_operation: u32,
    },
    CoordinateMismatch {
        output_argument: u32,
        reference_block: u32,
        reference_statement: u32,
        gpu_block: u32,
        gpu_operation: u32,
    },
    GuardMismatch {
        output_argument: u32,
        reference_block: u32,
        reference_statement: u32,
        gpu_block: u32,
        gpu_operation: u32,
    },
}

impl fmt::Display for ReferenceEffectBijectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGpuOutput {
                output_argument,
                reference_block,
                reference_statement,
            } => write!(
                formatter,
                "reference output argument {} write at block {reference_block}, statement {reference_statement} has no GPU output effect",
                output_argument + 1,
            ),
            Self::ExtraGpuOutput {
                output_argument,
                gpu_block,
                gpu_operation,
            } => write!(
                formatter,
                "GPU output argument {} write at block {gpu_block}, operation {gpu_operation} has no reference output effect",
                output_argument + 1,
            ),
            Self::AmbiguousGpuOutput {
                output_argument,
                reference_block,
                reference_statement,
            } => write!(
                formatter,
                "reference output argument {} write at block {reference_block}, statement {reference_statement} has multiple indistinguishable GPU output effects",
                output_argument + 1,
            ),
            Self::CoordinateMismatch {
                output_argument,
                reference_block,
                reference_statement,
                gpu_block,
                gpu_operation,
            } => write!(
                formatter,
                "output argument {} coordinate mismatch between reference block {reference_block}, statement {reference_statement} and GPU block {gpu_block}, operation {gpu_operation}",
                output_argument + 1,
            ),
            Self::GuardMismatch {
                output_argument,
                reference_block,
                reference_statement,
                gpu_block,
                gpu_operation,
            } => write!(
                formatter,
                "output argument {} guard mismatch between reference block {reference_block}, statement {reference_statement} and GPU block {gpu_block}, operation {gpu_operation}",
                output_argument + 1,
            ),
        }
    }
}

impl std::error::Error for ReferenceEffectBijectionErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_effect_v1::{
        ReferenceConstantV1, ReferenceEffectExpressionV1, ReferenceGuardAtomV1,
        ReferenceGuardClauseV1, ReferenceOperandV1, ReferenceScalarTypeV1, ReferenceValueV1,
    };

    fn constant(bits: u128) -> ReferenceEffectExpressionV1 {
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
            scalar: ReferenceScalarTypeV1::U32,
            bits,
        })
    }

    fn point(axis: u32) -> ReferenceOutputCoordinateV1 {
        ReferenceOutputCoordinateV1::LogicalPoint(
            vec![ReferenceEffectExpressionV1::PointCoordinate { axis }].into_boxed_slice(),
        )
    }

    fn guarded() -> ReferencePathPredicateV1 {
        ReferencePathPredicateV1 {
            clauses: vec![ReferenceGuardClauseV1 {
                atoms: vec![ReferenceGuardAtomV1::SwitchValueSet {
                    discriminant: ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 },
                    values: vec![0].into_boxed_slice(),
                    inside_set: false,
                }]
                .into_boxed_slice(),
            }]
            .into_boxed_slice(),
        }
    }

    fn reference_effect() -> ReferenceOutputWriteV1 {
        ReferenceOutputWriteV1 {
            argument: 1,
            block: 2,
            statement: 3,
            coordinate: point(0),
            guard: guarded(),
            rhs: constant(17),
            value: ReferenceValueV1::Use(ReferenceOperandV1::Constant(
                ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::U32,
                    bits: 17,
                },
            )),
        }
    }

    fn gpu_effect() -> CompilerExtractedGpuOutputEffectV1 {
        let reference = reference_effect();
        CompilerExtractedGpuOutputEffectV1 {
            output_argument: reference.argument,
            block: 5,
            operation: 7,
            coordinate: reference.coordinate,
            guard: reference.guard,
        }
    }

    #[test]
    fn accepts_complete_independently_extracted_bijection() {
        let pairs = establish_reference_effect_bijection_v1(&[reference_effect()], &[gpu_effect()])
            .unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].reference_block, 2);
        assert_eq!(pairs[0].gpu_operation, 7);
    }

    #[test]
    fn rejects_missing_gpu_output() {
        let error =
            establish_reference_effect_bijection_v1(&[reference_effect()], &[]).unwrap_err();
        assert!(matches!(
            error,
            ReferenceEffectBijectionErrorV1::MissingGpuOutput { .. }
        ));
    }

    #[test]
    fn rejects_extra_gpu_output() {
        let error = establish_reference_effect_bijection_v1(&[], &[gpu_effect()]).unwrap_err();
        assert!(matches!(
            error,
            ReferenceEffectBijectionErrorV1::ExtraGpuOutput { .. }
        ));
    }

    #[test]
    fn rejects_coordinate_mismatch() {
        let mut gpu = gpu_effect();
        gpu.coordinate = point(1);
        let error =
            establish_reference_effect_bijection_v1(&[reference_effect()], &[gpu]).unwrap_err();
        assert!(matches!(
            error,
            ReferenceEffectBijectionErrorV1::CoordinateMismatch { .. }
        ));
    }

    #[test]
    fn rejects_guard_mismatch() {
        let mut gpu = gpu_effect();
        gpu.guard = ReferencePathPredicateV1::unconditional_v1();
        let error =
            establish_reference_effect_bijection_v1(&[reference_effect()], &[gpu]).unwrap_err();
        assert!(matches!(
            error,
            ReferenceEffectBijectionErrorV1::GuardMismatch { .. }
        ));
    }

    #[test]
    fn rejects_ambiguous_gpu_output_site() {
        let gpu = gpu_effect();
        let error =
            establish_reference_effect_bijection_v1(&[reference_effect()], &[gpu.clone(), gpu])
                .unwrap_err();
        assert!(matches!(
            error,
            ReferenceEffectBijectionErrorV1::AmbiguousGpuOutput { .. }
        ));
    }
}
