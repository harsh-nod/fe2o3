//! Reads actual canonical scalar operations, control selectors and ordered phi edges.

use super::*;
use fe2o3_kernel_ir::{CanonicalKirBlockCoordinateV1 as Block, ValueId};

pub(crate) struct TargetEdge {
    pub target: Block,
    pub arguments: Vec<(usize, usize)>,
}

pub(crate) enum TargetBranch {
    Goto,
    Switch {
        selector: usize,
        cases: Vec<(u128, usize)>,
        otherwise: usize,
    },
    Return,
}

pub(crate) struct TargetBlock {
    owner: usize,
    coordinate: Block,
    pub program: CanonicalProgramV30,
    pub edges: Vec<TargetEdge>,
    pub branch: TargetBranch,
    owned: Vec<bool>,
}

impl TargetBlock {
    pub(crate) fn derive(
        inventory: &Inventory<'_>,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let block = inventory.blocks().get(block).ok_or(Error::Statement(
            "original MIR target control block is absent",
        ))?;
        let function = inventory
            .functions()
            .get(block.coordinate.function.0 as usize)
            .ok_or(Error::Statement(
                "original MIR target control function is absent",
            ))?;
        let capacity = block
            .operations
            .len()
            .checked_add(function.definitions.len())
            .ok_or(Resource::Arithmetic)?;
        let mut program = CanonicalProgramV30 {
            nodes: vector(capacity, out)?,
            definitions: vector(function.definitions.len(), out)?,
            definition_start: function.definitions.start,
            returned: None,
            arguments: inventory.definitions().len(),
        };
        out.budget.charge_work(function.definitions.len())?;
        program.definitions.resize(function.definitions.len(), None);
        let mut owned = vector(function.definitions.len(), out)?;
        out.budget.charge_work(function.definitions.len())?;
        owned.resize(function.definitions.len(), false);
        for operation in block.operations.clone() {
            for definition in inventory.operations()[operation].results.clone() {
                out.budget.charge_work(1)?;
                owned[definition - function.definitions.start] = true;
            }
        }
        for at in block.operations.clone() {
            out.budget.charge_work(8)?;
            let operation = &inventory.operations()[at];
            if operation.results.len() != 1
                || !operation.effects.is_empty()
                || operation.operands.len() > 3
            {
                return Err(Error::Statement(
                    "original MIR target control operation effect or arity is not modeled",
                ));
            }
            let mut inputs = [0usize; 3];
            for (ordinal, operand) in operation.operands.clone().enumerate() {
                inputs[ordinal] = read(
                    &mut program,
                    inventory,
                    &owned,
                    inventory.uses()[operand].definition,
                    out,
                )?;
            }
            let result = operation.results.start;
            let ty = scalar(inventory.definitions()[result].ty)?;
            let expression = operation_expression(
                &operation.operation.kind,
                &inputs[..operation.operands.len()],
                ty,
                &program.nodes,
            )?;
            let node = program.push(
                NodeV30 {
                    scalar: ty,
                    expression,
                },
                out,
            )?;
            if program.definitions[result - program.definition_start]
                .replace(node)
                .is_some()
            {
                return Err(Error::Statement(
                    "original MIR target control result is repeated",
                ));
            }
        }
        let mut edges = vector(block.edges.len(), out)?;
        for edge in block.edges.clone() {
            out.budget.charge_work(3)?;
            let edge = &inventory.edges()[edge];
            if edge.target.function != block.coordinate.function
                || edge.arguments.len() != edge.bindings.len()
            {
                return Err(Error::Statement("original MIR target control edge differs"));
            }
            let mut arguments = vector(edge.bindings.len(), out)?;
            for binding in &inventory.edge_arguments()[edge.bindings.clone()] {
                out.budget.charge_work(3)?;
                let value = read(
                    &mut program,
                    inventory,
                    &owned,
                    binding.incoming_definition,
                    out,
                )?;
                let target = inventory
                    .definitions()
                    .get(binding.target_definition)
                    .ok_or(Error::Statement(
                        "original MIR target edge parameter is absent",
                    ))?;
                if !matches!(target.coordinate, Definition::BlockArgument { block, .. } if block == edge.target)
                    || scalar(target.ty)? != program.nodes[value].scalar
                    || arguments
                        .last()
                        .is_some_and(|&(last, _)| last >= binding.target_definition)
                {
                    return Err(Error::Statement(
                        "original MIR target edge parameter type or owner differs",
                    ));
                }
                arguments.push((binding.target_definition, value));
            }
            edges.push(TargetEdge {
                target: edge.target,
                arguments,
            });
        }
        let branch = match block.terminator {
            Terminator::Branch { .. } if edges.len() == 1 => TargetBranch::Goto,
            Terminator::ConditionalBranch { condition, .. } if edges.len() == 2 => {
                let selector = read_value(
                    &mut program,
                    inventory,
                    &owned,
                    block.coordinate,
                    *condition,
                    out,
                )?;
                if program.nodes[selector].scalar != ScalarV30::Bool {
                    return Err(Error::Statement(
                        "original MIR target branch condition is not boolean",
                    ));
                }
                let mut cases = vector(1, out)?;
                cases.push((1, 0));
                TargetBranch::Switch {
                    selector,
                    cases,
                    otherwise: 1,
                }
            }
            Terminator::Switch {
                selector, cases, ..
            } if edges.len() == cases.len() + 1 => {
                let selector = read_value(
                    &mut program,
                    inventory,
                    &owned,
                    block.coordinate,
                    *selector,
                    out,
                )?;
                let mut values = vector(cases.len(), out)?;
                for (ordinal, case) in cases.iter().enumerate() {
                    out.budget.charge_work(2)?;
                    values.push((u128::from(case.value), ordinal));
                }
                TargetBranch::Switch {
                    selector,
                    cases: values,
                    otherwise: cases.len(),
                }
            }
            Terminator::IntegerSwitch {
                selector, cases, ..
            } if edges.len() == cases.len() + 1 => {
                let selector = read_value(
                    &mut program,
                    inventory,
                    &owned,
                    block.coordinate,
                    *selector,
                    out,
                )?;
                let mut values = vector(cases.len(), out)?;
                for (ordinal, case) in cases.iter().enumerate() {
                    out.budget.charge_work(2)?;
                    values.push((super::super::super::bits(&case.value), ordinal));
                }
                TargetBranch::Switch {
                    selector,
                    cases: values,
                    otherwise: cases.len(),
                }
            }
            Terminator::Return { values } if values.len() <= 1 && edges.is_empty() => {
                if let Some(&value) = values.first() {
                    program.returned = Some(read_value(
                        &mut program,
                        inventory,
                        &owned,
                        block.coordinate,
                        value,
                        out,
                    )?);
                }
                TargetBranch::Return
            }
            _ => {
                return Err(Error::Statement(
                    "original MIR target exceptional control is not modeled",
                ));
            }
        };
        Ok(Self {
            owner: std::ptr::from_ref(inventory.owner()) as usize,
            coordinate: block.coordinate,
            program,
            edges,
            branch,
            owned,
        })
    }

    pub(crate) fn observe(
        &mut self,
        inventory: &Inventory<'_>,
        definition: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        self.check_owner(inventory)?;
        read(&mut self.program, inventory, &self.owned, definition, out)
    }

    pub(crate) fn coordinate(&self, inventory: &Inventory<'_>) -> Result<Block> {
        self.check_owner(inventory)?;
        Ok(self.coordinate)
    }

    fn check_owner(&self, inventory: &Inventory<'_>) -> Result<()> {
        if self.owner != std::ptr::from_ref(inventory.owner()) as usize {
            return Err(Error::Statement(
                "original MIR target block has a foreign owner",
            ));
        }
        Ok(())
    }
}

fn read(
    program: &mut CanonicalProgramV30,
    inventory: &Inventory<'_>,
    owned: &[bool],
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    out.budget.charge_work(4)?;
    let slot = definition
        .checked_sub(program.definition_start)
        .filter(|slot| *slot < program.definitions.len())
        .ok_or(Error::Statement(
            "original MIR target input definition is foreign",
        ))?;
    if let Some(node) = program.definitions[slot] {
        return Ok(node);
    }
    if owned[slot] {
        return Err(Error::Statement(
            "original MIR target input precedes its block definition",
        ));
    }
    let node = program.push(
        NodeV30 {
            scalar: scalar(inventory.definitions()[definition].ty)?,
            expression: ExpressionV30::Argument(
                u32::try_from(definition).map_err(|_| Resource::Arithmetic)?,
            ),
        },
        out,
    )?;
    program.definitions[slot] = Some(node);
    Ok(node)
}

fn read_value(
    program: &mut CanonicalProgramV30,
    inventory: &Inventory<'_>,
    owned: &[bool],
    block: Block,
    value: ValueId,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let definition = inventory
        .definition_index_for_value(block.function, value, out.budget)?
        .ok_or(Error::Statement(
            "original MIR target selector or return definition is absent",
        ))?;
    read(program, inventory, owned, definition, out)
}

fn headers() -> usize {
    operation_headers_v31()
        + size_of::<TargetBlock>()
        + size_of::<Result<TargetBlock>>()
        + size_of::<CanonicalProgramV30>()
        + size_of::<Vec<bool>>()
        + size_of::<Vec<TargetEdge>>()
        + size_of::<Vec<(usize, usize)>>()
        + size_of::<Vec<(u128, usize)>>()
        + size_of::<TargetEdge>()
        + size_of::<TargetBranch>()
        + size_of::<[usize; 3]>()
        + size_of::<NodeV30>()
        + size_of::<Option<usize>>()
        + size_of::<&Inventory<'_>>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Result<usize>>()
        + size_of::<Result<ExpressionV30>>()
}
