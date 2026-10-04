//! Fresh source-owner replay for durable conditional facts. Copied coordinates
//! describe a checked binding; they cannot replace its live relation or account.
use super::*;
use crate::ProductionSourceArgumentErrorV1;
use fe2o3_kernel_ir::ValueId;
use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalIdV1, SemanticTypeIdV1};

type Error = ProductionConditionalAggregateErrorV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Descriptive coordinates copied from a checked whole-argument binding.
/// Consumers cannot substitute these values for a fresh owner/ledger replay.
pub struct ProductionConditionalSourceCoordinatesV1 {
    parameter: u32,
    value: ValueId,
    source: u32,
    adjusted: u32,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
}

impl ProductionConditionalSourceCoordinatesV1 {
    pub const fn canonical_parameter(self) -> u32 {
        self.parameter
    }
    pub const fn canonical_value(self) -> ValueId {
        self.value
    }
    pub const fn source_argument(self) -> u32 {
        self.source
    }
    pub const fn adjusted_argument(self) -> u32 {
        self.adjusted
    }
    pub const fn semantic_local(self) -> SemanticLocalIdV1 {
        self.local
    }
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.ty
    }
    /// Allocation origins follow original source arguments, not CPU/FnAbi slots.
    pub fn allocation_origin(self) -> u64 {
        u64::from(self.source) + 1
    }
}

fn source_error(error: ProductionSourceArgumentErrorV1) -> Error {
    match error {
        ProductionSourceArgumentErrorV1::ArgumentCorrespondenceResource(error) => {
            Error::Resource(error)
        }
        error => Error::Source(error),
    }
}

pub(super) fn replay_canonical(
    source: &ProductionSourceArgumentRelationV1<'_, '_>,
    canonical: &ConditionalTotalViewFactsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    source
        .replay_v1(canonical.module(), canonical.function(), budget)
        .map_err(source_error)
}

pub(super) fn bind(
    source: &ProductionSourceArgumentRelationV1<'_, '_>,
    parameter: u32,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<ProductionConditionalSourceCoordinatesV1, Error> {
    let binding = source
        .bind_whole_parameter_v1(parameter, value, budget)
        .map_err(source_error)?;
    budget.charge_work(7)?;
    Ok(ProductionConditionalSourceCoordinatesV1 {
        parameter: binding.canonical_parameter(),
        value: binding.canonical_value(),
        source: binding.source_argument(),
        adjusted: binding.adjusted_argument(),
        local: binding.semantic_local(),
        ty: binding.semantic_type(),
    })
}

pub(super) fn require_global_x(
    kernel: &ProductionRankedKernelV1,
    index: ProductionRankedValueV1,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    use dialect_kernel::DYNAMIC_EXTENT;
    let mut invocation = false;
    let mut layout = false;
    for block in kernel.blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(5)?;
            match operation {
                ProductionRankedOperationV1::InvocationIndex {
                    result,
                    dimension,
                    launch_extent,
                } => {
                    if invocation
                        || ProductionRankedValueV1::Local(*result) != index
                        || *dimension != 0
                        || *launch_extent != DYNAMIC_EXTENT
                    {
                        return Err(Error::Subject("conditional global-X index"));
                    }
                    invocation = true;
                }
                ProductionRankedOperationV1::ExecutionLayout { global_extents, .. } => {
                    if layout || *global_extents != [DYNAMIC_EXTENT, 1, 1] {
                        return Err(Error::Subject("conditional D1 execution layout"));
                    }
                    layout = true;
                }
                _ => {}
            }
        }
    }
    if !invocation || !layout {
        return Err(Error::Subject("missing conditional global-X domain"));
    }
    Ok(())
}

/// Join exactly one global whole-slice recipe view to its checked source origin.
/// The returned operand is the input/output view's own length, not an ordinal
/// inferred from the source parameter or a different view's length.
pub(super) fn view_extent(
    kernel: &ProductionRankedKernelV1,
    view: ProductionRankedValueV1,
    source: ProductionConditionalSourceCoordinatesV1,
    element_bytes: u64,
    output: bool,
    budget: &mut Budget<'_>,
) -> Result<ProductionRankedValueV1, Error> {
    use ProductionRankedOperationV1 as O;
    use dialect_kernel::{DYNAMIC_EXTENT, MemorySpaceAttr};
    let width = element_bytes
        .checked_mul(8)
        .ok_or(ResourceError::Arithmetic)?;
    let mut extent = None;
    for block in kernel.blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(12)?;
            match operation {
                O::View {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    ..
                }
                | O::ViewInSpace {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    ..
                } if ProductionRankedValueV1::Local(*result) == view => {
                    let [value @ ProductionRankedValueV1::Argument(_)] = dynamic_extents.as_slice()
                    else {
                        return Err(Error::Subject("source-bound whole-slice extent"));
                    };
                    if extent.replace(*value).is_some()
                        || *allocation_origin != source.allocation_origin()
                        || u64::from(*element_width) != width
                        || *writable != output
                        || shape.as_slice() != [DYNAMIC_EXTENT]
                        || matches!(operation, O::ViewInSpace { memory_space, .. }
                            if *memory_space != MemorySpaceAttr::Global)
                    {
                        return Err(Error::Subject("source-bound whole-slice view"));
                    }
                }
                _ => {}
            }
        }
    }
    extent.ok_or(Error::Subject("missing source-bound whole-slice view"))
}

impl ProductionConditionalFinalGraphV1<'_> {
    pub(super) fn require_source_v1(
        &self,
        source: &ProductionSourceArgumentRelationV1<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        source
            .require_source_owner_v1(self.source_owner, budget)
            .map_err(source_error)?;
        budget.charge_work(36)?;
        if !std::ptr::eq(source.association(), self.source_association)
            || source.source_semantic_identity() != self.source_semantic_identity.as_bytes()
        {
            return Err(Error::Subject("conditional source/root owner substitution"));
        }
        replay_canonical(source, &self.canonical, budget)?;
        let kernel = self.pipeline.kernel().map_err(Error::Session)?;
        let output = self.output[0];
        require_global_x(kernel, output.index, budget)?;
        let actual = bind(
            source,
            self.canonical.output_parameter_index(),
            self.canonical.output_value(),
            budget,
        )?;
        if actual != output.source
            || view_extent(
                kernel,
                output.view,
                actual,
                self.canonical.element_bytes(),
                true,
                budget,
            )? != output.extent
        {
            return Err(Error::Subject(
                "conditional output source coordinates changed",
            ));
        }
        for read in &self.reads {
            budget.charge_work(7)?;
            let actual = bind(
                source,
                read.canonical.parameter(),
                read.canonical.slice(),
                budget,
            )?;
            if actual != read.source {
                return Err(Error::Subject(
                    "conditional read source coordinates changed",
                ));
            }
            view_extent(
                kernel,
                read.view,
                actual,
                read.canonical.element_bytes(),
                false,
                budget,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "conditional_aggregate_source_views_v1_tests.rs"]
mod tests;

pub(super) fn emit(
    source: ProductionConditionalSourceCoordinatesV1,
    put: &mut impl FnMut(&[u8]) -> Result<(), ResourceError>,
) -> Result<(), ResourceError> {
    for value in [
        source.parameter,
        source.value.0,
        source.source,
        source.adjusted,
        source.local.index(),
        source.ty.index(),
    ] {
        put(&value.to_le_bytes())?;
    }
    put(&source.allocation_origin().to_le_bytes())
}
