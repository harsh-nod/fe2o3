fn require_publication_view_v1(
    view: ProductionRankedValueV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<(), ProductionRankedKernelErrorV1> {
    match require_value(view, argument_count, locals)? {
        RecipeValueKindV1::View {
            rank: 1,
            element_width: 32,
            writable: true,
            dynamic_extent: Some(_),
            memory_space: MemorySpaceAttr::Global,
            ..
        } => Ok(()),
        _ => Err(ProductionRankedKernelErrorV1::InvalidShape),
    }
}

fn validate_publication_recipe_v1(
    operation: &ProductionRankedOperationV1,
    argument_count: usize,
    locals: &[RecipeValueKindV1],
) -> Result<Option<(ProductionRankedValueIdV1, RecipeValueKindV1)>, ProductionRankedKernelErrorV1> {
    match operation {
        ProductionRankedOperationV1::PublicationAtomicStoreU32 { view, index, value } => {
            require_publication_view_v1(*view, argument_count, locals)?;
            require_index(*index, argument_count, locals)?;
            if !matches!(value, 1 | 2) {
                return Err(ProductionRankedKernelErrorV1::InvalidShape);
            }
            Ok(None)
        }
        ProductionRankedOperationV1::PublicationAtomicLoadU32 {
            result,
            view,
            index,
        } => {
            require_publication_view_v1(*view, argument_count, locals)?;
            require_index(*index, argument_count, locals)?;
            Ok(Some((
                *result,
                RecipeValueKindV1::PublicationAcquiredU32 {
                    view: *view,
                    index: *index,
                },
            )))
        }
        ProductionRankedOperationV1::PublicationReadGuard {
            result,
            index,
            physical_extent,
            acquired,
            ..
        } => {
            require_index(*index, argument_count, locals)?;
            require_index(*physical_extent, argument_count, locals)?;
            let RecipeValueKindV1::PublicationAcquiredU32 {
                view,
                index: acquired_index,
            } = require_value(*acquired, argument_count, locals)?
            else {
                return Err(ProductionRankedKernelErrorV1::InvalidShape);
            };
            require_publication_view_v1(view, argument_count, locals)?;
            if acquired_index != *index {
                return Err(ProductionRankedKernelErrorV1::InvalidShape);
            }
            Ok(Some((*result, RecipeValueKindV1::Index)))
        }
        _ => Err(ProductionRankedKernelErrorV1::InvalidShape),
    }
}

type PublicationMaterializationV1 = (Ptr<Operation>, Option<(ProductionRankedValueIdV1, Value)>);

fn materialize_publication_recipe_v1(
    context: &mut pliron::context::Context,
    recipe: &ProductionRankedOperationV1,
    arguments: &[Value],
    locals: &mut Vec<Value>,
    block_arguments: &HashMap<(u32, u32), Value>,
) -> Result<PublicationMaterializationV1, ProductionRankedKernelErrorV1> {
    let failure = || {
        ProductionRankedKernelErrorV1::Materialization(
            "validated static publication operation failed materialization",
        )
    };
    match recipe {
        ProductionRankedOperationV1::PublicationAtomicStoreU32 { view, index, value } => {
            let op = RankedAccessOp::new_publication_store_u32(
                context,
                resolve_value(*view, arguments, locals, block_arguments)?,
                resolve_value(*index, arguments, locals, block_arguments)?,
                *value,
            )
            .map_err(|_| failure())?;
            Ok((op.get_operation(), None))
        }
        ProductionRankedOperationV1::PublicationAtomicLoadU32 {
            result,
            view,
            index,
        } => {
            let op = RankedAccessOp::new_publication_load_u32(
                context,
                resolve_value(*view, arguments, locals, block_arguments)?,
                resolve_value(*index, arguments, locals, block_arguments)?,
            )
            .map_err(|_| failure())?;
            let value = op.publication_read_result(context).ok_or_else(failure)?;
            Ok((op.get_operation(), Some((*result, value))))
        }
        ProductionRankedOperationV1::PublicationReadGuard {
            result,
            success,
            index,
            physical_extent,
            acquired,
        } => {
            let op = PublicationReadGuardOp::new(
                context,
                resolve_value(*index, arguments, locals, block_arguments)?,
                resolve_value(*physical_extent, arguments, locals, block_arguments)?,
                resolve_value(*acquired, arguments, locals, block_arguments)?,
            )
            .map_err(|_| failure())?;
            if result.get() as usize != locals.len()
                || result.get().checked_add(1) != Some(success.get())
            {
                return Err(failure());
            }
            locals.push(op.result(context));
            locals.push(op.success(context));
            Ok((op.get_operation(), None))
        }
        _ => Err(failure()),
    }
}

#[cfg(test)]
#[path = "publication_v1_tests.rs"]
mod publication_recipe_tests;
