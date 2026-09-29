//! Bounded read-only dependencies, not an alias, dominance or convergence proof.
use super::*;
use fe2o3_kernel_ir::{BasicBlock, Operation};

const INDEX_LIMIT: usize = 32768;
const OPERATION_LIMIT: usize = 32768;
const NODE_LIMIT: usize = 4096;
const EDGE_LIMIT: usize = 32768;

#[derive(Clone, Copy)]
enum Definition<'a> {
    Parameter(usize),
    BlockParameter(&'a BasicBlock, usize),
    Result(&'a BasicBlock, usize, &'a Operation, usize),
}

struct Row<'a> {
    id: ValueId,
    definition: Definition<'a>,
}

struct Slice<'a> {
    index: Vec<Row<'a>>,
    queue: Vec<ValueId>,
    node_limit: usize,
    processed: usize,
    incomplete: bool,
    unresolved: usize,
    opaque: usize,
    edges: usize,
}

fn selector(terminator: Option<&Terminator>) -> Option<ValueId> {
    match terminator {
        Some(Terminator::ConditionalBranch { condition, .. }) => Some(*condition),
        Some(Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. }) => {
            Some(*selector)
        }
        _ => None,
    }
}

impl<'a> Slice<'a> {
    fn new(node_limit: usize) -> Option<Self> {
        let mut index = Vec::new();
        let mut queue = Vec::new();
        index.try_reserve_exact(INDEX_LIMIT).ok()?;
        queue.try_reserve_exact(node_limit).ok()?;
        Some(Self {
            index,
            queue,
            node_limit,
            processed: 0,
            incomplete: false,
            unresolved: 0,
            opaque: 0,
            edges: 0,
        })
    }

    fn insert(&mut self, id: ValueId, definition: Definition<'a>) {
        if self.index.len() == INDEX_LIMIT {
            self.incomplete = true;
        } else {
            self.index.push(Row { id, definition });
        }
    }

    fn enqueue<W: Write>(&mut self, out: &mut Trace<W>, id: ValueId) {
        // At most NODE_LIMIT squared equality checks, with no growing set.
        if self.queue.contains(&id) {
            return;
        }
        if self.queue.len() == self.node_limit {
            self.incomplete = true;
            out.row(format_args!("SLICE_OMIT id={} reason=node_limit", id.0));
        } else {
            self.queue.push(id);
        }
    }

    fn index<W: Write>(&mut self, out: &mut Trace<W>, body: &'a FunctionBody) {
        out.row(format_args!(
            "SLICE_BEGIN parameters={} blocks={} index_limit={INDEX_LIMIT} node_limit={} operation_limit={OPERATION_LIMIT}",
            body.parameters.len(), body.blocks.len(), self.node_limit
        ));
        self.incomplete |= body.parameters.len() > MAX_ITEMS || body.blocks.len() > MAX_BLOCKS;
        for (ordinal, id) in body.parameters.iter().take(MAX_ITEMS).enumerate() {
            self.insert(*id, Definition::Parameter(ordinal));
        }
        let mut operations = 0;
        for (ordinal, block) in body.blocks.iter().take(MAX_BLOCKS).enumerate() {
            if out.stopped() {
                self.incomplete = true;
                break;
            }
            out.row(format_args!(
                "SLICE_BLOCK ordinal={ordinal} id={} parameters={} operations={}",
                block.id.0,
                block.parameters.len(),
                block.operations.len()
            ));
            out.terminator(block.terminator.as_ref());
            self.incomplete |= block.parameters.len() > MAX_ITEMS;
            for (index, value) in block.parameters.iter().take(MAX_ITEMS).enumerate() {
                self.insert(value.id, Definition::BlockParameter(block, index));
            }
            if let Some(value) = selector(block.terminator.as_ref()) {
                out.row(format_args!(
                    "SLICE_ROOT block={} id={}",
                    block.id.0, value.0
                ));
                self.enqueue(out, value);
            }
            for (index, operation) in block.operations.iter().enumerate() {
                if operations == OPERATION_LIMIT {
                    self.incomplete = true;
                    break;
                }
                operations += 1;
                self.incomplete |= operation.results.len() > MAX_ITEMS;
                for (result, value) in operation.results.iter().take(MAX_ITEMS).enumerate() {
                    self.insert(
                        value.id,
                        Definition::Result(block, index, operation, result),
                    );
                }
            }
        }
        self.index.sort_unstable_by_key(|row| row.id);
        if self.index.windows(2).any(|rows| rows[0].id == rows[1].id) {
            self.incomplete = true;
            out.row(format_args!("SLICE_INVALID duplicate_definition=1"));
        }
        out.row(format_args!(
            "SLICE_INDEX definitions={} operations={operations}",
            self.index.len()
        ));
    }

    fn incoming<W: Write>(
        &mut self,
        out: &mut Trace<W>,
        body: &FunctionBody,
        target: BlockId,
        ordinal: usize,
    ) {
        let before = self.edges;
        let mut matched = 0;
        for block in body.blocks.iter().take(MAX_BLOCKS) {
            if self.edges == EDGE_LIMIT || out.stopped() {
                self.incomplete = true;
                break;
            }
            let mut edge =
                |role: &str, destination: BlockId, arguments: &[ValueId]| {
                    if self.edges == EDGE_LIMIT {
                        self.incomplete = true;
                        return;
                    }
                    self.edges += 1;
                    if destination != target {
                        return;
                    }
                    matched += 1;
                    out.row(format_args!(
                    "SLICE_INCOMING source={} target={} role={role} ordinal={ordinal} arguments={}",
                    block.id.0, target.0, arguments.len()
                ));
                    if let Some(value) = arguments.get(ordinal) {
                        out.row(format_args!("SLICE_DEP role=incoming id={}", value.0));
                        self.enqueue(out, *value);
                    } else {
                        self.incomplete = true;
                        out.row(format_args!("SLICE_INVALID missing_incoming_argument=1"));
                    }
                    if let Some(value) = selector(block.terminator.as_ref()) {
                        out.row(format_args!(
                            "SLICE_DEP role=incoming_selector id={}",
                            value.0
                        ));
                        self.enqueue(out, value);
                    }
                };
            match block.terminator.as_ref() {
                Some(Terminator::Branch { target, arguments }) => {
                    edge("branch", *target, arguments)
                }
                Some(Terminator::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                }) => {
                    edge("then", *then_target, then_arguments);
                    edge("else", *else_target, else_arguments);
                }
                Some(Terminator::Switch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                }) => {
                    for case in cases.iter().take(MAX_ITEMS) {
                        edge("case", case.target, &case.arguments);
                    }
                    edge("default", *default_target, default_arguments);
                    self.incomplete |= cases.len() > MAX_ITEMS;
                }
                Some(Terminator::IntegerSwitch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                }) => {
                    for case in cases.iter().take(MAX_ITEMS) {
                        edge("integer_case", case.target, &case.arguments);
                    }
                    edge("default", *default_target, default_arguments);
                    self.incomplete |= cases.len() > MAX_ITEMS;
                }
                _ => {}
            }
        }
        if matched == 0 {
            self.unresolved += 1;
            self.incomplete = true;
            out.row(format_args!(
                "SLICE_MISSING incoming_target={} ordinal={ordinal}",
                target.0
            ));
        }
        out.row(format_args!(
            "SLICE_INCOMING_END target={} ordinal={ordinal} scanned_edges={} matched={matched}",
            target.0,
            self.edges - before
        ));
    }

    fn visit<W: Write>(
        &mut self,
        out: &mut Trace<W>,
        body: &FunctionBody,
        types: Option<&[Type]>,
        report: Option<&AnalysisReport>,
        id: ValueId,
    ) {
        out.row(format_args!(
            "SLICE_NODE id={} variation={:?}",
            id.0,
            report.map(|r| r.value(id))
        ));
        let first = self.index.partition_point(|row| row.id < id);
        let Some(row) = self.index.get(first).filter(|row| row.id == id) else {
            self.unresolved += 1;
            self.incomplete = true;
            out.row(format_args!("SLICE_MISSING id={}", id.0));
            return;
        };
        if self.index.get(first + 1).is_some_and(|next| next.id == id) {
            self.unresolved += 1;
            self.incomplete = true;
            out.row(format_args!("SLICE_AMBIGUOUS id={}", id.0));
            return;
        }
        match row.definition {
            Definition::Parameter(ordinal) => out.row(format_args!(
                "SLICE_PARAMETER ordinal={ordinal} id={} scalar={:?}",
                id.0,
                types
                    .and_then(|types| types.get(ordinal))
                    .and_then(Type::as_scalar)
            )),
            Definition::BlockParameter(block, ordinal) => {
                out.row(format_args!(
                    "SLICE_PHI block={} ordinal={ordinal} id={} scalar={:?}",
                    block.id.0,
                    id.0,
                    block.parameters[ordinal].ty.as_scalar()
                ));
                self.incoming(out, body, block.id, ordinal);
            }
            Definition::Result(block, index, operation, ordinal) => {
                out.row(format_args!(
                    "SLICE_SITE block={} operation={index} ordinal={ordinal} id={} scalar={:?}",
                    block.id.0,
                    id.0,
                    operation.results[ordinal].ty.as_scalar()
                ));
                let unknown = out.unknown_operations;
                out.operation(&operation.kind);
                if out.unknown_operations != unknown
                    || matches!(
                        &operation.kind,
                        OperationKind::Load { .. }
                            | OperationKind::GuardedLoad { .. }
                            | OperationKind::Call { .. }
                    )
                {
                    self.opaque += 1;
                    out.row(format_args!(
                        "SLICE_OPAQUE id={} memory_or_callee_semantics_not_followed=1",
                        id.0
                    ));
                }
                let mut operands = 0;
                let result = operation.kind.try_visit_operands(|value| {
                    if operands == MAX_ITEMS || out.stopped() {
                        return Err(());
                    }
                    out.row(format_args!(
                        "SLICE_DEP role=operand ordinal={operands} id={}",
                        value.0
                    ));
                    operands += 1;
                    self.enqueue(out, value);
                    Ok(())
                });
                self.incomplete |= result.is_err();
                out.row(format_args!(
                    "SLICE_OPERANDS count={operands} truncated={}",
                    usize::from(result.is_err())
                ));
            }
        }
    }
}

impl<W: Write> Trace<W> {
    pub(super) fn selector_slice(
        &mut self,
        body: &FunctionBody,
        types: Option<&[Type]>,
        report: Option<&AnalysisReport>,
    ) {
        self.selector_slice_with_limit(body, types, report, NODE_LIMIT);
    }

    fn selector_slice_with_limit(
        &mut self,
        body: &FunctionBody,
        types: Option<&[Type]>,
        report: Option<&AnalysisReport>,
        node_limit: usize,
    ) {
        let Some(mut slice) = Slice::new(node_limit) else {
            self.truncated = true;
            self.row(format_args!("SLICE_ABORT allocation_failed=1"));
            return;
        };
        slice.index(self, body);
        while slice.processed < slice.queue.len() && !self.stopped() {
            let id = slice.queue[slice.processed];
            slice.processed += 1;
            slice.visit(self, body, types, report, id);
        }
        slice.incomplete |= slice.processed != slice.queue.len();
        self.truncated |= slice.incomplete;
        self.row(format_args!(
            "SLICE_END queued={} processed={} unresolved={} opaque={} scanned_edges={} incomplete={} semantic_proof=0",
            slice.queue.len(), slice.processed, slice.unresolved, slice.opaque,
            slice.edges, usize::from(slice.incomplete)
        ));
    }
}

#[cfg(test)]
#[path = "diagnostic_selector_slice_v1794_tests.rs"]
mod tests;
