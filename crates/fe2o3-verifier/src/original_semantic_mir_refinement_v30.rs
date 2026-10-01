//! Exact original Semantic MIR-to-canonical obligation, independent of policy.
//! This is generated proof input, never an executed or imported certificate.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
use std::mem::align_of;

const DOMAIN: &[u8] = b"FE2O3/ORIGINAL-SEMANTIC-MIR/SCALAR-TRACE/V30\0";
const CONTROL_DOMAIN: &[u8] = b"FE2O3/ORIGINAL-SEMANTIC-MIR/CONTROL-TRACE/V31\0";
type PreparationCapture<'view, 'source> = (&'view Source<'source>, usize, Ledger, usize);

/// Content identity of the complete original source and canonical proof scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginalSemanticMirRefinementSubject<const VERSION: u16> {
    semantic: [u8; 32],
    ssa: [u8; 32],
    canonical: VerifiedCanonicalKernelIrIdentityV18,
    statement: [u8; 32],
    census: [usize; 6],
}
/// Historical straight-line scalar statement identity, with its unchanged domain.
pub type OriginalSemanticMirRefinementSubjectV30 = OriginalSemanticMirRefinementSubject<30>;
/// Whole scalar CFG statement identity, distinct from the historical scalar domain.
pub type OriginalSemanticMirRefinementSubjectV31 = OriginalSemanticMirRefinementSubject<31>;

impl<const VERSION: u16> OriginalSemanticMirRefinementSubject<VERSION> {
    /// Exact original Semantic MIR content identity.
    pub const fn semantic_identity(self) -> [u8; 32] {
        self.semantic
    }
    /// Exact admitted original SSA identity, not an optimized reconstruction.
    pub const fn ssa_identity(self) -> [u8; 32] {
        self.ssa
    }
    /// Exact original canonical endpoint, before any optimizer policy.
    pub const fn canonical_identity(self) -> VerifiedCanonicalKernelIrIdentityV18 {
        self.canonical
    }
    /// Domain-separated whole-source generated statement identity.
    pub const fn statement_identity(self) -> [u8; 32] {
        self.statement
    }
    /// Complete roots, instances, statements, assignments, operations, definitions.
    pub const fn census(self) -> [usize; 6] {
        self.census
    }
}

/// Move-only request borrowing the original source owner. No constructor takes
/// caller-selected equations, endpoint rows, generated text or identities.
/// Preparation returns an unreserved receipt; reserve retained_storage before
/// querying it, and drop it before releasing that credit on the original ledger.
pub struct PreparedOriginalSemanticMirRefinement<'view, 'source, const VERSION: u16> {
    source: &'view Source<'source>,
    generated: CanonicalGeneratedVerusProofInputV3,
    subject: OriginalSemanticMirRefinementSubject<VERSION>,
    retained: usize,
    ledger: Ledger,
    slot: usize,
    required: usize,
}
/// Historical straight-line scalar request. It cannot be relabeled as a CFG request.
pub type PreparedOriginalSemanticMirRefinementV30<'view, 'source> =
    PreparedOriginalSemanticMirRefinement<'view, 'source, 30>;
/// Whole scalar CFG request retaining the genuine original source owner.
pub type PreparedOriginalSemanticMirRefinementV31<'view, 'source> =
    PreparedOriginalSemanticMirRefinement<'view, 'source, 31>;

impl<const VERSION: u16> PreparedOriginalSemanticMirRefinement<'_, '_, VERSION> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.source.check_query_v18(budget)?;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            return Err(self
                .source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }
    /// Storage receipt only, not evidence that any proof has executed.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Reads the exact derived subject on its original funded account.
    pub fn subject(
        &self,
        budget: &Budget<'_>,
    ) -> Result<OriginalSemanticMirRefinementSubject<VERSION>> {
        self.check(budget)?;
        Ok(self.subject)
    }
    /// Reads independently interpreted original/actual traces and their lemmas.
    pub fn generated_source(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(self.generated.source())
    }
    /// Requires the actual retained source object, not an equal-byte copy.
    pub fn check_original_source(&self, source: &Source<'_>, budget: &Budget<'_>) -> Result<()> {
        self.check(budget)?;
        if !std::ptr::eq(self.source, source) {
            return Err(Error::Statement(
                "original MIR request has a foreign source owner",
            ));
        }
        Ok(())
    }
    /// False until a separately admitted runtime proves this exact request.
    pub const fn authenticates_executed_proof(&self) -> bool {
        false
    }
    /// Generated source correspondence is not device or artifact authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn retain(error: Error, source: &Source<'_>) -> Error {
    match error {
        Error::Resource(resource)
        | Error::Inventory(CanonicalKirInventoryErrorV1::Resource(resource)) => {
            Error::Source(source.retain_query_resource_error_v18(resource))
        }
        other => other,
    }
}

/// Generates one complete all-root obligation over independently read original
/// MIR and actual canonical operations. Unsupported source control/effects or
/// whole bindings refuse the entire request. This never invokes Verus.
pub fn prepare_original_semantic_mir_refinement_v30<'view, 'source>(
    source: &'view Source<'source>,
    budget: &mut Budget<'_>,
) -> Result<PreparedOriginalSemanticMirRefinementV30<'view, 'source>> {
    prepare::<30>(source, budget)
}

/// Generates an all-root scalar CFG obligation with original local-state,
/// branch/loop and exact invocation-prefix semantics. Calls and memory refuse
/// the complete request. This function does not execute or import a proof.
pub fn prepare_original_semantic_mir_refinement_v31<'view, 'source>(
    source: &'view Source<'source>,
    budget: &mut Budget<'_>,
) -> Result<PreparedOriginalSemanticMirRefinementV31<'view, 'source>> {
    prepare::<31>(source, budget)
}

fn prepare<'view, 'source, const VERSION: u16>(
    source: &'view Source<'source>,
    budget: &mut Budget<'_>,
) -> Result<PreparedOriginalSemanticMirRefinement<'view, 'source, VERSION>> {
    source.check_query_v18(budget)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let headers = original_mir_headers::<VERSION>()?;
    let capture: PreparationCapture<'_, '_> = (source, floor, ledger, slot);
    let produce = move |budget: &mut Budget<'_>| {
        let (source, floor, ledger, slot) = std::convert::identity(capture);
        produce::<VERSION>(source, floor, ledger, slot, budget)
            .map_err(|error| retain(error, source))
    };
    #[cfg(test)]
    {
        assert_eq!(
            std::mem::size_of_val(&produce),
            size_of::<PreparationCapture<'_, '_>>()
        );
        assert_eq!(
            std::mem::align_of_val(&produce),
            align_of::<PreparationCapture<'_, '_>>()
        );
    }
    budget
        .with_prepaid_scope(floor, 1, 1, headers, produce)
        .map_err(|error| retain(error, source))
}

fn generate<const VERSION: u16>(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<(String, [usize; 6])> {
    let mut writer = Writer::new(budget)?;
    let census = match VERSION {
        30 => semantics::original_scalar_v30::generate(relation, &mut writer)?,
        31 => semantics::original_scalar_v30::generate_control_v31(relation, &mut writer)?,
        _ => {
            return Err(Error::Statement(
                "original MIR proof domain is not registered",
            ));
        }
    };
    Ok((writer.finish()?, census))
}

fn produce<'view, 'source, const VERSION: u16>(
    source: &'view Source<'source>,
    floor: usize,
    ledger: Ledger,
    slot: usize,
    budget: &mut Budget<'_>,
) -> Result<PreparedOriginalSemanticMirRefinement<'view, 'source, VERSION>> {
    let owner = source.canonical(budget)?;
    let original = source.source_ssa(budget)?;
    let (inventory, receipt) = Inventory::derive_v18(owner, budget)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let (text, census) =
        source.with_ranked_correspondence_v18(&inventory, budget, generate::<VERSION>)?;
    budget.charge_work(text.len().checked_mul(3).ok_or(Resource::Arithmetic)?)?;
    let generated = CanonicalGeneratedVerusProofInputV3::new(text.into_bytes())?;
    let semantic = *original.source_semantic_sha256();
    let ssa = *original.identity().as_bytes();
    let canonical = *owner.identity();
    let mut digest = Sha256::new();
    digest.update(match VERSION {
        30 => DOMAIN,
        31 => CONTROL_DOMAIN,
        _ => {
            return Err(Error::Statement(
                "original MIR proof domain is not registered",
            ));
        }
    });
    digest.update(semantic);
    digest.update(ssa);
    digest.update(canonical.canonical_length().to_le_bytes());
    digest.update(canonical.digest());
    for value in census {
        digest.update(
            u64::try_from(value)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
    }
    digest.update(generated.identity().as_bytes());
    let subject = OriginalSemanticMirRefinementSubject::<VERSION> {
        semantic,
        ssa,
        canonical,
        statement: digest.finalize().into(),
        census,
    };
    let retained = size_of::<PreparedOriginalSemanticMirRefinement<'_, '_, VERSION>>()
        .checked_add(query_headers::<VERSION>()?)
        .and_then(|n| n.checked_add(generated.source().len()))
        .ok_or(Resource::Arithmetic)?;
    let required = floor.checked_add(retained).ok_or(Resource::Arithmetic)?;
    source.check_query_v18(budget)?;
    drop(inventory);
    Ok(PreparedOriginalSemanticMirRefinement {
        source,
        generated,
        subject,
        retained,
        ledger,
        slot,
        required,
    })
}

fn query_headers<const VERSION: u16>() -> Result<usize> {
    [
        size_of::<(
            &PreparedOriginalSemanticMirRefinement<'_, '_, VERSION>,
            &Source<'_>,
            &Budget<'_>,
        )>(),
        size_of::<OriginalSemanticMirRefinementSubject<VERSION>>(),
        size_of::<Result<OriginalSemanticMirRefinementSubject<VERSION>>>(),
        size_of::<Result<&[u8]>>(),
        3 * size_of::<Result<()>>(),
        size_of::<std::result::Result<(), SourceError>>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total.checked_add(bytes).ok_or(Resource::Arithmetic.into())
    })
}

fn original_mir_headers<const VERSION: u16>() -> Result<usize> {
    type Request<'a, const V: u16> = PreparedOriginalSemanticMirRefinement<'a, 'a, V>;
    type InventoryResult<'a> = std::result::Result<
        (
            Inventory<'a>,
            fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        ),
        CanonicalKirInventoryErrorV1,
    >;
    [
        2 * SOURCE_LIMIT,
        query_headers::<VERSION>()?,
        size_of::<Request<'_, VERSION>>(),
        2 * size_of::<Result<Request<'_, VERSION>>>(),
        size_of::<std::thread::Result<Result<Request<'_, VERSION>>>>(),
        size_of::<PreparationCapture<'_, '_>>(),
        align_of::<PreparationCapture<'_, '_>>(),
        size_of::<std::panic::AssertUnwindSafe<PreparationCapture<'_, '_>>>(),
        size_of::<(&Source<'_>, &mut Budget<'_>, Ledger, usize, usize)>(),
        size_of::<(
            &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        )>(),
        size_of::<(
            &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
            &mut Budget<'_>,
        )>(),
        size_of::<(
            Inventory<'_>,
            fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        )>(),
        size_of::<InventoryResult<'_>>(),
        size_of::<Writer<'_, '_>>(),
        size_of::<Result<Writer<'_, '_>>>(),
        size_of::<(String, [usize; 6])>(),
        size_of::<Result<(String, [usize; 6])>>(),
        size_of::<Result<String>>(),
        size_of::<
            std::result::Result<
                CanonicalGeneratedVerusProofInputV3,
                GeneratedVerusProofInputErrorV3,
            >,
        >(),
        size_of::<OriginalSemanticMirRefinementSubject<VERSION>>(),
        size_of::<Sha256>(),
        2 * size_of::<Result<()>>(),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total.checked_add(bytes).ok_or(Resource::Arithmetic.into())
    })
}

#[cfg(test)]
fn query_headers_v30() -> Result<usize> {
    query_headers::<30>()
}

#[cfg(test)]
fn original_mir_headers_v30() -> Result<usize> {
    original_mir_headers::<30>()
}

#[cfg(test)]
#[path = "original_semantic_mir_refinement_v30_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "original_semantic_mir_refinement_v31_tests.rs"]
mod control_tests;
