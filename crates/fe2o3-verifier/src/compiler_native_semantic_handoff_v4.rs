//! Independent native capsule admission through the existing source/F checker.
use super::*;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as METADATA,
    InertSemanticCompilerModuleHandoffV4 as Handoff,
};
use fe2o3_compiler_lineage::NativeRefinedForwardingCarrierIdentityV1;

#[path = "compiler_native_semantic_handoff_joins_v4.rs"]
mod joins;

pub(super) struct Join<'a> {
    pub handoff: &'a Handoff,
    pub carrier: NativeRefinedForwardingCarrierIdentityV1,
}

/// Additional retained source/F and wrapper storage, returned unreserved.
/// The original transport backing and decoded metadata remain separately paid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveredCompilerNativeSemanticHandoffStorageV4(usize);
impl RecoveredCompilerNativeSemanticHandoffStorageV4 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owns the unchanged V4 transport and the independently recovered signed source
/// and actual F. Admission checks every native capsule association before moving
/// F out of the decoded history; no second final graph is constructed.
///
/// Public signature keys and matching content do not authenticate protected
/// compiler execution, currentness, live rustc ABI, or machine refinement. The
/// inventory/preflight receipts remain inert inputs to later custody checking.
/// This owner grants no publication, load or launch authority.
///
/// ```compile_fail
/// use fe2o3_verifier::RecoveredCompilerNativeSemanticHandoffV4 as Owner;
/// fn duplicate(value: Owner) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::RecoveredCompilerNativeSemanticHandoffV4 as Owner;
/// fn manufacture() -> Owner { Owner::default() }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::RecoveredCompilerNativeSemanticHandoffV4 as Owner;
/// fn mutate(value: Owner) { value.recovered().output().module().kernels.clear(); }
/// ```
pub struct RecoveredCompilerNativeSemanticHandoffV4 {
    handoff: Handoff,
    recovered: RecoveredCompilerRefinedForwardingOutputV1,
    storage: RecoveredCompilerNativeSemanticHandoffStorageV4,
}
impl AsRef<Handoff> for RecoveredCompilerNativeSemanticHandoffV4 {
    fn as_ref(&self) -> &Handoff {
        &self.handoff
    }
}

impl RecoveredCompilerNativeSemanticHandoffV4 {
    pub const fn handoff(&self) -> &Handoff {
        &self.handoff
    }
    pub const fn recovered(&self) -> &RecoveredCompilerRefinedForwardingOutputV1 {
        &self.recovered
    }
    pub const fn storage(&self) -> RecoveredCompilerNativeSemanticHandoffStorageV4 {
        self.storage
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn authenticates_rustc_abi(&self) -> bool {
        false
    }
}

/// Runs the native semantic checker while retaining the transaction token's lock
/// and exact decoded backing. Refusal leaves the ready record unconsumed.
/// Reserve the returned additional storage before retaining or using the token.
/// This supplies content/custody agreement, not protected compiler execution,
/// machine refinement, publication or launch authority.
pub fn recover_compiler_native_semantic_handoff_token_v4(
    token: fe2o3_artifact_transaction::CompilerModuleHandoffConsumptionTokenV4,
    budget: &mut Budget<'_>,
) -> Result<
    (
        fe2o3_artifact_transaction::CompilerModuleHandoffConsumptionTokenV4<
            RecoveredCompilerNativeSemanticHandoffV4,
        >,
        fe2o3_artifact_transaction::CompilerModuleHandoffStorageV4,
    ),
    fe2o3_artifact_transaction::CompilerModuleHandoffAdmissionErrorV4<
        CompilerRefinedForwardingOutputErrorV1,
    >,
> {
    token.try_map_handoff(budget, |handoff, budget| {
        recover_compiler_native_semantic_handoff_v4(handoff, budget)
            .map(|(owner, storage)| (owner, storage.retained_storage()))
    })
}

/// Consumes a decoded, prepaid V4 transport and independently checks its content
/// through the existing signed-source and final-F replay, with no legacy retry.
/// The caller must retain a reservation for `backing_capacity()` plus
/// `INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4`, including
/// enclosing/spare capacity, and must have prepaid the earlier content decode.
/// Reserve the returned additional storage before retaining or using the owner.
/// Success, refusal and unwind restore inherited storage, never work/failure history.
pub fn recover_compiler_native_semantic_handoff_v4(
    handoff: Handoff,
    budget: &mut Budget<'_>,
) -> R<(
    RecoveredCompilerNativeSemanticHandoffV4,
    RecoveredCompilerNativeSemanticHandoffStorageV4,
)> {
    scoped(budget, |budget| {
        budget.charge_work(8)?;
        if budget.storage_limit() > MAX_STORAGE {
            return Err(E::Mismatch("bounded storage cap"));
        }
        let floor = handoff
            .backing_capacity()
            .checked_add(METADATA)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        let header = size_of::<RecoveredCompilerNativeSemanticHandoffV4>();
        budget.reserve_storage(
            header
                .checked_add(size_of::<Join<'_>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let (recovered, storage) =
            recovery::recover_carrier(handoff.capsule().carrier_bytes(), Some(&handoff), budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        let storage = RecoveredCompilerNativeSemanticHandoffStorageV4(
            header
                .checked_add(storage.retained_storage())
                .ok_or(Resource::Arithmetic)?,
        );
        Ok((
            RecoveredCompilerNativeSemanticHandoffV4 {
                handoff,
                recovered,
                storage,
            },
            storage,
        ))
    })
}
