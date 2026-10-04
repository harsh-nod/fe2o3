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
            // The exact moved lowering and source maps remain borrowed from
            // this root; no second projection/frontend or reconstructed receipt.
            let slot = budget as *const _ as usize;
            let identity = budget.work_ledger_identity_v1();
            let storage = budget.storage();
            let work = budget.work();
            let credits = owned;
            let lowering = root.verification.ordinary().ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "private nominal root has no ordinary ranked lowering",
                ),
            )?;
            let validation = materialized
                .verify_private_bf16_nominal_candidate_translation_with_budget_v1(
                    root.semantic_root,
                    lowering,
                    &root.access_sources,
                    &root.executable_effect_sources,
                    budget,
                )
                .map_err(ProductionRankedProjectionErrorV1::StructuralValidation)?;
            if budget as *const _ as usize != slot
                || budget.work_ledger_identity_v1() != identity
                || budget.storage() != storage
                || budget.work() <= work
                || owned != credits
            {
                drop(validation);
                return Err(ranked_projection_source_v1::resource(Resource::Accounting));
            }
            source.require_floor(budget)?;
            budget.check_prior_denials_v1().map_err(ranked_projection_source_v1::resource)?;
            // One fixed SHA-256 comparison; no source-sized rehash or allocation.
            budget.charge_work(32).map_err(ranked_projection_source_v1::resource)?;
            if validation.semantic_sha256()
                != materialized.semantic_ssa().source_semantic().semantic_sha256().as_bytes()
                || validation.tensor_operations() != 1
                || validation.claims_indexed_address_equivalence()
                || validation.claims_complete_operational_equivalence()
                || validation.reconciled_projection_remains_trusted()
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "private nominal translation report/source differs",
                ));
            }
            #[cfg(test)]
            eprintln!("fe2o3-bf16-private-lowerer-validation-v1 completed=true tensors=1 memory={} values={} storage={} work={} same_account=true source_join=true normal_admission=false attached=false",
                validation.memory_effects(), validation.value_expressions(),
                budget.storage(), budget.work());
            drop(validation);
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
