use super::{Error, Result, bounded::charge};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceUse {
    StageTile,
    PublishTile,
    PublishWorkgroup,
    SharedWorkgroup,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BindingRole {
    IssuedTile,
    StagedTile,
    Workgroup,
}

#[derive(Default)]
pub(super) struct Uses {
    stage: usize,
    publish: usize,
    workgroup: usize,
    shared: usize,
}

impl Uses {
    pub(super) fn observe(
        &mut self,
        binding: BindingRole,
        usage: SourceUse,
        work: &mut usize,
    ) -> Result<()> {
        charge(work, 1)?;
        let slot = match (binding, usage) {
            (BindingRole::IssuedTile, SourceUse::StageTile) => &mut self.stage,
            (BindingRole::StagedTile, SourceUse::PublishTile) => &mut self.publish,
            (BindingRole::Workgroup, SourceUse::PublishWorkgroup) => &mut self.workgroup,
            (BindingRole::Workgroup, SourceUse::SharedWorkgroup) => &mut self.shared,
            _ => return Err(Error::Source("owned binding has an unmodeled source use")),
        };
        *slot = slot.checked_add(1).ok_or(Error::Work)?;
        if usage != SourceUse::SharedWorkgroup && *slot != 1 {
            return Err(Error::Source("owned binding is consumed more than once"));
        }
        Ok(())
    }

    pub(super) fn finish(self, shared_receivers: usize) -> Result<()> {
        if (self.stage, self.publish, self.workgroup) != (1, 1, 1)
            || self.shared != shared_receivers
        {
            return Err(Error::Source("owned source-use roster is incomplete"));
        }
        Ok(())
    }
}

/// The finite producer -> consumer route observed in the retained source.
/// Every incoming edge, including unreachable and unwind edges, participates.
pub(super) fn immediate_normal_edge(
    producer: u32,
    consumer: u32,
    producer_target: u32,
    consumer_target: u32,
    consumer_statements: usize,
    predecessors: impl IntoIterator<Item = u32>,
    work: &mut usize,
) -> Result<()> {
    charge(work, 1)?;
    if producer == consumer
        || producer_target != consumer
        || consumer_target == consumer
        || consumer_target == producer
        || consumer_statements != 0
    {
        return Err(Error::Source(
            "Publish is not the exact immediate normal-return use",
        ));
    }
    let mut count = 0usize;
    for predecessor in predecessors {
        charge(work, 1)?;
        if predecessor != producer {
            return Err(Error::Source("Publish has a bypass or reentry predecessor"));
        }
        count = count.checked_add(1).ok_or(Error::Work)?;
    }
    if count != 1 {
        return Err(Error::Source("Publish normal predecessor is not unique"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LocalAction {
    Define,
    Move,
    Copy,
    SharedBorrow,
    Other,
}

/// Complete raw-local visitor output, not a scan of selected instructions.
pub(super) fn exact_local_actions(
    actual: impl IntoIterator<Item = (u32, usize, LocalAction)>,
    expected: &[(u32, usize, LocalAction)],
    work: &mut usize,
) -> Result<()> {
    let mut seen = Vec::new();
    super::bounded::reserve(&mut seen, expected.len(), work)?;
    seen.resize(expected.len(), false);
    for action in actual {
        charge(work, 1)?;
        let mut found = None;
        for (index, candidate) in expected.iter().enumerate() {
            charge(work, 1)?;
            if *candidate == action && !seen[index] {
                found = Some(index);
                break;
            }
        }
        let index = found.ok_or(Error::Source(
            "retained owned local has an extra or changed use",
        ))?;
        seen[index] = true;
    }
    if seen.iter().any(|v| !v) {
        return Err(Error::Source("retained owned local lost a required use"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
