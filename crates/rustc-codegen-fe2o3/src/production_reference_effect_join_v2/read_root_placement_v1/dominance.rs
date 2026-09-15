use super::*;

/// Checks producer dominance without moving or repeating a source memory event.
/// Each distinct excluded read block is traversed once under the placement budget.
pub(super) struct ReadDominance {
    successors: Vec<Vec<usize>>,
    reachable: Vec<bool>,
    without_read: BTreeMap<usize, Vec<bool>>,
}

impl ReadDominance {
    pub(super) fn new(kernel: &ProductionRankedKernelV1, work: &mut Work) -> Result<Self, E> {
        let blocks = kernel.blocks();
        work.charge(blocks.len())?;
        if blocks.is_empty() {
            return Err(E::WriteLocation);
        }
        let mut successors = Vec::with_capacity(blocks.len());
        for block in blocks {
            let targets = terminator_successors_v2(block.terminator());
            work.charge(targets.len())?;
            if targets.iter().any(|target| *target >= blocks.len()) {
                return Err(E::WriteLocation);
            }
            successors.push(targets);
        }
        let reachable = reachable_without(&successors, None, work)?;
        Ok(Self {
            successors,
            reachable,
            without_read: BTreeMap::new(),
        })
    }

    pub(super) fn require_preceding(
        &mut self,
        read: Site,
        write: Site,
        work: &mut Work,
    ) -> Result<(), E> {
        work.charge(1)?;
        if self.reachable.get(read.0) != Some(&true) || self.reachable.get(write.0) != Some(&true) {
            return Err(reject(
                "read-root placement requires reachable source and write sites",
            ));
        }
        if read.0 == write.0 {
            return if read.1 < write.1 {
                Ok(())
            } else {
                Err(reject(
                    "read-root placement requires the original read before the write",
                ))
            };
        }
        if !self.without_read.contains_key(&read.0) {
            let reachable = reachable_without(&self.successors, Some(read.0), work)?;
            self.without_read.insert(read.0, reachable);
        }
        // A reachable write is dominated by the read block exactly when no
        // entry-to-write path survives removing that read block, including cycles.
        if self.without_read[&read.0][write.0] {
            return Err(reject(
                "read-root placement requires each original read to dominate the write",
            ));
        }
        Ok(())
    }
}

fn reachable_without(
    successors: &[Vec<usize>],
    excluded: Option<usize>,
    work: &mut Work,
) -> Result<Vec<bool>, E> {
    work.charge(successors.len())?;
    let mut seen = vec![false; successors.len()];
    if excluded == Some(0) {
        return Ok(seen);
    }
    work.charge(successors.len())?;
    let mut pending = Vec::with_capacity(successors.len());
    seen[0] = true;
    pending.push(0);
    while let Some(block) = pending.pop() {
        work.charge(1)?;
        for target in &successors[block] {
            work.charge(1)?;
            if Some(*target) != excluded && !seen[*target] {
                seen[*target] = true;
                pending.push(*target);
            }
        }
    }
    Ok(seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_dominance_retains_cycles_and_caches_only_the_same_excluded_block() {
        let successors = vec![vec![1], vec![2], vec![1, 3], vec![]];
        let mut work = Work { used: 0 };
        let reachable = reachable_without(&successors, None, &mut work).unwrap();
        let mut graph = ReadDominance {
            successors,
            reachable,
            without_read: BTreeMap::new(),
        };
        graph.require_preceding((1, 0), (3, 0), &mut work).unwrap();
        let previous = work.used;
        graph.require_preceding((1, 0), (2, 0), &mut work).unwrap();
        assert_eq!(work.used, previous + 1);
        assert_eq!(graph.without_read.len(), 1);
        assert!(matches!(
            graph.require_preceding((2, 0), (1, 0), &mut work),
            Err(E::UnsupportedReference(
                "read-root placement requires each original read to dominate the write"
            ))
        ));
        assert_eq!(graph.without_read.len(), 2);
    }

    #[test]
    fn read_dominance_uses_the_existing_placement_work_ceiling() {
        let successors = vec![vec![1], vec![]];
        let mut work = Work {
            used: MAX_REFERENCE_STATEMENTS_V1 - 1,
        };
        assert!(matches!(reachable_without(&successors, None, &mut work),
            Err(E::Recipe(ProductionRankedKernelErrorV1::ResourceLimit {
                resource: "source read root placement work", limit, actual,
            })) if limit == MAX_REFERENCE_STATEMENTS_V1 && actual == limit + 1));
    }
}
