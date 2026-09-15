//! Fixed-width row grammar. Bounds and logical work are precharged by the owner.
use super::*;

fn copy_slice<T: Copy>(source: &[T]) -> Result<Vec<T>> {
    let mut values = allocate(source.len())?;
    values.extend_from_slice(source);
    Ok(values)
}
pub(super) fn copy(candidate: Candidate<'_>) -> Result<Rows> {
    Ok(Rows {
        functions: copy_slice(candidate.functions)?,
        blocks: copy_slice(candidate.blocks)?,
        segments: copy_slice(candidate.segments)?,
        operations: copy_slice(candidate.operations)?,
        definitions: copy_slice(candidate.definitions)?,
        definition_outputs: copy_slice(candidate.definition_outputs)?,
        uses: copy_slice(candidate.uses)?,
        edges: copy_slice(candidate.edges)?,
        edge_arguments: copy_slice(candidate.edge_arguments)?,
    })
}
pub(super) fn encode(writer: &mut Writer, rows: Candidate<'_>) -> Result<()> {
    for row in rows.functions {
        writer.u32(row.input.0)?;
        writer.u32(row.output.0)?;
    }
    for row in rows.blocks {
        writer.block(row.output)?;
        writer.range(row.segments)?;
    }
    for row in rows.segments {
        writer.block(row.input)?;
        match row.connector {
            None => {
                writer.u32(0)?;
                writer.words([0; 3])?;
            }
            Some(edge) => {
                writer.u32(1)?;
                writer.edge(edge)?;
            }
        }
    }
    for row in rows.operations {
        writer.operation(row.output)?;
        match row.origin {
            Origin::Retained(operation) => {
                writer.u32(0)?;
                writer.operation(operation)?;
                writer.words([0; 2])?;
            }
            Origin::ConstantFrom(definition) => {
                writer.u32(1)?;
                writer.definition(definition)?;
            }
        }
    }
    for row in rows.definitions {
        writer.definition(row.input)?;
        writer.range(row.outputs)?;
    }
    for row in rows.definition_outputs {
        writer.definition(row.output)?;
        writer.u32(match row.kind {
            DescendantKind::Retained => 0,
            DescendantKind::Substituted => 1,
        })?;
    }
    for row in rows.uses {
        writer.use_coordinate(row.output)?;
        writer.use_coordinate(row.input)?;
    }
    for row in rows.edges {
        writer.edge(row.output)?;
        writer.edge(row.input)?;
    }
    for row in rows.edge_arguments {
        writer.edge_argument(row.output)?;
        writer.edge_argument(row.input)?;
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>, counts: [usize; 9]) -> Result<Rows> {
    macro_rules! read_rows {
        ($count:expr, $method:ident) => {{
            let mut values = allocate($count)?;
            for _ in 0..$count {
                values.push(reader.$method()?);
            }
            values
        }};
    }
    Ok(Rows {
        functions: read_rows!(counts[0], function_row),
        blocks: read_rows!(counts[1], block_row),
        segments: read_rows!(counts[2], segment),
        operations: read_rows!(counts[3], operation_row),
        definitions: read_rows!(counts[4], definition_row),
        definition_outputs: read_rows!(counts[5], descendant),
        uses: read_rows!(counts[6], use_row),
        edges: read_rows!(counts[7], edge_row),
        edge_arguments: read_rows!(counts[8], edge_argument_row),
    })
}

impl Writer {
    fn words<const N: usize>(&mut self, words: [u32; N]) -> Result<()> {
        for word in words {
            self.u32(word)?;
        }
        Ok(())
    }
    fn block(&mut self, block: Block) -> Result<()> {
        self.words([block.function.0, block.block])
    }
    fn operation(&mut self, operation: Operation) -> Result<()> {
        self.block(operation.block)?;
        self.u32(operation.operation)
    }
    fn range(&mut self, range: Range) -> Result<()> {
        self.words([range.start, range.len])
    }
    fn edge(&mut self, edge: Edge) -> Result<()> {
        self.block(edge.source)?;
        self.u32(edge.successor)
    }
    fn edge_argument(&mut self, argument: EdgeArgument) -> Result<()> {
        self.edge(argument.edge)?;
        self.u32(argument.argument)
    }
    fn definition(&mut self, definition: Definition) -> Result<()> {
        self.words(match definition {
            Definition::FunctionArgument { function, argument } => [0, function.0, 0, 0, argument],
            Definition::BlockArgument { block, argument } => {
                [1, block.function.0, block.block, 0, argument]
            }
            Definition::Result { operation, result } => [
                2,
                operation.block.function.0,
                operation.block.block,
                operation.operation,
                result,
            ],
        })
    }
    fn use_coordinate(&mut self, usage: Use) -> Result<()> {
        self.words(match usage {
            Use::OperationOperand { operation, operand } => [
                0,
                operation.block.function.0,
                operation.block.block,
                operation.operation,
                operand,
            ],
            Use::TerminatorOperand { block, operand } => {
                [1, block.function.0, block.block, 0, operand]
            }
        })
    }
}
impl Reader<'_> {
    fn words<const N: usize>(&mut self) -> Result<[u32; N]> {
        let mut words = [0; N];
        for word in &mut words {
            *word = self.u32()?;
        }
        Ok(words)
    }
    fn block(&mut self) -> Result<Block> {
        Ok(Block {
            function: Function(self.u32()?),
            block: self.u32()?,
        })
    }
    fn operation(&mut self) -> Result<Operation> {
        Ok(Operation {
            block: self.block()?,
            operation: self.u32()?,
        })
    }
    fn range(&mut self) -> Result<Range> {
        Ok(Range {
            start: self.u32()?,
            len: self.u32()?,
        })
    }
    fn edge(&mut self) -> Result<Edge> {
        Ok(Edge {
            source: self.block()?,
            successor: self.u32()?,
        })
    }
    fn edge_argument(&mut self) -> Result<EdgeArgument> {
        Ok(EdgeArgument {
            edge: self.edge()?,
            argument: self.u32()?,
        })
    }
    fn definition(&mut self) -> Result<Definition> {
        let [tag, function, block, operation, slot] = self.words()?;
        let function = Function(function);
        match tag {
            0 if block == 0 && operation == 0 => Ok(Definition::FunctionArgument {
                function,
                argument: slot,
            }),
            1 if operation == 0 => Ok(Definition::BlockArgument {
                block: Block { function, block },
                argument: slot,
            }),
            2 => Ok(Definition::Result {
                operation: Operation {
                    block: Block { function, block },
                    operation,
                },
                result: slot,
            }),
            _ => Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "definition tag or padding",
            )),
        }
    }
    fn use_coordinate(&mut self) -> Result<Use> {
        let [tag, function, block, operation, operand] = self.words()?;
        let block = Block {
            function: Function(function),
            block,
        };
        match tag {
            0 => Ok(Use::OperationOperand {
                operation: Operation { block, operation },
                operand,
            }),
            1 if operation == 0 => Ok(Use::TerminatorOperand { block, operand }),
            _ => Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                "use tag or padding",
            )),
        }
    }
    fn function_row(&mut self) -> Result<FunctionRow> {
        Ok(FunctionRow {
            input: Function(self.u32()?),
            output: Function(self.u32()?),
        })
    }
    fn block_row(&mut self) -> Result<BlockRow> {
        Ok(BlockRow {
            output: self.block()?,
            segments: self.range()?,
        })
    }
    fn segment(&mut self) -> Result<Segment> {
        let input = self.block()?;
        let tag = self.u32()?;
        let edge = self.edge()?;
        let connector = match tag {
            0 if edge
                == (Edge {
                    source: Block {
                        function: Function(0),
                        block: 0,
                    },
                    successor: 0,
                }) =>
            {
                None
            }
            1 => Some(edge),
            _ => {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "connector tag or padding",
                ));
            }
        };
        Ok(Segment { input, connector })
    }
    fn operation_row(&mut self) -> Result<OperationRow> {
        let output = self.operation()?;
        let origin = match self.u32()? {
            0 => {
                let operation = self.operation()?;
                if self.words::<2>()? != [0; 2] {
                    return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                        "retained origin padding",
                    ));
                }
                Origin::Retained(operation)
            }
            1 => Origin::ConstantFrom(self.definition()?),
            _ => {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "origin tag",
                ));
            }
        };
        Ok(OperationRow { output, origin })
    }
    fn definition_row(&mut self) -> Result<DefinitionRow> {
        Ok(DefinitionRow {
            input: self.definition()?,
            outputs: self.range()?,
        })
    }
    fn descendant(&mut self) -> Result<Descendant> {
        let output = self.definition()?;
        let kind = match self.u32()? {
            0 => DescendantKind::Retained,
            1 => DescendantKind::Substituted,
            _ => {
                return Err(CanonicalKirTransitionReceiptErrorV1::Malformed(
                    "descendant kind",
                ));
            }
        };
        Ok(Descendant { output, kind })
    }
    fn use_row(&mut self) -> Result<UseRow> {
        Ok(UseRow {
            output: self.use_coordinate()?,
            input: self.use_coordinate()?,
        })
    }
    fn edge_row(&mut self) -> Result<EdgeRow> {
        Ok(EdgeRow {
            output: self.edge()?,
            input: self.edge()?,
        })
    }
    fn edge_argument_row(&mut self) -> Result<EdgeArgumentRow> {
        Ok(EdgeArgumentRow {
            output: self.edge_argument()?,
            input: self.edge_argument()?,
        })
    }
}
