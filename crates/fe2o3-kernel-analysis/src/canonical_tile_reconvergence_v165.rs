//! Exact acyclic postdominance. Cyclic graphs retain the conservative V160 rule.
use super::*;

pub(super) struct Postdominance {
    parents: Vec<usize>,
}

impl Postdominance {
    pub(super) fn parent(&self, block: usize) -> Result<usize> {
        self.parents.get(block).copied().ok_or(Error::Inventory)
    }
}

struct Ancestors {
    levels: usize,
    depths: Vec<usize>,
    table: Vec<usize>,
}

impl Ancestors {
    fn at(&self, block: usize, level: usize) -> usize {
        self.table[block * self.levels + level]
    }

    fn join(&self, mut a: usize, mut b: usize, meter: &mut Meter<'_, '_, Error>) -> Result<usize> {
        if self.depths[a] < self.depths[b] {
            std::mem::swap(&mut a, &mut b);
        }
        let difference = self.depths[a] - self.depths[b];
        for level in 0..self.levels {
            meter.work(1)?;
            if difference & (1usize << level) != 0 {
                a = self.at(a, level);
            }
        }
        if a == b {
            return Ok(a);
        }
        for level in (0..self.levels).rev() {
            meter.work(1)?;
            if self.at(a, level) != self.at(b, level) {
                a = self.at(a, level);
                b = self.at(b, level);
            }
        }
        Ok(self.at(a, 0))
    }
}

pub(super) fn derive(
    graph: &Graph<'_, '_>,
    reachable: &[bool],
    meter: &mut Meter<'_, '_, Error>,
) -> Result<Option<Postdominance>> {
    meter.reserve(
        size_of::<Postdominance>() + size_of::<Ancestors>() + 3 * size_of::<Vec<usize>>(),
    )?;
    let mut incoming = filled(meter, graph.blocks, 0usize)?;
    let mut count = 0usize;
    for (block, live) in reachable.iter().copied().enumerate() {
        meter.work(1)?;
        if !live {
            continue;
        }
        count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
        for edge in graph.inventory.blocks()[graph.function.blocks.start + block]
            .edges
            .clone()
        {
            meter.work(1)?;
            let target = graph.target(edge)?;
            incoming[target] = incoming[target]
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
        }
    }
    let mut order = meter.table(graph.blocks)?.0;
    for (block, live) in reachable.iter().copied().enumerate() {
        meter.work(1)?;
        if live && incoming[block] == 0 {
            meter.push(&mut order, block)?;
        }
    }
    let mut head = 0;
    while head < order.len() {
        meter.work(1)?;
        let block = order[head];
        head += 1;
        for edge in graph.inventory.blocks()[graph.function.blocks.start + block]
            .edges
            .clone()
        {
            meter.work(1)?;
            let target = graph.target(edge)?;
            incoming[target] = incoming[target].checked_sub(1).ok_or(Error::Inventory)?;
            if incoming[target] == 0 {
                meter.push(&mut order, target)?;
            }
        }
    }
    if order.len() != count {
        return Ok(None);
    }
    // All terminal blocks reach one synthetic exit. A path that returns early
    // therefore prevents clearing divergence before a later real collective.
    let exit = graph.blocks;
    let nodes = exit.checked_add(1).ok_or(Resource::Arithmetic)?;
    let levels = (usize::BITS - nodes.leading_zeros()) as usize;
    let mut ancestors = Ancestors {
        levels,
        depths: filled(meter, nodes, 0usize)?,
        table: filled(
            meter,
            nodes.checked_mul(levels).ok_or(Resource::Arithmetic)?,
            exit,
        )?,
    };
    let mut parents = filled(meter, graph.blocks, exit)?;
    for &block in order.iter().rev() {
        meter.work(1)?;
        let mut parent = None;
        for edge in graph.inventory.blocks()[graph.function.blocks.start + block]
            .edges
            .clone()
        {
            meter.work(1)?;
            let target = graph.target(edge)?;
            parent = Some(match parent {
                None => target,
                Some(previous) => ancestors.join(previous, target, meter)?,
            });
        }
        let parent = parent.unwrap_or(exit);
        parents[block] = parent;
        ancestors.depths[block] = ancestors.depths[parent]
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?;
        ancestors.table[block * levels] = parent;
        for level in 1..levels {
            meter.work(1)?;
            ancestors.table[block * levels + level] =
                ancestors.at(ancestors.at(block, level - 1), level - 1);
        }
    }
    Ok(Some(Postdominance { parents }))
}
