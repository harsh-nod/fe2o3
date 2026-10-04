//! Private owning nominal continuation; not selected by ordinary compilation.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

/// Consume the actual materialized owner only after its outer materialization
/// postflights. The caller retains the original materialization account until
/// this returned program is dropped. The program owns the independent projection
/// account, acquired here before inventory or proof.
pub(crate) fn project_private_nominal_materialized_v1(
    materialized: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    root_inputs: &[ProductionRankedRootInputV1],
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    let (roots, phase) = {
        // Keep the actual owner stationary through every nested source borrow.
        let source = RankedProjectionSourceV1::from_materialized_nominal_private(&materialized)?;
        let mut ledger = ranked_projection_source_v1::projection_source_ledger_v1(&source)?;
        let roots = ledger.with_budget(|budget| {
            source.require_floor(budget)?;
            let mut owned = 0usize;
            let root = crate::production_pipeline::with_actual_retained_ranked_inputs_v1(
                &materialized,
                root_inputs,
                reference_bindings,
                budget,
                &mut owned,
                |actual, budget, owned| {
                    Ok::<_, Resource>(
                        canonical_assertion_facts_v1::consume_actual_nominal_root_v1(
                            &materialized,
                            &actual,
                            budget,
                            owned,
                        ),
                    )
                },
            )
            .map_err(ranked_projection_source_v1::resource)??;
            source.require_floor(budget)?;
            budget
                .check_prior_denials_v1()
                .map_err(ranked_projection_source_v1::resource)?;
            // Prepay the actual one-root roster before allocating its payload.
            let mut roots = Vec::new();
            {
                let mut resources =
                    bf16_nominal_preparation_resources_v1::PreparationResourcesV1::new(
                        budget, &mut owned,
                    );
                resources.reserve(&mut roots, 1)?;
            }
            roots.push(root);
            source.require_floor(budget)?;
            budget
                .check_prior_denials_v1()
                .map_err(ranked_projection_source_v1::resource)?;
            Ok::<_, ProductionRankedProjectionErrorV1>(roots.into_boxed_slice())
        })?;
        #[cfg(test)]
        {
            assert_eq!(roots.len(), 1);
            let actual = roots[0]
                .verification
                .ordinary()
                .expect("private nominal ordinary root");
            assert!(actual.all_mandatory_reports_are_clean());
            assert!(!roots[0].access_sources.is_empty());
            assert!(roots[0].executable_effect_sources.is_empty());
            ledger.with_budget(|budget| {
                assert!(budget.check_prior_denials_v1().is_ok());
                eprintln!("fe2o3-bf16-private-owned-projection-v1 roots=1 accesses={} storage={} work={} clean=true normal_admission=false",
                    roots[0].access_sources.len(), budget.storage(), budget.work());
            });
        }
        (
            roots,
            retained_phase_v1::RetainedProjectionPhaseV1::new(ledger),
        )
    };
    // These are the SAME moved objects/accounts. No receipt, result observation,
    // detached tuple or newly compiled replacement is used as authority.
    Ok(ProductionRankedSemanticProgramV1 {
        materialized,
        roots,
        phase,
    })
}
