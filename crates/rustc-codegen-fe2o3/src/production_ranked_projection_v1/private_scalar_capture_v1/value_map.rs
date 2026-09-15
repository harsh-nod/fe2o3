use super::*;
use std::ops::Deref;

#[cfg(test)]
#[path = "value_map_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ordered_join_tests.rs"]
mod ordered_join_tests;

// Logical scalar-key debit, matching partial-move path lookup accounting.
// This is not a bound on std BTreeMap comparisons, rebalancing, or CPU work.
fn key_work(entries: usize) -> usize {
    (usize::BITS - entries.max(1).leading_zeros()) as usize
}

// Only this wrapper mutates the map. Cached size is accounting, not a fact used
// in flow equality; Vec cloning can change capacity without changing facts.
#[derive(Debug, Default)]
pub(in super::super) struct Values {
    entries: BTreeMap<u32, Value>,
    nodes: usize,
}

impl Deref for Values {
    type Target = BTreeMap<u32, Value>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl PartialEq for Values {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl Eq for Values {}

impl From<BTreeMap<u32, Value>> for Values {
    fn from(entries: BTreeMap<u32, Value>) -> Self {
        let nodes = entries.values().map(|value| 1 + value.nodes()).sum();
        Self { entries, nodes }
    }
}

impl Clone for Values {
    fn clone(&self) -> Self {
        // Recompute from the clone, not the source's possibly larger capacity.
        Self::from(self.entries.clone())
    }
}

impl Values {
    pub(in super::super) fn join_ordered(
        &self,
        other: &Self,
        escaped: &mut BTreeSet<u32>,
        killed: &mut BTreeSet<u32>,
        budget: &mut Budget,
    ) -> Result<Self> {
        let mut joined = Self::default();
        let mut left = self.entries.iter().peekable();
        let mut right = other.entries.iter().peekable();
        // Each ordered cursor advances once per consumed key. Missing keys
        // still poison every lost target; final dead/escape invalidation is
        // performed by Flow::join after this complete inventory merge.
        while left.peek().is_some() || right.peek().is_some() {
            budget.charge(1)?;
            match (left.peek().copied(), right.peek().copied()) {
                (Some((&a, value)), Some((&b, incoming))) if a == b => {
                    budget.charge(1)?;
                    let value = value.join(incoming, escaped, budget)?;
                    joined.insert_new_metered(a, value, budget)?;
                    left.next();
                    right.next();
                }
                (Some((&a, value)), right_head)
                    if right_head.is_none_or(|(&b, _)| a < b) =>
                {
                    killed.insert(a);
                    value.targets(escaped, budget)?;
                    left.next();
                }
                (_, Some((&b, value))) => {
                    killed.insert(b);
                    value.targets(escaped, budget)?;
                    right.next();
                }
                (None, None) => break,
                _ => unreachable!("ordered scalar keys are equal, less or greater"),
            }
        }
        Ok(joined)
    }

    // The output merge contains unique keys. Entry performs one keyed search,
    // unlike get+insert; occupied entries reject before cache/map mutation.
    fn insert_new_metered(&mut self, local: u32, value: Value, budget: &mut Budget) -> Result<()> {
        let update = key_work(self.entries.len().checked_add(1).ok_or(())?);
        budget.charge(update)?;
        let std::collections::btree_map::Entry::Vacant(entry) = self.entries.entry(local) else {
            return Err(());
        };
        let incoming = 1usize.checked_add(value.nodes()).ok_or(())?;
        budget.charge(incoming)?;
        let nodes = self.nodes.checked_add(incoming).ok_or(())?;
        entry.insert(value);
        self.nodes = nodes;
        Ok(())
    }

    pub(in super::super) fn nodes(&self) -> usize {
        self.nodes
    }

    pub(in super::super) fn key_work(&self) -> usize {
        key_work(self.entries.len())
    }

    pub(in super::super) fn get_metered(
        &self,
        local: u32,
        budget: &mut Budget,
    ) -> Result<Option<&Value>> {
        budget.charge(self.key_work())?;
        Ok(self.entries.get(&local))
    }

    pub(in super::super) fn insert_metered(
        &mut self,
        local: u32,
        value: Value,
        budget: &mut Budget,
    ) -> Result<()> {
        // Separate old-value lookup and update; the latter may add one key.
        let update = key_work(self.entries.len().checked_add(1).ok_or(())?);
        budget.charge(self.key_work() + update)?;
        let incoming = 1 + value.nodes();
        let previous = self
            .entries
            .get(&local)
            .map_or(0, |value| 1 + value.nodes());
        budget.charge(incoming + previous)?;
        let nodes = self
            .nodes
            .checked_sub(previous)
            .and_then(|nodes| nodes.checked_add(incoming))
            .ok_or(())?;
        self.entries.insert(local, value);
        self.nodes = nodes;
        Ok(())
    }

    pub(super) fn remove_metered(&mut self, local: u32, budget: &mut Budget) -> Result<()> {
        budget.charge(self.key_work().checked_mul(2).ok_or(())?)?;
        let previous = self
            .entries
            .get(&local)
            .map_or(0, |value| 1 + value.nodes());
        budget.charge(previous + 1)?;
        let nodes = self.nodes.checked_sub(previous).ok_or(())?;
        self.entries.remove(&local);
        self.nodes = nodes;
        Ok(())
    }

    pub(super) fn invalidate(
        &mut self,
        targets: &BTreeSet<u32>,
        budget: &mut Budget,
    ) -> Result<bool> {
        let mut changed = false;
        for value in self.entries.values_mut() {
            // Value::invalidate only replaces a one-node Reference by Opaque.
            // It does not resize field trees, including on budget failure.
            changed |= value.invalidate(targets, budget)?;
        }
        Ok(changed)
    }

    #[cfg(test)]
    pub(in super::super) fn insert(&mut self, local: u32, value: Value) {
        self.insert_metered(local, value, &mut Budget::new(MAX_WORK))
            .unwrap();
    }

    #[cfg(test)]
    pub(in super::super) fn remove(&mut self, local: &u32) {
        self.remove_metered(*local, &mut Budget::new(MAX_WORK))
            .unwrap();
    }

    #[cfg(test)]
    pub(in super::super) fn edit(&mut self, local: u32, edit: impl FnOnce(&mut Value)) {
        let mut value = self.entries[&local].clone();
        edit(&mut value);
        self.insert(local, value);
    }
}
