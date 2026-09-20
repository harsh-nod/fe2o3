//! Backend-private logical signature data, not source or provider authority.
//!
//! This projection preserves exactly the type distinctions consumed by the
//! reference ABI relation. It is not a complete Rust signature: kernel return
//! types and output-carrier index-space arguments remain in source custody.
//! There is no encoding or contribution to the existing effect-IR digest.

use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_CALL_ARGUMENTS_V1, SemanticExternAbiV1, SemanticFunctionSafetyV1, SemanticMutabilityV1,
};

use super::{MAX_REFERENCE_POINT_AXES_V1, ReferenceArgumentRelationV1, ReferenceScalarTypeV1};

pub(crate) const MAX_REFERENCE_SIGNATURE_INPUTS_V1: usize = HARD_MAX_CALL_ARGUMENTS_V1 as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceRegionV1 {
    Erased,
    Static,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceCarrierV1 {
    DisjointSlice,
    WriteOnlyDisjointSlice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferencePointeeV1 {
    Scalar(ReferenceScalarTypeV1),
    Slice(ReferenceScalarTypeV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceSignatureInputV1 {
    Scalar(ReferenceScalarTypeV1),
    Reference {
        region: ReferenceRegionV1,
        mutability: SemanticMutabilityV1,
        pointee: ReferencePointeeV1,
    },
    /// A descriptive tag; only the rustc extractor authenticates its provider.
    NominalOutput {
        carrier: ReferenceCarrierV1,
        element: ReferenceScalarTypeV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceReturnShapeV1 {
    Unit,
    NonUnit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceSignatureSideV1 {
    Kernel,
    Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceSignatureErrorV1 {
    NonUnitReturn,
    UnsafeReference,
    InvalidReferenceAbi,
    InputLimit {
        side: ReferenceSignatureSideV1,
        actual: usize,
    },
    TooFewReferenceArguments {
        kernel: usize,
        reference: usize,
    },
    TooManyPointAxes {
        actual: usize,
    },
    InvalidPointCoordinate {
        reference_argument: usize,
    },
    ArgumentMismatch {
        kernel_argument: usize,
        reference_argument: usize,
    },
    UnsupportedKernelArgument {
        argument: usize,
    },
}

impl fmt::Display for ReferenceSignatureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NonUnitReturn => formatter.write_str(
                "safe Rust reference must return unit in V1; use explicit mutable outputs",
            ),
            Self::UnsafeReference => formatter.write_str("safe Rust reference is declared unsafe"),
            Self::InvalidReferenceAbi => {
                formatter.write_str("safe Rust reference must use the non-variadic Rust ABI")
            }
            Self::InputLimit { side, actual } => write!(
                formatter,
                "{side:?} signature has {actual} inputs; maximum is {MAX_REFERENCE_SIGNATURE_INPUTS_V1}",
            ),
            Self::TooFewReferenceArguments { kernel, reference } => write!(
                formatter,
                "safe Rust reference logical ABI has {reference} arguments but kernel has {kernel}; a point reference may only add leading usize coordinate arguments",
            ),
            Self::TooManyPointAxes { actual } => write!(
                formatter,
                "safe Rust point reference has {actual} coordinate axes; maximum is {MAX_REFERENCE_POINT_AXES_V1}",
            ),
            Self::InvalidPointCoordinate { reference_argument } => write!(
                formatter,
                "safe Rust point-reference coordinate argument {} must be usize",
                reference_argument.saturating_add(1),
            ),
            Self::ArgumentMismatch {
                kernel_argument,
                reference_argument,
            } => write!(
                formatter,
                "safe Rust reference logical ABI mismatch at argument {} (raw reference argument {})",
                kernel_argument.saturating_add(1),
                reference_argument.saturating_add(1),
            ),
            Self::UnsupportedKernelArgument { argument } => write!(
                formatter,
                "kernel argument {} type has no reference ABI relation",
                argument.saturating_add(1),
            ),
        }
    }
}

impl std::error::Error for ReferenceSignatureErrorV1 {}

/// Two owned input arrays; cloning this value clones both arrays. The extractor
/// checks capacities before converting its vectors to boxes. Callers account
/// for this inline value and `(kernel_count + reference_count) * size_of::<Input>()`
/// retained bytes separately from the existing effect IR and rustc query work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceLogicalSignaturePreimageV1 {
    kernel_inputs: Box<[ReferenceSignatureInputV1]>,
    reference_inputs: Box<[ReferenceSignatureInputV1]>,
    reference_return: ReferenceReturnShapeV1,
    reference_abi: SemanticExternAbiV1,
    reference_safety: SemanticFunctionSafetyV1,
    reference_c_variadic: bool,
}

impl ReferenceLogicalSignaturePreimageV1 {
    /// Takes already owned arrays without allocating or authenticating them.
    pub(crate) fn new(
        kernel_inputs: Box<[ReferenceSignatureInputV1]>,
        reference_inputs: Box<[ReferenceSignatureInputV1]>,
        reference_return: ReferenceReturnShapeV1,
        reference_abi: SemanticExternAbiV1,
        reference_safety: SemanticFunctionSafetyV1,
        reference_c_variadic: bool,
    ) -> Result<Self, ReferenceSignatureErrorV1> {
        Self::check_header_v1(
            kernel_inputs.len(),
            reference_inputs.len(),
            reference_return,
            reference_abi,
            reference_safety,
            reference_c_variadic,
        )?;
        Ok(Self {
            kernel_inputs,
            reference_inputs,
            reference_return,
            reference_abi,
            reference_safety,
            reference_c_variadic,
        })
    }

    /// Constant work; usable before the extractor reserves either input array.
    pub(crate) fn check_header_v1(
        kernel: usize,
        reference: usize,
        result: ReferenceReturnShapeV1,
        abi: SemanticExternAbiV1,
        safety: SemanticFunctionSafetyV1,
        c_variadic: bool,
    ) -> Result<usize, ReferenceSignatureErrorV1> {
        if safety != SemanticFunctionSafetyV1::Safe {
            return Err(ReferenceSignatureErrorV1::UnsafeReference);
        }
        if abi != SemanticExternAbiV1::Rust || c_variadic {
            return Err(ReferenceSignatureErrorV1::InvalidReferenceAbi);
        }
        if result != ReferenceReturnShapeV1::Unit {
            return Err(ReferenceSignatureErrorV1::NonUnitReturn);
        }
        for (side, actual) in [
            (ReferenceSignatureSideV1::Kernel, kernel),
            (ReferenceSignatureSideV1::Reference, reference),
        ] {
            if actual > MAX_REFERENCE_SIGNATURE_INPUTS_V1 {
                return Err(ReferenceSignatureErrorV1::InputLimit { side, actual });
            }
        }
        let axes = reference
            .checked_sub(kernel)
            .ok_or(ReferenceSignatureErrorV1::TooFewReferenceArguments { kernel, reference })?;
        if axes > MAX_REFERENCE_POINT_AXES_V1 {
            return Err(ReferenceSignatureErrorV1::TooManyPointAxes { actual: axes });
        }
        Ok(axes)
    }

    /// Original kernel source order, not adjusted FnAbi order.
    pub(crate) fn kernel_inputs(&self) -> &[ReferenceSignatureInputV1] {
        &self.kernel_inputs
    }

    /// Raw reference source order, including any leading coordinate inputs.
    pub(crate) fn reference_inputs(&self) -> &[ReferenceSignatureInputV1] {
        &self.reference_inputs
    }

    /// Checks each raw reference slot once; borrows the preimage with no allocation.
    pub(crate) fn derive_relations_v1(
        &self,
    ) -> Result<ReferenceLogicalAbiRelationV1<'_>, ReferenceSignatureErrorV1> {
        let axes = Self::check_header_v1(
            self.kernel_inputs().len(),
            self.reference_inputs().len(),
            self.reference_return,
            self.reference_abi,
            self.reference_safety,
            self.reference_c_variadic,
        )?;
        for (reference_argument, input) in self.reference_inputs()[..axes].iter().enumerate() {
            if *input != ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::Usize) {
                return Err(ReferenceSignatureErrorV1::InvalidPointCoordinate {
                    reference_argument,
                });
            }
        }
        for (argument, (kernel, reference)) in self
            .kernel_inputs()
            .iter()
            .zip(&self.reference_inputs()[axes..])
            .enumerate()
        {
            kernel_relation_v1(argument, argument + axes, *kernel, *reference)?;
        }
        Ok(ReferenceLogicalAbiRelationV1 {
            preimage: self,
            axes,
        })
    }
}

/// A checked logical relation only; neither source custody nor effect coverage.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReferenceLogicalAbiRelationV1<'a> {
    preimage: &'a ReferenceLogicalSignaturePreimageV1,
    axes: usize,
}

impl ReferenceLogicalAbiRelationV1<'_> {
    pub(crate) fn len(&self) -> usize {
        self.preimage.reference_inputs().len()
    }

    /// Both arguments are original source ordinals; neither is an ABI slot.
    pub(crate) fn reference_argument_for_kernel_argument_v1(
        &self,
        kernel_source_argument: u32,
    ) -> Option<u32> {
        let source = usize::try_from(kernel_source_argument).ok()?;
        self.preimage.kernel_inputs().get(source)?;
        u32::try_from(self.axes)
            .ok()?
            .checked_add(kernel_source_argument)
    }

    /// Constant-work access in raw reference order, without a second relation map.
    pub(crate) fn relation_at_raw_argument_v1(
        &self,
        reference_argument: u32,
    ) -> Option<ReferenceArgumentRelationV1> {
        let raw = usize::try_from(reference_argument).ok()?;
        let reference = *self.preimage.reference_inputs().get(raw)?;
        if raw < self.axes {
            return Some(ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument,
                axis: reference_argument,
            });
        }
        let argument = raw.checked_sub(self.axes)?;
        self.reference_argument_for_kernel_argument_v1(u32::try_from(argument).ok()?)?;
        let kernel = *self.preimage.kernel_inputs().get(argument)?;
        kernel_relation_v1(argument, raw, kernel, reference).ok()
    }
}

fn kernel_relation_v1(
    argument: usize,
    reference_argument: usize,
    kernel: ReferenceSignatureInputV1,
    reference: ReferenceSignatureInputV1,
) -> Result<ReferenceArgumentRelationV1, ReferenceSignatureErrorV1> {
    let mismatch = ReferenceSignatureErrorV1::ArgumentMismatch {
        kernel_argument: argument,
        reference_argument,
    };
    let source = u32::try_from(argument).map_err(|_| mismatch)?;
    match kernel {
        ReferenceSignatureInputV1::Scalar(scalar) if kernel == reference => {
            Ok(ReferenceArgumentRelationV1::ScalarInput {
                argument: source,
                scalar,
            })
        }
        ReferenceSignatureInputV1::Scalar(_) => Err(mismatch),
        ReferenceSignatureInputV1::Reference {
            mutability: SemanticMutabilityV1::Immutable,
            pointee: ReferencePointeeV1::Slice(element),
            ..
        } if kernel == reference => Ok(ReferenceArgumentRelationV1::SharedSliceInput {
            argument: source,
            element,
        }),
        ReferenceSignatureInputV1::Reference {
            mutability: SemanticMutabilityV1::Immutable,
            pointee: ReferencePointeeV1::Slice(_),
            ..
        } => Err(mismatch),
        ReferenceSignatureInputV1::NominalOutput { element, .. } => match reference {
            ReferenceSignatureInputV1::Reference {
                mutability: SemanticMutabilityV1::Mutable,
                pointee: ReferencePointeeV1::Slice(actual),
                ..
            } if element == actual => Ok(ReferenceArgumentRelationV1::DisjointOutputSlice {
                argument: source,
                element,
            }),
            ReferenceSignatureInputV1::Reference {
                mutability: SemanticMutabilityV1::Mutable,
                pointee: ReferencePointeeV1::Scalar(actual),
                ..
            } if element == actual => Ok(ReferenceArgumentRelationV1::DisjointOutputCoordinate {
                argument: source,
                element,
            }),
            _ => Err(mismatch),
        },
        ReferenceSignatureInputV1::Reference { .. } => {
            Err(ReferenceSignatureErrorV1::UnsupportedKernelArgument { argument })
        }
    }
}

#[cfg(test)]
#[path = "reference_signature_preimage_v1_tests.rs"]
mod tests;
