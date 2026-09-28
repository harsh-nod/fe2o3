//! C1: exact caller-origin authentication for the closed nominal helper.
//! Observational only: no transfer, final-pass approval, layout receipt, ranked
//! admission, normal continuation, destination accumulator or owned proof.
use super::bf16_nominal_call_projection_v1::CheckedNominalCallProjectionV1;
use super::tensor_capability_read_v1::CapabilityStateReadV1;
use super::{
    AuthenticatedTensorInstructionV1, SemanticBlockIdV1, SemanticCallableDeclV1,
    SemanticCompilerIntrinsicOperationV1, SemanticDirectCallV1, SemanticEdgeRoleV1,
    SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticLocalIdV1, SemanticMfmaStorageLayoutV1,
    SemanticScalarTypeV1, SemanticSourceProvenanceV1, SemanticTerminatorKindV1, SemanticTypeDeclV1,
    SemanticTypeIdV1, SemanticTypeShapeV1, SemanticUnwindActionV1,
    authenticate_tensor_instruction_read_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{Bf16NominalCallQueryErrorV1 as Error, ProductionPreRankedKirOwnerV1};

type Result<T> = std::result::Result<T, Error>;

// All operations below are fixed-arity indexed reads, no allocation or scans.
// These precharges cover actual local/contract checks and the four state reads.
// C2 must separately charge dense-state construction/scans/merges and all owned
// state/output headers; charging C1 does NOT meter legacy HashMap propagation.
const CALLER_SITE_WORK_V1: usize = 32;
const CALLER_AUTHENTICATION_WORK_V1: usize = 64;
const CALLER_TENSOR_WORK_V1: usize = 64;

fn require(condition: bool, why: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Unavailable(why))
    }
}

/// Ordinary scalar-array destination, never a nominal accumulator capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NominalArrayDestinationV1 {
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    continuation: SemanticBlockIdV1,
}
impl NominalArrayDestinationV1 {
    pub(super) const fn local(self) -> SemanticLocalIdV1 {
        self.local
    }
    pub(super) const fn ty(self) -> SemanticTypeIdV1 {
        self.ty
    }
    pub(super) const fn continuation(self) -> SemanticBlockIdV1 {
        self.continuation
    }
}

/// Checked immutable source association only; no capability or pass approval.
#[derive(Clone, Copy)]
pub(super) struct NominalCallerSiteV1<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    function: &'a SemanticFunctionDeclV1,
    call: &'a SemanticDirectCallV1,
    source: SemanticSourceProvenanceV1,
    destination: NominalArrayDestinationV1,
}
impl<'a> NominalCallerSiteV1<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_source(
        owner: &'a ProductionPreRankedKirOwnerV1,
        inventory: &'a CanonicalKirInventoryV1<'a>,
        root: SemanticFunctionIdV1,
        caller: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        call: &'a SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(CALLER_SITE_WORK_V1)?;
        require(
            inventory.belongs_to(owner.executable()),
            "nominal caller site inventory belongs to another owner",
        )?;
        require(
            root == caller,
            "nominal caller site is outside the selected root",
        )?;
        let semantic = owner.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(caller.index() as usize)
            .ok_or(Error::Unavailable("nominal caller source function absent"))?;
        let actual = function
            .blocks()
            .get(block.index() as usize)
            .ok_or(Error::Unavailable("nominal caller source block absent"))?;
        let SemanticTerminatorKindV1::Call(actual_call) = actual.terminator().kind() else {
            return Err(Error::Unavailable(
                "nominal caller source terminator is not a call",
            ));
        };
        require(
            std::ptr::eq(actual_call, call),
            "nominal caller site is not the actual borrowed call",
        )?;
        require(
            actual.terminator().source() == source,
            "nominal caller source provenance differs",
        )?;
        require(
            matches!(
                semantic.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { .. })
            ),
            "nominal caller source callee is not Defined",
        )?;
        let destination = array_destination(function, semantic.types(), call)?;
        Ok(Self {
            owner,
            inventory,
            root,
            caller,
            block,
            function,
            call,
            source,
            destination,
        })
    }
    pub(super) const fn owner(&self) -> &'a ProductionPreRankedKirOwnerV1 {
        self.owner
    }
    pub(super) const fn inventory(&self) -> &'a CanonicalKirInventoryV1<'a> {
        self.inventory
    }
    pub(super) const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    pub(super) const fn caller(&self) -> SemanticFunctionIdV1 {
        self.caller
    }
    pub(super) const fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub(super) const fn function(&self) -> &'a SemanticFunctionDeclV1 {
        self.function
    }
    pub(super) const fn call(&self) -> &'a SemanticDirectCallV1 {
        self.call
    }
    pub(super) const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }
    pub(super) const fn destination(&self) -> NominalArrayDestinationV1 {
        self.destination
    }
}

fn array_destination(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    call: &SemanticDirectCallV1,
) -> Result<NominalArrayDestinationV1> {
    let destination = call.destination().ok_or(Error::Unavailable(
        "nominal source array destination absent",
    ))?;
    let place = destination.place();
    require(
        place.projections().is_empty(),
        "nominal source array destination is projected",
    )?;
    require(
        matches!(call.unwind(), SemanticUnwindActionV1::Unreachable),
        "nominal source call unwind is outside the closed profile",
    )?;
    require(
        destination.edge().role() == SemanticEdgeRoleV1::CallReturn
            && function
                .blocks()
                .get(destination.edge().target().index() as usize)
                .is_some(),
        "nominal source array continuation absent or not CallReturn",
    )?;
    require(
        function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == place.ty()),
        "nominal source array local type differs",
    )?;
    require_array_type(types, place.ty())?;
    Ok(NominalArrayDestinationV1 {
        local: place.local(),
        ty: place.ty(),
        continuation: destination.edge().target(),
    })
}

fn require_array_type(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Result<()> {
    let Some(SemanticTypeShapeV1::Array { element, length: 4 }) = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(Error::Unavailable(
            "nominal source result is not a four-element array",
        ));
    };
    require(
        matches!(
            types
                .get(element.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
                bits: 32
            }))
        ),
        "nominal source array element is not f32",
    )
}

/// A per-visit authentication only, deliberately NOT Clone/Copy. The candidate
/// and same-owner/inventory source association remain live inside the HRTB
/// visitor. No constructor is exposed. tensor.accumulator is the INPUT origin;
/// the [f32;4] destination has no accumulator capability.
pub(super) struct AuthenticatedNominalCallerV1<'a> {
    candidate: &'a CheckedNominalCallProjectionV1<'a>,
    site: NominalCallerSiteV1<'a>,
    tensor: AuthenticatedTensorInstructionV1,
}
impl<'a> AuthenticatedNominalCallerV1<'a> {
    pub(super) const fn candidate(&self) -> &'a CheckedNominalCallProjectionV1<'a> {
        self.candidate
    }
    pub(super) const fn site(&self) -> &NominalCallerSiteV1<'a> {
        &self.site
    }
    pub(super) const fn tensor(&self) -> AuthenticatedTensorInstructionV1 {
        self.tensor
    }
}

/// Invalid is solely a provisional capability-lattice outcome. Structural and
/// resource errors remain Err and must never be swallowed by propagation.
pub(super) enum NominalTransferOutcomeV1<'a> {
    Invalid(&'static str),
    Authenticated(AuthenticatedNominalCallerV1<'a>),
}

/// Reads the ORIGINAL Defined call operands in caller state. No helper-formal
/// local is queried; no state is written or operand consumed. C2 commits exactly
/// once only after the complete query succeeds and removes the array destination.
pub(super) fn authenticate_nominal_call_v1<'a>(
    candidate: &'a CheckedNominalCallProjectionV1<'a>,
    caller_site: &NominalCallerSiteV1<'a>,
    state: &(impl CapabilityStateReadV1 + ?Sized),
    budget: &mut Budget<'_>,
) -> Result<NominalTransferOutcomeV1<'a>> {
    budget.charge_work(CALLER_AUTHENTICATION_WORK_V1)?;
    let checked = candidate.call();
    let emission = checked.emission();
    require(
        std::ptr::eq(emission.owner(), caller_site.owner)
            && checked.belongs_to(caller_site.inventory),
        "nominal caller authentication owner or inventory differs",
    )?;
    require(
        emission.root() == caller_site.root
            && caller_site.caller == caller_site.root
            && emission.source_call_block() == caller_site.block,
        "nominal caller authentication qualified source site differs",
    )?;
    require(
        std::ptr::eq(checked.source_call(), caller_site.call),
        "nominal caller authentication actual borrowed call differs",
    )?;
    let semantic = caller_site.owner.semantic_ssa().source_semantic();
    let actual_function = semantic
        .functions()
        .get(caller_site.caller.index() as usize)
        .ok_or(Error::Unavailable(
            "nominal authenticated caller function absent",
        ))?;
    require(
        std::ptr::eq(actual_function, caller_site.function),
        "nominal authenticated caller function identity differs",
    )?;
    let actual_block = actual_function
        .blocks()
        .get(caller_site.block.index() as usize)
        .ok_or(Error::Unavailable(
            "nominal authenticated caller block absent",
        ))?;
    require(
        actual_block.terminator().source() == caller_site.source,
        "nominal authenticated caller provenance differs",
    )?;
    let SemanticTerminatorKindV1::Call(actual_call) = actual_block.terminator().kind() else {
        return Err(Error::Unavailable(
            "nominal authenticated source terminal differs",
        ));
    };
    require(
        std::ptr::eq(actual_call, caller_site.call),
        "nominal authenticated source terminal call identity differs",
    )?;
    require(
        matches!(semantic.callables().get(actual_call.callee().index() as usize),
        Some(SemanticCallableDeclV1::Defined { function }) if *function == emission.helper()),
        "nominal authenticated Defined helper differs",
    )?;
    require(
        array_destination(actual_function, semantic.types(), actual_call)?
            == caller_site.destination,
        "nominal authenticated array destination differs",
    )?;
    let SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
        lhs_fragment,
        rhs_fragment,
        accumulator_fragment,
        lhs,
        rhs,
        accumulator,
        ..
    } = candidate.source_matrix_intrinsic()
    else {
        return Err(Error::Unavailable(
            "nominal authenticated source intrinsic is not MFMA",
        ));
    };
    require(
        actual_call.arguments().len() == 4
            && actual_call.arguments()[1].ty() == *lhs_fragment
            && actual_call.arguments()[2].ty() == *rhs_fragment
            && actual_call.arguments()[3].ty() == *accumulator_fragment,
        "nominal authenticated caller argument types differ",
    )?;
    // The sealed N1 source/FnABI relation validates context shared-borrow and
    // A/B/acc whole-value ownership. The shared core validates their real
    // dominating origins after ordinary source borrow/move transport.
    let tensor = match authenticate_nominal_tensor_origin(
        actual_call,
        state,
        *lhs,
        *rhs,
        *accumulator,
        candidate.required_tensor_contract(),
        budget,
    )? {
        Ok(tensor) => tensor,
        Err(reason) => return Ok(NominalTransferOutcomeV1::Invalid(reason)),
    };
    Ok(NominalTransferOutcomeV1::Authenticated(
        AuthenticatedNominalCallerV1 {
            candidate,
            site: *caller_site,
            tensor,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_nominal_tensor_origin(
    call: &SemanticDirectCallV1,
    state: &(impl CapabilityStateReadV1 + ?Sized),
    lhs: super::SemanticMfmaOperandContractV1,
    rhs: super::SemanticMfmaOperandContractV1,
    accumulator: super::SemanticMfmaAccumulatorContractV1,
    required: fe2o3_kernel_ir::TensorLayoutContractV1,
    budget: &mut Budget<'_>,
) -> Result<std::result::Result<AuthenticatedTensorInstructionV1, &'static str>> {
    budget.charge_work(CALLER_TENSOR_WORK_V1)?;
    Ok(
        authenticate_tensor_instruction_read_v1(call, state, lhs, rhs, accumulator).and_then(
            |tensor| {
                require_nominal_tensor_contract(tensor, required)?;
                Ok(tensor)
            },
        ),
    )
}

fn require_nominal_tensor_contract(
    tensor: AuthenticatedTensorInstructionV1,
    required: fe2o3_kernel_ir::TensorLayoutContractV1,
) -> std::result::Result<(), &'static str> {
    // ColumnMajor B has the same register-layout contract as RowMajor in the
    // generic core. This closed nominal source profile nevertheless requires
    // actual RowMajor load origins; do not silently normalize storage variants.
    if tensor.lhs.storage_layout != SemanticMfmaStorageLayoutV1::RowMajor
        || tensor.rhs.storage_layout != SemanticMfmaStorageLayoutV1::RowMajor
    {
        return Err("a nominal MFMA call outside its exact RowMajor source-storage profile");
    }
    if tensor.contract != required {
        return Err("a nominal MFMA call whose authenticated tensor contract differs");
    }
    Ok(())
}

#[cfg(test)]
#[path = "bf16_nominal_capabilities_v1_tests.rs"]
mod tests;
