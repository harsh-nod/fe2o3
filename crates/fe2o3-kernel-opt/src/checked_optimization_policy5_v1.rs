//! Fixed Policy5 keeps the exact Policy4 B/C/S prefix, then independently
//! checked private Load-to-Load forwarding S/O. Older schedules are unchanged.

use crate::{
    CanonicalPolicy4OptimizationErrorV1, CheckedCanonicalKernelIrOwnerPolicy4V1 as Prefix,
    CheckedLoadForwardingErrorV1, CheckedLoadForwardingOutputV1,
    optimize_checked_canonical_kernel_ir_policy4_v1, optimize_checked_load_forwarding_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirLoadForwardingErrorV1, CanonicalKirLoadForwardingRowV1 as Row,
    check_canonical_kir_load_forwarding_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Closed framing, two row counts, and all four actual B/C/S/O identities.
pub const POLICY5_EXECUTION_RECORD_BYTES_V1: usize = 200;

#[derive(Debug)]
pub enum CanonicalPolicy5OptimizationErrorV1 {
    Resource(Resource),
    Policy4(CanonicalPolicy4OptimizationErrorV1),
    Forwarding(CheckedLoadForwardingErrorV1),
    Relation(CanonicalKirLoadForwardingErrorV1),
    Execution,
    Panicked,
}
type Error = CanonicalPolicy5OptimizationErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked Policy5 composition: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Sealed execution record of this fixed composition, not a decoder-created
/// witness or protected origin. Its bytes alone grant no execution authority.
pub struct Policy5ExecutionWitnessV1 {
    bytes: [u8; POLICY5_EXECUTION_RECORD_BYTES_V1],
}
impl Policy5ExecutionWitnessV1 {
    pub const fn policy_version(&self) -> u16 {
        5
    }
    pub const fn canonical_bytes(&self) -> &[u8; POLICY5_EXECUTION_RECORD_BYTES_V1] {
        &self.bytes
    }
}

/// Retains actual B/C/S history in the move-only unchanged Policy4 prefix and
/// the new actual O/rows. No source, formal, target or default authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy5V1;
/// fn clone(owner: CheckedCanonicalKernelIrOwnerPolicy5V1) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_opt::{CheckedCanonicalKernelIrOwnerPolicy4V1, CheckedCanonicalKernelIrOwnerPolicy5V1};
/// fn relabel(owner: CheckedCanonicalKernelIrOwnerPolicy5V1) -> CheckedCanonicalKernelIrOwnerPolicy4V1 { owner }
/// ```
pub struct CheckedCanonicalKernelIrOwnerPolicy5V1 {
    prefix: Prefix,
    output: Owner,
    rows: Vec<Row>,
    execution: Policy5ExecutionWitnessV1,
    retained: usize,
}
impl CheckedCanonicalKernelIrOwnerPolicy5V1 {
    pub const fn intermediate_policy4(&self) -> &Prefix {
        &self.prefix
    }
    pub const fn owner(&self) -> &Owner {
        &self.output
    }
    pub fn load_forwarding_rows(&self) -> &[Row] {
        &self.rows
    }
    pub const fn execution(&self) -> &Policy5ExecutionWitnessV1 {
        &self.execution
    }
    pub fn native_input_audit_bytes(&self) -> &[u8] {
        self.prefix.native_input_audit_bytes()
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Replays unchanged B/C/S and independently rebuilds the actual S memory
    /// census for S/O. B and this complete owner remain caller-reserved.
    pub fn replay(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<(), Error> {
        scoped(budget, |budget| {
            self.prefix.replay(input, budget).map_err(Error::Policy4)?;
            self.replay_continuation(input, budget)
        })
    }

    /// Checks only this sealed S/O continuation and complete record subjects.
    /// This does not qualify B/C/S: production consumers must independently
    /// admit `intermediate_policy4()` before composing its source relation.
    pub fn replay_continuation(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<(), Error> {
        scoped(budget, |budget| {
            budget.charge_work(POLICY5_EXECUTION_RECORD_BYTES_V1 * 2 + 4)?;
            if self.execution.bytes != record(input, &self.prefix, &self.output, self.rows.len())? {
                return Err(Error::Execution);
            }
            let receipt = {
                let (_relation, receipt) = check_canonical_kir_load_forwarding_v1(
                    self.prefix.owner(),
                    &self.output,
                    &self.rows,
                    budget,
                )
                .map_err(Error::Relation)?;
                budget.reserve_storage(receipt.retained_storage())?;
                receipt
            };
            budget.release_storage(receipt.retained_storage())?;
            Ok(())
        })
    }
}

fn record(
    input: &Owner,
    prefix: &Prefix,
    output: &Owner,
    load_count: usize,
) -> Result<[u8; POLICY5_EXECUTION_RECORD_BYTES_V1], Resource> {
    policy5_record_v1(
        input,
        prefix.intermediate_policy3().owner(),
        prefix.owner(),
        output,
        prefix.forwarding_rows().len(),
        load_count,
    )
}

pub(crate) fn policy5_record_v1(
    input: &Owner,
    intermediate: &Owner,
    stored: &Owner,
    output: &Owner,
    store_count: usize,
    load_count: usize,
) -> Result<[u8; POLICY5_EXECUTION_RECORD_BYTES_V1], Resource> {
    let mut bytes = [0; POLICY5_EXECUTION_RECORD_BYTES_V1];
    bytes[..8].copy_from_slice(b"F2P5EX1\0");
    bytes[8..16].copy_from_slice(&[1, 0, 5, 0, 4, 0, 1, 0]);
    bytes[16..24].copy_from_slice(&1u64.to_le_bytes());
    bytes[24..32].copy_from_slice(
        &u64::try_from(store_count)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    bytes[32..40].copy_from_slice(
        &u64::try_from(load_count)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    for (offset, owner) in [
        (40, input),
        (80, intermediate),
        (120, stored),
        (160, output),
    ] {
        let identity = owner.canonical().identity();
        bytes[offset..offset + 32].copy_from_slice(identity.digest());
        bytes[offset + 32..offset + 40].copy_from_slice(&identity.canonical_length().to_le_bytes());
    }
    Ok(bytes)
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    match release {
        Ok(()) => result,
        Err(error) => {
            drop(result);
            Err(error.into())
        }
    }
}

/// Deterministic fixed B -> Policy3 C -> Policy4 S -> load-forwarded O.
/// Neither older policy nor its receipts are changed. All endpoints, analyses,
/// candidate copies, fresh replay and actual capacities share the caller budget.
/// Success returns the complete receipt and restores the incoming floor.
pub fn optimize_checked_canonical_kernel_ir_policy5_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalKernelIrOwnerPolicy5V1, Error> {
    scoped(budget, |budget| {
        budget.charge_work(3)?;
        let wrapper = size_of::<CheckedCanonicalKernelIrOwnerPolicy5V1>()
            .checked_sub(size_of::<Prefix>())
            .and_then(|n| n.checked_sub(size_of::<CheckedLoadForwardingOutputV1<'_>>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let prefix = optimize_checked_canonical_kernel_ir_policy4_v1(input, budget)
            .map_err(Error::Policy4)?;
        let prefix_storage = prefix.retained_storage();
        budget.reserve_storage(prefix_storage)?;
        let forwarded = optimize_checked_load_forwarding_v1(prefix.owner(), budget)
            .map_err(Error::Forwarding)?;
        budget.reserve_storage(forwarded.retained_storage())?;
        budget.charge_work(POLICY5_EXECUTION_RECORD_BYTES_V1 + 4)?;
        let execution = Policy5ExecutionWitnessV1 {
            bytes: record(input, &prefix, forwarded.output(), forwarded.rows().len())?,
        };
        let (output, rows, forwarding_storage) = forwarded.into_owned_parts();
        let retained = wrapper
            .checked_add(prefix_storage)
            .and_then(|n| n.checked_add(forwarding_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalKernelIrOwnerPolicy5V1 {
            prefix,
            output,
            rows,
            execution,
            retained,
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy5_v1_tests.rs"]
mod tests;
