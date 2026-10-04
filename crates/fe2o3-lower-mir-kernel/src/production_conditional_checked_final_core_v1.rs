//! Shared I-to-F coverage over borrowed independently checked relations.
use super::*;
pub(super) fn coordinate(
    facts: &Facts<'_>,
    location: fe2o3_kernel_ir::FunctionOperationLocation,
    budget: &mut Budget<'_>,
) -> Result<Site> {
    tail::site(occurrences::coordinate(facts, location, budget)?).map_err(Error::Coordinate)
}

pub(super) fn coverage(
    checked: &impl occurrences::CheckedPrefix,
    history: &History<'_>,
    before: &Facts<'_>,
    after: &Facts<'_>,
    ordered: &mut [Read],
    output: &[Read],
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(
        checked
            .output()
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(history.output().canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !std::ptr::eq(before.module(), checked.output().module())
        || !std::ptr::eq(after.module(), history.output().module())
        || before.function_ordinal() != after.function_ordinal()
    {
        return Err(Error::Mismatch("actual I/F conditional subjects"));
    }
    // The admitted global readonly loads cannot be deleted by these fixed
    // engines (K deletes only scalar bitwise definitions; P/F copy only private
    // loads). Check complete cardinality AND lineage, not raw read SSA equality.
    occurrences::coverage_subjects(before, after, ordered, output)?;
    if follow(
        history,
        coordinate(before, before.store_location(), budget)?,
        budget,
    )? != coordinate(after, after.store_location(), budget)?
    {
        return Err(Error::Mismatch("final conditional store occurrence"));
    }
    for index in 0..ordered.len() {
        let original = ordered[index];
        let site = follow(
            history,
            coordinate(before, original.location(), budget)?,
            budget,
        )?;
        let mut found = None;
        for read in output {
            budget.charge_work(2)?;
            if coordinate(after, read.location(), budget)? == site && found.replace(*read).is_some()
            {
                return Err(Error::Mismatch("unique final read occurrence"));
            }
        }
        let read = found.ok_or(Error::Mismatch("missing final read occurrence"))?;
        for previous in &ordered[..index] {
            budget.charge_work(2)?;
            if previous.location() == read.location() {
                return Err(Error::Mismatch("complete one-to-one final reads"));
            }
        }
        occurrences::read_premises(original, read, budget)?;
        ordered[index] = read;
    }
    Ok(())
}

fn mapped<T>(
    input: Site,
    rows: &[T],
    project: impl Fn(&T) -> (Option<Site>, Option<Site>),
    budget: &mut Budget<'_>,
) -> Result<Site> {
    let mut found = None;
    for row in rows {
        budget.charge_work(4)?;
        let (from, to) = project(row);
        if from != Some(input) {
            continue;
        }
        let output = to.ok_or(Error::Mismatch("conditional memory operation rewritten"))?;
        if found.replace(output).is_some() {
            return Err(Error::Mismatch("unique surviving memory occurrence"));
        }
    }
    found.ok_or(Error::Mismatch("missing surviving memory occurrence"))
}

// Only a sealed complete-history receipt reaches this function. The independent
// engines establish operation semantics, including K's operand substitutions.
pub(super) fn follow(history: &History<'_>, input: Site, budget: &mut Budget<'_>) -> Result<Site> {
    let p8 = history.prefix();
    let j = mapped(
        input,
        p8.policy7_relation()
            .continuation()
            .relation()
            .retained_operations(),
        |r| (Some(r.input), Some(r.output)),
        budget,
    )?;
    let k = mapped(
        j,
        p8.continuation().claims().occurrences.operations,
        |r| {
            (
                match r.origin {
                    Origin::Retained(input) => Some(input),
                    _ => None,
                },
                Some(r.output),
            )
        },
        budget,
    )?;
    let p = mapped(
        k,
        history.promotion().origins(),
        |r| {
            (
                Some(r.input),
                (r.kind == Promotion::Retained).then_some(r.output),
            )
        },
        budget,
    )?;
    // Checked P/H subdivision appends empty blocks and preserves every original
    // block's operations and roster coordinate; no synthetic memory is possible.
    budget.charge_work(1)?;
    let l = mapped(
        p,
        history.licm().origins(),
        |r| (Some(r.input), r.hoist.is_none().then_some(r.output)),
        budget,
    )?;
    let r = mapped(
        l,
        history.refinement().origins(),
        |r| {
            (
                Some(r.input()),
                match *r {
                    Refinement::Unchanged { output, .. } => Some(output),
                    _ => None,
                },
            )
        },
        budget,
    )?;
    mapped(
        r,
        history.forwarding().origins(),
        |r| (Some(r.input), r.store.is_none().then_some(r.output)),
        budget,
    )
}
