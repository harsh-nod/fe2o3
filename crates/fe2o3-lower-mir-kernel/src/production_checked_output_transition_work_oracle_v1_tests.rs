//! Independent fixture-only census for the selected-edge cache work change.
//! This does not call transition State, control-index, or budget helpers.
use fe2o3_kernel_ir::{
    BinaryOp, BlockId, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as Descendant, CanonicalKirOperationOriginV1 as Origin,
    CanonicalKirTransitionCandidateV1 as Candidate, CheckedBinaryOperator, ComparePredicate,
    Constant, FunctionBody, Module, Operation, OperationKind, ScalarType, Terminator, Type,
    ValueId,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub(super) enum Family {
    Licm,
    Preheaders,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Census {
    blocks: i64,
    conditionals: i64,
    switches: i64,
    conditional_phi_arguments: i64,
    rounds: i64,
}

impl Census {
    fn state_delta(self) -> i64 {
        let Self {
            blocks: b,
            conditionals: c,
            switches: s,
            conditional_phi_arguments: p,
            rounds: r,
        } = self;
        // New: B initialized slots, R refreshes of B blocks and C+S roots.
        // Old: 2C reachability roots and P phi roots each round, then 2C
        // final-control roots. Every selector in these fixtures has one root.
        b + r * (b + c + s) - r * (2 * c + p) - 2 * c
    }

    fn control_index_delta(self) -> i64 {
        // Index construction also retires C selected-successor queries and
        // 2C final executable-edge queries, each with one selector-root visit.
        self.state_delta() - 3 * self.conditionals
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplaySchedule {
    bc: Census,
    oi: Census,
}

impl ReplaySchedule {
    pub(super) fn complete_replay_delta(self) -> usize {
        // Two ordinary B/C checks, its control index, and one O/I check.
        usize::try_from(
            2 * self.bc.state_delta() + self.bc.control_index_delta() + self.oi.state_delta(),
        )
        .unwrap()
    }

    pub(super) fn map_refused_replay_delta(self) -> usize {
        // The P6 Map Storage denial precedes the O/I check.
        usize::try_from(2 * self.bc.state_delta() + self.bc.control_index_delta()).unwrap()
    }
}

fn expected(family: Family, mutation: bool) -> Census {
    let (blocks, conditionals, switches, conditional_phi_arguments, rounds) =
        match (family, mutation) {
            (Family::Preheaders, false) => (1, 0, 0, 0, 1),
            (Family::Licm, false) => (2, 1, 0, 0, 1),
            (Family::Preheaders, true) => (5, 1, 1, 0, 1),
            (Family::Licm, true) => (6, 2, 1, 0, 1),
        };
    Census {
        blocks,
        conditionals,
        switches,
        conditional_phi_arguments,
        rounds,
    }
}

fn edges(terminator: &Terminator) -> Vec<(BlockId, &[ValueId])> {
    match terminator {
        Terminator::Branch { target, arguments } => vec![(*target, arguments)],
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => vec![
            (*then_target, then_arguments),
            (*else_target, else_arguments),
        ],
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .iter()
            .map(|case| (case.target, case.arguments.as_slice()))
            .chain([(*default_target, default_arguments.as_slice())])
            .collect(),
        Terminator::Return { .. } => Vec::new(),
        other => panic!("fixture schedule does not cover {other:?}"),
    }
}

fn representative(parents: &BTreeMap<ValueId, ValueId>, mut value: ValueId) -> ValueId {
    for _ in 0..=parents.len() {
        let next = parents[&value];
        if next == value {
            return value;
        }
        value = next;
    }
    panic!("fixture equivalence cycle")
}

// Independent equality saturation over the actual edge payloads. There is no
// scalar folding in these fixture selectors. Original Rust SSA can already
// supply non-entry induction phis before private-cell promotion. Their distinct
// initial and increment values do not establish an alias in this census.
// Literal descendant aliases are counted separately from this phi census.
fn phi_rounds(body: &FunctionBody) -> usize {
    let mut parents = BTreeMap::new();
    for value in body
        .parameters
        .iter()
        .copied()
        .chain(body.blocks.iter().flat_map(|block| {
            block.parameters.iter().map(|value| value.id).chain(
                block
                    .operations
                    .iter()
                    .flat_map(|op| op.results.iter().map(|value| value.id)),
            )
        }))
    {
        parents.insert(value, value);
    }
    for round in 1..=parents.len() + 1 {
        let mut changed = false;
        for block in body.blocks.iter().skip(1) {
            for (ordinal, parameter) in block.parameters.iter().enumerate() {
                let current = representative(&parents, parameter.id);
                let mut candidates = BTreeSet::new();
                for predecessor in &body.blocks {
                    for (target, arguments) in edges(predecessor.terminator.as_ref().unwrap()) {
                        if target == block.id {
                            assert_eq!(arguments.len(), block.parameters.len());
                            let incoming = representative(&parents, arguments[ordinal]);
                            if incoming != current {
                                candidates.insert(incoming);
                            }
                        }
                    }
                }
                if candidates.len() == 1 {
                    let incoming = *candidates.first().unwrap();
                    parents.insert(current.max(incoming), current.min(incoming));
                    changed = true;
                }
            }
        }
        if !changed {
            return round;
        }
    }
    panic!("fixture phi equations did not converge")
}

fn census(module: &Module) -> Census {
    assert_eq!(module.functions.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    let mut result = Census {
        blocks: body.blocks.len().try_into().unwrap(),
        conditionals: 0,
        switches: 0,
        conditional_phi_arguments: 0,
        rounds: phi_rounds(body).try_into().unwrap(),
    };
    let mut reachable = BTreeSet::from([body.blocks[0].id]);
    loop {
        let before = reachable.len();
        for block in &body.blocks {
            if reachable.contains(&block.id) {
                for (target, _) in edges(block.terminator.as_ref().unwrap()) {
                    reachable.insert(target);
                }
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    assert_eq!(reachable.len(), body.blocks.len());
    for block in &body.blocks {
        let terminator = block.terminator.as_ref().unwrap();
        let selector = match terminator {
            Terminator::ConditionalBranch { condition, .. } => {
                result.conditionals += 1;
                for (target, arguments) in edges(terminator) {
                    let target = body.blocks.iter().find(|block| block.id == target).unwrap();
                    assert_eq!(arguments.len(), target.parameters.len());
                    result.conditional_phi_arguments +=
                        i64::try_from(target.parameters.len()).unwrap();
                }
                Some(*condition)
            }
            Terminator::Switch {
                selector, cases, ..
            } => {
                assert!(!cases.is_empty());
                result.switches += 1;
                Some(*selector)
            }
            _ => None,
        };
        if let Some(selector) = selector {
            // Selectors are original dynamic function parameters or their
            // directly defining nonconstant operation, never phi aliases.
            assert!(
                !body
                    .blocks
                    .iter()
                    .any(|block| block.parameters.iter().any(|p| p.id == selector))
            );
            if !body.parameters.contains(&selector) {
                let operation = body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|op| op.results.iter().any(|result| result.id == selector))
                    .unwrap();
                assert!(!matches!(operation.kind, OperationKind::Constant(_)));
            }
        }
    }
    result
}

fn definition_literal(module: &Module, definition: Definition) -> Option<(&Constant, &Type)> {
    let Definition::Result { operation, result } = definition else {
        return None;
    };
    let body = module
        .functions
        .get(operation.block.function.0 as usize)?
        .body
        .as_ref()?;
    let operation = body
        .blocks
        .get(operation.block.block as usize)?
        .operations
        .get(operation.operation as usize)?;
    let OperationKind::Constant(value) = &operation.kind else {
        return None;
    };
    Some((value, &operation.results.get(result as usize)?.ty))
}

// A fixture-local equality model, independent of transition State and its
// indexes. Literal facts are rediscovered from class members, not published by
// the production scalar evaluator. No candidate pair is an equality premise.
struct FixtureFacts<'a> {
    parents: BTreeMap<ValueId, ValueId>,
    types: BTreeMap<ValueId, &'a Type>,
    literals: BTreeMap<ValueId, &'a Constant>,
}

impl<'a> FixtureFacts<'a> {
    fn new(module: &'a Module) -> Result<Self, &'static str> {
        if module.functions.len() != 1 {
            return Err("fixture function count");
        }
        let function = &module.functions[0];
        let body = function.body.as_ref().ok_or("fixture declaration")?;
        if body.parameters.len() != function.signature.parameters.len() {
            return Err("fixture function arity");
        }
        let mut types = BTreeMap::new();
        for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
            if types.insert(*value, ty).is_some() {
                return Err("fixture duplicate value");
            }
        }
        let mut literals = BTreeMap::new();
        for block in &body.blocks {
            for value in block
                .parameters
                .iter()
                .chain(block.operations.iter().flat_map(|op| &op.results))
            {
                if types.insert(value.id, &value.ty).is_some() {
                    return Err("fixture duplicate value");
                }
            }
            for operation in &block.operations {
                if let OperationKind::Constant(value) = &operation.kind {
                    if operation.results.len() != 1 {
                        return Err("fixture constant arity");
                    }
                    literals.insert(operation.results[0].id, value);
                }
            }
        }
        Ok(Self {
            parents: types.keys().map(|value| (*value, *value)).collect(),
            types,
            literals,
        })
    }

    fn literal(&self, value: ValueId) -> Result<Option<(&'a Constant, &'a Type)>, &'static str> {
        if !self.parents.contains_key(&value) {
            return Err("fixture unknown value");
        }
        let root = representative(&self.parents, value);
        let mut found = None;
        for (member, literal) in &self.literals {
            if representative(&self.parents, *member) == root {
                let fact = (*literal, self.types[member]);
                if found.is_some_and(|prior| prior != fact) {
                    return Err("fixture conflicting literals");
                }
                found = Some(fact);
            }
        }
        Ok(found)
    }

    fn equal(&self, a: ValueId, b: ValueId) -> Result<bool, &'static str> {
        let literal_a = self.literal(a)?;
        let literal_b = self.literal(b)?;
        Ok(
            representative(&self.parents, a) == representative(&self.parents, b)
                || literal_a.is_some() && literal_a == literal_b,
        )
    }

    fn merge(&mut self, a: ValueId, b: ValueId) -> Result<bool, &'static str> {
        if self.types.get(&a) != self.types.get(&b) {
            return Err("fixture alias type");
        }
        let literal_a = self.literal(a)?;
        let literal_b = self.literal(b)?;
        if literal_a.is_some() && literal_b.is_some() && literal_a != literal_b {
            return Err("fixture unequal literal alias");
        }
        let a = representative(&self.parents, a);
        let b = representative(&self.parents, b);
        if a == b {
            return Ok(false);
        }
        self.parents.insert(a.max(b), a.min(b));
        Ok(true)
    }
}

// Ordinary MIR integer arithmetic is represented by a two-result checked
// operation even when its overflow result is dead. Prove that neither result
// acquires a scalar fact here; do not quietly treat it as a plain one-result Add.
fn fixture_checked_dynamic_binary(
    operation: &Operation,
    facts: &FixtureFacts<'_>,
    operator: CheckedBinaryOperator,
    left: ValueId,
    right: ValueId,
) -> Result<(), &'static str> {
    let ty = facts
        .types
        .get(&left)
        .copied()
        .ok_or("fixture unknown value")?;
    if !matches!(
        ty,
        Type::Scalar(
            ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
                | ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
        )
    ) || facts.types.get(&right).copied() != Some(ty)
        || operation.results.len() != 2
        || operation.results[0].ty != *ty
        || operation.results[1].ty != Type::BOOL
    {
        return Err("fixture checked integer shape");
    }
    let classify = |value| -> Result<Option<(bool, bool)>, &'static str> {
        let Some((literal, literal_type)) = facts.literal(value)? else {
            return Ok(None);
        };
        if literal_type != ty {
            return Err("fixture checked integer literal type");
        }
        let bits = match (ty, literal) {
            (Type::Scalar(ScalarType::U8), Constant::U8(value)) => *value as u64,
            (Type::Scalar(ScalarType::U16), Constant::U16(value)) => *value as u64,
            (Type::Scalar(ScalarType::U32), Constant::U32(value)) => *value as u64,
            (Type::Scalar(ScalarType::U64), Constant::U64(value)) => *value,
            (Type::Scalar(ScalarType::I8), Constant::I8(value)) => *value as u8 as u64,
            (Type::Scalar(ScalarType::I16), Constant::I16(value)) => *value as u16 as u64,
            (Type::Scalar(ScalarType::I32), Constant::I32(value)) => *value as u32 as u64,
            (Type::Scalar(ScalarType::I64), Constant::I64(value)) => *value as u64,
            _ => return Err("fixture checked integer literal type"),
        };
        Ok(Some((bits == 0, bits == 1)))
    };
    let a = classify(left)?;
    let b = classify(right)?;
    let zero = |value: Option<(bool, bool)>| value.is_some_and(|value| value.0);
    let one = |value: Option<(bool, bool)>| value.is_some_and(|value| value.1);
    // At least one operand remains unknown. Refuse every neutral-operand alias,
    // plus the zero/self cases whose mathematically fixed results would need
    // an actual scalar-fact extension. Overflow is not assumed to be false.
    let dependency = a.is_some() && b.is_some()
        || match operator {
            CheckedBinaryOperator::Add => zero(a) || zero(b),
            CheckedBinaryOperator::Subtract => zero(b) || facts.equal(left, right)?,
            CheckedBinaryOperator::Multiply => zero(a) || zero(b) || one(a) || one(b),
        };
    if dependency {
        return Err("fixture scalar alias or folding dependency");
    }
    Ok(())
}

fn fixture_scalar_grammar(
    body: &FunctionBody,
    facts: &FixtureFacts<'_>,
) -> Result<(), &'static str> {
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        match &operation.kind {
            OperationKind::Binary {
                op: BinaryOp::Checked(operator),
                lhs,
                rhs,
            } => {
                fixture_checked_dynamic_binary(operation, facts, *operator, *lhs, *rhs)?;
            }
            OperationKind::Binary {
                op,
                lhs: left,
                rhs: right,
            } => {
                let a = facts.literal(*left)?.map(|fact| fact.0);
                let b = facts.literal(*right)?.map(|fact| fact.0);
                // Actual loop increment and parity mask only; neither is an
                // integer identity or a constant-folding expression.
                if !matches!(op, BinaryOp::Add | BinaryOp::BitAnd)
                    || !matches!(
                        (a, b),
                        (None, Some(Constant::U32(1))) | (Some(Constant::U32(1)), None)
                    )
                {
                    return Err("fixture scalar alias or folding dependency");
                }
            }
            OperationKind::Compare {
                predicate,
                lhs: left,
                rhs: right,
            } => {
                let a = facts.literal(*left)?.map(|fact| fact.0);
                let b = facts.literal(*right)?.map(|fact| fact.0);
                if !matches!(
                    (predicate, a, b),
                    (ComparePredicate::Equal, None, Some(Constant::U32(0)))
                        | (ComparePredicate::Equal, Some(Constant::U32(0)), None)
                        | (ComparePredicate::LessThan, None, Some(Constant::U32(3)))
                ) {
                    return Err("fixture scalar comparison dependency");
                }
            }
            OperationKind::Unary { .. }
            | OperationKind::Cast { .. }
            | OperationKind::Select { .. } => {
                return Err("fixture scalar chain dependency");
            }
            OperationKind::Constant(_)
            | OperationKind::Alloca { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::GetElementPointer { .. }
            | OperationKind::Load { .. }
            | OperationKind::Store { .. } => (),
            _ => return Err("fixture operation outside reviewed grammar"),
        }
    }
    for block in &body.blocks {
        let selector = match block
            .terminator
            .as_ref()
            .ok_or("fixture missing terminator")?
        {
            Terminator::ConditionalBranch { condition, .. } => Some(*condition),
            Terminator::Switch { selector, .. } => Some(*selector),
            Terminator::Branch { .. } | Terminator::Return { .. } => None,
            _ => return Err("fixture terminator outside reviewed grammar"),
        };
        if let Some(selector) = selector {
            if facts.literal(selector)?.is_some() {
                return Err("fixture selected-edge dependency");
            }
        }
    }
    Ok(())
}

fn fixture_definition_value(module: &Module, definition: Definition) -> Option<ValueId> {
    match definition {
        Definition::FunctionArgument { function, argument } => module
            .functions
            .get(function.0 as usize)?
            .body
            .as_ref()?
            .parameters
            .get(argument as usize)
            .copied(),
        Definition::BlockArgument { block, argument } => Some(
            module
                .functions
                .get(block.function.0 as usize)?
                .body
                .as_ref()?
                .blocks
                .get(block.block as usize)?
                .parameters
                .get(argument as usize)?
                .id,
        ),
        Definition::Result { operation, result } => Some(
            module
                .functions
                .get(operation.block.function.0 as usize)?
                .body
                .as_ref()?
                .blocks
                .get(operation.block.block as usize)?
                .operations
                .get(operation.operation as usize)?
                .results
                .get(result as usize)?
                .id,
        ),
    }
}

// Only equality-preserving phis and independently equal typed literals can
// change these fixtures. Check the scalar/selector grammar after every sweep:
// a phi-derived constant used by arithmetic or control is outside this oracle.
fn literal_alias_rounds(module: &Module, rows: Candidate<'_>) -> Result<i64, &'static str> {
    let mut facts = FixtureFacts::new(module)?;
    let body = module.functions[0].body.as_ref().unwrap();
    let entry = body.blocks.first().ok_or("fixture missing entry")?.id;
    fixture_scalar_grammar(body, &facts)?;
    let incoming: Vec<_> = body
        .blocks
        .iter()
        .flat_map(|block| {
            edges(block.terminator.as_ref().unwrap())
                .into_iter()
                .map(move |(target, values)| (block.id, target, values))
        })
        .collect();
    let mut reachable = BTreeSet::from([entry]);
    loop {
        let before = reachable.len();
        for (source, target, values) in &incoming {
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == *target)
                .ok_or("fixture edge target")?;
            if values.len() != block.parameters.len() {
                return Err("fixture phi arity");
            }
            for (value, parameter) in values.iter().zip(&block.parameters) {
                if facts.types.get(value).copied() != Some(&parameter.ty) {
                    return Err("fixture edge type");
                }
            }
            if reachable.contains(source) {
                reachable.insert(*target);
            }
        }
        if before == reachable.len() {
            break;
        }
    }
    if reachable.len() != body.blocks.len() {
        return Err("fixture unreachable block");
    }
    let mut anchors = BTreeMap::new();
    for row in rows.definitions {
        let start = row.outputs.start as usize;
        let end = start
            .checked_add(row.outputs.len as usize)
            .ok_or("fixture row range")?;
        for descendant in rows
            .definition_outputs
            .get(start..end)
            .ok_or("fixture row range")?
        {
            if descendant.kind == Descendant::Retained {
                if anchors.insert(descendant.output, row.input).is_some() {
                    return Err("fixture duplicate anchor");
                }
            }
        }
    }
    for row in rows.operations {
        if let Origin::ConstantFrom(source) = row.origin {
            if definition_literal(module, source).is_none() {
                return Err("fixture synthesized nonliteral");
            }
            let target = Definition::Result {
                operation: row.output,
                result: 0,
            };
            if anchors.insert(target, source).is_some() {
                return Err("fixture duplicate anchor");
            }
        }
    }
    let mut aliases = Vec::new();
    for row in rows.definitions {
        let start = row.outputs.start as usize;
        let end = start
            .checked_add(row.outputs.len as usize)
            .ok_or("fixture row range")?;
        for descendant in rows
            .definition_outputs
            .get(start..end)
            .ok_or("fixture row range")?
        {
            let anchor = *anchors
                .get(&descendant.output)
                .ok_or("fixture missing anchor")?;
            if row.input != anchor {
                let source = fixture_definition_value(module, row.input)
                    .ok_or("fixture nonliteral alias")?;
                let target =
                    fixture_definition_value(module, anchor).ok_or("fixture nonliteral alias")?;
                aliases.push((source, target));
            }
        }
    }
    for round in 1..=facts.parents.len() + 1 {
        let mut changed = false;
        for block in body.blocks.iter().skip(1) {
            for (ordinal, parameter) in block.parameters.iter().enumerate() {
                let mut selected = None;
                let mut compatible = true;
                // Incoming lists are prepend-built from lexical edge order.
                // Keep this order explicit, not inferred from value numbers.
                for (_, target, values) in incoming.iter().rev() {
                    if *target != block.id {
                        continue;
                    }
                    let value = values[ordinal];
                    if representative(&facts.parents, value)
                        == representative(&facts.parents, parameter.id)
                    {
                        continue;
                    }
                    if let Some(previous) = selected {
                        compatible &= facts.equal(previous, value)?;
                    } else {
                        selected = Some(value);
                    }
                }
                if compatible {
                    if let Some(value) = selected {
                        changed |= facts.merge(parameter.id, value)?;
                    }
                }
            }
        }
        fixture_scalar_grammar(body, &facts)?;
        for (source, target) in &aliases {
            if facts.equal(*source, *target)? {
                changed |= facts.merge(*source, *target)?;
            } else if facts.literal(*source)?.is_some() && facts.literal(*target)?.is_some() {
                return Err("fixture unequal literal alias");
            }
        }
        if !changed {
            for (source, target) in &aliases {
                if !facts.equal(*source, *target)? {
                    return Err("fixture nonliteral alias");
                }
            }
            return i64::try_from(round).map_err(|_| "fixture round overflow");
        }
    }
    Err("fixture phi equations did not converge")
}

pub(super) fn assert_policy6_endpoints(
    owner: &crate::ProductionCheckedOutputOwnerPolicy6V1,
    family: Family,
    mutation: bool,
) -> ReplaySchedule {
    let fifth = owner.checked_output().intermediate_policy5();
    let third = fifth.intermediate_policy4().intermediate_policy3();
    for (name, module) in [
        ("B", owner.bound().module()),
        ("C", third.owner().module()),
        ("O", fifth.owner().module()),
        ("I", owner.output().module()),
    ] {
        assert_eq!(
            census(module),
            expected(family, mutation),
            "{family:?}/{mutation}/{name}"
        );
    }
    let mut bc = expected(family, mutation);
    bc.rounds = literal_alias_rounds(owner.bound().module(), third.occurrences().candidate())
        .unwrap_or_else(|error| {
            panic!(
                "B/C fixture dependency refusal: {error}\n{:#?}",
                owner.bound().module()
            )
        });
    let mut oi = expected(family, mutation);
    oi.rounds = literal_alias_rounds(
        fifth.owner().module(),
        owner
            .checked_output()
            .continuation()
            .occurrences()
            .candidate(),
    )
    .unwrap_or_else(|error| {
        panic!(
            "O/I fixture dependency refusal: {error}\n{:#?}",
            fifth.owner().module()
        )
    });
    ReplaySchedule { bc, oi }
}

#[test]
fn transition_cache_fixture_work_equations_are_literal_and_phase_sensitive() {
    for (family, mutation, d, index, complete, partial) in [
        (Family::Preheaders, false, 2, 2, 8, 6),
        (Family::Licm, false, 1, -2, 1, 0),
        (Family::Preheaders, true, 8, 5, 29, 21),
        (Family::Licm, true, 7, 1, 22, 15),
    ] {
        let census = expected(family, mutation);
        assert_eq!(census.state_delta(), d);
        assert_eq!(census.control_index_delta(), index);
        let schedule = ReplaySchedule {
            bc: census,
            oi: census,
        };
        assert_eq!(schedule.complete_replay_delta(), complete);
        assert_eq!(schedule.map_refused_replay_delta(), partial);
    }
    assert_eq!(249_631 + 3 * 1, 249_634);
    assert_eq!(238_296 + 2 * 1 + 0, 238_298);
    assert_eq!(65_552 + 8 + 6, 65_566);
    // BC and OI have independent literal-merge stabilization schedules.
    for (family, bc_rounds, oi_rounds, complete, partial) in [
        (Family::Preheaders, 1, 1, 29, 21),
        (Family::Preheaders, 1, 2, 34, 21),
        (Family::Preheaders, 2, 1, 44, 36),
        (Family::Preheaders, 2, 2, 49, 36),
        (Family::Licm, 1, 1, 22, 15),
        (Family::Licm, 1, 2, 27, 15),
        (Family::Licm, 2, 1, 37, 30),
        (Family::Licm, 2, 2, 42, 30),
    ] {
        let mut bc = expected(family, true);
        let mut oi = bc;
        bc.rounds = bc_rounds;
        oi.rounds = oi_rounds;
        let schedule = ReplaySchedule { bc, oi };
        assert_eq!(schedule.complete_replay_delta(), complete);
        assert_eq!(schedule.map_refused_replay_delta(), partial);
    }
}

#[test]
fn transition_cache_fixture_literal_alias_rounds_are_exact_and_refuse_other_dependencies() {
    use fe2o3_kernel_ir::{
        BasicBlock, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirDefinitionDescendantV1 as Output, CanonicalKirDefinitionTransitionV1 as Row,
        CanonicalKirFunctionCoordinateV1 as FunctionId,
        CanonicalKirOperationCoordinateV1 as OperationId, CanonicalKirTransitionRangeV1 as Range,
        Function, Operation, ScalarType, Signature, ValueDef,
    };
    fn candidate<'a>(definitions: &'a [Row], outputs: &'a [Output]) -> Candidate<'a> {
        Candidate {
            functions: &[],
            blocks: &[],
            segments: &[],
            operations: &[],
            definitions,
            definition_outputs: outputs,
            uses: &[],
            edges: &[],
            edge_arguments: &[],
        }
    }
    let definition = |operation| Definition::Result {
        operation: OperationId {
            block: Block {
                function: FunctionId(0),
                block: 0,
            },
            operation,
        },
        result: 0,
    };
    let mut block = BasicBlock::new(BlockId(0));
    for id in 0..2 {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("literal_round_control");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut definitions = [
        Row {
            input: definition(0),
            outputs: Range { start: 0, len: 1 },
        },
        Row {
            input: definition(1),
            outputs: Range { start: 1, len: 1 },
        },
    ];
    let mut outputs = [
        Output {
            output: definition(0),
            kind: Descendant::Retained,
        },
        Output {
            output: definition(1),
            kind: Descendant::Retained,
        },
    ];
    // These are inert equation controls, not admitted transition owners.
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(1)
    );
    outputs[1] = Output {
        output: definition(0),
        kind: Descendant::Substituted,
    };
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(2)
    );
    definitions[1].input = Definition::FunctionArgument {
        function: FunctionId(0),
        argument: 0,
    };
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture nonliteral alias")
    );
    definitions[1].input = definition(1);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Constant(Constant::U32(8));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture unequal literal alias")
    );
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        OperationKind::Constant(Constant::U32(7));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture scalar alias or folding dependency")
    );
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "unreviewed_call".into(),
                arguments: vec![],
            },
        ));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture operation outside reviewed grammar")
    );
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .pop();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .parameters
        .push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)));
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(2),
        "initial entry parameters do not add a phi round"
    );
    outputs[1] = Output {
        output: definition(1),
        kind: Descendant::Retained,
    };
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Ok(1),
        "initial entry parameters do not require any merge sweep"
    );
    let mut phi = BasicBlock::new(BlockId(1));
    phi.parameters
        .push(ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)));
    phi.terminator = Some(Terminator::Return { values: vec![] });
    module.functions[0].body.as_mut().unwrap().blocks.push(phi);
    assert_eq!(
        literal_alias_rounds(&module, candidate(&definitions, &outputs)),
        Err("fixture unreachable block")
    );
}

#[test]
fn transition_cache_fixture_rounds_follow_phi_dependency_order_not_block_count() {
    use fe2o3_kernel_ir::{BasicBlock, ScalarType, Type, ValueDef};
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)));
    header.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![ValueId(1)],
    });
    let mut latch = BasicBlock::new(BlockId(2));
    latch
        .parameters
        .push(ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)));
    latch.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    // This is only a control for the independent census, not an admitted owner.
    let mut body = FunctionBody {
        parameters: vec![ValueId(0)],
        blocks: vec![entry, header, latch],
    };
    assert_eq!(phi_rounds(&body), 3);
    body.blocks.swap(1, 2);
    assert_eq!(phi_rounds(&body), 2);
}

mod phi_controls {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirDefinitionDescendantV1 as Output, CanonicalKirDefinitionTransitionV1 as Row,
        CanonicalKirFunctionCoordinateV1 as FunctionId,
        CanonicalKirOperationCoordinateV1 as OperationId, CanonicalKirTransitionRangeV1 as Range,
        Function, Operation, ScalarType, Signature, SwitchCase, ValueDef,
    };

    fn ty() -> Type {
        Type::Scalar(ScalarType::U32)
    }
    fn parameter(value: u32) -> ValueDef {
        ValueDef::new(ValueId(value), ty())
    }
    fn constant(value: u32, bits: u32) -> Operation {
        Operation::effect_free(
            parameter(value),
            OperationKind::Constant(Constant::U32(bits)),
        )
    }
    fn jump(target: u32, values: &[u32]) -> Terminator {
        Terminator::Branch {
            target: BlockId(target),
            arguments: values.iter().copied().map(ValueId).collect(),
        }
    }
    fn block(
        id: u32,
        parameters: &[u32],
        operations: Vec<Operation>,
        terminator: Terminator,
    ) -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(id));
        block.parameters = parameters.iter().copied().map(parameter).collect();
        block.operations = operations;
        block.terminator = Some(terminator);
        block
    }
    fn module(parameters: Vec<Type>, values: Vec<ValueId>, blocks: Vec<BasicBlock>) -> Module {
        let mut module = Module::new("literal_phi_round_control");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(parameters, vec![]),
            values,
            blocks,
        ));
        module
    }
    fn candidate<'a>(definitions: &'a [Row], outputs: &'a [Output]) -> Candidate<'a> {
        Candidate {
            functions: &[],
            blocks: &[],
            segments: &[],
            operations: &[],
            definitions,
            definition_outputs: outputs,
            uses: &[],
            edges: &[],
            edge_arguments: &[],
        }
    }
    fn literal_alias() -> ([Row; 2], [Output; 2]) {
        let definition = |operation| Definition::Result {
            operation: OperationId {
                block: Block {
                    function: FunctionId(0),
                    block: 0,
                },
                operation,
            },
            result: 0,
        };
        (
            [
                Row {
                    input: definition(0),
                    outputs: Range { start: 0, len: 1 },
                },
                Row {
                    input: definition(1),
                    outputs: Range { start: 1, len: 1 },
                },
            ],
            [
                Output {
                    output: definition(0),
                    kind: Descendant::Retained,
                },
                Output {
                    output: definition(0),
                    kind: Descendant::Substituted,
                },
            ],
        )
    }

    #[test]
    fn induction_phi_keeps_distinct_initial_and_increment_values_with_literal_aliases() {
        let mut module = module(
            vec![ty()],
            vec![ValueId(0)],
            vec![
                block(
                    0,
                    &[],
                    vec![constant(1, 0), constant(8, 0)],
                    Terminator::Switch {
                        selector: ValueId(0),
                        cases: vec![SwitchCase {
                            value: 0,
                            target: BlockId(2),
                            arguments: vec![ValueId(1)],
                        }],
                        default_target: BlockId(1),
                        default_arguments: vec![],
                    },
                ),
                block(1, &[], vec![constant(2, 1)], jump(2, &[2])),
                block(
                    2,
                    &[3],
                    vec![
                        constant(4, 3),
                        Operation::effect_free(
                            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::Bool)),
                            OperationKind::Compare {
                                predicate: ComparePredicate::LessThan,
                                lhs: ValueId(3),
                                rhs: ValueId(4),
                            },
                        ),
                    ],
                    Terminator::ConditionalBranch {
                        condition: ValueId(5),
                        then_target: BlockId(3),
                        then_arguments: vec![],
                        else_target: BlockId(4),
                        else_arguments: vec![],
                    },
                ),
                block(
                    3,
                    &[],
                    vec![
                        constant(6, 1),
                        Operation::effect_free(
                            parameter(7),
                            OperationKind::Binary {
                                op: BinaryOp::Add,
                                lhs: ValueId(3),
                                rhs: ValueId(6),
                            },
                        ),
                    ],
                    jump(2, &[7]),
                ),
                block(4, &[], vec![], Terminator::Return { values: vec![] }),
            ],
        );
        assert_eq!(literal_alias_rounds(&module, candidate(&[], &[])), Ok(1));
        let (rows, outputs) = literal_alias();
        assert_eq!(
            literal_alias_rounds(&module, candidate(&rows, &outputs)),
            Ok(2)
        );
        // The genuine semantic Add producer uses checked(value, overflow).
        module.functions[0].body.as_mut().unwrap().blocks[3].operations[1] =
            Operation::checked_binary(
                parameter(7),
                ValueDef::new(ValueId(9), Type::BOOL),
                CheckedBinaryOperator::Add,
                ValueId(3),
                ValueId(6),
            );
        assert_eq!(literal_alias_rounds(&module, candidate(&[], &[])), Ok(1));
        assert_eq!(
            literal_alias_rounds(&module, candidate(&rows, &outputs)),
            Ok(2)
        );
    }

    fn checked_fixture(
        ty: Type,
        literal: Constant,
        operator: CheckedBinaryOperator,
        swap: bool,
    ) -> Module {
        let (left, right) = if swap {
            (ValueId(1), ValueId(0))
        } else {
            (ValueId(0), ValueId(1))
        };
        module(
            vec![ty.clone()],
            vec![ValueId(0)],
            vec![block(
                0,
                &[],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(1), ty.clone()),
                        OperationKind::Constant(literal),
                    ),
                    Operation::checked_binary(
                        ValueDef::new(ValueId(2), ty),
                        ValueDef::new(ValueId(3), Type::BOOL),
                        operator,
                        left,
                        right,
                    ),
                ],
                Terminator::Return { values: vec![] },
            )],
        )
    }

    #[test]
    fn checked_dynamic_arithmetic_is_typed_width_general_and_adds_no_scalar_rounds() {
        for (scalar, literal) in [
            (ScalarType::U8, Constant::U8(2)),
            (ScalarType::U16, Constant::U16(2)),
            (ScalarType::U32, Constant::U32(2)),
            (ScalarType::U64, Constant::U64(2)),
            (ScalarType::I8, Constant::I8(-2)),
            (ScalarType::I16, Constant::I16(-2)),
            (ScalarType::I32, Constant::I32(-2)),
            (ScalarType::I64, Constant::I64(-2)),
        ] {
            for operator in [
                CheckedBinaryOperator::Add,
                CheckedBinaryOperator::Subtract,
                CheckedBinaryOperator::Multiply,
            ] {
                for swap in [false, true] {
                    let source =
                        checked_fixture(Type::Scalar(scalar), literal.clone(), operator, swap);
                    assert_eq!(literal_alias_rounds(&source, candidate(&[], &[])), Ok(1));
                }
            }
        }
    }

    #[test]
    fn checked_neutral_constant_and_phi_derived_dependencies_are_not_silently_admitted() {
        for (operator, literal, swaps) in [
            (CheckedBinaryOperator::Add, 0, &[false, true][..]),
            (CheckedBinaryOperator::Subtract, 0, &[false][..]),
            (CheckedBinaryOperator::Multiply, 0, &[false, true][..]),
            (CheckedBinaryOperator::Multiply, 1, &[false, true][..]),
        ] {
            for swap in swaps {
                let source = checked_fixture(ty(), Constant::U32(literal), operator, *swap);
                assert_eq!(
                    literal_alias_rounds(&source, candidate(&[], &[])),
                    Err("fixture scalar alias or folding dependency")
                );
            }
        }
        let source = checked_fixture(
            ty(),
            Constant::U32(0),
            CheckedBinaryOperator::Subtract,
            true,
        );
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Ok(1),
            "zero minus an unknown is not an identity"
        );
        let mut source = checked_fixture(ty(), Constant::U32(1), CheckedBinaryOperator::Add, false);
        source.functions[0].signature.parameters.clear();
        source.functions[0]
            .body
            .as_mut()
            .unwrap()
            .parameters
            .clear();
        source.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .insert(0, constant(0, u32::MAX));
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture scalar alias or folding dependency"),
            "checked wrap and overflow require real scalar facts"
        );
        let source = module(
            vec![],
            vec![],
            vec![
                block(0, &[], vec![constant(0, 0)], jump(1, &[0])),
                block(
                    1,
                    &[1],
                    vec![
                        constant(2, 1),
                        Operation::checked_binary(
                            parameter(3),
                            ValueDef::new(ValueId(4), Type::BOOL),
                            CheckedBinaryOperator::Add,
                            ValueId(1),
                            ValueId(2),
                        ),
                    ],
                    Terminator::Return { values: vec![] },
                ),
            ],
        );
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture scalar alias or folding dependency"),
            "recheck after phi discovery, not only at entrance"
        );
    }

    #[test]
    fn checked_result_shapes_literal_types_and_overflow_candidates_are_independent_obligations() {
        for fault in 0..5 {
            let mut source =
                checked_fixture(ty(), Constant::U32(1), CheckedBinaryOperator::Add, false);
            let operation = &mut source.functions[0].body.as_mut().unwrap().blocks[0].operations[1];
            match fault {
                0 => {
                    operation.results.pop();
                }
                1 => {
                    operation
                        .results
                        .push(ValueDef::new(ValueId(4), Type::BOOL));
                }
                2 => operation.results[0].ty = Type::Scalar(ScalarType::I32),
                3 => operation.results[1].ty = ty(),
                _ => source.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::I32),
            }
            assert_eq!(
                literal_alias_rounds(&source, candidate(&[], &[])),
                Err("fixture checked integer shape")
            );
        }
        let source = checked_fixture(ty(), Constant::I32(1), CheckedBinaryOperator::Add, false);
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture checked integer literal type")
        );
        let mut source = checked_fixture(ty(), Constant::U32(1), CheckedBinaryOperator::Add, false);
        source.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(4), Type::BOOL),
                OperationKind::Constant(Constant::Bool(false)),
            ));
        let result = |operation, result| Definition::Result {
            operation: OperationId {
                block: Block {
                    function: FunctionId(0),
                    block: 0,
                },
                operation,
            },
            result,
        };
        let rows = [
            Row {
                input: result(1, 1),
                outputs: Range { start: 0, len: 1 },
            },
            Row {
                input: result(2, 0),
                outputs: Range { start: 1, len: 1 },
            },
        ];
        let outputs = [
            Output {
                output: result(2, 0),
                kind: Descendant::Substituted,
            },
            Output {
                output: result(2, 0),
                kind: Descendant::Retained,
            },
        ];
        assert_eq!(
            literal_alias_rounds(&source, candidate(&rows, &outputs)),
            Err("fixture nonliteral alias"),
            "candidate cannot declare unknown overflow false"
        );
    }

    #[test]
    fn phi_cycles_follow_scan_order_for_dynamic_and_literal_roots_without_candidate_premises() {
        for literal in [false, true] {
            let mut module = module(
                if literal { vec![] } else { vec![ty()] },
                if literal { vec![] } else { vec![ValueId(0)] },
                vec![
                    block(
                        0,
                        &[],
                        if literal {
                            vec![constant(0, 7)]
                        } else {
                            vec![]
                        },
                        jump(1, &[0]),
                    ),
                    block(1, &[1], vec![], jump(2, &[1])),
                    block(2, &[2], vec![], jump(1, &[2])),
                ],
            );
            assert_eq!(literal_alias_rounds(&module, candidate(&[], &[])), Ok(3));
            module.functions[0].body.as_mut().unwrap().blocks.swap(1, 2);
            assert_eq!(literal_alias_rounds(&module, candidate(&[], &[])), Ok(2));
        }
        let source = module(
            vec![ty(), ty()],
            vec![ValueId(0), ValueId(1)],
            vec![
                block(0, &[], vec![], jump(1, &[0])),
                block(1, &[2], vec![], Terminator::Return { values: vec![] }),
            ],
        );
        let argument = |argument| Definition::FunctionArgument {
            function: FunctionId(0),
            argument,
        };
        let mut rows = [
            Row {
                input: argument(0),
                outputs: Range { start: 0, len: 1 },
            },
            Row {
                input: Definition::BlockArgument {
                    block: Block {
                        function: FunctionId(0),
                        block: 1,
                    },
                    argument: 0,
                },
                outputs: Range { start: 1, len: 1 },
            },
        ];
        let mut outputs = [
            Output {
                output: argument(0),
                kind: Descendant::Retained,
            },
            Output {
                output: argument(0),
                kind: Descendant::Substituted,
            },
        ];
        assert_eq!(
            literal_alias_rounds(&source, candidate(&rows, &outputs)),
            Ok(2)
        );
        rows[0].input = argument(1);
        outputs[0].output = argument(1);
        outputs[1].output = argument(1);
        assert_eq!(
            literal_alias_rounds(&source, candidate(&rows, &outputs)),
            Err("fixture nonliteral alias")
        );
    }

    #[test]
    fn equal_typed_literal_edges_and_descendants_share_one_ordered_fixed_point() {
        let mut module = module(
            vec![Type::Scalar(ScalarType::Bool)],
            vec![ValueId(0)],
            vec![
                block(
                    0,
                    &[],
                    vec![constant(1, 7), constant(2, 7)],
                    Terminator::ConditionalBranch {
                        condition: ValueId(0),
                        then_target: BlockId(1),
                        then_arguments: vec![ValueId(1)],
                        else_target: BlockId(1),
                        else_arguments: vec![ValueId(2)],
                    },
                ),
                block(1, &[3], vec![], Terminator::Return { values: vec![] }),
            ],
        );
        assert_eq!(literal_alias_rounds(&module, candidate(&[], &[])), Ok(3));
        let (rows, outputs) = literal_alias();
        assert_eq!(
            literal_alias_rounds(&module, candidate(&rows, &outputs)),
            Ok(2)
        );
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
            OperationKind::Constant(Constant::U32(8));
        assert_eq!(literal_alias_rounds(&module, candidate(&[], &[])), Ok(1));
        assert_eq!(
            literal_alias_rounds(&module, candidate(&rows, &outputs)),
            Err("fixture unequal literal alias")
        );
    }

    #[test]
    fn phi_induced_scalar_control_and_malformed_edge_dependencies_stay_refused() {
        let entry_backedge = module(
            vec![],
            vec![],
            vec![block(0, &[0], vec![constant(1, 7)], jump(0, &[1]))],
        );
        assert_eq!(
            literal_alias_rounds(&entry_backedge, candidate(&[], &[])),
            Ok(1),
            "an initial entry parameter is not established by its backedge"
        );
        let mut source = module(
            vec![],
            vec![],
            vec![
                block(0, &[], vec![constant(0, 0)], jump(1, &[0])),
                block(1, &[1], vec![], Terminator::Return { values: vec![] }),
            ],
        );
        assert_eq!(literal_alias_rounds(&source, candidate(&[], &[])), Ok(2));
        source.functions[0].body.as_mut().unwrap().blocks[1].operations = vec![
            constant(2, 1),
            Operation::effect_free(
                parameter(3),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(1),
                    rhs: ValueId(2),
                },
            ),
        ];
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture scalar alias or folding dependency")
        );
        source.functions[0].body.as_mut().unwrap().blocks[1]
            .operations
            .clear();
        source.functions[0].body.as_mut().unwrap().blocks[1].terminator =
            Some(Terminator::Switch {
                selector: ValueId(1),
                cases: vec![SwitchCase {
                    value: 0,
                    target: BlockId(2),
                    arguments: vec![],
                }],
                default_target: BlockId(2),
                default_arguments: vec![],
            });
        source.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks
            .push(block(2, &[], vec![], Terminator::Return { values: vec![] }));
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture selected-edge dependency")
        );
        source.functions[0].body.as_mut().unwrap().blocks.pop();
        source.functions[0].body.as_mut().unwrap().blocks[1].terminator =
            Some(Terminator::Return { values: vec![] });
        source.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(jump(1, &[]));
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture phi arity")
        );
        source.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(jump(1, &[0]));
        source.functions[0].body.as_mut().unwrap().blocks[1].parameters[0].ty =
            Type::Scalar(ScalarType::I32);
        assert_eq!(
            literal_alias_rounds(&source, candidate(&[], &[])),
            Err("fixture edge type")
        );
    }
}
