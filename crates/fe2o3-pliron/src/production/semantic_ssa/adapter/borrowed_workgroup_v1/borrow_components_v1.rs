//! Shared all-use component walk. Repeated nodes retain the old fail-closed rule.
use super::*;

pub(super) fn members(
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    root: usize,
    budget: &mut Budget,
    extra_valid: impl Fn(usize) -> bool,
) -> Result<Result<BTreeSet<usize>, usize>, ProductionSemanticSsaErrorV1> {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        budget.charge(1)?;
        if !candidates[current].valid || !extra_valid(current) || !visited.insert(current) {
            return Ok(Err(current));
        }
        pending.extend(children[current].iter().copied());
    }
    Ok(Ok(visited))
}
