// Conditional unsigned address theorem only, not memory-value equality.
use super::*;

include!("guarded_addresses/sparse_facts.rs");

#[derive(Clone, Copy)]
struct Range {
    min: u64,
    max: u64,
}
const WORD: Range = Range {
    min: 0,
    max: u64::MAX,
};

fn word(p: &Formula, id: Id, value: u64) -> bool {
    matches!(p.node(id), Ok(Node::Word(actual)) if actual == value)
}
fn boolean(p: &Formula, id: Id, value: bool) -> bool {
    matches!(p.node(id), Ok(Node::Bool(actual)) if actual == value)
}

fn conjuncts(p: &Formula, root: Id, facts: &mut Facts, f: Charge<'_>) -> Result<()> {
    let mut pending = [false; NODES];
    if usize::from(root) >= p.len() {
        return Err(Error::Graph);
    }
    pending[usize::from(root)] = true;
    for index in (0..p.len()).rev() {
        charge(f, 1)?;
        if !pending[index] {
            continue;
        }
        match p.node(index as Id)? {
            Node::Select {
                boolean: true,
                condition,
                yes,
                no,
            } if boolean(p, no, false) => {
                if usize::from(condition) >= index
                    || usize::from(yes) >= index
                    || usize::from(no) >= index
                {
                    return Err(Error::Graph);
                }
                pending[usize::from(condition)] = true;
                pending[usize::from(yes)] = true;
            }
            _ => {
                charge(f, 1)?;
                facts.push_descending(index as Id)?;
            }
        }
    }
    Ok(())
}

fn fits_add(p: &Formula, id: Id, a: Id, b: Id) -> bool {
    let Ok(Node::Compare(Compare::LessEqual, compared, room)) = p.node(id) else {
        return false;
    };
    compared == b
        && matches!(p.node(room), Ok(Node::Binary(Binary::Subtract, max, other))
        if other == a && word(p, max, u64::MAX))
}

fn fits_multiply(p: &Formula, id: Id, a: Id, b: Id) -> bool {
    let Ok(Node::Select {
        boolean: true,
        condition,
        yes,
        no,
    }) = p.node(id)
    else {
        return false;
    };
    if !boolean(p, yes, true)
        || !matches!(p.node(condition), Ok(Node::Compare(Compare::Equal, tested, zero)) if tested == b && word(p, zero, 0))
    {
        return false;
    }
    let Ok(Node::Compare(Compare::LessEqual, tested, quotient)) = p.node(no) else {
        return false;
    };
    let Ok(Node::Binary(Binary::Divide, max, divisor)) = p.node(quotient) else {
        return false;
    };
    tested == a
        && word(p, max, u64::MAX)
        && matches!(p.node(divisor), Ok(Node::Select { boolean: false, condition: checked, yes: one, no: original })
            if checked == condition && original == b && word(p, one, 1))
}

fn guarded(
    p: &Formula,
    facts: &Facts,
    a: Id,
    b: Id,
    multiply: bool,
    f: Charge<'_>,
) -> Result<bool> {
    for id in facts.ascending() {
        charge(f, 1)?;
        // The closed predicate match visits at most nine formula nodes.
        charge(f, if multiply { 9 } else { 3 })?;
        if if multiply {
            fits_multiply(p, id as Id, a, b)
        } else {
            fits_add(p, id as Id, a, b)
        } {
            return Ok(true);
        }
    }
    Ok(false)
}

fn range(p: &Formula, index: Id, facts: &Facts, f: Charge<'_>) -> Result<()> {
    let mut needed = [false; NODES];
    let mut ranges = [None; NODES];
    if usize::from(index) >= p.len() {
        return Err(Error::Graph);
    }
    needed[usize::from(index)] = true;
    for current in (0..=usize::from(index)).rev() {
        charge(f, 1)?;
        if !needed[current] {
            continue;
        }
        match p.node(current as Id)? {
            Node::Input(slot) if usize::from(slot) < INPUTS => {}
            Node::Word(_) => {}
            Node::Binary(_, a, b) if usize::from(a) < current && usize::from(b) < current => {
                needed[usize::from(a)] = true;
                needed[usize::from(b)] = true;
            }
            _ => return Err(Error::Changed),
        }
    }
    for current in 0..=usize::from(index) {
        charge(f, 1)?;
        if !needed[current] {
            continue;
        }
        let value = match p.node(current as Id)? {
            Node::Input(_) => WORD,
            Node::Word(value) => Range {
                min: value,
                max: value,
            },
            Node::Binary(operation, a, b) => {
                let x: Range = ranges[usize::from(a)].ok_or(Error::Graph)?;
                let y: Range = ranges[usize::from(b)].ok_or(Error::Graph)?;
                match operation {
                    Binary::Add => {
                        if let (Some(min), Some(max)) =
                            (x.min.checked_add(y.min), x.max.checked_add(y.max))
                        {
                            Range { min, max }
                        } else if guarded(p, facts, a, b, false, f)? {
                            WORD
                        } else {
                            return Err(Error::Changed);
                        }
                    }
                    Binary::Multiply => {
                        if let (Some(min), Some(max)) =
                            (x.min.checked_mul(y.min), x.max.checked_mul(y.max))
                        {
                            Range { min, max }
                        } else if guarded(p, facts, a, b, true, f)? {
                            WORD
                        } else {
                            return Err(Error::Changed);
                        }
                    }
                    Binary::Subtract if x.min >= y.max => Range {
                        min: x.min - y.max,
                        max: x.max - y.min,
                    },
                    Binary::Divide if y.min > 0 => Range {
                        min: x.min / y.max,
                        max: x.max / y.min,
                    },
                    Binary::Remainder if y.min > 0 => Range {
                        min: 0,
                        max: x.max.min(y.max - 1),
                    },
                    _ => return Err(Error::Changed),
                }
            }
            _ => return Err(Error::Changed),
        };
        ranges[current] = Some(value);
    }
    Ok(())
}

pub(super) fn check(p: &Formula, f: Charge<'_>) -> Result<()> {
    // Fixed scratch arrays are charged before use; this is logical storage.
    charge(
        f,
        (3 * std::mem::size_of::<[bool; NODES]>()
            + std::mem::size_of::<Facts>()
            + std::mem::size_of::<[Option<Range>; NODES]>())
            .div_ceil(std::mem::size_of::<usize>()),
    )?;
    if p.len() > NODES || !matches!(p.node(7), Ok(Node::Input(7))) {
        return Err(Error::Graph);
    }
    // Reject forward/cyclic edges in the entire retained recipe, including
    // overflow witnesses that need not occur in the final address expression.
    let mut booleans = [false; NODES];
    for i in 0..p.len() {
        charge(f, 4)?;
        let before = |id: Id| usize::from(id) < i;
        let boolean = |id: Id| before(id) && booleans[usize::from(id)];
        let word = |id: Id| before(id) && !booleans[usize::from(id)];
        let valid = match p.node(i as Id)? {
            Node::Input(slot) => usize::from(slot) < INPUTS && usize::from(slot) == i,
            Node::Word(_) | Node::Bool(_) => true,
            Node::Binary(_, a, b) | Node::Compare(_, a, b) => word(a) && word(b),
            Node::Select {
                boolean: result,
                condition,
                yes,
                no,
            } => {
                boolean(condition)
                    && if result {
                        boolean(yes) && boolean(no)
                    } else {
                        word(yes) && word(no)
                    }
            }
        };
        if !valid {
            return Err(Error::Graph);
        }
        booleans[i] = matches!(
            p.node(i as Id)?,
            Node::Bool(_) | Node::Compare(..) | Node::Select { boolean: true, .. }
        );
    }
    for event in p.events {
        if usize::from(event.index) >= p.len()
            || usize::from(event.guard) >= p.len()
            || booleans[usize::from(event.index)]
            || !booleans[usize::from(event.guard)]
        {
            return Err(Error::Graph);
        }
        let mut facts = Facts::new();
        conjuncts(p, event.guard, &mut facts, f)?;
        let mut extent = false;
        for id in facts.ascending() {
            charge(f, 1)?;
            extent |= matches!(p.node(id), Ok(Node::Compare(Compare::Less, actual, 7)) if actual == event.index);
        }
        if !extent {
            return Err(Error::Changed);
        }
        range(p, event.index, &facts, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "guarded_addresses_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guarded_addresses/sparse_facts_tests.rs"]
mod sparse_tests;
