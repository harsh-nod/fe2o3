use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Meaning {
    Nominal(CallRole),
    Array([u8; 4]),
    Component(u8),
}
struct Resolver<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    relation: &'a Relation,
    visits: usize,
}
// Bound the logical live pieces at one recursive level. References borrow the
// retained owner; no SemanticRvalue/statement/graph is cloned into a frame.
const _: () = assert!(
    size_of::<Resolver<'static>>()
        + 3 * size_of::<Occurrences<'static>>()
        + 16 * size_of::<Meaning>()
        + 32 * size_of::<usize>()
        + 8 * size_of::<SsaValueV1>()
        <= FRAME_BYTES
);
impl Resolver<'_> {
    fn resolve(
        &mut self,
        function: SemanticFunctionIdV1,
        value: SsaValueV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> CallResult<Meaning> {
        budget.charge_work(16)?;
        if depth >= DEPTH || self.visits == 4096 {
            return refuse("nominal alias depth/work cap");
        }
        self.visits += 1;
        for role in CallRole::ALL {
            let row = self.relation.producers[role.index()];
            if row.function == function && row.value == value {
                return Ok(if role == CallRole::Values {
                    Meaning::Array([0, 1, 2, 3])
                } else {
                    Meaning::Nominal(role)
                });
            }
        }
        if function == self.relation.helper {
            for (index, formal) in self.relation.formals.iter().enumerate() {
                if *formal == value {
                    return Ok(Meaning::Nominal(
                        [
                            CallRole::Context,
                            CallRole::Lhs,
                            CallRole::Rhs,
                            CallRole::Zero,
                        ][index],
                    ));
                }
            }
        }
        let rows = source::rows(self.owner, function)?;
        match value {
            SsaValueV1::BlockArgument { block, variable } => {
                let plan = self
                    .owner
                    .plan_for_function(function)
                    .ok_or(CallError::Unavailable("actual SSA plan"))?;
                let mut incoming = None;
                for successor in rows.successors() {
                    budget.charge_work(1)?;
                    if successor.edge().target().index() != block.get() {
                        continue;
                    }
                    let arguments = plan
                        .plan()
                        .edge_arguments(successor.id())
                        .ok_or(CallError::Unavailable("actual SSA edge arguments"))?;
                    if arguments.len() > 4096 {
                        return refuse("edge argument cap");
                    }
                    let mut actual = None;
                    for argument in arguments {
                        budget.charge_work(1)?;
                        if argument.variable() == variable {
                            one(&mut actual, argument.value())?;
                        }
                    }
                    one(
                        &mut incoming,
                        actual.ok_or(CallError::Unavailable("missing edge variable"))?,
                    )?;
                }
                self.resolve(
                    function,
                    incoming.ok_or(CallError::Unavailable("missing SSA predecessor"))?,
                    depth + 1,
                    budget,
                )
            }
            SsaValueV1::Definition(_) => {
                let mut definition = None;
                for event in rows.events() {
                    budget.charge_work(1)?;
                    if let Some(SsaResolvedEventV1::Define { value: actual, .. }) = event.resolved()
                        && actual == value
                    {
                        if !event.is_reachable() || !event.is_promoted() {
                            return refuse("unavailable source definition");
                        }
                        one(&mut definition, event.site())?;
                    }
                }
                let Some(site @ Site::Statement { block, statement }) = definition else {
                    return refuse("origin is not admitted nominal producer/formal/assignment");
                };
                let statement = self.owner.source_semantic().functions()[function.index() as usize]
                    .blocks()
                    .get(block.get() as usize)
                    .and_then(|b| b.statements().get(statement as usize))
                    .ok_or(CallError::Unavailable("definition statement"))?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    return refuse("definition is not assignment");
                };
                if !assignment.destination().projections().is_empty() {
                    return refuse("projected destination");
                }
                self.rvalue(function, site, assignment.value().kind(), depth + 1, budget)
            }
        }
    }
    fn operand(
        &mut self,
        function: SemanticFunctionIdV1,
        site: Site,
        role: OperandRole,
        operand: &SemanticOperandV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> CallResult<Meaning> {
        let place = match operand {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => p,
            _ => return refuse("constant cannot construct nominal/component ancestry"),
        };
        let rows = source::rows(self.owner, function)?;
        let value = source::base_use(&rows, site, role, budget)?;
        let actual = self.resolve(function, value, depth, budget)?;
        if matches!(operand, SemanticOperandV1::Copy(_))
            && matches!(
                actual,
                Meaning::Nominal(CallRole::Lhs | CallRole::Rhs | CallRole::Zero | CallRole::Result)
            )
        {
            return refuse("nominal fragment copy unavailable");
        }
        match place.projections() {
            [] => Ok(actual),
            [projection] => {
                let index = component_index(projection.kind())?;
                let local = self.owner.source_semantic().functions()[function.index() as usize]
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or(CallError::Unavailable("array local"))?;
                source::array_type(self.owner, local.ty())?;
                let Meaning::Array(values) = actual else {
                    return refuse("component did not originate in values conversion");
                };
                values
                    .get(index)
                    .copied()
                    .map(Meaning::Component)
                    .ok_or(CallError::Unavailable("component range"))
            }
            _ => refuse("nested source projection"),
        }
    }
    fn rvalue(
        &mut self,
        function: SemanticFunctionIdV1,
        site: Site,
        value: &SemanticRvalueKindV1,
        depth: usize,
        budget: &mut Budget<'_>,
    ) -> CallResult<Meaning> {
        budget.charge_work(16)?;
        match value {
            SemanticRvalueKindV1::Use(operand) => self.operand(
                function,
                site,
                OperandRole::RvalueOperand(0),
                operand,
                depth,
                budget,
            ),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } if place.projections().is_empty() => {
                let rows = source::rows(self.owner, function)?;
                budget.charge_work(rows.elisions().len())?;
                if rows.elisions().contains(&site) {
                    return refuse("elided borrow not an SSA source use");
                }
                let input = source::base_use(&rows, site, OperandRole::RvaluePlace, budget)?;
                let meaning = self.resolve(function, input, depth, budget)?;
                if !matches!(
                    meaning,
                    Meaning::Nominal(CallRole::Context | CallRole::Lane)
                ) {
                    return refuse("shared borrow not a matrix context/current lane");
                }
                Ok(meaning)
            }
            SemanticRvalueKindV1::Aggregate(aggregate)
                if matches!(aggregate.kind(), SemanticAggregateKindV1::Array)
                    && aggregate.operands().len() == 4 =>
            {
                let mut values = [0; 4];
                for (index, operand) in aggregate.operands().iter().enumerate() {
                    let Meaning::Component(component) = self.operand(
                        function,
                        site,
                        OperandRole::RvalueOperand(index as u32),
                        operand,
                        depth,
                        budget,
                    )?
                    else {
                        return refuse("array reconstructs unrelated or nonscalar values");
                    };
                    values[index] = component;
                }
                Ok(Meaning::Array(values))
            }
            _ => refuse("computed/reconstructed nominal transport"),
        }
    }
    fn call_input(
        &mut self,
        row: Bf16CallInstanceProducerV1,
        index: usize,
        expected: CallRole,
        require_move: bool,
        budget: &mut Budget<'_>,
    ) -> CallResult<()> {
        let call = source::call_at(self.owner, row.function, row.block)
            .ok_or(CallError::Unavailable("producer call"))?;
        let operand = call
            .arguments()
            .get(index)
            .ok_or(CallError::Unavailable("producer input"))?;
        if !source::whole(operand)
            || (require_move && !matches!(operand, SemanticOperandV1::Move(_)))
        {
            return refuse("nominal terminal requires whole source operand/move");
        }
        let meaning = self.operand(
            row.function,
            source::site(row.block),
            OperandRole::CallArgument(index as u32),
            operand,
            0,
            budget,
        )?;
        if meaning != Meaning::Nominal(expected) {
            return refuse("actual nominal role substitution");
        }
        Ok(())
    }
}
pub(super) fn permutation(values: [u8; 4]) -> CallResult<[u8; 4]> {
    if values != [0, 1, 2, 3] && values != [1, 0, 2, 3] {
        return refuse("return is neither Identity nor Swap01");
    }
    Ok(values)
}
pub(super) fn component_index(kind: SemanticProjectionKindV1) -> CallResult<usize> {
    // The independently checked actual source array has length4. Retain the
    // MIR minimum-length constraint; do not mistake it for the array's length.
    match kind {
        SemanticProjectionKindV1::ConstantIndex {
            offset,
            minimum_length,
            from_end: false,
        } if offset < minimum_length && minimum_length <= 4 => Ok(offset as usize),
        _ => refuse("nonconstant/out-of-range source component projection"),
    }
}
pub(super) fn check(
    owner: &ProductionSemanticSsaOwnerV1,
    relation: &mut Relation,
    budget: &mut Budget<'_>,
) -> CallResult<()> {
    let mut resolver = Resolver {
        owner,
        relation,
        visits: 0,
    };
    for (index, expected) in [
        CallRole::Context,
        CallRole::Lhs,
        CallRole::Rhs,
        CallRole::Zero,
    ]
    .into_iter()
    .enumerate()
    {
        if resolver.resolve(relation.root, relation.arguments[index], 0, budget)?
            != Meaning::Nominal(expected)
        {
            return refuse("root call argument does not descend from actual producer");
        }
    }
    for role in [CallRole::Lhs, CallRole::Rhs] {
        resolver.call_input(
            relation.producers[role.index()],
            1,
            CallRole::Lane,
            false,
            budget,
        )?;
    }
    resolver.call_input(
        relation.producers[CallRole::Zero.index()],
        0,
        CallRole::Lane,
        false,
        budget,
    )?;
    for (index, expected) in [
        CallRole::Context,
        CallRole::Lhs,
        CallRole::Rhs,
        CallRole::Zero,
    ]
    .into_iter()
    .enumerate()
    {
        resolver.call_input(
            relation.producers[CallRole::Result.index()],
            index,
            expected,
            index != 0,
            budget,
        )?;
    }
    resolver.call_input(
        relation.producers[CallRole::Values.index()],
        0,
        CallRole::Result,
        true,
        budget,
    )?;
    // Validate the ENTIRE helper, not merely the backwards slice. One acyclic
    // unconditional path visits every block and the two actual calls in order.
    let helper = &owner.source_semantic().functions()[relation.helper.index() as usize];
    let mut next = helper.entry();
    let mut visited = 0u32;
    let mut calls = 0usize;
    let returned = loop {
        budget.charge_work(1)?;
        let index = next.index() as usize;
        if index >= helper.blocks().len() || index >= 32 || visited & (1 << index) != 0 {
            return refuse("helper path coordinate/cycle");
        }
        visited |= 1 << index;
        let block = &helper.blocks()[index];
        for (si, statement) in block.statements().iter().enumerate() {
            budget.charge_work(1)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty() {
                        return refuse("helper projected store unavailable");
                    }
                    resolver.rvalue(
                        relation.helper,
                        Site::Statement {
                            block: fe2o3_mir_model::SsaBlockIdV1::new(next.index()),
                            statement: si as u32,
                        },
                        assignment.value().kind(),
                        0,
                        budget,
                    )?;
                }
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
                _ => return refuse("helper statement outside nominal transport"),
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => next = edge.target(),
            SemanticTerminatorKindV1::Call(call) => {
                let expected = [CallRole::Result, CallRole::Values]
                    .get(calls)
                    .ok_or(CallError::Unavailable("extra helper call"))?;
                if relation.producers[expected.index()].block != next {
                    return refuse("helper calls are not actual MFMA then values conversion");
                }
                calls += 1;
                next = call
                    .destination()
                    .ok_or(CallError::Unavailable("helper call destination"))?
                    .edge()
                    .target();
            }
            SemanticTerminatorKindV1::Return if calls == 2 => {
                let rows = source::rows(owner, relation.helper)?;
                let value =
                    source::base_use(&rows, source::site(next), OperandRole::ReturnValue, budget)?;
                let Meaning::Array(values) = resolver.resolve(relation.helper, value, 0, budget)?
                else {
                    return refuse("helper return did not descend from conversion");
                };
                break values;
            }
            _ => return refuse("helper conditional/exceptional/extra terminal"),
        }
    };
    if visited.count_ones() as usize != helper.blocks().len() {
        return refuse("unreachable extra helper blocks");
    }
    relation.permutation = permutation(returned)?;
    Ok(())
}
