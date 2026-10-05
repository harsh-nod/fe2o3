//! Sparse boundaries with an append-only paid hash index. Every newly discovered
//! boundary visits only copies incident to its allocation; translated boundaries
//! reach a fixed point or an explicit structural refusal, never a byte expansion.
use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Boundary {
    pub(super) allocation: usize,
    pub(super) offset: u64,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Span {
    pub(super) start: usize,
    pub(super) length: usize,
}
impl Span {
    pub(super) fn range(self) -> R<std::ops::Range<usize>> {
        Ok(self.start..self.start.checked_add(self.length).ok_or_else(arithmetic)?)
    }
}

pub(super) struct Partitions {
    pub(super) boundaries: Vec<Boundary>,
    pub(super) allocations: Vec<Span>,
    pub(super) complete: bool,
}

impl Partitions {
    pub(super) fn range(
        &self,
        range: ByteRange,
        budget: &mut Budget<'_>,
    ) -> R<std::ops::Range<usize>> {
        charge(budget, 4)?;
        let span = *self
            .allocations
            .get(range.allocation)
            .ok_or_else(arithmetic)?;
        let rows = self.boundaries.get(span.range()?).ok_or_else(arithmetic)?;
        let find = |offset, budget: &mut Budget<'_>| -> R<usize> {
            let mut low = 0;
            let mut high = rows.len();
            while low < high {
                charge(budget, 4)?;
                let middle = low + (high - low) / 2;
                match rows[middle].offset.cmp(&offset) {
                    std::cmp::Ordering::Less => low = middle + 1,
                    std::cmp::Ordering::Greater => high = middle,
                    std::cmp::Ordering::Equal => {
                        return span.start.checked_add(middle).ok_or_else(arithmetic);
                    }
                }
            }
            Err(arithmetic())
        };
        Ok(find(range.start, budget)?..find(range.end()?, budget)?)
    }
}

struct BoundarySet {
    rows: Vec<Boundary>,
    slots: Vec<Option<usize>>,
}

fn hash(boundary: Boundary) -> u64 {
    let mut value = boundary.offset ^ (boundary.allocation as u64).wrapping_mul(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn grow<T: Copy>(rows: &mut Vec<T>, budget: &mut Budget<'_>) -> R<()> {
    charge(budget, 5)?;
    let capacity = rows
        .capacity()
        .max(4)
        .checked_mul(2)
        .ok_or_else(arithmetic)?;
    let mut next = scratch(capacity, budget)?;
    charge(budget, rows.len())?;
    next.extend_from_slice(rows);
    let bytes = size_of::<Vec<T>>()
        .checked_add(capacity_bytes(rows)?)
        .ok_or_else(arithmetic)?;
    let old = std::mem::replace(rows, next);
    drop(old);
    budget.release_storage(bytes)?;
    Ok(())
}

impl BoundarySet {
    fn new(budget: &mut Budget<'_>) -> R<Self> {
        Ok(Self {
            rows: scratch(0, budget)?,
            slots: filled(8, None, budget)?,
        })
    }

    fn slot(&self, boundary: Boundary, budget: &mut Budget<'_>) -> R<usize> {
        let mut slot = (hash(boundary) as usize) & (self.slots.len() - 1);
        for _ in 0..self.slots.len() {
            charge(budget, 5)?;
            match self.slots[slot] {
                Some(index) if self.rows[index] != boundary => {
                    slot = (slot + 1) & (self.slots.len() - 1)
                }
                _ => return Ok(slot),
            }
        }
        Err(arithmetic())
    }

    fn insert(&mut self, boundary: Boundary, maximum: usize, budget: &mut Budget<'_>) -> R<bool> {
        charge(budget, 5)?;
        let slot = self.slot(boundary, budget)?;
        if self.slots[slot].is_some() {
            return Ok(true);
        }
        if self.rows.len() == maximum {
            return Ok(false);
        }
        if self.rows.len().checked_add(1).ok_or_else(arithmetic)? > self.slots.len() / 2 {
            let count = self.slots.len().checked_mul(2).ok_or_else(arithmetic)?;
            let next = filled(count, None, budget)?;
            let bytes = size_of::<Vec<Option<usize>>>()
                .checked_add(capacity_bytes(&self.slots)?)
                .ok_or_else(arithmetic)?;
            let old = std::mem::replace(&mut self.slots, next);
            drop(old);
            budget.release_storage(bytes)?;
            for index in 0..self.rows.len() {
                charge(budget, 3)?;
                let slot = self.slot(self.rows[index], budget)?;
                self.slots[slot] = Some(index);
            }
        }
        let slot = self.slot(boundary, budget)?;
        if self.rows.len() == self.rows.capacity() {
            grow(&mut self.rows, budget)?;
        }
        self.slots[slot] = Some(self.rows.len());
        self.rows.push(boundary);
        Ok(true)
    }
}

#[derive(Clone, Copy)]
struct CopyEdge {
    source: ByteRange,
    destination: ByteRange,
    next: Option<usize>,
}

fn incomplete(
    census: &mut Census,
    allocations: Vec<Span>,
    budget: &mut Budget<'_>,
) -> R<Partitions> {
    for fact in &mut census.facts {
        charge(budget, 2)?;
        if fact.kind != OperationKindV38::None {
            fact.require(Obligation::CopyBoundaryClosure);
        }
    }
    Ok(Partitions {
        boundaries: scratch(0, budget)?,
        allocations,
        complete: false,
    })
}

pub(super) fn derive(
    census: &mut Census,
    maximum: usize,
    budget: &mut Budget<'_>,
) -> R<Partitions> {
    let mut partitions = filled(census.events.len(), Span::default(), budget)?;
    let mut boundaries = BoundarySet::new(budget)?;
    let mut heads = filled(census.events.len(), None, budget)?;
    let edge_count = census.events.len().checked_mul(2).ok_or_else(arithmetic)?;
    let mut edges = scratch::<CopyEdge>(edge_count, budget)?;
    for allocation in &census.allocations {
        charge(budget, 3)?;
        for offset in [0, allocation.extent] {
            if !boundaries.insert(
                Boundary {
                    allocation: allocation.operation,
                    offset,
                },
                maximum,
                budget,
            )? {
                return incomplete(census, partitions, budget);
            }
        }
    }
    for ordinal in 0..census.events.len() {
        charge(budget, 4)?;
        let event = census.events[ordinal];
        let ranges = match event {
            Event::Read(range) | Event::Write(range) => [Some(range), None],
            Event::Copy {
                source,
                destination,
            } => [Some(source), Some(destination)],
            _ => [None, None],
        };
        for range in ranges.into_iter().flatten() {
            for offset in [range.start, range.end()?] {
                if !boundaries.insert(
                    Boundary {
                        allocation: range.allocation,
                        offset,
                    },
                    maximum,
                    budget,
                )? {
                    return incomplete(census, partitions, budget);
                }
            }
        }
        if let Event::Copy {
            source,
            destination,
        } = event
        {
            for (source, destination) in [(source, destination), (destination, source)] {
                charge(budget, 4)?;
                let next = heads[source.allocation];
                heads[source.allocation] = Some(edges.len());
                edges.push(CopyEdge {
                    source,
                    destination,
                    next,
                });
            }
        }
    }
    let mut cursor = 0;
    while cursor < boundaries.rows.len() {
        charge(budget, 4)?;
        let boundary = boundaries.rows[cursor];
        cursor += 1;
        let mut edge = heads[boundary.allocation];
        while let Some(index) = edge {
            charge(budget, 7)?;
            let row = edges[index];
            edge = row.next;
            if boundary.offset < row.source.start || boundary.offset > row.source.end()? {
                continue;
            }
            let offset = row
                .destination
                .start
                .checked_add(boundary.offset - row.source.start)
                .ok_or_else(arithmetic)?;
            if !boundaries.insert(
                Boundary {
                    allocation: row.destination.allocation,
                    offset,
                },
                maximum,
                budget,
            )? {
                return incomplete(census, partitions, budget);
            }
        }
    }
    crate::canonical_kir_inventory_v1::heap_sort(
        &mut boundaries.rows,
        budget,
        |left, right, budget| {
            budget.charge_work(2)?;
            Ok(left.cmp(right))
        },
    )
    .map_err(Error::Inventory)?;
    for (index, boundary) in boundaries.rows.iter().enumerate() {
        charge(budget, 4)?;
        let span = &mut partitions[boundary.allocation];
        if span.length == 0 {
            span.start = index;
        }
        span.length = span.length.checked_add(1).ok_or_else(arithmetic)?;
    }
    Ok(Partitions {
        boundaries: boundaries.rows,
        allocations: partitions,
        complete: true,
    })
}

pub(super) fn headers() -> R<usize> {
    header_sum(&[
        h::<BoundarySet>()?,
        h::<Partitions>()?,
        h::<Span>()?,
        h::<Boundary>()?,
        h::<CopyEdge>()?,
        h::<&mut BoundarySet>()?,
        h::<&mut Census>()?,
        h::<&[Boundary]>()?,
        h::<&mut Vec<Boundary>>()?,
        h::<&mut Vec<Option<usize>>>()?,
        h::<std::slice::Iter<'_, Allocation>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, Boundary>>>()?,
        h::<std::array::IntoIter<u64, 2>>()?,
        h::<std::iter::Flatten<std::array::IntoIter<Option<ByteRange>, 2>>>()?,
        h::<std::array::IntoIter<(ByteRange, ByteRange), 2>>()?,
        h::<std::ops::Range<usize>>()?,
        h::<std::cmp::Ordering>()?,
        h::<[Option<ByteRange>; 2]>()?,
        h::<[u64; 2]>()?,
        h::<[(ByteRange, ByteRange); 2]>()?,
        header_copies::<usize>(22)?,
        header_copies::<Option<usize>>(5)?,
        header_copies::<u64>(5)?,
        header_copies::<bool>(3)?,
        h::<(&[Boundary], Span)>()?,
        h::<(Boundary, usize, &mut Budget<'_>)>()?,
        h::<(&mut Census, usize, &mut Budget<'_>)>()?,
    ])
}
