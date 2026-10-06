//! Actual pointer equations. Ordered incoming edges are not an origin union.
use super::*;
use crate::{
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument,
    CanonicalKirEdgeCoordinateV1 as EdgeCoordinate,
};

/// Inert description of one actual pointer definition. Only a checked selected
/// domain owner can give these equations conditional memory meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalSelectedPointerStepV30 {
    Unsupported,
    Formation { operation: Coordinate },
    Cast { input: usize },
    Parameter { first: usize, count: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSelectedPointerNodeV30 {
    pub definition: Definition,
    pub value: ValueId,
    pub scalar: Option<ScalarType>,
    pub space: AddressSpace,
    pub access: AccessMode,
    pub step: CanonicalSelectedPointerStepV30,
}

/// Parallel predecessor edges retain their actual source ordinal. Disconnected
/// predecessors remain explicit but cannot establish a reachable bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalSelectedPointerIncomingV30 {
    pub occurrence: EdgeArgument,
    pub source: BlockId,
    pub target: BlockId,
    pub parameter: usize,
    pub argument: usize,
    pub reachable: bool,
}

pub(super) struct PointerGraphV30 {
    pub(super) nodes: Vec<CanonicalSelectedPointerNodeV30>,
    pub(super) incoming: Vec<CanonicalSelectedPointerIncomingV30>,
    // Casts resolve to a formation or parameter, never through a chosen edge.
    pub(super) terminal: Vec<Option<usize>>,
    pub(super) seeded: Vec<bool>,
}

fn pointer_node_v30(
    definition: Definition,
    value: ValueId,
    ty: &Type,
) -> Option<CanonicalSelectedPointerNodeV30> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    Some(CanonicalSelectedPointerNodeV30 {
        definition,
        value,
        scalar: pointer.pointee.as_scalar(),
        space: pointer.address_space,
        access: pointer.access,
        step: CanonicalSelectedPointerStepV30::Unsupported,
    })
}

impl PointerGraphV30 {
    pub(super) fn find<M: GuardMeter>(
        &self,
        value: ValueId,
        meter: &mut M,
    ) -> std::result::Result<Option<usize>, ResourceError> {
        meter.find(&self.nodes, |row| row.value.cmp(&value))
    }

    pub(super) fn build<'g, M: GuardMeter>(
        function: &'g Function,
        coordinate: FunctionCoordinate,
        flow: &IndexedControlFlow,
        analysis: &mut GuardedAnalysisV1<'g, M>,
    ) -> std::result::Result<Self, ResourceError> {
        let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
        analysis.ledger.storage(pointer_graph_headers_v30()?)?;
        let mut graph = Self {
            nodes: Vec::new(),
            incoming: Vec::new(),
            terminal: Vec::new(),
            seeded: Vec::new(),
        };
        for (ordinal, (&value, ty)) in body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .enumerate()
        {
            analysis.ledger.charge(3)?;
            if let Some(row) = pointer_node_v30(
                Definition::FunctionArgument {
                    function: coordinate,
                    argument: u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?,
                },
                value,
                ty,
            ) {
                analysis.ledger.push(&mut graph.nodes, row)?;
            }
        }
        for (block, contents) in body.blocks.iter().enumerate() {
            analysis.ledger.charge(2)?;
            let at = BlockCoordinate {
                function: coordinate,
                block: u32::try_from(block).map_err(|_| ResourceError::Arithmetic)?,
            };
            for (ordinal, parameter) in contents.parameters.iter().enumerate() {
                analysis.ledger.charge(3)?;
                if let Some(row) = pointer_node_v30(
                    Definition::BlockArgument {
                        block: at,
                        argument: u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?,
                    },
                    parameter.id,
                    &parameter.ty,
                ) {
                    analysis.ledger.push(&mut graph.nodes, row)?;
                }
            }
            for (operation, row) in contents.operations.iter().enumerate() {
                analysis.ledger.charge(1)?;
                for (result, value) in row.results.iter().enumerate() {
                    analysis.ledger.charge(3)?;
                    if let Some(row) = pointer_node_v30(
                        Definition::Result {
                            operation: Coordinate {
                                block: at,
                                operation: u32::try_from(operation)
                                    .map_err(|_| ResourceError::Arithmetic)?,
                            },
                            result: u32::try_from(result).map_err(|_| ResourceError::Arithmetic)?,
                        },
                        value.id,
                        &value.ty,
                    ) {
                        analysis.ledger.push(&mut graph.nodes, row)?;
                    }
                }
            }
        }
        analysis
            .ledger
            .sort(&mut graph.nodes, 1, |a, b| a.value.cmp(&b.value))?;
        analysis.ledger.charge(graph.nodes.len())?;
        if graph
            .nodes
            .windows(2)
            .any(|rows| rows[0].value == rows[1].value)
        {
            return Err(ResourceError::Accounting);
        }
        for ordinal in 0..graph.nodes.len() {
            analysis.ledger.charge(6)?;
            let row = graph.nodes[ordinal];
            graph.nodes[ordinal].step = match row.definition {
                Definition::FunctionArgument { .. } => CanonicalSelectedPointerStepV30::Unsupported,
                Definition::Result { operation, result } => {
                    let actual = &body.blocks[operation.block.block as usize].operations
                        [operation.operation as usize];
                    match &actual.kind {
                        OperationKind::GetElementPointer { .. }
                            if result == 0 && actual.results.len() == 1 =>
                        {
                            CanonicalSelectedPointerStepV30::Formation { operation }
                        }
                        OperationKind::Cast {
                            kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                            value,
                            ..
                        } if result == 0 && actual.results.len() == 1 => {
                            let input = graph.find(*value, &mut analysis.ledger)?;
                            let actual_type = analysis.runtime_type(*value)?;
                            match (input, actual_type) {
                                (Some(input), Some(ty))
                                    if checked_pointer_cast_source_v18(actual, ty)
                                        == Some(*value) =>
                                {
                                    CanonicalSelectedPointerStepV30::Cast { input }
                                }
                                _ => CanonicalSelectedPointerStepV30::Unsupported,
                            }
                        }
                        _ => CanonicalSelectedPointerStepV30::Unsupported,
                    }
                }
                Definition::BlockArgument { block, argument } => {
                    let target = &body.blocks[block.block as usize];
                    analysis.ledger.charge(
                        crate::verification_index_v1::verification_ceil_log2_v1(body.blocks.len())
                            .checked_add(3)
                            .ok_or(ResourceError::Arithmetic)?,
                    )?;
                    let first = graph.incoming.len();
                    for &edge in flow
                        .incoming_edges(target.id)
                        .ok_or(ResourceError::Accounting)?
                    {
                        analysis.ledger.charge(
                            crate::verification_index_v1::verification_ceil_log2_v1(
                                body.blocks.len(),
                            )
                            .checked_mul(2)
                            .and_then(|work| work.checked_add(12))
                            .ok_or(ResourceError::Arithmetic)?,
                        )?;
                        let edge_row = flow.edge(edge).ok_or(ResourceError::Accounting)?;
                        let source = flow.edge_source(edge).ok_or(ResourceError::Accounting)?;
                        let source_position = flow
                            .block_position(source)
                            .ok_or(ResourceError::Accounting)?;
                        let arguments = flow.edge_arguments(function, edge);
                        if arguments.len() != target.parameters.len() {
                            return Err(ResourceError::Accounting);
                        }
                        let value = arguments[argument as usize];
                        let input = graph
                            .find(value, &mut analysis.ledger)?
                            .ok_or(ResourceError::Accounting)?;
                        let input_row = graph.nodes[input];
                        if (input_row.scalar, input_row.space, input_row.access)
                            != (row.scalar, row.space, row.access)
                        {
                            return Err(ResourceError::Accounting);
                        }
                        analysis.ledger.push(
                            &mut graph.incoming,
                            CanonicalSelectedPointerIncomingV30 {
                                occurrence: EdgeArgument {
                                    edge: EdgeCoordinate {
                                        source: BlockCoordinate {
                                            function: coordinate,
                                            block: u32::try_from(source_position)
                                                .map_err(|_| ResourceError::Arithmetic)?,
                                        },
                                        successor: u32::try_from(edge_row.ordinal())
                                            .map_err(|_| ResourceError::Arithmetic)?,
                                    },
                                    argument,
                                },
                                source,
                                target: target.id,
                                parameter: ordinal,
                                argument: input,
                                reachable: flow.is_reachable(source),
                            },
                        )?;
                    }
                    CanonicalSelectedPointerStepV30::Parameter {
                        first,
                        count: graph.incoming.len() - first,
                    }
                }
            };
        }
        graph.resolve_terminals(&mut analysis.ledger)?;
        graph.resolve_seeds(&mut analysis.ledger)?;
        Ok(graph)
    }

    pub(super) fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            incoming: Vec::new(),
            terminal: Vec::new(),
            seeded: Vec::new(),
        }
    }

    pub(super) fn retained_bytes(&self) -> std::result::Result<usize, ResourceError> {
        let mut bytes = 0usize;
        for (count, width) in [
            (
                self.nodes.capacity(),
                size_of::<CanonicalSelectedPointerNodeV30>(),
            ),
            (
                self.incoming.capacity(),
                size_of::<CanonicalSelectedPointerIncomingV30>(),
            ),
            (self.terminal.capacity(), size_of::<Option<usize>>()),
            (self.seeded.capacity(), size_of::<bool>()),
        ] {
            bytes = bytes
                .checked_add(count.checked_mul(width).ok_or(ResourceError::Arithmetic)?)
                .ok_or(ResourceError::Arithmetic)?;
        }
        Ok(bytes)
    }

    fn resolve_terminals<M: GuardMeter>(
        &mut self,
        meter: &mut M,
    ) -> std::result::Result<(), ResourceError> {
        let count = self.nodes.len();
        let mut state = Vec::new();
        let mut path = Vec::new();
        meter.reserve(&mut self.terminal, count)?;
        meter.reserve(&mut state, count)?;
        meter.reserve(&mut path, count)?;
        for (ordinal, node) in self.nodes.iter().enumerate() {
            meter.charge(3)?;
            let terminal = match node.step {
                CanonicalSelectedPointerStepV30::Formation { .. }
                | CanonicalSelectedPointerStepV30::Parameter { .. } => Some(ordinal),
                _ => None,
            };
            self.terminal.push(terminal);
            state.push(
                u8::from(!matches!(
                    node.step,
                    CanonicalSelectedPointerStepV30::Cast { .. }
                )) * 2,
            );
        }
        for start in 0..count {
            meter.charge(1)?;
            let mut current = start;
            while state[current] == 0 {
                meter.charge(4)?;
                state[current] = 1;
                path.push(current);
                let CanonicalSelectedPointerStepV30::Cast { input } = self.nodes[current].step
                else {
                    return Err(ResourceError::Accounting);
                };
                current = input;
            }
            let terminal = if state[current] == 2 {
                self.terminal[current]
            } else {
                None
            };
            while let Some(index) = path.pop() {
                meter.charge(2)?;
                self.terminal[index] = terminal;
                state[index] = 2;
            }
        }
        Ok(())
    }

    fn resolve_seeds<M: GuardMeter>(
        &mut self,
        meter: &mut M,
    ) -> std::result::Result<(), ResourceError> {
        let mut reverse = Vec::new();
        let mut queue = Vec::new();
        meter.reserve(&mut self.seeded, self.nodes.len())?;
        meter.reserve(&mut queue, self.nodes.len())?;
        for (node, row) in self.nodes.iter().enumerate() {
            meter.charge(3)?;
            let leaf = matches!(row.step, CanonicalSelectedPointerStepV30::Formation { .. });
            self.seeded.push(leaf);
            if leaf {
                queue.push(node);
            }
            match row.step {
                CanonicalSelectedPointerStepV30::Cast { input } => {
                    meter.push(&mut reverse, (input, node))?;
                }
                CanonicalSelectedPointerStepV30::Parameter { first, count } => {
                    for input in &self.incoming[first..first + count] {
                        meter.charge(1)?;
                        if input.reachable {
                            meter.push(&mut reverse, (input.argument, node))?;
                        }
                    }
                }
                _ => (),
            }
        }
        meter.sort(&mut reverse, 2, |a, b| a.cmp(b))?;
        let mut ranges = Vec::new();
        meter.reserve(&mut ranges, self.nodes.len())?;
        let mut cursor = 0;
        for node in 0..self.nodes.len() {
            meter.charge(1)?;
            let first = cursor;
            while reverse.get(cursor).is_some_and(|row| row.0 == node) {
                meter.charge(1)?;
                cursor += 1;
            }
            ranges.push(first..cursor);
        }
        let mut next = 0;
        while let Some(&node) = queue.get(next) {
            meter.charge(2)?;
            next += 1;
            for &(_, parent) in &reverse[ranges[node].clone()] {
                meter.charge(2)?;
                if !self.seeded[parent] {
                    self.seeded[parent] = true;
                    queue.push(parent);
                }
            }
        }
        Ok(())
    }
}

fn pointer_graph_headers_v30() -> std::result::Result<usize, ResourceError> {
    type Build<'a> = (
        &'a Function,
        FunctionCoordinate,
        &'a IndexedControlFlow,
        PointerGraphV30,
        CanonicalSelectedPointerNodeV30,
        CanonicalSelectedPointerIncomingV30,
        Option<CanonicalSelectedPointerNodeV30>,
        Option<&'a Type>,
        Option<ValueId>,
        Option<usize>,
        Definition,
        EdgeArgument,
        Coordinate,
        Vec<u8>,
        Vec<usize>,
        Vec<(usize, usize)>,
        Vec<std::ops::Range<usize>>,
        [usize; 16],
    );
    size_of::<Build<'_>>()
        .checked_add(2 * size_of::<std::result::Result<PointerGraphV30, ResourceError>>())
        .and_then(|n| n.checked_add(4 * size_of::<std::result::Result<(), ResourceError>>()))
        .ok_or(ResourceError::Arithmetic)
}

#[cfg(test)]
#[path = "canonical_selected_slice_graph_v30_tests.rs"]
mod tests;
