use super::*;

#[cfg(test)]
mod tests;

pub(super) fn exact_d1_workgroup(module: &Module, function: &Function) -> Option<u32> {
    let mut contracts = module
        .kernels
        .iter()
        .filter(|kernel| kernel.entry == function.id);
    let extent = |kernel: &fe2o3_kernel_ir::Kernel| {
        let size = kernel.workgroup_size?;
        (matches!(kernel.domain, fe2o3_kernel_ir::LaunchDomain::D1 { .. })
            && size.x != 0
            && size.y == 1
            && size.z == 1)
            .then_some(size.x)
    };
    let first = extent(contracts.next()?)?;
    contracts
        .all(|kernel| extent(kernel) == Some(first))
        .then_some(first)
}

pub(super) fn aligned_global_comparison(
    workgroup: Option<u32>,
    definitions: &BTreeMap<ValueId, &Operation>,
    predicate: ComparePredicate,
    lhs: ValueId,
    rhs: ValueId,
) -> bool {
    let Some(workgroup) = workgroup.filter(|extent| *extent != 0) else {
        return false;
    };
    let aligned = |predicate, index, bound_id| {
        let Some(operation) = definitions.get(&index) else {
            return false;
        };
        let [result] = operation.results.as_slice() else {
            return false;
        };
        if result.id != index || result.ty != Type::INDEX {
            return false;
        }
        if !matches!(&operation.kind, OperationKind::Intrinsic(fe2o3_kernel_ir::IntrinsicOperation {
            kind: IntrinsicKind::InvocationIndex { kind: IndexKind::Global, axis: Axis::X },
            result_type,
        }) if result_type == &Type::INDEX)
        {
            return false;
        }
        let Some(operation) = definitions.get(&bound_id) else {
            return false;
        };
        let [result] = operation.results.as_slice() else {
            return false;
        };
        let OperationKind::Constant(Constant::Index(bound)) = operation.kind else {
            return false;
        };
        if result.id != bound_id || result.ty != Type::INDEX {
            return false;
        }
        let cutoff = match predicate {
            ComparePredicate::LessThan | ComparePredicate::GreaterThanOrEqual => Some(bound),
            ComparePredicate::LessThanOrEqual | ComparePredicate::GreaterThan => {
                bound.checked_add(1)
            }
            ComparePredicate::Equal | ComparePredicate::NotEqual => None,
        };
        cutoff.is_some_and(|cutoff| cutoff % u64::from(workgroup) == 0)
    };
    aligned(predicate, lhs, rhs) || aligned(swap_predicate(predicate), rhs, lhs)
}

type ProofResult<T> = Result<T, ()>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Receipt {
    work: usize,
    // Cumulative requested rows for pass-local indexes and proposals only.
    rows: usize,
}

struct Budget {
    receipt: Receipt,
    work_limit: usize,
    row_limit: usize,
}

impl Budget {
    fn charge(&mut self, amount: usize) -> ProofResult<()> {
        let next = self.receipt.work.checked_add(amount).ok_or(())?;
        if next > self.work_limit {
            return Err(());
        }
        self.receipt.work = next;
        Ok(())
    }

    fn reserve_rows(&mut self, rows: usize) -> ProofResult<()> {
        let next = self.receipt.rows.checked_add(rows).ok_or(())?;
        if next > self.row_limit {
            return Err(());
        }
        self.charge(rows)?;
        self.receipt.rows = next;
        Ok(())
    }

    fn get<'a, K: Ord, V>(
        &mut self,
        map: &'a BTreeMap<K, V>,
        key: &K,
    ) -> ProofResult<Option<&'a V>> {
        // A lookup cannot compare more keys than the borrowed tree contains.
        self.charge(map.len().max(1))?;
        Ok(map.get(key))
    }

    fn contains<K: Ord>(&mut self, set: &BTreeSet<K>, key: &K) -> ProofResult<bool> {
        self.charge(set.len().max(1))?;
        Ok(set.contains(key))
    }
}

// Sorted borrowed rows let the pass meter every search comparison without
// relying on an assumed cost for the standard library's tree implementation.
struct BorrowedIndex<'a, K, V> {
    rows: Vec<(&'a K, &'a V)>,
}

impl<'a, K: Ord, V> BorrowedIndex<'a, K, V> {
    fn new(map: &'a BTreeMap<K, V>, budget: &mut Budget) -> ProofResult<Self> {
        budget.reserve_rows(map.len())?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(map.len()).map_err(|_| ())?;
        for (key, value) in map {
            budget.charge(1)?;
            rows.push((key, value));
        }
        Ok(Self { rows })
    }

    fn get(&self, key: &K, budget: &mut Budget) -> ProofResult<Option<&'a V>> {
        let mut low = 0;
        let mut high = self.rows.len();
        while low < high {
            budget.charge(1)?;
            let middle = low + (high - low) / 2;
            let (candidate, value) = self.rows[middle];
            match candidate.cmp(key) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Equal => return Ok(Some(value)),
                std::cmp::Ordering::Greater => high = middle,
            }
        }
        Ok(None)
    }
}

impl<'a, K: Ord> BorrowedIndex<'a, K, ()> {
    fn from_set(set: &'a BTreeSet<K>, budget: &mut Budget) -> ProofResult<Self> {
        budget.reserve_rows(set.len())?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(set.len()).map_err(|_| ())?;
        for key in set {
            budget.charge(1)?;
            rows.push((key, &()));
        }
        Ok(Self { rows })
    }
}

struct IndexedFacts<'a> {
    body: &'a FunctionBody,
    incoming: &'a BTreeMap<BlockId, Vec<Edge>>,
    dominators: &'a BTreeMap<BlockId, BTreeSet<BlockId>>,
    types: BorrowedIndex<'a, ValueId, Type>,
    definitions: BorrowedIndex<'a, ValueId, &'a Operation>,
}

impl<'a> IndexedFacts<'a> {
    fn new(facts: &'a Facts<'a>, budget: &mut Budget) -> ProofResult<Self> {
        Ok(Self {
            body: facts.body,
            incoming: facts.incoming,
            dominators: facts.dominators,
            types: BorrowedIndex::new(facts.types, budget)?,
            definitions: BorrowedIndex::new(facts.definitions, budget)?,
        })
    }
}

struct Facts<'a> {
    body: &'a FunctionBody,
    incoming: &'a BTreeMap<BlockId, Vec<Edge>>,
    dominators: &'a BTreeMap<BlockId, BTreeSet<BlockId>>,
    types: &'a BTreeMap<ValueId, Type>,
    definitions: &'a BTreeMap<ValueId, &'a Operation>,
}

#[derive(Clone, Copy)]
struct Comparison {
    predicate: ComparePredicate,
    lhs: ValueId,
    rhs: ValueId,
}

impl IndexedFacts<'_> {
    fn comparison(&self, value: ValueId, budget: &mut Budget) -> ProofResult<Option<Comparison>> {
        let Some(operation) = self.definitions.get(&value, budget)? else {
            return Ok(None);
        };
        let [result] = operation.results.as_slice() else {
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
        if result.id != value || result.ty != Type::BOOL {
            return Ok(None);
        }
        let lhs_type = self.types.get(&lhs, budget)?;
        let rhs_type = self.types.get(&rhs, budget)?;
        if lhs_type.and_then(unsigned_type_range).is_none()
            || lhs_type != rhs_type
            || (lhs_type == Some(&Type::BOOL)
                && !matches!(
                    predicate,
                    ComparePredicate::Equal | ComparePredicate::NotEqual
                ))
        {
            return Ok(None);
        }
        Ok(Some(Comparison {
            predicate,
            lhs,
            rhs,
        }))
    }

    fn constant(
        &self,
        value: ValueId,
        ty: &Type,
        budget: &mut Budget,
    ) -> ProofResult<Option<UnsignedRange>> {
        let Some(operation) = self.definitions.get(&value, budget)? else {
            return Ok(None);
        };
        let [result] = operation.results.as_slice() else {
            return Ok(None);
        };
        let OperationKind::Constant(ref constant) = operation.kind else {
            return Ok(None);
        };
        if result.id != value || &result.ty != ty || constant.ty() != *ty {
            return Ok(None);
        }
        Ok(unsigned_constant_range(constant))
    }

    fn exclusive_edge(
        &self,
        source: BlockId,
        target: BlockId,
        query_dominators: &BorrowedIndex<'_, BlockId, ()>,
        budget: &mut Budget,
    ) -> ProofResult<bool> {
        if source == target || query_dominators.get(&target, budget)?.is_none() {
            return Ok(false);
        }
        Ok(budget
            .get(self.incoming, &target)?
            .is_some_and(|edges| matches!(edges.as_slice(), [edge] if edge.source == source)))
    }

    fn range(
        &self,
        value: ValueId,
        query: BlockId,
        budget: &mut Budget,
    ) -> ProofResult<Option<UnsignedRange>> {
        let Some(base) = self.base_range(value, query, budget)? else {
            return Ok(None);
        };
        let Some(operation) = self.definitions.get(&value, budget)? else {
            return Ok(Some(base));
        };
        let [result, overflow] = operation.results.as_slice() else {
            return Ok(Some(base));
        };
        let OperationKind::Binary {
            op: BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Multiply),
            lhs,
            rhs,
        } = operation.kind
        else {
            return Ok(Some(base));
        };
        if result.id != value
            || overflow.ty != Type::BOOL
            || !matches!(
                result.ty,
                Type::Scalar(
                    ScalarType::U8
                        | ScalarType::U16
                        | ScalarType::U32
                        | ScalarType::U64
                        | ScalarType::U128
                        | ScalarType::Index
                )
            )
            || self.types.get(&lhs, budget)? != Some(&result.ty)
            || self.types.get(&rhs, budget)? != Some(&result.ty)
        {
            return Ok(Some(base));
        }
        let (factor, operand) = if let Some(factor) = self.constant(lhs, &result.ty, budget)? {
            (factor, rhs)
        } else if let Some(factor) = self.constant(rhs, &result.ty, budget)? {
            (factor, lhs)
        } else {
            return Ok(Some(base));
        };
        // This one-level transfer uses only original-CFG guards, never another
        // product transfer or a refined successor fact. A wrapping product is unknown.
        let Some(operand) = self.base_range(operand, query, budget)? else {
            return Ok(Some(base));
        };
        let Some(min) = operand.min.checked_mul(factor.min) else {
            return Ok(Some(base));
        };
        let Some(max) = operand.max.checked_mul(factor.max) else {
            return Ok(Some(base));
        };
        let Some(type_range) = unsigned_type_range(&result.ty) else {
            return Ok(Some(base));
        };
        if max > type_range.max {
            return Ok(Some(base));
        }
        Ok(Some(base.intersect(UnsignedRange { min, max })))
    }

    fn base_range(
        &self,
        value: ValueId,
        query: BlockId,
        budget: &mut Budget,
    ) -> ProofResult<Option<UnsignedRange>> {
        let Some(ty) = self.types.get(&value, budget)? else {
            return Ok(None);
        };
        let Some(mut range) = unsigned_type_range(ty) else {
            return Ok(None);
        };
        if let Some(constant) = self.constant(value, ty, budget)? {
            return Ok(Some(constant));
        }
        let Some(query_dominators) = budget.get(self.dominators, &query)? else {
            return Ok(None);
        };
        let query_dominators = BorrowedIndex::from_set(query_dominators, budget)?;
        for block in &self.body.blocks {
            budget.charge(1)?;
            // Both dominances use the original CFG. Strict source dominance also
            // excludes the implicit entry edge and facts from a future loop visit.
            if block.id == query || query_dominators.get(&block.id, budget)?.is_none() {
                continue;
            }
            match &block.terminator {
                Some(Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    else_target,
                    ..
                }) if then_target != else_target => {
                    let Some(comparison) = self.comparison(*condition, budget)? else {
                        continue;
                    };
                    let (predicate, other) = if comparison.lhs == value {
                        (comparison.predicate, comparison.rhs)
                    } else if comparison.rhs == value {
                        (swap_predicate(comparison.predicate), comparison.lhs)
                    } else {
                        continue;
                    };
                    let then_edge =
                        self.exclusive_edge(block.id, *then_target, &query_dominators, budget)?;
                    let else_edge =
                        self.exclusive_edge(block.id, *else_target, &query_dominators, budget)?;
                    let predicate = match (then_edge, else_edge) {
                        (true, false) => predicate,
                        (false, true) => invert_predicate(predicate),
                        _ => continue,
                    };
                    if let Some(bound) = self.constant(other, ty, budget)? {
                        if comparison_truth(predicate, range, bound) == Some(false) {
                            return Ok(None);
                        }
                        range = refine_unsigned_range(range, predicate, bound);
                    }
                }
                Some(Terminator::Switch {
                    selector,
                    cases,
                    default_target,
                    ..
                }) if *selector == value => {
                    for case in cases {
                        budget.charge(1)?;
                        if case.target == *default_target
                            || !self.exclusive_edge(
                                block.id,
                                case.target,
                                &query_dominators,
                                budget,
                            )?
                        {
                            continue;
                        }
                        let bound = u128::from(case.value);
                        if bound > unsigned_type_range(ty).ok_or(())?.max {
                            continue;
                        }
                        if bound < range.min || bound > range.max {
                            return Ok(None);
                        }
                        range = UnsignedRange::exact(bound);
                    }
                }
                Some(Terminator::IntegerSwitch {
                    selector,
                    cases,
                    default_target,
                    ..
                }) if *selector == value => {
                    for case in cases {
                        budget.charge(1)?;
                        if case.target == *default_target
                            || case.value.ty() != *ty
                            || !self.exclusive_edge(
                                block.id,
                                case.target,
                                &query_dominators,
                                budget,
                            )?
                        {
                            continue;
                        }
                        let Some(bound) = unsigned_constant_range(&case.value) else {
                            continue;
                        };
                        if bound.min < range.min || bound.max > range.max {
                            return Ok(None);
                        }
                        range = bound;
                    }
                }
                _ => {}
            }
        }
        Ok(Some(range))
    }
}

fn comparison_truth(
    predicate: ComparePredicate,
    lhs: UnsignedRange,
    rhs: UnsignedRange,
) -> Option<bool> {
    match predicate {
        ComparePredicate::Equal if lhs.min == lhs.max && lhs == rhs => Some(true),
        ComparePredicate::Equal if lhs.max < rhs.min || rhs.max < lhs.min => Some(false),
        ComparePredicate::NotEqual => {
            comparison_truth(ComparePredicate::Equal, lhs, rhs).map(|truth| !truth)
        }
        ComparePredicate::LessThan if lhs.max < rhs.min => Some(true),
        ComparePredicate::LessThan if lhs.min >= rhs.max => Some(false),
        ComparePredicate::LessThanOrEqual if lhs.max <= rhs.min => Some(true),
        ComparePredicate::LessThanOrEqual if lhs.min > rhs.max => Some(false),
        ComparePredicate::GreaterThan => comparison_truth(ComparePredicate::LessThan, rhs, lhs),
        ComparePredicate::GreaterThanOrEqual => {
            comparison_truth(ComparePredicate::LessThanOrEqual, rhs, lhs)
        }
        _ => None,
    }
}

pub(super) fn refine_successors(
    body: &FunctionBody,
    incoming: &BTreeMap<BlockId, Vec<Edge>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    types: &BTreeMap<ValueId, Type>,
    definitions: &BTreeMap<ValueId, &Operation>,
    effective: &mut BTreeMap<BlockId, BTreeSet<BlockId>>,
) {
    let facts = Facts {
        body,
        incoming,
        dominators,
        types,
        definitions,
    };
    let _ = refine_with_limits(
        &facts,
        effective,
        MAX_RELATIONAL_PROOF_WORK,
        MAX_RELATIONAL_PROOF_WORK,
    );
}

fn refine_with_limits(
    facts: &Facts<'_>,
    effective: &mut BTreeMap<BlockId, BTreeSet<BlockId>>,
    work_limit: usize,
    row_limit: usize,
) -> ProofResult<Receipt> {
    let mut budget = Budget {
        receipt: Receipt::default(),
        work_limit,
        row_limit,
    };
    let facts = IndexedFacts::new(facts, &mut budget)?;
    let mut proposals = Vec::new();
    let mut candidates = 0_usize;
    for block in &facts.body.blocks {
        budget.charge(1)?;
        if matches!(block.terminator, Some(Terminator::ConditionalBranch { then_target, else_target, .. }) if then_target != else_target)
        {
            candidates = candidates.checked_add(1).ok_or(())?;
        }
    }
    budget.reserve_rows(candidates)?;
    proposals.try_reserve_exact(candidates).map_err(|_| ())?;
    for block in &facts.body.blocks {
        budget.charge(1)?;
        let Some(Terminator::ConditionalBranch {
            condition,
            then_target,
            else_target,
            ..
        }) = &block.terminator
        else {
            continue;
        };
        if then_target == else_target {
            continue;
        }
        let Some(successors) = budget.get(effective, &block.id)? else {
            continue;
        };
        if successors.len() <= 1 {
            continue;
        }
        let Some(comparison) = facts.comparison(*condition, &mut budget)? else {
            continue;
        };
        let Some(lhs) = facts.range(comparison.lhs, block.id, &mut budget)? else {
            continue;
        };
        let Some(rhs) = facts.range(comparison.rhs, block.id, &mut budget)? else {
            continue;
        };
        let Some(truth) = comparison_truth(comparison.predicate, lhs, rhs) else {
            continue;
        };
        let target = if truth { *then_target } else { *else_target };
        if !budget.contains(successors, &target)? {
            continue;
        }
        budget.charge(1)?;
        proposals.push((block.id, target));
    }
    // Admit the complete mutation pass before changing any borrowed CFG owner.
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
