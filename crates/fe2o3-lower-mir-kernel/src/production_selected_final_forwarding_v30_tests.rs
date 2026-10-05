use super::*;

fn block(at: usize) -> Block {
    Block {
        function: FunctionCoordinate(0),
        block: at as u32,
    }
}

fn formation(at: usize) -> ActualNode {
    let operation = OpCoordinate {
        block: block(at),
        operation: 0,
    };
    ActualNode {
        definition: Definition::Result {
            operation,
            result: 0,
        },
        value: ValueId(at as u32),
        scalar: Some(ScalarType::U32),
        space: AddressSpace::Generic,
        access: AccessMode::ReadWrite,
        step: ActualStep::Formation { operation },
    }
}

fn parameter(at: usize, first: usize, count: usize) -> ActualNode {
    ActualNode {
        definition: Definition::BlockArgument {
            block: block(at),
            argument: 0,
        },
        value: ValueId(at as u32),
        scalar: Some(ScalarType::U32),
        space: AddressSpace::Generic,
        access: AccessMode::ReadWrite,
        step: ActualStep::Parameter { first, count },
    }
}

fn edge(input: usize, target: usize, ordinal: u32, reachable: bool) -> ActualIncoming {
    ActualIncoming {
        occurrence: EdgeArgument {
            edge: Edge {
                source: block(input),
                successor: ordinal,
            },
            argument: 0,
        },
        source: BlockId(input as u32),
        target: BlockId(target as u32),
        parameter: target,
        argument: input,
        reachable,
    }
}

fn run<T>(consume: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> T) -> T {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    consume(&mut budget)
}

fn solve(
    nodes: &[ActualNode],
    edges: &[ActualIncoming],
    terminals: &[usize],
    budget: &mut ArgumentBudgetV1<'_>,
) -> ForwardingV30 {
    let mut anchors = resources::vector(nodes.len(), budget).unwrap();
    budget.charge_work(nodes.len()).unwrap();
    anchors.resize(nodes.len(), false);
    for &at in terminals {
        anchors[at] = true;
    }
    ForwardingV30::derive(nodes, edges, anchors, budget).unwrap()
}

#[test]
fn selected_final_forwarding_keeps_original_parameter_cuts_and_helper_carriers() {
    run(|budget| {
        let nodes = [
            formation(0),
            formation(1),
            parameter(2, 0, 2),
            parameter(3, 2, 1),
        ];
        let edges = [
            edge(0, 2, 0, true),
            edge(1, 2, 0, true),
            edge(2, 3, 0, true),
        ];
        let forwarding = solve(&nodes, &edges, &[2], budget);
        assert_eq!(forwarding.origin(3, budget).unwrap(), Some(2));
        assert_eq!(
            forwarding.edge(2, 3, &nodes, &edges, budget).unwrap(),
            Some(2)
        );
        // The two original phi inputs still require exact source projections.
        assert_eq!(forwarding.edge(0, 2, &nodes, &edges, budget).unwrap(), None);
        assert_eq!(forwarding.edge(1, 2, &nodes, &edges, budget).unwrap(), None);
        let changed = [edges[0], edges[1], edge(1, 3, 0, true)];
        let mut forwarding = solve(&nodes, &changed, &[2], budget);
        assert_ne!(
            forwarding.edge(2, 3, &nodes, &changed, budget).unwrap(),
            Some(2)
        );
        let mut path = Vec::new();
        assert!(
            !forwarding
                .path(
                    3,
                    2,
                    2,
                    SelectedFinalForwardingStepV30::Incoming { actual: 2 },
                    &nodes,
                    &changed,
                    &mut path,
                    budget
                )
                .unwrap()
        );
        assert!(path.is_empty());
    });
}

#[test]
fn selected_final_forwarding_refuses_parallel_conflicts_including_unreachable_inputs() {
    run(|budget| {
        let nodes = [formation(0), formation(1), parameter(2, 0, 2)];
        for reachable in [false, true] {
            let edges = [edge(0, 2, 0, true), edge(1, 2, 1, reachable)];
            let forwarding = solve(&nodes, &edges, &[], budget);
            assert_eq!(forwarding.origin(2, budget).unwrap(), None);
            assert_eq!(forwarding.edge(0, 2, &nodes, &edges, budget).unwrap(), None);
            assert_eq!(forwarding.edge(1, 2, &nodes, &edges, budget).unwrap(), None);
        }
    });
}

#[test]
fn selected_final_forwarding_refuses_ungrounded_cycles_and_preserves_seeded_copies() {
    run(|budget| {
        let nodes = [formation(0), parameter(1, 0, 2), parameter(2, 2, 1)];
        let edges = [
            edge(0, 1, 0, true),
            edge(2, 1, 0, true),
            edge(1, 2, 0, true),
        ];
        let forwarding = solve(&nodes, &edges, &[], budget);
        assert_eq!(forwarding.origin(1, budget).unwrap(), Some(0));
        assert_eq!(forwarding.origin(2, budget).unwrap(), Some(0));
        let edges = [
            edge(2, 1, 0, true),
            edge(2, 1, 1, true),
            edge(1, 2, 0, true),
        ];
        let forwarding = solve(&nodes, &edges, &[], budget);
        assert_eq!(forwarding.origin(1, budget).unwrap(), None);
        assert_eq!(forwarding.origin(2, budget).unwrap(), None);
    });
}

#[test]
fn selected_final_forwarding_injection_requires_the_exact_path_not_a_shared_leaf() {
    run(|budget| {
        let nodes = [
            formation(0),
            parameter(1, 0, 1),
            parameter(2, 1, 1),
            parameter(3, 2, 1),
        ];
        let edges = [
            edge(0, 1, 0, true),
            edge(1, 2, 0, true),
            edge(0, 3, 1, true),
        ];
        let mut forwarding = solve(&nodes, &edges, &[], budget);
        let mut path = Vec::new();
        assert!(
            forwarding
                .path(
                    1,
                    2,
                    0,
                    SelectedFinalForwardingStepV30::Incoming { actual: 0 },
                    &nodes,
                    &edges,
                    &mut path,
                    budget
                )
                .unwrap()
        );
        assert_eq!(
            path,
            [
                SelectedFinalForwardingStepV30::Incoming { actual: 0 },
                SelectedFinalForwardingStepV30::Incoming { actual: 1 }
            ]
        );
        let before = path.clone();
        // A sibling projection carries the identical formation but does not
        // descend from this injection occurrence.
        assert!(
            !forwarding
                .path(
                    1,
                    3,
                    0,
                    SelectedFinalForwardingStepV30::Incoming { actual: 0 },
                    &nodes,
                    &edges,
                    &mut path,
                    budget
                )
                .unwrap()
        );
        assert_eq!(path, before);
    });
}

#[test]
fn selected_final_forwarding_path_records_casts_and_cannot_cross_a_source_cut() {
    run(|budget| {
        let mut cast = formation(2);
        cast.step = ActualStep::Cast { input: 1 };
        let nodes = [formation(0), parameter(1, 0, 1), cast, parameter(3, 1, 1)];
        let edges = [edge(0, 1, 0, true), edge(2, 3, 0, true)];
        let mut forwarding = solve(&nodes, &edges, &[3], budget);
        let mut path = Vec::new();
        assert!(
            forwarding
                .path(
                    1,
                    2,
                    0,
                    SelectedFinalForwardingStepV30::Incoming { actual: 0 },
                    &nodes,
                    &edges,
                    &mut path,
                    budget
                )
                .unwrap()
        );
        assert_eq!(
            path.last(),
            Some(&SelectedFinalForwardingStepV30::Cast {
                input: 1,
                target: 2
            })
        );
        assert!(
            !forwarding
                .path(
                    1,
                    3,
                    0,
                    SelectedFinalForwardingStepV30::Incoming { actual: 0 },
                    &nodes,
                    &edges,
                    &mut path,
                    budget
                )
                .unwrap()
        );
    });
}

#[test]
fn selected_final_forwarding_chain_has_independent_work_and_backing_oracles() {
    for count in [64usize, 128, 512] {
        let mut nodes = vec![formation(0)];
        let mut edges = Vec::new();
        for at in 1..count {
            nodes.push(parameter(at, edges.len(), 1));
            edges.push(edge(at - 1, at, 0, true));
        }
        // One anchor-vector reservation and n writes; 3n counting; five
        // solver vectors and 4n initialization; 3n seeds; 2n node visits and
        // two visits per link; 5n-1 propagation; three path vectors and 2n writes.
        let expected_work = 1
            + count
            + 3 * count
            + 5
            + 4 * count
            + 3 * count
            + 2 * count
            + 2 * (count - 1)
            + 5 * count
            - 1
            + 3
            + 2 * count;
        assert_eq!(expected_work, 22 * count + 6);
        let expected_storage = count
            * (size_of::<OriginStateV1<usize>>()
                + 4 * size_of::<usize>()
                + 2 * size_of::<bool>()
                + size_of::<Option<ParentV30>>())
            + (count - 1) * size_of::<(usize, usize)>();
        for (work_limit, storage_limit, success) in [
            (expected_work, 17 + expected_storage, true),
            (expected_work - 1, 17 + expected_storage, false),
            (expected_work, 17 + expected_storage - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(17).unwrap();
            let result = (|| {
                let mut anchors = resources::vector(count, &mut budget)?;
                budget.charge_work(count)?;
                anchors.resize(count, false);
                ForwardingV30::derive(&nodes, &edges, anchors, &mut budget)
            })();
            if success {
                let forwarding = result.unwrap();
                assert_eq!(forwarding.origins.last(), Some(&OriginStateV1::Exact(0)));
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    (expected_work, 17 + expected_storage, 17 + expected_storage)
                );
                drop(forwarding);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Work(_) | ArgumentResourceV1::Storage(_)
                    ))
                ));
            }
            budget.release_storage(budget.storage() - 17).unwrap();
            assert_eq!(budget.storage(), 17);
        }
    }
}

#[test]
fn selected_final_forwarding_fixed_frames_have_independent_field_oracles() {
    type Fields = (
        Vec<bool>,
        Vec<OriginStateV1<usize>>,
        Vec<usize>,
        Vec<Option<ParentV30>>,
        Vec<usize>,
        usize,
    );
    assert_eq!(size_of::<ForwardingV30>(), size_of::<Fields>());
    assert_eq!(
        size_of::<ParentV30>(),
        size_of::<(usize, SelectedFinalForwardingStepV30)>()
    );
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<Fields>()
        + h::<OriginWorkV1<usize>>()
        + size_of::<Result<OriginWorkV1<usize>, OriginWorkErrorV1>>()
        + size_of::<Result<Vec<OriginStateV1<usize>>, OriginWorkErrorV1>>()
        + 3 * size_of::<Result<(), OriginWorkErrorV1>>()
        + h::<Result<usize, usize>>()
        + h::<Vec<bool>>()
        + h::<Vec<OriginStateV1<usize>>>()
        + 2 * h::<Vec<usize>>()
        + h::<Vec<Option<ParentV30>>>()
        + h::<ParentV30>()
        + h::<Option<ParentV30>>()
        + h::<Option<usize>>()
        + h::<SelectedFinalForwardingStepV30>()
        + h::<Vec<SelectedFinalForwardingStepV30>>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()
        + h::<&[SelectedTransportRowV30]>()
        + h::<Option<&SelectedTransportRowV30>>()
        + h::<&[ActualNode]>()
        + h::<&[ActualIncoming]>()
        + h::<ActualNode>()
        + h::<ActualIncoming>()
        + h::<OriginStateV1<usize>>()
        + h::<Definition>()
        + h::<[usize; 16]>()
        + h::<[bool; 4]>()
        + h::<()>();
    assert_eq!(headers().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 17 + expected - usize::from(short));
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(headers().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == 17 + expected && error.limit() == 16 + expected));
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!((budget.work(), budget.storage()), (0, 17));
    }
}
