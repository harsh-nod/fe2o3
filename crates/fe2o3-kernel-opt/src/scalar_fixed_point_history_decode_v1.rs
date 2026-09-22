//! Fresh graph admission and independent semantic replay, never pass execution.
use crate::{
    CheckedCanonicalOptimizationReceiptV1 as IntegerRelation,
    ReplayedPolicy3SemanticRelationV1 as ScalarRelation,
    decode_and_check_canonical_optimization_receipt_v1,
    decode_and_check_published_policy3_semantic_relation_v1,
    private_cell_promotion_resources_v1 as resources,
    scalar_fixed_point_history_v1::{
        Error, InertScalarFixedPointHistoryRefV1 as Frame, Meter, Result,
        ScalarFixedPointHistoryStorageV1 as Storage, add, configured, same,
    },
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirTransitionCandidateV1 as Candidate, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    UnauthenticatedIntegerContinuationClaimV1 as Claim,
    read_unauthenticated_integer_continuation_claim_v1,
};
use std::mem::size_of;

pub struct DecodedScalarFixedPointRoundV1 {
    integer: Owner,
    output: Owner,
}
impl DecodedScalarFixedPointRoundV1 {
    pub const fn integer_output(&self) -> &Owner {
        &self.integer
    }
    pub const fn output(&self) -> &Owner {
        &self.output
    }
}
/// Every role is separately admitted, including equal-byte terminal roles.
/// Rows/claims are checked only by check_semantics; no optimizer owner is made.
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedScalarFixedPointHistoryV1;
/// fn duplicate(v: DecodedScalarFixedPointHistoryV1<'_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedScalarFixedPointHistoryV1, CheckedScalarFixedPointOwnerV1};
/// fn witness(v: DecodedScalarFixedPointHistoryV1<'_, '_>) -> CheckedScalarFixedPointOwnerV1 { v.into() }
/// ```
pub struct DecodedScalarFixedPointHistoryV1<'f, 'w> {
    frame: &'f Frame<'w>,
    rounds: Vec<DecodedScalarFixedPointRoundV1>,
    storage: Storage,
}
impl<'f, 'w> DecodedScalarFixedPointHistoryV1<'f, 'w> {
    pub const fn frame(&self) -> &'f Frame<'w> {
        self.frame
    }
    pub fn rounds(&self) -> &[DecodedScalarFixedPointRoundV1] {
        &self.rounds
    }
    pub fn output(&self) -> &Owner {
        self.rounds
            .last()
            .expect("nonempty admitted frame")
            .output()
    }
    pub const fn storage(&self) -> Storage {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }

    /// Complete actual-pair checks. Wire, all graph roles, the original input
    /// and sibling backing must remain prepaid. Returned receipt is unreserved.
    pub fn check_semantics<'a>(
        &'a self,
        input: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<ReplayedScalarFixedPointHistoryV1<'a, 'f, 'w>> {
        configured(budget)?;
        let required = add(
            add(self.storage.0, self.frame.storage().0)?,
            add(
                self.frame.canonical_bytes().len(),
                input.canonical().canonical_bytes().len(),
            )?,
        )?;
        if budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        let inherited = budget.storage();
        resources::scoped(budget, |meter| {
            let mut retained = size_of::<ReplayedScalarFixedPointHistoryV1<'_, '_, '_>>();
            meter.reserve(retained)?;
            meter.reserve(CHECK_SCRATCH)?;
            meter.work(4)?;
            let count = self.rounds.len();
            if count != self.frame.rounds().len()
                || count == 0
                || count > crate::SCALAR_FIXED_POINT_MAX_ROUNDS_V1
            {
                return Err(Error::Composition);
            }
            meter.work(crate::SCALAR_FIXED_POINT_EXECUTION_BYTES_V1 * 2)?;
            let expected = crate::checked_scalar_fixed_point_v1::execution_header(
                input,
                self.output(),
                count,
            )?;
            if !same(&expected, self.frame.execution_claim(), meter)? {
                return Err(Error::Composition);
            }
            let (mut rounds, slots) =
                meter.table::<ReplayedScalarFixedPointRoundV1<'_, '_>>(count)?;
            retained = add(retained, slots)?;
            let mut before = input;
            for (ordinal, (round, wire)) in self.rounds.iter().zip(self.frame.rounds()).enumerate()
            {
                meter.work(1)?;
                let claim = meter.derive(|b| {
                    read_unauthenticated_integer_continuation_claim_v1(
                        before,
                        &round.integer,
                        wire.integer_record(),
                        b,
                    )
                    .map_err(Error::IntegerClaim)
                })?;
                let integer = meter.derive(|b| {
                    decode_and_check_canonical_optimization_receipt_v1(
                        before,
                        &round.integer,
                        wire.integer_transition(),
                        b,
                    )
                    .map_err(Error::Semantic)
                })?;
                let paid = integer.storage().retained_storage();
                meter.reserve(paid)?;
                retained = add(retained, paid)?;
                let scalar = meter.derive(|b| {
                    decode_and_check_published_policy3_semantic_relation_v1(
                        &round.integer,
                        &round.output,
                        wire.policy3_receipt(),
                        b,
                    )
                    .map_err(Error::Policy3)
                })?;
                let paid = scalar.storage().retained_storage();
                meter.reserve(paid)?;
                retained = add(retained, paid)?;
                let terminal = same(
                    before.canonical().canonical_bytes(),
                    round.output.canonical().canonical_bytes(),
                    meter,
                )?;
                if terminal != (ordinal + 1 == count) {
                    return Err(Error::Terminal { ordinal });
                }
                meter.push(
                    &mut rounds,
                    ReplayedScalarFixedPointRoundV1 {
                        integer,
                        scalar,
                        claim,
                    },
                )?;
                before = &round.output;
            }
            meter.work(1)?;
            Ok(ReplayedScalarFixedPointHistoryV1 {
                decoded: self,
                input,
                rounds,
                inherited,
                storage: Storage(retained),
            })
        })
    }
}
pub(super) const CHECK_SCRATCH: usize =
    crate::SCALAR_FIXED_POINT_EXECUTION_BYTES_V1 + size_of::<[usize; 3]>() + size_of::<bool>();

/// Semantic receipts retain complete adjacent occurrence rows, not Pliron maps
/// or observed execution witnesses. Report/map claims remain unauthenticated.
pub struct ReplayedScalarFixedPointRoundV1<'a, 'w> {
    integer: IntegerRelation<'a, 'a>,
    scalar: ScalarRelation<'a, 'a, 'w>,
    claim: Claim<'w>,
}
impl<'a, 'w> ReplayedScalarFixedPointRoundV1<'a, 'w> {
    pub fn input(&self) -> &'a Owner {
        self.integer.input()
    }
    pub fn integer_output(&self) -> &'a Owner {
        self.integer.output()
    }
    pub fn output(&self) -> &'a Owner {
        self.scalar.semantic_receipt().output()
    }
    pub fn integer_rows(&self) -> Candidate<'_> {
        self.integer.receipt().candidate()
    }
    pub fn scalar_rows(&self) -> Candidate<'_> {
        self.scalar.semantic_receipt().receipt().candidate()
    }
    pub const fn integer_claim(&self) -> &Claim<'w> {
        &self.claim
    }
    pub const fn scalar_relation(&self) -> &ScalarRelation<'a, 'a, 'w> {
        &self.scalar
    }
    fn retained(&self) -> Result<usize> {
        add(
            self.integer.storage().retained_storage(),
            self.scalar.storage().retained_storage(),
        )
    }
}
/// Inert complete semantics and exact unchanged terminal-round bytes. This
/// does not establish that the optimizer executed or exhausted its rewrite
/// rules. Observed execution, source/proof/native authority remain separate.
/// ```compile_fail
/// use fe2o3_kernel_opt::ReplayedScalarFixedPointHistoryV1;
/// fn forge<'a>() -> ReplayedScalarFixedPointHistoryV1<'a, 'a, 'a> { Default::default() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedScalarFixedPointHistoryV1 as D, ReplayedScalarFixedPointHistoryV1 as R};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O, CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// fn escape<'f, 'w>(v: D<'f, 'w>, input: &O, b: &mut B<'_>) -> R<'static, 'f, 'w> { v.check_semantics(input, b).unwrap() }
/// ```
pub struct ReplayedScalarFixedPointHistoryV1<'a, 'f, 'w> {
    decoded: &'a DecodedScalarFixedPointHistoryV1<'f, 'w>,
    input: &'a Owner,
    rounds: Vec<ReplayedScalarFixedPointRoundV1<'a, 'w>>,
    inherited: usize,
    storage: Storage,
}
impl<'a, 'f, 'w> ReplayedScalarFixedPointHistoryV1<'a, 'f, 'w> {
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    pub fn output(&self) -> &'a Owner {
        self.decoded.output()
    }
    pub fn rounds(&self) -> &[ReplayedScalarFixedPointRoundV1<'a, 'w>] {
        &self.rounds
    }
    pub const fn decoded(&self) -> &'a DecodedScalarFixedPointHistoryV1<'f, 'w> {
        self.decoded
    }
    pub const fn storage(&self) -> Storage {
        self.storage
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Fresh immutable replay under the original caller's still-paid backing.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        configured(budget)?;
        if budget.storage() < add(self.inherited, self.storage.0)? {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(add(self.rounds.len(), 1)?)?;
            let mut expected = add(
                size_of::<Self>(),
                self.rounds
                    .capacity()
                    .checked_mul(size_of::<ReplayedScalarFixedPointRoundV1<'_, '_>>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            for round in &self.rounds {
                expected = add(expected, round.retained()?)?;
            }
            if expected != self.storage.0 {
                return Err(Resource::Accounting.into());
            }
            let fresh = meter.derive(|b| self.decoded.check_semantics(self.input, b))?;
            let paid = fresh.storage.0;
            meter.reserve(paid)?;
            meter.work(1)?;
            drop(fresh);
            meter.release(paid)?;
            Ok(())
        })
    }
}
fn admit(
    bytes: &[u8],
    ordinal: usize,
    integer: bool,
    meter: &mut Meter<'_, '_>,
) -> Result<(Owner, usize)> {
    let (owner, receipt) = meter.derive(|b| {
        Owner::from_canonical_bytes_with_verification_budget_v12(bytes, b).map_err(|error| {
            Error::Admission {
                ordinal,
                integer,
                error,
            }
        })
    })?;
    let paid = receipt.retained_storage();
    meter.reserve(paid)?;
    Ok((owner, paid))
}
/// Fresh graph-only materialization. Equal bytes never alias graph role owners.
/// Complete candidate rows/claims are independently decoded by check_semantics.
/// Full returned header, graph receipts and actual Vec capacity are unreserved.
pub fn materialize_scalar_fixed_point_history_v1<'f, 'w>(
    frame: &'f Frame<'w>,
    budget: &mut Budget<'_>,
) -> Result<DecodedScalarFixedPointHistoryV1<'f, 'w>> {
    configured(budget)?;
    if budget.storage() < add(frame.storage().0, frame.canonical_bytes().len())? {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        let mut retained = size_of::<DecodedScalarFixedPointHistoryV1<'_, '_>>();
        meter.reserve(retained)?;
        let (mut rounds, slots) =
            meter.table::<DecodedScalarFixedPointRoundV1>(frame.rounds().len())?;
        retained = add(retained, slots)?;
        for (ordinal, round) in frame.rounds().iter().enumerate() {
            meter.work(1)?;
            let (integer, paid) = admit(round.integer_output_bytes(), ordinal, true, meter)?;
            retained = add(retained, paid)?;
            let (output, paid) = admit(round.output_bytes(), ordinal, false, meter)?;
            retained = add(retained, paid)?;
            meter.push(
                &mut rounds,
                DecodedScalarFixedPointRoundV1 { integer, output },
            )?;
        }
        meter.work(1)?;
        Ok(DecodedScalarFixedPointHistoryV1 {
            frame,
            rounds,
            storage: Storage(retained),
        })
    })
}

#[cfg(test)]
#[path = "scalar_fixed_point_history_decode_v1_tests.rs"]
mod tests;
