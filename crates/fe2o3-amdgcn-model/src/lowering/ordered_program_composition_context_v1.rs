//! Private exact-owner context; never a boolean helper opt-in.
use super::*;

pub(in crate::lowering) fn validate_owner_context(
    module: &Module,
    target: LoweringTarget,
    owner: &VerifiedOrderedProgramCompositionV1,
) -> Result<(), LoweringErrors> {
    if !std::ptr::eq(module, owner.canonical().module())
        || target != LoweringTarget::Gfx942XnackMinusV1
    {
        return Err(reject(
            LoweringLocation::module(module),
            "composition requires the actual immutable owner module and gfx942:xnack-",
        ));
    }
    // Existing complete-module emission recognizes these intrinsic identities
    // specially. A structural helper must stay an ordinary retained definition,
    // never be silently suppressed or reinterpreted as a builtin call.
    for helper in owner.helpers() {
        let function = &module.functions[helper.function_ordinal() as usize];
        if FloatOperation::from_intrinsic_id(&function.id).is_some()
            || AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id).is_some()
        {
            return Err(reject(
                LoweringLocation::device_function(module, function),
                "composition helper cannot use a specially interpreted intrinsic identity",
            ));
        }
    }
    Ok(())
}
fn reject(location: LoweringLocation, message: &'static str) -> LoweringErrors {
    LoweringErrors::one(
        location,
        LoweringDiagnosticCode::UnsupportedInlineAssembly,
        message,
    )
}

impl FunctionLowerer<'_> {
    pub(in crate::lowering) fn validate_ordered_composition_v1(
        &self,
        operation: &Operation,
        location: &LoweringLocation,
        owner: &VerifiedOrderedProgramCompositionV1,
    ) -> Result<(), LoweringErrors> {
        validate_owner_context(self.module, self.target, owner)?;
        if self.wave_width != Some(WaveWidth::Wave64) {
            return Err(reject(
                location.clone(),
                "composition function must inherit exact Wave64",
            ));
        }
        let module = owner.canonical().module();
        let Some(function_ordinal) = module
            .functions
            .iter()
            .position(|f| std::ptr::eq(f, self.function))
        else {
            return Err(reject(location.clone(), "composition function is foreign"));
        };
        if function_ordinal == owner.root_function_ordinal() as usize {
            if self
                .kernel
                .is_none_or(|kernel| !std::ptr::eq(kernel, &module.kernels[0]))
                || self.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
            {
                return Err(reject(
                    location.clone(),
                    "composition root geometry changed",
                ));
            }
        } else if self.kernel.is_some()
            || self.workgroup_size.is_some()
            || !owner
                .helpers()
                .iter()
                .any(|h| h.function_ordinal() as usize == function_ordinal)
        {
            return Err(reject(
                location.clone(),
                "composition helper is not a retained ordinary device function",
            ));
        }
        let definition = owner
            .definitions()
            .iter()
            .find(|definition| {
                let site = definition.site();
                site.function_ordinal() as usize == function_ordinal
                    && Some(site.block()) == location.block
                    && Some(site.operation_ordinal() as usize) == location.operation
            })
            .ok_or_else(|| {
                reject(
                    location.clone(),
                    "operation is absent from exact composition definition roster",
                )
            })?;
        let site = definition.site();
        let actual = module.functions[function_ordinal]
            .body
            .as_ref()
            .and_then(|body| body.blocks.get(site.block_ordinal() as usize))
            .filter(|block| block.id == site.block())
            .and_then(|block| block.operations.get(site.operation_ordinal() as usize))
            .ok_or_else(|| reject(location.clone(), "composition operation coordinate changed"))?;
        if !std::ptr::eq(actual, operation) {
            return Err(reject(
                location.clone(),
                "composition operation is not the actual retained operation",
            ));
        }
        fe2o3_kernel_ir::validate_gfx942_ordered_program_v1(operation, |value| {
            self.bindings
                .get(&value)
                .and_then(ValueBinding::value)
                .and_then(|(_, ty)| ty.as_scalar())
        })
        .map_err(|error| {
            LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedInlineAssembly,
                format!("invalid composition program: {error}"),
            )
        })?;
        // Prefix/callee/control-flow policy belongs to the structural owner.
        // Ordinary function target/type/CFG preflight still runs unchanged.
        Ok(())
    }
}

impl CapacityLimitedText {
    pub(in crate::lowering) fn try_new_composition_v1(
        module: &Module,
    ) -> Result<Self, LoweringErrors> {
        let mut output = Self::try_new(module, MAX_COMPILER_MODULE_TEXT_BYTES)?;
        output
            .output
            .try_reserve_exact(MAX_COMPILER_MODULE_TEXT_BYTES)
            .map_err(|_| {
                LoweringErrors::one(
                    LoweringLocation::module(module),
                    LoweringDiagnosticCode::AllocationFailure,
                    "could not reserve exact bounded composition LLVM output",
                )
            })?;
        if output.output.capacity() != MAX_COMPILER_MODULE_TEXT_BYTES {
            return Err(LoweringErrors::one(
                LoweringLocation::module(module),
                LoweringDiagnosticCode::AllocationFailure,
                "composition LLVM output allocator capacity differs from prepaid bound",
            ));
        }
        Ok(output)
    }
}
