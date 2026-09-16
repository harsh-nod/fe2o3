// Owned copies of the checked index's control occurrences. These private rows
// retain identities for a later relation; they do not replace its edge rules.

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceOutputControlSelectionV1 {
    Branch,
    Boolean {
        value: ValueId,
        expected: bool,
    },
    SwitchCase {
        value: ValueId,
        case: u64,
    },
    SwitchDefault {
        value: ValueId,
    },
    IntegerCase {
        value: ValueId,
        case: fe2o3_kernel_ir::Constant,
    },
    IntegerDefault {
        value: ValueId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceOutputControlEdgeIdentityV1 {
    coordinate: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    target: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    selection: SourceOutputControlSelectionV1,
    argument_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceOutputControlEdgeRowV1 {
    input: SourceOutputControlEdgeIdentityV1,
    checked: fe2o3_kernel_analysis::CanonicalKirEdgeControlV1,
    output: Option<SourceOutputControlEdgeIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputControlUseIdentityV1 {
    coordinate: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    value: ValueId,
    definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputControlUseRowV1 {
    input: SourceOutputControlUseIdentityV1,
    output: Option<SourceOutputControlUseIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputControlArgumentIdentityV1 {
    coordinate: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1,
    value: ValueId,
    incoming: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    target: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    target_value: ValueId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputControlArgumentRowV1 {
    input: SourceOutputControlArgumentIdentityV1,
    output: Option<SourceOutputControlArgumentIdentityV1>,
}

#[derive(Debug, Eq, PartialEq)]
struct SourceOutputCheckedControlRowsV1 {
    edges: Vec<SourceOutputControlEdgeRowV1>,
    uses: Vec<SourceOutputControlUseRowV1>,
    arguments: Vec<SourceOutputControlArgumentRowV1>,
    compare_uses: Vec<SourceOutputControlUseRowV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceOutputCheckedControlRowsStorageV1(usize);

impl SourceOutputCheckedControlRowsStorageV1 {
    const fn retained_storage(self) -> usize {
        self.0
    }
}

fn source_output_control_edge_identity_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputControlEdgeIdentityV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let edge = assert_origin_find_v1(inventory.edges(), budget, |row, budget| {
        budget.charge_work(4)?;
        Ok(row.coordinate.cmp(&coordinate))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("checked control edge identity is absent"))?;
    let block = assert_origin_find_v1(inventory.blocks(), budget, |row, budget| {
        budget.charge_work(3)?;
        Ok(row.coordinate.cmp(&coordinate.source))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("checked control source block is absent"))?;
    budget.charge_work(7).map_err(Error::Resource)?;
    let edge = &inventory.edges()[edge];
    let occurrence = coordinate.successor as usize;
    let selection = match inventory.blocks()[block].terminator {
        Terminator::Branch { .. } if occurrence == 0 => SourceOutputControlSelectionV1::Branch,
        Terminator::ConditionalBranch { condition, .. } if occurrence < 2 => {
            SourceOutputControlSelectionV1::Boolean {
                value: *condition,
                expected: occurrence == 0,
            }
        }
        Terminator::Switch {
            selector, cases, ..
        } => {
            if occurrence == cases.len() {
                SourceOutputControlSelectionV1::SwitchDefault { value: *selector }
            } else {
                SourceOutputControlSelectionV1::SwitchCase {
                    value: *selector,
                    case: cases
                        .get(occurrence)
                        .ok_or(Error::Invalid(
                            "checked control switch occurrence is absent",
                        ))?
                        .value,
                }
            }
        }
        Terminator::IntegerSwitch {
            selector, cases, ..
        } => {
            if occurrence == cases.len() {
                SourceOutputControlSelectionV1::IntegerDefault { value: *selector }
            } else {
                SourceOutputControlSelectionV1::IntegerCase {
                    value: *selector,
                    // Constant is fixed-size; clone preserves signedness/width
                    // and does not allocate or encode a new wire vocabulary.
                    case: cases
                        .get(occurrence)
                        .ok_or(Error::Invalid(
                            "checked control integer occurrence is absent",
                        ))?
                        .value
                        .clone(),
                }
            }
        }
        _ => {
            return Err(Error::Invalid(
                "checked control terminator has no such occurrence",
            ));
        }
    };
    Ok(SourceOutputControlEdgeIdentityV1 {
        coordinate,
        target: edge.target,
        selection,
        argument_count: u32::try_from(edge.arguments.len())
            .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
    })
}

fn source_output_control_use_identity_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputControlUseIdentityV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Use;
    budget.charge_work(2).map_err(Error::Resource)?;
    let (block, operand) = match coordinate {
        Use::OperationOperand { operation, operand } => (operation.block, operand),
        Use::TerminatorOperand { block, operand } => (block, operand),
    };
    let block = assert_origin_find_v1(inventory.blocks(), budget, |row, budget| {
        budget.charge_work(3)?;
        Ok(row.coordinate.cmp(&block))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("checked control operand block is absent"))?;
    budget.charge_work(9).map_err(Error::Resource)?;
    let block = &inventory.blocks()[block];
    // Inventory uses are grouped by block/operation, not by the enum's Ord.
    // Resolve the exact structured operand span, never binary-search all uses.
    let span = match coordinate {
        Use::OperationOperand { operation, .. } => {
            let operations = inventory
                .operations()
                .get(block.operations.clone())
                .ok_or(Error::Invalid("checked control operation span is absent"))?;
            let row = operations
                .get(operation.operation as usize)
                .ok_or(Error::Invalid(
                    "checked control operand operation is absent",
                ))?;
            if row.coordinate != operation {
                return Err(Error::Invalid(
                    "checked control operation coordinate differs",
                ));
            }
            row.operands.clone()
        }
        Use::TerminatorOperand { .. } => block.terminator_uses.clone(),
    };
    let uses = inventory
        .uses()
        .get(span)
        .ok_or(Error::Invalid("checked control operand span is absent"))?;
    let used = uses
        .get(operand as usize)
        .ok_or(Error::Invalid("checked control operand identity is absent"))?;
    if used.coordinate != coordinate {
        return Err(Error::Invalid("checked control operand coordinate differs"));
    }
    let definition = inventory
        .definitions()
        .get(used.definition)
        .ok_or(Error::Invalid(
            "checked control operand definition is absent",
        ))?;
    if definition.value != Some(used.value) {
        return Err(Error::Invalid(
            "checked control operand definition value differs",
        ));
    }
    Ok(SourceOutputControlUseIdentityV1 {
        coordinate,
        value: used.value,
        definition: definition.coordinate,
    })
}

fn source_output_control_argument_identity_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputControlArgumentIdentityV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let index = assert_origin_find_v1(inventory.edge_arguments(), budget, |row, budget| {
        budget.charge_work(5)?;
        Ok(row.coordinate.cmp(&coordinate))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid(
        "checked control edge argument identity is absent",
    ))?;
    budget.charge_work(8).map_err(Error::Resource)?;
    let argument = &inventory.edge_arguments()[index];
    let incoming = inventory
        .definitions()
        .get(argument.incoming_definition)
        .ok_or(Error::Invalid(
            "checked control incoming argument definition is absent",
        ))?;
    let target = inventory
        .definitions()
        .get(argument.target_definition)
        .ok_or(Error::Invalid(
            "checked control target argument definition is absent",
        ))?;
    if incoming.value != Some(argument.value) {
        return Err(Error::Invalid(
            "checked control argument definition value differs",
        ));
    }
    Ok(SourceOutputControlArgumentIdentityV1 {
        coordinate,
        value: argument.value,
        incoming: incoming.coordinate,
        target: target.coordinate,
        target_value: target.value.ok_or(Error::Invalid(
            "checked control target argument has no block value",
        ))?,
    })
}

// The caller already owns and reserves both inventories and the original
// checked index. No index is reconstructed and no inventory borrow escapes.
// Every exit restores the incoming floor; success transfers exactly the owned
// row header and actual capacities, separately reserved by the receiving view.
fn source_output_checked_control_rows_v1(
    source: &ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        SourceOutputCheckedControlRowsV1,
        SourceOutputCheckedControlRowsStorageV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    source_output_global_scratch_scope_v1(budget, |budget| {
        budget.charge_work(5).map_err(Error::Resource)?;
        if !std::ptr::eq(source.executable(), coordinates.input())
            || !std::ptr::eq(coordinates.output(), transition.input().owner())
            || !std::ptr::eq(transition.input(), control.input())
            || !std::ptr::eq(transition.output(), control.output())
        {
            return Err(Error::Invalid("control row input owners differ"));
        }
        budget.charge_work(2).map_err(Error::Resource)?;
        let floor = budget.storage();
        budget
            .reserve_storage(std::mem::size_of::<SourceOutputCheckedControlRowsV1>())
            .map_err(Error::Resource)?;
        let mut rows = SourceOutputCheckedControlRowsV1 {
            edges: Vec::new(),
            uses: Vec::new(),
            arguments: Vec::new(),
            compare_uses: Vec::new(),
        };
        for edge in control.input().edges() {
            budget.charge_work(2).map_err(Error::Resource)?;
            let input =
                source_output_control_edge_identity_v1(control.input(), edge.coordinate, budget)?;
            let checked = control
                .edge(edge.coordinate, budget)
                .map_err(Error::Transition)?;
            let output = match checked.placement {
                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Retained(coordinate) => Some(
                    source_output_control_edge_identity_v1(control.output(), coordinate, budget)?,
                ),
                fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::InternalConnector(_)
                | fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::Omitted => None,
            };
            assert_origin_push_v1(
                &mut rows.edges,
                SourceOutputControlEdgeRowV1 {
                    input,
                    checked,
                    output,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
        // Use exact inventory spans, including Return operands and every edge
        // argument use. Predicate uses are not inferred from selected targets.
        for block in control.input().blocks() {
            budget.charge_work(2).map_err(Error::Resource)?;
            let uses = control
                .input()
                .uses()
                .get(block.terminator_uses.clone())
                .ok_or(Error::Invalid(
                    "checked control terminator use span is absent",
                ))?;
            for used in uses {
                budget.charge_work(2).map_err(Error::Resource)?;
                let input = source_output_control_use_identity_v1(
                    control.input(),
                    used.coordinate,
                    budget,
                )?;
                let output = match control
                    .operand(used.coordinate, budget)
                    .map_err(Error::Transition)?
                {
                    Some(expected) => {
                        let actual = source_output_control_use_identity_v1(
                            control.output(),
                            expected.coordinate,
                            budget,
                        )?;
                        budget.charge_work(4).map_err(Error::Resource)?;
                        if actual.definition != expected.definition {
                            return Err(Error::Invalid(
                                "checked control mapped operand definition differs",
                            ));
                        }
                        Some(actual)
                    }
                    None => None,
                };
                assert_origin_push_v1(
                    &mut rows.uses,
                    SourceOutputControlUseRowV1 { input, output },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
        }
        // Copy original Compare operand occurrences before the checked index
        // is dropped. An eliminated operand remains explicitly absent in O.
        for operation in control.input().operations() {
            budget.charge_work(1).map_err(Error::Resource)?;
            if !matches!(operation.operation.kind, OperationKind::Compare { .. }) {
                continue;
            }
            budget.charge_work(1).map_err(Error::Resource)?;
            let uses = control
                .input()
                .uses()
                .get(operation.operands.clone())
                .ok_or(Error::Invalid("checked Compare operand span absent"))?;
            for used in uses {
                budget.charge_work(2).map_err(Error::Resource)?;
                let input = source_output_control_use_identity_v1(
                    control.input(),
                    used.coordinate,
                    budget,
                )?;
                let output = match control
                    .operand(used.coordinate, budget)
                    .map_err(Error::Transition)?
                {
                    Some(expected) => {
                        let actual = source_output_control_use_identity_v1(
                            control.output(),
                            expected.coordinate,
                            budget,
                        )?;
                        budget.charge_work(1).map_err(Error::Resource)?;
                        if actual.definition != expected.definition {
                            return Err(Error::Invalid("checked Compare definition differs"));
                        }
                        Some(actual)
                    }
                    None => None,
                };
                assert_origin_push_v1(
                    &mut rows.compare_uses,
                    SourceOutputControlUseRowV1 { input, output },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
        }
        for argument in control.input().edge_arguments() {
            budget.charge_work(2).map_err(Error::Resource)?;
            let input = source_output_control_argument_identity_v1(
                control.input(),
                argument.coordinate,
                budget,
            )?;
            let output = match control
                .edge_argument(argument.coordinate, budget)
                .map_err(Error::Transition)?
            {
                Some(coordinate) => Some(source_output_control_argument_identity_v1(
                    control.output(),
                    coordinate,
                    budget,
                )?),
                None => None,
            };
            assert_origin_push_v1(
                &mut rows.arguments,
                SourceOutputControlArgumentRowV1 { input, output },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
        budget.charge_work(1).map_err(Error::Resource)?;
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        Ok((rows, SourceOutputCheckedControlRowsStorageV1(retained)))
    })
}
