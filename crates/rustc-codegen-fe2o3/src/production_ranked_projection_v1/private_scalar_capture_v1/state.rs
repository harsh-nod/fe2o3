use super::*;
use std::rc::Rc;
use super::flow_failure::{ResourceFailure, SharedStorage, StorageOperation};

#[path = "value_map.rs"]
mod value_map;
use value_map::Values;

#[path = "retained_flow.rs"]
mod retained_flow;
pub(super) use retained_flow::RetainedFlow;

#[cfg(test)]
#[path = "failure_budget_tests.rs"]
mod failure_budget_tests;

#[cfg(test)]
#[path = "enum_state_tests.rs"]
mod enum_tests;

pub(super) struct Budget {
    pub(super) remaining: usize,
    pub(super) work_exhausted: bool,
    storage: Rc<SharedStorage>,
}

// Reservations cover values retained outside the currently executing flow.
// Flow operations preflight that flow and their scratch space separately.
pub(super) struct Storage {
    live: Rc<SharedStorage>,
    nodes: usize,
}

impl Budget {
    pub(super) fn new(remaining: usize) -> Self {
        Self {
            remaining,
            work_exhausted: false,
            storage: Rc::new(SharedStorage::default()),
        }
    }

    pub(super) fn charge(&mut self, amount: usize) -> Result<()> {
        let Some(remaining) = self.remaining.checked_sub(amount) else {
            self.work_exhausted = true;
            self.storage.record(ResourceFailure::Work {
                remaining: self.remaining,
                requested: amount,
            });
            return Err(());
        };
        self.remaining = remaining;
        Ok(())
    }

    pub(super) fn check_storage(&self, parts: &[usize]) -> Result<()> {
        let total = parts
            .iter()
            .try_fold(self.storage.get(), |sum, part| sum.checked_add(*part));
        total.filter(|total| *total <= MAX_STORAGE).map(|_| ()).ok_or_else(|| {
            self.storage.record(ResourceFailure::Storage {
                operation: StorageOperation::Check,
                retained: self.storage.get(),
                replaced: 0,
                requested: total.map(|total| total - self.storage.get()),
                attempted: total,
                limit: MAX_STORAGE,
            });
        })
    }

    pub(super) fn failure(&self) -> Option<ResourceFailure> {
        self.storage.failure()
    }

    pub(super) fn reserve(&self, nodes: usize) -> Result<Storage> {
        self.check_storage(&[nodes])?;
        self.storage.set(self.storage.get() + nodes);
        Ok(Storage {
            live: Rc::clone(&self.storage),
            nodes,
        })
    }
}

impl Storage {
    pub(super) fn resize(&mut self, nodes: usize) -> Result<()> {
        let attempted = self
            .live
            .get()
            .checked_sub(self.nodes)
            .and_then(|live| live.checked_add(nodes));
        let total = attempted
            .filter(|total| *total <= MAX_STORAGE)
            .ok_or_else(|| {
                self.live.record(ResourceFailure::Storage {
                    operation: StorageOperation::Resize,
                    retained: self.live.get(),
                    replaced: self.nodes,
                    requested: Some(nodes),
                    attempted,
                    limit: MAX_STORAGE,
                });
            })?;
        self.live.set(total);
        self.nodes = nodes;
        Ok(())
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        self.live.set(self.live.get() - self.nodes);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Value {
    // Initialized data without an address origin. In particular this is never
    // an authenticated reference, even when its semantic type is a pointer.
    Opaque,
    Reference(Reference),
    Fields(Vec<Option<Value>>),
    // Fields conditional on a constructed variant, or scalar-only alternatives
    // of an initialized call result. Missing cases are unreachable, not data.
    Variants {
        ty: SemanticTypeIdV1,
        possible: u8,
        fields: Vec<Option<Value>>,
    },
}

impl Value {
    pub(super) fn depth(&self) -> usize {
        match self {
            Self::Fields(fields) | Self::Variants { fields, .. } => {
                1 + fields.iter().flatten().map(Self::depth).max().unwrap_or(0)
            }
            _ => 1,
        }
    }

    pub(super) fn nodes(&self) -> usize {
        match self {
            Self::Fields(fields) | Self::Variants { fields, .. } => {
                1 + fields.capacity() - fields.len()
                    + fields
                        .iter()
                        .map(|value| value.as_ref().map_or(1, Self::nodes))
                        .sum::<usize>()
            }
            _ => 1,
        }
    }

    fn reference_nodes(&self, budget: &mut Budget) -> Result<usize> {
        budget.charge(1)?;
        match self {
            Self::Reference(_) => Ok(1),
            Self::Opaque => Ok(0),
            Self::Fields(fields) | Self::Variants { fields, .. } => {
                let mut count = 0;
                for value in fields {
                    count += match value {
                        Some(value) => value.reference_nodes(budget)?,
                        None => {
                            budget.charge(1)?;
                            0
                        }
                    };
                }
                Ok(count)
            }
        }
    }

    pub(super) fn contains_references(&self) -> bool {
        match self {
            Self::Reference(_) => true,
            Self::Fields(fields) | Self::Variants { fields, .. } => {
                fields.iter().flatten().any(Self::contains_references)
            }
            Self::Opaque => false,
        }
    }

    fn targets(&self, targets: &mut BTreeSet<u32>, budget: &mut Budget) -> Result<()> {
        budget.charge(1)?;
        match self {
            Self::Reference(reference) => {
                targets.insert(reference.target);
            }
            Self::Fields(fields) | Self::Variants { fields, .. } => {
                for value in fields {
                    match value {
                        Some(value) => value.targets(targets, budget)?,
                        None => budget.charge(1)?,
                    }
                }
            }
            Self::Opaque => {}
        }
        Ok(())
    }

    fn invalidate(&mut self, targets: &BTreeSet<u32>, budget: &mut Budget) -> Result<bool> {
        budget.charge(1)?;
        match self {
            Self::Reference(reference) if targets.contains(&reference.target) => {
                *self = Self::Opaque;
                Ok(true)
            }
            Self::Fields(fields) | Self::Variants { fields, .. } => {
                let mut invalidated = false;
                for value in fields {
                    match value {
                        Some(value) => invalidated |= value.invalidate(targets, budget)?,
                        None => budget.charge(1)?,
                    }
                }
                Ok(invalidated)
            }
            _ => Ok(false),
        }
    }

    fn join(&self, other: &Self, escaped: &mut BTreeSet<u32>, budget: &mut Budget) -> Result<Self> {
        budget.charge(self.nodes() + other.nodes())?;
        if self == other {
            return Ok(self.clone());
        }
        if let (
            Self::Variants {
                ty,
                possible: left_possible,
                fields: left,
            },
            Self::Variants {
                ty: right_ty,
                possible: right_possible,
                fields: right,
            },
        ) = (self, other)
            && ty == right_ty
            && left.len() == 2
            && right.len() == 2
            && (1..=3).contains(left_possible)
            && (1..=3).contains(right_possible)
        {
            let mut fields = Vec::with_capacity(2);
            for variant in 0..2 {
                let left_active = left_possible & (1 << variant) != 0;
                let right_active = right_possible & (1 << variant) != 0;
                fields.push(
                    match (left_active, right_active, &left[variant], &right[variant]) {
                        (true, true, Some(left), Some(right)) => {
                            Some(left.join(right, escaped, budget)?)
                        }
                        (true, false, Some(value), None) | (false, true, None, Some(value)) => {
                            Some(value.clone())
                        }
                        (false, false, None, None) => None,
                        _ => {
                            self.targets(escaped, budget)?;
                            other.targets(escaped, budget)?;
                            return Ok(Self::Opaque);
                        }
                    },
                );
            }
            return Ok(Self::Variants {
                ty: *ty,
                possible: left_possible | right_possible,
                fields,
            });
        }
        if let (Self::Fields(left), Self::Fields(right)) = (self, other)
            && left.len() == right.len()
        {
            let mut fields = Vec::with_capacity(left.len());
            for (left, right) in left.iter().zip(right) {
                fields.push(match (left, right) {
                    (Some(left), Some(right)) => Some(left.join(right, escaped, budget)?),
                    (Some(value), None) | (None, Some(value)) => {
                        value.targets(escaped, budget)?;
                        None
                    }
                    (None, None) => None,
                });
            }
            return Ok(Self::Fields(fields));
        }
        // A lost alias remains potentially live; don't later recreate proof for
        // its referent while an untracked pointer could still reach it.
        self.targets(escaped, budget)?;
        other.targets(escaped, budget)?;
        Ok(Self::Opaque)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Flow {
    pub(super) values: Values,
    pub(super) escaped: BTreeSet<u32>,
    pub(super) dead: BTreeSet<u32>,
}

impl Flow {
    // The source is covered by the retained-entry or outgoing-flow reservation.
    pub(super) fn clone_retained(&self, budget: &mut Budget) -> Result<Self> {
        let nodes = self.nodes();
        // Clone the values and rebuild their capacity-sensitive size cache.
        budget.charge(nodes.checked_mul(2).ok_or(())?)?;
        budget.check_storage(&[nodes])?;
        Ok(self.clone())
    }

    pub(super) fn nodes(&self) -> usize {
        // One extra retained accounting word per flow, under the same ceiling.
        1 + self.dead.len() + self.escaped.len() + self.values.nodes()
    }

    fn reference_nodes(&self, budget: &mut Budget) -> Result<usize> {
        budget.charge(self.values.len())?;
        let mut count = 0;
        for value in self.values.values() {
            count += value.reference_nodes(budget)?;
        }
        Ok(count)
    }

    pub(super) fn copy_value(&self, local: u32, budget: &mut Budget) -> Result<Option<Value>> {
        let value = self.values.get_metered(local, budget)?;
        let nodes = value.map_or(0, Value::nodes);
        budget.charge(1 + 2 * nodes)?;
        budget.check_storage(&[self.nodes(), nodes])?;
        Ok(value.cloned())
    }

    pub(super) fn invalidate(&mut self, target: u32, budget: &mut Budget) -> Result<()> {
        budget.charge(1)?;
        // Singleton scratch set and at most one newly escaped target.
        budget.check_storage(&[self.nodes(), 2])?;
        let targets = BTreeSet::from([target]);
        let live_alias = self.values.invalidate(&targets, budget)?;
        if live_alias {
            self.escaped.insert(target);
        }
        Ok(())
    }

    pub(super) fn kill(&mut self, local: u32, budget: &mut Budget) -> Result<()> {
        budget.charge(1)?;
        self.values.remove_metered(local, budget)?;
        self.invalidate(local, budget)
    }

    pub(super) fn assign(
        &mut self,
        local: u32,
        value: Option<Value>,
        budget: &mut Budget,
    ) -> Result<()> {
        let nodes = value.as_ref().map_or(0, Value::nodes);
        budget.charge(1 + nodes)?;
        // Incoming value, its target set, and destination/poison growth remain
        // live across kill/invalidate and the eventual move into the map.
        budget.check_storage(&[self.nodes(), nodes, nodes, 2])?;
        self.kill(local, budget)?;
        if let Some(mut value) = value
            && !self.dead.contains(&local)
        {
            let mut targets = BTreeSet::new();
            value.targets(&mut targets, budget)?;
            budget.charge(targets.len())?;
            // At most one retained-map lookup per target, including short cuts.
            budget.charge(
                targets
                    .len()
                    .checked_mul(self.values.key_work())
                    .ok_or(())?,
            )?;
            targets.retain(|target| {
                *target == local
                    || self.escaped.contains(target)
                    || !self.values.contains_key(target)
            });
            value.invalidate(&targets, budget)?;
            self.values.insert_metered(local, value, budget)?;
        }
        Ok(())
    }

    pub(super) fn escape(&mut self, target: u32, budget: &mut Budget) -> Result<()> {
        let references = self.reference_nodes(budget)?;
        budget.charge(1)?;
        // Each stored reference edge is enqueued at most once after visiting
        // its containing local. Include pending, targets, nested and poison.
        budget.check_storage(&[
            self.nodes(),
            references,
            references,
            references,
            references,
            4,
        ])?;
        let mut pending = VecDeque::with_capacity(references + 1);
        pending.push_back(target);
        let mut targets = BTreeSet::new();
        while let Some(target) = pending.pop_front() {
            budget.charge(1)?;
            if !targets.insert(target) {
                continue;
            }
            if let Some(value) = self.values.get_metered(target, budget)? {
                let mut nested = BTreeSet::new();
                value.targets(&mut nested, budget)?;
                budget.charge(nested.len())?;
                pending.extend(nested);
            }
        }
        budget.charge(targets.len())?;
        self.escaped.extend(targets.iter().copied());
        self.values.invalidate(&targets, budget)?;
        Ok(())
    }

    pub(super) fn escape_value(&mut self, value: &Value, budget: &mut Budget) -> Result<()> {
        budget.charge(value.nodes())?;
        let _value_storage = budget.reserve(value.nodes())?;
        let references = value.reference_nodes(budget)?;
        budget.charge(1)?;
        budget.check_storage(&[self.nodes(), references])?;
        let _targets_storage = budget.reserve(references)?;
        let mut targets = BTreeSet::new();
        value.targets(&mut targets, budget)?;
        for target in targets {
            self.escape(target, budget)?;
        }
        Ok(())
    }

    pub(super) fn forget_references(&mut self, budget: &mut Budget) -> Result<()> {
        let references = self.reference_nodes(budget)?;
        budget.charge(1)?;
        budget.check_storage(&[self.nodes(), references, references])?;
        let mut targets = BTreeSet::new();
        for value in self.values.values() {
            value.targets(&mut targets, budget)?;
        }
        budget.charge(targets.len())?;
        self.escaped.extend(targets.iter().copied());
        self.values.invalidate(&targets, budget)?;
        Ok(())
    }

    pub(super) fn join(&self, other: &Self, budget: &mut Budget) -> Result<Self> {
        let nodes = self.nodes().checked_add(other.nodes()).ok_or(())?;
        budget.charge(nodes)?;
        let references = self.reference_nodes(budget)? + other.reference_nodes(budget)?;
        // Both inputs, joined map/sets and killed-target scratch coexist.
        // Each output/scratch is bounded by input nodes plus lost references.
        // A caller may already reserve an input; double counting is conservative.
        budget.check_storage(&[nodes, nodes, nodes, references, references])?;
        let mut joined = Self {
            values: Values::default(),
            escaped: self.escaped.union(&other.escaped).copied().collect(),
            dead: self.dead.union(&other.dead).copied().collect(),
        };
        let mut killed = BTreeSet::new();
        joined.values = self.values.join_ordered(
            &other.values, &mut joined.escaped, &mut killed, budget,
        )?;
        budget.charge(joined.escaped.len() + joined.dead.len() + joined.values.len())?;
        killed.extend(joined.escaped.iter().copied());
        killed.extend(joined.dead.iter().copied());
        for local in &joined.dead {
            joined.values.remove_metered(*local, budget)?;
        }
        joined.values.invalidate(&killed, budget)?;
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> Budget {
        Budget::new(MAX_WORK)
    }
    fn reference(target: u32) -> Value {
        Value::Reference(Reference {
            target,
            mutable: false,
            borrow: Site {
                block: 0,
                statement: 1,
            },
        })
    }
    fn flow() -> Flow {
        Flow {
            values: BTreeMap::from([
                (0, Value::Opaque),
                (1, reference(0)),
                (2, Value::Fields(vec![Some(reference(0))])),
            ])
            .into(),
            ..Flow::default()
        }
    }

    #[test]
    fn private_scalar_capture_flow_join_rejects_ambiguous_reference_targets() {
        let left = flow();
        let mut right = left.clone();
        right.values.insert(3, Value::Opaque);
        right.values.insert(1, reference(3));
        let joined = left.join(&right, &mut budget()).unwrap();
        assert_eq!(joined.values.get(&1), Some(&Value::Opaque));
        assert!(joined.escaped.contains(&0));
        assert!(!joined.values[&2].contains_references());
    }

    #[test]
    fn private_scalar_capture_flow_missing_predecessor_kills_origin() {
        let left = flow();
        let mut right = left.clone();
        right.values.remove(&0);
        let joined = left.join(&right, &mut budget()).unwrap();
        assert!(!joined.values.contains_key(&0));
        assert!(!joined.values[&1].contains_references());
        assert!(!joined.values[&2].contains_references());
    }

    #[test]
    fn private_scalar_capture_flow_redefinition_and_death_invalidate_aliases() {
        for death in [false, true] {
            let mut flow = flow();
            if death {
                flow.kill(0, &mut budget()).unwrap();
            } else {
                flow.assign(0, Some(Value::Opaque), &mut budget()).unwrap();
            }
            assert!(flow.escaped.contains(&0));
            assert!(!flow.values[&1].contains_references());
            assert!(!flow.values[&2].contains_references());
        }
    }

    #[test]
    fn private_scalar_capture_flow_dead_storage_cannot_be_reinitialized_without_live() {
        let mut flow = Flow::default();
        flow.dead.insert(0);
        flow.assign(0, Some(Value::Opaque), &mut budget()).unwrap();
        assert!(!flow.values.contains_key(&0));
        flow.dead.remove(&0);
        flow.assign(0, Some(Value::Opaque), &mut budget()).unwrap();
        assert_eq!(flow.values.get(&0), Some(&Value::Opaque));
    }

    #[test]
    fn private_scalar_capture_flow_escape_poison_survives_reassignment() {
        let mut flow = flow();
        flow.escape(2, &mut budget()).unwrap();
        assert!(flow.escaped.contains(&0));
        flow.assign(0, Some(Value::Opaque), &mut budget()).unwrap();
        flow.assign(1, Some(reference(0)), &mut budget()).unwrap();
        assert_eq!(flow.values.get(&1), Some(&Value::Opaque));
    }

    #[test]
    fn private_scalar_capture_flow_budget_exhaustion_cannot_produce_join() {
        let flow = flow();
        assert!(flow.join(&flow, &mut Budget::new(0)).is_err());
    }

    #[test]
    fn private_scalar_capture_storage_exact_ceiling_and_failed_resize_are_atomic() {
        let budget = budget();
        let mut held = budget.reserve(MAX_STORAGE).unwrap();
        assert!(budget.check_storage(&[0]).is_ok());
        assert!(budget.reserve(1).is_err());
        assert!(held.resize(MAX_STORAGE + 1).is_err());
        assert_eq!(budget.storage.get(), MAX_STORAGE);
        held.resize(MAX_STORAGE - 1).unwrap();
        let last = budget.reserve(1).unwrap();
        assert_eq!(budget.storage.get(), MAX_STORAGE);
        drop(last);
        drop(held);
        assert_eq!(budget.storage.get(), 0);
        assert!(budget.check_storage(&[usize::MAX, 1]).is_err());
    }

    #[test]
    fn private_scalar_capture_storage_copy_counts_retained_and_temporary_value() {
        let flow = flow();
        let mut budget = budget();
        let mut held = budget.reserve(MAX_STORAGE - flow.nodes()).unwrap();
        assert!(budget.check_storage(&[flow.nodes()]).is_ok());
        assert!(flow.copy_value(0, &mut budget).is_err());
        held.resize(MAX_STORAGE - flow.nodes() - 1).unwrap();
        assert_eq!(
            flow.copy_value(0, &mut budget).unwrap(),
            Some(Value::Opaque)
        );
    }

    #[test]
    fn private_scalar_capture_storage_source_and_successor_clones_share_the_ceiling() {
        let source = flow();
        let mut budget = budget();
        let _entries = budget.reserve(MAX_STORAGE - 2 * source.nodes()).unwrap();
        let flow = source.clone_retained(&mut budget).unwrap();
        let flow_storage = budget.reserve(flow.nodes()).unwrap();
        let next = flow.clone_retained(&mut budget).unwrap();
        let next_storage = budget.reserve(next.nodes()).unwrap();
        assert!(flow.clone_retained(&mut budget).is_err());
        drop(next);
        drop(next_storage);
        drop(flow);
        drop(flow_storage);
        assert!(source.clone_retained(&mut budget).is_ok());
    }

    #[test]
    fn private_scalar_capture_storage_join_counts_inputs_output_and_scratch() {
        let flow = flow();
        let mut budget = budget();
        let mut retained = budget.reserve(MAX_STORAGE - 2 * flow.nodes()).unwrap();
        assert!(budget.check_storage(&[flow.nodes(), flow.nodes()]).is_ok());
        assert!(flow.join(&flow, &mut budget).is_err());
        let references = 2 * flow.reference_nodes(&mut budget).unwrap();
        let peak = 6 * flow.nodes() + 2 * references;
        retained.resize(MAX_STORAGE - peak).unwrap();
        assert_eq!(flow.join(&flow, &mut budget).unwrap(), flow);
        retained.resize(MAX_STORAGE - peak + 1).unwrap();
        assert!(flow.join(&flow, &mut budget).is_err());
    }

    #[test]
    fn private_scalar_capture_storage_nested_operand_reservations_release_on_error() {
        fn transfer(budget: &mut Budget, flow: &Flow) -> Result<()> {
            let _fields = budget.reserve(3)?;
            let _operand = budget.reserve(1)?;
            flow.copy_value(0, budget)?;
            Ok(())
        }
        let flow = flow();
        let mut budget = budget();
        let retained_nodes = MAX_STORAGE - flow.nodes() - 4;
        let _retained = budget.reserve(retained_nodes).unwrap();
        assert!(transfer(&mut budget, &flow).is_err());
        assert_eq!(budget.storage.get(), retained_nodes);
        assert_eq!(
            flow.copy_value(0, &mut budget).unwrap(),
            Some(Value::Opaque)
        );
    }

    #[test]
    fn private_scalar_capture_reference_roster_and_empty_fields_consume_work() {
        let value = Value::Fields(vec![None; MAX_FIELDS]);
        assert!(value.reference_nodes(&mut Budget::new(MAX_FIELDS)).is_err());
        assert!(
            value
                .targets(&mut BTreeSet::new(), &mut Budget::new(MAX_FIELDS))
                .is_err()
        );
        assert_eq!(
            value.reference_nodes(&mut Budget::new(MAX_FIELDS + 1)),
            Ok(0)
        );
    }
}
