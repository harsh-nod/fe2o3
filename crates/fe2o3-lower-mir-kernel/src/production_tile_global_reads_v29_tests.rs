use super::*;
use crate::{
    ProductionCheckedTileGlobalReadsV29, ProductionTileGlobalReadErrorV29 as ReadError,
    ProductionTilePendingKindV29 as PendingKind, ProductionTileScalarOrderV29 as Order,
    ProductionTileScalarTransportOwnerV29 as Owner, with_checked_tile_global_reads_v29,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, ExecutionOperationV15,
};

#[path = "production_tile_global_reads_hostile_v29_tests.rs"]
mod hostile;
#[path = "production_tile_global_reads_resources_v29_tests.rs"]
mod resources;

const LIMIT: usize = 1_000_000_000;
const FLOOR: usize = 37;

#[derive(Clone, Copy)]
enum SourceCase {
    Repeated,
    Slots,
    Shifted,
}
fn source(case: SourceCase) -> ProductionSemanticSsaOwnerV1 {
    match case {
        SourceCase::Repeated => tile_parts_repeated_owner(),
        SourceCase::Slots => tile_parts_repeated_slot_owner(),
        SourceCase::Shifted => tile_parts_entry_slot_owner(),
    }
}
fn pending_source(case: SourceCase, profiled: bool, budget: &mut Budget<'_>)
    -> Result<ProductionPendingScopedSourceOwnerV29, ProductionPendingScopedSourceErrorV29>
{
    use kernel_argument_abi_v18::tests::{fixture_descriptor_ownership_v18, FixtureKernelAbiV18};
    let owner = fixture_descriptor_ownership_v18(source(case));
    let projected = fixture_descriptor_ownership_v18(source(case));
    let semantic = projected.source_semantic();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let roots = [root_input(&projected)];
    let workgroup = semantic.functions()[2].abi().source_input_types()[0];
    let classes = [
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Provider {
            function: HELPER,
            identity: semantic.functions()[1].identity(),
        },
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Derive {
            binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
            operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
            context: CONTEXT,
            workgroup,
        },
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
        ProductionScopeCallableCandidateV29::Ordinary,
    ];
    let SemanticTerminatorKindV1::Call(derive) =
        semantic.functions()[1].blocks()[0].terminator().kind()
    else {
        panic!("genuine source derive call");
    };
    let events = [
        (
            ROOT,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        ),
        (
            HELPER,
            0,
            semantic.functions()[1].blocks()[0].statements().len(),
            ProductionScopeEventKindV29::Call {
                callee: derive.callee(),
                kind: ProductionScopeCallKindV29::Derive,
            },
        ),
        (
            HELPER,
            1,
            0,
            ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(2),
                kind: ProductionScopeCallKindV29::Ordinary,
            },
        ),
        (HELPER, 2, 0, ProductionScopeEventKindV29::Return),
    ]
    .map(
        |(function, block, statement_count, kind)| crate::ProductionScopeEventCandidateV29 {
            function,
            block: SemanticBlockIdV1::from_index(block),
            statement_count,
            kind,
        },
    );
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: projected.source_semantic_sha256(), roots: &roots,
        classes: &classes, events: &events,
    };
    if profiled {
        let profile = FixtureKernelAbiV18::new(&projected);
        let profile_roots = profile.roots();
        ProductionPendingScopedSourceOwnerV29::try_materialize_with_kernel_abi_budget_v18(
            owner, launch, input, ProductionKernelArgumentAbiInputV18 { roots: &profile_roots },
            ProductionSemanticKirLimitsV1::default(), budget,
        )
    } else {
        ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
            owner, launch, input, ProductionSemanticKirLimitsV1::default(), budget,
        )
    }
}
fn owner(case: SourceCase, order: Order, budget: &mut Budget<'_>) -> Owner {
    let pending = pending_source(case, true, budget).unwrap();
    let mut donor = Some(pending);
    let owner = Owner::try_from_pending_with_budget_v29(&mut donor, order, budget).unwrap();
    assert!(donor.is_none());
    owner
}

#[test]
fn real_tile_sources_require_the_original_kernel_descriptor_profile() {
    for case in [SourceCase::Repeated, SourceCase::Slots, SourceCase::Shifted] {
        for profiled in [false, true] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let result = pending_source(case, profiled, &mut budget);
            if profiled {
                let pending = result.unwrap();
                assert!(pending.pending_module().functions.iter().filter_map(|f| f.body.as_ref())
                    .flat_map(|body| &body.blocks).flat_map(|block| &block.operations)
                    .any(|operation| matches!(operation.kind, OperationKind::Execution(
                        ExecutionOperationV15::MaskedTileLoadU32 { .. }))));
                let retained = pending.adopted_storage();
                assert_eq!(budget.storage(), FLOOR + retained);
                drop(pending);
                budget.release_storage(retained).unwrap();
            } else {
                assert!(matches!(&result, Err(ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported { detail, .. }))
                    if *detail == "execution lifecycle differs from its retained source instance"),
                    "an absent descriptor must not classify Generic memory as Global");
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
fn release(owner: Owner, budget: &mut Budget<'_>) {
    let storage = owner.adopted_storage();
    drop(owner);
    budget.release_storage(storage).unwrap();
}
fn consume<T>(
    owner: &Owner,
    budget: &mut Budget<'_>,
    callback: impl for<'s, 'w> FnOnce(
        &ProductionCheckedTileGlobalReadsV29<'s>,
        &mut Budget<'w>,
    ) -> Result<T, ReadError>,
) -> Result<T, ReadError> {
    owner
        .with_checked_transport_v29(budget, |transport, budget| {
            Ok(with_checked_tile_global_reads_v29(
                transport, budget, callback,
            ))
        })
        .map_err(ReadError::Transport)?
}

#[test]
fn both_layouts_check_every_generated_load_with_real_helper_and_repeated_aliases() {
    for order in [Order::Blocked, Order::Striped] {
        for case in [SourceCase::Repeated, SourceCase::Slots, SourceCase::Shifted] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = owner(case, order, &mut budget);
            let before = budget.storage();
            consume(&owner, &mut budget, |checked, budget| {
                let transport = checked.transport(budget)?;
                let inventory = transport.current_inventory(budget)?;
                let original = transport.pending_ancestor(budget)?.pending_module();
                let actual: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18 =
                    checked.graph(budget)?.owner(budget)?;
                assert_eq!(actual.module().storage_layouts, original.storage_layouts);
                assert_eq!(*actual.identity(), transport.current_identity(budget)?);
                let mut expected = 0_usize;
                for function in &original.functions {
                    if let Some(body) = &function.body {
                        for block in &body.blocks {
                            for operation in &block.operations {
                                if let OperationKind::Execution(
                                    ExecutionOperationV15::MaskedTileLoadU32 { elements, .. },
                                ) = operation.kind
                                {
                                    expected += usize::from(elements);
                                }
                            }
                        }
                    }
                }
                assert!(
                    expected >= 4,
                    "genuine repeated source Loads, not an empty graph"
                );
                assert_eq!(checked.read_count(budget)?, expected);
                assert!(std::ptr::eq(
                    checked.graph(budget)?.owner(budget)?,
                    inventory.owner()
                ));
                let mut physical = std::collections::BTreeSet::new();
                let mut aliases = 0;
                for ordinal in 0..expected {
                    let read = checked.read(ordinal, budget)?;
                    assert!(physical.insert(read.operation()));
                    assert_eq!(read.checked_operations(), 3);
                    assert_eq!(read.domain().element_bytes(), 4);
                    assert!(!read.aliases().is_empty());
                    for index in read.aliases() {
                        let alias = checked.alias(index, budget)?;
                        assert_eq!(alias.read(), ordinal);
                        assert_eq!(
                            transport.source_alias(alias.source_alias(), budget)?.root(),
                            read.root()
                        );
                        aliases += 1;
                    }
                    let subject = checked
                        .pending_obligation(read.obligation(), budget)?
                        .global_read()
                        .unwrap();
                    assert_eq!(subject.output(), read.operation());
                    let crate::ProductionTileSourceSpanV29::Terminator(span) = transport
                        .source_alias(subject.source_alias(), budget)?
                        .source()
                    else {
                        panic!("actual helper Load terminator");
                    };
                    assert_ne!(
                        span.semantic_function(),
                        transport
                            .root(read.root(), budget)?
                            .launch()
                            .selected_root()
                    );
                }
                assert_eq!(aliases, checked.alias_count(budget)?);
                let mut pending = [0_usize; 5];
                for ordinal in 0..checked.pending_obligation_count(budget)? {
                    let row = checked.pending_obligation(ordinal, budget)?;
                    pending[match row.kind() {
                        PendingKind::Source { .. } => 0,
                        PendingKind::CollectiveLifecycle { .. } => 1,
                        PendingKind::LaunchGeometry { .. } => 2,
                        PendingKind::GlobalRead => 3,
                        PendingKind::RetainedAttachment { .. } => 4,
                    }] += 1;
                }
                assert!(pending.iter().all(|count| *count > 0));
                assert_eq!(pending[3], expected);
                assert_eq!(pending[0], transport.source_alias_count(budget)?);
                assert_eq!(pending[4], transport.attachment_count(budget)?);
                assert!(!checked.grants_artifact_or_launch_authority());
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), before);
            release(owner, &mut budget);
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn same_byte_source_owners_keep_distinct_actual_graph_custody() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let first = owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let second = owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let first_address = consume(&first, &mut budget, |view, budget| {
        Ok(std::ptr::from_ref(view.graph(budget)?.owner(budget)?) as usize)
    })
    .unwrap();
    let second_address = consume(&second, &mut budget, |view, budget| {
        Ok(std::ptr::from_ref(view.graph(budget)?.owner(budget)?) as usize)
    })
    .unwrap();
    assert_ne!(first_address, second_address);
    release(second, &mut budget);
    release(first, &mut budget);
    assert_eq!(budget.storage(), 0);
}
