//! Original value-component schemas, independent of physical storage layouts.
use super::super::super::{ScalarV30, Shape, Type, TypeId};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticProjectionKindV1 as Projection, SemanticProjectionV1,
};
use std::ops::Range;

const MAX_LEAVES: usize = 1 << 20;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Unsupported,
    Leaf(ScalarV30),
    Fields { first: usize, count: usize },
    Array { element: TypeId, count: usize },
}

#[derive(Clone, Copy, Debug)]
struct Child {
    ty: TypeId,
    first_leaf: usize,
}

#[derive(Clone, Copy, Debug)]
struct Count {
    leaves: usize,
    paths: usize,
}

struct Leaf {
    path: Range<usize>,
    ty: TypeId,
    scalar: ScalarV30,
}

pub(super) struct SourceAggregateTypesV42 {
    kinds: Vec<Kind>,
    children: Vec<Child>,
    counts: Vec<Option<Count>>,
    roots: Vec<Range<usize>>,
    leaves: Vec<Leaf>,
    paths: Vec<u32>,
}

#[derive(Clone, Copy)]
struct CountFrame {
    ty: TypeId,
    next: usize,
    count: Count,
    supported: bool,
}

#[derive(Clone, Copy)]
struct WalkFrame {
    ty: TypeId,
    next: usize,
    path_len: usize,
}

fn sum(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn product(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}

impl SourceAggregateTypesV42 {
    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.reserve_storage(headers())?;
        write!(
            out,
            "open spec fn invocation_source_aggregate_leaf_count_v42(ty: int) -> int {{\n"
        )
        .map_err(|_| out.error())?;
        for (ty, range) in self.roots.iter().enumerate() {
            out.budget.charge_work(2)?;
            if !range.is_empty() {
                write!(out, " if ty == {ty}int {{ {}int }} else", range.len())
                    .map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ -1int }}\n}}\nopen spec fn invocation_source_aggregate_leaf_path_v42(ty: int, ordinal: int) -> Seq<int> {{\n").map_err(|_| out.error())?;
        for (ty, range) in self.roots.iter().enumerate() {
            for (ordinal, leaf) in self.leaves[range.clone()].iter().enumerate() {
                out.budget.charge_work(2)?;
                write!(out, " if ty == {ty}int && ordinal == {ordinal}int {{ seq![")
                    .map_err(|_| out.error())?;
                for field in &self.paths[leaf.path.clone()] {
                    out.budget.charge_work(1)?;
                    write!(out, "{field}int,").map_err(|_| out.error())?;
                }
                write!(out, "] }} else").map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ seq![] }}\n}}\nopen spec fn invocation_source_aggregate_leaf_bits_v42(ty: int, ordinal: int) -> int {{\n").map_err(|_| out.error())?;
        for (ty, range) in self.roots.iter().enumerate() {
            for (ordinal, leaf) in self.leaves[range.clone()].iter().enumerate() {
                out.budget.charge_work(2)?;
                write!(
                    out,
                    " if ty == {ty}int && ordinal == {ordinal}int {{ {}int }} else",
                    leaf.scalar.width()
                )
                .map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ -1int }}\n}}\nopen spec fn invocation_source_aggregate_child_v42(ty: int, field: int) -> Option<int> {{\n").map_err(|_| out.error())?;
        for (ty, kind) in self.kinds.iter().enumerate() {
            out.budget.charge_work(1)?;
            match *kind {
                Kind::Fields { first, count } => {
                    write!(
                        out,
                        " if ty == {ty}int && 0 <= field < {count}int {{ Some(seq!["
                    )
                    .map_err(|_| out.error())?;
                    for child in &self.children[first..first + count] {
                        out.budget.charge_work(1)?;
                        write!(out, "{}int,", child.ty.index()).map_err(|_| out.error())?;
                    }
                    write!(out, "][field]) }} else").map_err(|_| out.error())?;
                }
                Kind::Array { element, count } => {
                    write!(
                        out,
                        " if ty == {ty}int && 0 <= field < {count}int {{ Some({}int) }} else",
                        element.index()
                    )
                    .map_err(|_| out.error())?;
                }
                Kind::Unsupported | Kind::Leaf(_) => {}
            }
        }
        write!(out, " {{ None }}\n}}\n").map_err(|_| out.error())
    }

    pub(super) fn derive(
        types: &[Type],
        abi: &source_abi::SourceAbi,
        requested: &[bool],
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        if requested.len() != types.len() {
            return Err(mismatch());
        }
        let mut edges = 0;
        for declaration in types {
            out.budget.charge_work(1)?;
            if let Shape::Tuple(fields) | Shape::Aggregate(fields) = declaration.shape() {
                edges = sum(edges, fields.fields().len())?;
            }
        }
        let mut kinds = vector(types.len(), out)?;
        let mut children = vector(edges, out)?;
        for (index, declaration) in types.iter().enumerate() {
            out.budget.charge_work(2)?;
            let ty = TypeId::from_index(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
            let nominal = abi.slice(ty, out)?.is_some() || abi.witness(ty, out)?.is_some();
            let kind = if nominal
                || declaration.layout().is_uninhabited()
                || declaration.layout().size_bytes().is_none()
            {
                Kind::Unsupported
            } else {
                match declaration.shape() {
                    Shape::Unit if declaration.layout().size_bytes() == Some(0) => {
                        Kind::Leaf(ScalarV30::Unit)
                    }
                    Shape::Scalar(_) => match ScalarV30::from_source(types, ty) {
                        Ok(scalar)
                            if declaration.layout().size_bytes()
                                == Some(if scalar == ScalarV30::Bool {
                                    1
                                } else {
                                    u64::from(scalar.width() / 8)
                                }) =>
                        {
                            Kind::Leaf(scalar)
                        }
                        Ok(_) => Kind::Unsupported,
                        Err(_) => Kind::Unsupported,
                    },
                    Shape::Tuple(fields) | Shape::Aggregate(fields)
                        if fields.fields().is_empty()
                            && declaration.layout().size_bytes() == Some(0) =>
                    {
                        Kind::Leaf(ScalarV30::Unit)
                    }
                    Shape::Tuple(fields) | Shape::Aggregate(fields) => {
                        let first = children.len();
                        for &ty in fields.fields() {
                            out.budget.charge_work(1)?;
                            children.push(Child { ty, first_leaf: 0 });
                        }
                        Kind::Fields {
                            first,
                            count: children.len() - first,
                        }
                    }
                    Shape::Array { length: 0, .. }
                        if declaration.layout().size_bytes() == Some(0) =>
                    {
                        Kind::Leaf(ScalarV30::Unit)
                    }
                    Shape::Array { length: 0, .. } => Kind::Unsupported,
                    Shape::Array { element, length } => Kind::Array {
                        element: *element,
                        count: usize::try_from(*length).map_err(|_| Resource::Arithmetic)?,
                    },
                    _ => Kind::Unsupported,
                }
            };
            kinds.push(kind);
        }
        let mut counts: Vec<Option<Count>> = vector(types.len(), out)?;
        counts.resize(types.len(), None);
        let mut colors = vector(types.len(), out)?;
        colors.resize(types.len(), 0_u8);
        out.budget.charge_work(product(types.len(), 2)?)?;
        let mut stack = vector(types.len(), out)?;
        for root in 0..types.len() {
            out.budget.charge_work(1)?;
            if !requested[root] || colors[root] != 0 {
                continue;
            }
            stack.push(CountFrame {
                ty: TypeId::from_index(root as u32),
                next: 0,
                count: Count {
                    leaves: 0,
                    paths: 0,
                },
                supported: true,
            });
            colors[root] = 1;
            while let Some(frame) = stack.last().copied() {
                out.budget.charge_work(5)?;
                let index = frame.ty.index() as usize;
                let (child, multiplicity) = match kinds[index] {
                    Kind::Fields { first, count } if frame.next < count => {
                        (Some(children[first + frame.next].ty), 1)
                    }
                    Kind::Array { element, count } if frame.next == 0 => (Some(element), count),
                    _ => (None, 0),
                };
                if let Some(child) = child {
                    let child_index = child.index() as usize;
                    match colors.get(child_index).copied().ok_or_else(mismatch)? {
                        0 => {
                            if stack.len() == stack.capacity() {
                                return Err(Resource::Accounting.into());
                            }
                            colors[child_index] = 1;
                            stack.push(CountFrame {
                                ty: child,
                                next: 0,
                                count: Count {
                                    leaves: 0,
                                    paths: 0,
                                },
                                supported: true,
                            });
                            continue;
                        }
                        1 => {
                            return Err(Error::Statement(
                                "original aggregate type graph is cyclic",
                            ));
                        }
                        _ => {}
                    }
                    let current = stack.last_mut().ok_or_else(mismatch)?;
                    if let Kind::Fields { first, .. } = kinds[index] {
                        children[first + current.next].first_leaf = current.count.leaves;
                    }
                    current.next += 1;
                    if let Some(child_count) = counts[child_index] {
                        current.count.leaves = sum(
                            current.count.leaves,
                            product(child_count.leaves, multiplicity)?,
                        )?;
                        current.count.paths = sum(
                            current.count.paths,
                            product(sum(child_count.paths, child_count.leaves)?, multiplicity)?,
                        )?;
                    } else {
                        current.supported = false;
                    }
                    continue;
                }
                counts[index] = match kinds[index] {
                    Kind::Unsupported => None,
                    Kind::Leaf(_) => Some(Count {
                        leaves: 1,
                        paths: 0,
                    }),
                    _ if frame.supported => Some(frame.count),
                    _ => None,
                };
                colors[index] = 2;
                stack.pop();
            }
        }
        let count_scratch = sum(
            product(colors.capacity(), size_of::<u8>())?,
            product(stack.capacity(), size_of::<CountFrame>())?,
        )?;
        drop(colors);
        drop(stack);
        out.budget.release_storage(count_scratch)?;
        let mut total = Count {
            leaves: 0,
            paths: 0,
        };
        for count in &counts {
            out.budget.charge_work(2)?;
            if let Some(count) = count {
                total.leaves = sum(total.leaves, count.leaves)?;
                total.paths = sum(total.paths, count.paths)?;
            }
        }
        if total.leaves > MAX_LEAVES {
            return Err(Error::Statement(
                "original aggregate leaf census exceeds structural limit",
            ));
        }
        let mut leaves = vector(total.leaves, out)?;
        let mut paths = vector(total.paths, out)?;
        let mut roots = vector(types.len(), out)?;
        let mut walk = vector(types.len(), out)?;
        let mut path = vector(types.len(), out)?;
        for root in 0..types.len() {
            out.budget.charge_work(2)?;
            let first = leaves.len();
            if counts[root].is_some() {
                walk.push(WalkFrame {
                    ty: TypeId::from_index(root as u32),
                    next: 0,
                    path_len: 0,
                });
                while let Some(frame) = walk.last().copied() {
                    out.budget.charge_work(4)?;
                    path.truncate(frame.path_len);
                    let child = match kinds[frame.ty.index() as usize] {
                        Kind::Leaf(scalar) => {
                            let begin = paths.len();
                            out.budget.charge_work(path.len())?;
                            if sum(paths.len(), path.len())? > paths.capacity()
                                || leaves.len() == leaves.capacity()
                            {
                                return Err(Resource::Accounting.into());
                            }
                            paths.extend_from_slice(&path);
                            leaves.push(Leaf {
                                path: begin..paths.len(),
                                ty: frame.ty,
                                scalar,
                            });
                            None
                        }
                        Kind::Fields { first, count } if frame.next < count => {
                            Some(children[first + frame.next].ty)
                        }
                        Kind::Array { element, count } if frame.next < count => Some(element),
                        _ => None,
                    };
                    if let Some(child) = child {
                        walk.last_mut().ok_or_else(mismatch)?.next += 1;
                        if path.len() == path.capacity() || walk.len() == walk.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        path.push(u32::try_from(frame.next).map_err(|_| Resource::Arithmetic)?);
                        walk.push(WalkFrame {
                            ty: child,
                            next: 0,
                            path_len: path.len(),
                        });
                    } else {
                        walk.pop();
                    }
                }
            }
            roots.push(first..leaves.len());
            if leaves.len() - first != counts[root].map_or(0, |count| count.leaves) {
                return Err(Resource::Accounting.into());
            }
        }
        if (leaves.len(), paths.len()) != (total.leaves, total.paths) {
            return Err(Resource::Accounting.into());
        }
        let walk_scratch = sum(
            product(walk.capacity(), size_of::<WalkFrame>())?,
            product(path.capacity(), size_of::<u32>())?,
        )?;
        drop(walk);
        drop(path);
        out.budget.release_storage(walk_scratch)?;
        Ok(Self {
            kinds,
            children,
            counts,
            roots,
            leaves,
            paths,
        })
    }

    pub(super) fn leaf_count(&self, ty: TypeId, out: &mut Writer<'_, '_>) -> Result<Option<usize>> {
        out.budget.charge_work(1)?;
        let range = self.roots.get(ty.index() as usize).ok_or_else(mismatch)?;
        Ok((!range.is_empty()).then_some(range.len()))
    }

    pub(super) fn component_range(
        &self,
        mut ty: TypeId,
        path: &[SemanticProjectionV1],
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(Range<usize>, TypeId)>> {
        if self.leaf_count(ty, out)?.is_none() {
            return Ok(None);
        }
        let mut first = 0;
        for projection in path {
            out.budget.charge_work(5)?;
            let (next, offset) = match (projection.kind(), self.kinds[ty.index() as usize]) {
                (Projection::Field(field), Kind::Fields { first, count })
                    if (field as usize) < count =>
                {
                    let child = self.children[first + field as usize];
                    (child.ty, child.first_leaf)
                }
                (
                    Projection::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                    Kind::Array { element, count },
                ) => {
                    let offset = usize::try_from(offset).map_err(|_| Resource::Arithmetic)?;
                    let minimum =
                        usize::try_from(minimum_length).map_err(|_| Resource::Arithmetic)?;
                    if minimum > count
                        || (from_end && (offset == 0 || offset > count))
                        || (!from_end && offset >= count)
                    {
                        return Err(mismatch());
                    }
                    let ordinal = if from_end { count - offset } else { offset };
                    (
                        element,
                        product(
                            ordinal,
                            self.counts[element.index() as usize]
                                .ok_or_else(mismatch)?
                                .leaves,
                        )?,
                    )
                }
                _ => return Ok(None),
            };
            if next != projection.result_type() {
                return Err(mismatch());
            }
            first = sum(first, offset)?;
            ty = next;
        }
        let count = self.counts[ty.index() as usize]
            .ok_or_else(mismatch)?
            .leaves;
        Ok(Some((first..sum(first, count)?, ty)))
    }
}

#[derive(Clone, Copy)]
pub(in super::super) struct SourceAggregateLeafV42<'a, 'view, 'source> {
    slots: &'a SourceSlots<'view, 'source>,
    index: usize,
}

impl<'a, 'view, 'source> SourceAggregateLeafV42<'a, 'view, 'source> {
    fn row(&self, out: &mut Writer<'_, '_>) -> Result<&'a Leaf> {
        self.slots.with_source_query_v42(out, |out| {
            out.budget.charge_work(1)?;
            self.slots
                .aggregates
                .leaves
                .get(self.index)
                .ok_or_else(mismatch)
        })
    }

    pub(in super::super) fn path(&self, out: &mut Writer<'_, '_>) -> Result<&'a [u32]> {
        let row = self.row(out)?;
        self.slots
            .aggregates
            .paths
            .get(row.path.clone())
            .ok_or_else(mismatch)
    }

    pub(in super::super) fn source_type(&self, out: &mut Writer<'_, '_>) -> Result<TypeId> {
        Ok(self.row(out)?.ty)
    }

    pub(in super::super) fn scalar(&self, out: &mut Writer<'_, '_>) -> Result<ScalarV30> {
        Ok(self.row(out)?.scalar)
    }
}

impl<'view, 'source> SourceSlots<'view, 'source> {
    pub(in super::super) fn aggregate_leaf_count(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<usize>> {
        self.with_source_query_v42(out, |out| self.aggregates.leaf_count(ty, out))
    }

    pub(in super::super) fn aggregate_leaf<'a>(
        &'a self,
        ty: TypeId,
        ordinal: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<SourceAggregateLeafV42<'a, 'view, 'source>> {
        self.with_source_query_v42(out, |out| {
            out.budget.charge_work(3)?;
            let range = self
                .aggregates
                .roots
                .get(ty.index() as usize)
                .ok_or_else(mismatch)?;
            if ordinal >= range.len() {
                return Err(Error::Statement("original aggregate leaf ordinal differs"));
            }
            Ok(SourceAggregateLeafV42 {
                slots: self,
                index: range.start + ordinal,
            })
        })
    }

    pub(in super::super) fn aggregate_component_range(
        &self,
        ty: TypeId,
        path: &[SemanticProjectionV1],
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(Range<usize>, TypeId)>> {
        self.with_source_query_v42(out, |out| self.aggregates.component_range(ty, path, out))
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceAggregateTypesV42>()
        + h::<CountFrame>()
        + h::<WalkFrame>()
        + h::<Leaf>()
        + h::<Child>()
        + h::<Kind>()
        + h::<Count>()
        + 8 * size_of::<Vec<usize>>()
        + 24 * size_of::<usize>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_aggregate_values_v42_tests.rs"]
mod tests;
