//! Mandatory transport of ordered source selections through the checked graph.
//! This relation retains conditional obligations; it does not discharge them.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockControlV1 as BlockControl, CanonicalKirEdgeControlV1 as EdgeControl,
    CanonicalKirEdgePlacementV1 as EdgePlacement, CanonicalKirOutputUseV1 as OutputUse,
};
use fe2o3_kernel_ir::{
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate, CanonicalKirTransitionRangeV1,
};

#[path = "production_selected_final_source_v30.rs"]
pub(in super::super) mod final_source;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SelectedDefinitionRoleV30 {
    AccessPointer,
    AccessValue,
    Node(usize),
    Leaf { leaf: usize, component: usize },
    Guard(usize),
    GuardSelector(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SelectedDefinitionV30 {
    pub(super) input: Definition,
    // The complete checked range, never a guessed representative descendant.
    pub(super) outputs: CanonicalKirTransitionRangeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SelectedEdgeV30 {
    pub(super) input: EdgeArgument,
    pub(super) incoming: SelectedDefinitionV30,
    pub(super) target: SelectedDefinitionV30,
    pub(super) control: EdgeControl,
    pub(super) output: Option<EdgeArgument>,
    pub(super) output_incoming: Option<Definition>,
    pub(super) output_target: Option<Definition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SelectedTransportRowV30 {
    Access {
        root: usize,
        ordinal: usize,
        instance: ProductionCallInstanceIdV1,
        anchor: usize,
        disposition: ProductionOptimizedSourceOperationV18,
        output_pointer: Option<Definition>,
        output_value: Option<Definition>,
    },
    Definition {
        role: SelectedDefinitionRoleV30,
        relation: SelectedDefinitionV30,
    },
    Node {
        ordinal: usize,
        original: SourceReferenceSelectionNodeV29,
        parameter: Option<(Block, usize, BlockControl)>,
    },
    Incoming {
        ordinal: usize,
        original: SourceReferenceSelectionEdgeV29,
        relation: SelectedEdgeV30,
    },
    Invocation {
        node: usize,
        input_node: usize,
        original: Definition,
        relation: SelectedEdgeV30,
    },
    Leaf {
        ordinal: usize,
        original: PendingSourceSelectedLeafV30,
    },
    Guard {
        ordinal: usize,
        original: PendingSourceSelectedGuardV30,
        input: Edge,
        control: EdgeControl,
        output_selector: Option<OutputUse>,
    },
    Obligation {
        ordinal: usize,
        original: PendingSourceSelectedObligationV30,
        input: Block,
        control: BlockControl,
    },
}

enum Writer<'a> {
    Build(&'a mut Vec<SelectedTransportRowV30>),
    Replay {
        rows: &'a [SelectedTransportRowV30],
        next: usize,
    },
}

struct SelectedReplayFrameV30<'view, 'source, 'budget, 'work> {
    optimized: &'view ProductionOptimizedSourceCorrespondenceV18<'source>,
    root: usize,
    budget: &'budget mut ArgumentBudgetV1<'work>,
}

impl SelectedReplayFrameV30<'_, '_, '_, '_> {
    fn replay(self) -> SourceOwnedResultV18<()> {
        let Self {
            optimized,
            root,
            budget,
        } = self;
        budget.charge_work(3)?;
        let range = optimized.index.selected_roots.get(root).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("selected optimized root index"),
        )?;
        let rows = optimized.index.selected.get(range.clone()).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("selected optimized root range"),
        )?;
        let end = root.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        visit(
            optimized.original,
            optimized.checked,
            optimized.control,
            optimized.index,
            root..end,
            &mut Writer::Replay { rows, next: 0 },
            budget,
        )
    }
}

impl Writer<'_> {
    fn row(
        &mut self,
        row: SelectedTransportRowV30,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.charge_work(size_of::<SelectedTransportRowV30>())?;
        match self {
            Self::Build(rows) => {
                if rows.len() == rows.capacity() {
                    return resources::binding(
                        "selected optimized transport exceeded its original census",
                    );
                }
                rows.push(row);
            }
            Self::Replay { rows, next } => {
                if rows.get(*next) != Some(&row) {
                    return resources::binding(
                        "selected optimized transport differs from its complete original relation",
                    );
                }
                *next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        Ok(())
    }

    fn finish(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        budget.charge_work(1)?;
        if let Self::Replay { rows, next } = self {
            if *next != rows.len() {
                return resources::binding(
                    "selected optimized transport differs from its complete original relation",
                );
            }
        }
        Ok(())
    }
}

pub(super) fn headers() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SelectedReplayFrameV30<'_, '_, '_, '_>>()?,
        argument_product_v1(2, size_of::<SourceOwnedResultV18<()>>())?,
        scoped_raw_admission_v29::selected_source_rows_headers_v30()?,
        h::<Writer<'_>>()?,
        h::<Vec<SelectedTransportRowV30>>()?,
        h::<Vec<std::ops::Range<usize>>>()?,
        h::<(Vec<SelectedTransportRowV30>, Vec<std::ops::Range<usize>>)>()?,
        h::<SelectedTransportRowV30>()?,
        argument_product_v1(6, h::<SelectedDefinitionV30>()?)?,
        h::<SelectedEdgeV30>()?,
        h::<Option<Definition>>()?,
        h::<&[PendingSourceSelectedAccessV30]>()?,
        h::<&PendingSourceSelectedAccessV30>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()?,
        h::<SourceAddressValueAccessV29>()?,
        h::<Option<SourceAddressValueAccessV29>>()?,
        h::<Option<(Definition, Definition)>>()?,
        h::<BlockControl>()?,
        h::<EdgeControl>()?,
        h::<Option<OutputUse>>()?,
        h::<[Option<ValueId>; 7]>()?,
        h::<std::ops::Range<usize>>()?,
        argument_product_v1(6, h::<Definition>()?)?,
        h::<EdgeArgument>()?,
        h::<Edge>()?,
        h::<OpCoordinate>()?,
        h::<ProductionOptimizedSourceOperationV18>()?,
        h::<Option<(Block, usize, BlockControl)>>()?,
        h::<usize>()?,
        h::<()>()?,
    ])
}

fn definition(
    inventory: &Inventory<'_>,
    function: FunctionCoordinate,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Definition> {
    inventory
        .definition_for_value(function, value, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .map(|row| row.coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized value coordinate",
        ))
}

fn relation(
    checked: &Transition<'_, '_, '_, '_>,
    input: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SelectedDefinitionV30> {
    Ok(SelectedDefinitionV30 {
        input,
        outputs: attachments::definition_range(checked, input, budget)?,
    })
}

fn block(
    inventory: &Inventory<'_>,
    function: FunctionCoordinate,
    value: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Block> {
    inventory
        .block_for_id(function, value, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .map(|row| row.coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized block coordinate",
        ))
}

fn edge<'a>(
    inventory: &'a Inventory<'_>,
    coordinate: Edge,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'a>> {
    let owner = resources::block_index(inventory, coordinate.source, budget)?;
    budget.charge_work(3)?;
    let range = &inventory.blocks()[owner].edges;
    let index = range
        .start
        .checked_add(coordinate.successor as usize)
        .filter(|index| *index < range.end)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    inventory
        .edges()
        .get(index)
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized edge occurrence",
        ))
}

fn argument<'a>(
    inventory: &'a Inventory<'_>,
    coordinate: EdgeArgument,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1> {
    let owner = edge(inventory, coordinate.edge, budget)?;
    budget.charge_work(3)?;
    let index = owner
        .bindings
        .start
        .checked_add(coordinate.argument as usize)
        .filter(|index| *index < owner.bindings.end)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    inventory
        .edge_arguments()
        .get(index)
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized edge argument occurrence",
        ))
}

fn is_descendant(
    checked: &Transition<'_, '_, '_, '_>,
    source: SelectedDefinitionV30,
    output: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let first = source.outputs.start as usize;
    let end = first
        .checked_add(source.outputs.len as usize)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let rows = checked.rows().definition_outputs.get(first..end).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("selected optimized descendant range"),
    )?;
    // The checked range is already ordered by complete output coordinate.
    charge_execution_cfg_lookup_v29(rows.len(), budget).map_err(source_emission_error_v18)?;
    if rows
        .binary_search_by_key(&output, |row| row.output)
        .is_err()
    {
        return resources::binding(
            "selected optimized actual endpoint is not a checked descendant",
        );
    }
    Ok(())
}

fn transport_edge(
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    function: FunctionCoordinate,
    source: BlockId,
    ordinal: usize,
    target: BlockId,
    parameter: usize,
    value: Option<ValueId>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SelectedEdgeV30> {
    let input = EdgeArgument {
        edge: Edge {
            source: block(checked.input(), function, source, budget)?,
            successor: u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        },
        argument: u32::try_from(parameter).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    };
    let edge_row = edge(checked.input(), input.edge, budget)?;
    let row = argument(checked.input(), input, budget)?;
    budget.charge_work(6)?;
    if edge_row.target_id != target || Some(row.value) != value {
        return resources::binding(
            "selected optimized incoming edge differs from original physical replay",
        );
    }
    let incoming = relation(
        checked,
        checked
            .input()
            .definitions()
            .get(row.incoming_definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "selected optimized incoming definition",
            ))?
            .coordinate,
        budget,
    )?;
    let target_definition = checked
        .input()
        .definitions()
        .get(row.target_definition)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized target definition",
        ))?
        .coordinate;
    if target_definition
        != (Definition::BlockArgument {
            block: edge_row.target,
            argument: input.argument,
        })
    {
        return resources::binding("selected optimized original parameter differs");
    }
    let target = relation(checked, target_definition, budget)?;
    let placement = control.edge(input.edge, budget).map_err(transition_error)?;
    let output = control
        .edge_argument(input, budget)
        .map_err(transition_error)?;
    let (output_incoming, output_target) = if let Some(output) = output {
        if placement.placement != EdgePlacement::Retained(output.edge) {
            return resources::binding("selected optimized argument outlived its original edge");
        }
        let row = argument(checked.output(), output, budget)?;
        budget.charge_work(2)?;
        let output_incoming = checked
            .output()
            .definitions()
            .get(row.incoming_definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "selected optimized output incoming",
            ))?
            .coordinate;
        let output_target = checked
            .output()
            .definitions()
            .get(row.target_definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "selected optimized output target",
            ))?
            .coordinate;
        is_descendant(checked, incoming, output_incoming, budget)?;
        is_descendant(checked, target, output_target, budget)?;
        (Some(output_incoming), Some(output_target))
    } else {
        (None, None)
    };
    Ok(SelectedEdgeV30 {
        input,
        incoming,
        target,
        control: placement,
        output,
        output_incoming,
        output_target,
    })
}

fn row_count(
    rows: &[PendingSourceSelectedAccessV30],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let mut count = 0usize;
    for row in rows {
        budget.charge_work(8)?;
        count = argument_sum_v1(&[
            count,
            3,
            argument_product_v1(2, row.selection.nodes.len())?,
            row.selection.edges.len(),
            row.leaves.len(),
            argument_product_v1(3, row.guards.len())?,
            row.obligations.len(),
        ])?;
        for node in &row.selection.nodes {
            budget.charge_work(1)?;
            count = count
                .checked_add(usize::from(node.invocation.is_some()))
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        for leaf in &row.leaves {
            budget.charge_work(1)?;
            count = count
                .checked_add(match leaf.origin {
                    PendingSourceSelectedLeafOriginV30::Issued(_) => 7,
                    PendingSourceSelectedLeafOriginV30::Descriptor(_) => 5,
                })
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
    }
    Ok(count)
}

fn value_row(
    checked: &Transition<'_, '_, '_, '_>,
    function: FunctionCoordinate,
    role: SelectedDefinitionRoleV30,
    value: ValueId,
    writer: &mut Writer<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SelectedDefinitionV30> {
    let relation = relation(
        checked,
        definition(checked.input(), function, value, budget)?,
        budget,
    )?;
    writer.row(
        SelectedTransportRowV30::Definition { role, relation },
        budget,
    )?;
    Ok(relation)
}

fn access_output(
    checked: &Transition<'_, '_, '_, '_>,
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    source: &PendingSourceSelectedAccessV30,
    disposition: ProductionOptimizedSourceOperationV18,
    pointer: SelectedDefinitionV30,
    value: SelectedDefinitionV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(Definition, Definition)>> {
    let ProductionOptimizedSourceOperationV18::Retained { output, .. } = disposition else {
        return Ok(None);
    };
    let actual = source_operation_row_v18(checked.output(), output, budget)?.operation;
    scoped_raw_admission_v29::check_issued_original_effect_v18(
        original,
        root,
        source.instance.index(),
        source.anchor,
        actual,
        budget,
    )?;
    let access = source_address_value_access_v29(actual)
        .map_err(source_emission_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized memory opcode",
        ))?;
    budget.charge_work(7)?;
    if access.object || access.access != source.memory || access.writing != source.writing {
        return resources::binding("selected optimized memory effect or attributes differ");
    }
    let actual_pointer = definition(
        checked.output(),
        output.block.function,
        access.pointer,
        budget,
    )?;
    let actual_value = definition(
        checked.output(),
        output.block.function,
        access.value,
        budget,
    )?;
    is_descendant(checked, pointer, actual_pointer, budget)?;
    is_descendant(checked, value, actual_value, budget)?;
    let old_pointer = resources::definition_index(checked.input(), pointer.input, budget)?;
    let new_pointer = resources::definition_index(checked.output(), actual_pointer, budget)?;
    let old_value = resources::definition_index(checked.input(), value.input, budget)?;
    let new_value = resources::definition_index(checked.output(), actual_value, budget)?;
    budget.charge_work(8)?;
    if checked.input().definitions()[old_pointer].ty
        != checked.output().definitions()[new_pointer].ty
        || checked.input().definitions()[old_value].ty
            != checked.output().definitions()[new_value].ty
    {
        return resources::binding("selected optimized memory endpoint types differ");
    }
    Ok(Some((actual_pointer, actual_value)))
}

fn visit(
    original: &ProductionSourceCorrespondenceV18<'_>,
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    index: &SourceIndex,
    roots: std::ops::Range<usize>,
    writer: &mut Writer<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    for root in roots {
        let function = FunctionCoordinate(
            u32::try_from(original.source.root_row(root)?.function_ordinal)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let rows =
            scoped_raw_admission_v29::checked_selected_source_rows_v30(original, root, budget)?;
        for (ordinal, source) in rows.iter().enumerate() {
            let input = OpCoordinate {
                block: block(checked.input(), function, source.operation.0, budget)?,
                operation: source.operation.1,
            };
            let disposition = attachments::operation(checked, control, index, input, budget)?;
            let pointer = relation(
                checked,
                definition(checked.input(), function, source.pointer, budget)?,
                budget,
            )?;
            let value = relation(
                checked,
                definition(checked.input(), function, source.value, budget)?,
                budget,
            )?;
            let output = access_output(
                checked,
                original,
                root,
                source,
                disposition,
                pointer,
                value,
                budget,
            )?;
            writer.row(
                SelectedTransportRowV30::Access {
                    root,
                    ordinal,
                    instance: source.instance,
                    anchor: source.anchor,
                    disposition,
                    output_pointer: output.map(|row| row.0),
                    output_value: output.map(|row| row.1),
                },
                budget,
            )?;
            writer.row(
                SelectedTransportRowV30::Definition {
                    role: SelectedDefinitionRoleV30::AccessPointer,
                    relation: pointer,
                },
                budget,
            )?;
            writer.row(
                SelectedTransportRowV30::Definition {
                    role: SelectedDefinitionRoleV30::AccessValue,
                    relation: value,
                },
                budget,
            )?;
            for (ordinal, node) in source.selection.nodes.iter().enumerate() {
                let node_definition = value_row(
                    checked,
                    function,
                    SelectedDefinitionRoleV30::Node(ordinal),
                    node.pointer,
                    writer,
                    budget,
                )?;
                let parameter = if let Some((target, parameter)) = node.parameter {
                    let target = block(checked.input(), function, target, budget)?;
                    budget.charge_work(2)?;
                    if node_definition.input
                        != (Definition::BlockArgument {
                            block: target,
                            argument: u32::try_from(parameter)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        })
                    {
                        return resources::binding("selected optimized node parameter differs");
                    }
                    Some((
                        target,
                        parameter,
                        control.block(target, budget).map_err(transition_error)?,
                    ))
                } else {
                    None
                };
                writer.row(
                    SelectedTransportRowV30::Node {
                        ordinal,
                        original: node.original,
                        parameter,
                    },
                    budget,
                )?;
                if let Some(invocation) = node.invocation {
                    let (target, parameter) =
                        node.parameter
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected optimized invocation has no parameter",
                            ))?;
                    let relation = transport_edge(
                        checked,
                        control,
                        function,
                        invocation.source,
                        0,
                        target,
                        parameter,
                        invocation.argument,
                        budget,
                    )?;
                    let original =
                        definition(checked.input(), function, invocation.original, budget)?;
                    writer.row(
                        SelectedTransportRowV30::Invocation {
                            node: ordinal,
                            input_node: invocation.input,
                            original,
                            relation,
                        },
                        budget,
                    )?;
                }
            }
            for (ordinal, incoming) in source.selection.edges.iter().enumerate() {
                let relation = transport_edge(
                    checked,
                    control,
                    function,
                    incoming.source,
                    incoming.ordinal,
                    incoming.target,
                    incoming.parameter,
                    incoming.argument,
                    budget,
                )?;
                writer.row(
                    SelectedTransportRowV30::Incoming {
                        ordinal,
                        original: incoming.original,
                        relation,
                    },
                    budget,
                )?;
            }
            for (ordinal, leaf) in source.leaves.iter().enumerate() {
                writer.row(
                    SelectedTransportRowV30::Leaf {
                        ordinal,
                        original: *leaf,
                    },
                    budget,
                )?;
                let values = match leaf.origin {
                    PendingSourceSelectedLeafOriginV30::Issued(row) => [
                        Some(row.root_input),
                        Some(row.receiver),
                        Some(row.index),
                        Some(row.length),
                        Some(row.present),
                        Some(row.data),
                        Some(row.pointer),
                    ],
                    PendingSourceSelectedLeafOriginV30::Descriptor(row) => [
                        Some(row.slice),
                        Some(row.original_index),
                        Some(row.data),
                        Some(row.index),
                        Some(row.pointer),
                        None,
                        None,
                    ],
                };
                for (component, value) in values.into_iter().enumerate() {
                    budget.charge_work(1)?;
                    if let Some(value) = value {
                        value_row(
                            checked,
                            function,
                            SelectedDefinitionRoleV30::Leaf {
                                leaf: ordinal,
                                component,
                            },
                            value,
                            writer,
                            budget,
                        )?;
                    }
                }
            }
            for (ordinal, guard) in source.guards.iter().enumerate() {
                value_row(
                    checked,
                    function,
                    SelectedDefinitionRoleV30::Guard(ordinal),
                    guard.condition,
                    writer,
                    budget,
                )?;
                let input = Edge {
                    source: block(checked.input(), function, guard.block, budget)?,
                    successor: u32::try_from(guard.edge)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                };
                let block_index = resources::block_index(checked.input(), input.source, budget)?;
                let first = checked.input().blocks()[block_index].terminator_uses.start;
                budget.charge_work(4)?;
                let selector = checked
                    .input()
                    .uses()
                    .get(first)
                    .filter(|row| {
                        row.coordinate
                            == (UseCoordinate::TerminatorOperand {
                                block: input.source,
                                operand: 0,
                            })
                    })
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "selected optimized original guard selector",
                    ))?;
                let selector_relation = relation(
                    checked,
                    checked
                        .input()
                        .definitions()
                        .get(selector.definition)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected optimized original guard definition",
                        ))?
                        .coordinate,
                    budget,
                )?;
                writer.row(
                    SelectedTransportRowV30::Definition {
                        role: SelectedDefinitionRoleV30::GuardSelector(ordinal),
                        relation: selector_relation,
                    },
                    budget,
                )?;
                let output_selector = control
                    .operand(selector.coordinate, budget)
                    .map_err(transition_error)?;
                if let Some(output) = output_selector {
                    is_descendant(checked, selector_relation, output.definition, budget)?;
                }
                let control = control.edge(input, budget).map_err(transition_error)?;
                writer.row(
                    SelectedTransportRowV30::Guard {
                        ordinal,
                        original: *guard,
                        input,
                        control,
                        output_selector,
                    },
                    budget,
                )?;
            }
            for (ordinal, obligation) in source.obligations.iter().enumerate() {
                let input = block(checked.input(), function, obligation.block, budget)?;
                let control = control.block(input, budget).map_err(transition_error)?;
                writer.row(
                    SelectedTransportRowV30::Obligation {
                        ordinal,
                        original: *obligation,
                        input,
                        control,
                    },
                    budget,
                )?;
            }
        }
    }
    writer.finish(budget)
}

pub(super) fn build(
    original: &ProductionSourceCorrespondenceV18<'_>,
    checked: &Transition<'_, '_, '_, '_>,
    control: &Control<'_, '_, '_>,
    index: &SourceIndex,
    credit: &std::cell::Cell<usize>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(Vec<SelectedTransportRowV30>, Vec<std::ops::Range<usize>>)> {
    let mut count = 0usize;
    let root_count = original.source.root_count(budget)?;
    let mut roots = resources::owned_vector(root_count, credit, budget)?;
    for root in 0..root_count {
        let rows =
            scoped_raw_admission_v29::checked_selected_source_rows_v30(original, root, budget)?;
        let first = count;
        count = count
            .checked_add(row_count(rows, budget)?)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        roots.push(first..count);
    }
    let mut rows = resources::owned_vector(count, credit, budget)?;
    visit(
        original,
        checked,
        control,
        index,
        0..root_count,
        &mut Writer::Build(&mut rows),
        budget,
    )?;
    budget.charge_work(1)?;
    if rows.len() != count {
        return resources::binding("selected optimized transport lost an original record");
    }
    Ok((rows, roots))
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    // Kept private to the source/final adapters. The same checked transition and
    // original owner remain borrowed; raw records are not publication evidence.
    pub(in super::super) fn replay_selected_transport_v30(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.query(budget)?;
        self.original.retain_query(
            SelectedReplayFrameV30 {
                optimized: self,
                root,
                budget,
            }
            .replay(),
        )
    }
}

#[cfg(test)]
include!("production_optimized_source_selection_oracle_v30_tests.rs");
