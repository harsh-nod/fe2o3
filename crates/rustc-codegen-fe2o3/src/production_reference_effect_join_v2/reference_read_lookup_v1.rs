//! Joins existing RHS leaves and independently retained immutable source reads.

use super::*;
use crate::production_ranked_projection_v1::ReferenceReadRosterV1;
use fe2o3_pliron::ProductionSemanticLoadV2;

pub(super) struct ReferenceReadLookupV1<'a> {
    pub(super) kernel: &'a ProductionRankedKernelV1,
    pub(super) loads: Vec<&'a ProductionSemanticLoadV2>,
    pub(super) roster: &'a ReferenceReadRosterV1,
}

impl<'a> ReferenceReadLookupV1<'a> {
    pub(super) fn new(
        kernel: &'a ProductionRankedKernelV1,
        expression: &'a ProductionSemanticExpressionV2,
        roster: &'a ReferenceReadRosterV1,
    ) -> Result<Self, ProductionReferenceEffectJoinErrorV2> {
        let stats = expression
            .validate()
            .map_err(ProductionReferenceEffectJoinErrorV2::SemanticExpression)?;
        roster
            .charge(stats.nodes)
            .map_err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference)?;
        let mut leaves = Vec::new();
        leaves.try_reserve_exact(stats.nodes).map_err(|_| {
            ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                "reference read lookup storage cannot be reserved",
            )
        })?;
        collect_semantic_loads_v2(expression, &mut leaves);
        roster
            .charge(leaves.len())
            .map_err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference)?;
        let mut occurrences = BTreeMap::new();
        for load in leaves.into_iter().chain(roster.loads()) {
            roster
                .charge(1 + load.indices.len())
                .map_err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference)?;
            let key = (load.block, load.operation);
            if let Some(previous) = occurrences.insert(key, load)
                && previous != load
            {
                return Err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference(
                    "one reference read occurrence has conflicting source metadata",
                ));
            }
        }
        roster
            .charge(occurrences.len())
            .map_err(ProductionReferenceEffectJoinErrorV2::UnsupportedReference)?;
        Ok(Self {
            kernel,
            loads: occurrences.into_values().collect(),
            roster,
        })
    }
}
