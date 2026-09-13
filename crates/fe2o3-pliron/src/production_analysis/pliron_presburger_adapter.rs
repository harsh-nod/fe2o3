//! Private adapter from owner-custodied sparse Pliron facts to public,
//! pointer-independent Presburger models.

use dialect_kernel::MAX_RANKED_MEMORY_RANK;
use fe2o3_kernel_analysis::{
    PresburgerAffineExprV1, PresburgerBoxV1, PresburgerFailureV1, PresburgerMapExprV1,
    PresburgerMapV1, PresburgerSetV1,
};

use super::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitV1, ProductionAnalysisResourceLimitsV1,
    ProductionAnalysisResourcePhaseV1, ProductionAnalysisResourceUpperBoundV1,
};
use super::pliron_sparse_index::{SparseIndexAnalysisV1, SparseIndexFactV1};

/// Bounds the cache-owned Presburger adapter. Search-domain work belongs to
/// the pass issuing a concrete query; constructing this adapter only copies
/// the bounded launch-extent vector retained by sparse-index analysis.
pub(crate) fn preflight_presburger_resource_upper_bound_v1(
    sparse: Option<&SparseIndexAnalysisV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    presburger_resource_upper_bound_for_rank_v1(
        sparse.map_or(0, |sparse| sparse.launch_extents().len()),
        limits,
    )
}

fn presburger_resource_upper_bound_for_rank_v1(
    rank: usize,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::Presburger;
    if rank > MAX_RANKED_MEMORY_RANK {
        return Err(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "Presburger adapter rank hard limit",
        });
    }
    let work = rank
        .checked_add(1)
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "Presburger adapter work upper bound",
        })?;
    let retained = rank
        .checked_add(1)
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "Presburger adapter retained storage upper bound",
        })?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, work, retained, rank)?;
    limits.require(phase, bound)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlironPresburgerAnalysisV1 {
    launch_extents: Vec<u64>,
}

impl PlironPresburgerAnalysisV1 {
    pub(crate) fn from_sparse(sparse: &SparseIndexAnalysisV1) -> Self {
        Self::for_launch_extents(sparse.launch_extents().to_vec())
    }

    pub(crate) fn for_launch_extents(launch_extents: Vec<u64>) -> Self {
        Self { launch_extents }
    }

    pub(crate) fn map_for_facts(
        &self,
        facts: &[SparseIndexFactV1],
    ) -> Result<PresburgerMapV1, PresburgerFailureV1> {
        self.map_for_facts_over_extents(facts, &self.launch_extents)
    }

    pub(crate) fn map_for_facts_over_extents(
        &self,
        facts: &[SparseIndexFactV1],
        launch_extents: &[u64],
    ) -> Result<PresburgerMapV1, PresburgerFailureV1> {
        if launch_extents.contains(&0) {
            return Err(PresburgerFailureV1::Unsupported {
                detail: "a dynamic launch extent has no finite compiler bound",
            });
        }
        let domain = PresburgerSetV1::box_only(PresburgerBoxV1::zero_based(launch_extents)?);
        let outputs = facts
            .iter()
            .map(|fact| self.map_expr_for_fact_over_extents(fact, launch_extents))
            .collect::<Result<Vec<_>, _>>()?;
        PresburgerMapV1::new(domain, outputs)
    }

    fn map_expr_for_fact_over_extents(
        &self,
        fact: &SparseIndexFactV1,
        launch_extents: &[u64],
    ) -> Result<PresburgerMapExprV1, PresburgerFailureV1> {
        let affine = |constant: u64, coefficients: &[u64]| {
            if coefficients
                .iter()
                .skip(launch_extents.len())
                .any(|coefficient| *coefficient != 0)
            {
                return Err(PresburgerFailureV1::Unsupported {
                    detail: "an affine index depends on an undeclared invocation dimension",
                });
            }
            PresburgerAffineExprV1::new(
                i128::from(constant),
                coefficients
                    .iter()
                    .take(launch_extents.len())
                    .map(|coefficient| i128::from(*coefficient))
                    .collect(),
            )
        };
        match fact {
            SparseIndexFactV1::Affine(expression) => Ok(PresburgerMapExprV1::Affine(affine(
                expression.constant_term(),
                expression.coefficients(),
            )?)),
            SparseIndexFactV1::Remainder { dividend, modulus } if *modulus != 0 => {
                Ok(PresburgerMapExprV1::Remainder {
                    dividend: affine(dividend.constant_term(), dividend.coefficients())?,
                    modulus: i128::from(*modulus),
                })
            }
            SparseIndexFactV1::Remainder { .. } => Err(PresburgerFailureV1::InvalidModel {
                detail: "sparse remainder has a zero modulus",
            }),
            SparseIndexFactV1::MachineOverflow(_) => {
                Err(PresburgerFailureV1::MachineIntegerOverflow {
                    bits: 64,
                    signed: false,
                })
            }
            SparseIndexFactV1::Unknown
            | SparseIndexFactV1::CheckedTiled2D(_)
            | SparseIndexFactV1::CheckedRowStriped2D(_) => Err(PresburgerFailureV1::Unsupported {
                detail: "index fact is outside the affine/remainder Presburger fragment",
            }),
        }
    }
}

#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    #[test]
    fn presburger_adapter_bound_accepts_exact_limits_and_rejects_one_under() {
        let rank = 7;
        let exact = presburger_resource_upper_bound_for_rank_v1(
            rank,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), 8);
        assert_eq!(exact.retained_storage_upper_bound(), 8);
        assert_eq!(exact.peak_storage_upper_bound(), 15);
        assert_eq!(
            presburger_resource_upper_bound_for_rank_v1(
                rank,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Ok(exact)
        );
        assert!(
            presburger_resource_upper_bound_for_rank_v1(
                rank,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            )
            .is_err()
        );
        assert!(
            presburger_resource_upper_bound_for_rank_v1(MAX_RANKED_MEMORY_RANK + 1, unbounded())
                .is_err()
        );
    }

    const fn unbounded() -> ProductionAnalysisResourceLimitsV1 {
        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
    }
}
