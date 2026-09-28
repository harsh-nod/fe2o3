use super::*;
use std::convert::Infallible;

#[path = "private_slots_engine_v18.rs"]
pub(super) mod engine;

struct Legacy<'source, 'view> {
    source: &'source Function,
    definitions: &'view Definitions<'source>,
    types: &'source BTreeMap<ValueId, Type>,
    blocks: BTreeMap<BlockId, &'source crate::BasicBlock>,
}
impl<'source, 'view> Legacy<'source, 'view> {
    fn new(
        source: &'source Function,
        definitions: &'view Definitions<'source>,
        types: &'source BTreeMap<ValueId, Type>,
    ) -> Self {
        Self {
            source,
            definitions,
            types,
            blocks: source
                .body
                .as_ref()
                .expect("verified function is defined")
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect(),
        }
    }
}
impl<'source> engine::Environment<'source> for Legacy<'source, '_> {
    type Error = Infallible;
    fn source(&self) -> &'source Function {
        self.source
    }
    fn step(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn arithmetic(&self) -> Infallible {
        panic!("private-slot table size overflow")
    }
    fn reachable(&mut self, block: BlockId) -> Result<bool, Infallible> {
        Ok(self.definitions.is_reachable(block))
    }
    fn block(&mut self, block: BlockId) -> Result<Option<&'source crate::BasicBlock>, Infallible> {
        Ok(self.blocks.get(&block).copied())
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Infallible> {
        Ok(self.definitions.exact_ssa_origin(value, self.types))
    }
    fn ty(&mut self, value: ValueId) -> Result<Option<&'source Type>, Infallible> {
        Ok(self.types.get(&value))
    }
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool, Infallible> {
        Ok(left == right)
    }
    fn empty<T: Copy>(&mut self) -> Result<Vec<T>, Infallible> {
        Ok(Vec::new())
    }
    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>, Infallible> {
        Ok(vec![value; count])
    }
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), Infallible> {
        rows.push(value);
        Ok(())
    }
    fn sort<T: Copy>(
        &mut self,
        rows: &mut [T],
        key: impl Fn(&T) -> u32 + Copy,
    ) -> Result<(), Infallible> {
        rows.sort_by_key(key);
        Ok(())
    }
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl Fn(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>, Infallible> {
        Ok(rows.binary_search_by(compare).ok())
    }
}

pub(super) fn classify_eligible_private_slots(
    function: &Function,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
) -> BTreeSet<ValueId> {
    let slots = exact_origin_v18::infallible(engine::classify(&mut Legacy::new(
        function,
        definitions,
        value_types,
    )));
    let mut eligible = BTreeSet::new();
    for slot in slots {
        if let Some((location, pointer)) = slot.escape {
            reasons.insert(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location,
                pointer,
            });
        } else {
            eligible.insert(slot.value);
        }
    }
    eligible
}

pub(super) fn collect_private_load_sources(
    function: &Function,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
) -> BTreeMap<ValueId, ValueId> {
    let slots = eligible_private_slots
        .iter()
        .map(|value| engine::Slot {
            value: *value,
            location: definitions
                .operations
                .get(value)
                .expect("eligible slot has original allocation")
                .1,
            escape: None,
        })
        .collect::<Vec<_>>();
    exact_origin_v18::infallible(engine::loads(
        &mut Legacy::new(function, definitions, value_types),
        &slots,
    ))
    .into_iter()
    .map(|row| (row.value, row.source))
    .collect()
}
