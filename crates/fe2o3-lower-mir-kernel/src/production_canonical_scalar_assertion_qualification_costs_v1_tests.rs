// Explicit fixture transcript, independent of observed inventories/receipts.
// The original lowerer appends the reserved declaration after the root; scalar
// passes retain that declaration and capability metadata even after elision.
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirCallRefV1, CanonicalKirDefinitionRefV1,
    CanonicalKirEdgeArgumentRefV1, CanonicalKirEdgeRefV1, CanonicalKirEffectRefV1,
    CanonicalKirFunctionRefV1, CanonicalKirInventoryV1, CanonicalKirKernelRefV1,
    CanonicalKirOperationRefV1, CanonicalKirUseRefV1,
};
use fe2o3_kernel_ir::{
    BlockId, CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1, ValueId,
};
use std::mem::size_of;

pub(super) const ROOT_NAME: &str = "assert_root_0";
pub(super) const TRAP_NAME: &str = "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_trap";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Graph {
    LiteralOriginal,
    LiteralFinal,
    DynamicRetained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Counts {
    pub functions: usize,
    pub blocks: usize,
    pub definitions: usize,
    pub operations: usize,
    pub uses: usize,
    pub edges: usize,
    pub arguments: usize,
    pub effects: usize,
    pub calls: usize,
    pub kernels: usize,
}

impl Graph {
    pub(super) fn counts(self) -> Counts {
        match self {
            Self::LiteralOriginal => Counts {
                functions: 2,
                blocks: 3,
                definitions: 1,
                operations: 2,
                uses: 1,
                edges: 2,
                arguments: 0,
                effects: 0,
                calls: 1,
                kernels: 1,
            },
            Self::LiteralFinal => Counts {
                functions: 2,
                blocks: 1,
                definitions: 0,
                operations: 0,
                uses: 0,
                edges: 0,
                arguments: 0,
                effects: 0,
                calls: 0,
                kernels: 1,
            },
            Self::DynamicRetained => Counts {
                functions: 2,
                blocks: 3,
                definitions: 3,
                operations: 3,
                uses: 3,
                edges: 2,
                arguments: 0,
                effects: 0,
                calls: 1,
                kernels: 1,
            },
        }
    }

    pub(super) fn wire_bytes(self) -> usize {
        let text = |bytes: usize| 4 + bytes;
        // wire.rs: fixed header; length-prefixed texts/counts; exact v12
        // extension-capability tag, namespace and name. This is arithmetic,
        // not an invocation of the encoder or an observed wire length.
        let capability = 4 + 1 + text(12) + text(21);
        let module = 20 + text(17 + 64) + 4 + 4 + capability;
        let scalar_type = 2;
        let value_definition = 4 + scalar_type;
        let bool_constant = 4 + value_definition + 1 + 2;
        let u64_constant = 4 + value_definition + 1 + 9;
        let comparison = 4 + value_definition + 1 + 1 + 4 + 4;
        let block_header = 4 + 4 + 4 + 1;
        let conditional = 1 + 4 + 4 + 4 + 4 + 4;
        let empty_return = 1 + 4;
        let trap_call = 4 + 1 + text(44) + 4;
        let unreachable = 1;
        let blocks = match self {
            Self::LiteralOriginal => {
                block_header
                    + bool_constant
                    + conditional
                    + block_header
                    + empty_return
                    + block_header
                    + trap_call
                    + unreachable
            }
            Self::LiteralFinal => block_header + empty_return,
            Self::DynamicRetained => {
                block_header
                    + u64_constant
                    + comparison
                    + conditional
                    + block_header
                    + empty_return
                    + block_header
                    + trap_call
                    + unreachable
            }
        };
        let argument = usize::from(self == Self::DynamicRetained);
        let root = text(13)
            + 4
            + argument * scalar_type
            + 4
            + 1
            + 4
            + argument * 4
            + 4
            + blocks
            + capability;
        let declaration = text(44) + 4 + 4 + 1 + capability;
        // One full physical workgroup: static D1(64), workgroup [64,1,1].
        let kernel = text(13) + text(13) + 1 + 1 + 4 + 1 + 12 + capability;
        module + root + declaration + kernel
    }

    pub(super) fn inventory_storage(self) -> usize {
        size_of::<CanonicalKirInventoryV1<'_>>()
            + self
                .inventory_arrays()
                .iter()
                .map(|(n, width)| n * width)
                .sum::<usize>()
    }

    fn inventory_arrays(self) -> [(usize, usize); 13] {
        let c = self.counts();
        [
            (c.functions, size_of::<CanonicalKirFunctionRefV1<'_>>()),
            (c.blocks, size_of::<CanonicalKirBlockRefV1<'_>>()),
            (c.definitions, size_of::<CanonicalKirDefinitionRefV1<'_>>()),
            (c.operations, size_of::<CanonicalKirOperationRefV1<'_>>()),
            (c.uses, size_of::<CanonicalKirUseRefV1>()),
            (c.edges, size_of::<CanonicalKirEdgeRefV1<'_>>()),
            (c.arguments, size_of::<CanonicalKirEdgeArgumentRefV1>()),
            (c.effects, size_of::<CanonicalKirEffectRefV1<'_>>()),
            (c.calls, size_of::<CanonicalKirCallRefV1<'_>>()),
            (c.kernels, size_of::<CanonicalKirKernelRefV1<'_>>()),
            (
                c.functions,
                size_of::<(&str, CanonicalKirFunctionCoordinateV1)>(),
            ),
            (
                c.blocks,
                size_of::<(CanonicalKirBlockCoordinateV1, BlockId, usize)>(),
            ),
            (
                c.definitions,
                size_of::<(CanonicalKirFunctionCoordinateV1, ValueId, usize)>(),
            ),
        ]
    }

    pub(super) fn inventory(self, trace: &mut Trace) -> usize {
        let c = self.counts();
        let scope = trace.enter(&[]);
        // Census and fill each charge a separate unit before every visit;
        // merging these into one charge would change an interior quota prefix.
        let visits = 1
            + c.functions
            + 2 * c.blocks
            + c.definitions
            + c.operations
            + c.uses
            + c.edges
            + c.arguments
            + c.effects
            + c.calls
            + c.kernels;
        for _ in 0..visits {
            trace.work(1);
        }
        trace.reserve(size_of::<CanonicalKirInventoryV1<'_>>());
        for (count, width) in self.inventory_arrays() {
            if count != 0 {
                trace.work(1);
                trace.reserve(count * width);
            }
        }
        for _ in 0..visits + c.functions + c.blocks + c.definitions {
            trace.work(1);
        }
        // Heap construction/extraction on [root, reserved] compares 14 bytes
        // including its terminator margin. The reserved spelling sorts first.
        for amount in [1, 14, 1, 1] {
            trace.work(amount);
        }
        numeric_heap_ascending(c.blocks, trace);
        numeric_heap_ascending(c.definitions, trace);
        // Every live value is used once in these two source fixtures. Their
        // exact ids are increasing, and the use traversal is argument,
        // constant, comparison (literal: just its condition constant).
        for rank in 0..c.uses {
            trace.work(1);
            numeric_find(c.definitions, rank, trace);
        }
        if c.edges != 0 {
            assert_eq!(c.edges, 2);
            for target in [1, 2] {
                trace.work(1);
                numeric_find(c.blocks, target, trace);
            }
        }
        if c.calls != 0 {
            assert_eq!(c.calls, 1);
            trace.work(1);
            // Binary search visits root, then the exact reserved name.
            for amount in [1, 14, 1, 45] {
                trace.work(amount);
            }
        }
        trace.work(1);
        for amount in [1, 14] {
            trace.work(amount);
        }
        let retained = self.inventory_storage();
        trace.leave(scope);
        retained
    }
}

fn numeric_find(count: usize, rank: usize, trace: &mut Trace) {
    assert!(rank < count);
    let (mut first, mut end) = (0, count);
    loop {
        let middle = first + (end - first) / 2;
        trace.work(1);
        trace.work(1);
        match middle.cmp(&rank) {
            std::cmp::Ordering::Less => first = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return,
        }
    }
}

// Closed comparison schedules, not calls to the inventory's sort/preflight.
// These fixtures have only 0, 1 or 3 increasing numeric keys.
fn numeric_heap_ascending(count: usize, trace: &mut Trace) {
    match count {
        0 | 1 => {}
        3 => {
            for _ in 0..12 {
                trace.work(1);
            }
        }
        _ => panic!("fixture needs a separately derived sorting transcript"),
    }
}

#[test]
fn qualification_fixture_wire_and_inventory_arithmetic_are_literal() {
    assert_eq!(Graph::LiteralOriginal.wire_bytes(), 577);
    assert_eq!(Graph::LiteralFinal.wire_bytes(), 459);
    assert_eq!(Graph::DynamicRetained.wire_bytes(), 610);
    for (graph, work) in [
        (Graph::LiteralOriginal, 169),
        (Graph::LiteralFinal, 53),
        (Graph::DynamicRetained, 203),
    ] {
        let mut trace = Trace::new(29);
        let storage = graph.inventory(&mut trace);
        let predicted = trace.success();
        assert_eq!(predicted.work, work);
        assert_eq!((predicted.storage, predicted.peak), (29, 29 + storage));
    }
}

fn check_inventory_cut(
    owner: &super::CsGraphV1,
    graph: Graph,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) {
    use fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    let mut trace = Trace::new(floor);
    let retained = graph.inventory(&mut trace);
    let predicted = trace.run(work_limit, storage_limit);
    let mut work = Work::new(work_limit);
    {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = CanonicalKirInventoryV1::derive(owner, &mut budget);
        match (predicted.denied_at, result) {
            (None, Ok((inventory, receipt))) => {
                assert_eq!(receipt.retained_storage(), retained);
                budget.reserve_storage(retained).unwrap();
                assert!(std::ptr::eq(inventory.owner(), owner));
                let c = graph.counts();
                assert_eq!(inventory.functions().len(), c.functions);
                assert_eq!(inventory.blocks().len(), c.blocks);
                assert_eq!(inventory.definitions().len(), c.definitions);
                assert_eq!(inventory.operations().len(), c.operations);
                assert_eq!(inventory.uses().len(), c.uses);
                assert_eq!(inventory.edges().len(), c.edges);
                assert_eq!(inventory.edge_arguments().len(), c.arguments);
                assert_eq!(inventory.effects().len(), c.effects);
                assert_eq!(inventory.calls().len(), c.calls);
                assert_eq!(inventory.kernels().len(), c.kernels);
                assert_eq!(
                    owner.canonical().canonical_bytes().len(),
                    graph.wire_bytes()
                );
                assert_eq!(owner.module().functions[0].id.as_str(), ROOT_NAME);
                assert_eq!(owner.module().functions[1].id.as_str(), TRAP_NAME);
                assert!(owner.module().functions[0].body.is_some());
                assert!(owner.module().functions[1].body.is_none());
                drop(inventory);
                budget.release_storage(retained).unwrap();
            }
            (
                Some((_, Denial::Work { .. })),
                Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_))),
            ) => {}
            (
                Some((_, Denial::Storage { .. })),
                Err(CanonicalKirInventoryErrorV1::Resource(Resource::Storage(_))),
            ) => {}
            (_, result) => panic!("inventory cost mismatch: {predicted:?}, {:?}", result.err()),
        }
        assert_eq!(budget.work(), predicted.work);
        assert_eq!(budget.storage(), predicted.storage);
        assert_eq!(budget.peak_storage(), predicted.peak);
        assert_eq!(budget.failed_storage(), predicted.first_storage);
    }
    assert_eq!(work.failed_work(), predicted.first_work);
}

pub(super) fn qualify_graph_components(
    owner: &super::ProductionCanonicalScalarFixedPointOwnerV1,
    dynamic: bool,
) {
    let floor = owner.retained_storage_floor_v1() + 29;
    let original = if dynamic {
        Graph::DynamicRetained
    } else {
        Graph::LiteralOriginal
    };
    let final_graph = if dynamic {
        Graph::DynamicRetained
    } else {
        Graph::LiteralFinal
    };
    for (actual, graph) in [
        (owner.original_source().executable(), original),
        (owner.output(), final_graph),
    ] {
        let mut trace = Trace::new(floor);
        let retained = graph.inventory(&mut trace);
        let complete = trace.success();
        // Complete and one-short logical storage, terminal work and the first
        // array allocation's work debit. No boundary comes from actual receipts.
        check_inventory_cut(actual, graph, floor, complete.work, floor + retained);
        check_inventory_cut(actual, graph, floor, complete.work - 1, usize::MAX);
        check_inventory_cut(actual, graph, floor, usize::MAX, floor + retained - 1);
        let c = graph.counts();
        let census = 1
            + c.functions
            + 2 * c.blocks
            + c.definitions
            + c.operations
            + c.uses
            + c.edges
            + c.arguments
            + c.effects
            + c.calls
            + c.kernels;
        check_inventory_cut(actual, graph, floor, census, usize::MAX);
    }
}
