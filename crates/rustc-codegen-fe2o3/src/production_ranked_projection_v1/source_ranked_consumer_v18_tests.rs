use super::*;

include!("source_entry_consumer_v18_tests.rs");

// Called only from the actual pinned-rustc Prepared-source test driver. The
// small recipe is an inert private-symbol namespace census, not a second
// executable program or a claim of ranked/effect/allocation correspondence.
pub(crate) fn inspect_actual_source_scalar_consumer_v18(
    view: &fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>,
    budget: &mut Budget<'_>,
    hostile: u8,
) -> Result<usize, fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18> {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as Source;
    let original = view.source_semantic(budget)?;
    let graph = view.canonical(budget)?;
    let (_, physical) = view.root(0, budget)?;
    let function = &graph.module().functions[physical];
    let namespace = fe2o3_pliron::ProductionRankedKernelV1::new(
        function.id.as_str(),
        function.signature.parameters.len(),
        vec![fe2o3_pliron::ProductionRankedBlockV1::new(
            vec![], fe2o3_pliron::ProductionRankedTerminatorV1::Return,
        )],
    ).unwrap();
    let checked = view.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
        view.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
            relation.with_scalar_leaves_v18(0, &namespace, budget, |leaves, budget| {
                let count = source_ranked_consumer_v18::check_source_scalar_stores_v18(original, leaves, budget)?;
                assert!(count > 0, "actual resolver-to-Store relation must run, not an empty census");
                match hostile {
                    0 => {}
                    1 => {
                        let substituted = fe2o3_mir_model::semantic_mir_v1::InertSemanticMirRequestV1::new_with_callables(
                            original.target(), original.types().to_vec(), original.allocations().to_vec(),
                            original.statics().to_vec(), original.vtables().to_vec(),
                            original.functions().to_vec(), original.callables().to_vec(), original.roots().to_vec(),
                        ).unwrap().admit_current_production(
                            fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default(),
                        ).unwrap();
                        assert_eq!(substituted.wire_version(), original.wire_version());
                        assert_eq!(substituted.semantic_sha256(), original.semantic_sha256());
                        assert!(!std::ptr::eq(&substituted, original));
                        let error = source_ranked_consumer_v18::check_source_scalar_stores_v18(
                            &substituted, leaves, budget,
                        ).unwrap_err();
                        assert!(matches!(error, ProductionRankedProjectionErrorV1::Incomplete(
                            "source scalar original declaration is foreign to semantic owner"
                        )), "same-content foreign source is not the retained original declaration: {error:?}");
                    }
                    2 => {
                        let visited = std::cell::Cell::new(false);
                        let failure = leaves.visit_store_inputs(budget, |request, budget| -> Result<(), ProductionRankedProjectionErrorV1> {
                            let scalar = request.scalar(budget)?;
                            visited.set(true);
                            // The fixture's actual value is a root-dependent
                            // u32 expression or constant 13, never this value.
                            request.check_expression(&ProductionSemanticExpressionV2::Constant {
                                scalar, bits: 0x5a17_c3e9,
                            }, budget).map_err(Into::into)
                        }).unwrap_err();
                        assert!(visited.get());
                        assert!(matches!(&failure, ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::SourceOwned(Source::Binding(
                                "actual scalar expression differs from its original source value"
                            )))));
                        return Err(failure);
                    }
                    _ => panic!("unknown actual-source scalar consumer test mode"),
                }
                assert!(std::ptr::eq(graph, view.canonical(budget)?));
                Ok(count)
            })
        })
    }));
    match checked {
        Ok(count) => Ok(count),
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::SourceOwned(error))) => Err(error),
        Err(error) => panic!("actual backend resolver/source-Store consumer failed: {error:?}"),
    }
}
use crate::reference_effect_v1::*;
use crate::reference_effect_v1::reference_signature_preimage_v1::{
    ReferenceReturnShapeV1, ReferenceSignatureInputV1,
};
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticExternAbiV1, SemanticFunctionSafetyV1};
use std::mem::size_of;

fn binding_expression(bits: u128) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::U32, bits,
    })
}

fn binding_operand(bits: u128) -> ReferenceOperandV1 {
    ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::U32, bits,
    })
}

fn binding_identity() -> ReferenceFunctionIdentityV1 {
    ReferenceFunctionIdentityV1 {
        def_path_hash: [1; 16], function_sha256: [2; 32],
        item_definition_sha256: [3; 32], monomorphization_sha256: [4; 32],
        generic_type_arguments_sha256: [5; 32], const_generic_arguments_sha256: [6; 32],
        rustc_mir_body_sha256: [7; 32],
    }
}

// Inert payload for allocation accounting, not an authenticated source producer.
fn binding_payload(nested: bool) -> AuthenticatedReferenceEffectBindingV1 {
    let signature = ReferenceLogicalSignaturePreimageV1::new(
        vec![ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::U32)].into_boxed_slice(),
        vec![ReferenceSignatureInputV1::Scalar(ReferenceScalarTypeV1::U32)].into_boxed_slice(),
        ReferenceReturnShapeV1::Unit, SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe, false,
    ).unwrap();
    let mut binding = AuthenticatedReferenceEffectBindingV1 {
        registration_path: "r".into(), logical_kernel_name: "k".into(),
        kernel: binding_identity(), reference: binding_identity(),
        signature_preimage: signature, effect_ir_sha256: [8; 32],
        effect_ir: ReferenceEffectIrV1 {
            argument_count: 1, local_count: 2,
            relations: vec![ReferenceArgumentRelationV1::ScalarInput {
                argument: 0, scalar: ReferenceScalarTypeV1::U32,
            }].into_boxed_slice(),
            blocks: Box::default(), loop_summaries: Box::default(),
            observable_output_effects: Box::default(),
        }, observable_output_writes: Box::default(),
    };
    if !nested { return binding; }
    let place = || ReferencePlaceV1 {
        local: 1, projection: vec![ReferencePlaceProjectionV1::Field(0)].into_boxed_slice(),
    };
    let binary = || ReferenceEffectExpressionV1::Binary {
        operation: ReferenceBinaryOpV1::Add,
        lhs: Box::new(binding_expression(11)), rhs: Box::new(binding_expression(13)), checked: false,
    };
    let write = ReferenceOutputWriteV1 {
        argument: 0, block: 0, statement: 0,
        coordinate: ReferenceOutputCoordinateV1::LogicalPoint(
            vec![binding_expression(3), binary()].into_boxed_slice()),
        guard: ReferencePathPredicateV1 {
            clauses: vec![ReferenceGuardClauseV1 {
                atoms: vec![
                    ReferenceGuardAtomV1::SwitchValueSet {
                        discriminant: binding_expression(1), values: vec![1, 2].into_boxed_slice(), inside_set: true,
                    },
                    ReferenceGuardAtomV1::Assert { condition: binding_expression(1), expected: true },
                ].into_boxed_slice(),
            }].into_boxed_slice(),
        },
        rhs: ReferenceEffectExpressionV1::InputLoad { reference_argument: 0, index: Box::new(binary()) },
        value: ReferenceValueV1::Use(ReferenceOperandV1::Copy(place())),
    };
    binding.effect_ir.blocks = vec![ReferenceBlockV1 {
        block: 0,
        assignments: vec![ReferenceAssignmentV1 {
            statement: 0,
            destination: ReferencePlaceV1 { local: 1,
                projection: vec![ReferencePlaceProjectionV1::Field(0), ReferencePlaceProjectionV1::Field(1)].into_boxed_slice() },
            value: ReferenceValueV1::SafeHelperCall {
                helper: binding_identity(),
                parameters: vec![ReferenceScalarTypeV1::U32; 2].into_boxed_slice(),
                result: ReferenceScalarTypeV1::U32,
                arguments: vec![ReferenceOperandV1::Copy(place()), binding_operand(9)].into_boxed_slice(),
                summary: Box::new(ReferenceEffectExpressionV1::Binary {
                    operation: ReferenceBinaryOpV1::Add,
                    lhs: Box::new(ReferenceEffectExpressionV1::InputLoad { reference_argument: 0, index: Box::new(binding_expression(1)) }),
                    rhs: Box::new(ReferenceEffectExpressionV1::Unary { operation: ReferenceUnaryOpV1::Not, operand: Box::new(binding_expression(2)) }),
                    checked: false,
                }),
            },
        }].into_boxed_slice(),
        terminator: ReferenceTerminatorV1::Assert {
            condition: binding_operand(1), expected: true, success: 0,
            bounds_check: Some(ReferenceBoundsCheckV1 {
                index: ReferenceOperandV1::Copy(ReferencePlaceV1 { local: 1, projection: Box::default() }),
                length: binding_operand(8),
            }),
        },
    }].into_boxed_slice();
    binding.effect_ir.loop_summaries = vec![ReferenceLoopSummaryV2 {
        header: 0, latch: 0, exit: 1, exact_iterations: Some(1), maximum_iterations: 1,
        carried_locals: vec![0, 1].into_boxed_slice(),
        initial_state_sha256: [9; 32], transition_sha256: [10; 32], variant_sha256: [11; 32],
    }].into_boxed_slice();
    binding.effect_ir.observable_output_effects = vec![write.clone()].into_boxed_slice();
    binding.observable_output_writes = vec![write].into_boxed_slice();
    binding
}

fn independent_binding_payload(nested: bool) -> usize {
    let base = size_of::<AuthenticatedReferenceEffectBindingV1>() + 2
        + size_of::<ReferenceLogicalSignaturePreimageV1>()
        + 2 * size_of::<ReferenceSignatureInputV1>()
        + size_of::<ReferenceEffectIrV1>() + size_of::<ReferenceArgumentRelationV1>();
    if !nested { return base; }
    let source_block = size_of::<ReferenceBlockV1>() + size_of::<ReferenceAssignmentV1>()
        + 2 * size_of::<ReferencePlaceV1>() + 3 * size_of::<ReferencePlaceProjectionV1>()
        + size_of::<ReferenceValueV1>() + 2 * size_of::<ReferenceScalarTypeV1>()
        + 2 * size_of::<ReferenceOperandV1>() + 5 * size_of::<ReferenceEffectExpressionV1>()
        + size_of::<ReferenceTerminatorV1>() + 3 * size_of::<ReferenceOperandV1>()
        + size_of::<ReferenceBoundsCheckV1>() + size_of::<ReferencePlaceV1>() + 1;
    let loops = size_of::<ReferenceLoopSummaryV2>() + 2 * size_of::<u32>();
    let write = size_of::<ReferenceOutputWriteV1>() + size_of::<ReferenceOutputCoordinateV1>()
        + size_of::<ReferencePathPredicateV1>() + size_of::<ReferenceGuardClauseV1>()
        + 2 * size_of::<ReferenceGuardAtomV1>() + 2 * size_of::<u128>()
        + 10 * size_of::<ReferenceEffectExpressionV1>()
        + size_of::<ReferenceValueV1>() + size_of::<ReferenceOperandV1>()
        + size_of::<ReferencePlaceV1>() + size_of::<ReferencePlaceProjectionV1>();
    base + source_block + loops + 2 * write
}

// Independent structural header premise, including the canonical refusal latch.
#[allow(dead_code)]
enum BindingCensusHeaderWork<'scope, 'ledger> {
    Legacy(&'scope mut SourceClosureWorkV1),
    Canonical { budget: &'scope mut Budget<'ledger>, first: Option<Resource> },
}
#[allow(dead_code)]
struct BindingCensusHeader<'binding, 'scope, 'ledger> {
    work: BindingCensusHeaderWork<'scope, 'ledger>,
    payload: usize,
    expressions: [Option<(&'binding ReferenceEffectExpressionV1, usize)>;
        fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
}

#[test]
fn original_binding_census_has_independent_nested_payload_and_exact_work_storage() {
    let header = size_of::<BindingCensusHeader<'_, '_, '_>>();
    for nested in [false, true] {
        let original = binding_payload(nested);
        let payload = independent_binding_payload(nested);
        let total_work = header + payload;
        for inherited in [0, 17] {
            for work_limit in [inherited + total_work - 1, inherited + total_work] {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, inherited + header);
                budget.charge_work(inherited).unwrap();
                budget.reserve_storage(inherited).unwrap();
                let result = binding_clone_envelope_v18(&original, &mut budget);
                if work_limit == inherited + total_work {
                    assert_eq!(result.unwrap(), payload);
                    assert_eq!(budget.work(), work_limit);
                } else {
                    assert!(matches!(result, Err(BindingCloneEnvelopeErrorV18::Resource(Resource::Work(_)))));
                    assert!(budget.work() <= work_limit);
                }
                assert_eq!(budget.storage(), inherited + header);
                budget.release_storage(header).unwrap();
                assert_eq!(budget.storage(), inherited);
                assert_eq!(work.failed_work().is_some(), work_limit < inherited + total_work);
            }
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, inherited + header - 1);
            budget.reserve_storage(inherited).unwrap();
            assert!(matches!(binding_clone_envelope_v18(&original, &mut budget),
                Err(BindingCloneEnvelopeErrorV18::Resource(Resource::Storage(_)))));
            assert_eq!(budget.work(), header);
            assert_eq!(budget.storage(), inherited);
            assert_eq!(budget.failed_storage(), Some(inherited + header));
        }
    }
}

#[test]
fn binding_clone_census_stops_at_first_original_work_denial_before_scratch() {
    let original = binding_payload(true);
    let header = size_of::<BindingCensusHeader<'_, '_, '_>>();
    let mut work = Work::new(header - 1);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let error = binding_clone_envelope_v18(&original, &mut budget).unwrap_err();
    let BindingCloneEnvelopeErrorV18::Resource(Resource::Work(error)) = error else {
        panic!("a real canonical refusal must remain a typed work failure");
    };
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
    assert_eq!(work.failed_work(), Some(error.actual()));
}

fn source_resource(error: ProductionRankedProjectionErrorV1) -> Resource {
    match error {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::SourceOwned(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(error))) => error,
        other => panic!("expected original canonical ledger refusal, got {other:?}"),
    }
}

#[test]
fn source_root_partition_retains_original_order_with_independent_exact_limits() {
    use source_ranked_consumer_resources_v18::SourceBindingAllocationV18 as Allocation;
    let header = size_of::<Allocation<'_, '_>>();
    // Three prepaid root rows, three pushes, six sort charges (34), two
    // adjacent comparisons (20), three output rows, three empty pushes, and
    // two lookups (1 + 21 + 1 each). This does not call the implementation to
    // derive its work premise.
    let required = header + 3 + 3 + 34 + 20 + 3 + 3 + 2 * (1 + 21 + 1);
    let logical = header + size_of::<Vec<(&str, usize)>>() + 3 * size_of::<(&str, usize)>()
        + size_of::<Vec<Vec<usize>>>() + 3 * size_of::<Vec<usize>>()
        + 2 * (size_of::<Vec<usize>>() + size_of::<usize>());
    let mut observed_storage = None;
    for work_limit in [required, required - 1] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut allocation = Allocation::new(Some(&mut budget)).unwrap();
        let result = partition_reference_effect_binding_indices_v18(
            &["beta", "alpha", "gamma"], &["gamma", "alpha"], &mut allocation);
        drop(allocation);
        if work_limit == required {
            let rows = result.unwrap();
            assert_eq!(rows, vec![vec![], vec![1], vec![0]]);
            let retained_rows = size_of::<Vec<Vec<usize>>>() + rows.capacity() * size_of::<Vec<usize>>()
                + 2 * size_of::<Vec<usize>>() + rows.iter().map(|row| row.capacity() * size_of::<usize>()).sum::<usize>();
            // The sorted borrowed-root scratch uses try_reserve_exact(3). A
            // returned allocator capacity above three is still charged by the
            // production helper; the live retained vector capacity is read here.
            assert!(budget.storage() >= logical);
            assert!(budget.storage() >= header + retained_rows);
            observed_storage = Some(budget.storage());
            drop(rows);
        } else {
            assert!(matches!(source_resource(result.unwrap_err()), Resource::Work(_)));
        }
        assert_eq!(budget.work(), work_limit);
        let retained = budget.storage();
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!(work.failed_work().is_some(), work_limit < required);
    }
    let exact_storage = observed_storage.unwrap();
    for storage_limit in [exact_storage, exact_storage - 1] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, storage_limit);
        let mut allocation = Allocation::new(Some(&mut budget)).unwrap();
        let result = partition_reference_effect_binding_indices_v18(
            &["beta", "alpha", "gamma"], &["gamma", "alpha"], &mut allocation);
        drop(allocation);
        if storage_limit == exact_storage {
            assert_eq!(result.as_ref().unwrap(), &vec![vec![], vec![1], vec![0]]);
            assert_eq!(budget.storage(), exact_storage);
            drop(result);
        } else {
            assert!(matches!(source_resource(result.unwrap_err()), Resource::Storage(_)));
            assert_eq!(budget.failed_storage(), Some(exact_storage));
        }
        let retained = budget.storage();
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn source_root_partition_preserves_duplicate_and_out_of_roster_refusals() {
    use source_ranked_consumer_resources_v18::SourceBindingAllocationV18 as Allocation;
    for (roots, bindings) in [
        (vec!["b", "a", "b"], vec!["a"]),
        (vec!["b", "a"], vec!["a", "a"]),
        (vec!["b", "a"], vec!["c"]),
    ] {
        let legacy = partition_reference_effect_binding_indices_v1(&roots, &bindings).unwrap_err();
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let mut allocation = Allocation::new(Some(&mut budget)).unwrap();
        let actual = partition_reference_effect_binding_indices_v18(&roots, &bindings, &mut allocation).unwrap_err();
        drop(allocation);
        assert_eq!(actual.to_string(), legacy.to_string());
        assert!(matches!(actual, ProductionRankedProjectionErrorV1::Unsupported(_)));
        let storage = budget.storage();
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn complete_binding_clone_and_box_conversion_keep_all_coexisting_payload_paid() {
    use source_ranked_consumer_resources_v18::{SourceBindingAllocationV18 as Allocation, push};
    let original = binding_payload(true);
    let payload = independent_binding_payload(true);
    let census_header = size_of::<BindingCensusHeader<'_, '_, '_>>();
    let header = size_of::<Allocation<'_, '_>>();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(37).unwrap();
    let mut allocation = Allocation::new(Some(&mut budget)).unwrap();
    let mut rows = allocation.rows(1).unwrap();
    let clone = allocation.binding(&original).unwrap();
    assert_eq!(clone, original);
    assert_ne!(clone.registration_path.as_ptr(), original.registration_path.as_ptr());
    assert_ne!(clone.effect_ir.blocks.as_ptr(), original.effect_ir.blocks.as_ptr());
    let extra = clone.registration_path.capacity() - clone.registration_path.len()
        + clone.logical_kernel_name.capacity() - clone.logical_kernel_name.len();
    push(&mut rows, clone).unwrap();
    let capacity = rows.capacity();
    allocation.box_storage::<AuthenticatedReferenceEffectBindingV1>(rows.len()).unwrap();
    let boxed = AuthenticatedReferenceEffectBindingsV1::new(rows);
    drop(allocation);
    let expected = 37 + header + size_of::<Vec<AuthenticatedReferenceEffectBindingV1>>()
        + capacity * size_of::<AuthenticatedReferenceEffectBindingV1>()
        + census_header + payload + extra
        + size_of::<Box<[AuthenticatedReferenceEffectBindingV1]>>() + size_of::<AuthenticatedReferenceEffectBindingV1>();
    assert_eq!(budget.storage(), expected);
    assert_eq!(budget.work(), header + 1 + census_header + 2 * payload + 1);
    assert_eq!(boxed.as_slice(), std::slice::from_ref(&original));
    drop(boxed);
    budget.release_storage(expected - 37).unwrap();
    assert_eq!(budget.storage(), 37);
}
