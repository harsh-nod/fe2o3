//! Consuming native original-N V4 association, not F2NOUT1 or publication.
//! The existing signed native owner is mandatory; inert inputs cannot create it.
use super::super::original_receipts::OriginalNativeInputReceiptErrorV1;
use super::{
    Budget, Graph, NativeFinalOutputReceiptsPolicy6V1, NativeOutputHandoffErrorV1,
    PreparedNativeFinalOutputPolicy6V1, Resource, Stage6,
};
use fe2o3_compiler_lineage::{
    InertFormalMemoryReceiptV3, InertProofBindingReceiptV3, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "production_native_input_association_joins_policy6_v1.rs"]
pub(in crate::production_pipeline) mod joins;
#[cfg(test)]
pub(crate) use joins::source_identity_tests::{
    exercise_original_kernel_identity_substitution_v1, exercise_unsigned_original_identities_v1,
};
#[cfg(test)]
#[path = "production_native_input_association_policy6_v1_tests.rs"]
mod tests;

#[derive(Debug)]
pub(crate) enum NativeInputAssociationErrorPolicy6V1 {
    Resource(Resource),
    Native(Box<NativeOutputHandoffErrorV1>),
    Original(Box<OriginalNativeInputReceiptErrorV1>),
    Mismatch(&'static str),
    Panicked,
}
type E = NativeInputAssociationErrorPolicy6V1;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<NativeOutputHandoffErrorV1> for E {
    fn from(error: NativeOutputHandoffErrorV1) -> Self {
        Self::Native(Box::new(error))
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native original input association: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error.as_ref()),
            Self::Original(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OriginalInputAssociationStoragePolicy6V1(usize);
impl OriginalInputAssociationStoragePolicy6V1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Two owned inert receipts only. The original N envelope remains in the
/// existing native source lineage, with no persistent duplicate graph or wire.
pub(crate) struct NativeOriginalInputAssociationReceiptsPolicy6V1 {
    formal_memory: InertFormalMemoryReceiptV3,
    proof_binding: InertProofBindingReceiptV3,
    retained: usize,
}
impl NativeOriginalInputAssociationReceiptsPolicy6V1 {
    /// Copies bounded opaque inputs; contents are admitted only with the actual
    /// signed native owner. Borrowed caller backing remains in its external
    /// accounting domain. Reserve the returned full receipt before later work,
    /// and drop this owner before releasing that reservation.
    pub(crate) fn try_from_preimages_v1(
        original_formal: &[u8],
        proof_binding_v4: &[u8],
        budget: &mut Budget<'_>,
    ) -> R<(Self, OriginalInputAssociationStoragePolicy6V1)> {
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            if [original_formal, proof_binding_v4].iter().any(|bytes| {
                bytes.is_empty() || bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
            }) {
                return Err(E::Mismatch("input association preimage extent"));
            }
            let bytes = original_formal
                .len()
                .checked_add(proof_binding_v4.len())
                .ok_or(Resource::Arithmetic)?;
            let retained = size_of::<Self>()
                .checked_add(bytes)
                .ok_or(Resource::Arithmetic)?;
            // Prepay final payloads and transient Vec-to-Box overlap before copy.
            budget.reserve_storage(retained.checked_add(bytes).ok_or(Resource::Arithmetic)?)?;
            budget.charge_work(bytes.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
            let formal = copy_prepaid(original_formal, budget)?;
            let proof = copy_prepaid(proof_binding_v4, budget)?;
            let owner = Self {
                formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(formal)
                    .map_err(|_| E::Mismatch("original FormalMemory receipt identity"))?,
                proof_binding: InertProofBindingReceiptV3::from_canonical_preimage(proof)
                    .map_err(|_| E::Mismatch("original ProofBinding receipt identity"))?,
                retained,
            };
            Ok((owner, OriginalInputAssociationStoragePolicy6V1(retained)))
        })
    }
    pub(crate) fn formal_memory(&self) -> &InertFormalMemoryReceiptV3 {
        &self.formal_memory
    }
    pub(crate) fn proof_binding(&self) -> &InertProofBindingReceiptV3 {
        &self.proof_binding
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
}

// The requested length and conversion overlap are prepaid by the caller.
// Any allocator excess is charged before initializing bytes or growing again.
fn copy_prepaid(bytes: &[u8], budget: &mut Budget<'_>) -> R<Vec<u8>> {
    budget.charge_work(bytes.len())?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| Resource::Allocation)?;
    let excess = copy
        .capacity()
        .checked_sub(bytes.len())
        .ok_or(Resource::Accounting)?;
    budget.reserve_storage(excess)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut deferred_panic = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            deferred_panic = Some(payload);
            Err(E::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < floor {
        drop(result);
        drop(deferred_panic);
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drop(result);
        drop(deferred_panic);
        return Err(error.into());
    }
    // A hostile caught payload's destructor must not skip floor restoration.
    drop(deferred_panic);
    result
}

fn require_incoming(final_floor: usize, receipt_floor: usize, budget: &mut Budget<'_>) -> R<usize> {
    budget.charge_work(3)?;
    let required = final_floor
        .checked_add(receipt_floor)
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < required {
        return Err(Resource::Accounting.into());
    }
    Ok(required)
}

fn finish_receipt(
    incoming: usize,
    budget: &mut Budget<'_>,
) -> R<(usize, OriginalInputAssociationStoragePolicy6V1)> {
    let delta = size_of::<PreparedNativeAssociatedInputFinalOutputPolicy6V1>()
        .checked_sub(size_of::<PreparedNativeFinalOutputPolicy6V1>())
        .and_then(|bytes| {
            bytes.checked_sub(size_of::<NativeOriginalInputAssociationReceiptsPolicy6V1>())
        })
        .ok_or(Resource::Arithmetic)?;
    let retained = incoming.checked_add(delta).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(delta)?;
    Ok((retained, OriginalInputAssociationStoragePolicy6V1(delta)))
}

fn check(
    final_output: &PreparedNativeFinalOutputPolicy6V1,
    receipts: &NativeOriginalInputAssociationReceiptsPolicy6V1,
    budget: &mut Budget<'_>,
) -> R<usize> {
    scoped(budget, |budget| {
        let incoming = require_incoming(
            final_output.retained_storage_floor_v1(),
            receipts.retained,
            budget,
        )?;
        // This is the signed-custody boundary. No unsigned component can
        // construct the retained native source owner or skip its complete replay.
        final_output.verify_equivalence(budget)?;
        joins::check(&final_output.native, receipts, budget)?;
        Ok(incoming)
    })
}

/// The same signed native owner and exact final-I receipts, extended with a
/// checked original-N V4 input association. No signed whole-chain proof,
/// compiler-origin capability, output-wire admission or publisher is added.
pub(crate) struct PreparedNativeAssociatedInputFinalOutputPolicy6V1 {
    final_output: PreparedNativeFinalOutputPolicy6V1,
    input_receipts: NativeOriginalInputAssociationReceiptsPolicy6V1,
    retained_floor: usize,
}
impl PreparedNativeFinalOutputPolicy6V1 {
    /// Consumes both already-reserved owners once. Incoming reservations remain
    /// caller-owned on all exits. Success returns only additional wrapper growth,
    /// unreserved; the N envelope and inherited history are not copied.
    pub(crate) fn try_admit_original_input_association_v1(
        self,
        receipts: NativeOriginalInputAssociationReceiptsPolicy6V1,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedNativeAssociatedInputFinalOutputPolicy6V1,
        OriginalInputAssociationStoragePolicy6V1,
    )> {
        scoped(budget, move |budget| {
            let incoming = check(&self, &receipts, budget)?;
            let (retained_floor, receipt) = finish_receipt(incoming, budget)?;
            Ok((
                PreparedNativeAssociatedInputFinalOutputPolicy6V1 {
                    final_output: self,
                    input_receipts: receipts,
                    retained_floor,
                },
                receipt,
            ))
        })
    }
}
impl PreparedNativeAssociatedInputFinalOutputPolicy6V1 {
    pub(crate) fn original(&self) -> &Graph {
        let inputs = self.final_output.native.stage.inputs();
        match inputs.output.owner {
            super::OutputOwnerV1::Direct6(owner) => {
                owner.source_semantic_kir().pre_ranked_executable().unwrap()
            }
            super::OutputOwnerV1::Erased6(owner) => owner.original_source().executable(),
            _ => unreachable!("private fixed Policy6 native owner"),
        }
    }
    pub(crate) fn original_kernel_ir_preimage(&self) -> &[u8] {
        match &self.final_output.native.stage {
            Stage6::Direct(stage) => stage.native_original_envelope_v1(),
            Stage6::Erased(stage) => stage.native_original_envelope_v1(),
        }
    }
    pub(crate) fn output(&self) -> &Graph {
        self.final_output.output()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.final_output.handoff()
    }
    pub(crate) fn input_receipts(&self) -> &NativeOriginalInputAssociationReceiptsPolicy6V1 {
        &self.input_receipts
    }
    pub(crate) fn final_receipts(&self) -> &NativeFinalOutputReceiptsPolicy6V1 {
        self.final_output.receipts()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        check(&self.final_output, &self.input_receipts, budget).map(|_| ())
    }
}
