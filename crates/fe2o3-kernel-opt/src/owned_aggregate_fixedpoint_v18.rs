//! Policy12 composes actual checked scalar rounds and aggregate/SSA mutation.
use crate::{
    OwnedAggregateSsaContinuationV18 as Aggregate, OwnedAggregateSsaErrorV18,
    prepare_owned_aggregate_ssa_v18, private_cell_promotion_resources_v1 as resources,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV18 as Inventory,
    CanonicalKirTransitionErrorV1, check_canonical_kir_transition_v18,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrIdentityV18 as Identity, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerMixedFixedpointV18 as Scalar, KirCheckedNeutralOptimizationErrorV1,
    KirNeutralOptimizationErrorV18, KirOptimizationMapErrorV12,
    optimize_neutral_kernel_ir_mixed_fixedpoint_v18,
};
use std::{fmt, mem::size_of};

#[path = "aggregate_fixedpoint_witness_v18.rs"]
mod witness;

/// Closed outer limit, separate from the unchanged inner Policy11 limit.
pub const AGGREGATE_FIXEDPOINT_MAX_ROUNDS_V18: usize = 32;

#[derive(Debug)]
pub enum OwnedAggregateFixedpointErrorV18 {
    Resource(Resource),
    Scalar(KirNeutralOptimizationErrorV18),
    Adoption(KirCheckedNeutralOptimizationErrorV1),
    Aggregate(OwnedAggregateSsaErrorV18),
    Inventory(CanonicalKirInventoryErrorV1),
    Map(KirOptimizationMapErrorV12),
    Transition(CanonicalKirTransitionErrorV1),
    Inconsistent(&'static str),
    RoundLimit { completed: usize, limit: usize },
    ForeignInput,
    Panicked,
}
type Error = OwnedAggregateFixedpointErrorV18;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
macro_rules! error_from {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for Error {
            fn from(value: $ty) -> Self {
                Self::$variant(value)
            }
        }
    };
}
error_from!(Resource, Resource);
error_from!(KirNeutralOptimizationErrorV18, Scalar);
error_from!(KirCheckedNeutralOptimizationErrorV1, Adoption);
error_from!(OwnedAggregateSsaErrorV18, Aggregate);
error_from!(CanonicalKirInventoryErrorV1, Inventory);
error_from!(KirOptimizationMapErrorV12, Map);
error_from!(CanonicalKirTransitionErrorV1, Transition);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "aggregate Policy12: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Scalar(e) => Some(e),
            Self::Adoption(e) => Some(e),
            Self::Aggregate(e) => Some(e),
            Self::Inventory(e) => Some(e),
            Self::Map(e) => Some(e),
            Self::Transition(e) => Some(e),
            _ => None,
        }
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}

/// Actual two-stage round. Both nominal owners remain retained and immutable.
pub struct AggregateFixedpointRoundV18 {
    scalar: Scalar,
    aggregate: Aggregate,
    changed: bool,
}
impl AggregateFixedpointRoundV18 {
    pub const fn scalar(&self) -> &Scalar {
        &self.scalar
    }
    pub const fn aggregate(&self) -> &Aggregate {
        &self.aggregate
    }
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

/// Move-only Policy12 custody. This is not a Policy11 owner or a source proof.
/// The caller must retain the genuine original separately for replay and source
/// composition. Historical scalar occurrence rows describe each scalar stage,
/// never the intervening aggregate memory/CFG transformation.
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedAggregateFixedpointV18;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18;
/// fn relabel(v: OwnedAggregateFixedpointV18) -> CheckedNeutralKernelIrOwnerMixedFixedpointV18 { v }
/// ```
pub struct OwnedAggregateFixedpointV18 {
    input: Identity,
    rounds: Vec<AggregateFixedpointRoundV18>,
    canonical: Vec<u8>,
    retained: usize,
}
impl OwnedAggregateFixedpointV18 {
    pub const fn policy_version(&self) -> u16 {
        12
    }
    pub const fn graph_schema(&self) -> u16 {
        18
    }
    pub const fn input_identity(&self) -> &Identity {
        &self.input
    }
    pub fn rounds(&self) -> &[AggregateFixedpointRoundV18] {
        &self.rounds
    }
    pub fn owner(&self) -> &Owner {
        self.rounds
            .last()
            .expect("closed nonempty fixed policy")
            .aggregate
            .output()
    }
    pub fn input_audit_bytes(&self) -> &[u8] {
        self.rounds
            .first()
            .expect("closed nonempty fixed policy")
            .scalar
            .input_audit_bytes()
    }
    /// Every round's full actual scalar witness and aggregate recipe, not just
    /// the terminal round or graph digests. These inert bytes grant no authority.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub fn replay_against(&self, input: &Owner, budget: &mut Budget<'_>) -> Result<()> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter
                .reserve(size_of::<(&Self, &Owner, &mut Budget<'_>)>() + size_of::<Result<()>>())?;
            meter.work(self.rounds.len())?;
            if self.retained != retained(self)? {
                return Err(Resource::Accounting.into());
            }
            replay(self, input, meter)?;
            witness::compare(self, meter)
        })
    }
}
fn retained(value: &OwnedAggregateFixedpointV18) -> Result<usize> {
    let mut total = add(
        size_of::<OwnedAggregateFixedpointV18>(),
        value.canonical.capacity(),
    )?;
    total = add(
        total,
        value
            .rounds
            .capacity()
            .checked_mul(size_of::<AggregateFixedpointRoundV18>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    for round in &value.rounds {
        total = add(
            total,
            round
                .scalar
                .storage()
                .retained_storage()
                .checked_sub(size_of::<Scalar>())
                .ok_or(Resource::Accounting)?,
        )?;
        total = add(
            total,
            round
                .aggregate
                .retained_storage()
                .checked_sub(size_of::<Aggregate>())
                .ok_or(Resource::Accounting)?,
        )?;
    }
    Ok(total)
}
fn scalar(
    input: &Owner,
    layouts: StorageLayoutLimitsV1,
    meter: &mut Meter<'_, '_>,
) -> Result<Scalar> {
    type Observed<'a> = fe2o3_pliron::KirNeutralOptimizationOutputMixedFixedpointV18<'a>;
    type Frames<'a> = (
        Observed<'a>,
        Result<Observed<'a>>,
        Result<Scalar>,
        usize,
        &'a Owner,
        StorageLayoutLimitsV1,
        &'a mut Budget<'a>,
    );
    meter.reserve(size_of::<Frames<'_>>())?;
    meter.derive(|budget| {
        let observed = optimize_neutral_kernel_ir_mixed_fixedpoint_v18(input, layouts, budget)?;
        budget.reserve_storage(observed.storage().retained_storage())?;
        // Consuming adoption owns the observation reservation and restores its
        // entry floor on refusal/success, returning an unreserved checked owner.
        Ok(observed.try_check_and_finish_v18(budget)?)
    })
}
fn changed(a: &Owner, b: &Owner, meter: &mut Meter<'_, '_>) -> Result<bool> {
    meter.work(add(a.canonical_bytes().len(), b.canonical_bytes().len())?)?;
    Ok(a.canonical_bytes() != b.canonical_bytes())
}
fn replay(
    value: &OwnedAggregateFixedpointV18,
    input: &Owner,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    type InventoryResult<'a> = Result<(
        Inventory<'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
    )>;
    type PairResult<'a> = Result<(
        fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'a, 'a, 'a>,
        fe2o3_kernel_analysis::CanonicalKirTransitionStorageV1,
    )>;
    meter.reserve(size_of::<(
        InventoryResult<'_>,
        InventoryResult<'_>,
        PairResult<'_>,
        Result<()>,
    )>())?;
    meter.work(size_of::<Identity>())?;
    if input.identity() != &value.input {
        return Err(Error::ForeignInput);
    }
    if value.rounds.is_empty() || value.rounds.len() > AGGREGATE_FIXEDPOINT_MAX_ROUNDS_V18 {
        return Err(Error::Inconsistent("closed nonempty bounded round roster"));
    }
    let mut original = input;
    for (index, round) in value.rounds.iter().enumerate() {
        meter.work(add(
            original.canonical_bytes().len(),
            round.scalar.execution().canonical_bytes().len(),
        )?)?;
        if round.scalar.input_audit_bytes() != original.canonical_bytes()
            || round.scalar.execution().policy_version() != 11
            || round.scalar.execution().graph_schema() != 18
            || !round.scalar.map().matches_execution(round.scalar.report())
        {
            return Err(Error::Inconsistent(
                "exact original scalar stage and full Policy11 report",
            ));
        }
        meter.derive(|b| {
            Ok(round
                .scalar
                .map()
                .check_against(original, round.scalar.owner(), b)?)
        })?;
        let (a, ar) = meter.derive(|b| Ok(Inventory::derive_v18(original, b)?))?;
        meter.reserve(ar.retained_storage())?;
        let (b, br) =
            meter.derive(|budget| Ok(Inventory::derive_v18(round.scalar.owner(), budget)?))?;
        meter.reserve(br.retained_storage())?;
        let (pair, receipt) = meter.derive(|budget| {
            Ok(check_canonical_kir_transition_v18(
                &a,
                &b,
                round.scalar.occurrences().candidate(),
                budget,
            )?)
        })?;
        meter.reserve(receipt.retained_storage())?;
        drop(pair);
        meter.release(receipt.retained_storage())?;
        drop(b);
        drop(a);
        meter.release(add(ar.retained_storage(), br.retained_storage())?)?;
        meter.derive(|budget| {
            Ok(round
                .aggregate
                .replay_against(round.scalar.owner(), budget)?)
        })?;
        let scalar_changed = changed(original, round.scalar.owner(), meter)?;
        let aggregate_changed = changed(round.scalar.owner(), round.aggregate.output(), meter)?;
        if aggregate_changed != (round.aggregate.promoted_allocations() > 0)
            || round.changed != (scalar_changed || aggregate_changed)
            || round.changed == (index + 1 == value.rounds.len())
        {
            return Err(Error::Inconsistent(
                "every nonterminal round changed and full terminal round unchanged",
            ));
        }
        original = round.aggregate.output();
    }
    Ok(())
}

/// Executes the closed P11-fixedpoint/aggregate-SSA roster through a complete
/// unchanged outer round. This is real checked mutation, not a caller pass list
/// or inert wrapper. Every affected analysis is rederived on its actual owner.
/// Exhaustion is a refusal with no partial output. The returned receipt is
/// unreserved and must be reserved before later controlled work.
pub fn optimize_owned_aggregate_fixedpoint_v18(
    input: &Owner,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<OwnedAggregateFixedpointV18> {
    run(input, layouts, AGGREGATE_FIXEDPOINT_MAX_ROUNDS_V18, budget)
}
fn run(
    input: &Owner,
    layouts: StorageLayoutLimitsV1,
    limit: usize,
    budget: &mut Budget<'_>,
) -> Result<OwnedAggregateFixedpointV18> {
    resources::scoped(budget, |meter| {
        type Frames<'a> = (
            &'a Owner,
            StorageLayoutLimitsV1,
            &'a mut Budget<'a>,
            Scalar,
            Aggregate,
            Result<Scalar>,
            Result<Aggregate>,
            Result<OwnedAggregateFixedpointV18>,
        );
        meter.reserve(size_of::<OwnedAggregateFixedpointV18>() + size_of::<Frames<'_>>())?;
        let (rounds, _) = meter.table(limit)?;
        let mut value = OwnedAggregateFixedpointV18 {
            input: *input.identity(),
            rounds,
            canonical: Vec::new(),
            retained: 0,
        };
        for _ in 0..limit {
            let original = value.rounds.last().map_or(input, |r| r.aggregate.output());
            let scalar = scalar(original, layouts, meter)?;
            meter.reserve(scalar.storage().retained_storage())?;
            let aggregate = meter
                .derive(|b| Ok(prepare_owned_aggregate_ssa_v18(scalar.owner(), layouts, b)?))?;
            meter.reserve(aggregate.retained_storage())?;
            let scalar_changed = changed(original, scalar.owner(), meter)?;
            let aggregate_changed = changed(scalar.owner(), aggregate.output(), meter)?;
            let changed = scalar_changed || aggregate_changed;
            meter.push(
                &mut value.rounds,
                AggregateFixedpointRoundV18 {
                    scalar,
                    aggregate,
                    changed,
                },
            )?;
            if !changed {
                replay(&value, input, meter)?;
                value.canonical = witness::encode(&value, meter)?;
                meter.work(value.rounds.len())?;
                value.retained = retained(&value)?;
                return Ok(value);
            }
        }
        Err(Error::RoundLimit {
            completed: value.rounds.len(),
            limit,
        })
    })
}

#[cfg(test)]
#[path = "aggregate_fixedpoint_v18_tests.rs"]
mod tests;
