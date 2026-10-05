use super::*;

type Result<T> = std::result::Result<T, ()>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Receipt {
    pub(super) work: usize,
    pub(super) rows: usize,
}

struct Budget {
    receipt: Receipt,
    work_limit: usize,
    row_limit: usize,
}

impl Budget {
    fn charge(&mut self, work: usize) -> Result<()> {
        let next = self.receipt.work.checked_add(work).ok_or(())?;
        if next > self.work_limit {
            return Err(());
        }
        self.receipt.work = next;
        Ok(())
    }

    fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>> {
        let rows = self.receipt.rows.checked_add(capacity).ok_or(())?;
        if rows > self.row_limit {
            return Err(());
        }
        self.charge(capacity)?;
        let mut vector = Vec::new();
        vector.try_reserve_exact(capacity).map_err(|_| ())?;
        let actual = self.receipt.rows.checked_add(vector.capacity()).ok_or(())?;
        if actual > self.row_limit {
            return Err(());
        }
        self.charge(vector.capacity().checked_sub(capacity).ok_or(())?)?;
        self.receipt.rows = actual;
        Ok(vector)
    }

    fn get<'a, K: Ord, V>(&mut self, map: &'a BTreeMap<K, V>, key: &K) -> Result<Option<&'a V>> {
        self.charge(map.len().max(1))?;
        Ok(map.get(key))
    }

    fn contains<K: Ord>(&mut self, set: &BTreeSet<K>, key: &K) -> Result<bool> {
        self.charge(set.len().max(1))?;
        Ok(set.contains(key))
    }

    fn push<T: Copy>(&mut self, vector: &mut Vec<T>, value: T) -> Result<()> {
        self.charge(1)?;
        if vector.len() == vector.capacity() {
            // Keep old and new backing paid simultaneously. Cumulative rows
            // deliberately do not refund the retired temporary allocation.
            let capacity = vector.capacity().max(1).checked_mul(2).ok_or(())?;
            let mut grown = self.vector(capacity)?;
            self.charge(vector.len())?;
            grown.extend_from_slice(vector);
            *vector = grown;
        }
        vector.push(value);
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct Value {
    range: Option<UnsignedRange>,
    contextual: bool,
}

#[derive(Clone, Copy, Default)]
struct Row<'a> {
    ty: Option<&'a Type>,
    operation: Option<&'a Operation>,
    value: Value,
    epoch: usize,
    done: bool,
    busy: usize,
    value_guard: Option<usize>,
    slice_guard: Option<usize>,
}

struct Index<'a> {
    rows: Vec<Row<'a>>,
}

impl<'a> Index<'a> {
    fn new(
        types: &'a BTreeMap<ValueId, Type>,
        definitions: &BTreeMap<ValueId, &'a Operation>,
        budget: &mut Budget,
    ) -> Result<Self> {
        let mut length = 0;
        for id in types.keys() {
            budget.charge(1)?;
            length = length.max((id.0 as usize).checked_add(1).ok_or(())?);
        }
        let mut rows = budget.vector(length)?;
        rows.resize(length, Row::default());
        for (id, ty) in types {
            budget.charge(1)?;
            rows[id.0 as usize].ty = Some(ty);
        }
        for (id, operation) in definitions {
            budget.charge(1)?;
            let row = rows.get_mut(id.0 as usize).ok_or(())?;
            if row.ty.is_none() || row.operation.replace(operation).is_some() {
                return Err(());
            }
        }
        Ok(Self { rows })
    }

    fn row(&self, id: ValueId, budget: &mut Budget) -> Result<Row<'a>> {
        budget.charge(1)?;
        self.rows
            .get(id.0 as usize)
            .copied()
            .filter(|row| row.ty.is_some())
            .ok_or(())
    }

    fn single(&self, id: ValueId, budget: &mut Budget) -> Result<Option<&'a Operation>> {
        let row = self.row(id, budget)?;
        let Some(operation) = row.operation else {
            return Ok(None);
        };
        budget.charge(1)?;
        Ok(matches!(operation.results.as_slice(),[result] if result.id==id && Some(&result.ty)==row.ty).then_some(operation))
    }

    fn constant(&self, id: ValueId, budget: &mut Budget) -> Result<Option<UnsignedRange>> {
        let Some(operation) = self.single(id, budget)? else {
            return Ok(None);
        };
        let OperationKind::Constant(constant) = &operation.kind else {
            return Ok(None);
        };
        if constant.ty() != operation.results[0].ty {
            return Ok(None);
        }
        Ok(constant_range(constant))
    }

    fn key(&self, id: ValueId, budget: &mut Budget) -> Result<Key> {
        if let Some(operation) = self.single(id, budget)?
            && operation.results[0].ty == Type::INDEX
            && let OperationKind::SliceLength { slice } = operation.kind
            && matches!(self.row(slice, budget)?.ty, Some(Type::Slice(_)))
        {
            return Ok(Key::SliceLength(slice));
        }
        Ok(Key::Value(id))
    }

    fn comparison(
        &self,
        id: ValueId,
        budget: &mut Budget,
    ) -> Result<Option<(Key, ComparePredicate, UnsignedRange)>> {
        let Some(operation) = self.single(id, budget)? else {
            return Ok(None);
        };
        let OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } = operation.kind
        else {
            return Ok(None);
        };
        if operation.results[0].ty != Type::BOOL {
            return Ok(None);
        }
        let ty = self.row(lhs, budget)?.ty;
        if ty != self.row(rhs, budget)?.ty || ty.and_then(unsigned_type_range).is_none() {
            return Ok(None);
        }
        if ty == Some(&Type::BOOL)
            && !matches!(
                predicate,
                ComparePredicate::Equal | ComparePredicate::NotEqual
            )
        {
            return Ok(None);
        }
        if let Some(bound) = self.constant(rhs, budget)? {
            return Ok(Some((self.key(lhs, budget)?, predicate, bound)));
        }
        if let Some(bound) = self.constant(lhs, budget)? {
            return Ok(Some((
                self.key(rhs, budget)?,
                swap_predicate(predicate),
                bound,
            )));
        }
        Ok(None)
    }

    fn guard_head(&self, key: Key, budget: &mut Budget) -> Result<Option<usize>> {
        Ok(match key {
            Key::Value(id) => self.row(id, budget)?.value_guard,
            Key::SliceLength(id) => self.row(id, budget)?.slice_guard,
        })
    }

    fn attach_guard(
        &mut self,
        key: Key,
        ordinal: usize,
        budget: &mut Budget,
    ) -> Result<Option<usize>> {
        budget.charge(1)?;
        let (Key::Value(id) | Key::SliceLength(id)) = key;
        let row = self.rows.get_mut(id.0 as usize).ok_or(())?;
        if row.ty.is_none() {
            return Err(());
        }
        Ok(match key {
            Key::Value(_) => row.value_guard.replace(ordinal),
            Key::SliceLength(_) => row.slice_guard.replace(ordinal),
        })
    }
}

fn constant_range(constant: &Constant) -> Option<UnsignedRange> {
    unsigned_constant_range(constant).or_else(|| {
        // Signed constants are transported only through Select/switch. They do
        // not enter unsigned arithmetic or range-refinement rules.
        let value = match constant {
            Constant::I8(v) => i128::from(*v),
            Constant::I16(v) => i128::from(*v),
            Constant::I32(v) => i128::from(*v),
            Constant::I64(v) => i128::from(*v),
            _ => return None,
        };
        u128::try_from(value).ok().map(UnsignedRange::exact)
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Key {
    Value(ValueId),
    SliceLength(ValueId),
}

#[derive(Clone, Copy)]
struct Guard {
    key: Key,
    source: BlockId,
    target: BlockId,
    predicate: ComparePredicate,
    bound: UnsignedRange,
    next: Option<usize>,
}

fn guards(body: &FunctionBody, index: &mut Index<'_>, budget: &mut Budget) -> Result<Vec<Guard>> {
    let mut capacity = 0usize;
    for block in &body.blocks {
        budget.charge(1)?;
        let count = match &block.terminator {
            Some(Terminator::ConditionalBranch { .. }) => 2,
            Some(Terminator::Switch { cases, .. }) => cases.len(),
            Some(Terminator::IntegerSwitch { cases, .. }) => cases.len(),
            _ => 0,
        };
        capacity = capacity.checked_add(count).ok_or(())?;
    }
    let mut guards = budget.vector(capacity)?;
    for block in &body.blocks {
        budget.charge(1)?;
        match &block.terminator {
            Some(Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            }) if then_target != else_target => {
                if let Some((key, predicate, bound)) = index.comparison(*condition, budget)? {
                    let next = index.attach_guard(key, guards.len(), budget)?;
                    guards.push(Guard {
                        key,
                        source: block.id,
                        target: *then_target,
                        predicate,
                        bound,
                        next,
                    });
                    let next = index.attach_guard(key, guards.len(), budget)?;
                    guards.push(Guard {
                        key,
                        source: block.id,
                        target: *else_target,
                        predicate: invert_predicate(predicate),
                        bound,
                        next,
                    });
                }
            }
            Some(Terminator::Switch {
                selector,
                cases,
                default_target,
                ..
            }) => {
                let ty = index.row(*selector, budget)?.ty;
                if let Some(range) = ty.and_then(unsigned_type_range) {
                    let key = index.key(*selector, budget)?;
                    for case in cases {
                        budget.charge(1)?;
                        if case.target != *default_target && u128::from(case.value) <= range.max {
                            let next = index.attach_guard(key, guards.len(), budget)?;
                            guards.push(Guard {
                                key,
                                source: block.id,
                                target: case.target,
                                predicate: ComparePredicate::Equal,
                                bound: UnsignedRange::exact(u128::from(case.value)),
                                next,
                            });
                        }
                    }
                }
            }
            Some(Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                ..
            }) => {
                let ty = index.row(*selector, budget)?.ty;
                if ty.and_then(unsigned_type_range).is_some() {
                    let key = index.key(*selector, budget)?;
                    for case in cases {
                        budget.charge(1)?;
                        if case.target != *default_target
                            && Some(&case.value.ty()) == ty
                            && let Some(bound) = unsigned_constant_range(&case.value)
                        {
                            let next = index.attach_guard(key, guards.len(), budget)?;
                            guards.push(Guard {
                                key,
                                source: block.id,
                                target: case.target,
                                predicate: ComparePredicate::Equal,
                                bound,
                                next,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(guards)
}

struct Query<'a, 'b, 'c> {
    context: &'b UniformityPhysicalLaunchV2<'c>,
    index: Index<'a>,
    guards: Vec<Guard>,
    stack: Vec<(ValueId, bool)>,
    incoming: &'b BTreeMap<BlockId, Vec<Edge>>,
    dominators: &'b BTreeMap<BlockId, BTreeSet<BlockId>>,
    epoch: usize,
}

impl Query<'_, '_, '_> {
    fn cached(&self, id: ValueId, budget: &mut Budget) -> Result<Option<Value>> {
        let row = self.index.row(id, budget)?;
        Ok((row.done && (!row.value.contextual || row.epoch == self.epoch)).then_some(row.value))
    }

    fn operands(&self, id: ValueId, budget: &mut Budget) -> Result<[Option<ValueId>; 3]> {
        let Some(op) = self.index.row(id, budget)?.operation else {
            return Ok([None; 3]);
        };
        Ok(match op.kind {
            OperationKind::Binary { lhs, rhs, .. } | OperationKind::Compare { lhs, rhs, .. } => {
                [Some(lhs), Some(rhs), None]
            }
            OperationKind::Unary { operand, .. } => [Some(operand), None, None],
            OperationKind::Cast { value, .. } => [Some(value), None, None],
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => [Some(condition), Some(true_value), Some(false_value)],
            _ => [None; 3],
        })
    }

    fn value(&mut self, id: ValueId, block: BlockId, budget: &mut Budget) -> Result<Value> {
        self.epoch = self.epoch.checked_add(1).ok_or(())?;
        self.stack.clear();
        budget.push(&mut self.stack, (id, false))?;
        while let Some((next, expanded)) = self.stack.pop() {
            budget.charge(1)?;
            if self.cached(next, budget)?.is_some() {
                continue;
            }
            if expanded {
                let mut value = self.evaluate(next, budget)?;
                let row = self.index.row(next, budget)?;
                if row.ty.and_then(unsigned_type_range).is_some() {
                    let key = self.index.key(next, budget)?;
                    let mut head = self.index.guard_head(key, budget)?;
                    while let Some(ordinal) = head {
                        budget.charge(1)?;
                        let guard = self.guards.get(ordinal).ok_or(())?;
                        if guard.key != key {
                            return Err(());
                        }
                        head = guard.next;
                        value.contextual = true;
                        let Some(dominators) = budget.get(self.dominators, &block)? else {
                            continue;
                        };
                        if guard.source==block || guard.source==guard.target
                            || !budget.contains(dominators,&guard.source)? || !budget.contains(dominators,&guard.target)?
                            || !budget.get(self.incoming,&guard.target)?.is_some_and(|edges|matches!(edges.as_slice(),[edge] if edge.source==guard.source))
                        { continue; }
                        if let Some(range) = value.range {
                            value.range = if contextual_control::comparison_truth(
                                guard.predicate,
                                range,
                                guard.bound,
                            ) == Some(false)
                            {
                                None
                            } else {
                                Some(refine_unsigned_range(range, guard.predicate, guard.bound))
                            };
                        }
                    }
                }
                let row = self.index.rows.get_mut(next.0 as usize).ok_or(())?;
                row.value = value;
                row.epoch = self.epoch;
                row.done = true;
                row.busy = 0;
                continue;
            }
            if self.index.row(next, budget)?.busy == self.epoch {
                continue;
            }
            self.index.rows[next.0 as usize].busy = self.epoch;
            budget.push(&mut self.stack, (next, true))?;
            for operand in self.operands(next, budget)?.into_iter().flatten().rev() {
                budget.charge(1)?;
                if self.cached(operand, budget)?.is_none()
                    && self.index.row(operand, budget)?.busy != self.epoch
                {
                    budget.push(&mut self.stack, (operand, false))?;
                }
            }
        }
        self.cached(id, budget)?.ok_or(())
    }

    fn evaluate(&self, id: ValueId, budget: &mut Budget) -> Result<Value> {
        let row = self.index.row(id, budget)?;
        let ty = row.ty.ok_or(())?;
        let Some(op) = row.operation else {
            return Ok(Value {
                range: unsigned_type_range(ty),
                contextual: true,
            });
        };
        budget.charge(op.results.len())?;
        let Some(ordinal) = op
            .results
            .iter()
            .position(|result| result.id == id && &result.ty == ty)
        else {
            return Err(());
        };
        let operands = self.operands(id, budget)?;
        let mut values = [Value {
            range: None,
            contextual: true,
        }; 3];
        for (slot, operand) in operands.into_iter().enumerate() {
            if let Some(operand) = operand {
                values[slot] = self.cached(operand, budget)?.unwrap_or(Value {
                    range: None,
                    contextual: true,
                });
            }
        }
        let contextual = operands
            .iter()
            .enumerate()
            .any(|(i, id)| id.is_some() && values[i].contextual);
        let mut range = None;
        match &op.kind {
            OperationKind::Constant(constant)
                if ordinal == 0 && op.results.len() == 1 && constant.ty() == *ty =>
            {
                range = constant_range(constant)
            }
            OperationKind::Intrinsic(intrinsic)
                if ordinal == 0
                    && op.results.len() == 1
                    && *ty == Type::INDEX
                    && intrinsic.result_type == Type::INDEX =>
            {
                if let IntrinsicKind::InvocationIndex { kind, axis } = intrinsic.kind {
                    range = self.context.invocation_range(kind, axis);
                }
            }
            OperationKind::SliceLength { slice }
                if ordinal == 0
                    && op.results.len() == 1
                    && *ty == Type::INDEX
                    && matches!(self.index.row(*slice, budget)?.ty, Some(Type::Slice(_))) =>
            {
                return Ok(Value {
                    range: unsigned_type_range(ty),
                    contextual: true,
                });
            }
            OperationKind::Binary {
                op: operator,
                lhs,
                rhs,
            } => {
                let input = self.index.row(*lhs, budget)?.ty;
                let same = input == self.index.row(*rhs, budget)?.ty;
                if same
                    && let (Some(a), Some(b), Some(type_range)) = (
                        values[0].range,
                        values[1].range,
                        input.and_then(unsigned_type_range),
                    )
                {
                    if let BinaryOp::Checked(checked) = operator {
                        if matches!(op.results.as_slice(),[value,flag] if Some(&value.ty)==input && flag.ty==Type::BOOL)
                        {
                            let result = checked_result_range(*checked, a, b, type_range.max);
                            range = match ordinal {
                                0 => result,
                                1 if result.is_some() => Some(UnsignedRange::exact(0)),
                                _ => None,
                            };
                        }
                    } else if ordinal == 0 && op.results.len() == 1 && Some(ty) == input {
                        range = if *ty == Type::BOOL {
                            bool_binary(*operator, a, b)
                        } else {
                            binary_result_range(*operator, a, b, type_range.max)
                        };
                    }
                }
            }
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand,
            } if ordinal == 0
                && op.results.len() == 1
                && *ty == Type::BOOL
                && self.index.row(*operand, budget)?.ty == Some(&Type::BOOL) =>
            {
                range = values[0]
                    .range
                    .filter(|r| r.max <= 1)
                    .map(|r| UnsignedRange {
                        min: 1 - r.max,
                        max: 1 - r.min,
                    });
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } if ordinal == 0 && op.results.len() == 1 && *ty == Type::BOOL => {
                let input = self.index.row(*lhs, budget)?.ty;
                if input == self.index.row(*rhs, budget)?.ty
                    && input.and_then(unsigned_type_range).is_some()
                    && let (Some(a), Some(b)) = (values[0].range, values[1].range)
                {
                    range = contextual_control::comparison_truth(*predicate, a, b)
                        .map(|truth| UnsignedRange::exact(u128::from(truth)));
                }
            }
            OperationKind::Cast { kind, value, to }
                if ordinal == 0 && op.results.len() == 1 && to == ty =>
            {
                if let (Some(source), Some(source_ty)) =
                    (values[0].range, self.index.row(*value, budget)?.ty)
                {
                    range = cast_result_range(*kind, source_ty, ty, source);
                }
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } if ordinal == 0
                && op.results.len() == 1
                && self.index.row(*condition, budget)?.ty == Some(&Type::BOOL)
                && self.index.row(*true_value, budget)?.ty == Some(ty)
                && self.index.row(*false_value, budget)?.ty == Some(ty) =>
            {
                range = match values[0].range {
                    Some(r) if r.min == 1 && r.max == 1 => values[1].range,
                    Some(r) if r.min == 0 && r.max == 0 => values[2].range,
                    Some(r) if r.max <= 1 => {
                        values[1]
                            .range
                            .zip(values[2].range)
                            .map(|(a, b)| UnsignedRange {
                                min: a.min.min(b.min),
                                max: a.max.max(b.max),
                            })
                    }
                    _ => None,
                };
            }
            _ => {}
        }
        // Unknown producers retain only their complete unsigned type domain,
        // as the historical direct-guard proof does. No memory, call or phi
        // value is selected. Checked flags with possible overflow stay [0,1].
        Ok(Value {
            range: range.or_else(|| unsigned_type_range(ty)),
            contextual,
        })
    }
}

fn bool_binary(op: BinaryOp, a: UnsignedRange, b: UnsignedRange) -> Option<UnsignedRange> {
    if a.max > 1 || b.max > 1 {
        return None;
    }
    match op {
        BinaryOp::BitAnd => Some(UnsignedRange {
            min: a.min & b.min,
            max: a.max & b.max,
        }),
        BinaryOp::BitOr => Some(UnsignedRange {
            min: a.min | b.min,
            max: a.max | b.max,
        }),
        BinaryOp::BitXor if a.min == a.max && b.min == b.max => {
            Some(UnsignedRange::exact(a.min ^ b.min))
        }
        _ => None,
    }
}

pub(super) fn refine(
    context: &UniformityPhysicalLaunchV2<'_>,
    body: &FunctionBody,
    incoming: &BTreeMap<BlockId, Vec<Edge>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    types: &BTreeMap<ValueId, Type>,
    definitions: &BTreeMap<ValueId, &Operation>,
    effective: &mut BTreeMap<BlockId, BTreeSet<BlockId>>,
    work_limit: usize,
    row_limit: usize,
) -> Result<Receipt> {
    let mut budget = Budget {
        receipt: Receipt::default(),
        work_limit,
        row_limit,
    };
    let mut index = Index::new(types, definitions, &mut budget)?;
    let guards = guards(body, &mut index, &mut budget)?;
    let stack = Vec::new();
    let mut query = Query {
        context,
        index,
        guards,
        stack,
        incoming,
        dominators,
        epoch: 0,
    };
    let mut proposals = budget.vector(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge(1)?;
        let selector = match &block.terminator {
            Some(Terminator::ConditionalBranch { condition, .. }) => *condition,
            Some(
                Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. },
            ) => *selector,
            _ => continue,
        };
        let Some(range) = query.value(selector, block.id, &mut budget)?.range else {
            continue;
        };
        if range.min != range.max {
            continue;
        }
        let target = match &block.terminator {
            Some(Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            }) if range.max <= 1 => {
                if range.max == 1 {
                    *then_target
                } else {
                    *else_target
                }
            }
            Some(Terminator::Switch {
                cases,
                default_target,
                ..
            }) => {
                let mut selected = None;
                for case in cases {
                    budget.charge(1)?;
                    if u128::from(case.value) == range.min {
                        if selected.is_some() {
                            return Err(());
                        }
                        selected = Some(case.target);
                    }
                }
                selected.unwrap_or(*default_target)
            }
            Some(Terminator::IntegerSwitch {
                cases,
                default_target,
                ..
            }) => {
                let ty = query.index.row(selector, &mut budget)?.ty;
                let mut selected = None;
                for case in cases {
                    budget.charge(1)?;
                    if Some(&case.value.ty()) != ty {
                        return Err(());
                    }
                    if constant_range(&case.value) == Some(range) {
                        if selected.is_some() {
                            return Err(());
                        }
                        selected = Some(case.target);
                    }
                }
                selected.unwrap_or(*default_target)
            }
            _ => continue,
        };
        let Some(successors) = budget.get(effective, &block.id)? else {
            continue;
        };
        if successors.len() > 1 && budget.contains(successors, &target)? {
            budget.charge(1)?;
            proposals.push((block.id, target));
        }
    }
    for (block, _) in &proposals {
        budget.charge(2)?;
        budget.charge(effective.len().max(1))?;
        if let Some(successors) = budget.get(effective, block)? {
            budget.charge(successors.len())?;
        }
    }
    for (block, target) in proposals {
        if let Some(successors) = effective.get_mut(&block) {
            successors.retain(|successor| *successor == target);
        }
    }
    Ok(budget.receipt)
}
