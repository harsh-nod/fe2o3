//! Bounded reconstruction of policy values, never admission of their origin.
use crate::{
    NativeConditionalPacketErrorV2, NativeConditionalRootPolicyV2,
    with_decoded_native_conditional_source_packet_v2,
};
use fe2o3_compiler_lineage::{
    NativeConditionalPolicyRosterErrorV1, read_native_conditional_policy_roster_v1,
};
use fe2o3_functional_proof::{
    FunctionalRefinementBoundaryV2 as Boundary, FunctionalRefinementImportErrorV2,
    FunctionalRefinementImportPolicyV2 as Formula, VerusToolchainIdentityV2 as Toolchain,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_pliron::ProductionRefinementStagingPolicyV2 as Effects;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest, Sha256};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

// Logical codec/crypto and collection allowances, not instruction or RSS bounds.
const KEY_WORK: usize = 128 * 1024;
const FRAME: usize = 8192;
// The pinned BTreeSet uses fixed-size nodes. This conservative per-key quote
// includes an extra root node, insertion scratch and intermediate tree headers.
const EFFECT_KEY_STORAGE: usize = 1024;

struct Root {
    semantic_root: u32,
    kernel_binding: [u8; 32],
    effects: Effects,
    formula: Formula,
}

/// Move-only inert policy values reconstructed from a source-bound roster.
///
/// This does not trust the roster's issuer, compiler, signer keys or toolchain.
/// A production caller must independently authenticate that exact roster before
/// passing its views to source recovery. Content agreement is not that approval.
/// Keep the returned charge on the original account. Source backing may retire
/// after reconstruction: the policy values no longer borrow it.
///
/// ```compile_fail
/// use fe2o3_verifier::InertNativeConditionalPolicyRosterV1 as R;
/// fn duplicate(value: R) { let _ = value.clone(); }
/// ```
pub struct InertNativeConditionalPolicyRosterV1 {
    roots: Vec<Root>,
    identity: [u8; 32],
    retained: usize,
    ledger: Ledger,
    address: usize,
}

/// Unreserved complete owner charge. Reserve before keeping or lending views.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalPolicyRosterStorageV1(usize);
impl NativeConditionalPolicyRosterStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum NativeConditionalPolicyReconstructionErrorV1 {
    Resource(Resource),
    Frame(NativeConditionalPolicyRosterErrorV1<Resource>),
    Packet(NativeConditionalPacketErrorV2),
    Formula(FunctionalRefinementImportErrorV2),
    Mismatch(&'static str),
}
type E = NativeConditionalPolicyReconstructionErrorV1;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional policy reconstruction: {self:?}")
    }
}
impl std::error::Error for E {}

/// Reconstructs inert policy values and checks their complete source association.
/// Both input slices' backing remains prepaid. Failure/unwind retains terminal
/// reservations; only known successful construction transfers an unreserved owner.
pub fn reconstruct_inert_native_conditional_policy_roster_v1(
    bytes: &[u8],
    source_packet: &[u8],
    budget: &mut Budget<'_>,
) -> Result<
    (
        InertNativeConditionalPolicyRosterV1,
        NativeConditionalPolicyRosterStorageV1,
    ),
    E,
> {
    budget.charge_work(8)?;
    let incoming = budget.storage();
    if incoming
        < bytes
            .len()
            .checked_add(source_packet.len())
            .ok_or(Resource::Arithmetic)?
    {
        return Err(Resource::Accounting.into());
    }
    let ledger = budget.work_ledger_identity_v1();
    let address = budget as *const Budget<'_> as usize;
    budget.reserve_storage(FRAME)?;
    let frame = read_native_conditional_policy_roster_v1(bytes, budget.storage_limit(), |work| {
        budget.charge_work(work)
    })
    .map_err(E::Frame)?;
    budget.charge_work(
        source_packet
            .len()
            .checked_add(192)
            .ok_or(Resource::Arithmetic)?,
    )?;
    if frame.source_packet_len() != source_packet.len() as u64
        || Sha256::digest(source_packet).as_slice() != frame.source_packet_sha256()
    {
        return Err(E::Mismatch("roster differs from exact source packet"));
    }
    let header = size_of::<InertNativeConditionalPolicyRosterV1>()
        + size_of::<NativeConditionalPolicyRosterStorageV1>();
    budget.reserve_storage(header)?;
    let (mut roots, capacity) = vector(frame.root_count(), budget)?;
    let mut retained = header.checked_add(capacity).ok_or(Resource::Arithmetic)?;
    for row in frame.roots() {
        let count = row.effect_signer_count();
        let tree = count
            .checked_add(1)
            .and_then(|n| n.checked_mul(EFFECT_KEY_STORAGE))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(
            count
                .checked_mul(2048)
                .and_then(|n| n.checked_add(KEY_WORK))
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.reserve_storage(tree)?;
        let effects = Effects::new(
            row.effect_signers()
                .iter()
                .copied()
                .map(DigestV1::from_untrusted_bytes),
            toolchain(*row.effect_toolchain())?,
        )
        .map_err(|_| E::Mismatch("invalid effect signer policy"))?;
        let boundary = match row.formula_boundary() {
            1 => Boundary::SafeReferenceMirToKernelMir,
            2 => Boundary::SafeReferenceSourceToKernelMir,
            3 => Boundary::SafeReferenceMirToLivePliron,
            _ => return Err(E::Mismatch("unknown formula boundary")),
        };
        let formula = Formula::new(
            *row.formula_verifying_key(),
            toolchain(*row.formula_toolchain())?,
            boundary,
        )
        .map_err(E::Formula)?;
        roots.push(Root {
            semantic_root: row.semantic_root(),
            kernel_binding: *row.kernel_binding(),
            effects,
            formula,
        });
        retained = retained.checked_add(tree).ok_or(Resource::Arithmetic)?;
    }
    let result =
        with_decoded_native_conditional_source_packet_v2(source_packet, budget, |source, b| {
            b.charge_work(roots.len().checked_mul(40).ok_or(Resource::Arithmetic)?)?;
            if source.roots.len() != roots.len() {
                return Err(E::Mismatch("incomplete policy root roster"));
            }
            for (row, root) in source.roots.iter().zip(&roots) {
                if row.semantic_root != root.semantic_root
                    || row.launch.kernel_binding() != root.kernel_binding
                {
                    return Err(E::Mismatch("source-ordered policy root association"));
                }
            }
            Ok(())
        })
        .map_err(E::Packet)?;
    result?;
    let expected = incoming
        .checked_add(FRAME)
        .and_then(|n| n.checked_add(retained))
        .ok_or(Resource::Arithmetic)?;
    if budget.work_ledger_identity_v1() != ledger || budget.storage() != expected {
        return Err(Resource::Accounting.into());
    }
    let owner = InertNativeConditionalPolicyRosterV1 {
        roots,
        identity: *frame.identity().sha256(),
        retained,
        ledger,
        address,
    };
    budget.release_storage(FRAME.checked_add(retained).ok_or(Resource::Arithmetic)?)?;
    Ok((owner, NativeConditionalPolicyRosterStorageV1(retained)))
}

impl InertNativeConditionalPolicyRosterV1 {
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    fn check_account(&self, budget: &mut Budget<'_>) -> Result<(), E> {
        budget.charge_work(8)?;
        if budget.work_ledger_identity_v1() != self.ledger
            || budget as *const Budget<'_> as usize != self.address
            || budget.storage() < self.retained
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    /// Compares every field against independently accepted caller policy. The
    /// caller retains and prepays that backing; equality does not admit its origin.
    pub fn require_expected_policies(
        &self,
        expected: &[NativeConditionalRootPolicyV2<'_>],
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        self.check_account(budget)?;
        if expected.len() != self.roots.len() {
            return Err(E::Mismatch("accepted policy root count"));
        }
        for (root, expected) in self.roots.iter().zip(expected) {
            let visits = root
                .effects
                .signer_identities()
                .len()
                .checked_mul(64)
                .and_then(|n| n.checked_add(1024))
                .ok_or(Resource::Arithmetic)?;
            budget.charge_work(visits)?;
            if root.semantic_root != expected.semantic_root
                || root.effects != *expected.effects
                || root.formula != *expected.formula
            {
                return Err(E::Mismatch(
                    "accepted policy differs from source-bound roster",
                ));
            }
        }
        Ok(())
    }

    /// Lends policy values, without accepting their trust provenance. The view
    /// array cannot escape; callback-owned output stays charged on this account.
    /// A replaced ledger, damaged floor, or unwind never earns a scratch refund.
    pub fn with_root_policies<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl for<'view> FnOnce(
            &'view [NativeConditionalRootPolicyV2<'view>],
            &mut Budget<'_>,
        ) -> R,
    ) -> Result<R, E> {
        self.check_account(budget)?;
        budget.charge_work(self.roots.len())?;
        budget.reserve_storage(FRAME)?;
        let (mut views, capacity) = vector(self.roots.len(), budget)?;
        for root in &self.roots {
            views.push(NativeConditionalRootPolicyV2 {
                semantic_root: root.semantic_root,
                effects: &root.effects,
                formula: &root.formula,
            });
        }
        let floor = budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| consume(&views, budget)));
        drop(views);
        let intact = budget.work_ledger_identity_v1() == self.ledger
            && budget as *const Budget<'_> as usize == self.address
            && budget.storage() >= floor;
        match result {
            Err(payload) => resume_unwind(payload),
            Ok(result) if !intact => {
                drop(result);
                Err(Resource::Accounting.into())
            }
            Ok(result) => {
                budget.release_storage(FRAME.checked_add(capacity).ok_or(Resource::Arithmetic)?)?;
                Ok(result)
            }
        }
    }
}

fn toolchain(fields: [[u8; 32]; 5]) -> Result<Toolchain, E> {
    let [verus, verus_config, solver, solver_config, runtime] =
        fields.map(DigestV1::from_untrusted_bytes);
    Toolchain::new(verus, verus_config, solver, solver_config, runtime).map_err(E::Formula)
}

fn vector<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize), E> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok((values, actual))
}

#[cfg(test)]
#[path = "compiler_native_conditional_policy_roster_v1_tests.rs"]
mod tests;
