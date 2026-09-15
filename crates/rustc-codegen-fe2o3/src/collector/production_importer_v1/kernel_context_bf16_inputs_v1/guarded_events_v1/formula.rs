//! Closed unsigned source-event formulas. No load symbols or memory equality.

#[path = "guarded_addresses.rs"]
mod guarded_addresses;

pub(super) fn check_guarded_addresses(p: &Formula, f: Charge<'_>) -> Result<()> {
    guarded_addresses::check(p, f)
}

pub(super) const INPUTS: usize = 8;
pub(super) const NODES: usize = 256;
pub(super) const CONDITIONS: usize = 12;
pub(super) type Id = u16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Compare {
    Less,
    LessEqual,
    Equal,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Node {
    Input(u8),
    Word(u64),
    Bool(bool),
    Binary(Binary, Id, Id),
    Compare(Compare, Id, Id),
    Select {
        boolean: bool,
        condition: Id,
        yes: Id,
        no: Id,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Budget,
    Capacity,
    Source,
    Graph,
    Changed,
    Roster,
}
pub(super) type Result<T> = std::result::Result<T, Error>;

pub(super) struct Formula {
    nodes: [Node; NODES],
    len: usize,
    pub(super) events: [Event; 4],
    pub(super) precondition: Id,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Event {
    pub(super) index: Id,
    pub(super) guard: Id,
}
pub(super) type Charge<'a> = &'a mut dyn FnMut(usize) -> bool;
pub(super) fn charge(f: Charge<'_>, n: usize) -> Result<()> {
    if f(n) { Ok(()) } else { Err(Error::Budget) }
}

impl Formula {
    fn new(f: Charge<'_>) -> Result<Self> {
        charge(
            f,
            std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>()),
        )?;
        let mut result = Self {
            nodes: [Node::Word(0); NODES],
            len: 0,
            events: [Event { index: 0, guard: 0 }; 4],
            precondition: 0,
        };
        for input in 0..INPUTS {
            result.push(Node::Input(input as u8), f)?;
        }
        Ok(result)
    }
    pub(super) fn node(&self, id: Id) -> Result<Node> {
        self.nodes
            .get(id as usize)
            .copied()
            .filter(|_| (id as usize) < self.len)
            .ok_or(Error::Graph)
    }
    pub(super) fn len(&self) -> usize {
        self.len
    }
    fn push(&mut self, node: Node, f: Charge<'_>) -> Result<Id> {
        charge(f, 1)?;
        let id = self.len;
        if id == NODES {
            return Err(Error::Capacity);
        }
        self.nodes[id] = node;
        self.len += 1;
        Ok(id as Id)
    }
    fn word(&mut self, value: u64, f: Charge<'_>) -> Result<Id> {
        self.push(Node::Word(value), f)
    }
    fn binary(&mut self, op: Binary, a: Id, b: Id, f: Charge<'_>) -> Result<Id> {
        self.push(Node::Binary(op, a, b), f)
    }
    fn compare(&mut self, op: Compare, a: Id, b: Id, f: Charge<'_>) -> Result<Id> {
        self.push(Node::Compare(op, a, b), f)
    }
    fn fits_add(&mut self, a: Id, b: Id, f: Charge<'_>) -> Result<Id> {
        let max = self.word(u64::MAX, f)?;
        let room = self.binary(Binary::Subtract, max, a, f)?;
        self.compare(Compare::LessEqual, b, room, f)
    }
    fn fits_mul(&mut self, a: Id, b: Id, f: Charge<'_>) -> Result<Id> {
        // A total expression: even eager evaluation divides by at least one.
        let zero = self.word(0, f)?;
        let one = self.word(1, f)?;
        let max = self.word(u64::MAX, f)?;
        let is_zero = self.compare(Compare::Equal, b, zero, f)?;
        let divisor = self.push(
            Node::Select {
                boolean: false,
                condition: is_zero,
                yes: one,
                no: b,
            },
            f,
        )?;
        let bound = self.binary(Binary::Divide, max, divisor, f)?;
        let fits = self.compare(Compare::LessEqual, a, bound, f)?;
        let yes = self.push(Node::Bool(true), f)?;
        self.push(
            Node::Select {
                boolean: true,
                condition: is_zero,
                yes,
                no: fits,
            },
            f,
        )
    }
    fn conjunction(&mut self, values: &[Id], f: Charge<'_>) -> Result<Id> {
        let Some((&first, rest)) = values.split_first() else {
            return Err(Error::Graph);
        };
        let mut result = first;
        let no = self.push(Node::Bool(false), f)?;
        for &next in rest {
            result = self.push(
                Node::Select {
                    boolean: true,
                    condition: result,
                    yes: next,
                    no,
                },
                f,
            )?;
        }
        Ok(result)
    }
}

// Inputs: lane, offset, rows, columns, stride, first base, second base, extent.
// The compiler attaches every leaf to a retained source operand or typed issuer;
// these formulas alone never authenticate those inputs.
pub(super) fn reference(role_b: bool, f: Charge<'_>) -> Result<Formula> {
    use Binary::*;
    let mut p = Formula::new(f)?;
    let c16 = p.word(16, f)?;
    let c4 = p.word(4, f)?;
    let c64 = p.word(64, f)?;
    p.precondition = p.compare(Compare::Less, 0, c64, f)?;
    let lane_minor = p.binary(Remainder, 0, c16, f)?;
    let group = p.binary(Divide, 0, c16, f)?;
    let group = p.binary(Multiply, group, c4, f)?;
    let (minor_base, reduction_base) = if role_b { (6, 5) } else { (5, 6) };
    let minor = p.binary(Add, minor_base, lane_minor, f)?;
    let reduction = p.binary(Add, reduction_base, group, f)?;
    let minor_ok = p.fits_add(minor_base, lane_minor, f)?;
    let reduction_ok = p.fits_add(reduction_base, group, f)?;
    for component in 0..4 {
        let c = p.word(component as u64, f)?;
        let k = p.binary(Add, reduction, c, f)?;
        let k_ok = p.fits_add(reduction, c, f)?;
        let (row, column) = if role_b { (k, minor) } else { (minor, k) };
        let row_ok = p.compare(Compare::Less, row, 2, f)?;
        let column_ok = p.compare(Compare::Less, column, 3, f)?;
        let scaled = p.binary(Multiply, row, 4, f)?;
        let scaled_ok = p.fits_mul(row, 4, f)?;
        let start = p.binary(Add, 1, scaled, f)?;
        let start_ok = p.fits_add(1, scaled, f)?;
        let index = p.binary(Add, start, column, f)?;
        let index_ok = p.fits_add(start, column, f)?;
        let physical_ok = p.compare(Compare::Less, index, 7, f)?;
        let guard = p.conjunction(
            &[
                minor_ok,
                reduction_ok,
                k_ok,
                row_ok,
                column_ok,
                scaled_ok,
                start_ok,
                index_ok,
                physical_ok,
            ],
            f,
        )?;
        p.events[component] = Event { index, guard };
    }
    Ok(p)
}

/// Production read projection follows the checked address construction, not
/// the reference builder. No read may be merged, even at a coincident address.
pub(super) fn projected(role_b: bool, f: Charge<'_>) -> Result<Formula> {
    use Binary::*;
    let mut p = Formula::new(f)?;
    let wave = p.word(64, f)?;
    p.precondition = p.compare(Compare::Less, 0, wave, f)?;
    let divisor = p.word(16, f)?;
    let group = p.binary(Divide, 0, divisor, f)?;
    let factor = p.word(4, f)?;
    let group = p.binary(Multiply, group, factor, f)?;
    let low = p.binary(Remainder, 0, divisor, f)?;
    let base = if role_b { 6 } else { 5 };
    let reduction_base = if role_b { 5 } else { 6 };
    let low_ok = p.fits_add(base, low, f)?;
    let low = p.binary(Add, base, low, f)?;
    let first_ok = p.fits_add(reduction_base, group, f)?;
    let first = p.binary(Add, reduction_base, group, f)?;
    for component in 0..4 {
        let mut conditions = [0; CONDITIONS];
        conditions[0] = low_ok;
        conditions[1] = first_ok;
        let component_value = p.word(component as u64, f)?;
        conditions[2] = p.fits_add(first, component_value, f)?;
        let reduction = p.binary(Add, first, component_value, f)?;
        let (row, column) = if role_b {
            (reduction, low)
        } else {
            (low, reduction)
        };
        conditions[3] = p.compare(Compare::Less, row, 2, f)?;
        conditions[4] = p.compare(Compare::Less, column, 3, f)?;
        conditions[5] = p.fits_mul(row, 4, f)?;
        let row_offset = p.binary(Multiply, row, 4, f)?;
        conditions[6] = p.fits_add(1, row_offset, f)?;
        let offset = p.binary(Add, 1, row_offset, f)?;
        conditions[7] = p.fits_add(offset, column, f)?;
        let index = p.binary(Add, offset, column, f)?;
        conditions[8] = p.compare(Compare::Less, index, 7, f)?;
        let guard = p.conjunction(&conditions[..9], f)?;
        p.events[component] = Event { index, guard };
    }
    Ok(p)
}

#[cfg(test)]
pub(super) fn evaluate(p: &Formula, input: [u64; INPUTS]) -> Result<[(bool, u64); 4]> {
    let mut values = [0u64; NODES];
    for i in 0..p.len() {
        let get = |id: Id| {
            values
                .get(id as usize)
                .copied()
                .filter(|_| (id as usize) < i)
                .ok_or(Error::Graph)
        };
        values[i] = match p.node(i as Id)? {
            Node::Input(slot) => *input.get(slot as usize).ok_or(Error::Graph)?,
            Node::Word(value) => value,
            Node::Bool(value) => u64::from(value),
            Node::Binary(op, a, b) => {
                let (a, b) = (get(a)?, get(b)?);
                match op {
                    Binary::Add => a.wrapping_add(b),
                    Binary::Subtract => a.wrapping_sub(b),
                    Binary::Multiply => a.wrapping_mul(b),
                    Binary::Divide => a.checked_div(b).ok_or(Error::Graph)?,
                    Binary::Remainder => a.checked_rem(b).ok_or(Error::Graph)?,
                }
            }
            Node::Compare(op, a, b) => {
                let (a, b) = (get(a)?, get(b)?);
                u64::from(match op {
                    Compare::Less => a < b,
                    Compare::LessEqual => a <= b,
                    Compare::Equal => a == b,
                })
            }
            Node::Select {
                condition, yes, no, ..
            } => {
                if get(condition)? != 0 {
                    get(yes)?
                } else {
                    get(no)?
                }
            }
        };
    }
    Ok(p.events.map(|event| {
        (
            values[event.guard as usize] != 0,
            values[event.index as usize],
        )
    }))
}
