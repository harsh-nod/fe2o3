//! Shared occurrence transport from the independently checked aggregate pair.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirOperationCoordinateV1 as Operation,
};

/// One surviving operation. A replaced private read is not a retained effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirAggregateOperationTransportV30 {
    output: Operation,
    retained: bool,
}
impl CanonicalKirAggregateOperationTransportV30 {
    /// Returns the actual output occurrence, including entry-prefix insertion.
    pub const fn output(self) -> Operation {
        self.output
    }
    /// Reports whether the original operation is retained, rather than a copy.
    pub const fn is_retained(self) -> bool {
        self.retained
    }
}

/// Indexed transport over exact borrowed inventories and the actual checked
/// pair. No graph, executable, source proof, or authority is manufactured.
/// Keep the pair, both inventories and the returned storage receipt paid.
pub struct CheckedCanonicalKirAggregateOccurrencesV30<'s, 'g> {
    checked: &'s CheckedCanonicalKirAggregateSsaV18<'g>,
    input: &'s Inventory<'g>,
    output: &'s Inventory<'g>,
    operations: Vec<Option<CanonicalKirAggregateOperationTransportV30>>,
    definitions: Vec<Option<usize>>,
}

fn headers() -> Result<usize> {
    type Frame<'a> = (
        &'a CheckedCanonicalKirAggregateSsaV18<'a>,
        [&'a Inventory<'a>; 2],
        &'a mut Budget<'a>,
        CheckedCanonicalKirAggregateOccurrencesV30<'a, 'a>,
        CanonicalKirAggregateSsaStorageV18,
        Result<(
            CheckedCanonicalKirAggregateOccurrencesV30<'a, 'a>,
            CanonicalKirAggregateSsaStorageV18,
        )>,
        [&'a crate::CanonicalKirBlockRefV1<'a>; 2],
        &'a crate::CanonicalKirOperationRefV1<'a>,
        [&'a crate::CanonicalKirDefinitionRefV1<'a>; 2],
        Option<&'a crate::CanonicalKirDefinitionRefV1<'a>>,
        [&'a crate::CanonicalKirEdgeRefV1<'a>; 2],
        Option<usize>,
        Result<Option<usize>>,
        (usize, ValueId, &'a Inventory<'a>),
        [usize; 10],
    );
    add(size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>())
}

impl<'s, 'g> CheckedCanonicalKirAggregateOccurrencesV30<'s, 'g> {
    /// Builds the shared index from an existing checked pair, never from a
    /// caller-authored witness alone. The returned receipt is unreserved.
    pub fn derive(
        checked: &'s CheckedCanonicalKirAggregateSsaV18<'g>,
        input: &'s Inventory<'g>,
        output: &'s Inventory<'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirAggregateSsaStorageV18)> {
        resources::scoped(budget, |meter| {
            meter.reserve(headers()?)?;
            meter.work(8)?;
            if !std::ptr::eq(input.owner(), checked.input())
                || !std::ptr::eq(output.owner(), checked.output())
                || input.blocks().len() != output.blocks().len()
                || input.edges().len() != output.edges().len()
                || input.operations().len() != checked.witness().actions().len()
            {
                return Err(Error::Inconsistent(
                    "aggregate occurrence exact pair inventories",
                ));
            }
            let (mut operations, _) = meter.table(input.operations().len())?;
            for (before, after) in input.blocks().iter().zip(output.blocks()) {
                meter.work(8)?;
                if before.coordinate != after.coordinate
                    || before.operations.start != operations.len()
                {
                    return Err(Error::Inconsistent("aggregate occurrence block census"));
                }
                let prefix = usize::from(
                    before.coordinate.block == 0
                        && checked.witness().conditions()[before.coordinate.function.0 as usize]
                            .is_some(),
                );
                let mut next = add(after.operations.start, prefix)?;
                for input_index in before.operations.clone() {
                    meter.work(4)?;
                    let action = checked.witness().actions()[input_index];
                    let row = match action {
                        Action::Remove => None,
                        Action::Retain | Action::Copy(_) => {
                            let output = output
                                .operations()
                                .get(next)
                                .filter(|row| {
                                    next < after.operations.end
                                        && row.coordinate.block == after.coordinate
                                })
                                .ok_or(Error::Inconsistent("aggregate occurrence output extent"))?;
                            next = add(next, 1)?;
                            Some(CanonicalKirAggregateOperationTransportV30 {
                                output: output.coordinate,
                                retained: matches!(action, Action::Retain),
                            })
                        }
                    };
                    meter.push(&mut operations, row)?;
                }
                if next != after.operations.end {
                    return Err(Error::Inconsistent(
                        "aggregate occurrence complete output block",
                    ));
                }
            }
            let (mut definitions, _) = meter.table(input.definitions().len())?;
            for row in input.definitions() {
                meter.work(6)?;
                let function = match row.coordinate {
                    Definition::FunctionArgument { function, .. } => function,
                    Definition::BlockArgument { block, .. } => block.function,
                    Definition::Result { operation, .. } => operation.block.function,
                };
                let found = match row.value {
                    Some(value) => meter.derive(|budget| {
                        Ok(output.definition_index_for_value(function, value, budget)?)
                    })?,
                    None => {
                        let Definition::FunctionArgument { argument, .. } = row.coordinate else {
                            return Err(Error::Inconsistent(
                                "aggregate external declaration argument",
                            ));
                        };
                        let f = output
                            .functions()
                            .get(function.0 as usize)
                            .filter(|f| f.coordinate == function)
                            .ok_or(Error::Inconsistent(
                                "aggregate occurrence declaration function",
                            ))?;
                        let index = add(f.definitions.start, argument as usize)?;
                        if index >= f.definitions.end {
                            return Err(Error::Inconsistent(
                                "aggregate occurrence declaration argument",
                            ));
                        }
                        Some(index)
                    }
                };
                if let Some(index) = found {
                    let actual = output.definitions().get(index).ok_or(Error::Inconsistent(
                        "aggregate occurrence definition extent",
                    ))?;
                    if actual.value != row.value
                        || actual.ty != row.ty
                        || (row.value.is_none() && actual.coordinate != row.coordinate)
                    {
                        return Err(Error::Inconsistent(
                            "aggregate occurrence definition payload",
                        ));
                    }
                }
                meter.push(&mut definitions, found)?;
            }
            for (before, after) in input.edges().iter().zip(output.edges()) {
                meter.work(add(5, before.arguments.len())?)?;
                if before.coordinate != after.coordinate
                    || before.target != after.target
                    || before.target_id != after.target_id
                    || after.arguments.get(..before.arguments.len()) != Some(before.arguments)
                {
                    return Err(Error::Inconsistent(
                        "aggregate occurrence ordered edge prefix",
                    ));
                }
            }
            let retained = [size_of::<Self>(), bytes(&operations)?, bytes(&definitions)?]
                .into_iter()
                .try_fold(0, add)?;
            Ok((
                Self {
                    checked,
                    input,
                    output,
                    operations,
                    definitions,
                },
                CanonicalKirAggregateSsaStorageV18(retained),
            ))
        })
    }

    /// Returns the exact independently checked pair retained by this index.
    pub const fn checked(&self) -> &'s CheckedCanonicalKirAggregateSsaV18<'g> {
        self.checked
    }
    /// Returns the exact input inventory whose dense ordinals this index uses.
    pub const fn input(&self) -> &'s Inventory<'g> {
        self.input
    }
    /// Returns the exact output inventory, not an equal-byte replacement owner.
    pub const fn output(&self) -> &'s Inventory<'g> {
        self.output
    }
    /// Returns transport for an input operation ordinal; None denotes removal.
    pub fn operation(
        &self,
        input: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalKirAggregateOperationTransportV30>> {
        budget.charge_work(2)?;
        self.operations
            .get(input)
            .copied()
            .ok_or(Error::Inconsistent(
                "aggregate occurrence operation ordinal",
            ))
    }
    /// Returns the actual surviving definition, including a concrete copy
    /// replacing a private read. Removed allocation/projection values have None.
    pub fn definition(&self, input: usize, budget: &mut Budget<'_>) -> Result<Option<Definition>> {
        budget.charge_work(3)?;
        self.definitions
            .get(input)
            .ok_or(Error::Inconsistent(
                "aggregate occurrence definition ordinal",
            ))?
            .map(|output| {
                self.output
                    .definitions()
                    .get(output)
                    .map(|row| row.coordinate)
                    .ok_or(Error::Inconsistent(
                        "aggregate occurrence output definition ordinal",
                    ))
            })
            .transpose()
    }
    /// Returns an exact successor occurrence. Parallel edges remain distinct;
    /// appended SSA arguments never renumber the original argument prefix.
    pub fn edge(&self, input: usize, budget: &mut Budget<'_>) -> Result<Edge> {
        budget.charge_work(2)?;
        self.output
            .edges()
            .get(input)
            .map(|row| row.coordinate)
            .ok_or(Error::Inconsistent("aggregate occurrence edge ordinal"))
    }
    /// Always false: indexed occurrence transport grants no executable authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
