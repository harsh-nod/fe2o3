fn source_coordinate(source: TileScalarSourceV29) -> R<ProductionTileSourceCoordinateV29> {
    use ProductionTileSourceCoordinateV29 as S;
    Ok(match source {
        TileScalarSourceV29::FunctionParameter {
            function,
            parameter,
        } => S::FunctionParameter {
            function,
            parameter,
        },
        TileScalarSourceV29::Block { function, block } => S::Block { function, block },
        TileScalarSourceV29::BlockParameter {
            function,
            block,
            parameter,
        } => S::BlockParameter {
            function,
            block,
            parameter,
        },
        TileScalarSourceV29::Operation(point) => S::Operation(operation_coordinate(point)?),
        TileScalarSourceV29::Result { operation, result } => S::Result {
            operation: operation_coordinate(operation)?,
            result,
        },
        TileScalarSourceV29::Terminator { function, block } => {
            S::Terminator(block_coordinate(function, block)?)
        }
        TileScalarSourceV29::Edge {
            function,
            block,
            edge,
        } => S::Edge(EdgeCoordinate {
            source: block_coordinate(function, block)?,
            successor: u32_index(edge)?,
        }),
    })
}
fn target_coordinate(piece: TileScalarPieceV29) -> R<ProductionTileTargetCoordinateV29> {
    use ProductionTileTargetCoordinateV29 as T;
    Ok(match piece {
        TileScalarPieceV29::FunctionParameter {
            function,
            parameter,
        } => T::FunctionParameter {
            function,
            parameter,
        },
        TileScalarPieceV29::Block { function, block } => {
            T::Block(block_coordinate(function, block)?)
        }
        TileScalarPieceV29::BlockParameter {
            function,
            block,
            parameter,
        } => T::BlockParameter {
            block: block_coordinate(function, block)?,
            parameter,
        },
        TileScalarPieceV29::Operation(point) => T::Operation(operation_coordinate(point)?),
        TileScalarPieceV29::Result { operation, result } => T::Result {
            operation: operation_coordinate(operation)?,
            result,
        },
        TileScalarPieceV29::Terminator { function, block } => {
            T::Terminator(block_coordinate(function, block)?)
        }
        TileScalarPieceV29::Edge {
            function,
            block,
            edge,
        } => T::Edge(EdgeCoordinate {
            source: block_coordinate(function, block)?,
            successor: u32_index(edge)?,
        }),
        TileScalarPieceV29::Anchor(point) => T::Anchor {
            block: block_coordinate(point.function, point.block)?,
            operation_gap: point.operation,
        },
        TileScalarPieceV29::ErasedValue(value) => T::ErasedValue(value),
    })
}
fn expansion_stage(stage: TileScalarStageV29) -> ProductionTileExpansionStageV29 {
    use ProductionTileExpansionStageV29 as S;
    match stage {
        TileScalarStageV29::Preserved => S::Preserved,
        TileScalarStageV29::Erased => S::Erased,
        TileScalarStageV29::Prelude => S::Prelude,
        TileScalarStageV29::Predicate => S::Predicate,
        TileScalarStageV29::Read => S::Read,
        TileScalarStageV29::ReadBlock => S::ReadBlock,
        TileScalarStageV29::Join => S::Join,
        TileScalarStageV29::Conditional => S::Conditional,
        TileScalarStageV29::ReadBranch => S::ReadBranch,
        TileScalarStageV29::Parts => S::Parts,
    }
}
fn member(range: &Range<usize>, ordinal: usize) -> R<usize> {
    let index = sum(&[range.start, ordinal])?;
    if index >= range.end {
        return Err(invalid("coordinate range"));
    }
    Ok(index)
}
fn function<'a, 'g>(
    inventory: &'a Inventory<'g>,
    coordinate: FunctionCoordinate,
) -> R<&'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'g>> {
    inventory
        .functions()
        .get(coordinate.0 as usize)
        .filter(|row| row.coordinate == coordinate)
        .ok_or_else(|| invalid("function coordinate"))
}
fn block_index(inventory: &Inventory<'_>, coordinate: BlockCoordinate) -> R<usize> {
    let index = member(
        &function(inventory, coordinate.function)?.blocks,
        coordinate.block as usize,
    )?;
    if inventory.blocks()[index].coordinate != coordinate {
        return Err(invalid("block coordinate"));
    }
    Ok(index)
}
fn operation_index(inventory: &Inventory<'_>, coordinate: OperationCoordinate) -> R<usize> {
    let block = block_index(inventory, coordinate.block)?;
    let index = member(
        &inventory.blocks()[block].operations,
        coordinate.operation as usize,
    )?;
    if inventory.operations()[index].coordinate != coordinate {
        return Err(invalid("operation coordinate"));
    }
    Ok(index)
}
fn edge_index(inventory: &Inventory<'_>, coordinate: EdgeCoordinate) -> R<usize> {
    let block = block_index(inventory, coordinate.source)?;
    let index = member(
        &inventory.blocks()[block].edges,
        coordinate.successor as usize,
    )?;
    if inventory.edges()[index].coordinate != coordinate {
        return Err(invalid("edge coordinate"));
    }
    Ok(index)
}
fn use_index(inventory: &Inventory<'_>, coordinate: UseCoordinate) -> R<usize> {
    let index = match coordinate {
        UseCoordinate::OperationOperand { operation, operand } => member(
            &inventory.operations()[operation_index(inventory, operation)?].operands,
            operand as usize,
        )?,
        UseCoordinate::TerminatorOperand { block, operand } => member(
            &inventory.blocks()[block_index(inventory, block)?].terminator_uses,
            operand as usize,
        )?,
    };
    if inventory.uses()[index].coordinate != coordinate {
        return Err(invalid("use coordinate"));
    }
    Ok(index)
}
fn edge_argument_index(inventory: &Inventory<'_>, coordinate: EdgeArgumentCoordinate) -> R<usize> {
    let index = member(
        &inventory.edges()[edge_index(inventory, coordinate.edge)?].bindings,
        coordinate.argument as usize,
    )?;
    if inventory.edge_arguments()[index].coordinate != coordinate {
        return Err(invalid("edge payload coordinate"));
    }
    Ok(index)
}
fn definition_index(inventory: &Inventory<'_>, coordinate: DefinitionCoordinate) -> R<usize> {
    let index = match coordinate {
        DefinitionCoordinate::FunctionArgument {
            function: f,
            argument,
        } => {
            let row = function(inventory, f)?;
            if argument as usize >= row.function.signature.parameters.len() {
                return Err(invalid("signature argument"));
            }
            member(&row.definitions, argument as usize)?
        }
        DefinitionCoordinate::BlockArgument { block, argument } => member(
            &inventory.blocks()[block_index(inventory, block)?].parameters,
            argument as usize,
        )?,
        DefinitionCoordinate::Result { operation, result } => member(
            &inventory.operations()[operation_index(inventory, operation)?].results,
            result as usize,
        )?,
    };
    if inventory.definitions()[index].coordinate != coordinate {
        return Err(invalid("definition coordinate"));
    }
    Ok(index)
}
struct Inverse {
    operations: Vec<Option<usize>>,
    definitions: Vec<Option<usize>>,
    blocks: Vec<Option<usize>>,
    terminators: Vec<Option<usize>>,
    edges: Vec<Option<usize>>,
}
impl Inverse {
    fn new(inventory: &Inventory<'_>, budget: &mut ArgumentBudgetV1<'_>) -> R<Self> {
        budget.reserve_storage(size_of::<Self>())?;
        Ok(Self {
            operations: optional_slots(inventory.operations().len(), budget)?,
            definitions: optional_slots(inventory.definitions().len(), budget)?,
            blocks: optional_slots(inventory.blocks().len(), budget)?,
            terminators: optional_slots(inventory.blocks().len(), budget)?,
            edges: optional_slots(inventory.edges().len(), budget)?,
        })
    }
    fn storage(&self) -> R<usize> {
        sum(&[
            size_of::<Self>(),
            bytes::<Option<usize>>(sum(&[
                self.operations.capacity(),
                self.definitions.capacity(),
                self.blocks.capacity(),
                self.terminators.capacity(),
                self.edges.capacity(),
            ])?)?,
        ])
    }
}
fn bind(rows: &mut [Option<usize>], index: usize, piece: usize) -> R<()> {
    let row = rows
        .get_mut(index)
        .ok_or_else(|| invalid("inverse index"))?;
    if row.replace(piece).is_some() {
        return Err(invalid("duplicate structural owner"));
    }
    Ok(())
}
fn parent(rows: &[Option<usize>], index: usize) -> R<ProductionTileProvenanceV29> {
    rows.get(index)
        .copied()
        .flatten()
        .map(ProductionTileProvenanceV29::Piece)
        .ok_or_else(|| invalid("omitted structural owner"))
}
fn validate_target(target: ProductionTileTargetCoordinateV29, inventory: &Inventory<'_>) -> R<()> {
    use ProductionTileTargetCoordinateV29 as T;
    match target {
        T::FunctionParameter {
            function,
            parameter,
        } => {
            definition_index(
                inventory,
                DefinitionCoordinate::FunctionArgument {
                    function: FunctionCoordinate(u32_index(function)?),
                    argument: u32_index(parameter)?,
                },
            )?;
        }
        T::Block(block) | T::Terminator(block) => {
            block_index(inventory, block)?;
        }
        T::BlockParameter { block, parameter } => {
            definition_index(
                inventory,
                DefinitionCoordinate::BlockArgument {
                    block,
                    argument: u32_index(parameter)?,
                },
            )?;
        }
        T::Operation(operation) => {
            operation_index(inventory, operation)?;
        }
        T::Result { operation, result } => {
            definition_index(
                inventory,
                DefinitionCoordinate::Result {
                    operation,
                    result: u32_index(result)?,
                },
            )?;
        }
        T::Edge(edge) => {
            edge_index(inventory, edge)?;
        }
        T::Anchor {
            block,
            operation_gap,
        } => {
            if operation_gap
                > inventory.blocks()[block_index(inventory, block)?]
                    .operations
                    .len()
            {
                return Err(invalid("output gap"));
            }
        }
        T::ErasedValue(_) => {}
    }
    Ok(())
}
fn index_graph(
    candidate: &ScopedTileScalarCandidateV29,
    inventory: &Inventory<'_>,
    tables: &mut Tables,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    if !inventory.belongs_to(&candidate.output) {
        return Err(invalid("foreign current owner"));
    }
    let mut inverse = Inverse::new(inventory, budget)?;
    tables.origins = vector(candidate.relations.origins.len(), budget)?;
    tables.pieces = vector(candidate.relations.pieces.len(), budget)?;
    for (ordinal, origin) in candidate.relations.origins.iter().enumerate() {
        budget.charge_work(1)?;
        let end = sum(&[origin.first, origin.count])?;
        if origin.first != tables.pieces.len() {
            return Err(invalid("origin coverage"));
        }
        let rows = candidate
            .relations
            .pieces
            .get(origin.first..end)
            .ok_or_else(|| invalid("origin pieces"))?;
        let source = source_coordinate(origin.source)?;
        for row in rows {
            budget.charge_work(1)?;
            let piece = tables.pieces.len();
            let target = target_coordinate(row.piece)?;
            validate_target(target, inventory)?;
            // Source result aliases may coincide with the operation's structural pieces.
            if !matches!(source, ProductionTileSourceCoordinateV29::Result { .. }) {
                use ProductionTileTargetCoordinateV29 as T;
                match target {
                    T::FunctionParameter {
                        function,
                        parameter,
                    } => bind(
                        &mut inverse.definitions,
                        definition_index(
                            inventory,
                            DefinitionCoordinate::FunctionArgument {
                                function: FunctionCoordinate(u32_index(function)?),
                                argument: u32_index(parameter)?,
                            },
                        )?,
                        piece,
                    )?,
                    T::Block(block) => {
                        bind(&mut inverse.blocks, block_index(inventory, block)?, piece)?
                    }
                    T::BlockParameter { block, parameter } => bind(
                        &mut inverse.definitions,
                        definition_index(
                            inventory,
                            DefinitionCoordinate::BlockArgument {
                                block,
                                argument: u32_index(parameter)?,
                            },
                        )?,
                        piece,
                    )?,
                    T::Operation(operation) => bind(
                        &mut inverse.operations,
                        operation_index(inventory, operation)?,
                        piece,
                    )?,
                    T::Terminator(block) => bind(
                        &mut inverse.terminators,
                        block_index(inventory, block)?,
                        piece,
                    )?,
                    T::Edge(edge) => bind(&mut inverse.edges, edge_index(inventory, edge)?, piece)?,
                    T::Result { .. } | T::Anchor { .. } | T::ErasedValue(_) => {}
                }
            }
            push(
                &mut tables.pieces,
                ProductionTilePieceV29 {
                    target,
                    origin: ordinal,
                    stage: expansion_stage(row.stage),
                    component: row.component,
                },
                budget,
            )?;
        }
        push(
            &mut tables.origins,
            ProductionTileOriginV29 {
                source,
                pieces: origin.first..end,
            },
            budget,
        )?;
    }
    if tables.pieces.len() != candidate.relations.pieces.len() {
        return Err(invalid("unowned pieces"));
    }
    tables.operations = vector(inventory.operations().len(), budget)?;
    for (index, row) in inventory.operations().iter().enumerate() {
        budget.charge_work(1)?;
        push(
            &mut tables.operations,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::Operation(row.coordinate),
                parent(&inverse.operations, index)?,
                None,
                None,
                None,
            ),
            budget,
        )?;
    }
    tables.definitions = vector(inventory.definitions().len(), budget)?;
    for (index, row) in inventory.definitions().iter().enumerate() {
        budget.charge_work(1)?;
        let provenance = match row.coordinate {
            DefinitionCoordinate::Result { operation, .. } => {
                parent(&inverse.operations, operation_index(inventory, operation)?)?
            }
            DefinitionCoordinate::FunctionArgument {
                function: f,
                argument,
            } if function(inventory, f)?.function.body.is_none() => {
                if inverse.definitions[index].is_some() || row.value.is_some() {
                    return Err(invalid("declaration provenance"));
                }
                ProductionTileProvenanceV29::DeclarationArgument {
                    function: f,
                    argument,
                }
            }
            _ => parent(&inverse.definitions, index)?,
        };
        push(
            &mut tables.definitions,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::Definition(row.coordinate),
                provenance,
                Some(index),
                None,
                row.value,
            ),
            budget,
        )?;
    }
    tables.uses = vector(inventory.uses().len(), budget)?;
    for row in inventory.uses() {
        budget.charge_work(1)?;
        let provenance = match row.coordinate {
            UseCoordinate::OperationOperand { operation, .. } => {
                parent(&inverse.operations, operation_index(inventory, operation)?)?
            }
            UseCoordinate::TerminatorOperand { block, .. } => {
                parent(&inverse.terminators, block_index(inventory, block)?)?
            }
        };
        if row.definition >= tables.definitions.len() {
            return Err(invalid("use definition"));
        }
        push(
            &mut tables.uses,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::Use(row.coordinate),
                provenance,
                Some(row.definition),
                None,
                Some(row.value),
            ),
            budget,
        )?;
    }
    tables.blocks = vector(inventory.blocks().len(), budget)?;
    tables.terminators = vector(inventory.blocks().len(), budget)?;
    for (index, row) in inventory.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        push(
            &mut tables.blocks,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::Block(row.coordinate),
                parent(&inverse.blocks, index)?,
                None,
                None,
                None,
            ),
            budget,
        )?;
        push(
            &mut tables.terminators,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::Terminator(row.coordinate),
                parent(&inverse.terminators, index)?,
                None,
                None,
                None,
            ),
            budget,
        )?;
    }
    tables.edges = vector(inventory.edges().len(), budget)?;
    for (index, row) in inventory.edges().iter().enumerate() {
        budget.charge_work(1)?;
        block_index(inventory, row.target)?;
        push(
            &mut tables.edges,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::Edge(row.coordinate),
                parent(&inverse.edges, index)?,
                None,
                None,
                None,
            ),
            budget,
        )?;
    }
    tables.edge_arguments = vector(inventory.edge_arguments().len(), budget)?;
    for row in inventory.edge_arguments() {
        budget.charge_work(1)?;
        if row.incoming_definition >= tables.definitions.len()
            || row.target_definition >= tables.definitions.len()
        {
            return Err(invalid("edge definition"));
        }
        push(
            &mut tables.edge_arguments,
            occurrence(
                ProductionTileOccurrenceCoordinateV29::EdgeArgument(row.coordinate),
                parent(&inverse.edges, edge_index(inventory, row.coordinate.edge)?)?,
                Some(row.incoming_definition),
                Some(row.target_definition),
                Some(row.value),
            ),
            budget,
        )?;
    }
    let transient = inverse.storage()?;
    drop(inverse);
    budget.release_storage(transient)?;
    Ok(())
}
fn occurrence(
    coordinate: ProductionTileOccurrenceCoordinateV29,
    provenance: ProductionTileProvenanceV29,
    definition: Option<usize>,
    target_definition: Option<usize>,
    value: Option<ValueId>,
) -> ProductionTileOccurrenceV29 {
    ProductionTileOccurrenceV29 {
        coordinate,
        provenance,
        definition,
        target_definition,
        value,
        piece_aliases: 0..0,
    }
}

fn target_occurrence(
    target: ProductionTileTargetCoordinateV29,
) -> R<Option<ProductionTileOccurrenceCoordinateV29>> {
    use ProductionTileOccurrenceCoordinateV29 as C;
    use ProductionTileTargetCoordinateV29 as T;
    Ok(Some(match target {
        T::FunctionParameter {
            function,
            parameter,
        } => C::Definition(DefinitionCoordinate::FunctionArgument {
            function: FunctionCoordinate(u32_index(function)?),
            argument: u32_index(parameter)?,
        }),
        T::Block(block) => C::Block(block),
        T::BlockParameter { block, parameter } => {
            C::Definition(DefinitionCoordinate::BlockArgument {
                block,
                argument: u32_index(parameter)?,
            })
        }
        T::Operation(operation) => C::Operation(operation),
        T::Result { operation, result } => C::Definition(DefinitionCoordinate::Result {
            operation,
            result: u32_index(result)?,
        }),
        T::Terminator(block) => C::Terminator(block),
        T::Edge(edge) => C::Edge(edge),
        T::Anchor { .. } | T::ErasedValue(_) => return Ok(None),
    }))
}
fn mutable_occurrence<'a>(
    inventory: &Inventory<'_>,
    tables: &'a mut Tables,
    coordinate: ProductionTileOccurrenceCoordinateV29,
) -> R<&'a mut ProductionTileOccurrenceV29> {
    use ProductionTileOccurrenceCoordinateV29 as C;
    let row = match coordinate {
        C::Operation(c) => tables.operations.get_mut(operation_index(inventory, c)?),
        C::Definition(c) => tables.definitions.get_mut(definition_index(inventory, c)?),
        C::Block(c) => tables.blocks.get_mut(block_index(inventory, c)?),
        C::Terminator(c) => tables.terminators.get_mut(block_index(inventory, c)?),
        C::Edge(c) => tables.edges.get_mut(edge_index(inventory, c)?),
        C::Use(c) => tables.uses.get_mut(use_index(inventory, c)?),
        C::EdgeArgument(c) => tables
            .edge_arguments
            .get_mut(edge_argument_index(inventory, c)?),
    }
    .ok_or_else(|| invalid("alias coordinate"))?;
    if row.coordinate != coordinate {
        return Err(invalid("alias subject"));
    }
    Ok(row)
}
fn index_piece_aliases(
    inventory: &Inventory<'_>,
    tables: &mut Tables,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    type Key = (ProductionTileOccurrenceCoordinateV29, usize);
    budget.reserve_storage(size_of::<Vec<Key>>())?;
    let mut count = 0;
    for piece in &tables.pieces {
        budget.charge_work(1)?;
        if target_occurrence(piece.target)?.is_some() {
            count = sum(&[count, 1])?;
        }
        if let ProductionTileTargetCoordinateV29::Operation(operation) = piece.target {
            count = sum(&[
                count,
                inventory.operations()[operation_index(inventory, operation)?]
                    .results
                    .len(),
            ])?;
        }
    }
    let mut keys: Vec<Key> = vector(count, budget)?;
    for (ordinal, piece) in tables.pieces.iter().enumerate() {
        budget.charge_work(1)?;
        if let Some(coordinate) = target_occurrence(piece.target)? {
            push(&mut keys, (coordinate, ordinal), budget)?;
        }
        // A result retains both its defining operation's structural association and
        // every explicit source-result association. Neither substitutes for the other.
        if let ProductionTileTargetCoordinateV29::Operation(operation) = piece.target {
            for definition in inventory.operations()[operation_index(inventory, operation)?]
                .results
                .clone()
            {
                budget.charge_work(1)?;
                push(
                    &mut keys,
                    (
                        ProductionTileOccurrenceCoordinateV29::Definition(
                            inventory.definitions()[definition].coordinate,
                        ),
                        ordinal,
                    ),
                    budget,
                )?;
            }
        }
    }
    sort(&mut keys, |key| *key, budget)?;
    tables.piece_aliases = vector(keys.len(), budget)?;
    let mut cursor = 0;
    while cursor < keys.len() {
        let coordinate = keys[cursor].0;
        let first = cursor;
        loop {
            budget.charge_work(1)?;
            let (_, ordinal) = keys[cursor];
            if cursor > first && keys[cursor - 1].1 >= ordinal {
                return Err(invalid("alias ordinal order"));
            }
            let target = tables.pieces[ordinal].target;
            let defining_operation = matches!((target, coordinate),
                (ProductionTileTargetCoordinateV29::Operation(a), ProductionTileOccurrenceCoordinateV29::Definition(DefinitionCoordinate::Result { operation: b, .. })) if a == b);
            if target_occurrence(target)? != Some(coordinate) && !defining_operation {
                return Err(invalid("reverse piece alias"));
            }
            push(&mut tables.piece_aliases, ordinal, budget)?;
            cursor = sum(&[cursor, 1])?;
            if cursor == keys.len() || keys[cursor].0 != coordinate {
                break;
            }
        }
        let row = mutable_occurrence(inventory, tables, coordinate)?;
        if !row.piece_aliases.is_empty() {
            return Err(invalid("duplicate alias range"));
        }
        row.piece_aliases = first..cursor;
    }
    // Uses and ordered payloads share the exact owner's alias range, without Cartesian duplication.
    for index in 0..tables.uses.len() {
        budget.charge_work(1)?;
        let range = match inventory.uses()[index].coordinate {
            UseCoordinate::OperationOperand { operation, .. } => tables.operations
                [operation_index(inventory, operation)?]
            .piece_aliases
            .clone(),
            UseCoordinate::TerminatorOperand { block, .. } => tables.terminators
                [block_index(inventory, block)?]
            .piece_aliases
            .clone(),
        };
        tables.uses[index].piece_aliases = range;
    }
    for index in 0..tables.edge_arguments.len() {
        budget.charge_work(1)?;
        tables.edge_arguments[index].piece_aliases = tables.edges
            [edge_index(inventory, inventory.edge_arguments()[index].coordinate.edge)?]
        .piece_aliases
        .clone();
    }
    for rows in [
        &tables.operations,
        &tables.definitions,
        &tables.blocks,
        &tables.terminators,
        &tables.edges,
        &tables.uses,
        &tables.edge_arguments,
    ] {
        for row in rows {
            budget.charge_work(1)?;
            let aliases = tables
                .piece_aliases
                .get(row.piece_aliases.clone())
                .ok_or_else(|| invalid("foreign alias range"))?;
            if matches!(
                row.provenance,
                ProductionTileProvenanceV29::DeclarationArgument { .. }
            ) {
                if !aliases.is_empty() {
                    return Err(invalid("declaration aliases"));
                }
            } else if aliases.is_empty() {
                return Err(invalid("missing occurrence aliases"));
            }
        }
    }
    let transient = sum(&[size_of::<Vec<Key>>(), bytes::<Key>(keys.capacity())?])?;
    drop(keys);
    budget.release_storage(transient)?;
    Ok(())
}

fn attachment_family(value: TileAttachmentFamilyV29) -> ProductionTileAttachmentFamilyV29 {
    match value {
        TileAttachmentFamilyV29::InstanceSpans => ProductionTileAttachmentFamilyV29::InstanceSpans,
        TileAttachmentFamilyV29::InstanceSeeds => ProductionTileAttachmentFamilyV29::InstanceSeeds,
        TileAttachmentFamilyV29::InstanceControls => {
            ProductionTileAttachmentFamilyV29::InstanceControls
        }
        TileAttachmentFamilyV29::InstanceCalls => ProductionTileAttachmentFamilyV29::InstanceCalls,
        TileAttachmentFamilyV29::InstanceReturns => {
            ProductionTileAttachmentFamilyV29::InstanceReturns
        }
        TileAttachmentFamilyV29::RawSidecar => ProductionTileAttachmentFamilyV29::RawSidecar,
        TileAttachmentFamilyV29::SourceSlot => ProductionTileAttachmentFamilyV29::SourceSlot,
        TileAttachmentFamilyV29::MemoryAnchor => ProductionTileAttachmentFamilyV29::MemoryAnchor,
        TileAttachmentFamilyV29::PrivateArray => ProductionTileAttachmentFamilyV29::PrivateArray,
        TileAttachmentFamilyV29::Lifecycle => ProductionTileAttachmentFamilyV29::Lifecycle,
        TileAttachmentFamilyV29::Assertion => ProductionTileAttachmentFamilyV29::Assertion,
        TileAttachmentFamilyV29::TerminalFailure => ProductionTileAttachmentFamilyV29::TerminalFailure,
    }
}

#[cfg(test)]
#[test]
fn terminal_failure_attachment_tags_remain_distinct_in_public_queries() {
    use TileAttachmentFieldV29 as Internal;
    use ProductionTileAttachmentFieldV29 as Public;
    assert_eq!(attachment_family(TileAttachmentFamilyV29::TerminalFailure),
        ProductionTileAttachmentFamilyV29::TerminalFailure);
    let fields = [(Internal::FailureSourceBlock, Public::FailureSourceBlock),
        (Internal::FailureSourceEdge, Public::FailureSourceEdge),
        (Internal::FailureOriginalTarget, Public::FailureOriginalTarget),
        (Internal::FailureOriginalDiagnostic, Public::FailureOriginalDiagnostic),
        (Internal::FailureBlock, Public::FailureBlock), (Internal::FailureGap, Public::FailureGap),
        (Internal::FailureCleanup, Public::FailureCleanup), (Internal::FailureOperand, Public::FailureOperand),
        (Internal::FailureDiagnostic, Public::FailureDiagnostic), (Internal::FailureTerminator, Public::FailureTerminator)];
    for (index, (internal, public)) in fields.iter().enumerate() {
        assert_eq!(attachment_field(*internal), *public);
        assert!(fields[..index].iter().all(|(prior, _)| prior != internal));
    }
}

#[cfg(test)]
#[test]
fn invocation_attachment_fields_preserve_distinct_query_tags() {
    use ProductionTileAttachmentFieldV29 as Public;
    use TileAttachmentFieldV29 as Internal;
    let fields = [
        (Internal::RawInvocationSpan, Public::RawInvocationSpan),
        (
            Internal::RawInvocationPreheader,
            Public::RawInvocationPreheader,
        ),
        (Internal::RawInvocationEntry, Public::RawInvocationEntry),
        (
            Internal::RawInvocationTerminator,
            Public::RawInvocationTerminator,
        ),
        (Internal::RawInvocationEdge, Public::RawInvocationEdge),
        (
            Internal::RawInvocationArgument,
            Public::RawInvocationArgument,
        ),
        (Internal::RawInvocationInput, Public::RawInvocationInput),
        (
            Internal::RawInvocationInputMap,
            Public::RawInvocationInputMap,
        ),
        (
            Internal::RawInvocationParameter,
            Public::RawInvocationParameter,
        ),
        (Internal::RawInvocationOutput, Public::RawInvocationOutput),
        (
            Internal::RawInvocationConversion,
            Public::RawInvocationConversion,
        ),
    ];
    for (index, (internal, public)) in fields.iter().enumerate() {
        assert_eq!(attachment_field(*internal), *public);
        assert!(
            fields[..index]
                .iter()
                .all(|(_, previous)| previous != public)
        );
    }
}

#[cfg(test)]
#[test]
fn scalar_payload_fields_preserve_definition_address_and_operand_query_tags() {
    use ProductionTileAttachmentFieldV29 as Public;
    use TileAttachmentFieldV29 as Internal;
    let pairs = [
        (Internal::MemoryPosition, Public::MemoryPosition),
        (Internal::MemoryPointer, Public::MemoryPointer),
        (Internal::MemoryLoadResult, Public::MemoryLoadResult),
        (Internal::MemoryStoreValue, Public::MemoryStoreValue),
        (Internal::MemoryStoreUse, Public::MemoryStoreUse),
        (Internal::ObjectOperand, Public::ObjectOperand),
        (Internal::ObjectOperandUse, Public::ObjectOperandUse),
        (Internal::ObjectResult, Public::ObjectResult),
    ];
    for (index, (internal, expected)) in pairs.iter().enumerate() {
        assert_eq!(attachment_field(*internal), *expected);
        for (_, other) in &pairs[..index] { assert_ne!(*expected, *other); }
    }
}

fn attachment_field(value: TileAttachmentFieldV29) -> ProductionTileAttachmentFieldV29 {
    match value {
        TileAttachmentFieldV29::Span => ProductionTileAttachmentFieldV29::Span,
        TileAttachmentFieldV29::Parameters => ProductionTileAttachmentFieldV29::Parameters,
        TileAttachmentFieldV29::PhysicalBlock => ProductionTileAttachmentFieldV29::PhysicalBlock,
        TileAttachmentFieldV29::Terminator => ProductionTileAttachmentFieldV29::Terminator,
        TileAttachmentFieldV29::Edge => ProductionTileAttachmentFieldV29::Edge,
        TileAttachmentFieldV29::EdgeArgument => ProductionTileAttachmentFieldV29::EdgeArgument,
        TileAttachmentFieldV29::ReturnDefinition => {
            ProductionTileAttachmentFieldV29::ReturnDefinition
        }
        TileAttachmentFieldV29::ReturnUse => ProductionTileAttachmentFieldV29::ReturnUse,
        TileAttachmentFieldV29::ExpectedTarget => ProductionTileAttachmentFieldV29::ExpectedTarget,
        TileAttachmentFieldV29::ExpectedArgument => {
            ProductionTileAttachmentFieldV29::ExpectedArgument
        }
        TileAttachmentFieldV29::ArgumentPreparation => {
            ProductionTileAttachmentFieldV29::ArgumentPreparation
        }
        TileAttachmentFieldV29::CallSite => ProductionTileAttachmentFieldV29::CallSite,
        TileAttachmentFieldV29::DestinationPreparation => {
            ProductionTileAttachmentFieldV29::DestinationPreparation
        }
        TileAttachmentFieldV29::DestinationRange => {
            ProductionTileAttachmentFieldV29::DestinationRange
        }
        TileAttachmentFieldV29::DestinationPointer => {
            ProductionTileAttachmentFieldV29::DestinationPointer
        }
        TileAttachmentFieldV29::ArgumentDefinition => {
            ProductionTileAttachmentFieldV29::ArgumentDefinition
        }
        TileAttachmentFieldV29::ArgumentUse => ProductionTileAttachmentFieldV29::ArgumentUse,
        TileAttachmentFieldV29::ResultDefinition => {
            ProductionTileAttachmentFieldV29::ResultDefinition
        }
        TileAttachmentFieldV29::ReturnSite => ProductionTileAttachmentFieldV29::ReturnSite,
        TileAttachmentFieldV29::ReturnComponentInput => {
            ProductionTileAttachmentFieldV29::ReturnComponentInput
        }
        TileAttachmentFieldV29::ReturnComponentConversion => {
            ProductionTileAttachmentFieldV29::ReturnComponentConversion
        }
        TileAttachmentFieldV29::ReturnComponentOutput => {
            ProductionTileAttachmentFieldV29::ReturnComponentOutput
        }
        TileAttachmentFieldV29::ReturnComponentUse => {
            ProductionTileAttachmentFieldV29::ReturnComponentUse
        }
        TileAttachmentFieldV29::TransportComponentConversion => {
            ProductionTileAttachmentFieldV29::TransportComponentConversion
        }
        TileAttachmentFieldV29::TransportComponentOutput => {
            ProductionTileAttachmentFieldV29::TransportComponentOutput
        }
        TileAttachmentFieldV29::TransportComponentUse => {
            ProductionTileAttachmentFieldV29::TransportComponentUse
        }
        TileAttachmentFieldV29::RawBlock => ProductionTileAttachmentFieldV29::RawBlock,
        TileAttachmentFieldV29::RawStatementSpan => {
            ProductionTileAttachmentFieldV29::RawStatementSpan
        }
        TileAttachmentFieldV29::RawTerminatorSpan => {
            ProductionTileAttachmentFieldV29::RawTerminatorSpan
        }
        TileAttachmentFieldV29::RawSyntheticSpan => {
            ProductionTileAttachmentFieldV29::RawSyntheticSpan
        }
        TileAttachmentFieldV29::RawInvocationSpan => {
            ProductionTileAttachmentFieldV29::RawInvocationSpan
        }
        TileAttachmentFieldV29::RawInvocationPreheader => {
            ProductionTileAttachmentFieldV29::RawInvocationPreheader
        }
        TileAttachmentFieldV29::RawInvocationEntry => {
            ProductionTileAttachmentFieldV29::RawInvocationEntry
        }
        TileAttachmentFieldV29::RawInvocationTerminator => {
            ProductionTileAttachmentFieldV29::RawInvocationTerminator
        }
        TileAttachmentFieldV29::RawInvocationEdge => {
            ProductionTileAttachmentFieldV29::RawInvocationEdge
        }
        TileAttachmentFieldV29::RawInvocationArgument => {
            ProductionTileAttachmentFieldV29::RawInvocationArgument
        }
        TileAttachmentFieldV29::RawInvocationInput => {
            ProductionTileAttachmentFieldV29::RawInvocationInput
        }
        TileAttachmentFieldV29::RawInvocationInputMap => {
            ProductionTileAttachmentFieldV29::RawInvocationInputMap
        }
        TileAttachmentFieldV29::RawInvocationParameter => {
            ProductionTileAttachmentFieldV29::RawInvocationParameter
        }
        TileAttachmentFieldV29::RawInvocationOutput => {
            ProductionTileAttachmentFieldV29::RawInvocationOutput
        }
        TileAttachmentFieldV29::RawInvocationConversion => {
            ProductionTileAttachmentFieldV29::RawInvocationConversion
        }
        TileAttachmentFieldV29::RawParameter => ProductionTileAttachmentFieldV29::RawParameter,
        TileAttachmentFieldV29::RawParameterComponent => {
            ProductionTileAttachmentFieldV29::RawParameterComponent
        }
        TileAttachmentFieldV29::RawIgnoredParameter => {
            ProductionTileAttachmentFieldV29::RawIgnoredParameter
        }
        TileAttachmentFieldV29::RawGeneratedInput => {
            ProductionTileAttachmentFieldV29::RawGeneratedInput
        }
        TileAttachmentFieldV29::RawGeneratedOutput => {
            ProductionTileAttachmentFieldV29::RawGeneratedOutput
        }
        TileAttachmentFieldV29::RawCallArguments => {
            ProductionTileAttachmentFieldV29::RawCallArguments
        }
        TileAttachmentFieldV29::RawCallOperation => {
            ProductionTileAttachmentFieldV29::RawCallOperation
        }
        TileAttachmentFieldV29::RawCallDestination => {
            ProductionTileAttachmentFieldV29::RawCallDestination
        }
        TileAttachmentFieldV29::RawCallDestinationPointer => {
            ProductionTileAttachmentFieldV29::RawCallDestinationPointer
        }
        TileAttachmentFieldV29::RawNoNormalReturnTerminator => {
            ProductionTileAttachmentFieldV29::RawNoNormalReturnTerminator
        }
        TileAttachmentFieldV29::RawReturnTerminator => {
            ProductionTileAttachmentFieldV29::RawReturnTerminator
        }
        TileAttachmentFieldV29::RawReturnInput => ProductionTileAttachmentFieldV29::RawReturnInput,
        TileAttachmentFieldV29::RawReturnConversion => {
            ProductionTileAttachmentFieldV29::RawReturnConversion
        }
        TileAttachmentFieldV29::RawTransportArgument => {
            ProductionTileAttachmentFieldV29::RawTransportArgument
        }
        TileAttachmentFieldV29::RawTransportConversion => {
            ProductionTileAttachmentFieldV29::RawTransportConversion
        }
        TileAttachmentFieldV29::SlotRawPointer => ProductionTileAttachmentFieldV29::SlotRawPointer,
        TileAttachmentFieldV29::SlotPointer => ProductionTileAttachmentFieldV29::SlotPointer,
        TileAttachmentFieldV29::SlotCount => ProductionTileAttachmentFieldV29::SlotCount,
        TileAttachmentFieldV29::SlotCountLocation => {
            ProductionTileAttachmentFieldV29::SlotCountLocation
        }
        TileAttachmentFieldV29::SlotAllocation => ProductionTileAttachmentFieldV29::SlotAllocation,
        TileAttachmentFieldV29::MemoryPosition => ProductionTileAttachmentFieldV29::MemoryPosition,
        TileAttachmentFieldV29::MemoryPointer => ProductionTileAttachmentFieldV29::MemoryPointer,
        TileAttachmentFieldV29::MemoryLoadResult => ProductionTileAttachmentFieldV29::MemoryLoadResult,
        TileAttachmentFieldV29::MemoryStoreValue => ProductionTileAttachmentFieldV29::MemoryStoreValue,
        TileAttachmentFieldV29::MemoryStoreUse => ProductionTileAttachmentFieldV29::MemoryStoreUse,
        TileAttachmentFieldV29::ObjectOperand => ProductionTileAttachmentFieldV29::ObjectOperand,
        TileAttachmentFieldV29::ObjectOperandUse => ProductionTileAttachmentFieldV29::ObjectOperandUse,
        TileAttachmentFieldV29::ObjectResult => ProductionTileAttachmentFieldV29::ObjectResult,
        TileAttachmentFieldV29::ArrayCountLocation => {
            ProductionTileAttachmentFieldV29::ArrayCountLocation
        }
        TileAttachmentFieldV29::ArrayAllocation => {
            ProductionTileAttachmentFieldV29::ArrayAllocation
        }
        TileAttachmentFieldV29::ArrayPointer => ProductionTileAttachmentFieldV29::ArrayPointer,
        TileAttachmentFieldV29::ArrayCount => ProductionTileAttachmentFieldV29::ArrayCount,
        TileAttachmentFieldV29::ArraySourceRange => {
            ProductionTileAttachmentFieldV29::ArraySourceRange
        }
        TileAttachmentFieldV29::ArrayOriginalIndex => {
            ProductionTileAttachmentFieldV29::ArrayOriginalIndex
        }
        TileAttachmentFieldV29::ArrayDirectDefinition => {
            ProductionTileAttachmentFieldV29::ArrayDirectDefinition
        }
        TileAttachmentFieldV29::ArrayLiteralValue => {
            ProductionTileAttachmentFieldV29::ArrayLiteralValue
        }
        TileAttachmentFieldV29::ArrayLiteralDefinition => {
            ProductionTileAttachmentFieldV29::ArrayLiteralDefinition
        }
        TileAttachmentFieldV29::ArrayOffsetLocation => {
            ProductionTileAttachmentFieldV29::ArrayOffsetLocation
        }
        TileAttachmentFieldV29::ArrayGepLocation => {
            ProductionTileAttachmentFieldV29::ArrayGepLocation
        }
        TileAttachmentFieldV29::ArrayMemoryLocation => {
            ProductionTileAttachmentFieldV29::ArrayMemoryLocation
        }
        TileAttachmentFieldV29::ArrayOffset => ProductionTileAttachmentFieldV29::ArrayOffset,
        TileAttachmentFieldV29::ArrayGep => ProductionTileAttachmentFieldV29::ArrayGep,
        TileAttachmentFieldV29::LifecycleOriginalGap => {
            ProductionTileAttachmentFieldV29::LifecycleOriginalGap
        }
        TileAttachmentFieldV29::LifecycleBeforeGap => {
            ProductionTileAttachmentFieldV29::LifecycleBeforeGap
        }
        TileAttachmentFieldV29::LifecycleOperation => {
            ProductionTileAttachmentFieldV29::LifecycleOperation
        }
        TileAttachmentFieldV29::LifecycleOperand => {
            ProductionTileAttachmentFieldV29::LifecycleOperand
        }
        TileAttachmentFieldV29::LifecycleResult => {
            ProductionTileAttachmentFieldV29::LifecycleResult
        }
        TileAttachmentFieldV29::AssertSourceRange => {
            ProductionTileAttachmentFieldV29::AssertSourceRange
        }
        TileAttachmentFieldV29::AssertSourceBlock => {
            ProductionTileAttachmentFieldV29::AssertSourceBlock
        }
        TileAttachmentFieldV29::AssertSuccessBlock => {
            ProductionTileAttachmentFieldV29::AssertSuccessBlock
        }
        TileAttachmentFieldV29::AssertFailureBlock => {
            ProductionTileAttachmentFieldV29::AssertFailureBlock
        }
        TileAttachmentFieldV29::AssertCapturedCondition => {
            ProductionTileAttachmentFieldV29::AssertCapturedCondition
        }
        TileAttachmentFieldV29::AssertCapturedArgument => {
            ProductionTileAttachmentFieldV29::AssertCapturedArgument
        }
        TileAttachmentFieldV29::AssertConditionUse => {
            ProductionTileAttachmentFieldV29::AssertConditionUse
        }
        TileAttachmentFieldV29::AssertConditionDefinition => {
            ProductionTileAttachmentFieldV29::AssertConditionDefinition
        }
        TileAttachmentFieldV29::AssertSuccessEdge => {
            ProductionTileAttachmentFieldV29::AssertSuccessEdge
        }
        TileAttachmentFieldV29::AssertFailureEdge => {
            ProductionTileAttachmentFieldV29::AssertFailureEdge
        }
        TileAttachmentFieldV29::AssertSuccessArgument => {
            ProductionTileAttachmentFieldV29::AssertSuccessArgument
        }
        TileAttachmentFieldV29::FailureSourceBlock => ProductionTileAttachmentFieldV29::FailureSourceBlock,
        TileAttachmentFieldV29::FailureSourceEdge => ProductionTileAttachmentFieldV29::FailureSourceEdge,
        TileAttachmentFieldV29::FailureOriginalTarget => ProductionTileAttachmentFieldV29::FailureOriginalTarget,
        TileAttachmentFieldV29::FailureOriginalDiagnostic => ProductionTileAttachmentFieldV29::FailureOriginalDiagnostic,
        TileAttachmentFieldV29::FailureBlock => ProductionTileAttachmentFieldV29::FailureBlock,
        TileAttachmentFieldV29::FailureGap => ProductionTileAttachmentFieldV29::FailureGap,
        TileAttachmentFieldV29::FailureCleanup => ProductionTileAttachmentFieldV29::FailureCleanup,
        TileAttachmentFieldV29::FailureOperand => ProductionTileAttachmentFieldV29::FailureOperand,
        TileAttachmentFieldV29::FailureDiagnostic => ProductionTileAttachmentFieldV29::FailureDiagnostic,
        TileAttachmentFieldV29::FailureTerminator => ProductionTileAttachmentFieldV29::FailureTerminator,
    }
}
