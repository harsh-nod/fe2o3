//! Borrowed original-N receipt component. No V4 proof association is inferred.
use super::*;
use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1 as Ranked;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as MAX_ROOTS, MultiRootProofRosterKindV3 as Kind,
    MultiRootProofRosterTranscriptV3 as Roster, NativeNeutralModuleRefV1,
};
use fe2o3_kernel_ir::{
    InertFormalMemoryReceiptFormatV4 as Formal, MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
};
use fe2o3_lower_mir_kernel::{
    OriginalNativeFormalMemoryErrorV1, OriginalNativeFormalMemoryV1,
    analyze_original_native_formal_memory_v1, analyze_original_unit_local_formal_memory_v1,
};

#[derive(Debug)]
pub(crate) enum OriginalNativeInputReceiptErrorV1 {
    Resource(Resource),
    Native(Box<NativeOutputHandoffErrorV1>),
    Formal(Box<OriginalNativeFormalMemoryErrorV1>),
    Mismatch(&'static str),
    Panicked,
}
type Error = OriginalNativeInputReceiptErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<NativeOutputHandoffErrorV1> for Error {
    fn from(error: NativeOutputHandoffErrorV1) -> Self {
        Self::Native(Box::new(error))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "original native receipt: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Native(error) => Some(error.as_ref()),
            Self::Formal(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OriginalReceiptStoragePolicy6V1(usize);
impl OriginalReceiptStoragePolicy6V1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Lifetime-bound view of exact borrowed input bytes and the existing signed
/// native owner. Neither graph nor wire is cloned. No new proof is constructed.
pub(crate) struct CheckedOriginalNativeInputReceiptsPolicy6V1<'n, 'r> {
    native: &'n PreparedNativeCheckedOutputWorkerHandoffPolicy6V1,
    kernel_ir: &'r [u8],
    formal_memory: &'r [u8],
}
impl CheckedOriginalNativeInputReceiptsPolicy6V1<'_, '_> {
    pub(crate) fn original(&self) -> &Graph {
        match self.native.stage.inputs().output.owner {
            OutputOwnerV1::Direct6(owner) => {
                owner.source_semantic_kir().pre_ranked_executable().unwrap()
            }
            OutputOwnerV1::Erased6(owner) => owner.original_source().executable(),
            _ => unreachable!("private fixed Policy6 native stage"),
        }
    }
    pub(crate) fn kernel_ir(&self) -> &[u8] {
        self.kernel_ir
    }
    pub(crate) fn formal_memory(&self) -> &[u8] {
        self.formal_memory
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Full native replay still requires its genuine signed source owner. This
/// first slice only checks N's existing envelope/formal roster; it does not
/// associate a V4 input proof, reinterpret F2NTR/F2NOUT1 or admit publication.
/// Borrowed wire backing is outside this checker's logical-storage domain:
/// it is not copied or added to the native owner's required floor. In particular,
/// a slice may alias that owner's already-accounted native envelope. The caller
/// retains responsibility for the backing's lifetime and accounting in its own
/// domain. Both slices still have the existing independent 4 MiB framing caps.
/// All new decoded scratch and the witness header are charged here. On success
/// the additional borrowed witness header receipt is returned unreserved.
pub(crate) fn check_original_native_input_receipts_policy6_v1<'n, 'r>(
    native: &'n PreparedNativeCheckedOutputWorkerHandoffPolicy6V1,
    kernel_ir: &'r [u8],
    formal_memory: &'r [u8],
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedOriginalNativeInputReceiptsPolicy6V1<'n, 'r>,
    OriginalReceiptStoragePolicy6V1,
)> {
    scope(budget, |budget| {
        if budget.storage() < native.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        native.verify_equivalence(budget)?;
        let ranked = match &native.stage {
            Stage6::Direct(stage) => stage.native_final_receipt_ranked_v1(),
            Stage6::Erased(stage) => stage.native_final_receipt_ranked_v1(),
        };
        let inputs = native.stage.inputs();
        check_component(
            inputs.output.owner,
            inputs.output.catalog,
            ranked,
            &inputs.bindings.typed_descriptor_roots,
            kernel_ir,
            formal_memory,
            budget,
        )?;
        let retained = size_of::<CheckedOriginalNativeInputReceiptsPolicy6V1<'_, '_>>();
        budget.reserve_storage(retained)?;
        Ok((
            CheckedOriginalNativeInputReceiptsPolicy6V1 {
                native,
                kernel_ir,
                formal_memory,
            },
            OriginalReceiptStoragePolicy6V1(retained),
        ))
    })
}

fn scope<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut deferred_panic = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            deferred_panic = Some(payload);
            Err(Error::Panicked)
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
    drop(deferred_panic);
    result
}

fn analyze<'n>(
    owner: OutputOwnerV1<'n>,
    budget: &mut Budget<'_>,
) -> Result<OriginalNativeFormalMemoryV1<'n>> {
    let (reports, storage) = match owner {
        OutputOwnerV1::Direct6(owner) => {
            analyze_original_native_formal_memory_v1(owner.source_semantic_kir(), budget)
        }
        OutputOwnerV1::Erased6(owner) => {
            analyze_original_unit_local_formal_memory_v1(owner.erased_source(), budget)
        }
        _ => return Err(Error::Mismatch("fixed original-N source owner")),
    }
    .map_err(|error| Error::Formal(Box::new(error)))?;
    budget.reserve_storage(storage.retained_storage())?;
    Ok(reports)
}

fn subject(original: &Graph, catalog: &Catalog) -> Result<Subject> {
    Subject::new(
        *original.canonical().identity().digest(),
        original.canonical().identity().canonical_length(),
        *catalog.digest(),
        u64::try_from(catalog.canonical_bytes().len()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(|_| Error::Mismatch("original native subject"))
}

fn same_bytes(
    left: &[u8],
    right: &[u8],
    budget: &mut Budget<'_>,
    field: &'static str,
) -> Result<()> {
    budget.charge_work(
        left.len()
            .checked_add(right.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if left != right {
        return Err(Error::Mismatch(field));
    }
    Ok(())
}

/// Private unsigned component reused by the genuine signed-native endpoint.
/// Its source path is closed to the retained Policy6 Direct/Erased owner.
pub(super) fn check_component(
    owner: OutputOwnerV1<'_>,
    catalog: &Catalog,
    ranked: &Ranked,
    typed: &[TypedDescriptorRootV1],
    kernel_ir: &[u8],
    formal_memory: &[u8],
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope(budget, |budget| {
        budget.charge_work(8)?;
        if [kernel_ir, formal_memory]
            .iter()
            .any(|wire| wire.is_empty() || wire.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3)
        {
            return Err(Error::Mismatch("original receipt preimage extent"));
        }
        let source = owner.source(catalog)?;
        let reports = analyze(owner, budget)?;
        if !std::ptr::eq(reports.original(), source.original) {
            return Err(Error::Mismatch("exact original formal owner"));
        }
        let expected = subject(source.original, catalog)?;
        budget.charge_work(112)?;
        let envelope = NativeNeutralModuleRefV1::decode(kernel_ir)
            .map_err(|_| Error::Mismatch("original KernelIr native envelope"))?;
        if envelope.subject() != &expected {
            return Err(Error::Mismatch("original KernelIr subject"));
        }
        same_bytes(
            envelope.graph_bytes(),
            source.original.canonical().canonical_bytes(),
            budget,
            "original KernelIr graph bytes",
        )?;
        same_bytes(
            envelope.catalog_bytes(),
            catalog.canonical_bytes(),
            budget,
            "original KernelIr catalog bytes",
        )?;
        let storage = formal_memory
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<Roster>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(storage)?;
        budget.charge_work(formal_memory.len())?;
        // Existing codec metadata/formal payloads retain their own capped
        // domains; visible decoded headers and bytes are prepaid here.
        let roster = Roster::decode(formal_memory)
            .map_err(|_| Error::Mismatch("original FormalMemory native roster"))?;
        check_roots(source, ranked, typed, &reports, &expected, &roster, budget)
    })
}

#[path = "production_native_original_receipt_joins_policy6_v1.rs"]
mod joins;
use joins::check_roots;
#[cfg(test)]
#[path = "production_native_original_receipts_policy6_v1_tests.rs"]
pub(crate) mod tests;
