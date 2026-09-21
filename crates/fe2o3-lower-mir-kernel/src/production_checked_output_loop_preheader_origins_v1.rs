use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeCoordinateV1 as Edge,
};
use std::ops::Range;

/// Synthetic CFG origin in the immediate genuine promoted input. This is not
/// a Rust block/span or a historical numbered block-transition certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionLoopPreheaderOriginV1 {
    input_header: Block,
    output_preheader: Block,
    incoming: Range<usize>,
    parameters: Range<usize>,
}
impl ProductionLoopPreheaderOriginV1 {
    /// Header coordinate in the immediate promoted input, not a Rust block/span.
    pub const fn input_header(&self) -> Block {
        self.input_header
    }
    /// Newly appended empty forwarding block in the actual final graph.
    pub const fn output_preheader(&self) -> Block {
        self.output_preheader
    }
    /// Range in the owning continuation's complete flat incoming-edge roster.
    pub fn incoming(&self) -> Range<usize> {
        self.incoming.clone()
    }
    /// Range in the owning continuation's ordered forwarding-parameter roster.
    pub fn parameters(&self) -> Range<usize> {
        self.parameters.clone()
    }
}
/// One exact redirected occurrence. Selector/case position and ordered edge
/// arguments remain on the actual immutable graphs authenticated by the pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLoopPreheaderIncomingOriginV1 {
    input: Edge,
    output: Edge,
}
impl ProductionLoopPreheaderIncomingOriginV1 {
    /// Original external-header edge occurrence, retaining its successor ordinal.
    pub const fn input(&self) -> Edge {
        self.input
    }
    /// Same occurrence redirected to the final synthetic preheader by the pair.
    pub const fn output(&self) -> Edge {
        self.output
    }
}
/// Positional neutral forwarding, not a new Rust local or a type-equality guess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionLoopPreheaderParameterOriginV1 {
    input_header_parameter: Definition,
    output_preheader_parameter: Definition,
}
impl ProductionLoopPreheaderParameterOriginV1 {
    /// Positional definition in the immediate promoted input header.
    pub const fn input_header_parameter(&self) -> Definition {
        self.input_header_parameter
    }
    /// Matching positional definition in the final synthetic forwarding block.
    pub const fn output_preheader_parameter(&self) -> Definition {
        self.output_preheader_parameter
    }
}
type BlockOrigin = ProductionLoopPreheaderOriginV1;
type Incoming = ProductionLoopPreheaderIncomingOriginV1;
type Parameter = ProductionLoopPreheaderParameterOriginV1;

pub(super) struct Origins {
    blocks: Vec<BlockOrigin>,
    incoming: Vec<Incoming>,
    parameters: Vec<Parameter>,
}
impl Origins {
    pub(super) fn blocks(&self) -> &[BlockOrigin] {
        &self.blocks
    }
    pub(super) fn incoming(&self) -> &[Incoming] {
        &self.incoming
    }
    pub(super) fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }
    pub(super) fn backing(&self) -> HResult<usize> {
        self.blocks
            .capacity()
            .checked_mul(size_of::<BlockOrigin>())
            .and_then(|n| {
                n.checked_add(
                    self.incoming
                        .capacity()
                        .checked_mul(size_of::<Incoming>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    self.parameters
                        .capacity()
                        .checked_mul(size_of::<Parameter>())?,
                )
            })
            .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
    }
    pub(super) fn check_equal(
        &self,
        other: &Self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> HResult<()> {
        equal(&self.blocks, &other.blocks, budget)?;
        equal(&self.incoming, &other.incoming, budget)?;
        equal(&self.parameters, &other.parameters, budget)
    }
}
fn equal<T: PartialEq>(a: &[T], b: &[T], budget: &mut AssertOriginBudgetV1<'_>) -> HResult<()> {
    budget.charge_work(2)?;
    if a.len() != b.len() {
        return Err(HError::Origins("complete origin cardinality"));
    }
    for (a, b) in a.iter().zip(b) {
        budget.charge_work(
            size_of::<T>()
                .checked_add(1)
                .ok_or(AssertOriginResourceV1::Arithmetic)?,
        )?;
        if a != b {
            return Err(HError::Origins("exact ordered synthetic origins"));
        }
    }
    Ok(())
}
fn block_ordinal(
    inventory: &Inventory<'_>,
    block: Block,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> HResult<usize> {
    budget.charge_work(5)?;
    let function = inventory
        .functions()
        .get(block.function.0 as usize)
        .ok_or(HError::Origins("function ordinal"))?;
    let ordinal = function
        .blocks
        .start
        .checked_add(block.block as usize)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    if ordinal >= function.blocks.end
        || inventory
            .blocks()
            .get(ordinal)
            .is_none_or(|row| row.coordinate != block)
    {
        return Err(HError::Origins("block ordinal"));
    }
    Ok(ordinal)
}
fn original_edge(
    input: &Inventory<'_>,
    coordinate: Edge,
    header: Block,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> HResult<()> {
    let block = &input.blocks()[block_ordinal(input, coordinate.source, budget)?];
    budget.charge_work(4)?;
    let ordinal = block
        .edges
        .start
        .checked_add(coordinate.successor as usize)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    if ordinal >= block.edges.end
        || input
            .edges()
            .get(ordinal)
            .is_none_or(|row| row.coordinate != coordinate || row.target != header)
    {
        return Err(HError::Origins("exact original external edge occurrence"));
    }
    Ok(())
}

/// Pair replay is independent and complete before this call. Build the small
/// synthetic origin roster with dense indices and two whole edge sweeps, never
/// another loop solver or a per-header graph rescan. All new backing is paid.
pub(super) fn derive(
    pair: &PreheaderPair<'_>,
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> HResult<Origins> {
    budget.charge_work(4)?;
    if !std::ptr::eq(pair.input(), input.owner()) || !std::ptr::eq(pair.output(), output.owner()) {
        return Err(HError::Origins("actual pair inventory custody"));
    }
    let count = pair.preheaders().len();
    if count == 0 {
        return Ok(Origins {
            blocks: Vec::new(),
            incoming: Vec::new(),
            parameters: Vec::new(),
        });
    }
    let mut blocks = scratch::<BlockOrigin>(count, budget)?;
    let mut selected = scratch::<Option<usize>>(output.blocks().len(), budget)?;
    for _ in output.blocks() {
        budget.charge_work(1)?;
        selected.push(None);
    }
    let mut cursors = scratch::<usize>(count, budget)?;
    let mut parameter_count = 0usize;
    for (index, row) in pair.preheaders().iter().enumerate() {
        budget.charge_work(9)?;
        let header = &input.blocks()[block_ordinal(input, row.header, budget)?];
        let ordinal = block_ordinal(output, row.preheader, budget)?;
        let appended = &output.blocks()[ordinal];
        if selected[ordinal].is_some() || header.parameters.len() != appended.parameters.len() {
            return Err(HError::Origins(
                "unique preheader and ordered parameter cardinality",
            ));
        }
        selected[ordinal] = Some(index);
        let end = parameter_count
            .checked_add(header.parameters.len())
            .ok_or(AssertOriginResourceV1::Arithmetic)?;
        blocks.push(BlockOrigin {
            input_header: row.header,
            output_preheader: row.preheader,
            incoming: 0..0,
            parameters: parameter_count..end,
        });
        parameter_count = end;
        cursors.push(0);
    }
    for edge in output.edges() {
        budget.charge_work(2)?;
        if let Some(index) = selected[block_ordinal(output, edge.target, budget)?] {
            original_edge(input, edge.coordinate, blocks[index].input_header, budget)?;
            cursors[index] = cursors[index]
                .checked_add(1)
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
        }
    }
    let mut incoming_count = 0usize;
    for (block, count) in blocks.iter_mut().zip(&mut cursors) {
        budget.charge_work(4)?;
        if *count == 0 {
            return Err(HError::Origins("nonempty external edge roster"));
        }
        let end = incoming_count
            .checked_add(*count)
            .ok_or(AssertOriginResourceV1::Arithmetic)?;
        block.incoming = incoming_count..end;
        *count = incoming_count;
        incoming_count = end;
    }
    let mut incoming = scratch::<Incoming>(incoming_count, budget)?;
    // Initialized inert scratch only; every slot is overwritten exactly once
    // before Origins is returned, with complete per-row cursor closure below.
    for block in &blocks {
        for _ in block.incoming.clone() {
            budget.charge_work(1)?;
            let placeholder = Edge {
                source: block.input_header,
                successor: 0,
            };
            incoming.push(Incoming {
                input: placeholder,
                output: placeholder,
            });
        }
    }
    for edge in output.edges() {
        budget.charge_work(3)?;
        if let Some(index) = selected[block_ordinal(output, edge.target, budget)?] {
            let cursor = cursors[index];
            if cursor >= blocks[index].incoming.end {
                return Err(HError::Origins("incoming fill bounds"));
            }
            incoming[cursor] = Incoming {
                input: edge.coordinate,
                output: edge.coordinate,
            };
            cursors[index] = cursor
                .checked_add(1)
                .ok_or(AssertOriginResourceV1::Arithmetic)?;
        }
    }
    let mut parameters = scratch::<Parameter>(parameter_count, budget)?;
    for (block, cursor) in blocks.iter().zip(cursors) {
        budget.charge_work(3)?;
        if cursor != block.incoming.end {
            return Err(HError::Origins("complete incoming fill"));
        }
        for ordinal in 0..block.parameters.len() {
            budget.charge_work(3)?;
            let argument =
                u32::try_from(ordinal).map_err(|_| AssertOriginResourceV1::Arithmetic)?;
            parameters.push(Parameter {
                input_header_parameter: Definition::BlockArgument {
                    block: block.input_header,
                    argument,
                },
                output_preheader_parameter: Definition::BlockArgument {
                    block: block.output_preheader,
                    argument,
                },
            });
        }
    }
    if incoming.len() != incoming_count || parameters.len() != parameter_count {
        return Err(HError::Origins("complete backing cardinality"));
    }
    Ok(Origins {
        blocks,
        incoming,
        parameters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    impl ProductionOwnedLoopPreheadersContinuationV1 {
        pub(crate) fn exercise_source_preheader_origin_refusals_v1(
            &mut self,
            budget: &mut AssertOriginBudgetV1<'_>,
        ) {
            assert!(!self.data.origins.blocks.is_empty());
            assert!(self.data.origins.incoming.len() >= 2);
            assert!(!self.data.origins.parameters.is_empty());
            let floor = budget.storage();
            let last = self.data.origins.blocks.pop().unwrap();
            assert!(matches!(
                self.verify_equivalence(budget),
                Err(HError::Origins("complete origin cardinality"))
            ));
            assert_eq!(budget.storage(), floor);
            self.data.origins.blocks.push(last);
            self.data.origins.incoming.swap(0, 1);
            assert!(matches!(
                self.verify_equivalence(budget),
                Err(HError::Origins("exact ordered synthetic origins"))
            ));
            assert_eq!(budget.storage(), floor);
            self.data.origins.incoming.swap(0, 1);
            let old = self.data.origins.incoming[0].input;
            self.data.origins.incoming[0].input.successor = old.successor.checked_add(1).unwrap();
            assert!(matches!(
                self.verify_equivalence(budget),
                Err(HError::Origins("exact ordered synthetic origins"))
            ));
            assert_eq!(budget.storage(), floor);
            self.data.origins.incoming[0].input = old;
            let old = self.data.origins.parameters[0].input_header_parameter;
            let Definition::BlockArgument { block, argument } = old else {
                panic!("header parameter");
            };
            self.data.origins.parameters[0].input_header_parameter = Definition::BlockArgument {
                block,
                argument: argument.checked_add(1).unwrap(),
            };
            assert!(matches!(
                self.verify_equivalence(budget),
                Err(HError::Origins("exact ordered synthetic origins"))
            ));
            assert_eq!(budget.storage(), floor);
            self.data.origins.parameters[0].input_header_parameter = old;
            self.verify_equivalence(budget).unwrap();
        }
    }
}
