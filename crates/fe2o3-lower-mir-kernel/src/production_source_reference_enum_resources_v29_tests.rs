use super::*;
use production_call_instances_v1::with_production_call_instances_v1;

#[test]
fn memo_credit_real_enum_and_downcast_view_keep_rebuilt_payload_rows_paid() {
    for expire in [false, true] {
        with_enum_builder(|builder, budget| {
            let instance = builder.plan.root;
            let local = SemanticLocalIdV1::from_index(1);
            let site = SourceReferenceSiteV29 {
                instance, block: SemanticBlockIdV1::from_index(0), statement: Some(0),
            };
            let (reference, word) = if expire {
                let origin = builder.plan.raw_origins.len();
                emission_push_v1(&mut builder.plan.raw_origins, SourceReferenceRawOriginV29 {
                    site, source: 0, formation: SourceReferenceRawFormationV29::ReferenceCast,
                    instance, local, generation: 0, ty: WORD, pointer_type: REFERENCE,
                    first: 0, count: 0, parent: None, mutable: false,
                }, budget)?;
                let source = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
                    origin, expired: false,
                });
                let set = builder.raw_set(source, source, false, budget)?;
                (builder.node(REFERENCE, SourceReferenceNodeKindV29::Address(set), budget)?, builder.plain(WORD, budget)?)
            } else {
                let observation = builder.plan.enum_observations.len();
                emission_push_v1(&mut builder.plan.enum_observations, SourceReferenceEnumObservationV29 {
                    site, source: 0, instance, local, generation: 0, ty: WORD, first: 0, count: 0,
                }, budget)?;
                (builder.plain(REFERENCE, budget)?, builder.node(WORD, SourceReferenceNodeKindV29::Discriminant(observation), budget)?)
            };
            let alternative = builder.intern_enum_alternative(ENUM, 2, &[reference, word], budget)?;
            let source = builder.intern_enum_value(ENUM, &[alternative], budget)?;
            let path = enum_field(5, 2, 1);
            let view = builder.enum_path_node(source, &path.projections()[..1], budget)?;
            assert!(matches!(builder.plan.nodes[view].kind, SourceReferenceNodeKindV29::EnumView(_)));
            let retained = budget.storage();
            let alternatives = builder.plan.enum_alternatives.len();
            let next = if expire {
                builder.expire_addresses(instance, Some(local), Some(view), budget)?
            } else {
                builder.invalidate_discriminant_values(Some(instance), Some(local), Some(view), budget)?
            }.unwrap();
            let SourceReferenceNodeKindV29::EnumView(view) = builder.plan.nodes[next].kind else { panic!("downcast view lost") };
            let source = builder.plan.enum_views[view].source;
            let SourceReferenceNodeKindV29::Enum { first, count } = builder.plan.nodes[source].kind else { panic!("enum source lost") };
            let member = builder.plan.enum_member(first, count, 0, budget)?;
            let alternative = builder.plan.enum_alternative(member, budget)?;
            let child = builder.plan.children[alternative.first + usize::from(!expire)];
            if expire {
                let SourceReferenceNodeKindV29::Address(set) = builder.plan.nodes[child].kind else { panic!("address payload lost") };
                let row = builder.plan.raw_sets[set];
                assert!(builder.plan.raw_choice(SourceReferenceRawChoicesV29::Retained(row), 0, budget)?.unwrap().expired);
            } else { assert_eq!(builder.plan.nodes[child].kind, SourceReferenceNodeKindV29::Plain(None)); }
            assert!(builder.plan.enum_alternatives.len() > alternatives);
            assert!(budget.storage() > retained);
            Ok(())
        }).unwrap();
    }
}

fn with_enum_builder(
    consume: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let owner = enum_owner(EnumCase::Construct(2));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let capture = owner.occurrence_storage().expect("fixture captures its original occurrences once");
    budget.reserve_storage(capture.retained_storage())?;
    let result = with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
        let result = consume(&mut builder, budget);
        drop(builder);
        budget.release_storage(budget.storage() - floor).unwrap();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
    }).unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}

#[test]
fn opaque_enum_expansion_retains_original_values_without_eager_payload_fields() {
    with_enum_builder(|builder, budget| {
        let original = builder.plain(ENUM, budget)?;
        let children = builder.plan.children.len();
        let nodes = builder.plan.nodes.len();
        let expanded = builder.expand_plain_enum(original, budget)?;
        assert_eq!(builder.plan.children.len(), children);
        assert_eq!(builder.plan.nodes.len(), nodes + 1);
        let SourceReferenceNodeKindV29::Enum { first, count } = builder.plan.nodes[expanded].kind
            else { panic!("expected enum") };
        assert_eq!(count, 3);
        for offset in 0..count {
            let member = builder.plan.enum_member(first, count, offset, budget)?;
            let alternative = builder.plan.enum_alternative(member, budget)?;
            assert_eq!((alternative.opaque, alternative.first, alternative.count), (Some(original), 0, 0));
            assert!(original < expanded);
        }
        let retained = (builder.plan.nodes.len(), builder.plan.children.len(),
            builder.plan.enum_alternatives.len(), builder.plan.enum_members.len());
        assert_eq!(builder.expand_plain_enum(original, budget)?, expanded);
        assert_eq!(retained, (builder.plan.nodes.len(), builder.plan.children.len(),
            builder.plan.enum_alternatives.len(), builder.plan.enum_members.len()));
        let path = enum_field(5, 2, 1);
        let selected = builder.enum_path_node(expanded, path.projections(), budget)?;
        assert_eq!(builder.plan.nodes[selected].ty, WORD);
        assert_eq!(builder.plan.nodes[selected].kind, SourceReferenceNodeKindV29::Plain(None));
        assert!(builder.plan.nodes[selected].descriptor.is_none());
        assert!(builder.plan.loans.is_empty());
        assert_eq!(builder.plan.children.len(), children);
        Ok(())
    }).unwrap();
}

#[test]
fn opaque_enum_projection_checks_the_original_older_source_before_creating_fields() {
    with_enum_builder(|builder, budget| {
        let original = builder.plain(ENUM, budget)?;
        let expanded = builder.expand_plain_enum(original, budget)?;
        let member = builder.plan.enum_alternatives.iter().position(|row| row.variant == 1).unwrap();
        let future = builder.plain(ENUM, budget)?;
        builder.plan.enum_alternatives[member].opaque = Some(future);
        let before = builder.plan.nodes.len();
        let path = enum_field(5, 1, 1);
        assert!(builder.enum_projected_nodes(expanded, path.projections(), budget).is_err());
        assert_eq!(builder.plan.nodes.len(), before);
        Ok(())
    }).unwrap();
}

#[test]
fn rebuilt_alternative_canonicalization_has_a_literal_work_boundary_and_no_allocation() {
    for short in [false, true] {
        let completed = std::cell::Cell::new(false);
        let result = run_enum(EnumCase::Construct(2), |_, budget| {
            let mut members = source_reference_scratch_v29(3, budget)?;
            members.extend_from_slice(&[3, 1, 3]);
            // One heapify sift8, two extraction swap1+sift8, three dedup visits2.
            let exact = 8 + 2 * (1 + 8) + 3 * 2;
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before_work = budget.work();
            let before_storage = budget.storage();
            let result = SourceReferenceBuilderV29::canonicalize_enum_members(&mut members, budget);
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.work() - before_work, if short { exact - 2 } else { exact });
            assert_eq!(budget.storage(), before_storage);
            if short {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)))));
                let stopped = budget.work();
                assert!(SourceReferenceBuilderV29::canonicalize_enum_members(&mut members, budget).is_err());
                assert_eq!(budget.work(), stopped);
            } else {
                assert_eq!(members, [1, 3]);
            }
            completed.set(true);
            Err(source_reference_error_v29("completed enum sorting resource boundary"))
        });
        assert!(completed.get(), "sorting assertions must finish outside panic conversion");
        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "completed enum sorting resource boundary", .. })));
    }
}

#[test]
fn actual_enum_plan_queries_have_independent_exact_work_boundaries() {
    for short in [false, true] {
        let completed = std::cell::Cell::new(false);
        let result = run_enum(EnumCase::Construct(2), |plan, budget| {
            let (first, count) = plan.nodes.iter().find_map(|node| {
                if node.ty != ENUM { return None; }
                match node.kind {
                    SourceReferenceNodeKindV29::Enum { first, count } => Some((first, count)),
                    _ => None,
                }
            }).expect("actual constructor did not retain enum state");
            // Declaration/shape/variant3, member/bound2, alternative lookup1.
            let exact = 3 + 2 + 1;
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before_work = budget.work();
            let before_storage = budget.storage();
            let queried = (|| {
                assert_eq!(plan.enum_variant_fields(ENUM, 2, budget)?, [REFERENCE, WORD]);
                let member = plan.enum_member(first, count, 0, budget)?;
                let alternative = plan.enum_alternative(member, budget)?;
                assert_eq!((alternative.ty, alternative.variant, alternative.count), (ENUM, 2, 2));
                Ok::<(), ProductionSemanticKirErrorV1>(())
            })();
            assert_eq!(queried.is_err(), short);
            assert_eq!(budget.work() - before_work, exact - usize::from(short));
            assert_eq!(budget.storage(), before_storage);
            if short {
                assert!(matches!(queried, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)))));
                let stopped = budget.work();
                assert!(plan.enum_alternative(0, budget).is_err());
                assert_eq!(budget.work(), stopped);
            }
            completed.set(true);
            Err(source_reference_error_v29("completed enum query resource boundary"))
        });
        assert!(completed.get(), "resource assertions must finish outside panic conversion");
        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "completed enum query resource boundary", .. })));
    }
}

#[test]
fn actual_enum_alternatives_reuse_whole_states_without_field_cross_products() {
    run_enum(EnumCase::CorrelatedLoans, |plan, _| {
        let alternatives = plan.enum_alternatives.iter().filter(|row| row.ty == ENUM && row.variant == 0)
            .collect::<Vec<_>>();
        assert!(alternatives.len() >= 2);
        for (index, left) in alternatives.iter().enumerate() {
            for right in &alternatives[index + 1..] {
                let a = &plan.children[left.first..left.first + left.count];
                let b = &plan.children[right.first..right.first + right.count];
                assert!(a.iter().zip(b).any(|(&a, &b)| plan.nodes[a].kind != plan.nodes[b].kind
                    || plan.nodes[a].storage != plan.nodes[b].storage),
                    "equal complete alternatives must be interned before copying children");
            }
        }
        Ok(())
    }).unwrap();
}

fn inert_activation(builder: &SourceReferenceBuilderV29<'_, '_, '_>) -> SourceReferenceStorageActivationV29 {
    SourceReferenceStorageActivationV29 {
        instance: builder.plan.root,
        local: SemanticLocalIdV1::from_index(0),
        generation: 0,
        origin: SourceReferenceActivationOriginV29::Entry,
    }
}

#[test]
fn activation_interning_has_independent_exact_and_short_resource_boundaries() {
    // This tests only the private interner, not original-site admission. Source
    // and custody are tested separately through the authenticated producer.
    for mode in 0..3 {
        let reached = std::cell::Cell::new(false);
        with_enum_builder(|builder, _| {
            let row = inert_activation(builder);
            let map = 2 * 32 * std::mem::size_of::<((usize, u32, u32), usize, usize)>();
            let vector = 4 * std::mem::size_of::<SourceReferenceStorageActivationV29>();
            let exact_work = 32 + 32 + 2 + 3;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - usize::from(mode == 1));
            let mut budget = ArgumentBudgetV1::new(&mut work, 79 + map + vector - usize::from(mode == 2));
            budget.reserve_storage(79)?;
            let nodes = builder.plan.nodes.len();
            let result = builder.intern_storage_activation(row, &mut budget);
            assert_eq!(result.is_err(), mode != 0);
            assert_eq!(builder.plan.nodes.len(), nodes, "allocation rows must never fabricate values");
            assert_eq!(builder.plan.storage_activations.len(), usize::from(mode == 0));
            assert_eq!(builder.plan.storage_activation_sites.len(), usize::from(mode == 0));
            if mode == 0 {
                assert_eq!(budget.work(), exact_work);
                assert_eq!(budget.storage(), 79 + map + vector);
                assert_eq!(builder.plan.storage_activations, [row]);
            } else {
                assert_eq!(budget.work(), if mode == 1 { 66 } else { exact_work });
                assert_eq!(budget.storage(), 79 + map);
                let before = (budget.work(), budget.storage());
                assert!(builder.intern_storage_activation(row, &mut budget).is_err());
                assert_eq!((budget.work(), budget.storage()), before, "resource failure must stay sticky");
                assert!(builder.plan.storage_activations.is_empty());
                assert!(builder.plan.storage_activation_sites.is_empty());
            }
            reached.set(true);
            Ok(())
        }).unwrap();
        assert!(reached.get());
    }
}

#[test]
fn activation_fixed_site_revisits_are_allocation_free_and_reject_conflicting_origins() {
    with_enum_builder(|builder, outer| {
        let row = inert_activation(builder);
        builder.intern_storage_activation(row, outer)?;
        for changed in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(32 + 2);
            let mut budget = ArgumentBudgetV1::new(&mut work, 83);
            budget.reserve_storage(83)?;
            let mut supplied = row;
            if changed {
                supplied.origin = SourceReferenceActivationOriginV29::StorageLive(SourceReferenceSiteV29 {
                    instance: row.instance, block: SemanticBlockIdV1::from_index(0), statement: Some(0),
                });
            }
            assert_eq!(builder.intern_storage_activation(supplied, &mut budget).is_err(), changed);
            assert_eq!(budget.work(), 34);
            assert_eq!(budget.storage(), 83);
            assert_eq!(builder.plan.storage_activations, [row]);
            assert_eq!(builder.plan.storage_activation_sites.len(), 1);
        }
        Ok(())
    }).unwrap();
}

fn with_original_activation_builder(
    consume: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_original_source_builder(tag_only_activation_owner_v29(1), consume)
}

fn with_original_source_builder(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(97)?;
    let capture = owner.occurrence_storage().expect("fixture captures its original occurrences once");
    budget.reserve_storage(capture.retained_storage())?;
    let demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner, demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(), &mut budget,
    )?;
    let floor = budget.storage();
    let persistent = layouts.persistent_storage_for_test();
    let result = with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let lens = demands.root_lens(&owner, 0, budget).unwrap();
        let result = with_source_reference_descriptor_demands_scope_v29(
            instances, SourceReferenceStorageV29::ScalarCells, Some(&mut layouts), None,
            Some(lens), budget, |plan, root, budget| {
                let mut builder = SourceReferenceBuilderV29::new_with_descriptor_demands(
                    plan.instances, SourceReferenceStorageV29::ScalarCells, root, None,
                    plan.storage_demands, budget,
                )?;
                builder.collect_storage_selectors(budget)?;
                builder.function(builder.plan.root, None, budget)?;
                consume(&mut builder, budget).map_err(Into::into)
            },
        );
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
    }).unwrap();
    // The second builder is test-owned; the production scope retains callback
    // reservations. Keep any shared schema growth paid until layouts is dropped.
    let growth = layouts.persistent_storage_for_test() - persistent;
    let scratch = budget.storage() - floor - growth;
    assert!(layouts.permits_root_emission_refund(&owner, scratch, &budget));
    budget.release_storage(scratch)?;
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 97);
    result.and(cleanup).and(demand_cleanup)
}

#[test]
fn original_activation_producer_rejects_changed_sites_generations_and_custody() {
    for mode in 0..11 {
        let reached = std::cell::Cell::new(false);
        let result = with_original_activation_builder(|builder, budget| {
            let mut row = *builder.plan.storage_activations.iter().find(|row|
                row.local.index() == 2 && matches!(row.origin, SourceReferenceActivationOriginV29::StorageLive(_)))
                .expect("actual StorageLive activation");
            let rows = builder.plan.storage_activations.clone();
            let indices = builder.plan.storage_activation_sites.clone();
            let nodes = builder.plan.nodes.len();
            match mode {
                0 => row.generation += 1,
                1 => if let SourceReferenceActivationOriginV29::StorageLive(ref mut site) = row.origin {
                    site.statement = Some(1); // SetDiscriminant is not StorageLive.
                },
                2 => if let SourceReferenceActivationOriginV29::StorageLive(ref mut site) = row.origin {
                    site.statement = None;
                },
                3 => row.origin = SourceReferenceActivationOriginV29::Entry,
                4 => builder.plan.root = capture_instance(&builder.plan, 0),
                5 => builder.plan.slot ^= 1,
                6 => builder.plan.source[0] ^= 1,
                7 => builder.storage_requests = None,
                8 => if let SourceReferenceActivationOriginV29::StorageLive(ref mut site) = row.origin {
                    site.instance = builder.plan.root;
                },
                10 => builder.epochs[row.instance.index()][0] = usize::MAX,
                _ => {},
            }
            if mode == 9 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                foreign.reserve_storage(budget.storage())?;
                assert!(builder.retain_storage_activation(row, &mut foreign).is_err());
            } else {
                assert!(builder.retain_storage_activation(row, budget).is_err());
            }
            assert_eq!(builder.plan.storage_activations, rows);
            assert_eq!(builder.plan.storage_activation_sites, indices);
            assert_eq!(builder.plan.nodes.len(), nodes);
            reached.set(true);
            Ok(())
        });
        assert!(reached.get(), "hostile assertions must run beyond source-plan construction");
        assert_eq!(result.is_err(), matches!(mode, 4 | 5 | 6 | 9),
            "a failed owner query must remain sticky at the original scope exit");
    }
}

#[test]
fn authenticated_activation_revisit_has_an_independent_literal_work_and_storage_oracle() {
    for short in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = with_original_activation_builder(|builder, budget| {
            let requests = builder.storage_requests.unwrap();
            let row = *builder.plan.storage_activations.iter().find(|row|
                row.local.index() == 2 && matches!(row.origin, SourceReferenceActivationOriginV29::StorageLive(_)))
                .expect("original tag-only live activation");
            let ordinal = requests.iter().position(|request|
                (request.instance, request.local) == (row.instance, row.local)).unwrap();
            // Independently enumerate the balanced tree's insertion-point leaf
            // depths from numeric intervals, without invoking the source search
            // or measuring a successful producer's work counter.
            let mut levels = std::collections::VecDeque::from([(0, requests.len(), 0)]);
            let comparisons = loop {
                let (first, end, depth) = levels.pop_front().unwrap();
                if first == end {
                    if first == ordinal { break depth; }
                } else {
                    let middle = first + (end - first) / 2;
                    levels.push_back((first, middle, depth + 1));
                    levels.push_back((middle + 1, end, depth + 1));
                }
            };
            // Owner5, borrowed binary lookup2 per comparison + final1, original
            // activation validation12, paid map lookup, duplicate row comparison2.
            let map_work = (builder.plan.storage_activation_sites.len().ilog2() as usize + 2) * 16;
            let exact = 5 + comparisons * 2 + 1 + 12 + map_work + 2;
            let headers = 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>()
                + std::mem::size_of::<SourceReferenceStorageActivationV29>();
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before = (budget.work(), budget.storage());
            let count = builder.plan.storage_activations.len();
            assert_eq!(builder.retain_storage_activation(row, budget).is_err(), short);
            assert_eq!(budget.work() - before.0, exact - if short { 2 } else { 0 });
            assert_eq!(budget.storage() - before.1, headers);
            assert_eq!(builder.plan.storage_activations.len(), count);
            assert_eq!(builder.plan.storage_activation_sites.len(), count);
            if short {
                let stopped = (budget.work(), budget.storage());
                assert!(builder.retain_storage_activation(row, budget).is_err());
                assert_eq!((budget.work(), budget.storage()), stopped);
            }
            reached.set(true);
            Err(source_reference_error_v29("completed activation producer resource boundary"))
        });
        assert!(reached.get(), "producer resource assertions must complete outside scope panic handling");
        if short {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(_)))));
        } else {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "completed activation producer resource boundary", .. })));
        }
    }
}

#[test]
fn genuine_empty_original_demand_lens_does_not_allocate_an_activation_census() {
    let owner = empty_demand_boundary_owner_v29();
    let reached = std::cell::Cell::new(false);
    run_enum_with_original_demands(owner, |plan, budget| {
        let (requests, _) = plan.storage_demands.expect("genuine original empty lens")
            .requests(plan.instances, budget)?;
        assert!(requests.is_empty());
        assert!(!plan.has_storage_demands);
        assert!(plan.storage_activations.is_empty());
        assert_eq!(plan.storage_activations.capacity(), 0);
        assert!(plan.storage_activation_sites.is_empty());
        assert!(plan.representation_demands.is_empty());
        reached.set(true);
        Ok(())
    }).unwrap();
    assert!(reached.get());
}

fn with_original_boundary_builder(
    consume: impl FnOnce(&mut SourceReferenceBuilderV29<'_, '_, '_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_original_source_builder(enum_owner(EnumCase::Construct(2)), consume)
}

#[test]
fn original_boundary_queries_refuse_changed_source_role_type_index_and_owner() {
    for mode in 0..11 {
        let reached = std::cell::Cell::new(false);
        let result = with_original_boundary_builder(|builder, budget| {
            let ordinal = builder.plan.boundary_values.iter().position(|row|
                matches!(row.role, SourceReferenceBoundaryRoleV29::Argument(0))).unwrap();
            let original = builder.plan.boundary_values[ordinal];
            let row = &mut builder.plan.boundary_values[ordinal];
            match mode {
                0 => row.site.statement = Some(0),
                1 => row.site.block = SemanticBlockIdV1::from_index(u32::MAX),
                2 => row.role = SourceReferenceBoundaryRoleV29::Argument(u32::MAX),
                3 => row.role = SourceReferenceBoundaryRoleV29::Return,
                4 => row.node = usize::MAX,
                5 => row.node = builder.plan.nodes.iter().position(|node| node.ty == UNIT).unwrap(),
                6 => { builder.plan.boundary_sites.insert(
                    source_reference_boundary_key_v29(original.site, original.role), usize::MAX); },
                7 => { builder.plan.boundary_sites.remove(
                    &source_reference_boundary_key_v29(original.site, original.role)); },
                8 => builder.plan.source[0] ^= 1,
                9 => builder.plan.slot ^= 1,
                _ => {},
            }
            if mode == 10 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                foreign.reserve_storage(budget.storage())?;
                assert!(builder.plan.boundary_value(ordinal, &mut foreign).is_err());
            } else {
                assert!(builder.plan.boundary_value(ordinal, budget).is_err());
            }
            reached.set(true);
            Ok(())
        });
        assert!(reached.get(), "malformed boundary must be tested after authentic evaluation");
        if mode >= 8 {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting))), "mode {mode}: {result:?}");
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn original_boundary_producer_refuses_noneffect_sites_without_row_publication() {
    with_original_boundary_builder(|builder, budget| {
        let row = builder.plan.boundary_values[0];
        let rows = builder.plan.boundary_values.clone();
        let sites = builder.plan.boundary_sites.clone();
        for effect in [None, Some(SourceReferenceSiteV29 { statement: Some(0), ..row.site })] {
            builder.effect_site = effect;
            assert!(builder.retain_boundary_value(row.site, row.role, row.node, budget).is_err());
            assert_eq!(builder.plan.boundary_values, rows);
            assert_eq!(builder.plan.boundary_sites, sites);
        }
        builder.effect_site = Some(row.site);
        assert!(builder.retain_boundary_value(row.site, row.role, usize::MAX, budget).is_err());
        assert_eq!(builder.plan.boundary_values, rows);
        assert_eq!(builder.plan.boundary_sites, sites);
        Ok(())
    }).unwrap();
}

#[test]
fn boundary_fixed_site_revisits_preserve_one_row_and_original_correlated_nodes() {
    with_original_boundary_builder(|builder, budget| {
        let row = *builder.plan.boundary_values.iter().find(|row|
            matches!(row.role, SourceReferenceBoundaryRoleV29::Argument(_))
                && builder.plan.nodes[row.node].ty == CAPTURE).unwrap();
        let before = (builder.plan.boundary_values.clone(), builder.plan.boundary_sites.clone(),
            builder.plan.nodes.len(), builder.plan.children.len());
        builder.effect_site = Some(row.site);
        for _ in 0..64 {
            builder.retain_boundary_value(row.site, row.role, row.node, budget)?;
        }
        assert_eq!(builder.plan.boundary_values, before.0);
        assert_eq!(builder.plan.boundary_sites, before.1);
        assert_eq!((builder.plan.nodes.len(), builder.plan.children.len()), (before.2, before.3));
        assert_eq!(builder.plan.boundary_at(row.site, row.role, budget)?.unwrap().1, row);
        Ok(())
    }).unwrap();
}

#[test]
fn original_boundary_publication_has_independent_exact_and_short_work_storage_limits() {
    for mode in 0..3 {
        let reached = std::cell::Cell::new(false);
        let result = with_original_boundary_builder(|builder, budget| {
            let row = builder.plan.boundary_values[0];
            builder.effect_site = Some(row.site);
            // Discard test-only row/index replicas together. Nodes still come
            // from the actual original evaluation, and their owner is unchanged.
            builder.plan.boundary_values = Vec::new();
            builder.plan.boundary_sites = BTreeMap::new();
            let headers = std::mem::size_of::<SourceReferenceBoundaryValueV29>()
                + std::mem::size_of::<(usize, u32, Option<usize>, SourceReferenceBoundaryRoleV29)>()
                + 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
            let map = 2 * 32 * std::mem::size_of::<(
                (usize, u32, Option<usize>, SourceReferenceBoundaryRoleV29), usize, usize)>();
            let vector = 4 * std::mem::size_of::<SourceReferenceBoundaryValueV29>();
            // owner5 + site2 + (owner5 + source9 + type3), two tree32
            // reservations, push2, vector3, zero prior rows to copy.
            let exact = 5 + 2 + (5 + 9 + 3) + 32 + 32 + 2 + 3;
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(mode == 1))?;
            budget.reserve_storage(usize::MAX - budget.storage() - headers - map - vector
                + usize::from(mode == 2))?;
            let before = (budget.work(), budget.storage(), builder.plan.nodes.len());
            let published = builder.retain_boundary_value(row.site, row.role, row.node, budget);
            assert_eq!(published.is_err(), mode != 0);
            assert_eq!(budget.work() - before.0, exact - if mode == 1 { 3 } else { 0 });
            assert_eq!(budget.storage() - before.1, headers + map + if mode == 0 { vector } else { 0 });
            assert_eq!(builder.plan.nodes.len(), before.2);
            assert_eq!(builder.plan.boundary_values.len(), usize::from(mode == 0));
            assert_eq!(builder.plan.boundary_sites.len(), usize::from(mode == 0));
            if mode == 0 {
                assert_eq!(builder.plan.boundary_values, [row]);
                assert_eq!(builder.plan.boundary_sites.get(&source_reference_boundary_key_v29(row.site, row.role)), Some(&0));
            } else {
                let stopped = (budget.work(), budget.storage());
                assert!(builder.retain_boundary_value(row.site, row.role, row.node, budget).is_err());
                assert_eq!((budget.work(), budget.storage()), stopped);
                assert!(builder.plan.boundary_values.is_empty());
                assert!(builder.plan.boundary_sites.is_empty());
            }
            reached.set(true);
            Err(source_reference_error_v29("completed original boundary publication limits"))
        });
        assert!(reached.get());
        match mode {
            0 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "completed original boundary publication limits", .. }))),
            1 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(_))))),
            2 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage { .. })))),
            _ => unreachable!(),
        }
    }
}

#[test]
fn original_boundary_revisit_has_a_literal_work_and_header_oracle() {
    for short in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = with_original_boundary_builder(|builder, budget| {
            let row = builder.plan.boundary_values[0];
            builder.effect_site = Some(row.site);
            let map_work = (builder.plan.boundary_sites.len().ilog2() as usize + 2) * 16;
            // Producer owner/site/type24 + lookupM; checked ordinal owner/type24
            // + lookupM; duplicate identity2. No node/child/schema allocation.
            let exact = 24 + map_work + 24 + map_work + 2;
            let headers = std::mem::size_of::<SourceReferenceBoundaryValueV29>()
                + std::mem::size_of::<(usize, u32, Option<usize>, SourceReferenceBoundaryRoleV29)>()
                + 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before = (budget.work(), budget.storage());
            let rows = builder.plan.boundary_values.clone();
            let sites = builder.plan.boundary_sites.clone();
            assert_eq!(builder.retain_boundary_value(row.site, row.role, row.node, budget).is_err(), short);
            assert_eq!(budget.work() - before.0, exact - if short { 2 } else { 0 });
            assert_eq!(budget.storage() - before.1, headers);
            assert_eq!(builder.plan.boundary_values, rows);
            assert_eq!(builder.plan.boundary_sites, sites);
            reached.set(true);
            Ok(())
        });
        assert!(reached.get());
        if short {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(_)))));
        } else {
            result.unwrap();
        }
    }
}
