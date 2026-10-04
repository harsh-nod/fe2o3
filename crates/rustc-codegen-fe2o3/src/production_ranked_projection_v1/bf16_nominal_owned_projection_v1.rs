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

#[cfg(test)]
mod roster_observation {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkLedgerIdentityV1 as WorkIdentity,
    };
    type Error = ProductionRankedVerificationErrorV1;
    // Only fixed-size observation records and fixed comparisons introduced here.
    // Existing roster authentication/replay keeps its original metering domain.
    // Two fixed passes: capture and subsequent equality/request comparison.
    const SOURCE_JOIN_WORK: usize = 2 * (32 + 4 + 12);
    const ACCOUNT_JOIN_WORK: usize = 8;
    const REPORT_JOIN_WORK: usize = 32 + 4;

    #[derive(Clone, Copy, Eq, PartialEq)]
    struct SourceJoin {
        semantic: [u8; 32],
        root: SemanticFunctionIdV1,
        accesses: usize,
        access_count: usize,
        ranked_ir: usize,
        ranked_bytes: usize,
        permutation: [u8; 4],
    }
    struct Before {
        account: WorkIdentity,
        storage: usize,
        work: usize,
        source: SourceJoin,
    }
    // Consumed before refund: no paid terminal counter survives its reservation.
    struct ValidatedObservation {
        work: usize,
        storage: usize,
        peak: usize,
    }
    impl ValidatedObservation {
        fn emit(self, requested: [u8; 4]) {
            eprintln!(
                "fe2o3-bf16-private-roster-conversion-v1 completed=true roots=1 accesses=3 permutation={} work={} storage={} observation_storage={} peak={} same_account=true source_join=true fresh_lowerer_validation=true cleanup_pending=true normal_admission=false attached=false",
                if requested == [0, 1, 2, 3] {
                    "identity"
                } else {
                    "swap01"
                },
                self.work,
                self.storage,
                OBSERVATION_STORAGE,
                self.peak
            );
        }
    }
    const OBSERVATION_STORAGE: usize = std::mem::size_of::<Before>()
        + std::mem::size_of::<SourceJoin>()
        + std::mem::size_of::<ValidatedObservation>();

    fn source_join(
        owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        root: SemanticFunctionIdV1,
        accesses: &[fe2o3_lower_mir_kernel::ProductionRankedAccessSourceV1],
        ranked_ir: &str,
        budget: &mut Budget<'_>,
    ) -> Result<SourceJoin, Error> {
        budget
            .charge_work(SOURCE_JOIN_WORK)
            .map_err(Error::ConditionalResource)?;
        let emission = owner
            .bf16_call_instance_emission_v1()
            .ok_or(Error::RosterMetadata(
                "roster observer has no nominal emission",
            ))?;
        if !std::ptr::eq(emission.owner(), owner)
            || emission.root() != root
            || accesses.len() != 3
            || ranked_ir.is_empty()
        {
            return Err(Error::RosterMetadata(
                "roster observer source/root/access shape differs",
            ));
        }
        Ok(SourceJoin {
            semantic: *owner
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            root,
            accesses: accesses.as_ptr() as usize,
            access_count: accesses.len(),
            ranked_ir: ranked_ir.as_ptr() as usize,
            ranked_bytes: ranked_ir.len(),
            permutation: emission.return_permutation(),
        })
    }
    fn require_same_source(
        before: &SourceJoin,
        after: &SourceJoin,
        requested: [u8; 4],
    ) -> Result<(), Error> {
        if !matches!(requested, [0, 1, 2, 3] | [1, 0, 2, 3])
            || before != after
            || after.permutation != requested
        {
            return Err(Error::RosterMetadata(
                "consuming roster source/access/Return join differs",
            ));
        }
        Ok(())
    }
    fn require_same_account(before: &Before, budget: &mut Budget<'_>) -> Result<(), Error> {
        budget
            .check_prior_denials_v1()
            .map_err(Error::ConditionalResource)?;
        budget
            .charge_work(ACCOUNT_JOIN_WORK)
            .map_err(Error::ConditionalResource)?;
        if budget.work_ledger_identity_v1() != before.account
            || budget.storage() != before.storage
            || budget.work() <= before.work
        {
            return Err(Error::ConditionalResource(Resource::Accounting));
        }
        Ok(())
    }

    impl ProductionRankedSemanticProgramV1 {
        /// Initial source checks finish inside the live Program loan. No paid
        /// observation survives the consuming conversion, including on Err.
        pub(crate) fn observe_private_nominal_roster_for_test_v1(
            mut self,
            requested: [u8; 4],
        ) -> Result<(), Error> {
            self.phase.require_clean_v1()?;
            self.phase.with_budget(|budget| {
                budget
                    .check_prior_denials_v1()
                    .map_err(Error::ConditionalResource)?;
                budget
                    .reserve_storage(std::mem::size_of::<SourceJoin>())
                    .map_err(Error::ConditionalResource)?;
                {
                    let [root] = self.roots.as_ref() else {
                        return Err(Error::RosterMetadata(
                            "roster observer requires one actual root",
                        ));
                    };
                    let source = source_join(
                        &self.materialized,
                        root.semantic_root,
                        &root.access_sources,
                        &root.ranked_ir,
                        budget,
                    )?;
                    require_same_source(&source, &source, requested)?;
                }
                budget
                    .release_storage(std::mem::size_of::<SourceJoin>())
                    .map_err(Error::ConditionalResource)?;
                Ok(())
            })?;
            consume_and_observe(self, requested)
        }
    }

    pub(super) fn consume_and_observe(
        program: ProductionRankedSemanticProgramV1,
        requested: [u8; 4],
    ) -> Result<(), Error> {
        // This actual shipping conversion owns all source/account data. A
        // refused conversion has no caller-local paid record left to destroy.
        let mut roster = program.into_verified_roster_receipt()?;
        roster.phase.require_clean_v1()?;
        let materialized = &roster.materialized;
        let roots = &roster.source_order_roots;
        let observation = roster.phase.with_budget(|budget| {
            budget
                .check_prior_denials_v1()
                .map_err(Error::ConditionalResource)?;
            budget
                .reserve_storage(OBSERVATION_STORAGE)
                .map_err(Error::ConditionalResource)?;
            let [root] = roots.as_ref() else {
                return Err(Error::RosterMetadata(
                    "converted roster changed actual root count",
                ));
            };
            let before = Before {
                account: budget.work_ledger_identity_v1(),
                storage: budget.storage(),
                work: budget.work(),
                source: source_join(
                    materialized,
                    root.semantic_root,
                    &root.access_sources,
                    &root.ranked_ir,
                    budget,
                )?,
            };
            require_same_source(&before.source, &before.source, requested)?;
            require_same_account(&before, budget)?;
            // Fresh full validation on the actual moved source/maps. Compare
            // snapshots before/after this call inside one surviving phase loan;
            // none is reconstructed or carried across a consuming transition.
            let validation = materialized
                .verify_private_bf16_nominal_candidate_translation_with_budget_v1(
                    root.semantic_root,
                    &root.lowering,
                    &root.access_sources,
                    &root.executable_effect_sources,
                    budget,
                )
                .map_err(Error::Custody)?;
            let after = source_join(
                materialized,
                root.semantic_root,
                &root.access_sources,
                &root.ranked_ir,
                budget,
            )?;
            require_same_source(&before.source, &after, requested)?;
            budget
                .charge_work(REPORT_JOIN_WORK)
                .map_err(Error::ConditionalResource)?;
            if validation.semantic_sha256() != &after.semantic
                || validation.tensor_operations() != 1
                || validation.claims_indexed_address_equivalence()
                || validation.claims_complete_operational_equivalence()
                || validation.reconciled_projection_remains_trusted()
            {
                return Err(Error::RosterMetadata(
                    "converted roster fresh validation differs",
                ));
            }
            drop(validation);
            require_same_account(&before, budget)?;
            drop(before);
            Ok(ValidatedObservation {
                work: budget.work(),
                storage: budget.storage(),
                peak: budget.peak_storage(),
            })
        })?;
        observation.emit(requested);
        roster.phase.require_clean_v1()?;
        roster.phase.with_budget(|budget| {
            budget
                .check_prior_denials_v1()
                .map_err(Error::ConditionalResource)?;
            budget
                .release_storage(OBSERVATION_STORAGE)
                .map_err(Error::ConditionalResource)
        })?;
        drop(roster);
        // The unchanged outer entry marker follows actual source/account drop
        // and the caller's independent original materialization postflight.
        Ok(())
    }

    #[test]
    fn roster_observation_joins_reject_each_changed_moved_source_field() {
        let bytes = [0u8; 4];
        let good = SourceJoin {
            semantic: [7; 32],
            root: SemanticFunctionIdV1::from_index(2),
            accesses: bytes.as_ptr() as usize,
            access_count: 3,
            ranked_ir: bytes.as_ptr() as usize,
            ranked_bytes: 4,
            permutation: [0, 1, 2, 3],
        };
        assert!(require_same_source(&good, &good, [0, 1, 2, 3]).is_ok());
        for field in 0..7 {
            let mut bad = good;
            match field {
                0 => bad.semantic[0] ^= 1,
                1 => bad.root = SemanticFunctionIdV1::from_index(3),
                2 => bad.accesses ^= 1,
                3 => bad.access_count += 1,
                4 => bad.ranked_ir ^= 1,
                5 => bad.ranked_bytes += 1,
                _ => bad.permutation = [1, 0, 2, 3],
            }
            assert!(require_same_source(&good, &bad, [0, 1, 2, 3]).is_err());
        }
        let swap = SourceJoin {
            permutation: [1, 0, 2, 3],
            ..good
        };
        assert!(require_same_source(&swap, &swap, [1, 0, 2, 3]).is_ok());
        assert!(require_same_source(&swap, &swap, [0, 1, 2, 3]).is_err());
        let unsupported = SourceJoin {
            permutation: [0, 0, 2, 3],
            ..good
        };
        assert!(require_same_source(&unsupported, &unsupported, [0, 0, 2, 3]).is_err());
    }

    #[test]
    fn roster_observation_account_is_not_reconstructed_from_counters() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let mut first = Work::new(100);
        let mut second = Work::new(100);
        let mut a = Budget::new(&mut first, 32);
        let mut b = Budget::new(&mut second, 32);
        a.reserve_storage(16).unwrap();
        b.reserve_storage(16).unwrap();
        let source = SourceJoin {
            semantic: [0; 32],
            root: SemanticFunctionIdV1::from_index(0),
            accesses: 0,
            access_count: 3,
            ranked_ir: 0,
            ranked_bytes: 1,
            permutation: [0, 1, 2, 3],
        };
        let before = Before {
            account: a.work_ledger_identity_v1(),
            storage: 16,
            work: 0,
            source,
        };
        assert!(require_same_account(&before, &mut a).is_ok());
        assert!(matches!(
            require_same_account(&before, &mut b),
            Err(Error::ConditionalResource(Resource::Accounting))
        ));
        a.release_storage(1).unwrap();
        assert!(matches!(
            require_same_account(&before, &mut a),
            Err(Error::ConditionalResource(Resource::Accounting))
        ));
    }

    #[test]
    fn roster_observation_account_gate_preserves_prior_denial_before_its_work() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        let mut work = Work::new(ACCOUNT_JOIN_WORK);
        let mut budget = Budget::new(&mut work, 1);
        budget.charge_work(ACCOUNT_JOIN_WORK).unwrap();
        let original = budget.reserve_storage(2).unwrap_err();
        let before = Before {
            account: budget.work_ledger_identity_v1(),
            storage: 0,
            work: ACCOUNT_JOIN_WORK,
            source: SourceJoin {
                semantic: [0; 32],
                root: SemanticFunctionIdV1::from_index(0),
                accesses: 0,
                access_count: 3,
                ranked_ir: 0,
                ranked_bytes: 1,
                permutation: [0, 1, 2, 3],
            },
        };
        for _ in 0..3 {
            assert!(matches!(require_same_account(&before, &mut budget),
                Err(Error::ConditionalResource(error)) if error == original));
            assert_eq!(budget.work(), ACCOUNT_JOIN_WORK);
            assert!(budget.failed_work().is_none());
            assert_eq!(budget.failed_storage(), Some(2));
        }
    }
}

#[cfg(test)]
pub(super) fn refuse_roster_observation_conversion_fixture_v1(
    program: ProductionRankedSemanticProgramV1,
) -> Result<(), ProductionRankedVerificationErrorV1> {
    roster_observation::consume_and_observe(program, [0, 1, 2, 3])
}
