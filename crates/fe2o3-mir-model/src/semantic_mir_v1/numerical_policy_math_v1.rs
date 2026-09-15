//! Retained policy-bound FP32 consumer requirements, not numerical proof.

use super::*;

impl InertSemanticMirRequestV1 {
    /// Admits policy-bound FP32 consumers without erasing their open obligations.
    pub fn admit_exact_v19(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V19, limits)
    }
}

/// The only source policy supported by this consumer schema. Unknown policies
/// must be rejected by the authenticated source importer, never mapped here.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticNumericalModeV1 {
    StrictIeee,
}

/// Required target behavior, not evidence of an available implementation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticF32MathImplementationV1 {
    /// LLVM constrained operations, round-to-nearest-even, ignored exceptions.
    ConstrainedLlvm,
    /// Strict OCML FP32 ABI with fast, finite-only and unsafe modes disabled.
    OcmlAbiV1,
    /// IEEE FP32 sqrt, round-to-nearest-even, ignored exceptions.
    IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
}

impl SemanticF32MathFunctionV1 {
    pub const fn required_implementation(self) -> SemanticF32MathImplementationV1 {
        use SemanticF32MathImplementationV1::*;
        match self {
            Self::Sqrt => IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
            Self::FusedMultiplyAdd
            | Self::Floor
            | Self::Ceil
            | Self::Truncate
            | Self::RoundTiesEven => ConstrainedLlvm,
            Self::Sin
            | Self::Cos
            | Self::Exp
            | Self::Exp2
            | Self::Ln
            | Self::Log2
            | Self::Log10 => OcmlAbiV1,
        }
    }
}

/// Ordered source type edges; neither retained reference may be erased.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticNumericalPolicyMathTypesV1 {
    pub bound_reference: SemanticTypeIdV1,
    pub bound: SemanticTypeIdV1,
    pub math_reference: SemanticTypeIdV1,
    pub math: SemanticTypeIdV1,
    pub policy_reference: SemanticTypeIdV1,
    pub capability: SemanticTypeIdV1,
    pub element: SemanticTypeIdV1,
}

impl SemanticNumericalPolicyMathTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 7]) -> Self {
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

    pub const fn all(self) -> [SemanticTypeIdV1; 7] {
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

/// An inert source contract. Admission checks structural consistency only;
/// authenticated nominal identities, SSA issuance/constructor custody and
/// strict target refinement remain mandatory downstream obligations.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticNumericalPolicyMathContractV1 {
    types: SemanticNumericalPolicyMathTypesV1,
    policy: SemanticTypeIdentityV1,
    kernel_brand: SemanticTypeIdentityV1,
    function: SemanticF32MathFunctionV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
}

impl SemanticNumericalPolicyMathContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        types: SemanticNumericalPolicyMathTypesV1,
        policy: SemanticTypeIdentityV1,
        kernel_brand: SemanticTypeIdentityV1,
        function: SemanticF32MathFunctionV1,
        mode: SemanticNumericalModeV1,
        implementation: SemanticF32MathImplementationV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        source_identity: SemanticFunctionIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let ids = types.all();
        if policy.as_bytes() == &[0; 32]
            || kernel_brand.as_bytes() == &[0; 32]
            || policy == kernel_brand
            || source_identity.as_bytes() == &[0; 32]
            || mode != SemanticNumericalModeV1::StrictIeee
            || implementation != function.required_implementation()
            || ids.iter().enumerate().any(|(index, id)| {
                u64::from(id.index()) >= HARD_MAX_TYPES_V1 || ids[..index].contains(id)
            })
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            types,
            policy,
            kernel_brand,
            function,
            provenance,
            source_identity,
        })
    }

    pub const fn types(self) -> SemanticNumericalPolicyMathTypesV1 {
        self.types
    }
    pub const fn policy(self) -> SemanticTypeIdentityV1 {
        self.policy
    }
    pub const fn kernel_brand(self) -> SemanticTypeIdentityV1 {
        self.kernel_brand
    }
    pub const fn function(self) -> SemanticF32MathFunctionV1 {
        self.function
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }

    pub const fn numerical_requirements(
        self,
    ) -> (SemanticNumericalModeV1, SemanticF32MathImplementationV1) {
        (
            SemanticNumericalModeV1::StrictIeee,
            self.function.required_implementation(),
        )
    }

    pub const fn obligations(self) -> SemanticExecutionSafetyObligationsV1 {
        SemanticExecutionSafetyObligationsV1(
            SemanticExecutionSafetyObligationsV1::TARGET_SUPPORT
                | SemanticExecutionSafetyObligationsV1::NUMERICAL_POLICY,
        )
    }

    pub fn signature(self) -> SemanticExecutionCapabilitySignatureV1 {
        let mut inputs = [self.types.element; 4];
        inputs[0] = self.types.bound_reference;
        SemanticExecutionCapabilitySignatureV1::new(
            &inputs[..self.function.arity() + 1],
            self.types.element,
        )
        .expect("FP32 consumer arity is bounded by four")
    }
}

pub(super) fn abi_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    contract: SemanticNumericalPolicyMathContractV1,
) -> bool {
    let ids = contract.types;
    let all = ids.all();
    // Request validation already establishes global identity uniqueness. These
    // seven lookups and pairwise checks have a fixed bound independent of types.
    for (index, id) in all.iter().enumerate() {
        let Some(ty) = request.types.get(id.index() as usize) else {
            return false;
        };
        if ty.identity().as_bytes() == &[0; 32]
            || ty.identity() == contract.policy
            || ty.identity() == contract.kernel_brand
            || ty.layout().is_uninhabited()
            || all[..index]
                .iter()
                .any(|other| request.types[other.index() as usize].identity() == ty.identity())
        {
            return false;
        }
    }
    for (reference, pointee) in [
        (ids.bound_reference, ids.bound),
        (ids.math_reference, ids.math),
        (ids.policy_reference, ids.capability),
    ] {
        if !shared_reference_to(request, reference, pointee) {
            return false;
        }
    }
    let SemanticTypeShapeV1::Aggregate(bound) = request.types[ids.bound.index() as usize].shape()
    else {
        return false;
    };
    let [math, policy, phantom] = bound.fields() else {
        return false;
    };
    if *math != ids.math_reference
        || *policy != ids.policy_reference
        || request
            .types
            .get(phantom.index() as usize)
            .is_none_or(|ty| ty.layout().size_bytes() != Some(0) || ty.layout().is_uninhabited())
    {
        return false;
    }
    for id in [ids.math, ids.capability] {
        let ty = &request.types[id.index() as usize];
        if !matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
            || ty.layout().size_bytes() != Some(0)
            || ty.layout().alignment_bytes() != 1
        {
            return false;
        }
    }
    let element = &request.types[ids.element.index() as usize];
    if !matches!(
        element.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
    ) || element.layout().size_bytes() != Some(4)
    {
        return false;
    }
    let signature = contract.signature();
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() as usize == contract.function.arity() + 1
        && signature.matches(abi.source_input_types(), abi.source_output_type())
        && abi.hidden_arguments().is_empty()
        && abi.source_argument_ownership().len() == contract.function.arity() + 1
        && abi
            .source_argument_ownership()
            .iter()
            .enumerate()
            .all(|(index, ownership)| {
                *ownership
                    == if index == 0 {
                        SemanticSourceArgumentOwnershipV1::SharedBorrow
                    } else {
                        SemanticSourceArgumentOwnershipV1::ByValue
                    }
            })
        && abi.arguments().len() == contract.function.arity() + 1
        && abi.arguments().iter().enumerate().all(|(index, argument)| {
            argument.role() == SemanticAbiArgumentRoleV1::Source
                && argument.ty()
                    == if index == 0 {
                        ids.bound_reference
                    } else {
                        ids.element
                    }
                && direct_value(argument.value())
        })
        && abi.return_value().ty() == ids.element
        && direct_value(abi.return_value())
        && kernel_capability_provenance_matches(request, contract.provenance)
}

fn direct_value(value: &SemanticAbiValueV1) -> bool {
    matches!(value.mode(), SemanticAbiPassModeV1::Direct(_))
        && value.adjusted().is_none()
        && value.pointee_override().is_none()
}

impl IntrinsicCapabilityClaimsV1 {
    pub(super) fn record_policy_math(
        &mut self,
        contract: SemanticNumericalPolicyMathContractV1,
    ) -> bool {
        self.record_math_origin(contract.types.math, contract.provenance, contract.kernel_brand)
            && self.record_policy_math_binding(
                contract.types.capability, contract.policy, contract.provenance, contract.kernel_brand,
            )
    }

    pub(super) fn record_policy_math_binding(
        &mut self,
        capability: SemanticTypeIdV1,
        policy: SemanticTypeIdentityV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        kernel_brand: SemanticTypeIdentityV1,
    ) -> bool {
        let binding = (policy, provenance);
        let policy_matches = match self.numerical_policies.entry(capability) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(binding);
                true
            }
            std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == binding,
        };
        policy_matches
            && match self.numerical_math_brands.entry(capability) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(kernel_brand);
                    true
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    *entry.get() == kernel_brand
                }
            }
    }
}

pub(super) fn encode(
    writer: &mut CanonicalWriterV1,
    contract: SemanticNumericalPolicyMathContractV1,
) -> Result<(), SemanticMirErrorV1> {
    for id in contract.types.all() {
        writer.u32(id.index())?;
    }
    writer.identity(*contract.policy.as_bytes())?;
    writer.identity(*contract.kernel_brand.as_bytes())?;
    writer.u8(match contract.function {
        SemanticF32MathFunctionV1::Sqrt => 0,
        SemanticF32MathFunctionV1::FusedMultiplyAdd => 1,
        SemanticF32MathFunctionV1::Floor => 2,
        SemanticF32MathFunctionV1::Ceil => 3,
        SemanticF32MathFunctionV1::Truncate => 4,
        SemanticF32MathFunctionV1::RoundTiesEven => 5,
        SemanticF32MathFunctionV1::Sin => 6,
        SemanticF32MathFunctionV1::Cos => 7,
        SemanticF32MathFunctionV1::Exp => 8,
        SemanticF32MathFunctionV1::Exp2 => 9,
        SemanticF32MathFunctionV1::Ln => 10,
        SemanticF32MathFunctionV1::Log2 => 11,
        SemanticF32MathFunctionV1::Log10 => 12,
    })?;
    writer.u8(0)?; // StrictIeee, explicitly encoded; no default decode path.
    writer.u8(match contract.function.required_implementation() {
        SemanticF32MathImplementationV1::ConstrainedLlvm => 0,
        SemanticF32MathImplementationV1::OcmlAbiV1 => 1,
        SemanticF32MathImplementationV1::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1 => 2,
    })?;
    encode_kernel_capability_provenance(writer, contract.provenance)?;
    writer.u32(contract.obligations().bits())?;
    writer.identity(*contract.source_identity.as_bytes())
}
