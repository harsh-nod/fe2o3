use super::*;
use crate::CanonicalKernelIrWorkBudgetV1;

fn fixture(count: usize, seeded: bool) -> PointerGraphV30 {
    let function = FunctionCoordinate(0);
    let block = BlockCoordinate { function, block: 0 };
    let mut graph = PointerGraphV30::empty();
    for node in 0..count {
        graph.nodes.push(CanonicalSelectedPointerNodeV30 {
            definition: Definition::BlockArgument {
                block,
                argument: node as u32,
            },
            value: ValueId(node as u32),
            scalar: Some(ScalarType::U32),
            space: AddressSpace::Generic,
            access: AccessMode::ReadOnly,
            step: if node == 0 && seeded {
                CanonicalSelectedPointerStepV30::Formation {
                    operation: Coordinate {
                        block,
                        operation: 0,
                    },
                }
            } else {
                CanonicalSelectedPointerStepV30::Parameter {
                    first: graph.incoming.len(),
                    count: 2,
                }
            },
        });
        if node == 0 && seeded {
            continue;
        }
        for (edge, argument) in [node.saturating_sub(1), (node + 1) % count]
            .into_iter()
            .enumerate()
        {
            graph.incoming.push(CanonicalSelectedPointerIncomingV30 {
                occurrence: EdgeArgument {
                    edge: EdgeCoordinate {
                        source: block,
                        successor: edge as u32,
                    },
                    argument: node as u32,
                },
                source: BlockId(0),
                target: BlockId(0),
                parameter: node,
                argument,
                reachable: true,
            });
        }
    }
    graph
}

#[test]
fn selected_pointer_seed_propagation_is_indexed_and_never_selects_one_predecessor() {
    for count in [8usize, 64, 512, 4096] {
        for seeded in [false, true] {
            let mut graph = fixture(count, seeded);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
            let mut budget = Budget::new(&mut work, 100_000_000);
            let mut meter = LiveGuardMeter::new(&mut budget, 100_000_000, 100_000_000, 100_000_000);
            graph.resolve_seeds(&mut meter).unwrap();
            assert_eq!(graph.seeded.len(), count);
            assert!(graph.seeded.iter().all(|row| *row == seeded));
            drop(meter);
            // One sort plus one reverse adjacency traversal; no repeated
            // full-node seed fixed point. The constants include vector growth.
            let log = usize::BITS as usize - (2 * count - 1).leading_zeros() as usize;
            assert!(
                budget.work() <= 100 * count * (log + 1),
                "{count}: {}",
                budget.work()
            );
            assert!(
                budget.peak_storage() <= 200 * count,
                "{count}: {}",
                budget.peak_storage()
            );
        }
    }
}

#[test]
fn selected_pointer_graph_charges_its_own_zero_result_operation_traversal() {
    let mut measured = Vec::new();
    for count in [0usize, 8, 64, 512, 4096] {
        let mut module = super::super::tests::fixture(true, false, false, false);
        let function = &mut module.functions[0];
        let body = function.body.as_mut().unwrap();
        body.blocks[0].operations.push(Operation::effect_free(
            crate::ValueDef::new(
                ValueId(17),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ));
        body.blocks[0].operations.extend((0..count).map(|_| {
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(17),
                    value: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            )
        }));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let flow = crate::control_flow::analyze_control_flow_with_verification_budget_v1(
            function,
            crate::ControlFlowLimits::DEFAULT,
            &mut budget,
        )
        .unwrap();
        let meter = LiveGuardMeter::new(&mut budget, usize::MAX, usize::MAX, usize::MAX);
        let GuardedControlCollectionV1::Selected(seed) =
            GuardedControlV1::collect_preserving_ledger_v24::<false>(
                function,
                flow.indexed_v15(),
                meter,
            )
            .unwrap()
        else {
            panic!("genuine guarded selected read");
        };
        let mut analysis = GuardedAnalysisV1::empty(seed, false);
        collect_actual_definitions(&mut analysis, function).unwrap();
        collect_actual_origins(&mut analysis, function, flow.indexed_v15()).unwrap();
        // Earlier CFG/definition scans are deliberately excluded. Only this
        // pointer-graph pass must pay for each additional zero-result Store.
        let before = analysis.ledger.budget.work();
        let graph = PointerGraphV30::build(
            function,
            FunctionCoordinate(0),
            flow.indexed_v15(),
            &mut analysis,
        )
        .unwrap();
        let own_work = analysis.ledger.budget.work() - before;
        measured.push((count, own_work, graph.nodes.len(), graph.incoming.len()));
    }
    let baseline = measured[0];
    for (count, work, nodes, incoming) in measured {
        assert_eq!(work - baseline.1, count);
        assert_eq!((nodes, incoming), (baseline.2, baseline.3));
    }
}
