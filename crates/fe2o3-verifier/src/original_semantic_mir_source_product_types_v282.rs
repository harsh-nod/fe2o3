//! Original typed product paths. Nominal values are opaque atoms, not fields
//! reconstructed from their layout. A schema supplies no value or loan authority.
use super::super::super::{ScalarV30, Shape, Type, TypeId};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExecutionRoleV29 as ExecutionRole, SemanticMutabilityV1 as Mutability,
    SemanticPointerKindV1 as PointerKind, SemanticPointerMetadataV1 as PointerMetadata,
    SemanticProjectionKindV1 as Projection, SemanticProjectionV1,
    SemanticRustTypeKindV1 as RustType,
};
use std::ops::Range;

const MAX_COMPONENTS: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Atom {
    Scalar(ScalarV30),
    Pointer {
        mutable: bool,
        reference: bool,
    },
    Slice {
        metadata_bits: u32,
        mutable: bool,
        reference: bool,
    },
    ExecutionAggregate(ExecutionRole),
    ExecutionReference {
        role: ExecutionRole,
        mutable: bool,
    },
    DescriptorReference {
        mutable: bool,
    },
    Enum,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Unsupported,
    Atom(Atom),
    Fields { first: usize, count: usize },
    Array { element: TypeId, count: usize },
}

#[derive(Clone, Copy)]
struct Child {
    ty: TypeId,
    first: usize,
}

#[derive(Clone, Copy)]
struct Count {
    atoms: usize,
    paths: usize,
    non_plain: bool,
}

struct Component {
    path: Range<usize>,
    ty: TypeId,
    atom: Atom,
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
    depth: usize,
}

pub(super) struct SourceProductTypesV282 {
    kinds: Vec<Kind>,
    composites: Vec<bool>,
    children: Vec<Child>,
    counts: Vec<Option<Count>>,
    roots: Vec<Range<usize>>,
    components: Vec<Component>,
    paths: Vec<u32>,
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Resource::Arithmetic.into())
}

fn mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right).ok_or(Resource::Arithmetic.into())
}

fn execution_role(declaration: &Type) -> Option<ExecutionRole> {
    match declaration.rust_type_kind() {
        RustType::Execution(role @ (ExecutionRole::KernelContext | ExecutionRole::Workgroup)) => {
            Some(role)
        }
        _ => None,
    }
}

impl SourceProductTypesV282 {
    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.reserve_storage(headers())?;
        write!(
            out,
            "spec fn invocation_source_product_composite_v282(ty: int) -> bool {{\n"
        )
        .map_err(|_| out.error())?;
        for (ty, composite) in self.composites.iter().enumerate() {
            out.budget.charge_work(2)?;
            if *composite && self.counts[ty].is_some() {
                write!(out, " ty == {ty}int ||").map_err(|_| out.error())?;
            }
        }
        write!(out, " false\n}}\n").map_err(|_| out.error())?;
        write!(
            out,
            "spec fn invocation_source_product_type_v282(ty: int) -> bool {{\n"
        )
        .map_err(|_| out.error())?;
        for (ty, kind) in self.kinds.iter().enumerate() {
            out.budget.charge_work(2)?;
            if matches!(kind, Kind::Fields { .. } | Kind::Array { .. })
                && self.counts[ty].is_some_and(|count| count.non_plain)
            {
                write!(out, " ty == {ty}int ||").map_err(|_| out.error())?;
            }
        }
        write!(out, " false\n}}\nspec fn invocation_source_product_component_count_v282(ty: int) -> int {{\n").map_err(|_| out.error())?;
        for (ty, range) in self.roots.iter().enumerate() {
            out.budget.charge_work(1)?;
            if !range.is_empty() {
                write!(out, " if ty == {ty}int {{ {}int }} else", range.len())
                    .map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ -1int }}\n}}\nspec fn invocation_source_product_component_path_v282(ty: int, ordinal: int) -> Seq<int> {{\n").map_err(|_| out.error())?;
        for (ty, range) in self.roots.iter().enumerate() {
            for (ordinal, component) in self.components[range.clone()].iter().enumerate() {
                out.budget.charge_work(2)?;
                write!(out, " if ty == {ty}int && ordinal == {ordinal}int {{ seq![")
                    .map_err(|_| out.error())?;
                for field in &self.paths[component.path.clone()] {
                    out.budget.charge_work(1)?;
                    write!(out, "{field}int,").map_err(|_| out.error())?;
                }
                write!(out, "] }} else").map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ seq![] }}\n}}\nspec fn invocation_source_product_component_type_v282(ty: int, ordinal: int) -> Option<int> {{\n").map_err(|_| out.error())?;
        for (ty, range) in self.roots.iter().enumerate() {
            for (ordinal, component) in self.components[range.clone()].iter().enumerate() {
                out.budget.charge_work(2)?;
                write!(
                    out,
                    " if ty == {ty}int && ordinal == {ordinal}int {{ Some({}int) }} else",
                    component.ty.index()
                )
                .map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ None }}\n}}\nspec fn invocation_source_product_child_count_v282(ty: int) -> int {{\n").map_err(|_| out.error())?;
        for (ty, kind) in self.kinds.iter().enumerate() {
            out.budget.charge_work(2)?;
            if !self.composites[ty] || self.counts[ty].is_none() {
                continue;
            }
            let count = match kind {
                Kind::Fields { count, .. } | Kind::Array { count, .. } => *count,
                Kind::Atom(Atom::Scalar(ScalarV30::Unit)) => 0,
                _ => return Err(mismatch()),
            };
            write!(out, " if ty == {ty}int {{ {count}int }} else").map_err(|_| out.error())?;
        }
        write!(out, " {{ -1int }}\n}}\nspec fn invocation_source_product_atom_kind_v282(ty: int) -> InvocationSourceProductAtomKindV282 {{\n").map_err(|_| out.error())?;
        for (ty, kind) in self.kinds.iter().enumerate() {
            out.budget.charge_work(2)?;
            let Kind::Atom(atom) = kind else { continue };
            if self.counts[ty].is_none() {
                continue;
            }
            write!(
                out,
                " if ty == {ty}int {{ InvocationSourceProductAtomKindV282::"
            )
            .map_err(|_| out.error())?;
            match atom {
                Atom::Scalar(scalar) => write!(out, "Scalar({}int)", scalar.width()),
                Atom::Pointer { mutable, reference } => write!(out, "Pointer {{ mutable: {mutable}, reference: {reference} }}"),
                Atom::Slice {
                    metadata_bits,
                    mutable,
                    reference,
                } => write!(
                    out,
                    "Slice {{ metadata_bits: {metadata_bits}int, mutable: {mutable}, reference: {reference} }}"
                ),
                Atom::ExecutionAggregate(role) => write!(
                    out,
                    "ExecutionAggregate({}int)",
                    execution_role_code(*role)?
                ),
                Atom::ExecutionReference { role, mutable } => write!(
                    out,
                    "ExecutionReference {{ role: {}int, mutable: {mutable} }}",
                    execution_role_code(*role)?
                ),
                Atom::DescriptorReference { mutable } => {
                    write!(out, "DescriptorReference {{ mutable: {mutable} }}")
                }
                Atom::Enum => write!(out, "Enum"),
            }
            .map_err(|_| out.error())?;
            write!(out, " }} else").map_err(|_| out.error())?;
        }
        write!(out, " {{ InvocationSourceProductAtomKindV282::Unsupported }}\n}}\nspec fn invocation_source_product_child_v282(ty: int, field: int) -> Option<int> {{\n").map_err(|_| out.error())?;
        for (ty, kind) in self.kinds.iter().enumerate() {
            out.budget.charge_work(1)?;
            if self.counts[ty].is_none() {
                continue;
            }
            match *kind {
                Kind::Fields { first, count } => {
                    write!(
                        out,
                        " if ty == {ty}int && 0 <= field < {count}int {{ Some(seq!["
                    )
                    .map_err(|_| out.error())?;
                    for child in &self.children[first..add(first, count)?] {
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
                Kind::Atom(_) | Kind::Unsupported => {}
            }
        }
        write!(out, " {{ None }}\n}}\n").map_err(|_| out.error())
    }

    pub(super) fn derive(
        types: &[Type],
        abi: &source_abi::SourceAbi,
        aggregates: &source_aggregates::SourceAggregateTypesV42,
        requested: &[bool],
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        if requested.len() != types.len() {
            return Err(mismatch());
        }
        let mut edges = 0usize;
        for declaration in types {
            out.budget.charge_work(1)?;
            if let Shape::Tuple(fields) | Shape::Aggregate(fields) = declaration.shape() {
                edges = add(edges, fields.fields().len())?;
            }
        }
        let mut kinds = vector(types.len(), out)?;
        let mut composites = vector(types.len(), out)?;
        let mut children = vector(edges, out)?;
        for (index, declaration) in types.iter().enumerate() {
            out.budget.charge_work(4)?;
            let ty = TypeId::from_index(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
            composites.push(
                matches!(declaration.rust_type_kind(), RustType::Ordinary)
                    && matches!(
                        declaration.shape(),
                        Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. }
                    ),
            );
            let kind = if declaration.layout().is_uninhabited()
                || declaration.layout().size_bytes().is_none()
            {
                Kind::Unsupported
            } else if let Some(role) = execution_role(declaration) {
                // Issuance remains in the authenticated execution event. Here
                // only complete, already modeled snapshots may be transported.
                if aggregates.leaf_count(ty, out)?.is_some() {
                    Kind::Atom(Atom::ExecutionAggregate(role))
                } else {
                    Kind::Unsupported
                }
            } else if matches!(declaration.rust_type_kind(), RustType::Execution(_))
                || abi.witness(ty, out)?.is_some()
                || abi.slice(ty, out)?.is_some()
            {
                Kind::Unsupported
            } else {
                match declaration.shape() {
                    Shape::Pointer(pointer)
                        if pointer.address_space() == 0 && pointer.pointer_width_bits() == 64 =>
                    {
                        let referent = types
                            .get(pointer.pointee().index() as usize)
                            .ok_or_else(mismatch)?;
                        let mutable = pointer.mutability() == Mutability::Mutable;
                        let reference = pointer.kind() == PointerKind::Reference;
                        if let Some(role) = execution_role(referent) {
                            if pointer.kind() == PointerKind::Reference
                                && pointer.metadata() == PointerMetadata::None
                            {
                                Kind::Atom(Atom::ExecutionReference { role, mutable })
                            } else {
                                Kind::Unsupported
                            }
                        } else if abi.slice(pointer.pointee(), out)?.is_some() {
                            if pointer.kind() == PointerKind::Reference
                                && pointer.metadata() == PointerMetadata::None
                            {
                                Kind::Atom(Atom::DescriptorReference { mutable })
                            } else {
                                Kind::Unsupported
                            }
                        } else if abi.witness(pointer.pointee(), out)?.is_some()
                            || matches!(referent.rust_type_kind(), RustType::Execution(_))
                        {
                            Kind::Unsupported
                        } else {
                            match pointer.metadata() {
                                PointerMetadata::None
                                    if declaration.layout().size_bytes() == Some(8) =>
                                {
                                    Kind::Atom(Atom::Pointer { mutable, reference })
                                }
                                PointerMetadata::SliceLength
                                    if matches!(referent.shape(), Shape::Slice { .. }) =>
                                {
                                    Kind::Atom(Atom::Slice {
                                        metadata_bits:
                                            super::super::source_bytes::slice_metadata_bits_v36(
                                                declaration,
                                                out,
                                            )?,
                                        mutable,
                                        reference,
                                    })
                                }
                                _ => Kind::Unsupported,
                            }
                        }
                    }
                    Shape::Enum { .. } => Kind::Atom(Atom::Enum),
                    Shape::Unit if declaration.layout().size_bytes() == Some(0) => {
                        Kind::Atom(Atom::Scalar(ScalarV30::Unit))
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
                            Kind::Atom(Atom::Scalar(scalar))
                        }
                        _ => Kind::Unsupported,
                    },
                    Shape::Tuple(fields) | Shape::Aggregate(fields)
                        if fields.fields().is_empty()
                            && declaration.layout().size_bytes() == Some(0) =>
                    {
                        Kind::Atom(Atom::Scalar(ScalarV30::Unit))
                    }
                    Shape::Tuple(fields) | Shape::Aggregate(fields) => {
                        let first = children.len();
                        for &ty in fields.fields() {
                            out.budget.charge_work(1)?;
                            if children.len() == children.capacity() {
                                return Err(Resource::Accounting.into());
                            }
                            children.push(Child { ty, first: 0 });
                        }
                        Kind::Fields {
                            first,
                            count: children.len() - first,
                        }
                    }
                    Shape::Array { length: 0, .. }
                        if declaration.layout().size_bytes() == Some(0) =>
                    {
                        Kind::Atom(Atom::Scalar(ScalarV30::Unit))
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
        let mut colors = vector(types.len(), out)?;
        counts.resize(types.len(), None);
        colors.resize(types.len(), 0u8);
        out.budget.charge_work(mul(types.len(), 2)?)?;
        let mut stack = vector(types.len(), out)?;
        for root in 0..types.len() {
            out.budget.charge_work(1)?;
            if !requested[root] || colors[root] != 0 {
                continue;
            }
            stack.push(CountFrame {
                ty: TypeId::from_index(u32::try_from(root).map_err(|_| Resource::Arithmetic)?),
                next: 0,
                count: Count {
                    atoms: 0,
                    paths: 0,
                    non_plain: false,
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
                                    atoms: 0,
                                    paths: 0,
                                    non_plain: false,
                                },
                                supported: true,
                            });
                            continue;
                        }
                        1 => return Err(Error::Statement("original product type graph is cyclic")),
                        _ => {}
                    }
                    let current = stack.last_mut().ok_or_else(mismatch)?;
                    if let Kind::Fields { first, .. } = kinds[index] {
                        children[first + current.next].first = current.count.atoms;
                    }
                    current.next = add(current.next, 1)?;
                    match counts[child_index] {
                        Some(count) => {
                            current.count.atoms =
                                add(current.count.atoms, mul(count.atoms, multiplicity)?)?;
                            current.count.paths = add(
                                current.count.paths,
                                mul(add(count.paths, count.atoms)?, multiplicity)?,
                            )?;
                            current.count.non_plain |= count.non_plain;
                        }
                        None => current.supported = false,
                    }
                    continue;
                }
                counts[index] = match kinds[index] {
                    Kind::Unsupported => None,
                    Kind::Atom(atom) => Some(Count {
                        atoms: 1,
                        paths: 0,
                        non_plain: !matches!(atom, Atom::Scalar(_)),
                    }),
                    _ if frame.supported => Some(frame.count),
                    _ => None,
                };
                colors[index] = 2;
                stack.pop();
            }
        }
        let credit = add(
            mul(colors.capacity(), size_of::<u8>())?,
            mul(stack.capacity(), size_of::<CountFrame>())?,
        )?;
        drop(colors);
        drop(stack);
        out.budget.release_storage(credit)?;
        let mut total = Count {
            atoms: 0,
            paths: 0,
            non_plain: false,
        };
        for count in &counts {
            out.budget.charge_work(2)?;
            if let Some(count) = count {
                total.atoms = add(total.atoms, count.atoms)?;
                total.paths = add(total.paths, count.paths)?;
            }
        }
        if total.atoms > MAX_COMPONENTS {
            return Err(Error::Statement(
                "original product component census exceeds structural limit",
            ));
        }
        let mut components = vector(total.atoms, out)?;
        let mut paths = vector(total.paths, out)?;
        let mut roots = vector(types.len(), out)?;
        let mut walk = vector(types.len(), out)?;
        let mut path = vector(types.len(), out)?;
        for root in 0..types.len() {
            out.budget.charge_work(2)?;
            let first = components.len();
            if counts[root].is_some() {
                walk.push(WalkFrame {
                    ty: TypeId::from_index(u32::try_from(root).map_err(|_| Resource::Arithmetic)?),
                    next: 0,
                    depth: 0,
                });
                while let Some(frame) = walk.last().copied() {
                    out.budget.charge_work(4)?;
                    path.truncate(frame.depth);
                    let child = match kinds[frame.ty.index() as usize] {
                        Kind::Atom(atom) => {
                            let begin = paths.len();
                            out.budget.charge_work(path.len())?;
                            if add(paths.len(), path.len())? > paths.capacity()
                                || components.len() == components.capacity()
                            {
                                return Err(Resource::Accounting.into());
                            }
                            paths.extend_from_slice(&path);
                            components.push(Component {
                                path: begin..paths.len(),
                                ty: frame.ty,
                                atom,
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
                        walk.last_mut().ok_or_else(mismatch)?.next = add(frame.next, 1)?;
                        if path.len() == path.capacity() || walk.len() == walk.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        path.push(u32::try_from(frame.next).map_err(|_| Resource::Arithmetic)?);
                        walk.push(WalkFrame {
                            ty: child,
                            next: 0,
                            depth: path.len(),
                        });
                    } else {
                        walk.pop();
                    }
                }
            }
            roots.push(first..components.len());
            if components.len() - first != counts[root].map_or(0, |count| count.atoms) {
                return Err(Resource::Accounting.into());
            }
        }
        if (components.len(), paths.len()) != (total.atoms, total.paths) {
            return Err(Resource::Accounting.into());
        }
        let credit = add(
            mul(walk.capacity(), size_of::<WalkFrame>())?,
            mul(path.capacity(), size_of::<u32>())?,
        )?;
        drop(walk);
        drop(path);
        out.budget.release_storage(credit)?;
        Ok(Self {
            kinds,
            composites,
            children,
            counts,
            roots,
            components,
            paths,
        })
    }

    pub(super) fn is_product(&self, ty: TypeId, out: &mut Writer<'_, '_>) -> Result<bool> {
        out.budget.charge_work(2)?;
        let index = ty.index() as usize;
        let count = self.counts.get(index).ok_or_else(mismatch)?;
        Ok(count.is_some_and(|count| count.non_plain)
            && matches!(
                self.kinds.get(index),
                Some(Kind::Fields { .. } | Kind::Array { .. })
            ))
    }

    pub(super) fn component_range(
        &self,
        mut ty: TypeId,
        path: &[SemanticProjectionV1],
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(Range<usize>, TypeId)>> {
        if !self.is_product(ty, out)? {
            return Ok(None);
        }
        let mut first = 0usize;
        for projection in path {
            out.budget.charge_work(5)?;
            let (next, offset) = match (projection.kind(), self.kinds[ty.index() as usize]) {
                (Projection::Field(field), Kind::Fields { first, count })
                    if (field as usize) < count =>
                {
                    let child = self.children[first + field as usize];
                    (child.ty, child.first)
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
                        mul(
                            ordinal,
                            self.counts[element.index() as usize]
                                .ok_or_else(mismatch)?
                                .atoms,
                        )?,
                    )
                }
                _ => return Ok(None),
            };
            if next != projection.result_type() {
                return Err(mismatch());
            }
            first = add(first, offset)?;
            ty = next;
        }
        let count = self.counts[ty.index() as usize].ok_or_else(mismatch)?.atoms;
        Ok(Some((first..add(first, count)?, ty)))
    }
}

fn execution_role_code(role: ExecutionRole) -> Result<u8> {
    match role {
        ExecutionRole::KernelContext => Ok(0),
        ExecutionRole::Workgroup => Ok(1),
        _ => Err(mismatch()),
    }
}

#[derive(Clone, Copy)]
pub(in super::super) struct SourceProductComponentV282<'a, 'view, 'source> {
    slots: &'a SourceSlots<'view, 'source>,
    index: usize,
}

impl<'a, 'view, 'source> SourceProductComponentV282<'a, 'view, 'source> {
    fn row(&self, out: &mut Writer<'_, '_>) -> Result<&'a Component> {
        self.slots.with_source_query_v42(out, |out| {
            out.budget.charge_work(1)?;
            self.slots
                .products
                .components
                .get(self.index)
                .ok_or_else(mismatch)
        })
    }

    pub(in super::super) fn path(&self, out: &mut Writer<'_, '_>) -> Result<&'a [u32]> {
        self.slots
            .products
            .paths
            .get(self.row(out)?.path.clone())
            .ok_or_else(mismatch)
    }

    pub(in super::super) fn source_type(&self, out: &mut Writer<'_, '_>) -> Result<TypeId> {
        Ok(self.row(out)?.ty)
    }

    pub(in super::super) fn atom(&self, out: &mut Writer<'_, '_>) -> Result<Atom> {
        Ok(self.row(out)?.atom)
    }
}

impl<'view, 'source> SourceSlots<'view, 'source> {
    pub(in super::super) fn product_type_supported_v282(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.with_source_query_v42(out, |out| {
            out.budget.charge_work(2)?;
            Ok(!self
                .products
                .roots
                .get(ty.index() as usize)
                .ok_or_else(mismatch)?
                .is_empty())
        })
    }

    pub(in super::super) fn product_component_count_v282(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<usize>> {
        self.with_source_query_v42(out, |out| {
            if !self.products.is_product(ty, out)? {
                return Ok(None);
            }
            out.budget.charge_work(1)?;
            Ok(Some(
                self.products
                    .roots
                    .get(ty.index() as usize)
                    .ok_or_else(mismatch)?
                    .len(),
            ))
        })
    }

    pub(in super::super) fn is_product_v282(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.with_source_query_v42(out, |out| self.products.is_product(ty, out))
    }

    pub(in super::super) fn product_component_v282<'a>(
        &'a self,
        ty: TypeId,
        ordinal: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<SourceProductComponentV282<'a, 'view, 'source>> {
        self.with_source_query_v42(out, |out| {
            out.budget.charge_work(3)?;
            let range = self
                .products
                .roots
                .get(ty.index() as usize)
                .ok_or_else(mismatch)?;
            if ordinal >= range.len() {
                return Err(Error::Statement(
                    "original product component ordinal differs",
                ));
            }
            Ok(SourceProductComponentV282 {
                slots: self,
                index: add(range.start, ordinal)?,
            })
        })
    }

    pub(in super::super) fn product_component_range_v282(
        &self,
        ty: TypeId,
        path: &[SemanticProjectionV1],
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(Range<usize>, TypeId)>> {
        self.with_source_query_v42(out, |out| self.products.component_range(ty, path, out))
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceProductTypesV282>()
        + h::<CountFrame>()
        + h::<WalkFrame>()
        + h::<Component>()
        + h::<Child>()
        + h::<Kind>()
        + h::<Count>()
        + h::<Atom>()
        + 9 * size_of::<Vec<usize>>()
        + 32 * size_of::<usize>()
}
