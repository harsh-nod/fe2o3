//! One invocation's fixed-capacity Copy slabs. No dynamic data escapes.
use super::*;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Fact {
    Unknown,
    Known(scalar::Scalar),
    Input,
    ConvertedInput,
}
pub(super) struct Scratch {
    pub(super) facts: Vec<Fact>,
    pub(super) aliases: Vec<bool>,
    pub(super) visited: Vec<bool>,
}
fn bytes<E, F: FnMut(usize) -> std::result::Result<(), E>>(
    count: usize,
    element: usize,
    existing: usize,
    phase: Phase,
    cx: &Context<'_, E, F>,
) -> Result<usize, E> {
    let actual = count
        .checked_mul(element)
        .and_then(|n| n.checked_add(existing))
        .ok_or_else(|| cx.resource(Resource::SizeOverflow { phase }))?;
    let actual_u64 =
        u64::try_from(actual).map_err(|_| cx.resource(Resource::SizeOverflow { phase }))?;
    let maximum = cx.limits.limit(SemanticMirResourceV1::CanonicalBytes);
    if actual_u64 > maximum {
        return Err(cx.resource(Resource::ScratchLimit {
            phase,
            actual: actual_u64,
            maximum,
        }));
    }
    Ok(actual)
}
fn slab<T: Copy, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    count: usize,
    value: T,
    live: &mut usize,
    kind: Slab,
    cx: &mut Context<'_, E, F>,
) -> Result<Vec<T>, E> {
    cx.work(1)?;
    let requested = Phase {
        slab: kind,
        backing: Backing::Requested,
    };
    bytes(count, size_of::<T>(), *live, requested, cx)?;
    let mut result = Vec::new();
    result.try_reserve_exact(count).map_err(|_| {
        cx.resource(Resource::Allocation {
            phase: requested,
            requested_elements: count,
            element_bytes: size_of::<T>(),
        })
    })?;
    let actual = Phase {
        slab: kind,
        backing: Backing::Actual,
    };
    let accepted = bytes(result.capacity(), size_of::<T>(), *live, actual, cx)?;
    cx.work(count)?;
    result.resize(count, value);
    *live = accepted;
    Ok(result)
}
impl Scratch {
    pub(super) fn new<E, F: FnMut(usize) -> std::result::Result<(), E>>(
        locals: usize,
        blocks: usize,
        cx: &mut Context<'_, E, F>,
    ) -> Result<Self, E> {
        cx.work(1)?;
        let mut live = bytes(
            1,
            size_of::<Self>(),
            0,
            Phase {
                slab: Slab::Header,
                backing: Backing::Requested,
            },
            cx,
        )?;
        let facts = slab(locals, Fact::Unknown, &mut live, Slab::Locals, cx)?;
        let aliases = slab(locals, false, &mut live, Slab::Aliases, cx)?;
        let visited = slab(blocks, false, &mut live, Slab::Blocks, cx)?;
        Ok(Self {
            facts,
            aliases,
            visited,
        })
    }
}

#[cfg(test)]
mod tests;
