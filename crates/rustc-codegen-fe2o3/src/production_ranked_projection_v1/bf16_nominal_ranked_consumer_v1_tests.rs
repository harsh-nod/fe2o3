//! Synthetic container/resource controls plus the real mandatory Pliron engine.
//! These values are not authenticated frontend/source-owner fixtures.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_pliron::ProductionSessionErrorV1;

fn small() -> (Analysis, Snapshot) {
    (
        Analysis::new(20, 32).unwrap(),
        Snapshot::new(128, 30).unwrap(),
    )
}
fn fixture(index: u64) -> emission::Emitted {
    let mut out = emission::Emitted::empty();
    let id = ProductionRankedValueIdV1::new;
    out.blocks.push(ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: 1,
                global_extents: [1, 1, 1],
                workgroup_extents: [1, 1, 1],
                subgroup_size: 1,
                full_physical_workgroups: true,
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: id(0),
                element_width: 32,
                writable: false,
                shape: vec![1],
                dynamic_extents: vec![],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 1,
                noalias_class: 1,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: id(1),
                value: index,
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view: ProductionRankedValueV1::Local(id(0)),
                indices: vec![ProductionRankedValueV1::Local(id(1))],
            },
        ],
        ProductionRankedTerminatorV1::Return,
    ));
    out.complete = true;
    out
}
#[test]
fn nominal_compile_selected_prepayment_exact_and_one_short() {
    let (a, s) = small();
    for (w, p, ok) in [(50, 39, true), (49, 39, false), (50, 38, false)] {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, p);
        budget.reserve_storage(7).unwrap();
        let mut owned = 0;
        let mut pending = Pending::new();
        let result = pay(
            &mut pending,
            a,
            s,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert_eq!(result.is_ok(), ok);
        assert_eq!(pending.phase, Phase::Entered);
        if ok {
            assert_eq!(budget.work(), 50);
            assert_eq!(budget.storage(), 39);
            assert_eq!(owned, 32);
        } else {
            assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
        }
    }
}
#[test]
fn nominal_compile_prepayment_never_retries_or_accepts_unmetered_context() {
    let (a, s) = small();
    let mut pending = Pending::new();
    assert!(pay(&mut pending, a, s, &mut PreparationResourcesV1::unmetered()).is_err());
    assert_eq!(pending.phase, Phase::Fresh);
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1000);
    let mut owned = 0;
    let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
    pay(&mut pending, a, s, &mut meter).unwrap();
    assert!(pay(&mut pending, a, s, &mut meter).is_err());
}
#[test]
fn nominal_compile_operand_domain_is_original_extent_argument_only() {
    assert!(argument(ProductionRankedValueV1::Argument(0)).is_ok());
    assert!(argument(ProductionRankedValueV1::Argument(1)).is_err());
    assert!(argument(ProductionRankedValueV1::Argument(u32::MAX)).is_err());
    assert!(
        argument(ProductionRankedValueV1::Local(
            ProductionRankedValueIdV1::new(7)
        ))
        .is_ok()
    );
}
#[test]
fn nominal_compile_lookup_work_is_checked_and_covers_three_linear_scans() {
    assert_eq!(lookup_work(0).unwrap(), 64);
    assert_eq!(lookup_work(4096).unwrap(), 64 + 12 * 4096);
    assert!(lookup_work(usize::MAX).is_err());
}
#[test]
fn nominal_compile_incomplete_payload_refuses_before_vec_move() {
    let mut out = fixture(0);
    out.complete = false;
    let mut pending = Pending::new();
    let mut work = Work::new(10000);
    let mut budget = Budget::new(&mut work, 10000);
    let mut owned = 0;
    assert!(
        validate_payload(
            &out,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned)
        )
        .is_err()
    );
    assert_eq!(out.blocks.len(), 1);
    assert!(pending.lowering.is_empty());
}
#[test]
fn nominal_compile_invented_uniform_control_is_refused() {
    let mut out = fixture(0);
    out.blocks[0] = ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::AnalysisSplit {
            control_dependencies: vec![ProductionRankedValueV1::Argument(0)],
            first_block: 0,
            second_block: 0,
        },
    );
    let mut pending = Pending::new();
    // No operations: entry, complete closed control census, then block.
    let required = 64usize
        .checked_add(fixture_control_validation_work())
        .unwrap()
        .checked_add(32)
        .unwrap();
    assert_eq!(required, 25312);
    let mut work = Work::new(required);
    let mut budget = Budget::new(&mut work, size_of::<usize>());
    let mut owned = 0;
    let result = validate_payload(
        &out,
        &mut pending,
        &mut PreparationResourcesV1::new(&mut budget, &mut owned),
    );
    assert!(matches!(result, Err(Error::Incomplete(reason))
        if reason == "nominal verifier would invent uniform control"));
    assert_eq!(budget.work(), required);
    assert_eq!(budget.failed_work(), None);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.storage(), owned);
    assert_eq!(owned, size_of::<usize>());
    drop(pending);
    drop(out);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
fn engine(index: u64, a: Analysis) -> (Phase, Result<()>, bool) {
    let s = Snapshot::production_hard_ceiling();
    let work_limit = a
        .max_work()
        .checked_add(s.max_work())
        .unwrap()
        .checked_add(1_000_000)
        .unwrap();
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, a.max_peak_storage() + 1_000_000);
    let ledger = budget.work_ledger_identity_v1();
    let mut owned = 0;
    let mut pending = Pending::new();
    let mut out = fixture(index);
    let result = {
        let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
        pay(&mut pending, a, s, &mut meter).unwrap();
        validate_payload(&out, &mut pending, &mut meter).unwrap();
        compile_payload(
            "nominal_consumer_control",
            &mut out,
            &mut pending,
            a,
            s,
            &mut meter,
        )
    };
    assert!(budget.work_ledger_identity_v1() == ledger);
    // This test keeps the Budget alive until the actual pending session drops.
    let consumed = out.blocks.is_empty();
    assert_eq!(pending.lowering.capacity(), 1);
    assert!(owned >= size_of::<ProductionRankedKernelLoweringInputV1>());
    if !pending.lowering.is_empty() {
        assert!(
            pending
                .lowering
                .first()
                .unwrap()
                .all_mandatory_reports_are_clean()
        );
        assert_eq!(budget.storage(), owned);
    }
    // Return only copied diagnostics/state without exporting a live session.
    let phase = pending.phase;
    drop(pending);
    drop(out);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
    (phase, result, consumed)
}
#[test]
fn nominal_compile_actual_closed_engine_consumes_exact_recipe_and_retains_session() {
    let (phase, result, consumed) = engine(0, Analysis::production_hard_ceiling());
    result.unwrap();
    assert!(consumed);
    assert_eq!(phase, Phase::Verified);
}
#[test]
fn nominal_compile_actual_bounds_refusal_is_not_a_success_or_fallback() {
    let (phase, result, consumed) = engine(1, Analysis::production_hard_ceiling());
    assert!(
        matches!(result,Err(Error::Compile{error,..}) if matches!(*error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::RankedBounds(_))))
    );
    assert!(consumed);
    assert_eq!(phase, Phase::Compiling);
}
#[test]
fn nominal_compile_zero_analysis_has_actual_typed_resource_refusal() {
    let (phase, result, consumed) = engine(0, Analysis::new(0, 0).unwrap());
    assert!(
        matches!(result,Err(Error::Compile{error,..}) if matches!(*error,
        ProductionRankedCompileErrorV1::Session(ProductionSessionErrorV1::AnalysisResourceLimit{..})))
    );
    assert!(consumed);
    assert_eq!(phase, Phase::Compiling);
}
// The one-block/four-operation fixture retains no checked-view sites, but
// validating that absence still prepays the complete closed namespace scans.
// Vec growth relocates zero existing rows and retains exactly one usize.
fn fixture_control_validation_work() -> usize {
    let namespace = 64usize
        .checked_add(
            MAX_BLOCKS
                .checked_mul(MAX_BLOCKS)
                .unwrap()
                .checked_mul(24)
                .unwrap(),
        )
        .unwrap();
    let rows = 64usize
        .checked_add(MAX_BLOCKS.checked_mul(16).unwrap())
        .unwrap();
    namespace.checked_add(rows).unwrap()
}
fn fixture_payload_validation_work() -> usize {
    // Entry, namespace, block, four operations, and two access operands.
    [64, fixture_control_validation_work(), 32, 4 * 32, 2 * 4]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .unwrap()
}
fn fixture_transformed_validation_work() -> usize {
    // Entry, same closed namespace, and one operation-count comparison.
    [32, fixture_control_validation_work(), 8]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .unwrap()
}

#[test]
fn nominal_compile_payload_fixture_exact_and_one_short_preserve_original_account() {
    let required = fixture_payload_validation_work();
    assert_eq!(required, 25448);
    for (available, succeeds) in [(required, true), (required - 1, false)] {
        let out = fixture(0);
        let mut pending = Pending::new();
        let mut work = Work::new(11 + available);
        let mut budget = Budget::new(&mut work, 7 + size_of::<usize>());
        budget.charge_work(11).unwrap();
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut owned = 0;
        let result = validate_payload(
            &out,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        if succeeds {
            result.unwrap();
            assert_eq!(budget.work(), 11 + required);
            assert_eq!(budget.failed_work(), None);
        } else {
            assert!(matches!(result,
                Err(Error::CanonicalAssertions(
                    crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error))
                )) if error.actual() == 11 + required && error.limit() == 11 + available
            ));
            assert_eq!(budget.work(), 11 + required - 8);
            assert_eq!(budget.failed_work(), Some(11 + required));
            assert!(budget.check_prior_denials_v1().is_err());
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(pending.operation_counts, [4]);
        assert_eq!(owned, size_of::<usize>());
        assert_eq!(budget.storage(), 7 + owned);
        assert_eq!(budget.failed_storage(), None);
        drop(pending);
        drop(out);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 7);
        assert_eq!(budget.failed_work(), (!succeeds).then_some(11 + required));
    }
}

#[test]
fn nominal_compile_transformed_coordinate_census_is_not_reported_boolean_authority() {
    let required = fixture_payload_validation_work()
        .checked_add(fixture_transformed_validation_work())
        .unwrap();
    assert_eq!(required, 50704);
    for (available, reaches_coordinate_check) in [(required, true), (required - 1, false)] {
        let out = fixture(0);
        let mut pending = Pending::new();
        let mut work = Work::new(available);
        let mut budget = Budget::new(&mut work, size_of::<usize>());
        let ledger = budget.work_ledger_identity_v1();
        let mut owned = 0;
        let kernel = ProductionRankedKernelV1::new(
            "control",
            1,
            vec![ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let result = {
            let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
            validate_payload(&out, &mut pending, &mut meter).unwrap();
            check_transformed(&kernel, &out, &pending, &mut meter)
        };
        if reaches_coordinate_check {
            // A work denial must never satisfy this semantic-refusal control.
            assert!(matches!(result, Err(Error::Incomplete(reason))
                if reason == "nominal constructor changed an operation coordinate"));
            assert_eq!(budget.work(), required);
            assert_eq!(budget.failed_work(), None);
        } else {
            assert!(matches!(result,
                Err(Error::CanonicalAssertions(
                    crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error))
                )) if error.actual() == required && error.limit() == available
            ));
            assert_eq!(budget.work(), required - 8);
            assert_eq!(budget.failed_work(), Some(required));
            assert!(budget.check_prior_denials_v1().is_err());
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), owned);
        assert_eq!(owned, size_of::<usize>());
        assert_eq!(budget.failed_storage(), None);
        drop(kernel);
        drop(pending);
        drop(out);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!(
            budget.failed_work(),
            (!reaches_coordinate_check).then_some(required)
        );
    }
}
#[test]
fn nominal_compile_selected_genuine_two_case_profile_fits_existing_source_work_ceiling() {
    let a = Analysis::production_hard_ceiling();
    let s = Snapshot::production_hard_ceiling();
    // Positive pays analysis+snapshot; the independent refusal pays snapshot.
    let work = a
        .max_work()
        .checked_add(s.max_work().checked_mul(2).unwrap())
        .unwrap();
    assert!(
        work < usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap()
    );
    assert!(a.max_peak_storage() < crate::production_canonical_phase_policy_v1::STORAGE_LIMIT);
}

#[test]
fn nominal_compile_lowering_slot_is_prepaid_exactly_before_allocation() {
    let bytes = size_of::<ProductionRankedKernelLoweringInputV1>();
    assert!(bytes > 0);
    for (limit, succeeds) in [(7 + bytes, true), (7 + bytes - 1, false)] {
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(7).unwrap();
        let mut owned = 0;
        let mut pending = Pending::new();
        pending.phase = Phase::Entered; // Synthetic phase/owner-layout control.
        let result = enter_compiler(
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(pending.phase, Phase::Compiling);
        assert!(pending.lowering.is_empty());
        if succeeds {
            assert_eq!(pending.lowering.capacity(), 1);
            assert_eq!(owned, bytes);
            assert_eq!(budget.storage(), 7 + bytes);
        } else {
            assert_eq!(pending.lowering.capacity(), 0);
            assert_eq!(owned, 0);
            assert_eq!(budget.storage(), 7);
            assert!(budget.failed_storage().is_some());
        }
        // Refusal/success are both one-shot; no already-paid slot is reused.
        assert!(
            enter_compiler(
                &mut pending,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn nominal_compile_lowering_slot_refuses_unmetered_before_allocation() {
    let mut pending = Pending::new();
    pending.phase = Phase::Entered;
    assert!(enter_compiler(&mut pending, &mut PreparationResourcesV1::unmetered()).is_err());
    assert_eq!(pending.phase, Phase::Entered);
    assert_eq!(pending.lowering.capacity(), 0);
    assert!(pending.lowering.is_empty());
}

// Inert semantic model data plus real Budget/shared-row calls. These fixtures do
// not impersonate an authenticated canonical owner or admitted frontend.
fn correspondence_function() -> SemanticFunctionDeclV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let ty = SemanticTypeIdV1::from_index(0);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([10; 32]),
        SemanticLayoutIdentityV1::from_sha256([10; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let local = SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([20; 32]),
        ty,
        SemanticLocalRoleV1::Return,
        provenance,
    );
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([80; 32]),
        provenance,
        vec![],
        SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([11; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([12; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([13; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([14; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([15; 32]),
        provenance,
        abi,
        vec![local],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
}
fn correspondence_stream() -> (Vec<ProductionRankedBlockV1>, [ProjectedAccessSourceV1; 3]) {
    let blocks = vec![ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view: ProductionRankedValueV1::Argument(0),
                indices: vec![],
            },
            ProductionRankedOperationV1::AllocationEffect {
                kind: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 1,
                noalias_class: 1,
            },
            ProductionRankedOperationV1::AllocationEffect {
                kind: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 2,
                noalias_class: 2,
            },
        ],
        ProductionRankedTerminatorV1::Return,
    )];
    let source = ProjectedAccessSourceV1 {
        block: 0,
        operation: 0,
        access: AccessKindAttr::Read,
        memory_space: MemorySpaceAttr::Private,
        source: SemanticSourceProvenanceV1::unavailable(),
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block: 0,
            statement: None,
        }),
        output_extent: None,
    };
    (
        blocks,
        [
            source,
            ProjectedAccessSourceV1 {
                operation: 1,
                memory_space: MemorySpaceAttr::Global,
                ..source
            },
            ProjectedAccessSourceV1 {
                operation: 2,
                memory_space: MemorySpaceAttr::Global,
                ..source
            },
        ],
    )
}
struct CorrespondenceFacts<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for CorrespondenceFacts<'_, '_> {
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        Err(Error::Incomplete(
            "inert facts must not supply canonical initializer",
        ))
    }
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0
            .charge_work(amount)
            .map_err(crate::production_ranked_projection_v1::ranked_projection_source_v1::resource)
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        Err(Error::Incomplete(
            "inert facts must not supply canonical materialization",
        ))
    }
    fn condition(&mut self, _: usize, _: bool, _: SemanticBlockIdV1)
        -> Result<crate::production_ranked_projection_v1::canonical_assertion_facts_v1::ProjectedAssertionConditionV1>
    {
        Err(Error::Incomplete(
            "inert facts must not supply canonical assertion",
        ))
    }
}
#[test]
fn nominal_map_private_none_preserves_operations_and_retained_order_and_ordinals() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    let original_sources = sources;
    let mut pending = Pending::new();
    let mut work = Work::new(10000);
    let mut budget = Budget::new(&mut work, 10000);
    let mut owned = 0;
    {
        let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
        meter.reserve(&mut pending.ordinals, sources.len()).unwrap();
        meter.reserve(&mut pending.accesses, sources.len()).unwrap();
    }
    let identity = budget.work_ledger_identity_v1();
    for source in &sources {
        budget
            .charge_work(lookup_work(pending.ordinals.len()).unwrap())
            .unwrap();
        let result = ranked_access_source_row_v1::row(
            &[],
            &function,
            &blocks,
            source,
            |site| {
                pending
                    .ordinals
                    .iter()
                    .find(|(key, _)| *key == site)
                    .map(|(_, n)| *n)
            },
            &mut CorrespondenceFacts(&mut budget),
        );
        assert!(budget.failed_work().is_none());
        record_access_result(&function, &blocks, source, result, &mut pending).unwrap();
    }
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(pending.unmapped_private_reads, 1);
    assert_eq!(
        pending.accesses,
        vec![
            ProductionRankedAccessSourceV1::new(0, None, 0, 0, 1),
            ProductionRankedAccessSourceV1::new(0, None, 1, 0, 2),
        ]
    );
    assert_eq!(
        pending.ordinals,
        vec![(sources[0].semantic_site.unwrap(), 2)]
    );
    assert_eq!(sources, original_sources);
    assert_eq!(blocks[0].operations().len(), 3);
    assert!(matches!(
        blocks[0].operations()[0],
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            ..
        }
    ));
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn nominal_map_private_predicate_exact_and_one_short_work_preserve_denial() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    for (limit, succeeds) in [(48, true), (47, false)] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let mut pending = Pending::new();
        let result = ranked_access_source_row_v1::row(
            &[],
            &function,
            &blocks,
            &sources[0],
            |_| None,
            &mut CorrespondenceFacts(&mut budget),
        );
        if succeeds {
            assert!(matches!(result, Ok(None)));
            record_access_result(&function, &blocks, &sources[0], result, &mut pending).unwrap();
            assert_eq!(pending.unmapped_private_reads, 1);
            assert_eq!(budget.work(), 48);
            assert!(budget.failed_work().is_none());
        } else {
            assert!(
                record_access_result(&function, &blocks, &sources[0], result, &mut pending)
                    .is_err()
            );
            assert!(budget.failed_work().is_some());
            assert_eq!(pending.unmapped_private_reads, 0);
            assert!(pending.ordinals.is_empty() && pending.accesses.is_empty());
        }
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn nominal_map_none_refuses_nonprivate_write_or_wrong_actual_operation() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    for source in [
        ProjectedAccessSourceV1 {
            memory_space: MemorySpaceAttr::Global,
            ..sources[0]
        },
        ProjectedAccessSourceV1 {
            access: AccessKindAttr::Write,
            ..sources[0]
        },
        ProjectedAccessSourceV1 {
            operation: 1,
            ..sources[0]
        },
    ] {
        let mut pending = Pending::new();
        assert!(record_access_result(&function, &blocks, &source, Ok(None), &mut pending).is_err());
        assert_eq!(pending.unmapped_private_reads, 0);
        assert!(pending.ordinals.is_empty() && pending.accesses.is_empty());
    }
}
#[test]
fn nominal_map_none_refuses_missing_and_out_of_range_source_coordinates() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    for source in [
        ProjectedAccessSourceV1 {
            semantic_site: None,
            ..sources[0]
        },
        ProjectedAccessSourceV1 {
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 1,
                statement: None,
            }),
            ..sources[0]
        },
        ProjectedAccessSourceV1 {
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 0,
                statement: Some(0),
            }),
            ..sources[0]
        },
        ProjectedAccessSourceV1 {
            block: 1,
            ..sources[0]
        },
        ProjectedAccessSourceV1 {
            operation: 3,
            ..sources[0]
        },
    ] {
        let mut pending = Pending::new();
        assert!(record_access_result(&function, &blocks, &source, Ok(None), &mut pending).is_err());
        assert_eq!(pending.unmapped_private_reads, 0);
    }
}
#[test]
fn nominal_map_shared_row_error_is_not_reclassified_as_private_none() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    let source = ProjectedAccessSourceV1 {
        operation: usize::MAX,
        ..sources[0]
    };
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 0);
    let result = ranked_access_source_row_v1::row(
        &[],
        &function,
        &blocks,
        &source,
        |_| None,
        &mut CorrespondenceFacts(&mut budget),
    );
    let mut pending = Pending::new();
    assert!(matches!(
        record_access_result(&function, &blocks, &source, result, &mut pending),
        Err(Error::Unsupported(
            "ranked access correspondence is outside the projected graph"
        ))
    ));
    assert_eq!(pending.unmapped_private_reads, 0);
    assert!(pending.ordinals.is_empty() && pending.accesses.is_empty());
}
#[test]
fn nominal_map_private_none_counter_overflow_refuses_without_new_row() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    let mut pending = Pending::new();
    pending.unmapped_private_reads = usize::MAX;
    assert!(record_access_result(&function, &blocks, &sources[0], Ok(None), &mut pending).is_err());
    assert_eq!(pending.unmapped_private_reads, usize::MAX);
    assert!(pending.ordinals.is_empty() && pending.accesses.is_empty());
}
#[test]
fn nominal_map_some_private_row_is_retained_and_ordinal_overflow_is_unchanged() {
    let function = correspondence_function();
    let (blocks, sources) = correspondence_stream();
    let row = ProductionRankedAccessSourceV1::new(0, None, 0, 0, 0);
    let mut pending = Pending::new();
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 10000);
    let mut owned = 0;
    {
        let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
        meter.reserve(&mut pending.ordinals, 1).unwrap();
        meter.reserve(&mut pending.accesses, 1).unwrap();
    }
    // Result injection is only a component control, not a claimed source proof.
    record_access_result(&function, &blocks, &sources[0], Ok(Some(row)), &mut pending).unwrap();
    assert_eq!(pending.unmapped_private_reads, 0);
    assert_eq!(pending.accesses, vec![row]);
    pending.ordinals[0].1 = u32::MAX;
    assert!(matches!(
        record_access_result(&function, &blocks, &sources[0], Ok(Some(row)), &mut pending),
        Err(Error::Unsupported("semantic access ordinal overflow"))
    ));
    assert_eq!(pending.accesses, vec![row]);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
