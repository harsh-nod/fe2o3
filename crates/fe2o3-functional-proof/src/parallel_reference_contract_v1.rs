//! Canonical workload-neutral relation between sequential reference effects
//! and their parallel GPU realization.
//!
//! These values are checked encodings, not evidence. Production admission
//! independently reconstructs every fact from retained reference MIR, the
//! live ranked graph, mandatory analyses, and authenticated proof receipts.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::{SemanticEvaluationOrderV1, SemanticIeeeExceptionalValueV1, SemanticIeeeRoundingV1};

pub const HARD_MAX_PARALLEL_OUTPUT_RELATIONS_V1: usize = 4_096;
pub const HARD_MAX_PARALLEL_CALL_SUMMARIES_V1: usize = 4_096;
pub const HARD_MAX_RELATION_SUMMARIES_V1: usize = 256;
pub const HARD_MAX_PARALLEL_CALL_ARGUMENTS_V1: u16 = 256;

const CONTRACT_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE-CONTRACT/V1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParallelHierarchyLevelV1 {
    Invocation,
    Subgroup,
    Workgroup,
    Grid,
}

pub const COMPLETE_GPU_HIERARCHY_V1: [ParallelHierarchyLevelV1; 4] = [
    ParallelHierarchyLevelV1::Invocation,
    ParallelHierarchyLevelV1::Subgroup,
    ParallelHierarchyLevelV1::Workgroup,
    ParallelHierarchyLevelV1::Grid,
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParallelExecutionScopeV1 {
    SequentialReference,
    Invocation,
    Subgroup,
    Workgroup,
    Grid,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParallelFoldOrderV1 {
    Preserved,
    AlgebraicallyReordered {
        associativity_proof: DigestV1,
        commutativity_proof: DigestV1,
    },
    ErrorBoundedReordering {
        proof: DigestV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParallelScheduleRelationV1 {
    /// Total ownership supplies a bijection from logical coordinates to
    /// independent GPU write owners.
    PointwiseBijection,
    /// A live permutation collective supplies mapping and inverse witnesses.
    Permutation { collective: DigestV1 },
    /// A live finite fold supplies the contribution domain and operator.
    Fold {
        collective: DigestV1,
        order: ParallelFoldOrderV1,
        reference_order: SemanticEvaluationOrderV1,
    },
    /// A canonical live loop and recurrence collective jointly represent a
    /// bounded sequential recurrence. Dynamic bounds require an identity
    /// independently derived by the compiler's canonical-loop verifier; a
    /// declaration never supplies the bound itself.
    BoundedRecurrence {
        collective: DigestV1,
        loop_contract: DigestV1,
        dynamic_bound_proof: Option<DigestV1>,
        reference_order: SemanticEvaluationOrderV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParallelNumericalPolicyV1 {
    ExactBitVector,
    /// Exact IEEE-754 operator identity at the MIR/PLIRON boundary. This does
    /// not claim that a later target instruction implements IEEE values.
    IeeeOperatorCongruence {
        rounding: SemanticIeeeRoundingV1,
        exceptional_values: SemanticIeeeExceptionalValueV1,
    },
    ErrorBounded {
        absolute_error_f64_bits: u64,
        relative_error_f64_bits: u64,
        witness_root: DigestV1,
        proof: DigestV1,
    },
    /// Explicitly represented for a precise diagnostic, but never admitted.
    UnboundedRelaxed,
}

impl ParallelNumericalPolicyV1 {
    pub fn validate(self) -> Result<(), ParallelReferenceContractErrorV1> {
        match self {
            Self::ExactBitVector | Self::IeeeOperatorCongruence { .. } => Ok(()),
            Self::ErrorBounded {
                absolute_error_f64_bits,
                relative_error_f64_bits,
                witness_root,
                proof,
            } => {
                let absolute = f64::from_bits(absolute_error_f64_bits);
                let relative = f64::from_bits(relative_error_f64_bits);
                if witness_root.is_zero()
                    || proof.is_zero()
                    || !absolute.is_finite()
                    || !relative.is_finite()
                    || absolute < 0.0
                    || relative < 0.0
                    || (absolute == 0.0 && relative == 0.0)
                {
                    return Err(ParallelReferenceContractErrorV1::InvalidNumericalPolicy);
                }
                Ok(())
            }
            Self::UnboundedRelaxed => Err(ParallelReferenceContractErrorV1::UnboundedRelaxedPolicy),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ParallelCallKindV1 {
    SafeRustHelper,
    CompilerIntrinsic,
    CooperativeTensorIntrinsic {
        site_ordinal: u32,
        layout_identity: DigestV1,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParallelCallSummaryV1 {
    identity: DigestV1,
    callsite_identity: DigestV1,
    actual_root: DigestV1,
    reference_root: DigestV1,
    authenticated_proof: DigestV1,
    argument_count: u16,
    scope: ParallelExecutionScopeV1,
    kind: ParallelCallKindV1,
    numerical_policy: ParallelNumericalPolicyV1,
}

impl ParallelCallSummaryV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: DigestV1,
        callsite_identity: DigestV1,
        actual_root: DigestV1,
        reference_root: DigestV1,
        authenticated_proof: DigestV1,
        argument_count: u16,
        scope: ParallelExecutionScopeV1,
        kind: ParallelCallKindV1,
        numerical_policy: ParallelNumericalPolicyV1,
    ) -> Result<Self, ParallelReferenceContractErrorV1> {
        if [
            identity,
            callsite_identity,
            actual_root,
            reference_root,
            authenticated_proof,
        ]
        .into_iter()
        .any(DigestV1::is_zero)
            || argument_count > HARD_MAX_PARALLEL_CALL_ARGUMENTS_V1
            || matches!(kind, ParallelCallKindV1::CooperativeTensorIntrinsic { layout_identity, .. } if layout_identity.is_zero())
            || matches!(kind, ParallelCallKindV1::CooperativeTensorIntrinsic { .. })
                && scope != ParallelExecutionScopeV1::Subgroup
        {
            return Err(ParallelReferenceContractErrorV1::InvalidCallSummary);
        }
        numerical_policy.validate()?;
        Ok(Self {
            identity,
            callsite_identity,
            actual_root,
            reference_root,
            authenticated_proof,
            argument_count,
            scope,
            kind,
            numerical_policy,
        })
    }

    pub const fn identity(&self) -> DigestV1 {
        self.identity
    }
    pub const fn callsite_identity(&self) -> DigestV1 {
        self.callsite_identity
    }
    pub const fn actual_root(&self) -> DigestV1 {
        self.actual_root
    }
    pub const fn reference_root(&self) -> DigestV1 {
        self.reference_root
    }
    pub const fn authenticated_proof(&self) -> DigestV1 {
        self.authenticated_proof
    }
    pub const fn argument_count(&self) -> u16 {
        self.argument_count
    }
    pub const fn scope(&self) -> ParallelExecutionScopeV1 {
        self.scope
    }
    pub const fn kind(&self) -> ParallelCallKindV1 {
        self.kind
    }
    pub const fn numerical_policy(&self) -> ParallelNumericalPolicyV1 {
        self.numerical_policy
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParallelOutputRelationV1 {
    identity: DigestV1,
    output_contract: DigestV1,
    logical_domain: DigestV1,
    schedule: ParallelScheduleRelationV1,
    numerical_policy: ParallelNumericalPolicyV1,
    hierarchy: Box<[ParallelHierarchyLevelV1]>,
    call_summaries: Box<[DigestV1]>,
    authenticated_proof: DigestV1,
}

impl ParallelOutputRelationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: DigestV1,
        output_contract: DigestV1,
        logical_domain: DigestV1,
        schedule: ParallelScheduleRelationV1,
        numerical_policy: ParallelNumericalPolicyV1,
        hierarchy: Vec<ParallelHierarchyLevelV1>,
        call_summaries: Vec<DigestV1>,
        authenticated_proof: DigestV1,
    ) -> Result<Self, ParallelReferenceContractErrorV1> {
        if [
            identity,
            output_contract,
            logical_domain,
            authenticated_proof,
        ]
        .into_iter()
        .any(DigestV1::is_zero)
            || hierarchy.as_slice() != COMPLETE_GPU_HIERARCHY_V1
            || call_summaries.len() > HARD_MAX_RELATION_SUMMARIES_V1
            || call_summaries.iter().any(|identity| identity.is_zero())
            || call_summaries
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != call_summaries.len()
        {
            return Err(ParallelReferenceContractErrorV1::InvalidOutputRelation);
        }
        numerical_policy.validate()?;
        if !valid_schedule(schedule) {
            return Err(ParallelReferenceContractErrorV1::InvalidScheduleRelation);
        }
        Ok(Self {
            identity,
            output_contract,
            logical_domain,
            schedule,
            numerical_policy,
            hierarchy: hierarchy.into_boxed_slice(),
            call_summaries: call_summaries.into_boxed_slice(),
            authenticated_proof,
        })
    }

    pub const fn identity(&self) -> DigestV1 {
        self.identity
    }
    pub const fn output_contract(&self) -> DigestV1 {
        self.output_contract
    }
    pub const fn logical_domain(&self) -> DigestV1 {
        self.logical_domain
    }
    pub const fn schedule(&self) -> ParallelScheduleRelationV1 {
        self.schedule
    }
    pub const fn numerical_policy(&self) -> ParallelNumericalPolicyV1 {
        self.numerical_policy
    }
    pub fn hierarchy(&self) -> &[ParallelHierarchyLevelV1] {
        &self.hierarchy
    }
    pub fn call_summaries(&self) -> &[DigestV1] {
        &self.call_summaries
    }
    pub const fn authenticated_proof(&self) -> DigestV1 {
        self.authenticated_proof
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelReferenceContractV1 {
    semantic_contract_identity: DigestV1,
    relations: Box<[ParallelOutputRelationV1]>,
    call_summaries: Box<[ParallelCallSummaryV1]>,
}

impl ParallelReferenceContractV1 {
    pub fn new(
        semantic_contract_identity: DigestV1,
        relations: Vec<ParallelOutputRelationV1>,
        call_summaries: Vec<ParallelCallSummaryV1>,
    ) -> Result<Self, ParallelReferenceContractErrorV1> {
        if semantic_contract_identity.is_zero()
            || relations.is_empty()
            || relations.len() > HARD_MAX_PARALLEL_OUTPUT_RELATIONS_V1
            || call_summaries.len() > HARD_MAX_PARALLEL_CALL_SUMMARIES_V1
        {
            return Err(ParallelReferenceContractErrorV1::ResourceLimit);
        }
        let relation_ids = relations
            .iter()
            .map(ParallelOutputRelationV1::identity)
            .collect::<BTreeSet<_>>();
        let output_ids = relations
            .iter()
            .map(ParallelOutputRelationV1::output_contract)
            .collect::<BTreeSet<_>>();
        if relation_ids.len() != relations.len() || output_ids.len() != relations.len() {
            return Err(ParallelReferenceContractErrorV1::DuplicateIdentity);
        }
        let summaries = call_summaries
            .iter()
            .map(ParallelCallSummaryV1::identity)
            .collect::<BTreeSet<_>>();
        if summaries.len() != call_summaries.len() {
            return Err(ParallelReferenceContractErrorV1::DuplicateIdentity);
        }
        let used = relations
            .iter()
            .flat_map(|relation| relation.call_summaries.iter().copied())
            .collect::<BTreeSet<_>>();
        if !used.is_subset(&summaries) {
            return Err(ParallelReferenceContractErrorV1::UnknownCallSummary);
        }
        if used != summaries {
            return Err(ParallelReferenceContractErrorV1::UnusedCallSummary);
        }
        Ok(Self {
            semantic_contract_identity,
            relations: relations.into_boxed_slice(),
            call_summaries: call_summaries.into_boxed_slice(),
        })
    }

    pub const fn semantic_contract_identity(&self) -> DigestV1 {
        self.semantic_contract_identity
    }
    pub fn relations(&self) -> &[ParallelOutputRelationV1] {
        &self.relations
    }
    pub fn call_summaries(&self) -> &[ParallelCallSummaryV1] {
        &self.call_summaries
    }

    pub fn canonical_sha256(&self) -> DigestV1 {
        let mut digest = Sha256::new();
        put_blob(&mut digest, CONTRACT_DOMAIN_V1);
        put_digest(&mut digest, self.semantic_contract_identity);
        put_u64(&mut digest, self.relations.len() as u64);
        for relation in &self.relations {
            for value in [
                relation.identity,
                relation.output_contract,
                relation.logical_domain,
            ] {
                put_digest(&mut digest, value);
            }
            put_schedule(&mut digest, relation.schedule);
            put_numerical(&mut digest, relation.numerical_policy);
            put_u64(&mut digest, relation.hierarchy.len() as u64);
            for level in &relation.hierarchy {
                digest.update([hierarchy_tag(*level)]);
            }
            put_u64(&mut digest, relation.call_summaries.len() as u64);
            for summary in &relation.call_summaries {
                put_digest(&mut digest, *summary);
            }
            put_digest(&mut digest, relation.authenticated_proof);
        }
        put_u64(&mut digest, self.call_summaries.len() as u64);
        for summary in &self.call_summaries {
            for value in [
                summary.identity,
                summary.callsite_identity,
                summary.actual_root,
                summary.reference_root,
                summary.authenticated_proof,
            ] {
                put_digest(&mut digest, value);
            }
            digest.update(summary.argument_count.to_le_bytes());
            digest.update([scope_tag(summary.scope)]);
            match summary.kind {
                ParallelCallKindV1::SafeRustHelper => digest.update([1]),
                ParallelCallKindV1::CompilerIntrinsic => digest.update([2]),
                ParallelCallKindV1::CooperativeTensorIntrinsic {
                    site_ordinal,
                    layout_identity,
                } => {
                    digest.update([3]);
                    digest.update(site_ordinal.to_le_bytes());
                    put_digest(&mut digest, layout_identity);
                }
            }
            put_numerical(&mut digest, summary.numerical_policy);
        }
        DigestV1::from_untrusted_bytes(digest.finalize().into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParallelReferenceContractErrorV1 {
    ResourceLimit,
    DuplicateIdentity,
    InvalidOutputRelation,
    InvalidScheduleRelation,
    InvalidNumericalPolicy,
    UnboundedRelaxedPolicy,
    InvalidCallSummary,
    UnknownCallSummary,
    UnusedCallSummary,
}

impl fmt::Display for ParallelReferenceContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit => formatter.write_str("parallel-reference contract is empty or exceeds a hard resource limit"),
            Self::DuplicateIdentity => formatter.write_str("parallel-reference contract reuses a relation, output, or summary identity"),
            Self::InvalidOutputRelation => formatter.write_str("parallel output relation must bind a finite logical output through invocation, subgroup, workgroup, and grid ownership"),
            Self::InvalidScheduleRelation => formatter.write_str("parallel schedule relation is missing its live permutation, fold, recurrence, loop, bound, or order proof identity"),
            Self::InvalidNumericalPolicy => formatter.write_str("parallel numerical policy has an invalid type, finite error bound, witness, or proof identity"),
            Self::UnboundedRelaxedPolicy => formatter.write_str("unbounded relaxed floating-point semantics are not a correctness policy; provide an authenticated finite error bound"),
            Self::InvalidCallSummary => formatter.write_str("parallel helper or intrinsic summary has an invalid scope, callsite, typed root, layout, arity, or proof identity"),
            Self::UnknownCallSummary => formatter.write_str("parallel output relation names an undeclared helper or intrinsic summary"),
            Self::UnusedCallSummary => formatter.write_str("parallel helper or intrinsic summary is not consumed by any output relation"),
        }
    }
}

impl Error for ParallelReferenceContractErrorV1 {}

fn valid_schedule(schedule: ParallelScheduleRelationV1) -> bool {
    match schedule {
        ParallelScheduleRelationV1::PointwiseBijection => true,
        ParallelScheduleRelationV1::Permutation { collective } => !collective.is_zero(),
        ParallelScheduleRelationV1::Fold {
            collective, order, ..
        } => {
            !collective.is_zero()
                && match order {
                    ParallelFoldOrderV1::Preserved => true,
                    ParallelFoldOrderV1::AlgebraicallyReordered {
                        associativity_proof,
                        commutativity_proof,
                    } => !associativity_proof.is_zero() && !commutativity_proof.is_zero(),
                    ParallelFoldOrderV1::ErrorBoundedReordering { proof } => !proof.is_zero(),
                }
        }
        ParallelScheduleRelationV1::BoundedRecurrence {
            collective,
            loop_contract,
            dynamic_bound_proof,
            ..
        } => {
            !collective.is_zero()
                && !loop_contract.is_zero()
                && dynamic_bound_proof.is_none_or(|proof| !proof.is_zero())
        }
    }
}

fn hierarchy_tag(level: ParallelHierarchyLevelV1) -> u8 {
    match level {
        ParallelHierarchyLevelV1::Invocation => 1,
        ParallelHierarchyLevelV1::Subgroup => 2,
        ParallelHierarchyLevelV1::Workgroup => 3,
        ParallelHierarchyLevelV1::Grid => 4,
    }
}
fn scope_tag(scope: ParallelExecutionScopeV1) -> u8 {
    match scope {
        ParallelExecutionScopeV1::SequentialReference => 1,
        ParallelExecutionScopeV1::Invocation => 2,
        ParallelExecutionScopeV1::Subgroup => 3,
        ParallelExecutionScopeV1::Workgroup => 4,
        ParallelExecutionScopeV1::Grid => 5,
    }
}
fn order_tag(order: SemanticEvaluationOrderV1) -> u8 {
    match order {
        SemanticEvaluationOrderV1::SequentialAscending => 1,
        SemanticEvaluationOrderV1::SequentialDescending => 2,
        SemanticEvaluationOrderV1::Lexicographic => 3,
        SemanticEvaluationOrderV1::ExplicitTree => 4,
    }
}

fn put_schedule(digest: &mut Sha256, schedule: ParallelScheduleRelationV1) {
    match schedule {
        ParallelScheduleRelationV1::PointwiseBijection => digest.update([1]),
        ParallelScheduleRelationV1::Permutation { collective } => {
            digest.update([2]);
            put_digest(digest, collective);
        }
        ParallelScheduleRelationV1::Fold {
            collective,
            order,
            reference_order,
        } => {
            digest.update([3]);
            put_digest(digest, collective);
            match order {
                ParallelFoldOrderV1::Preserved => digest.update([1]),
                ParallelFoldOrderV1::AlgebraicallyReordered {
                    associativity_proof,
                    commutativity_proof,
                } => {
                    digest.update([2]);
                    put_digest(digest, associativity_proof);
                    put_digest(digest, commutativity_proof);
                }
                ParallelFoldOrderV1::ErrorBoundedReordering { proof } => {
                    digest.update([3]);
                    put_digest(digest, proof);
                }
            }
            digest.update([order_tag(reference_order)]);
        }
        ParallelScheduleRelationV1::BoundedRecurrence {
            collective,
            loop_contract,
            dynamic_bound_proof,
            reference_order,
        } => {
            digest.update([4]);
            put_digest(digest, collective);
            put_digest(digest, loop_contract);
            match dynamic_bound_proof {
                Some(proof) => {
                    digest.update([1]);
                    put_digest(digest, proof);
                }
                None => digest.update([0]),
            }
            digest.update([order_tag(reference_order)]);
        }
    }
}

fn put_numerical(digest: &mut Sha256, policy: ParallelNumericalPolicyV1) {
    match policy {
        ParallelNumericalPolicyV1::ExactBitVector => digest.update([1]),
        ParallelNumericalPolicyV1::IeeeOperatorCongruence {
            rounding,
            exceptional_values,
        } => {
            digest.update([2, rounding as u8, exceptional_values as u8]);
        }
        ParallelNumericalPolicyV1::ErrorBounded {
            absolute_error_f64_bits,
            relative_error_f64_bits,
            witness_root,
            proof,
        } => {
            digest.update([3]);
            digest.update(absolute_error_f64_bits.to_le_bytes());
            digest.update(relative_error_f64_bits.to_le_bytes());
            put_digest(digest, witness_root);
            put_digest(digest, proof);
        }
        ParallelNumericalPolicyV1::UnboundedRelaxed => digest.update([4]),
    }
}
fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    put_u64(digest, bytes.len() as u64);
    digest.update(bytes);
}
fn put_digest(digest: &mut Sha256, value: DigestV1) {
    put_blob(digest, value.as_bytes());
}
fn put_u64(digest: &mut Sha256, value: u64) {
    digest.update(value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }
    fn relation(
        policy: ParallelNumericalPolicyV1,
        summaries: Vec<DigestV1>,
    ) -> Result<ParallelOutputRelationV1, ParallelReferenceContractErrorV1> {
        ParallelOutputRelationV1::new(
            d(1),
            d(2),
            d(3),
            ParallelScheduleRelationV1::PointwiseBijection,
            policy,
            COMPLETE_GPU_HIERARCHY_V1.to_vec(),
            summaries,
            d(4),
        )
    }

    #[test]
    fn exact_pointwise_relation_covers_the_complete_hierarchy() {
        let relation = relation(ParallelNumericalPolicyV1::ExactBitVector, vec![]).unwrap();
        let contract = ParallelReferenceContractV1::new(d(5), vec![relation], vec![]).unwrap();
        assert_eq!(
            contract.relations()[0].hierarchy(),
            COMPLETE_GPU_HIERARCHY_V1
        );
        assert_ne!(contract.canonical_sha256(), DigestV1::ZERO);
    }

    #[test]
    fn unbounded_or_invalid_error_policies_fail_closed() {
        assert_eq!(
            relation(ParallelNumericalPolicyV1::UnboundedRelaxed, vec![]),
            Err(ParallelReferenceContractErrorV1::UnboundedRelaxedPolicy)
        );
        for (absolute, relative) in [
            (0.0, 0.0),
            (f64::NAN, 1.0),
            (-1.0, 1.0),
            (1.0, f64::INFINITY),
        ] {
            let policy = ParallelNumericalPolicyV1::ErrorBounded {
                absolute_error_f64_bits: absolute.to_bits(),
                relative_error_f64_bits: relative.to_bits(),
                witness_root: d(8),
                proof: d(9),
            };
            assert_eq!(
                relation(policy, vec![]),
                Err(ParallelReferenceContractErrorV1::InvalidNumericalPolicy)
            );
        }
        relation(
            ParallelNumericalPolicyV1::ErrorBounded {
                absolute_error_f64_bits: 1.0_f64.to_bits(),
                relative_error_f64_bits: 0.0_f64.to_bits(),
                witness_root: d(8),
                proof: d(9),
            },
            vec![],
        )
        .unwrap();
    }

    #[test]
    fn hierarchy_fold_and_dynamic_recurrence_proofs_are_closed() {
        assert_eq!(
            ParallelOutputRelationV1::new(
                d(1),
                d(2),
                d(3),
                ParallelScheduleRelationV1::PointwiseBijection,
                ParallelNumericalPolicyV1::ExactBitVector,
                vec![
                    ParallelHierarchyLevelV1::Invocation,
                    ParallelHierarchyLevelV1::Grid
                ],
                vec![],
                d(4)
            ),
            Err(ParallelReferenceContractErrorV1::InvalidOutputRelation)
        );
        let bad_fold = ParallelScheduleRelationV1::Fold {
            collective: d(6),
            order: ParallelFoldOrderV1::AlgebraicallyReordered {
                associativity_proof: DigestV1::ZERO,
                commutativity_proof: d(7),
            },
            reference_order: SemanticEvaluationOrderV1::SequentialAscending,
        };
        assert_eq!(
            ParallelOutputRelationV1::new(
                d(1),
                d(2),
                d(3),
                bad_fold,
                ParallelNumericalPolicyV1::ExactBitVector,
                COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                vec![],
                d(4)
            ),
            Err(ParallelReferenceContractErrorV1::InvalidScheduleRelation)
        );
        let bad_dynamic = ParallelScheduleRelationV1::BoundedRecurrence {
            collective: d(6),
            loop_contract: d(7),
            dynamic_bound_proof: Some(DigestV1::ZERO),
            reference_order: SemanticEvaluationOrderV1::SequentialAscending,
        };
        assert_eq!(
            ParallelOutputRelationV1::new(
                d(1),
                d(2),
                d(3),
                bad_dynamic,
                ParallelNumericalPolicyV1::ExactBitVector,
                COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                vec![],
                d(4)
            ),
            Err(ParallelReferenceContractErrorV1::InvalidScheduleRelation)
        );
    }

    #[test]
    fn summaries_must_be_scoped_and_consumed() {
        let summary = ParallelCallSummaryV1::new(
            d(10),
            d(11),
            d(12),
            d(13),
            d(14),
            2,
            ParallelExecutionScopeV1::Invocation,
            ParallelCallKindV1::CompilerIntrinsic,
            ParallelNumericalPolicyV1::ExactBitVector,
        )
        .unwrap();
        assert_eq!(
            ParallelReferenceContractV1::new(
                d(5),
                vec![relation(ParallelNumericalPolicyV1::ExactBitVector, vec![]).unwrap()],
                vec![summary.clone()]
            ),
            Err(ParallelReferenceContractErrorV1::UnusedCallSummary)
        );
        ParallelReferenceContractV1::new(
            d(5),
            vec![relation(ParallelNumericalPolicyV1::ExactBitVector, vec![d(10)]).unwrap()],
            vec![summary],
        )
        .unwrap();
        assert_eq!(
            ParallelCallSummaryV1::new(
                d(10),
                d(11),
                d(12),
                d(13),
                d(14),
                2,
                ParallelExecutionScopeV1::Workgroup,
                ParallelCallKindV1::CooperativeTensorIntrinsic {
                    site_ordinal: 0,
                    layout_identity: d(15)
                },
                ParallelNumericalPolicyV1::ExactBitVector
            ),
            Err(ParallelReferenceContractErrorV1::InvalidCallSummary)
        );
    }
}
