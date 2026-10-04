//! Inert ConditionalDirect transport, separate from ordinary source packet V1.
//! Opaque leaf frames and signatures are not admitted here. Source completeness,
//! accepted policy, CPU replay, formula import and launch origin remain separate.

use crate::{
    InertFunctionalRefinementReceiptSignatureV2 as Signature,
    NativeCompilerStagingCommitmentV1 as Staging,
};
use fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 as SIGNATURE_BYTES;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Account,
};
use fe2o3_lower_mir_kernel::ProductionSourceLaunchRootInputV1 as Launch;
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

mod decode;
#[cfg(test)]
mod tests;
mod wire;

const MAGIC: &[u8; 8] = b"F2NSRC2\0";
const VERSION: u16 = 2;
const ROUTE: u8 = 3;
const MAX_BYTES: usize = fe2o3_compiler_lineage::MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1;
const MAX_ROOTS: usize = fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3;
const EFFECT_BYTES: usize = 9 * 32 + SIGNATURE_BYTES + 32;
const EVIDENCE_BYTES: usize = 4 + SIGNATURE_BYTES + 32;
const MIN_ROOT_BYTES: usize = 83 + EFFECT_BYTES + EVIDENCE_BYTES;

/// Borrowed original source/N and ordered inert root records, not verified input.
#[derive(Clone, Copy, Debug)]
pub struct NativeConditionalSourcePacketInputV2<'a> {
    pub semantic_mir: &'a [u8],
    pub native_module: &'a [u8],
    pub canonical_kernel_order: &'a [u32],
    pub roots: &'a [NativeConditionalSourceRootV2<'a>],
}

/// Lossless independent fields; semantic equality and signature policy are not
/// inferred from the names, ranks, commitments or bytes supplied here.
#[derive(Clone, Copy, Debug)]
pub struct NativeConditionalSourceRootV2<'a> {
    pub semantic_root: u32,
    pub launch_rank: u8,
    pub launch: Launch<'a>,
    pub induction_bytes: &'a [u8],
    pub recipe_bytes: &'a [u8],
    pub source_rows_bytes: &'a [u8],
    pub ranked_ir: &'a str,
    pub cpu_input_bytes: &'a [u8],
    pub staging_commitments: &'a [Staging],
    pub effect_receipts: &'a [Signature],
    pub formula_receipt: &'a Signature,
}

/// Closed framing or original-account resource refusal; never proof authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeConditionalPacketErrorV2 {
    Resource(Resource),
    Invalid(&'static str),
}
type E = NativeConditionalPacketErrorV2;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Invalid(reason) => write!(f, "invalid conditional source packet: {reason}"),
        }
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

/// Unreserved output capacity, excluding the caller's inline Vec header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeConditionalSourcePacketStorageV2(usize);
impl NativeConditionalSourcePacketStorageV2 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Encodes only the closed V2 frame, with a complete counted permutation.
/// Nested frames remain opaque, including B1 CPU bytes and extent proposals.
/// The aggregate 4 MiB cap includes every repeated frame, name and signature.
/// Reserve the returned receipt before further controlled allocation.
pub fn encode_native_conditional_source_packet_v2(
    input: NativeConditionalSourcePacketInputV2<'_>,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, NativeConditionalSourcePacketStorageV2), E> {
    scoped(budget, |s| {
        s.reserve(size_of::<Vec<u8>>() * 2)?;
        check_order(input.canonical_kernel_order, input.roots.len(), s)?;
        let length = wire::encode(input, 0, None, s.budget)?;
        let mut bytes = s.vector::<u8>(length)?;
        let written = wire::encode(input, length, Some(&mut bytes), s.budget)?;
        require(
            written == length && bytes.len() == length,
            "count/fill mismatch",
        )?;
        Ok((bytes, NativeConditionalSourcePacketStorageV2(length)))
    })
}

/// Lends prepaid decoded metadata; the callback may return its own paid owner.
/// Only codec reservations are refunded, after dropping metadata. Callback Err
/// is nested in R; callers must finish outer postchecks before installing it.
/// A failed outer postcheck drops the provisional callback payload. A substituted
/// account or damaged floor is never refunded. Work/denial history is not reset.
///
/// ```compile_fail
/// use fe2o3_verifier::compiler_native_conditional_source_packet_v2::*;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'a>(bytes: &'a [u8], budget: &mut Budget<'_>)
///     -> NativeConditionalSourcePacketInputV2<'a> {
///     with_decoded_native_conditional_source_packet_v2(bytes, budget, |view, _| view).unwrap()
/// }
/// ```
pub fn with_decoded_native_conditional_source_packet_v2<R>(
    bytes: &[u8],
    budget: &mut Budget<'_>,
    consume: impl for<'view> FnOnce(NativeConditionalSourcePacketInputV2<'view>, &mut Budget<'_>) -> R,
) -> Result<R, E> {
    scoped(budget, |s| {
        let packet = decode::decode(bytes, s)?;
        s.reserve(size_of::<Vec<NativeConditionalSourceRootV2<'_>>>())?;
        let mut roots = s.vector(packet.roots.len())?;
        for root in &packet.roots {
            s.budget.charge_work(1)?;
            roots.push(root.view());
        }
        let account = s.budget.work_ledger_identity_v1();
        let floor = s.budget.storage();
        let result = consume(
            NativeConditionalSourcePacketInputV2 {
                semantic_mir: packet.semantic,
                native_module: packet.native,
                canonical_kernel_order: &packet.order,
                roots: &roots,
            },
            s.budget,
        );
        drop(roots);
        drop(packet);
        if let Err(error) = intact(s.budget, account, floor) {
            drop(result);
            return Err(error);
        }
        Ok(result)
    })
}

fn require(ok: bool, reason: &'static str) -> Result<(), E> {
    if ok { Ok(()) } else { Err(E::Invalid(reason)) }
}
fn add(a: usize, b: usize) -> Result<usize, E> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn mul(a: usize, b: usize) -> Result<usize, E> {
    a.checked_mul(b).ok_or(Resource::Arithmetic.into())
}
fn intact(budget: &Budget<'_>, account: Account, floor: usize) -> Result<(), E> {
    if budget.work_ledger_identity_v1() != account || budget.storage() < floor {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

struct Scope<'b, 'w> {
    budget: &'b mut Budget<'w>,
    reserved: &'b mut usize,
}
impl Scope<'_, '_> {
    fn reserve(&mut self, amount: usize) -> Result<(), E> {
        let next = add(*self.reserved, amount)?;
        self.budget.reserve_storage(amount)?;
        *self.reserved = next;
        Ok(())
    }
    // Headers are prepaid by the containing owner. Reject excess capacity rather
    // than retaining an allocation beyond its prepayment; no ZSTs are used here.
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, E> {
        if size_of::<T>() == 0 {
            return Err(Resource::Allocation.into());
        }
        let payload = mul(count, size_of::<T>())?;
        self.budget.charge_work(add(payload, 1)?)?;
        self.reserve(payload)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        if rows.capacity() != count {
            return Err(Resource::Allocation.into());
        }
        Ok(rows)
    }
}

fn check_order(order: &[u32], count: usize, s: &mut Scope<'_, '_>) -> Result<(), E> {
    s.budget.charge_work(2)?;
    require(
        (1..=MAX_ROOTS).contains(&count) && order.len() == count,
        "root/order count",
    )?;
    let mut seen = s.vector::<u8>(count)?;
    seen.resize(count, 0);
    for &ordinal in order {
        s.budget.charge_work(2)?;
        let slot = seen
            .get_mut(ordinal as usize)
            .ok_or(E::Invalid("order index"))?;
        require(*slot == 0, "duplicate order index")?;
        *slot = 1;
    }
    Ok(())
}

fn scoped<R>(
    budget: &mut Budget<'_>,
    operation: impl FnOnce(&mut Scope<'_, '_>) -> Result<R, E>,
) -> Result<R, E> {
    let account = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let mut reserved = 0;
    let result = catch_unwind(AssertUnwindSafe(|| {
        operation(&mut Scope {
            budget,
            reserved: &mut reserved,
        })
    }));
    // Operation-local owners have dropped on rejection/unwind. Encoder output
    // transfers unreserved; decoder callback-owned reservations remain paid.
    let cleanup = add(floor, reserved).and_then(|protected| intact(budget, account, protected));
    if let Err(error) = cleanup {
        match result {
            Ok(result) => {
                drop(result);
                return Err(error);
            }
            Err(panic) => resume_unwind(panic),
        }
    }
    if let Err(error) = budget.release_storage(reserved) {
        match result {
            Ok(result) => {
                drop(result);
                return Err(error.into());
            }
            Err(panic) => resume_unwind(panic),
        }
    }
    match result {
        Ok(result) => result,
        Err(panic) => resume_unwind(panic),
    }
}
