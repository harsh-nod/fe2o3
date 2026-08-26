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
pub const HARD_MAX_PARALLEL_CALL_ARGUMENTS_V1: u16 = 256;
pub const HARD_MAX_AGGREGATE_FUNCTIONAL_OUTPUTS_V1: usize = 64;

const CONTRACT_DOMAIN_V1: &[u8] = b"FE2O3/PARALLEL-REFERENCE-CONTRACT/V1\0";
const CONTRACT_DOMAIN_V2: &[u8] = b"FE2O3/PARALLEL-REFERENCE-CONTRACT/V2\0";

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParallelOutputRelationV1 {
    identity: DigestV1,
    output_contract: DigestV1,
    logical_domain: DigestV1,
    ranked_view_identity: DigestV1,
    ownership_identity: DigestV1,
    frame_identity: DigestV1,
    schedule: ParallelScheduleRelationV1,
    numerical_policy: ParallelNumericalPolicyV1,
    hierarchy: Box<[ParallelHierarchyLevelV1]>,
    tensor_refinement_identity: Option<DigestV1>,
    policy_checked_staging_identity: DigestV1,
}

impl ParallelOutputRelationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: DigestV1,
        output_contract: DigestV1,
        logical_domain: DigestV1,
        ranked_view_identity: DigestV1,
        ownership_identity: DigestV1,
        frame_identity: DigestV1,
        schedule: ParallelScheduleRelationV1,
        numerical_policy: ParallelNumericalPolicyV1,
        hierarchy: Vec<ParallelHierarchyLevelV1>,
        tensor_refinement_identity: Option<DigestV1>,
        policy_checked_staging_identity: DigestV1,
    ) -> Result<Self, ParallelReferenceContractErrorV1> {
        if [
            identity,
            output_contract,
            logical_domain,
            ranked_view_identity,
            ownership_identity,
            frame_identity,
            policy_checked_staging_identity,
        ]
        .into_iter()
        .any(DigestV1::is_zero)
            || hierarchy.as_slice() != COMPLETE_GPU_HIERARCHY_V1
            || tensor_refinement_identity.is_some_and(DigestV1::is_zero)
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
            ranked_view_identity,
            ownership_identity,
            frame_identity,
            schedule,
            numerical_policy,
            hierarchy: hierarchy.into_boxed_slice(),
            tensor_refinement_identity,
            policy_checked_staging_identity,
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
    pub const fn ranked_view_identity(&self) -> DigestV1 {
        self.ranked_view_identity
    }
    pub const fn ownership_identity(&self) -> DigestV1 {
        self.ownership_identity
    }
    pub const fn frame_identity(&self) -> DigestV1 {
        self.frame_identity
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
    /// Claim-specific receipt identity for a bounded cooperative-tensor
    /// composition, distinct from the output-effect proof.
    pub const fn tensor_refinement_identity(&self) -> Option<DigestV1> {
        self.tensor_refinement_identity
    }
    pub const fn policy_checked_staging_identity(&self) -> DigestV1 {
        self.policy_checked_staging_identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelReferenceContractV1 {
    semantic_contract_identity: DigestV1,
    output_product_identity: DigestV1,
    relations: Box<[ParallelOutputRelationV1]>,
}

impl ParallelReferenceContractV1 {
    pub fn new(
        semantic_contract_identity: DigestV1,
        output_product_identity: DigestV1,
        relations: Vec<ParallelOutputRelationV1>,
    ) -> Result<Self, ParallelReferenceContractErrorV1> {
        if semantic_contract_identity.is_zero()
            || output_product_identity.is_zero()
            || relations.is_empty()
            || relations.len() > HARD_MAX_PARALLEL_OUTPUT_RELATIONS_V1
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
        let view_ids = relations
            .iter()
            .map(ParallelOutputRelationV1::ranked_view_identity)
            .collect::<BTreeSet<_>>();
        let ownership_ids = relations
            .iter()
            .map(ParallelOutputRelationV1::ownership_identity)
            .collect::<BTreeSet<_>>();
        let frame_ids = relations
            .iter()
            .map(ParallelOutputRelationV1::frame_identity)
            .collect::<BTreeSet<_>>();
        if [
            relation_ids.len(),
            output_ids.len(),
            view_ids.len(),
            ownership_ids.len(),
            frame_ids.len(),
        ]
        .into_iter()
        .any(|count| count != relations.len())
        {
            return Err(ParallelReferenceContractErrorV1::DuplicateIdentity);
        }
        Ok(Self {
            semantic_contract_identity,
            output_product_identity,
            relations: relations.into_boxed_slice(),
        })
    }

    pub const fn semantic_contract_identity(&self) -> DigestV1 {
        self.semantic_contract_identity
    }
    pub const fn output_product_identity(&self) -> DigestV1 {
        self.output_product_identity
    }
    pub fn relations(&self) -> &[ParallelOutputRelationV1] {
        &self.relations
    }

    pub fn canonical_sha256(&self) -> DigestV1 {
        let mut digest = Sha256::new();
        let tensor_v2 = self
            .relations
            .iter()
            .any(|relation| relation.tensor_refinement_identity.is_some());
        put_blob(
            &mut digest,
            if tensor_v2 {
                CONTRACT_DOMAIN_V2
            } else {
                CONTRACT_DOMAIN_V1
            },
        );
        put_digest(&mut digest, self.semantic_contract_identity);
        put_digest(&mut digest, self.output_product_identity);
        put_u64(&mut digest, self.relations.len() as u64);
        for relation in &self.relations {
            for value in [
                relation.identity,
                relation.output_contract,
                relation.logical_domain,
                relation.ranked_view_identity,
                relation.ownership_identity,
                relation.frame_identity,
            ] {
                put_digest(&mut digest, value);
            }
            put_schedule(&mut digest, relation.schedule);
            put_numerical(&mut digest, relation.numerical_policy);
            put_u64(&mut digest, relation.hierarchy.len() as u64);
            for level in &relation.hierarchy {
                digest.update([hierarchy_tag(*level)]);
            }
            if tensor_v2 {
                match relation.tensor_refinement_identity {
                    None => digest.update([0]),
                    Some(identity) => {
                        digest.update([1]);
                        put_digest(&mut digest, identity);
                    }
                }
            }
            put_digest(&mut digest, relation.policy_checked_staging_identity);
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
}

impl fmt::Display for ParallelReferenceContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit => formatter.write_str("parallel-reference contract is empty or exceeds a hard resource limit"),
            Self::DuplicateIdentity => formatter.write_str("parallel-reference contract reuses a relation, output, ranked view, ownership, or frame identity"),
            Self::InvalidOutputRelation => formatter.write_str("parallel output relation must bind a finite logical output through invocation, subgroup, workgroup, and grid ownership"),
            Self::InvalidScheduleRelation => formatter.write_str("parallel schedule relation is missing its live permutation, fold, recurrence, loop, bound, or order proof identity"),
            Self::InvalidNumericalPolicy => formatter.write_str("parallel numerical policy has an invalid type, finite error bound, witness, or proof identity"),
            Self::UnboundedRelaxedPolicy => formatter.write_str("unbounded relaxed floating-point semantics are not a correctness policy; provide an authenticated finite error bound"),
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
    ) -> Result<ParallelOutputRelationV1, ParallelReferenceContractErrorV1> {
        ParallelOutputRelationV1::new(
            d(1),
            d(2),
            d(3),
            d(20),
            d(21),
            d(22),
            ParallelScheduleRelationV1::PointwiseBijection,
            policy,
            COMPLETE_GPU_HIERARCHY_V1.to_vec(),
            None,
            d(4),
        )
    }

    #[test]
    fn exact_pointwise_relation_covers_the_complete_hierarchy() {
        let relation = relation(ParallelNumericalPolicyV1::ExactBitVector).unwrap();
        let contract = ParallelReferenceContractV1::new(d(5), d(6), vec![relation]).unwrap();
        assert_eq!(
            contract.relations()[0].hierarchy(),
            COMPLETE_GPU_HIERARCHY_V1
        );
        assert_ne!(contract.canonical_sha256(), DigestV1::ZERO);
    }

    #[test]
    fn canonical_v1_none_hash_is_stable_and_tensor_v2_binds_the_claim() {
        let scalar = relation(ParallelNumericalPolicyV1::ExactBitVector).unwrap();
        let scalar_contract =
            ParallelReferenceContractV1::new(d(5), d(6), vec![scalar.clone()]).unwrap();
        assert_eq!(
            scalar_contract.canonical_sha256(),
            DigestV1::from_untrusted_bytes([
                213, 95, 232, 195, 103, 219, 84, 38, 63, 7, 99, 212, 212, 196, 94, 28, 163, 116,
                19, 53, 68, 178, 233, 55, 215, 221, 90, 51, 185, 215, 65, 35,
            ]),
        );
        let tensor_relation = |identity| {
            ParallelOutputRelationV1::new(
                scalar.identity(),
                scalar.output_contract(),
                scalar.logical_domain(),
                scalar.ranked_view_identity(),
                scalar.ownership_identity(),
                scalar.frame_identity(),
                scalar.schedule(),
                scalar.numerical_policy(),
                scalar.hierarchy().to_vec(),
                Some(identity),
                scalar.policy_checked_staging_identity(),
            )
            .unwrap()
        };
        let first =
            ParallelReferenceContractV1::new(d(5), d(6), vec![tensor_relation(d(30))]).unwrap();
        let second =
            ParallelReferenceContractV1::new(d(5), d(6), vec![tensor_relation(d(31))]).unwrap();
        assert_eq!(
            first.canonical_sha256(),
            DigestV1::from_untrusted_bytes([
                216, 122, 125, 99, 68, 227, 66, 65, 97, 27, 161, 11, 127, 67, 240, 105, 39, 54,
                172, 116, 241, 125, 171, 59, 200, 48, 177, 97, 58, 81, 189, 88,
            ]),
        );
        assert_ne!(first.canonical_sha256(), scalar_contract.canonical_sha256());
        assert_ne!(first.canonical_sha256(), second.canonical_sha256());
    }

    #[test]
    fn unbounded_or_invalid_error_policies_fail_closed() {
        assert_eq!(
            relation(ParallelNumericalPolicyV1::UnboundedRelaxed),
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
                relation(policy),
                Err(ParallelReferenceContractErrorV1::InvalidNumericalPolicy)
            );
        }
        relation(ParallelNumericalPolicyV1::ErrorBounded {
            absolute_error_f64_bits: 1.0_f64.to_bits(),
            relative_error_f64_bits: 0.0_f64.to_bits(),
            witness_root: d(8),
            proof: d(9),
        })
        .unwrap();
    }

    #[test]
    fn hierarchy_fold_and_dynamic_recurrence_proofs_are_closed() {
        assert_eq!(
            ParallelOutputRelationV1::new(
                d(1),
                d(2),
                d(3),
                d(20),
                d(21),
                d(22),
                ParallelScheduleRelationV1::PointwiseBijection,
                ParallelNumericalPolicyV1::ExactBitVector,
                vec![
                    ParallelHierarchyLevelV1::Invocation,
                    ParallelHierarchyLevelV1::Grid
                ],
                None,
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
                d(20),
                d(21),
                d(22),
                bad_fold,
                ParallelNumericalPolicyV1::ExactBitVector,
                COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                None,
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
                d(20),
                d(21),
                d(22),
                bad_dynamic,
                ParallelNumericalPolicyV1::ExactBitVector,
                COMPLETE_GPU_HIERARCHY_V1.to_vec(),
                None,
                d(4)
            ),
            Err(ParallelReferenceContractErrorV1::InvalidScheduleRelation)
        );
    }

    fn distinct_relation(
        base: u8,
        ranked_view_identity: DigestV1,
        ownership_identity: DigestV1,
        frame_identity: DigestV1,
        schedule: ParallelScheduleRelationV1,
    ) -> ParallelOutputRelationV1 {
        ParallelOutputRelationV1::new(
            d(base),
            d(base + 1),
            d(100),
            ranked_view_identity,
            ownership_identity,
            frame_identity,
            schedule,
            ParallelNumericalPolicyV1::ExactBitVector,
            COMPLETE_GPU_HIERARCHY_V1.to_vec(),
            None,
            d(base + 2),
        )
        .unwrap()
    }

    #[test]
    fn output_product_rejects_reused_view_ownership_or_frame_identity() {
        let pointwise = ParallelScheduleRelationV1::PointwiseBijection;
        for second in [
            distinct_relation(20, d(40), d(51), d(52), pointwise),
            distinct_relation(20, d(50), d(41), d(52), pointwise),
            distinct_relation(20, d(50), d(51), d(42), pointwise),
        ] {
            let first = distinct_relation(10, d(40), d(41), d(42), pointwise);
            assert_eq!(
                ParallelReferenceContractV1::new(d(5), d(6), vec![first, second]),
                Err(ParallelReferenceContractErrorV1::DuplicateIdentity)
            );
        }
    }

    #[test]
    fn output_product_accepts_independent_output_schedules() {
        let pointwise = distinct_relation(
            10,
            d(40),
            d(41),
            d(42),
            ParallelScheduleRelationV1::PointwiseBijection,
        );
        let folded = distinct_relation(
            20,
            d(50),
            d(51),
            d(52),
            ParallelScheduleRelationV1::Fold {
                collective: d(60),
                order: ParallelFoldOrderV1::Preserved,
                reference_order: SemanticEvaluationOrderV1::SequentialAscending,
            },
        );
        let contract =
            ParallelReferenceContractV1::new(d(5), d(6), vec![pointwise, folded]).unwrap();
        assert_eq!(contract.relations().len(), 2);
        assert!(matches!(
            contract.relations()[0].schedule(),
            ParallelScheduleRelationV1::PointwiseBijection
        ));
        assert!(matches!(
            contract.relations()[1].schedule(),
            ParallelScheduleRelationV1::Fold { .. }
        ));
        assert_ne!(contract.canonical_sha256(), DigestV1::ZERO);
    }
}
