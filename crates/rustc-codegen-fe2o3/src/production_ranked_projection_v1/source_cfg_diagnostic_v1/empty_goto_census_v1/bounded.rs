//! Error-only structural observations. No retained graph or admission result escapes.

use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Goto,
    Switch,
    Call,
    Assert,
    Drop,
    FalseEdge,
    Return,
    TailCall,
    UnwindResume,
    UnwindTerminate,
    Abort,
    Unreachable,
}

#[derive(Clone, Copy)]
pub(super) struct Header {
    pub empty: bool,
    pub kind: Kind,
}

#[derive(Clone, Copy)]
pub(super) struct Edge {
    pub target: usize,
    pub goto: bool,
}

pub(super) trait Source {
    fn len(&self) -> usize;
    fn entry(&self) -> usize;
    fn header(&self, block: usize) -> Header;
    fn edges(
        &self,
        block: usize,
        visit: &mut dyn FnMut(Edge) -> Result<(), Stop>,
    ) -> Result<(), Stop>;
}

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub work: usize,
    pub scratch_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stop {
    Work,
    Storage,
    Allocation,
    Malformed,
    IncompleteScan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Chains {
    Unavailable(Stop),
    // All-source, without induction/proof-site pins. Never ranked eligibility.
    StructuralOnly {
        single_entry_acyclic_interiors: usize,
        runs: usize,
        cycle_blocks_retained: usize,
        blocks_after_structural_only: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Census {
    pub declared_blocks: usize,
    pub entry: usize,
    pub visited_headers: usize,
    pub visited_edges: usize,
    pub empty_statements: usize,
    pub literal_empty_gotos: usize,
    // Kind discriminant order, documented by the adapter's bounded output.
    pub empty_by_kind: [usize; 12],
    pub scan_stop: Option<Stop>,
    pub chains: Chains,
    pub work: usize,
    pub dynamic_scratch_peak_bytes: usize,
}

struct Work {
    used: usize,
    limit: usize,
}
impl Work {
    fn one(&mut self) -> Result<(), Stop> {
        if self.used == self.limit {
            return Err(Stop::Work);
        }
        self.used += 1;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Node {
    next: usize,
    incoming: u8,
    incoming_goto: bool,
    literal: bool,
    // 0 unvisited literal, 1 active, 2 acyclic, 3 literal cycle, 4 pinned.
    color: u8,
}

pub(super) fn capture(source: &impl Source, limits: Limits) -> Census {
    let mut result = Census {
        declared_blocks: source.len(),
        entry: source.entry(),
        visited_headers: 0,
        visited_edges: 0,
        empty_statements: 0,
        literal_empty_gotos: 0,
        empty_by_kind: [0; 12],
        scan_stop: None,
        chains: Chains::Unavailable(Stop::IncompleteScan),
        work: 0,
        dynamic_scratch_peak_bytes: 0,
    };
    let mut work = Work {
        used: 0,
        limit: limits.work,
    };
    if result.entry >= result.declared_blocks {
        result.scan_stop = Some(Stop::Malformed);
        return result;
    }
    let scan = (|| {
        for block in 0..source.len() {
            work.one()?;
            let header = source.header(block);
            result.visited_headers += 1;
            if header.empty {
                result.empty_statements += 1;
                result.empty_by_kind[header.kind as usize] += 1;
            }
            let mut edge_count = 0usize;
            source.edges(block, &mut |edge| {
                work.one()?;
                result.visited_edges += 1;
                edge_count += 1;
                if edge.target >= source.len()
                    || (edge.goto && header.kind != Kind::Goto)
                    || (header.kind == Kind::Goto && !edge.goto)
                {
                    return Err(Stop::Malformed);
                }
                Ok(())
            })?;
            if header.kind == Kind::Goto {
                if edge_count != 1 {
                    return Err(Stop::Malformed);
                }
                result.literal_empty_gotos += usize::from(header.empty);
            }
        }
        Ok(())
    })();
    if let Err(stop) = scan {
        result.scan_stop = Some(stop);
    } else {
        result.chains = match chain_counts(
            source,
            limits,
            &mut work,
            &mut result.dynamic_scratch_peak_bytes,
        ) {
            Ok(chains) => chains,
            Err(stop) => Chains::Unavailable(stop),
        };
    }
    result.work = work.used;
    result
}

fn chain_counts(
    source: &impl Source,
    limits: Limits,
    work: &mut Work,
    peak: &mut usize,
) -> Result<Chains, Stop> {
    // One checked reservation; no adjacency copy, per-edge paths or DFS stack.
    // The Vec header is included; other diagnostic state is fixed-size stack data.
    let bytes = |capacity: usize| {
        capacity
            .checked_mul(size_of::<Node>())
            .and_then(|n| n.checked_add(size_of::<Vec<Node>>()))
            .ok_or(Stop::Storage)
    };
    let requested = bytes(source.len())?;
    if requested > limits.scratch_bytes {
        return Err(Stop::Storage);
    }
    work.one()?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(source.len())
        .map_err(|_| Stop::Allocation)?;
    *peak = bytes(nodes.capacity())?;
    if *peak > limits.scratch_bytes {
        return Err(Stop::Storage);
    }
    for _ in 0..source.len() {
        work.one()?;
        nodes.push(Node {
            next: usize::MAX,
            incoming: 0,
            incoming_goto: true,
            literal: false,
            color: 4,
        });
    }
    for block in 0..source.len() {
        work.one()?;
        let header = source.header(block);
        nodes[block].literal = header.empty && header.kind == Kind::Goto;
        let mut edges = 0usize;
        source.edges(block, &mut |edge| {
            work.one()?;
            if edge.target >= source.len()
                || (edge.goto && header.kind != Kind::Goto)
                || (header.kind == Kind::Goto && !edge.goto)
            {
                return Err(Stop::Malformed);
            }
            edges += 1;
            let incoming = &mut nodes[edge.target];
            incoming.incoming = incoming.incoming.saturating_add(1).min(2);
            incoming.incoming_goto &= edge.goto;
            if header.kind == Kind::Goto {
                nodes[block].next = edge.target;
            }
            Ok(())
        })?;
        if header.kind == Kind::Goto && edges != 1 {
            return Err(Stop::Malformed);
        }
    }
    for node in &mut nodes {
        work.one()?;
        if node.literal {
            node.color = 0;
        }
    }
    // Each active walk is finished before another starts. Following next links
    // a second time replaces a stack; each literal node is visited O(1) times.
    // Detect cycles BEFORE entry/indegree pins, so even entry-containing pure
    // empty-Goto cycles are retained, not reported as acyclic interiors.
    for start in 0..nodes.len() {
        work.one()?;
        if nodes[start].color != 0 {
            continue;
        }
        let mut current = start;
        loop {
            work.one()?;
            if nodes[current].color != 0 {
                break;
            }
            nodes[current].color = 1;
            current = nodes[current].next;
        }
        if nodes[current].color == 1 {
            let cycle = current;
            loop {
                work.one()?;
                nodes[current].color = 3;
                current = nodes[current].next;
                if current == cycle {
                    break;
                }
            }
        }
        current = start;
        loop {
            work.one()?;
            if nodes[current].color != 1 {
                break;
            }
            nodes[current].color = 2;
            current = nodes[current].next;
        }
    }
    for (block, node) in nodes.iter_mut().enumerate() {
        work.one()?;
        if node.color == 2 && (block == source.entry() || node.incoming != 1 || !node.incoming_goto)
        {
            node.color = 4;
        }
    }
    let (mut interiors, mut links, mut cycles) = (0usize, 0usize, 0usize);
    for node in &nodes {
        work.one()?;
        if node.color == 2 {
            interiors += 1;
            links += usize::from(nodes[node.next].color == 2);
        }
        cycles += usize::from(node.color == 3);
    }
    Ok(Chains::StructuralOnly {
        single_entry_acyclic_interiors: interiors,
        runs: interiors.checked_sub(links).ok_or(Stop::Malformed)?,
        cycle_blocks_retained: cycles,
        blocks_after_structural_only: source.len().checked_sub(interiors).ok_or(Stop::Malformed)?,
    })
}

#[cfg(test)]
#[path = "bounded/tests.rs"]
mod tests;
