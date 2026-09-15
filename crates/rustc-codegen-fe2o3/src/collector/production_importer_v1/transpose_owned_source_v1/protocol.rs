//! Complete original-source occurrence coverage. This ledger is not an issuer:
//! only the live source observer can supply an authenticated flow to its caller.
use super::{Error, Result, bounded};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Role {
    Issue,
    Stage,
    Publish,
}

pub(super) struct Roster<K> {
    expected: Vec<(Role, K)>,
    used: Vec<bool>,
}

impl<K: Copy + Ord> Roster<K> {
    pub(super) fn new(
        expected: impl IntoIterator<Item = (Role, K)>,
        work: &mut usize,
    ) -> Result<Self> {
        let mut ordered = Vec::new();
        for entry in expected {
            bounded::charge(work, 1)?;
            bounded::insert_unique(&mut ordered, entry, work)?;
        }
        let mut used = Vec::new();
        bounded::reserve(&mut used, ordered.len(), work)?;
        used.resize(ordered.len(), false);
        Ok(Self {
            expected: ordered,
            used,
        })
    }

    /// All three relationships publish atomically, after every original
    /// terminal occurrence has been matched. Error leaves this ledger intact.
    pub(super) fn consume_flow(
        &mut self,
        issue: K,
        stage: K,
        publish: K,
        work: &mut usize,
    ) -> Result<()> {
        if issue == stage || issue == publish || stage == publish {
            return Err(Error::Source("transpose occurrence roles collapsed"));
        }
        let mut found = [0; 3];
        for (slot, key) in [
            (Role::Issue, issue),
            (Role::Stage, stage),
            (Role::Publish, publish),
        ]
        .into_iter()
        .enumerate()
        {
            bounded::charge(work, bounded::search_work(self.expected.len()) + 1)?;
            let index = self
                .expected
                .binary_search(&key)
                .map_err(|_| Error::Source("transpose flow changed required source occurrence"))?;
            if self.used[index] {
                return Err(Error::Source("transpose source occurrence consumed twice"));
            }
            found[slot] = index;
        }
        for index in found {
            self.used[index] = true;
        }
        Ok(())
    }

    pub(super) fn finish(self, work: &mut usize) -> Result<()> {
        for used in self.used {
            bounded::charge(work, 1)?;
            if !used {
                return Err(Error::Source(
                    "transpose source occurrence roster incomplete",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "roster_tests.rs"]
mod roster_tests;
