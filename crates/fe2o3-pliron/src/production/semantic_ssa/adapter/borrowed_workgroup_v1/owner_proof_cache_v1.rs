//! One result per owner in one immutable candidate-graph traversal.
use super::*;

#[derive(Default)]
pub(super) struct OwnerProofCache {
    results: BTreeMap<(u32, SemanticTypeIdV1), bool>,
}

impl OwnerProofCache {
    // The caller keeps this cache local to sites_with_defined, after all candidate
    // mutations. The proof checks every root of the owner, not only this root.
    pub(super) fn prove(
        &mut self,
        root: &SemanticBorrowCandidateV1,
        budget: &mut Budget,
        proof: impl FnOnce(&mut Budget) -> Result<bool, ProductionSemanticSsaErrorV1>,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        budget.charge(1)?;
        let key = (root.source_local, root.source_type);
        if let Some(&accepted) = self.results.get(&key) {
            return Ok(accepted);
        }
        let accepted = proof(budget)?;
        budget.charge(1)?;
        self.results.insert(key, accepted);
        Ok(accepted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(limit: usize) -> Budget {
        Budget {
            remaining: limit,
            limit,
            profile: FlowWorkProfile::default(),
        }
    }

    fn root(owner: u32, owned: u32) -> SemanticBorrowCandidateV1 {
        SemanticBorrowCandidateV1 {
            site: SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0,
            },
            source_local: owner,
            source_type: SemanticTypeIdV1::from_index(owned),
            source_reference: None,
            value_alias: false, source_kind: SemanticBorrowCandidateSourceV1::Direct,
            valid: true,
            consumers: 0,
            intrinsic_consumer: false,
        }
    }

    #[test]
    fn completed_positive_and_negative_are_each_evaluated_once_per_owner_and_type() {
        let mut cache = OwnerProofCache::default();
        let mut budget = budget(100);
        for (owner, owned, accepted) in [(1, 1, true), (2, 1, false), (1, 2, false)] {
            let candidate = root(owner, owned);
            assert_eq!(
                cache.prove(&candidate, &mut budget, |_| Ok(accepted)),
                Ok(accepted)
            );
            assert_eq!(
                cache.prove(&candidate, &mut budget, |_| panic!(
                    "must reuse completed proof"
                )),
                Ok(accepted)
            );
        }
    }

    #[test]
    fn failed_proof_is_not_cached() {
        let mut cache = OwnerProofCache::default();
        let mut budget = budget(100);
        let root = root(1, 1);
        assert_eq!(
            cache.prove(&root, &mut budget, |_| Err(
                ProductionSemanticSsaErrorV1::ReplayMismatch
            )),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
        assert_eq!(cache.prove(&root, &mut budget, |_| Ok(false)), Ok(false));
    }

    #[test]
    fn insertion_exhaustion_does_not_publish_a_completed_result() {
        let mut cache = OwnerProofCache::default();
        let root = root(1, 1);
        let proof = |budget: &mut Budget| {
            budget.charge(3)?;
            Ok(true)
        };
        let mut short = budget(4);
        assert!(cache.prove(&root, &mut short, proof).is_err());
        assert!(cache.results.is_empty());
        let mut exact = budget(5);
        assert_eq!(cache.prove(&root, &mut exact, proof), Ok(true));
        assert_eq!(exact.remaining, 0);
        let mut lookup = budget(1);
        assert_eq!(
            cache.prove(&root, &mut lookup, |_| panic!("must use completed result")),
            Ok(true)
        );
        assert_eq!(lookup.remaining, 0);
    }
}
