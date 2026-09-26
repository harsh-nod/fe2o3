//! Complete target syntax decisions over the one immutable V18 graph.

use super::storage_native_v18::StorageEmissionContextV18 as Context;
use super::*;
use fe2o3_kernel_ir::{
    StorageLayoutKindV1 as Kind, StorageOperationV1 as Storage, StorageProjectionV1 as Projection,
};

pub(super) fn check(context: &Context<'_>) -> Result<(), LoweringErrors> {
    let module = context.owner.module();
    context.check_current(module, context.target)?;
    let view = context.owner.verified_storage_module_ref_v1();
    context.charge(
        1 + fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1.len()
            + fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1.len(),
    )?;
    let pointers = storage_v1::StorageTargetContextV1::new(
        view.storage().layouts(),
        context.target,
        fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
        fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
    )
    .map_err(|_| context.reject("storage target layout is not the exact reviewed profile"))?;
    for (ordinal, row) in module.storage_layouts.iter().enumerate() {
        context.charge(1)?;
        if row.size == 0 || row.size > u64::from(u32::MAX) {
            return Err(context
                .reject("zero-sized or segment-unaddressable storage rows are not implemented"));
        }
        match &row.kind {
            Kind::Scalar(scalar) => {
                if !supported_scalar(*scalar, context.target)
                    || (*scalar == ScalarType::Index && row.size != 8)
                {
                    return Err(
                        context.reject("storage scalar differs from the exact AMDGPU value layout")
                    );
                }
            }
            Kind::Vector(vector) => {
                if !supported_scalar(vector.element, context.target) {
                    return Err(
                        context.reject("storage vector scalar is not supported by this target")
                    );
                }
            }
            Kind::Pointer(_) => {
                pointers
                    .pointer_recipe(fe2o3_kernel_ir::StorageLayoutIdV1(ordinal as u32))
                    .map_err(|_| {
                        context.reject("storage pointer row has no exact AMDGPU representation")
                    })?;
            }
            Kind::Record(fields) | Kind::Union(fields) => context.charge(fields.len())?,
            Kind::Array { .. } => {}
            Kind::Slice { length, .. } => {
                context.charge(2)?;
                if context.row(length.layout)?.size != 8 {
                    return Err(context.reject(
                        "storage slice length differs from the 64-bit target index layout",
                    ));
                }
            }
            Kind::Variants { variants, .. } => context.charge(variants.len())?,
        }
    }
    if let Some(roles) = context.root_roles {
        roles.validate(context)?;
    }
    for function in &module.functions {
        context.charge(1 + function.id.as_str().len())?;
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            check_type(context, ty, false)?;
        }
        if function.role != FunctionRole::InternalHelper
            && function.signature.results.iter().any(extended_type)
        {
            return Err(context
                .reject("storage value exports require an authenticated physical ABI successor"));
        }
        if function.body.is_none() && function.signature.parameters.iter().any(extended_type) {
            return Err(
                context.reject("external storage ABI is not admitted by this target component")
            );
        }
        if let Some(body) = &function.body {
            for block in &body.blocks {
                context.charge(1)?;
                for parameter in &block.parameters {
                    check_type(context, &parameter.ty, false)?;
                }
                for operation in &block.operations {
                    context.charge(1)?;
                    for result in &operation.results {
                        check_type(context, &result.ty, false)?;
                    }
                    match &operation.kind {
                        OperationKind::Storage(Storage::CopyObject {
                            source_access,
                            destination_access,
                            ..
                        }) => {
                            if source_access.volatile || destination_access.volatile {
                                return Err(context.reject("volatile object copy requires a precise endpoint snapshot lowering"));
                            }
                        }
                        OperationKind::Storage(_) => {}
                        OperationKind::Alloca { element, .. }
                        | OperationKind::WorkgroupMemory(fe2o3_kernel_ir::WorkgroupMemory {
                            element,
                            ..
                        }) => check_type(context, element, true)?,
                        OperationKind::Execution(_)
                        | OperationKind::VerificationContract(_)
                        | OperationKind::Gfx942OrderedRegion(_)
                        | OperationKind::Gfx942OrderedProgram(_)
                        | OperationKind::VectorLoad(_)
                        | OperationKind::VectorStore(_)
                        | OperationKind::VectorLayoutConvert(_) => {
                            return Err(context.reject(
                                "V18 transport does not authorize another target operation profile",
                            ));
                        }
                        OperationKind::Cast { to, .. } => check_type(context, to, false)?,
                        OperationKind::Intrinsic(intrinsic) => {
                            check_type(context, &intrinsic.result_type, false)?
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn extended_type(mut ty: &Type) -> bool {
    loop {
        match ty {
            Type::Scalar(_) | Type::Unit => return false,
            Type::Pointer(pointer) => {
                if matches!(
                    pointer.address_space,
                    KernelAddressSpace::Constant | KernelAddressSpace::Generic
                ) {
                    return true;
                }
                ty = &pointer.pointee;
            }
            Type::Slice(_) | Type::Vector(_) | Type::StorageObject(_) | Type::Execution(_) => {
                return true;
            }
        }
    }
}

pub(super) fn check_type(
    context: &Context<'_>,
    mut ty: &Type,
    mut nested: bool,
) -> Result<(), LoweringErrors> {
    loop {
        context.charge(1)?;
        match ty {
            Type::Scalar(s) if supported_scalar(*s, context.target) => return Ok(()),
            Type::Vector(v) if supported_scalar(v.element, context.target) => return Ok(()),
            Type::StorageObject(id) if nested => {
                context.row(*id)?;
                return Ok(());
            }
            Type::Pointer(p) => {
                ty = &p.pointee;
                nested = true;
            }
            Type::Slice(s) => {
                ty = &s.element;
                nested = true;
            }
            Type::Unit if nested => return Ok(()),
            _ => return Err(context.reject("unsupported storage target value type")),
        }
    }
}

pub(super) fn signature(context: &Context<'_>, function: &Function) -> Result<(), LoweringErrors> {
    for ty in function
        .signature
        .parameters
        .iter()
        .chain(&function.signature.results)
    {
        check_type(context, ty, false)?;
    }
    if function.signature.results.len() > MAX_INTERNAL_HELPER_RESULT_COMPONENTS_V1 {
        return Err(context.reject("storage helper result component limit exceeded"));
    }
    Ok(())
}

impl FunctionLowerer<'_> {
    fn storage_narrow_type(&self, ty: &Type) -> Result<(), LoweringErrors> {
        if let Type::Vector(vector) = ty {
            self.validate_narrow_type_capability(
                &Type::Scalar(vector.element),
                &self.function_location(),
            )
        } else {
            self.validate_narrow_type_capability(ty, &self.function_location())
        }
    }

    pub(super) fn storage_context(&self) -> Result<&Context<'_>, LoweringErrors> {
        let context = self.storage_v18.ok_or_else(|| {
            LoweringErrors::one(
                self.function_location(),
                LoweringDiagnosticCode::UnsupportedOperation,
                "storage operation has no V18 owner",
            )
        })?;
        context.check_current(self.module, self.target)?;
        Ok(context)
    }

    pub(super) fn storage_validate_parameters(&mut self) -> Result<(), LoweringErrors> {
        let context = self.storage_v18.expect("V18 branch");
        let body = self
            .function
            .body
            .as_ref()
            .ok_or_else(|| context.reject("storage function body is absent"))?;
        // Binding storage and names are owned by the shared emitter's bounded
        // graph policy; this pass does not introduce another value map.
        for (index, (&value, ty)) in body
            .parameters
            .iter()
            .zip(&self.function.signature.parameters)
            .enumerate()
        {
            check_type(context, ty, false)?;
            self.storage_narrow_type(ty)?;
            let role = match (context.root_roles, self.kernel) {
                (Some(roles), Some(kernel)) => {
                    Some(roles.parameter(context, kernel, self.function, index)?)
                }
                _ => None,
            };
            if self.kernel.is_some()
                && matches!(ty, Type::Pointer(p) if p.address_space != KernelAddressSpace::Global)
                && !matches!(role, Some(RootParameterRoleV29::InlineObject(_)))
            {
                return Err(
                    context.reject("root pointer parameters require real global allocation ABI")
                );
            }
            if self.kernel.is_some()
                && matches!(ty, Type::Slice(s) if s.address_space != KernelAddressSpace::Global)
            {
                return Err(
                    context.reject("root slice parameters require real global allocation ABI")
                );
            }
            self.bindings
                .insert(value, storage_binding(ty, format!("%arg{index}"), None));
        }
        for block in &body.blocks {
            context.charge(1)?;
            for parameter in &block.parameters {
                check_type(context, &parameter.ty, false)?;
                self.storage_narrow_type(&parameter.ty)?;
                self.bindings.insert(
                    parameter.id,
                    storage_binding(&parameter.ty, value_name(parameter.id), None),
                );
            }
            for operation in &block.operations {
                context.charge(1)?;
                for result in &operation.results {
                    check_type(context, &result.ty, false)?;
                    self.storage_narrow_type(&result.ty)?;
                    let name = match &operation.kind {
                        OperationKind::Constant(c) => {
                            constant_value(c).unwrap_or_else(|| value_name(result.id))
                        }
                        _ => value_name(result.id),
                    };
                    self.bindings.insert(
                        result.id,
                        storage_binding(&result.ty, name, direct_unsigned_constant_v1(operation)),
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn storage_validate_operation(
        &self,
        operation: &Operation,
    ) -> Result<bool, LoweringErrors> {
        let context = self.storage_context()?;
        match &operation.kind {
            OperationKind::Storage(op) => {
                if let Storage::Project {
                    base,
                    step: Projection::Variant { .. },
                } | Storage::ReadDiscriminant { address: base, .. } = op
                {
                    let row = self.storage_address(*base)?.1;
                    if let Kind::Variants { encoding, .. } = row.kind {
                        if let Kind::Pointer(pointer) = context.row(encoding.tag().layout)?.kind {
                            if !matches!(encoding, fe2o3_kernel_ir::StorageVariantEncodingV1::Niche { .. }) {
                                return Err(context.reject("pointer discriminant requires a niche encoding"));
                            }
                            storage_v1::pointer_encoding(context.target, pointer)
                                .map_err(|_| context.reject("pointer niche has no exact LLVM target encoding"))?;
                        }
                    }
                }
                Ok(true)
            }
            OperationKind::Alloca {
                element: Type::StorageObject(id),
                address_space,
                alignment,
                ..
            } => {
                let row = context.row(*id)?;
                if *address_space != KernelAddressSpace::Private || *alignment < row.alignment {
                    return Err(context.reject(
                        "storage allocation requires private space and actual object alignment",
                    ));
                }
                Ok(true)
            }
            OperationKind::WorkgroupMemory(memory)
                if matches!(memory.element, Type::StorageObject(_)) =>
            {
                if self.kernel.is_none() {
                    return Err(context.reject("helpers cannot declare kernel LDS objects"));
                }
                let Type::StorageObject(id) = memory.element else {
                    unreachable!()
                };
                let row = context.row(id)?;
                if memory.alignment < row.alignment {
                    return Err(context.reject("storage LDS alignment is insufficient"));
                }
                if let WorkgroupMemoryExtent::Static(count) = memory.extent {
                    if count == 0 {
                        return Err(context.reject("empty storage LDS objects are not implemented"));
                    }
                    row.size
                        .checked_mul(u64::from(count))
                        .filter(|n| *n <= u64::from(u32::MAX))
                        .ok_or_else(|| {
                            context.reject("storage LDS extent exceeds addressable segment")
                        })?;
                }
                Ok(true)
            }
            OperationKind::Select { true_value, .. }
                if extended_type(self.value_type(*true_value)) =>
            {
                Ok(true)
            }
            OperationKind::GetElementPointer { base: pointer, .. }
            | OperationKind::Load { pointer, .. }
            | OperationKind::GuardedLoad { pointer, .. }
            | OperationKind::Store { pointer, .. }
            | OperationKind::GuardedStore { pointer, .. }
                if matches!(self.value_type(*pointer), Type::Pointer(ty)
                    if ty.address_space == KernelAddressSpace::Generic) =>
            {
                let Type::Pointer(pointer) = self.value_type(*pointer) else {
                    unreachable!("checked generic pointer operand")
                };
                if !supported_memory_type(&pointer.pointee, self.target) {
                    return Err(context.reject("unsupported generic memory pointee type"));
                }
                Ok(true)
            }
            OperationKind::Call { callee, arguments }
                if self
                    .module
                    .function(callee)
                    .is_some_and(|f| f.role == FunctionRole::InternalHelper)
                    && FloatOperation::from_intrinsic_call(callee, arguments).is_none()
                    && AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                        .is_none() =>
            {
                signature(
                    context,
                    self.module.function(callee).expect("checked callee"),
                )?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

fn storage_binding(ty: &Type, name: String, unsigned_constant: Option<u64>) -> ValueBinding {
    match ty {
        Type::Slice(_) => ValueBinding::Slice {
            data_name: format!("{name}.data"),
            length_name: format!("{name}.len"),
            ty: ty.clone(),
        },
        _ => ValueBinding::Value {
            llvm_name: name,
            ty: ty.clone(),
            unsigned_constant,
        },
    }
}
