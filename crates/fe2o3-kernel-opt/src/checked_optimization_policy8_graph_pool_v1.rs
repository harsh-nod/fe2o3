//! Fresh V12 graph admission only. Historical policy claims remain inert.
use crate::{CanonicalPolicy8HistoryRoleV1 as Role, InertPolicy8HistoryRefV1 as Frame};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError, CanonicalKernelIrReplayStorageV12,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

const ROLES: [Role; 7] = [
    Role::B,
    Role::C,
    Role::S,
    Role::O,
    Role::I,
    Role::J,
    Role::K,
];
const MAX_STORED: usize = 6;
const ROLE_WORK: usize = 82;

/// Graph-pool construction refusal, not a policy-semantic verdict.
#[derive(Debug)]
pub enum CanonicalPolicy8GraphPoolErrorV1 {
    /// Shared cumulative work, storage, allocation or accounting refusal.
    Resource(Resource),
    /// Fresh admission of this first-use pool ordinal failed.
    Admission {
        /// Canonical first-use ordinal of the rejected stored graph.
        index: usize,
        /// Unchanged upstream V12 admission refusal and diagnostics.
        error: AdmissionError,
    },
    /// A claimed role differs from its freshly admitted or external subject.
    RoleIdentity(Role),
    /// An inaccessible or out-of-range inert locator was refused defensively.
    Pool,
    /// Owned partial construction was dropped after an unwind.
    Panicked,
}
type Error = CanonicalPolicy8GraphPoolErrorV1;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy8 history graph admission: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Unreserved logical transfer: wrapper, observed Vec backing and every complete
/// opaque V12 admission receipt. Inline owner headers deliberately overlap;
/// this layer does not infer any credit from upstream receipt internals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy8GraphPoolStorageV1(usize);
impl CanonicalPolicy8GraphPoolStorageV1 {
    /// Reserve before subsequent controlled allocation while the owner lives.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

struct Entry {
    owner: Owner,
    storage: CanonicalKernelIrReplayStorageV12,
}

/// Move-only fresh graph owners tied to the exact borrowed inert frame and K.
/// Each stored non-K graph is decoded and fully V12-verified once. Role aliases
/// resolve to the same retained owner; external aliases borrow the supplied K.
/// Equal-byte independently admitted K is valid, not proof of producer custody.
/// Prefix/row claims remain opaque; this is neither transformation preservation
/// nor execution, signed-source, native, protected admission or launch authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::AdmittedPolicy8HistoryGraphPoolV1;
/// fn duplicate(value: AdmittedPolicy8HistoryGraphPoolV1<'_, '_, '_>) {
///     let _ = value.clone();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{AdmittedPolicy8HistoryGraphPoolV1 as Pool,
///     InertPolicy8HistoryRefV1 as Frame, admit_policy8_history_graph_pool_v1 as admit};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'w, 'k>(frame: Frame<'w, 'k>, b: &mut Budget<'_>) -> Pool<'static, 'w, 'k> {
///     admit(&frame, b).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::AdmittedPolicy8HistoryGraphPoolV1 as Pool;
/// fn escape_k<'f, 'w, 'k>(pool: Pool<'f, 'w, 'k>) -> Pool<'f, 'w, 'static> { pool }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::AdmittedPolicy8HistoryGraphPoolV1 as Pool;
/// fn escape_wire<'f, 'w, 'k>(pool: Pool<'f, 'w, 'k>) -> Pool<'f, 'static, 'k> { pool }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{AdmittedPolicy8HistoryGraphPoolV1 as Pool, CanonicalPolicy8HistoryRoleV1 as Role};
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 as Owner;
/// fn escape<'a>(pool: Pool<'_, '_, '_>) -> &'a Owner { pool.graph(Role::B) }
/// ```
pub struct AdmittedPolicy8HistoryGraphPoolV1<'frame, 'wire, 'k> {
    frame: &'frame Frame<'wire, 'k>,
    entries: Vec<Entry>,
    role_indices: [Option<usize>; 7],
    storage: CanonicalPolicy8GraphPoolStorageV1,
}
impl<'frame, 'wire, 'k> AdmittedPolicy8HistoryGraphPoolV1<'frame, 'wire, 'k> {
    /// Original borrowed syntax/opaque prefix claims, not a semantic receipt.
    pub const fn frame(&self) -> &'frame Frame<'wire, 'k> {
        self.frame
    }
    /// Actual role subject, freshly admitted from its pool bytes or borrowed K.
    pub fn graph(&self, role: Role) -> &Owner {
        match self.role_indices[role as usize] {
            Some(index) => &self.entries[index].owner,
            None => self.frame.external_output(),
        }
    }
    /// One fresh owner by canonical first-use ordinal, without exposing mutation.
    pub fn stored_graph(&self, index: usize) -> Option<&Owner> {
        self.entries.get(index).map(|entry| &entry.owner)
    }
    /// Number of fresh admissions; a complete no-op may have zero stored graphs.
    pub fn stored_graph_count(&self) -> usize {
        self.entries.len()
    }
    /// Exact external owner borrow, never reconstructed from a digest.
    pub const fn external_output(&self) -> &'k Owner {
        self.frame.external_output()
    }
    /// Complete conservative logical transfer, returned unreserved.
    pub const fn storage(&self) -> CanonicalPolicy8GraphPoolStorageV1 {
        self.storage
    }
    /// Graph admission alone proves no transformation relation.
    pub const fn proves_semantic_preservation(&self) -> bool {
        false
    }
    /// There is no authenticated execution record in this result.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// Source, proof, native and launch authority remain absent.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if ledger != budget.work_ledger_identity_v1() {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    // Payload destruction can itself unwind, but only after valid-ledger cleanup.
    drop(payloads);
    result
}

fn entries(count: usize, budget: &mut Budget<'_>) -> Result<Vec<Entry>, Error> {
    let requested = count
        .checked_mul(size_of::<Entry>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let observed = values
        .capacity()
        .checked_mul(size_of::<Entry>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        observed
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(values)
}

/// Admits each distinct stored graph exactly once, in frame first-use order, on
/// the caller's one cumulative Budget. This reuses the existing full allocation-
/// metered V12 decoder/verifier/hash; no Module clone or unbudgeted decode occurs.
/// Each role's raw digest and length are checked against an actual typed owner.
/// The external K is the exact borrow already bound by the inert frame reader.
///
/// The frame, wire and K backing remain caller-owned/external or reserved once;
/// this API neither copies nor silently charges that borrowed backing. Its own
/// header and observed Vec capacity coexist with each COMPLETE upstream logical
/// receipt; bounded inline-header overlap is intentional, with no inferred credit.
/// Decoder tree bounds/requested payload and transferred error diagnostics retain
/// the upstream convention. This is not allocator overhead, RSS or diagnostic
/// retention accounting, and does not change any wire/work/storage cap.
///
/// Each successful entry receipt is re-reserved before the next admission.
/// Failed owners and scratch drop before floor cleanup; accepted work/peak/first
/// denial remain cumulative. Original Work and incoming floor are checked before
/// any refund. Success returns the owner with its storage UNRESERVED: reserve its
/// complete transfer before subsequent controlled allocation, release only after
/// dropping it. No semantic history replay or prefix authentication is performed.
pub fn admit_policy8_history_graph_pool_v1<'frame, 'wire, 'k>(
    frame: &'frame Frame<'wire, 'k>,
    budget: &mut Budget<'_>,
) -> Result<AdmittedPolicy8HistoryGraphPoolV1<'frame, 'wire, 'k>, Error> {
    scoped(budget, |budget| {
        let floor = budget.storage();
        budget.reserve_storage(size_of::<AdmittedPolicy8HistoryGraphPoolV1<'_, '_, '_>>())?;
        budget.charge_work(1)?;
        let count = frame.stored_graph_count();
        if count > MAX_STORED {
            return Err(Error::Pool);
        }
        let mut entries = entries(count, budget)?;
        for index in 0..count {
            budget.charge_work(1)?;
            let bytes = frame
                .unverified_pool_graph_bytes(index)
                .ok_or(Error::Pool)?;
            let (owner, storage) =
                Owner::from_canonical_bytes_with_verification_budget_v12(bytes, budget)
                    .map_err(|error| Error::Admission { index, error })?;
            budget.reserve_storage(storage.retained_storage())?;
            entries.push(Entry { owner, storage });
        }
        let mut role_indices = [None; 7];
        for (ordinal, role) in ROLES.into_iter().enumerate() {
            budget.charge_work(ROLE_WORK)?;
            let locator = frame.role(role);
            let index = locator
                .pool_index()
                .map(usize::try_from)
                .transpose()
                .map_err(|_| Error::Pool)?;
            let owner = match index {
                Some(index) => &entries.get(index).ok_or(Error::Pool)?.owner,
                None => frame.external_output(),
            };
            let identity = owner.canonical().identity();
            if locator.digest() != *identity.digest()
                || locator.canonical_length() != identity.canonical_length()
            {
                return Err(Error::RoleIdentity(role));
            }
            role_indices[ordinal] = index;
        }
        budget.charge_work(count)?;
        let mut retained = entries
            .capacity()
            .checked_mul(size_of::<Entry>())
            .and_then(|bytes| {
                bytes.checked_add(size_of::<AdmittedPolicy8HistoryGraphPoolV1<'_, '_, '_>>())
            })
            .ok_or(Resource::Arithmetic)?;
        for entry in &entries {
            retained = retained
                .checked_add(entry.storage.retained_storage())
                .ok_or(Resource::Arithmetic)?;
        }
        if budget.storage().checked_sub(floor) != Some(retained) {
            return Err(Resource::Accounting.into());
        }
        Ok(AdmittedPolicy8HistoryGraphPoolV1 {
            frame,
            entries,
            role_indices,
            storage: CanonicalPolicy8GraphPoolStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy8_graph_pool_resource_v1_tests.rs"]
mod resource_tests;
