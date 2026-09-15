//! Preserve the leafwise join order while reusing exact incoming branch owners.
use super::*;

const LEVELS: usize = (u32::BITS - GROUP_SHIFT) as usize;
type Ancestors<'a> = [Option<&'a Rc<Node>>; LEVELS];

pub(super) struct SourceLeaves<'a> {
    ancestors: Ancestors<'a>,
    current: Option<&'a Rc<Node>>,
    occupied: u32,
    started: bool,
}

impl<'a> SourceLeaves<'a> {
    pub(super) fn new(root: &'a Rc<Node>) -> Self {
        Self {
            ancestors: [None; LEVELS],
            current: Some(root),
            occupied: 0,
            started: false,
        }
    }

    pub(super) fn next_leaf(&mut self, budget: &Budget) -> Result<Option<&'a Rc<Node>>> {
        let Some(mut current) = self.current else {
            return Ok(None);
        };
        if self.started {
            loop {
                if self.occupied == 0 {
                    self.current = None;
                    return Ok(None);
                }
                budget.work(1)?;
                let level = self.occupied.trailing_zeros() as usize;
                let parent = self.ancestors[level].expect("occupied source ancestor");
                let Kind::Branch { zero, one, .. } = &parent.kind else {
                    unreachable!()
                };
                if Rc::ptr_eq(current, zero) {
                    current = one;
                    break;
                }
                debug_assert!(Rc::ptr_eq(current, one));
                self.ancestors[level] = None;
                self.occupied &= !(1u32 << level);
                current = parent;
            }
        }
        loop {
            // Same node-visit debit and zero-before-one order as the old DFS.
            budget.work(1)?;
            match &current.kind {
                Kind::Leaf { .. } => {
                    self.current = Some(current);
                    self.started = true;
                    return Ok(Some(current));
                }
                Kind::Branch { zero, .. } => {
                    let bit = current.bit();
                    let level = bit.trailing_zeros() as usize;
                    debug_assert!(level < LEVELS && self.ancestors[level].is_none());
                    self.ancestors[level] = Some(current);
                    self.occupied |= bit;
                    current = zero;
                }
            }
        }
    }

    pub(super) fn insert(
        &self,
        root: &Rc<Node>,
        leaf: Rc<Node>,
        budget: &Budget,
    ) -> Result<Rc<Node>> {
        InsertContext {
            budget,
            ancestors: Some(&self.ancestors),
        }
        .insert(root, leaf)
    }
}

pub(super) fn insert(root: &Rc<Node>, leaf: Rc<Node>, budget: &Budget) -> Result<Rc<Node>> {
    InsertContext {
        budget,
        ancestors: None,
    }
    .insert(root, leaf)
}

// The recursive call retains one context pointer in place of the old budget
// pointer. Ancestor lookup is O(1); no per-level source search or heap index.
struct InsertContext<'a, 'tree> {
    budget: &'a Budget,
    ancestors: Option<&'a Ancestors<'tree>>,
}

impl InsertContext<'_, '_> {
    fn branch(&self, bit: u32, zero: Rc<Node>, one: Rc<Node>) -> Result<Rc<Node>> {
        if let Some(ancestors) = self.ancestors {
            self.budget.work(1)?;
            if let Some(candidate) = ancestors[bit.trailing_zeros() as usize] {
                let prefix = (zero.key() & !(bit - 1)) | bit;
                if matches!(&candidate.kind, Kind::Branch { prefix: found, zero: a, one: b }
                    if *found == prefix && Rc::ptr_eq(a, &zero) && Rc::ptr_eq(b, &one))
                {
                    // Keep the branch-update work debit, without allocating a
                    // second owner of the exact immutable branch already live.
                    self.budget.work(1)?;
                    return Ok(candidate.clone());
                }
            }
        }
        branch(bit, zero, one, self.budget)
    }

    fn insert(&self, root: &Rc<Node>, leaf: Rc<Node>) -> Result<Rc<Node>> {
        self.budget.work(1)?;
        let group = leaf.key();
        let difference = root.key() ^ group;
        let split = if difference == 0 {
            0
        } else {
            1u32 << (31 - difference.leading_zeros())
        };
        if split > root.bit() {
            return if group & split == 0 {
                self.branch(split, leaf, root.clone())
            } else {
                self.branch(split, root.clone(), leaf)
            };
        }
        match &root.kind {
            Kind::Leaf { .. } => Ok(leaf),
            Kind::Branch { zero, one, .. } => {
                let bit = root.bit();
                if group & bit == 0 {
                    self.branch(bit, self.insert(zero, leaf)?, one.clone())
                } else {
                    self.branch(bit, zero.clone(), self.insert(one, leaf)?)
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) const fn scratch_bytes() -> usize {
    size_of::<SourceLeaves<'_>>()
        + size_of::<InsertContext<'_, '_>>()
        + size_of::<Option<&SourceLeaves<'_>>>()
}
