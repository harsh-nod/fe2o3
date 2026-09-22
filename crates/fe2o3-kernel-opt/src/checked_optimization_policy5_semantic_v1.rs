//! Borrowed Policy5 semantic composition. The existing P4 wire and P5 record
//! are unchanged; no new framing or execution-witness constructor is provided.

use crate::{
    CanonicalPolicy4ExecutionReceiptErrorV1,
    CheckedCanonicalKernelIrOwnerPolicy5V1 as CheckedOwner,
    CheckedCanonicalPolicy4ExecutionReceiptV1, POLICY5_EXECUTION_RECORD_BYTES_V1,
    ReplayedPolicy4SemanticRelationV1, checked_optimization_policy5_v1::policy5_record_v1,
    decode_and_check_canonical_policy4_execution_receipt_v1,
    decode_and_check_published_policy4_semantic_relation_v1,
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

/// Failure of borrowed semantic composition or authentication against an actual
/// sealed owner. Neither successful result grants publication authority.
#[derive(Debug)]
pub enum CanonicalPolicy5SemanticErrorV1 {
    Header,
    ExecutionClaim,
    ExecutionWitness,
    Policy4(CanonicalPolicy4ExecutionReceiptErrorV1),
    Forwarding(CanonicalKirLoadForwardingErrorV1),
    Resource(Resource),
    Panicked,
}
type Error = CanonicalPolicy5SemanticErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy5 semantic composition: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Policy4(error) => Some(error),
            Self::Forwarding(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Header | Self::ExecutionClaim | Self::ExecutionWitness | Self::Panicked => None,
        }
    }
}

/// Claimed endpoint roles and borrowed existing records, not authenticated
/// execution. Equal-byte independently admitted owners are permitted. Each
/// actual supplied owner remains borrowed by the returned semantic relation.
#[derive(Clone, Copy)]
pub struct CanonicalPolicy5SemanticInputsV1<'a> {
    pub input: &'a Owner,
    pub intermediate: &'a Owner,
    pub stored: &'a Owner,
    pub output: &'a Owner,
    pub policy4_wire: &'a [u8],
    pub policy5_record: &'a [u8],
    pub load_rows: &'a [Row],
}

/// New retained wrapper plus the unchanged P4 receipt's owned decoded rows.
/// Borrowed graphs, wire, P5 record and P5 rows are excluded: their caller-owned
/// backing must remain live and be accounted once, or explicitly external.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy5SemanticStorageV1(usize);
impl CanonicalPolicy5SemanticStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Independently replayed B/C/S/O relations, without execution authentication,
/// source/formal/target proof, or protected publication authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedPolicy5SemanticRelationV1;
/// fn clone(receipt: ReplayedPolicy5SemanticRelationV1<'_>) { let _ = receipt.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::{CanonicalPolicy5SemanticInputsV1 as Inputs,
///     ReplayedPolicy5SemanticRelationV1 as Receipt,
///     check_published_policy5_semantic_relation_v1 as check};
/// fn escape<'a>(inputs: Inputs<'a>, output: Owner, budget: &mut Budget<'_>) -> Receipt<'a> {
///     check(Inputs { output: &output, ..inputs }, budget).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// use fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1 as Row;
/// use fe2o3_kernel_opt::{CanonicalPolicy5SemanticInputsV1 as Inputs,
///     ReplayedPolicy5SemanticRelationV1 as Receipt,
///     check_published_policy5_semantic_relation_v1 as check};
/// fn escape_rows<'a>(inputs: Inputs<'a>, rows: Vec<Row>, budget: &mut Budget<'_>) -> Receipt<'a> {
///     check(Inputs { load_rows: &rows, ..inputs }, budget).unwrap()
/// }
/// ```
pub struct ReplayedPolicy5SemanticRelationV1<'a> {
    policy4: ReplayedPolicy4SemanticRelationV1<'a, 'a, 'a, 'a>,
    output: &'a Owner,
    record: &'a [u8],
    rows: &'a [Row],
    storage: CanonicalPolicy5SemanticStorageV1,
}
impl<'a> ReplayedPolicy5SemanticRelationV1<'a> {
    pub const fn policy4_relation(&self) -> &ReplayedPolicy4SemanticRelationV1<'a, 'a, 'a, 'a> {
        &self.policy4
    }
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    pub const fn unauthenticated_execution_record(&self) -> &'a [u8] {
        self.record
    }
    pub const fn load_forwarding_rows(&self) -> &'a [Row] {
        self.rows
    }
    pub const fn storage(&self) -> CanonicalPolicy5SemanticStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Replay plus exact P4/P5 authentication against the retained sealed owner.
/// This does not synthesize a witness from records or qualify source/proofs.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_kernel_opt::check_canonical_policy5_execution_relation_v1 as check;
/// fn not_execution(b: &Owner, o: &Owner, budget: &mut Budget<'_>) {
///     let _ = check(b, o, &[], &[], &[], budget);
/// }
/// ```
pub struct CheckedCanonicalPolicy5ExecutionRelationV1<'a> {
    policy4: CheckedCanonicalPolicy4ExecutionReceiptV1<'a, 'a, 'a>,
    checked: &'a CheckedOwner,
    record: &'a [u8],
    rows: &'a [Row],
    storage: CanonicalPolicy5SemanticStorageV1,
}
impl<'a> CheckedCanonicalPolicy5ExecutionRelationV1<'a> {
    pub const fn policy4_execution(
        &self,
    ) -> &CheckedCanonicalPolicy4ExecutionReceiptV1<'a, 'a, 'a> {
        &self.policy4
    }
    pub const fn execution_owner(&self) -> &'a CheckedOwner {
        self.checked
    }
    pub const fn output(&self) -> &'a Owner {
        self.checked.owner()
    }
    pub const fn authenticated_execution_record(&self) -> &'a [u8] {
        self.record
    }
    pub const fn load_forwarding_rows(&self) -> &'a [Row] {
        self.rows
    }
    pub const fn storage(&self) -> CanonicalPolicy5SemanticStorageV1 {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        true
    }
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
    let mut panic_payload = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            panic_payload = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if budget.work_ledger_identity_v1() != ledger {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    match cleanup {
        Ok(()) => {
            drop(panic_payload);
            result
        }
        Err(error) => {
            // Do not overwrite an earlier payload or release a foreign ledger.
            let drop_payload = catch_unwind(AssertUnwindSafe(|| drop(result))).err();
            drop(panic_payload);
            drop(drop_payload);
            Err(error.into())
        }
    }
}

fn header(record: &[u8], budget: &mut Budget<'_>) -> Result<(), Error> {
    budget.charge_work(1)?;
    if record.len() != POLICY5_EXECUTION_RECORD_BYTES_V1 {
        return Err(Error::Header);
    }
    Ok(())
}

fn continuation(
    inputs: CanonicalPolicy5SemanticInputsV1<'_>,
    store_count: usize,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(POLICY5_EXECUTION_RECORD_BYTES_V1 * 2 + 4)?;
    budget.reserve_storage(POLICY5_EXECUTION_RECORD_BYTES_V1)?;
    let matches = {
        let expected = policy5_record_v1(
            inputs.input,
            inputs.intermediate,
            inputs.stored,
            inputs.output,
            store_count,
            inputs.load_rows.len(),
        )?;
        inputs.policy5_record == expected.as_slice()
    };
    budget.release_storage(POLICY5_EXECUTION_RECORD_BYTES_V1)?;
    if !matches {
        return Err(Error::ExecutionClaim);
    }
    let storage = {
        let (_relation, storage) = check_canonical_kir_load_forwarding_v1(
            inputs.stored,
            inputs.output,
            inputs.load_rows,
            budget,
        )
        .map_err(Error::Forwarding)?;
        budget.reserve_storage(storage.retained_storage())?;
        storage
    };
    budget.release_storage(storage.retained_storage())?;
    Ok(())
}

/// Check the unchanged P4 wire (including all nested syntax/identities), then
/// independently replay S/O with the existing typed load-forwarding checker.
/// No optimizer is run. All scratch and returned metadata use this one live
/// Work ledger. The incoming storage floor is restored on success/error/panic;
/// success returns an unreserved receipt to be reserved before further work.
pub fn check_published_policy5_semantic_relation_v1<'a>(
    inputs: CanonicalPolicy5SemanticInputsV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<ReplayedPolicy5SemanticRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        header(inputs.policy5_record, budget)?;
        budget.charge_work(2)?;
        let wrapper = size_of::<ReplayedPolicy5SemanticRelationV1<'_>>()
            .checked_sub(size_of::<ReplayedPolicy4SemanticRelationV1<'_, '_, '_, '_>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let policy4 = decode_and_check_published_policy4_semantic_relation_v1(
            inputs.input,
            inputs.intermediate,
            inputs.stored,
            inputs.policy4_wire,
            budget,
        )
        .map_err(Error::Policy4)?;
        let retained = policy4.storage().retained_storage();
        budget.reserve_storage(retained)?;
        continuation(inputs, policy4.forwarding_rows().len(), budget)?;
        Ok(ReplayedPolicy5SemanticRelationV1 {
            policy4,
            output: inputs.output,
            record: inputs.policy5_record,
            rows: inputs.load_rows,
            storage: CanonicalPolicy5SemanticStorageV1(
                wrapper.checked_add(retained).ok_or(Resource::Arithmetic)?,
            ),
        })
    })
}

/// Authenticate the exact P4 and P5 execution claims against a real sealed P5
/// owner, without rerunning optimization or decoding P4 twice. The complete
/// owner and all input backing remain caller-owned and borrowed. Storage and
/// cumulative-work contracts match the semantic-only entry point.
pub fn check_canonical_policy5_execution_relation_v1<'a>(
    input: &'a Owner,
    checked: &'a CheckedOwner,
    policy4_wire: &'a [u8],
    policy5_record: &'a [u8],
    load_rows: &'a [Row],
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalPolicy5ExecutionRelationV1<'a>, Error> {
    scoped(budget, |budget| {
        header(policy5_record, budget)?;
        budget.charge_work(2)?;
        let wrapper = size_of::<CheckedCanonicalPolicy5ExecutionRelationV1<'_>>()
            .checked_sub(size_of::<
                CheckedCanonicalPolicy4ExecutionReceiptV1<'_, '_, '_>,
            >())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let prefix = checked.intermediate_policy4();
        let policy4 = decode_and_check_canonical_policy4_execution_receipt_v1(
            input,
            prefix,
            policy4_wire,
            budget,
        )
        .map_err(Error::Policy4)?;
        let retained = policy4.storage().retained_storage();
        budget.reserve_storage(retained)?;
        continuation(
            CanonicalPolicy5SemanticInputsV1 {
                input,
                intermediate: prefix.intermediate_policy3().owner(),
                stored: prefix.owner(),
                output: checked.owner(),
                policy4_wire,
                policy5_record,
                load_rows,
            },
            policy4.semantic_relation().forwarding_rows().len(),
            budget,
        )?;
        budget.charge_work(
            load_rows
                .len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(POLICY5_EXECUTION_RECORD_BYTES_V1 + 1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if policy5_record != checked.execution().canonical_bytes()
            || load_rows != checked.load_forwarding_rows()
        {
            return Err(Error::ExecutionWitness);
        }
        Ok(CheckedCanonicalPolicy5ExecutionRelationV1 {
            policy4,
            checked,
            record: policy5_record,
            rows: load_rows,
            storage: CanonicalPolicy5SemanticStorageV1(
                wrapper.checked_add(retained).ok_or(Resource::Arithmetic)?,
            ),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy5_semantic_v1_tests.rs"]
mod tests;
