//! Genuine helper-source CPU observation only; no normal/target gate is bypassed.
use super::*;
use fe2o3_lower_mir_kernel::{
    Bf16CallInstanceEmissionViewV1, ProductionPreRankedKirOwnerV1, ProductionSemanticKirLimitsV1,
};
use std::cell::Cell;
#[path = "bf16_nominal_call_query_v1_tests.rs"]
mod nominal_call_query;
#[path = "bf16_nonempty_read_query_v1_tests.rs"]
mod nonempty_read_query;

#[derive(Clone, Copy)]
enum NominalQueryProfileV1 {
    Historical,
    SingleStridedRead,
    OwnedRoot,
    OwnedRoster { requested_return: [u8; 4] },
    OwnedModule { requested_return: [u8; 4] },
    OwnedAttached { requested_return: [u8; 4] },
    OwnedFormal { requested_return: [u8; 4] },
    OwnedRankedFormal { requested_return: [u8; 4] },
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(crate) struct CpuPhase {
    entry_storage: usize,
    source_storage: usize,
    occurrence_storage: Option<usize>,
    nominal_storage: Option<usize>,
    reverification_peak_storage: Option<usize>,
    phase_peak_storage: usize,
    materialized: bool,
    normal_attempted: bool,
    normal_succeeded: bool,
    final_storage: usize,
    same_ledger: bool,
    work: usize,
    failed_work: bool,
    failed_storage: bool,
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Test-only sibling. The live importer and exact SSA preparation are shared
    /// with the historical five-session transport gate. No second import or
    /// synthetic graph can satisfy the consumer callback.
    pub(crate) fn observe_bf16_call_source_cpu_for_test_v1<R: Copy + 'static>(
        self,
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    ) -> (Result<R, Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::Historical,
            inspect,
        )
    }

    /// Separate explicitly selected source-only observer. It invokes neither the
    /// old empty-only observers nor a numerical CPU callback; normal refusal stays.
    pub(crate) fn observe_bf16_single_strided_read_for_test_v1(
        self,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::SingleStridedRead,
            |_, _, _| Ok(()),
        )
    }

    /// Explicit private owning-entry test. Completes materialization postflight,
    /// then moves the same owner into the real boxed projection phase. This
    /// includes private source/ranked translation validation, but neither ordinary
    /// admission nor lowerer attachment, LLVM or target continuation.
    pub(crate) fn observe_bf16_owned_root_for_test_v1(
        self,
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(NominalQueryProfileV1::OwnedRoot, inspect)
    }

    /// Distinct next-boundary case. The unchanged v2 owning observer above
    /// still ends at Program; this additionally consumes its first real roster.
    pub(crate) fn observe_bf16_owned_roster_for_test_v1(
        self,
        requested_return: [u8; 4],
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::OwnedRoster { requested_return },
            inspect,
        )
    }

    /// Separate Stage A module-roster case. V2 and V3 endpoints remain unchanged;
    /// this consumes and revalidates the actual private module owning pair.
    pub(crate) fn observe_bf16_owned_module_for_test_v1(
        self,
        requested_return: [u8; 4],
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::OwnedModule { requested_return },
            inspect,
        )
    }

    /// Separate Stage B case, retaining and freshly replaying the actual intact
    /// nominal owner/report. No ordinary or legacy Connected attachment occurs.
    pub(crate) fn observe_bf16_owned_attached_for_test_v1(
        self,
        requested_return: [u8; 4],
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::OwnedAttached { requested_return },
            inspect,
        )
    }

    /// Diagnostic-only formal extraction from the actual intact owner. An
    /// incomplete/error analysis can be collected successfully but never admits
    /// formal, ordinary, LLVM or target continuation.
    pub(crate) fn observe_bf16_owned_formal_for_test_v1(
        self,
        requested_return: [u8; 4],
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::OwnedFormal { requested_return },
            inspect,
        )
    }

    /// Fresh actual-owner guard discharge, distinct from raw formal collection.
    pub(crate) fn observe_bf16_owned_ranked_formal_for_test_v1(
        self,
        requested_return: [u8; 4],
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    ) -> (Result<(), Box<ProductionPipelineError>>, Option<CpuPhase>) {
        self.observe_bf16_call_source_profile_for_test_v1(
            NominalQueryProfileV1::OwnedRankedFormal { requested_return },
            inspect,
        )
    }

    fn observe_bf16_call_source_profile_for_test_v1<R: Copy + 'static>(
        self,
        profile: NominalQueryProfileV1,
        inspect: impl for<'a, 'b, 'work> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &Bf16CallInstanceEmissionViewV1<'b>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    ) -> (Result<R, Box<ProductionPipelineError>>, Option<CpuPhase>) {
        let phase = Cell::new(None);
        let result = (|| {
            let (ssa, source_seed) = self.prepare_bf16_tile_values_source_v1()?;
            let prepared = ssa.with_retained_materialization_budget_v1(|prepared, budget| {
                let floor = budget.storage();
                let source_storage = source_seed.phase_storage_bytes();
                let ledger = budget.work_ledger_identity_v1();
                budget.reserve_storage(source_storage)
                    .map_err(materialization_resource_error_v29)?;
                let protected = floor.checked_add(source_storage)
                    .ok_or_else(|| materialization_resource_error_v29(Resource::Arithmetic))?;
                let mut occurrence_storage = None;
                let mut nominal_storage = None;
                let reverification_peak = Cell::new(None);
                // Unlike materialize_prepared_with_budget_v29 this custody core
                // adds NO output reservation. The nominal receipt is reserved
                // exactly once below; preexisting occurrences are separate.
                // Fixed Cell owner is covered by the richer 4096-byte allowance;
                // captured reference is included in the exact size_of<F> debit.
                let consume_input_frame = Cell::new(None);
                let mut actual_input_frame = 0usize;
                let result = consume_prepared_with_actual_inputs_for_test_v1(
                    prepared, budget, &mut actual_input_frame, |_, _| Ok(()),
                    |mut semantic_ssa, launch, _, ranked_inputs, reference_bindings, budget, frame_owned| {
                        // First accepted debit, before materialization or view.
                        assert!(consume_input_frame.replace(Some(*frame_owned)).is_none());
                        if semantic_ssa.occurrence_storage().is_some() {
                            return Err(unavailable("BF16 helper CPU occurrences were not fresh"));
                        }
                        let receipt = semantic_ssa.try_capture_occurrences_with_budget_v1(budget)
                            .map_err(|error| ProductionPipelineError::PreRankedMaterialization(
                                fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Occurrences(error)))?;
                        budget.reserve_storage(receipt.retained_storage())
                            .map_err(materialization_resource_error_v29)?;
                        occurrence_storage = Some(receipt.retained_storage());
                        let owner = ProductionPreRankedKirOwnerV1::try_materialize_bf16_nominal_with_budget_v1(
                            semantic_ssa, launch, ProductionSemanticKirLimitsV1::default(), budget,
                        ).map_err(ProductionPipelineError::PreRankedMaterialization)?;
                        let retained = owner.retained_analysis_storage_v1();
                        budget.reserve_storage(retained).map_err(materialization_resource_error_v29)?;
                        nominal_storage = Some(retained);
                        let before = budget.storage();
                        let observed = (|| {
                            let replay = owner.verify_bf16_nominal_equivalence_with_budget_v1(budget);
                            // The actual same-owner replay keeps the first
                            // retained envelope live while reserving a second.
                            // Never replace this ledger or widen its 2GiB cap.
                            reverification_peak.set(Some(budget.peak_storage()));
                            assert!(budget.peak_storage() <= 2 * 1024 * 1024 * 1024);
                            replay.map_err(ProductionPipelineError::PreRankedMaterialization)?;
                            let emission = owner.bf16_call_instance_emission_v1()
                                .ok_or_else(|| unavailable("actual nominal helper emission absent"))?;
                            fe2o3_lower_mir_kernel::with_checked_bf16_call_instance_v1(
                                owner.semantic_ssa(), budget, |relation, budget| {
                                    if !std::ptr::eq(relation.owner(), owner.semantic_ssa())
                                        || relation.root() != emission.root()
                                        || relation.helper() != emission.helper()
                                        || relation.call_block() != emission.source_call_block()
                                        || relation.return_permutation() != emission.return_permutation()
                                    {
                                        return Err(Error::Unavailable("genuine source/emission owner join"));
                                    }
                                    with_actual_retained_ranked_inputs_for_test_v1(
                                        &owner, ranked_inputs, reference_bindings, budget, frame_owned,
                                        |actual_inputs, budget| match profile {
                                            NominalQueryProfileV1::Historical => nominal_call_query::inspect(
                                                &owner, relation, &actual_inputs, budget,
                                            ),
                                            NominalQueryProfileV1::SingleStridedRead => nonempty_read_query::inspect(
                                                &owner, relation, &actual_inputs, budget,
                                            ),
                                            // The new account/proof/owning constructor runs only
                                            // after this materialization loan's full postflight.
                                            NominalQueryProfileV1::OwnedRoot
                                             | NominalQueryProfileV1::OwnedRoster { .. }
                                             | NominalQueryProfileV1::OwnedModule { .. }
                                             | NominalQueryProfileV1::OwnedAttached { .. }
                                             | NominalQueryProfileV1::OwnedFormal { .. }
                                             | NominalQueryProfileV1::OwnedRankedFormal { .. } => Ok(()),
                                        },
                                    )?;
                                    source_seed.with_relation(relation, budget, |source, budget| {
                                        inspect(source, &emission, budget)
                                    })
                                },
                            ).map_err(inspection)
                        })();
                        match observed {
                            Ok(value) => Ok((owner, value)),
                            Err(error) => {
                                // The borrowed checker catches callback unwinds.
                                // Drop the exact owner before its own receipt;
                                // callback-owned charges and work survive.
                                drop(owner);
                                if budget.work_ledger_identity_v1() != ledger || budget.storage() < before {
                                    return Err(materialization_resource_error_v29(Resource::Accounting));
                                }
                                budget.release_storage(retained).map_err(materialization_resource_error_v29)?;
                                Err(error)
                            }
                        }
                    },
                );
                if result.is_err() {
                    if let Some(bytes) = occurrence_storage {
                        let required = protected.checked_add(bytes)
                            .ok_or_else(|| materialization_resource_error_v29(Resource::Arithmetic))?;
                        if budget.work_ledger_identity_v1() != ledger || budget.storage() < required {
                            drop(result);
                            return Err(Box::new(materialization_resource_error_v29(Resource::Accounting)));
                        }
                        // The consumed SSA/nominal owner has already dropped.
                        budget.release_storage(bytes).map_err(materialization_resource_error_v29)?;
                    }
                }
                // All richer handoff callback/borrow frames have ended. The
                // returned owner keeps its separately reserved payload receipts.
                if budget.work_ledger_identity_v1() != ledger
                    || budget.storage() < protected.checked_add(actual_input_frame)
                        .ok_or_else(|| materialization_resource_error_v29(Resource::Arithmetic))?
                {
                    drop(result);
                    return Err(Box::new(materialization_resource_error_v29(Resource::Accounting)));
                }
                // Both accepted frame debits charge exactly their byte counts.
                // Callbacks have ended and original custody was checked above.
                if let Some(consume) = consume_input_frame.get() {
                    let view = actual_input_frame.checked_sub(consume)
                        .ok_or_else(|| materialization_resource_error_v29(Resource::Accounting))?;
                    eprintln!("fe2o3-root-prefix-handoff-v1 work={} storage={} consume={} view={}",
                        actual_input_frame, actual_input_frame, consume, view);
                } else {
                    // Pre-callback failure keeps its original error and has no
                    // completed actual-handoff observation.
                    assert!(result.is_err());
                }
                drop(consume_input_frame);
                budget.release_storage(actual_input_frame).map_err(materialization_resource_error_v29)?;
                drop(source_seed);
                if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
                    drop(result);
                    return Err(Box::new(materialization_resource_error_v29(Resource::Accounting)));
                }
                budget.release_storage(source_storage).map_err(materialization_resource_error_v29)?;
                assert!(phase.replace(Some(CpuPhase {
                    entry_storage: floor, source_storage, occurrence_storage, nominal_storage,
                    reverification_peak_storage: reverification_peak.get(),
                    phase_peak_storage: budget.peak_storage(),
                    materialized: result.is_ok(), normal_attempted: false, normal_succeeded: false,
                    final_storage: budget.storage(), same_ledger: budget.work_ledger_identity_v1() == ledger,
                    work: budget.work(), failed_work: budget.failed_work().is_some(),
                    failed_storage: budget.failed_storage().is_some(),
                })).is_none());
                result
            })?;
            // Keep the actual same account after the source callback returns.
            // This entry is opt-in and test-only; the default stack-backed
            // preparation helper and ordinary nominal refusal are unchanged.
            prepared
                .try_map(|prepared, budget| {
                    let before = phase.get().expect("successful nominal phase");
                    assert_eq!(budget.work(), before.work);
                    assert_eq!(budget.storage(), before.final_storage);
                    assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                    assert!(!budget.failed_work().is_some());
                    assert!(!budget.failed_storage().is_some());
                    let PreparedMaterializationV29 {
                        materialized: (materialized, observed),
                        ranked_roots,
                        bindings,
                    } = prepared;
                    if matches!(profile, NominalQueryProfileV1::OwnedRoot | NominalQueryProfileV1::OwnedRoster { .. } | NominalQueryProfileV1::OwnedModule { .. } | NominalQueryProfileV1::OwnedAttached { .. } | NominalQueryProfileV1::OwnedFormal { .. } | NominalQueryProfileV1::OwnedRankedFormal { .. }) {
                        // The ORIGINAL materialization Box is still alive here.
                        // Projection acquires its own original Box before any
                        // inventory/facts/proof; both accounts keep old caps.
                        let checked = MaterializedNeutralProductionCompilation {
                            materialized, ranked_roots, bindings,
                        }.verify_private_nominal_kernel_checks_v1().map_err(Box::new)?;
                        assert_eq!(budget.work(), before.work);
                        assert_eq!(budget.storage(), before.final_storage);
                        assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                        budget.check_prior_denials_v1()
                            .map_err(materialization_resource_error_v29)?;
                        if let NominalQueryProfileV1::OwnedRankedFormal { requested_return } = profile {
                            let RankedVerifiedProductionCompilation { ranked, bindings } = checked;
                            ranked.observe_private_nominal_ranked_formal_for_test_v1(requested_return)
                                .map_err(ProductionPipelineError::RankedVerification)
                                .map_err(Box::new)?;
                            drop(bindings);
                            // Formal attempt and actual intact source/report/projection pair dropped.
                            // The original materialization account is still retained.
                            assert_eq!(budget.work(), before.work);
                            assert_eq!(budget.storage(), before.final_storage);
                            assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                            budget.check_prior_denials_v1()
                                .map_err(materialization_resource_error_v29)?;
                        } else if let NominalQueryProfileV1::OwnedFormal { requested_return } = profile {
                            let RankedVerifiedProductionCompilation { ranked, bindings } = checked;
                            ranked.observe_private_nominal_formal_for_test_v1(requested_return)
                                .map_err(ProductionPipelineError::RankedVerification)
                                .map_err(Box::new)?;
                            drop(bindings);
                            // Formal attempt and actual intact source/report/projection pair dropped.
                            // The original materialization account is still retained.
                            assert_eq!(budget.work(), before.work);
                            assert_eq!(budget.storage(), before.final_storage);
                            assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                            budget.check_prior_denials_v1()
                                .map_err(materialization_resource_error_v29)?;
                        } else if let NominalQueryProfileV1::OwnedAttached { requested_return } = profile {
                            let RankedVerifiedProductionCompilation { ranked, bindings } = checked;
                            ranked.observe_private_nominal_attached_for_test_v1(requested_return)
                                .map_err(ProductionPipelineError::RankedVerification)
                                .map_err(Box::new)?;
                            drop(bindings);
                            // The actual intact source/report/projection pair has dropped.
                            // The original materialization account is still retained.
                            assert_eq!(budget.work(), before.work);
                            assert_eq!(budget.storage(), before.final_storage);
                            assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                            budget.check_prior_denials_v1()
                                .map_err(materialization_resource_error_v29)?;
                        } else if let NominalQueryProfileV1::OwnedModule { requested_return } = profile {
                            let RankedVerifiedProductionCompilation { ranked, bindings } = checked;
                            ranked.observe_private_nominal_module_for_test_v1(requested_return)
                                .map_err(ProductionPipelineError::RankedVerification)
                                .map_err(Box::new)?;
                            drop(bindings);
                            // The actual module/source/projection pair has dropped.
                            // The original materialization account is still retained.
                            assert_eq!(budget.work(), before.work);
                            assert_eq!(budget.storage(), before.final_storage);
                            assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                            budget.check_prior_denials_v1()
                                .map_err(materialization_resource_error_v29)?;
                        } else if let NominalQueryProfileV1::OwnedRoster { requested_return } = profile {
                            let RankedVerifiedProductionCompilation { ranked, bindings } = checked;
                            ranked.observe_private_nominal_roster_for_test_v1(requested_return)
                                .map_err(ProductionPipelineError::RankedVerification)
                                .map_err(Box::new)?;
                            drop(bindings);
                            // The roster/source/projection account have dropped;
                            // original materialization custody is STILL retained.
                            assert_eq!(budget.work(), before.work);
                            assert_eq!(budget.storage(), before.final_storage);
                            assert_eq!(budget.peak_storage(), before.phase_peak_storage);
                            budget.check_prior_denials_v1()
                                .map_err(materialization_resource_error_v29)?;
                        } else {
                            drop(checked);
                        }
                        eprintln!("fe2o3-bf16-private-owning-entry-v1 completed=true materialization_account_unchanged=true normal_admission=false target=false");
                        return Ok(observed);
                    }
                    // The original account stays live while the same nominal
                    // owner enters the unchanged refusing normal consumer.
                    let normal = MaterializedNeutralProductionCompilation {
                        materialized,
                        ranked_roots,
                        bindings,
                    }
                    .verify_general_kernel_checks();
                    let mut row = before;
                    row.normal_attempted = true;
                    row.normal_succeeded = normal.is_ok();
                    phase.set(Some(row));
                    assert_eq!(budget.work(), before.work);
                    assert_eq!(budget.storage(), before.final_storage);
                    eprintln!(
                        "fe2o3-bf16-retained-source-account-v1 work={} storage={} normal_ok={}",
                        budget.work(),
                        budget.storage(),
                        normal.is_ok(),
                    );
                    normal
                        .map(|checked| {
                            drop(checked);
                            observed
                        })
                        .map_err(Box::new)
                })
                .map(|finished| finished.finish_copy())
        })();
        (result, phase.get())
    }
}
