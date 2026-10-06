//! Source descriptor classes are learned from the admitted root ABI, not from
//! tuple layout or a canonical type. Cached rows remain tied to SourceSlots.

use super::*;
use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Definition, ScalarType};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1 as Callable, SemanticCompilerIntrinsicOperationV1 as Intrinsic,
    SemanticDisjointIndexSpaceV1 as IndexSpace, SemanticLocalIdV1 as Local,
    SemanticLocalRoleV1 as Role, SemanticScalarTypeV1 as Scalar, SemanticTypeIdV1 as TypeId,
    SemanticTypeShapeV1 as Shape,
};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct SliceClass {
    pub metadata_bits: u32,
    pub element: ScalarType,
    pub readable: bool,
    pub writable: bool,
    pub exclusive: bool,
}

#[derive(Clone, Copy, Debug)]
struct Argument {
    local: Local,
    ty: TypeId,
    parameter: Option<Definition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WitnessClass {
    raw_index: TypeId,
    raw_bits: u16,
    disjoint: bool,
    index_space: IndexSpace,
}

pub(super) struct SourceAbi {
    types: Vec<Option<SliceClass>>,
    roots: Vec<Range<usize>>,
    arguments: Vec<Option<Argument>>,
    witnesses: Vec<Option<WitnessClass>>,
}

fn needs_descriptor(shape: &Shape) -> bool {
    !matches!(
        shape,
        Shape::Unit | Shape::Scalar(_) | Shape::ValidityScalar(_) | Shape::Pointer(_)
    )
}

impl SourceAbi {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        relation: &Correspondence<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let source = relation.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let semantic = source.source_semantic(out.budget)?;
        let count = source.root_count(out.budget)?;
        let mut arguments_count = 0usize;
        for root in 0..count {
            out.budget.charge_work(1)?;
            let function = plan.root(root, out)?.function;
            let declaration = semantic
                .functions()
                .get(function.index() as usize)
                .ok_or_else(mismatch)?;
            arguments_count = arguments_count
                .checked_add(declaration.abi().source_input_types().len())
                .ok_or(Resource::Arithmetic)?;
        }
        let mut types = vector(semantic.types().len(), out)?;
        out.budget.charge_work(semantic.types().len())?;
        types.resize(semantic.types().len(), None);
        let mut witnesses = vector(semantic.types().len(), out)?;
        out.budget.charge_work(semantic.types().len())?;
        witnesses.resize(semantic.types().len(), None);
        for callable in semantic.callables() {
            out.budget.charge_work(2)?;
            let Callable::CompilerIntrinsic { operation, .. } = callable else {
                continue;
            };
            let (rows, raw_index) = match operation {
                Intrinsic::ThreadIndex1d {
                    index_witness,
                    raw_index,
                } => (
                    [Some((*index_witness, false, IndexSpace::Index1d)), None],
                    *raw_index,
                ),
                Intrinsic::ThreadIndexIntoDisjoint {
                    input_witness,
                    output_witness,
                    raw_index,
                    index_space,
                } => (
                    [
                        Some((*input_witness, false, *index_space)),
                        Some((*output_witness, true, *index_space)),
                    ],
                    *raw_index,
                ),
                _ => continue,
            };
            let Some(Shape::Scalar(Scalar::Integer {
                signed: false,
                bits: raw_bits @ (32 | 64),
            })) = semantic
                .types()
                .get(raw_index.index() as usize)
                .map(|ty| ty.shape())
            else {
                return Err(mismatch());
            };
            for (ty, disjoint, index_space) in rows.into_iter().flatten() {
                out.budget.charge_work(4)?;
                let class = WitnessClass {
                    raw_index,
                    raw_bits: *raw_bits,
                    disjoint,
                    index_space,
                };
                let stored = witnesses
                    .get_mut(ty.index() as usize)
                    .ok_or_else(mismatch)?;
                if stored.is_some_and(|prior| prior != class) {
                    return Err(mismatch());
                }
                *stored = Some(class);
            }
        }
        let mut roots = vector(count, out)?;
        let mut arguments = vector(arguments_count, out)?;
        out.budget.charge_work(arguments_count)?;
        arguments.resize(arguments_count, None);
        let mut first = 0usize;
        for root in 0..count {
            let function = plan.root(root, out)?.function;
            let declaration = semantic
                .functions()
                .get(function.index() as usize)
                .ok_or_else(mismatch)?;
            let inputs = declaration.abi().source_input_types();
            let end = first
                .checked_add(inputs.len())
                .ok_or(Resource::Arithmetic)?;
            let mut descriptor = false;
            for (local, row) in declaration.locals().iter().enumerate() {
                out.budget.charge_work(3)?;
                let Role::Argument(argument) = row.role() else {
                    continue;
                };
                let position = first
                    .checked_add(argument as usize)
                    .ok_or(Resource::Arithmetic)?;
                if position >= end || inputs.get(argument as usize) != Some(&row.ty()) {
                    return Err(mismatch());
                }
                let ty = semantic
                    .types()
                    .get(row.ty().index() as usize)
                    .ok_or_else(mismatch)?;
                descriptor |= needs_descriptor(ty.shape());
                if arguments[position]
                    .replace(Argument {
                        local: Local::from_index(
                            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                        ),
                        ty: row.ty(),
                        parameter: None,
                    })
                    .is_some()
                {
                    return Err(mismatch());
                }
            }
            out.budget.charge_work(inputs.len())?;
            if arguments[first..end].iter().any(Option::is_none) {
                return Err(mismatch());
            }
            if descriptor {
                // Output capacity is paid before entering the borrowed ABI scope.
                relation.with_root_slice_abis_v36(root, out.budget, |index, budget| {
                    if index.argument_count(budget)? != inputs.len() {
                        return Err(SourceError::Binding(
                            "original source ABI argument count differs",
                        ));
                    }
                    for (argument, row) in arguments[first..end].iter_mut().enumerate() {
                        budget.charge_work(4)?;
                        let row = row
                            .as_mut()
                            .ok_or(SourceError::Binding("original source ABI argument absent"))?;
                        let Some(recipe) = index.argument(
                            u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
                            row.local,
                            budget,
                        )?
                        else {
                            continue;
                        };
                        recipe.check(budget)?;
                        if recipe.function() != function
                            || recipe.argument() as usize != argument
                            || recipe.local() != row.local
                            || recipe.source_type() != row.ty
                        {
                            return Err(SourceError::Binding("original source ABI recipe differs"));
                        }
                        let declaration = semantic
                            .types()
                            .get(row.ty.index() as usize)
                            .ok_or(SourceError::Binding("original descriptor type absent"))?;
                        if declaration.identity() != recipe.source_type_identity() {
                            return Err(SourceError::Binding(
                                "original descriptor type identity differs",
                            ));
                        }
                        let class = SliceClass {
                            metadata_bits: u32::from(recipe.metadata_bits()),
                            element: recipe.element(),
                            readable: recipe.allows_reads(),
                            writable: recipe.allows_writes(),
                            exclusive: recipe.is_exclusive_contract(),
                        };
                        let stored = types
                            .get_mut(row.ty.index() as usize)
                            .ok_or(SourceError::Binding("original descriptor type slot absent"))?;
                        if stored.is_some_and(|prior| prior != class) {
                            return Err(SourceError::Binding(
                                "original descriptor type contracts differ",
                            ));
                        }
                        *stored = Some(class);
                        row.parameter = Some(recipe.parameter());
                    }
                    Ok(())
                })?;
            }
            roots.push(first..end);
            first = end;
        }
        if first != arguments_count {
            return Err(mismatch());
        }
        Ok(Self {
            types,
            roots,
            arguments,
            witnesses,
        })
    }

    pub(super) fn slice(&self, ty: TypeId, out: &mut Writer<'_, '_>) -> Result<Option<SliceClass>> {
        out.budget.charge_work(1)?;
        self.types
            .get(ty.index() as usize)
            .copied()
            .ok_or_else(mismatch)
    }

    pub(super) fn witness(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(TypeId, u16, bool, IndexSpace)>> {
        out.budget.charge_work(1)?;
        Ok(self
            .witnesses
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?
            .map(|class| {
                (
                    class.raw_index,
                    class.raw_bits,
                    class.disjoint,
                    class.index_space,
                )
            }))
    }

    pub(super) fn parameter(
        &self,
        root: usize,
        argument: usize,
        local: Local,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Definition>> {
        out.budget.charge_work(4)?;
        let range = self.roots.get(root).ok_or_else(mismatch)?;
        if argument >= range.len() {
            return Err(mismatch());
        }
        let row = self
            .arguments
            .get(
                range
                    .start
                    .checked_add(argument)
                    .ok_or(Resource::Arithmetic)?,
            )
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        if row.local != local {
            return Err(mismatch());
        }
        Ok(row.parameter)
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceAbi>()
        + h::<SliceClass>()
        + h::<WitnessClass>()
        + h::<Option<WitnessClass>>()
        + h::<Vec<Option<WitnessClass>>>()
        + h::<(TypeId, u16, bool, IndexSpace)>()
        + h::<Option<(TypeId, u16, bool, IndexSpace)>>()
        + h::<[Option<(TypeId, bool, IndexSpace)>; 2]>()
        + h::<Argument>()
        + h::<Option<Argument>>()
        + h::<Vec<Option<Argument>>>()
        + h::<Vec<Range<usize>>>()
        + h::<Vec<Option<SliceClass>>>()
        + h::<Option<SliceClass>>()
        + h::<Range<usize>>()
        + h::<Option<Definition>>()
        + h::<Local>()
        + h::<TypeId>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceRootSliceAbiV36<'_, '_>>()
        + h::<Option<fe2o3_lower_mir_kernel::ProductionSourceRootSliceAbiV36<'_, '_>>>()
        + 40 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_mir_source_abi_cache_headers_have_independent_field_envelope() {
        type Fields = (
            Vec<Option<SliceClass>>,
            Vec<Range<usize>>,
            Vec<Option<Argument>>,
            Vec<Option<WitnessClass>>,
        );
        type ArgumentFields = (Local, TypeId, Option<Definition>);
        type SliceFields = (u32, ScalarType, bool, bool, bool);
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(size_of::<SourceAbi>(), size_of::<Fields>());
        assert_eq!(size_of::<Argument>(), size_of::<ArgumentFields>());
        assert_eq!(size_of::<SliceClass>(), size_of::<SliceFields>());
        let expected = size_of::<Fields>()
            + 2 * size_of::<Result<SourceAbi>>()
            + size_of::<SliceFields>()
            + 2 * size_of::<Result<SliceClass>>()
            + h::<WitnessClass>()
            + h::<Option<WitnessClass>>()
            + h::<Vec<Option<WitnessClass>>>()
            + h::<(TypeId, u16, bool, IndexSpace)>()
            + h::<Option<(TypeId, u16, bool, IndexSpace)>>()
            + h::<[Option<(TypeId, bool, IndexSpace)>; 2]>()
            + size_of::<ArgumentFields>()
            + 2 * size_of::<Result<Argument>>()
            + h::<Option<Argument>>()
            + h::<Vec<Option<Argument>>>()
            + h::<Vec<Range<usize>>>()
            + h::<Vec<Option<SliceClass>>>()
            + h::<Option<SliceClass>>()
            + h::<Range<usize>>()
            + h::<Option<Definition>>()
            + h::<Local>()
            + h::<TypeId>()
            + h::<fe2o3_lower_mir_kernel::ProductionSourceRootSliceAbiV36<'_, '_>>()
            + h::<Option<fe2o3_lower_mir_kernel::ProductionSourceRootSliceAbiV36<'_, '_>>>()
            + 40 * size_of::<usize>()
            + 24 * size_of::<&()>();
        assert_eq!(headers(), expected);
    }
}
