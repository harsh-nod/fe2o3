use super::*;
use dialect_kernel::{DIALECT_NAME, register_dialect};
use pliron::{
    builtin::op_interfaces::OneRegionInterface, dialect::DialectName,
    linked_list::ContainsLinkedList, parsable::parse_from_str,
};

// This fixture spells the arithmetic and control independently of the
// checker's DAG. In particular, it never emits a guard by reading a formula.
struct DomainFixture {
    text: String,
    next_block: usize,
    next_value: usize,
    commute: bool,
}

impl DomainFixture {
    fn new(commute: bool) -> Self {
        Self {
            text: "builtin.func @checked_domain: builtin.function <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>\n{\n  ^entry(v: kernel.index, c: kernel.index, r: kernel.index, n: kernel.index, s: kernel.index, x: kernel.index):\n".to_owned(),
            next_block: 0, next_value: 0, commute,
        }
    }

    fn constant(&mut self, value: u64) -> String {
        let name = format!("constant{}", self.next_value);
        self.next_value += 1;
        self.text.push_str(&format!("    {name} = kernel.index_constant () [] [kernel_index_value: kernel.index_value {value}]: <() -> (kernel.index)>;\n"));
        name
    }

    fn binary(&mut self, kind: &str, lhs: &str, rhs: &str) -> String {
        let name = format!("arithmetic{}", self.next_value);
        self.next_value += 1;
        let (lhs, rhs) = if self.commute && matches!(kind, "Add" | "Multiply") {
            (rhs, lhs)
        } else {
            (lhs, rhs)
        };
        self.text.push_str(&format!("    {name} = kernel.index_binary ({lhs}, {rhs}) [] [kernel_index_binary_kind: kernel.index_binary_kind {kind}]: <(kernel.index, kernel.index) -> (kernel.index)>;\n"));
        name
    }

    fn label(&mut self) -> String {
        let result = format!("domain{}", self.next_block);
        self.next_block += 1;
        result
    }

    fn start(&mut self, name: &str) {
        self.text.push_str(&format!("\n  ^{name}():\n"));
    }

    fn branch(&mut self, lhs: &str, rhs: &str, yes: &str, no: &str) {
        self.text.push_str(&format!("    kernel.index_lt_br ({lhs}, {rhs}) [^{yes}, ^{no}] []: <(kernel.index, kernel.index) -> ()>\n"));
    }

    fn require(&mut self, lhs: &str, rhs: &str, truth: bool) {
        let next = self.label();
        if truth {
            self.branch(lhs, rhs, &next, "exit");
        } else {
            self.branch(lhs, rhs, "exit", &next);
        }
        self.start(&next);
    }

    fn carry(&mut self, maximum: &str, a: &str, b: &str, c: &str) {
        let quotient = self.binary("Divide", maximum, b);
        let tail = self.binary("Remainder", maximum, b);
        let next = self.label();
        let equal = self.label();
        let tail_check = self.label();
        self.branch(a, &quotient, &next, &equal);
        self.start(&equal);
        self.branch(&quotient, a, "exit", &tail_check);
        self.start(&tail_check);
        self.branch(&tail, c, "exit", &next);
        self.start(&next);
    }
}

#[derive(Clone, Copy, Default)]
struct FixtureOptions {
    constants: Option<[u64; 6]>,
    commute: bool,
    forwarded: bool,
    omit_component: bool,
    early_product: bool,
}

fn domain_source(geometry: CheckedDomainGeometryV1, options: FixtureOptions) -> String {
    let mut fixture = DomainFixture::new(options.commute);
    let roles = options
        .constants
        .map(|roles| roles.map(|value| fixture.constant(value)))
        .unwrap_or_else(|| ["v", "c", "r", "n", "s", "x"].map(str::to_owned));
    let [invocation, component, rows, columns, stride, extent] = &roles;
    let zero = fixture.constant(0);
    let maximum = fixture.constant(u64::MAX);
    let (lanes, elements) = match geometry {
        CheckedDomainGeometryV1::RowStriped([lanes, elements]) => (lanes, elements),
        CheckedDomainGeometryV1::Tiled([lanes, _, _, elements]) => (lanes, elements),
    };
    let lanes = fixture.constant(lanes);
    let elements = fixture.constant(elements);
    let early = options
        .early_product
        .then(|| fixture.binary("Multiply", component, &lanes));
    if !options.omit_component {
        fixture.require(component, &elements, true);
    }
    let lane = fixture.binary("Remainder", invocation, &lanes);
    let (row, column) = match geometry {
        CheckedDomainGeometryV1::RowStriped(_) => {
            let row = fixture.binary("Divide", invocation, &lanes);
            let base = early.unwrap_or_else(|| fixture.binary("Multiply", component, &lanes));
            let column = fixture.binary("Add", &base, &lane);
            (row, column)
        }
        CheckedDomainGeometryV1::Tiled([_, height, width, _]) => {
            let height = fixture.constant(height);
            let width_value = width;
            let width = fixture.constant(width);
            let padding = fixture.constant(width_value - 1);
            let maximum_columns = fixture.constant(u64::MAX - (width_value - 1));
            fixture.require(&maximum_columns, columns, false);
            let rounded = fixture.binary("Add", columns, &padding);
            let tiles = fixture.binary("Divide", &rounded, &width);
            fixture.require(&zero, &tiles, true);
            let tile = fixture.binary("Divide", invocation, &lanes);
            let tile_row = fixture.binary("Divide", &tile, &tiles);
            let tile_column = fixture.binary("Remainder", &tile, &tiles);
            let lane_row = fixture.binary("Divide", &lane, &width);
            let local_base = fixture.binary("Multiply", &lane_row, &elements);
            let local_row = fixture.binary("Add", &local_base, component);
            let local_column = fixture.binary("Remainder", &lane, &width);
            fixture.carry(&maximum, &tile_row, &height, &local_row);
            let row_base = fixture.binary("Multiply", &tile_row, &height);
            let row = fixture.binary("Add", &row_base, &local_row);
            fixture.carry(&maximum, &tile_column, &width, &local_column);
            let column_base = fixture.binary("Multiply", &tile_column, &width);
            let column = fixture.binary("Add", &column_base, &local_column);
            (row, column)
        }
    };
    fixture.require(&row, rows, true);
    fixture.require(&column, columns, true);
    fixture.require(stride, columns, false);
    fixture.require(&zero, stride, true);
    fixture.carry(&maximum, &row, stride, &column);
    let base = fixture.binary("Multiply", &row, stride);
    let index = fixture.binary("Add", &base, &column);
    fixture.require(&index, extent, true);
    let roles = if options.forwarded {
        fixture.text.push_str(&format!("    kernel.br_args ({}) [^access] []: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> ()>\n", roles.join(", ")));
        fixture.text.push_str("\n  ^access(av: kernel.index, ac: kernel.index, ar: kernel.index, an: kernel.index, ast: kernel.index, ax: kernel.index):\n");
        ["av", "ac", "ar", "an", "ast", "ax"].map(str::to_owned)
    } else {
        roles
    };
    let extent = &roles[5];
    fixture.text.push_str(&format!("    output = kernel.ranked_view ({extent}) [] [kernel_memory_space: kernel.memory_space Global, kernel_allocation_origin: kernel.allocation_origin 0, kernel_noalias_class: kernel.noalias_class 0]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;\n"));
    let (operation, attributes) = match geometry {
        CheckedDomainGeometryV1::RowStriped([lanes, elements]) => (
            "checked_row_striped_index_2d",
            format!(
                "kernel_lanes_per_row: kernel.index_value {lanes}, kernel_row_striped_elements_per_lane: kernel.index_value {elements}"
            ),
        ),
        CheckedDomainGeometryV1::Tiled([lanes, rows, columns, elements]) => (
            "checked_tiled_index_2d",
            format!(
                "kernel_lanes_per_tile: kernel.index_value {lanes}, kernel_tile_rows: kernel.index_value {rows}, kernel_tile_columns: kernel.index_value {columns}, kernel_elements_per_lane: kernel.index_value {elements}"
            ),
        ),
    };
    fixture.text.push_str(&format!("    checked, success = kernel.{operation} ({}) [] [{attributes}]: <(kernel.index, kernel.index, kernel.index, kernel.index, kernel.index, kernel.index) -> (kernel.index, kernel.checked_access_capability)>;\n", roles.join(", ")));
    fixture.text.push_str("    kernel.access (output, checked, success) [] [kernel_access_kind: kernel.access_kind Write]: <(kernel.ranked_view <32,true,[0]>, kernel.index, kernel.checked_access_capability) -> ()>;\n    kernel.br () [^exit] []: <() -> ()>\n\n  ^exit():\n    kernel.return () [] []: <() -> ()>\n}\n");
    fixture.text
}

fn with_domain_graph<T>(
    source: &str,
    query: impl FnOnce(&BoundsEdgeTransportV1<'_>, &FuncOp) -> T,
) -> T {
    let mut context = Context::new();
    fe2o3_pliron_owner_core::ensure_context_identity(&mut context).unwrap();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, source).unwrap();
    verify_operation(operation, &context).unwrap();
    let function = FuncOp::from_operation(operation);
    let blocks = function
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .collect::<Vec<_>>();
    let mut predecessors = vec![Vec::new(); blocks.len()];
    for (block, pointer) in blocks.iter().enumerate() {
        let terminator = pointer.deref(&context).get_terminator(&context).unwrap();
        for (successor, target) in terminator.deref(&context).successors().enumerate() {
            let target = blocks
                .iter()
                .position(|pointer| *pointer == target)
                .unwrap();
            predecessors[target].push(PredecessorEdge {
                block,
                successor,
                guard_fact: None,
            });
        }
    }
    query(
        &BoundsEdgeTransportV1 {
            context: &context,
            blocks: &blocks,
            predecessors: &predecessors,
        },
        &function,
    )
}

fn domain_query(
    graph: &BoundsEdgeTransportV1<'_>,
    budget: &mut RankedBoundsBudget,
) -> Result<bool, RankedBoundsFindingV1> {
    for (block, pointer) in graph.blocks.iter().enumerate() {
        for operation in pointer.deref(graph.context).iter(graph.context) {
            let Some(access) = Operation::get_op::<RankedAccessOp>(operation, graph.context) else {
                continue;
            };
            let view = access.view(graph.context);
            let ty = ranked_view_type(view, graph.context).unwrap();
            return checked_domain_bound_is_proven_v1(
                graph,
                block,
                access.indices(graph.context)[0],
                (
                    view,
                    0,
                    extent_expr(view, &ty.deref(graph.context), 0, graph.context),
                ),
                budget,
            );
        }
    }
    panic!("fixture must retain its following effect")
}
