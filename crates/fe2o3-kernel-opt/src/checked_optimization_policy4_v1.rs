//! Fixed Policy4 is unchanged Policy3 B/C followed by checked Store forwarding
//! C/O. The intermediate occurrence map never describes the final output.

use crate::{
    CanonicalPolicy3ExecutionReceiptErrorV1, CheckedStoreForwardingErrorV1,
    CheckedStoreForwardingOutputV1, KernelIrCheckedOptimizationErrorV1,
    decode_and_check_canonical_policy3_execution_receipt_v1,
    encode_checked_canonical_policy3_execution_receipt_v1,
    optimize_checked_canonical_kernel_ir_policy3_v1, optimize_checked_store_forwarding_v1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirStoreForwardingErrorV1, CanonicalKirStoreForwardingRowV1 as Row,
    check_canonical_kir_store_forwarding_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1 as Intermediate;
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Fixed composition record: framing/profile/count and three typed identities.
pub const POLICY4_EXECUTION_RECORD_BYTES_V1: usize = 152;

#[derive(Debug)]
pub enum CanonicalPolicy4OptimizationErrorV1 {
    Resource(Resource),
    Policy3(KernelIrCheckedOptimizationErrorV1),
    Policy3Receipt(CanonicalPolicy3ExecutionReceiptErrorV1),
    Forwarding(CheckedStoreForwardingErrorV1),
    Relation(CanonicalKirStoreForwardingErrorV1),
    InputHistory,
    Panicked,
}
impl From<Resource> for CanonicalPolicy4OptimizationErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalPolicy4OptimizationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked Policy4 composition: {self:?}")
    }
}
impl std::error::Error for CanonicalPolicy4OptimizationErrorV1 {}
type Error = CanonicalPolicy4OptimizationErrorV1;

/// Sealed evidence that the two fixed stages actually ran. Decoders cannot
/// construct this type. The retained Policy3 owner supplies its eight-pass
/// execution record; this record binds the additional fixed forwarding stage.
pub struct Policy4ExecutionWitnessV1 {
    bytes: [u8; POLICY4_EXECUTION_RECORD_BYTES_V1],
}
impl Policy4ExecutionWitnessV1 {
    /// Version of this sealed fixed composition, not of its Policy3 prefix.
    pub const fn policy_version(&self) -> u16 {
        4
    }

    /// Publishing these bytes does not authenticate their execution provenance.
    pub const fn canonical_bytes(&self) -> &[u8; POLICY4_EXECUTION_RECORD_BYTES_V1] {
        &self.bytes
    }
}

/// Move-only checked composition, with no self-reference and no raw-parts
/// constructor. C and O remain immutable and independently replayable.
/// No source, formal, target, launch, or default-pipeline authority is granted.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1;
/// fn duplicate(owner: CheckedCanonicalKernelIrOwnerPolicy4V1) { let _ = owner.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn relabel(owner: CheckedCanonicalKernelIrOwnerPolicy4V1) -> CheckedNeutralKernelIrOwnerPolicy3V1 {
///     owner
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1;
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn escape(owner: CheckedCanonicalKernelIrOwnerPolicy4V1) -> &'static VerifiedCanonicalKernelIrModuleV12 {
///     owner.owner()
/// }
/// ```
pub struct CheckedCanonicalKernelIrOwnerPolicy4V1 {
    intermediate: Intermediate,
    output: Owner,
    rows: Vec<Row>,
    execution: Policy4ExecutionWitnessV1,
    retained: usize,
}
impl CheckedCanonicalKernelIrOwnerPolicy4V1 {
    /// The actual fixed-Policy3 C, including B/C history and occurrence rows.
    pub const fn intermediate_policy3(&self) -> &Intermediate {
        &self.intermediate
    }
    /// The actual freshly verified final O, not C with another identity.
    pub const fn owner(&self) -> &Owner {
        &self.output
    }
    /// Inert C/O claims, meaningful only with both exact immutable endpoints.
    pub fn forwarding_rows(&self) -> &[Row] {
        &self.rows
    }
    pub const fn execution(&self) -> &Policy4ExecutionWitnessV1 {
        &self.execution
    }
    pub fn native_input_audit_bytes(&self) -> &[u8] {
        self.intermediate.native_input_audit_bytes()
    }
    /// Complete owner header, both owner payloads, history and actual row capacity.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Independently replays B/C and C/O. Caller retains B and this owner on the
    /// same ledger. No occurrence row from B/C is reused as an O occurrence.
    pub fn replay(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<(), Error> {
        scoped(budget, |budget| {
            let wire = encode_checked_canonical_policy3_execution_receipt_v1(
                input,
                &self.intermediate,
                budget,
            )
            .map_err(Error::Policy3Receipt)?;
            let wire_storage = wire.storage().retained_storage();
            budget.reserve_storage(wire_storage)?;
            let semantic = decode_and_check_canonical_policy3_execution_receipt_v1(
                input,
                &self.intermediate,
                wire.canonical_bytes(),
                budget,
            )
            .map_err(Error::Policy3Receipt)?;
            budget.reserve_storage(semantic.storage().retained_storage())?;
            let semantic_storage = semantic.storage().retained_storage();
            drop(semantic);
            budget.release_storage(semantic_storage)?;
            drop(wire);
            budget.release_storage(wire_storage)?;
            let storage = {
                let (_checked, storage) = check_canonical_kir_store_forwarding_v1(
                    self.intermediate.owner(),
                    &self.output,
                    &self.rows,
                    budget,
                )
                .map_err(Error::Relation)?;
                budget.reserve_storage(storage.retained_storage())?;
                storage
            };
            budget.release_storage(storage.retained_storage())?;
            Ok(())
        })
    }
}

pub(crate) fn policy4_record_v1(
    input: &Owner,
    intermediate: &Owner,
    output: &Owner,
    count: usize,
) -> Result<[u8; POLICY4_EXECUTION_RECORD_BYTES_V1], Resource> {
    let mut bytes = [0; POLICY4_EXECUTION_RECORD_BYTES_V1];
    bytes[..8].copy_from_slice(b"F2P4EX1\0");
    bytes[8..16].copy_from_slice(&[1, 0, 4, 0, 3, 0, 1, 0]);
    // Closed profile 1: exact existing Policy3 then the V1 forwarding service.
    bytes[16..24].copy_from_slice(&1u64.to_le_bytes());
    bytes[24..32].copy_from_slice(
        &u64::try_from(count)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    for (offset, owner) in [(32, input), (72, intermediate), (112, output)] {
        let identity = owner.canonical().identity();
        bytes[offset..offset + 32].copy_from_slice(identity.digest());
        bytes[offset + 32..offset + 40].copy_from_slice(&identity.canonical_length().to_le_bytes());
    }
    Ok(bytes)
}

fn scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result
}

/// Runs the exact B -> fixed Policy3 C -> checked forwarding O composition.
/// B is caller-reserved; success transfers `retained_storage()` and restores
/// the entry floor. One budget covers all coexisting stages. No pass is appended
/// to Policy3 and its bytes, roster, or execution identity are not changed.
pub fn optimize_checked_canonical_kernel_ir_policy4_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedCanonicalKernelIrOwnerPolicy4V1, Error> {
    scoped(budget, |budget| {
        budget.charge_work(3)?;
        let wrapper = size_of::<CheckedCanonicalKernelIrOwnerPolicy4V1>()
            .checked_sub(size_of::<Intermediate>())
            .and_then(|n| n.checked_sub(size_of::<CheckedStoreForwardingOutputV1<'_>>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let intermediate = optimize_checked_canonical_kernel_ir_policy3_v1(input, budget)
            .map_err(Error::Policy3)?;
        let intermediate_storage = intermediate.storage().retained_storage();
        budget.reserve_storage(intermediate_storage)?;
        budget.charge_work(2)?;
        let history = intermediate.native_input_audit_bytes();
        budget.charge_work(
            input
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(history.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if input.canonical().canonical_bytes() != history {
            return Err(Error::InputHistory);
        }
        let forwarded = optimize_checked_store_forwarding_v1(intermediate.owner(), budget)
            .map_err(Error::Forwarding)?;
        budget.reserve_storage(forwarded.retained_storage())?;
        budget.charge_work(POLICY4_EXECUTION_RECORD_BYTES_V1 + 4)?;
        let execution = Policy4ExecutionWitnessV1 {
            bytes: policy4_record_v1(
                input,
                intermediate.owner(),
                forwarded.output(),
                forwarded.rows().len(),
            )?,
        };
        let (output, rows, forwarded_storage) = forwarded.into_owned_parts();
        let retained = wrapper
            .checked_add(intermediate_storage)
            .and_then(|n| n.checked_add(forwarded_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok(CheckedCanonicalKernelIrOwnerPolicy4V1 {
            intermediate,
            output,
            rows,
            execution,
            retained,
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy4_v1_tests.rs"]
pub(crate) mod tests;
