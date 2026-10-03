//! Extraction-only branch before SSA construction; never a publication stage.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
use fe2o3_lower_mir_kernel::{
    ProductionCheckedU32AddCaptureRequestV1, ProductionSemanticKirLimitsV1,
    ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticCheckedBinaryOpV1, SemanticConstantValueV1,
    SemanticFunctionIdV1, SemanticFunctionRoleV1, SemanticOperandV1, SemanticRvalueKindV1,
    SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionError {
    RootCount,
    MissingEntryHelperCheckedAdd,
    AmbiguousEntryHelperCheckedAdd,
    CoordinateOverflow,
}

fn select(
    source: &AdmittedInertSemanticMirV1,
) -> Result<ProductionCheckedU32AddCaptureRequestV1, SelectionError> {
    let [root] = source.roots() else {
        return Err(SelectionError::RootCount);
    };
    let mut selected = None;
    let is_u32 = |ty: SemanticTypeIdV1| {
        matches!(
            source.types()[ty.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
    };
    for (function_index, function) in source.functions().iter().enumerate() {
        if function.role() != SemanticFunctionRoleV1::InternalHelper {
            continue;
        }
        let block = &function.blocks()[function.entry().index() as usize];
        for (ordinal, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assign) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::CheckedBinary(add) = assign.value().kind() else {
                continue;
            };
            if add.operation() != SemanticCheckedBinaryOpV1::Add {
                continue;
            }
            let SemanticOperandV1::Copy(lhs) = add.left() else {
                continue;
            };
            let SemanticOperandV1::Constant(rhs) = add.right() else {
                continue;
            };
            let SemanticConstantValueV1::Scalar(value) = rhs.value() else {
                continue;
            };
            if !lhs.projections().is_empty()
                || !assign.destination().projections().is_empty()
                || !is_u32(lhs.ty())
                || !is_u32(rhs.ty())
                || value.size_bytes() != 4
                || u32::try_from(value.bits()).is_err()
            {
                continue;
            }
            let request = ProductionCheckedU32AddCaptureRequestV1::new(
                *root,
                SemanticFunctionIdV1::from_index(
                    u32::try_from(function_index)
                        .map_err(|_| SelectionError::CoordinateOverflow)?,
                ),
                function.entry(),
                u32::try_from(ordinal).map_err(|_| SelectionError::CoordinateOverflow)?,
            );
            if selected.replace(request).is_some() {
                return Err(SelectionError::AmbiguousEntryHelperCheckedAdd);
            }
        }
    }
    selected.ok_or(SelectionError::MissingEntryHelperCheckedAdd)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// The same import owner is consumed once by capture-aware SSA/KIR lowering.
    /// Type/profile, root reachability and V8 checks remain in the existing
    /// capture constructor; the caller checks the retained prefix in process.
    pub(crate) fn extract_checked_u32_prefix_owner_v1(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<ProductionSemanticKirOwnerV1, ProductionPipelineError> {
        let transaction = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?;
        let request = select(transaction.stage.semantic_mir.semantic())
            .map_err(ProductionPipelineError::CheckedU32PrefixSelection)?;
        ProductionSemanticKirOwnerV1::try_lower_with_checked_u32_add_capture_v1(
            transaction.stage.semantic_mir,
            ProductionSemanticKirLimitsV1::default(),
            request,
            budget,
        )
        .map_err(ProductionPipelineError::CheckedU32PrefixCapture)
    }
}
