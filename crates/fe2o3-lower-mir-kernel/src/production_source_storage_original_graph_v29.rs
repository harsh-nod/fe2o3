// This finite index contains source type/role dependencies, never source values,
// storage activity, instances or a second physical layout table.
struct SourceStorageOriginalNodeV29 {
    key: RowKey,
    edges: std::ops::Range<usize>,
    id: Option<StorageLayoutIdV1>,
    depth: usize,
}

#[derive(Clone, Copy)]
struct SourceStorageOriginalEdgeV29 {
    target: usize,
    contained: bool,
}

#[derive(Default)]
struct SourceStorageOriginalGraphV29 {
    nodes: Vec<SourceStorageOriginalNodeV29>,
    edges: Vec<SourceStorageOriginalEdgeV29>,
    keys: BTreeMap<RowKey, usize>,
    missing: usize,
}

struct SourceStorageOriginalComponentsV29 {
    members: Vec<usize>,
    ranges: Vec<std::ops::Range<usize>>,
}

impl SourceStorageOriginalGraphV29 {
    fn insert(
        &mut self,
        layouts: &SourceStorageLayoutsV29<'_>,
        key: RowKey,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Error> {
        layouts.lease.work(1, budget)?;
        charge_execution_cfg_lookup_v29(self.keys.len(), budget)?;
        if let Some(&index) = self.keys.get(&key) {
            return Ok(index);
        }
        let id = layouts.original_checked_id(key, budget)?;
        let depth = if let Some(id) = id {
            let physical = layouts.physical.try_borrow()
                .map_err(|_| error("original closure has an active physical builder"))?;
            *physical.depths.get(id.0 as usize)
                .ok_or_else(|| error("original closure cached depth is absent"))?
        } else {
            layouts.check_row_capacity(self.missing, budget)?;
            self.missing = self.missing.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            0
        };
        layouts.original_map_reserve::<RowKey, usize>(self.keys.len(), budget)?;
        let index = self.nodes.len();
        layouts.lease.push(&mut self.nodes, SourceStorageOriginalNodeV29 {
            key, edges: 0..0, id, depth,
        }, budget)?;
        self.keys.insert(key, index);
        Ok(index)
    }

    fn discover(
        &mut self,
        layouts: &SourceStorageLayoutsV29<'_>,
        root: RowKey,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Error> {
        let root = self.insert(layouts, root, budget)?;
        let mut next = 0;
        let mut added_edges = 0;
        while next < self.nodes.len() {
            layouts.lease.work(1, budget)?;
            if self.nodes[next].id.is_some() {
                next += 1;
                continue;
            }
            let key = self.nodes[next].key;
            let (size, edge_count) = layouts.original_row_metrics(key)?;
            if size > layouts.limits.object_bytes {
                return Err(error("original closure exceeds object-size policy"));
            }
            added_edges = argument_sum_v1(&[added_edges, edge_count])?;
            if added_edges > layouts.limits.edges {
                return Err(error("original closure exceeds edge policy"));
            }
            let begin = self.edges.len();
            let mut child = 0;
            loop {
                layouts.lease.work(1, budget)?;
                let Some(key) = layouts.original_contained_key(key, child)? else { break; };
                child = child.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                let target = self.insert(layouts, key, budget)?;
                layouts.lease.push(&mut self.edges, SourceStorageOriginalEdgeV29 { target, contained: true }, budget)?;
            }
            if let Some(key) = layouts.original_reference_key(key, budget)? {
                let target = self.insert(layouts, key, budget)?;
                layouts.lease.push(&mut self.edges, SourceStorageOriginalEdgeV29 { target, contained: false }, budget)?;
            }
            if self.edges.len() - begin != edge_count {
                return Err(error("original closure dependency roster differs from source row"));
            }
            self.nodes[next].edges = begin..self.edges.len();
            next += 1;
        }
        Ok(root)
    }

    fn ordered(&self, layouts: &SourceStorageLayoutsV29<'_>, budget: &mut Budget<'_>) -> Result<Vec<usize>, Error> {
        layouts.lease.work(1, budget)?;
        if self.keys.len() != self.nodes.len() { return Err(ArgumentResourceV1::Accounting.into()); }
        let mut order = layouts.lease.vector(self.nodes.len(), budget)?;
        for (&key, &index) in &self.keys {
            layouts.lease.work(1, budget)?;
            let node = self.nodes.get(index).ok_or_else(|| error("original closure key index is absent"))?;
            if node.key != key || node.edges.start > node.edges.end || node.edges.end > self.edges.len() {
                return Err(error("original closure node identity or edge range changed"));
            }
            for edge in &self.edges[node.edges.clone()] {
                layouts.lease.work(1, budget)?;
                if edge.target >= self.nodes.len() { return Err(error("original closure edge target is absent")); }
            }
            order.push(index);
        }
        Ok(order)
    }

    fn containment(
        &mut self,
        layouts: &SourceStorageLayoutsV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let order = self.ordered(layouts, budget)?;
        let mut colors = layouts.lease.vector(self.nodes.len(), budget)?;
        layouts.lease.work(self.nodes.len(), budget)?;
        colors.resize(self.nodes.len(), 0_u8);
        let mut frames = layouts.lease.vector::<(usize, usize, usize)>(self.nodes.len(), budget)?;
        for root in order {
            layouts.lease.work(1, budget)?;
            if colors[root] == 2 { continue; }
            if layouts.limits.containment_depth == 0 { return Err(error("original closure exceeds containment policy")); }
            colors[root] = 1;
            frames.push((root, self.nodes[root].edges.start, self.nodes[root].depth.max(1)));
            while let Some((node, next, depth)) = frames.last().copied() {
                layouts.lease.work(1, budget)?;
                let top = frames.len() - 1;
                if next == self.nodes[node].edges.end {
                    if depth > layouts.limits.containment_depth {
                        return Err(error("original closure exceeds containment policy"));
                    }
                    self.nodes[node].depth = depth;
                    colors[node] = 2;
                    frames.pop();
                    if let Some((_, _, parent_depth)) = frames.last_mut() {
                        *parent_depth = (*parent_depth).max(depth.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?);
                    }
                    continue;
                }
                let edge = self.edges[next];
                frames[top].1 = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                if !edge.contained { continue; }
                match colors[edge.target] {
                    0 => {
                        if frames.len() >= layouts.limits.containment_depth {
                            return Err(error("original closure exceeds containment policy"));
                        }
                        colors[edge.target] = 1;
                        frames.push((edge.target, self.nodes[edge.target].edges.start, self.nodes[edge.target].depth.max(1)));
                    }
                    1 => return Err(error("original closure contains a by-value cycle")),
                    2 => frames[top].2 = depth.max(self.nodes[edge.target].depth.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?),
                    _ => return Err(ArgumentResourceV1::Accounting.into()),
                }
            }
        }
        Ok(())
    }

    fn components(
        &self,
        layouts: &SourceStorageLayoutsV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageOriginalComponentsV29, Error> {
        let order = self.ordered(layouts, budget)?;
        let count = self.nodes.len();
        let mut indices = layouts.lease.vector(count, budget)?;
        let mut low = layouts.lease.vector(count, budget)?;
        let mut active = layouts.lease.vector(count, budget)?;
        layouts.lease.work(argument_product_v1(count, 3)?, budget)?;
        indices.resize(count, usize::MAX);
        low.resize(count, usize::MAX);
        active.resize(count, false);
        let mut stack = layouts.lease.vector(count, budget)?;
        let mut frames = layouts.lease.vector::<(usize, usize)>(count, budget)?;
        let mut result = SourceStorageOriginalComponentsV29 {
            members: layouts.lease.vector(count, budget)?,
            ranges: layouts.lease.vector(count, budget)?,
        };
        let mut serial = 0_usize;
        for root in order {
            layouts.lease.work(1, budget)?;
            if indices[root] != usize::MAX { continue; }
            indices[root] = serial;
            low[root] = serial;
            serial = serial.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            active[root] = true;
            stack.push(root);
            frames.push((root, self.nodes[root].edges.start));
            while let Some(&(node, next)) = frames.last() {
                layouts.lease.work(1, budget)?;
                if next != self.nodes[node].edges.end {
                    frames.last_mut().ok_or(ArgumentResourceV1::Accounting)?.1 = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                    let child = self.edges[next].target;
                    if indices[child] == usize::MAX {
                        indices[child] = serial;
                        low[child] = serial;
                        serial = serial.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                        active[child] = true;
                        stack.push(child);
                        frames.push((child, self.nodes[child].edges.start));
                    } else if active[child] {
                        low[node] = low[node].min(indices[child]);
                    }
                    continue;
                }
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    low[parent] = low[parent].min(low[node]);
                }
                if low[node] == indices[node] {
                    let first = result.members.len();
                    loop {
                        layouts.lease.work(1, budget)?;
                        let member = stack.pop().ok_or(ArgumentResourceV1::Accounting)?;
                        active[member] = false;
                        result.members.push(member);
                        if member == node { break; }
                    }
                    result.ranges.push(first..result.members.len());
                }
            }
        }
        if !stack.is_empty() || result.members.len() != count {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(result)
    }
}
