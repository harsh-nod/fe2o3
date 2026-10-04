//! Scalar-replacement eligibility in O(layouts + containment edges), without
//! recursion or expanding array lengths into elements.
use super::{Error, LayoutKind, Meter, Resource, Result, filled, size_of};
use fe2o3_kernel_ir::StorageLayoutV1;

#[derive(Clone, Copy)]
struct Dependent {
    parent: usize,
    next: Option<usize>,
}

pub(super) fn derive(
    layouts: &[StorageLayoutV1],
    meter: &mut Meter<'_, '_>,
) -> Result<Vec<Option<bool>>> {
    let header = headers();
    meter.reserve(header)?;
    let mut edge_count = 0usize;
    for row in layouts {
        meter.work(2)?;
        let children = match &row.kind {
            LayoutKind::Record(fields) => fields.len(),
            LayoutKind::Array { .. } => 1,
            _ => 0,
        };
        edge_count = edge_count
            .checked_add(children)
            .ok_or(Resource::Arithmetic)?;
    }
    let mut safe = filled(layouts.len(), None, meter)?;
    let mut remaining = filled(layouts.len(), 0usize, meter)?;
    let mut heads = filled(layouts.len(), None, meter)?;
    let (mut dependents, edge_bytes) = meter.table::<Dependent>(edge_count)?;
    let (mut ready, queue_bytes) = meter.table::<usize>(layouts.len())?;
    for (parent, row) in layouts.iter().enumerate() {
        meter.work(4)?;
        let decision = match &row.kind {
            LayoutKind::Scalar(_) | LayoutKind::Vector(_) => Some(true),
            LayoutKind::Record(fields) => {
                remaining[parent] = fields.len();
                for field in fields.iter() {
                    link(
                        parent,
                        field.layout.0 as usize,
                        &mut heads,
                        &mut dependents,
                        meter,
                    )?;
                }
                fields.is_empty().then_some(true)
            }
            LayoutKind::Array { element, .. } => {
                remaining[parent] = 1;
                link(
                    parent,
                    element.0 as usize,
                    &mut heads,
                    &mut dependents,
                    meter,
                )?;
                None
            }
            // Pointers, tags and overlapping byte views are not scalar values.
            _ => Some(false),
        };
        safe[parent] = decision;
        if decision.is_some() {
            meter.push(&mut ready, parent)?;
        }
    }
    if dependents.len() != edge_count {
        return Err(Error::Inconsistent("aggregate layout edge census"));
    }
    let mut cursor = 0usize;
    while let Some(&child) = ready.get(cursor) {
        meter.work(3)?;
        cursor = cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
        let value = safe[child].ok_or(Error::Inconsistent("unresolved aggregate layout queue"))?;
        let mut next = heads[child];
        while let Some(edge) = next {
            meter.work(5)?;
            let dependent = dependents[edge];
            next = dependent.next;
            let parent = dependent.parent;
            if safe[parent].is_some() {
                continue;
            }
            remaining[parent] = remaining[parent]
                .checked_sub(1)
                .ok_or(Resource::Arithmetic)?;
            if !value || remaining[parent] == 0 {
                safe[parent] = Some(value);
                meter.push(&mut ready, parent)?;
            }
        }
    }
    // Each node settles once. Unseeded cycles remain unknown; a false child
    // propagates even through a cycle, matching the former least fixed point.
    let scratch = remaining
        .capacity()
        .checked_mul(size_of::<usize>())
        .and_then(|n| {
            heads
                .capacity()
                .checked_mul(size_of::<Option<usize>>())
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(edge_bytes))
        .and_then(|n| n.checked_add(queue_bytes))
        .and_then(|n| n.checked_add(header))
        .ok_or(Resource::Arithmetic)?;
    drop(remaining);
    drop(heads);
    drop(dependents);
    drop(ready);
    meter.release(scratch)?;
    Ok(safe)
}

fn link(
    parent: usize,
    child: usize,
    heads: &mut [Option<usize>],
    dependents: &mut Vec<Dependent>,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(4)?;
    let head = heads
        .get_mut(child)
        .ok_or(Error::Inconsistent("aggregate layout child"))?;
    let edge = dependents.len();
    meter.push(
        dependents,
        Dependent {
            parent,
            next: *head,
        },
    )?;
    *head = Some(edge);
    Ok(())
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + size_of::<Result<T>>()
    }
    h::<Vec<Option<bool>>>()
        + h::<Vec<usize>>() * 2
        + h::<Vec<Option<usize>>>()
        + h::<Vec<Dependent>>()
        + h::<Dependent>()
        + h::<Option<usize>>()
        + h::<Option<bool>>()
        + h::<&[StorageLayoutV1]>()
        + h::<&StorageLayoutV1>()
        + h::<&LayoutKind>()
        + h::<&mut [Option<usize>]>()
        + h::<&mut Vec<Dependent>>()
        + h::<&mut Option<usize>>()
        + h::<&mut Meter<'_, '_>>()
        + h::<()>()
        + h::<usize>() * 14
}

#[cfg(test)]
#[path = "canonical_kir_aggregate_layout_worklist_v36_tests.rs"]
mod tests;
