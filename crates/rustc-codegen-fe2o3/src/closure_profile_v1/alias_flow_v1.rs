//! Exact local alias discovery; the caller owns the shared work budget.

use super::*;

/// Returns direct value edges after checking single writes and acyclic forwarding.
pub(super) fn value_aliases(
    body: &Body<'_>,
    typed_closure_locals: &BTreeSet<Local>,
    charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
    let mut graph = AliasGraph::default();
    for block in body.basic_blocks.iter() {
        charge(1)?;
        for statement in &block.statements {
            charge(1)?;
            let Some((destination, value)) = statement.kind.as_assign() else {
                continue;
            };
            let Some(destination) = destination.as_local() else {
                continue;
            };
            if !typed_closure_locals.contains(&destination) {
                continue;
            }
            let source = match value {
                Rvalue::Use(Operand::Copy(source) | Operand::Move(source)) => source
                    .as_local()
                    .filter(|local| typed_closure_locals.contains(local)),
                _ => None,
            };
            graph.record_write(destination, source, charge)?;
        }
    }
    graph.finish_values(typed_closure_locals, charge)
}

/// Flattens exact value/reference edges using this body's direct `value_aliases` map.
pub(super) fn reference_aliases(
    body: &Body<'_>,
    roots: &BTreeSet<Local>,
    value_aliases: &BTreeMap<Local, Local>,
    charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
    let mut graph = AliasGraph::default();
    for block in body.basic_blocks.iter() {
        charge(1)?;
        for statement in &block.statements {
            charge(1)?;
            let Some((destination, value)) = statement.kind.as_assign() else {
                continue;
            };
            let Some(destination) = destination.as_local() else {
                continue;
            };
            let source = match value {
                Rvalue::Ref(_, _, source)
                | Rvalue::Use(Operand::Copy(source) | Operand::Move(source)) => source.as_local(),
                _ => None,
            };
            graph.record_write(destination, source, charge)?;
        }
    }
    // Rescanning value edges checks their writes together with reference writes.
    graph.resolve(roots, value_aliases, charge)
}

struct AliasWrite {
    source: Option<Local>,
    repeated: bool,
}

#[derive(Default)]
struct AliasGraph {
    writes: BTreeMap<Local, AliasWrite>,
    outgoing: BTreeMap<Local, BTreeSet<Local>>,
}

impl AliasGraph {
    fn record_write(
        &mut self,
        destination: Local,
        source: Option<Local>,
        charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
    ) -> Result<(), ClosureProfileErrorV1> {
        charge(1)?;
        self.writes
            .entry(destination)
            .and_modify(|write| write.repeated = true)
            .or_insert(AliasWrite {
                source,
                repeated: false,
            });
        if let Some(source) = source {
            charge(2)?;
            self.outgoing.entry(source).or_default().insert(destination);
        }
        Ok(())
    }

    fn finish_values(
        &self,
        typed_closure_locals: &BTreeSet<Local>,
        charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
    ) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
        let mut aliases = BTreeMap::new();
        let mut roots = BTreeSet::new();
        for &local in typed_closure_locals {
            charge(1)?;
            let write = self.writes.get(&local);
            if write.is_some_and(|write| write.repeated) {
                return Err(ClosureProfileErrorV1::new(
                    "closure value forwarding local is assigned more than once",
                ));
            }
            if let Some(source) = write.and_then(|write| write.source) {
                if local.as_usize() == 0 {
                    return Err(ClosureProfileErrorV1::new(
                        "returning a forwarded closure escapes its environment",
                    ));
                }
                charge(1)?;
                aliases.insert(local, source);
            } else {
                charge(1)?;
                roots.insert(local);
            }
        }
        // These provisional roots still require the parent's origin/layout checks.
        self.resolve(&roots, &aliases, charge)?;
        Ok(aliases)
    }

    fn resolve(
        &self,
        roots: &BTreeSet<Local>,
        value_aliases: &BTreeMap<Local, Local>,
        charge: &mut impl FnMut(usize) -> Result<(), ClosureProfileErrorV1>,
    ) -> Result<BTreeMap<Local, Local>, ClosureProfileErrorV1> {
        let mut aliases = BTreeMap::new();
        let mut pending = BTreeSet::new();
        for &root in roots {
            charge(1)?;
            if self
                .writes
                .get(&root)
                .is_some_and(|write| write.repeated || write.source.is_some())
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure root is also an alias destination or is assigned more than once",
                ));
            }
            charge(1)?;
            pending.insert((root, root));
        }
        loop {
            charge(1)?;
            let Some((source, root)) = pending.pop_first() else {
                break;
            };
            for &destination in self.outgoing.get(&source).into_iter().flatten() {
                charge(1)?;
                if destination.as_usize() == 0 {
                    return Err(ClosureProfileErrorV1::new(
                        "returning a forwarded closure escapes its environment",
                    ));
                }
                if roots.contains(&destination)
                    || self
                        .writes
                        .get(&destination)
                        .is_some_and(|write| write.repeated)
                {
                    return Err(ClosureProfileErrorV1::new(
                        "closure receiver alias is assigned more than once",
                    ));
                }
                charge(2)?;
                if aliases.insert(destination, root).is_some() {
                    return Err(ClosureProfileErrorV1::new(
                        "closure receiver alias is assigned more than once",
                    ));
                }
                pending.insert((destination, root));
            }
        }
        // Only required value edges must resolve; ordinary reference cycles are ignored.
        for (&destination, &source) in value_aliases {
            charge(1)?;
            let root = aliases.get(&destination);
            charge(1)?;
            if root.is_none()
                || root != roots.get(&source).or_else(|| aliases.get(&source))
                || self.writes.get(&destination).and_then(|write| write.source) != Some(source)
            {
                return Err(ClosureProfileErrorV1::new(
                    "closure value alias cycle or missing exact closure root",
                ));
            }
        }
        Ok(aliases)
    }
}

#[cfg(test)]
#[path = "alias_flow_v1_tests.rs"]
mod tests;
