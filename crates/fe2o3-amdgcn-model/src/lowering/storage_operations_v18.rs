//! Storage operations dispatched through the existing FunctionLowerer.

use super::storage_resources_v18::NameV18;
use super::*;
use fe2o3_kernel_ir::{
    PointerType, StorageLayoutKindV1 as Kind, StorageLayoutV1, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection,
};

pub(super) fn space_v18(space: KernelAddressSpace) -> u32 {
    match space {
        KernelAddressSpace::Generic => 0,
        KernelAddressSpace::Global => 1,
        KernelAddressSpace::Workgroup => 3,
        KernelAddressSpace::Constant => 4,
        KernelAddressSpace::Private => 5,
    }
}

pub(super) fn split_operation(operation: &Operation) -> bool {
    matches!(
        &operation.kind,
        OperationKind::Storage(Storage::Project {
            step: Projection::ArrayIndex(_) | Projection::Variant { .. },
            ..
        }) | OperationKind::Storage(Storage::ReadDiscriminant { .. }) | OperationKind::Alloca {
            element: Type::StorageObject(_),
            count: Some(_),
            ..
        }
    )
}

pub(super) fn continuation(block: BlockId, operation: usize) -> String {
    format!("storage_bb{}_op{}_continue", block.0, operation)
}

impl FunctionLowerer<'_> {
    pub(super) fn storage_name(&self, args: fmt::Arguments<'_>) -> Result<NameV18, LoweringErrors> {
        let context = self.storage_context()?;
        let name = NameV18::new(args)
            .map_err(|_| context.reject("generated storage operand name overflow"))?;
        context.charge(name.as_str().len())?;
        Ok(name)
    }

    pub(super) fn storage_address(
        &self,
        value: ValueId,
    ) -> Result<(&PointerType, &StorageLayoutV1), LoweringErrors> {
        let context = self.storage_context()?;
        let Type::Pointer(pointer) = self.value_type(value) else {
            return Err(context.reject("storage address is not a pointer"));
        };
        let Type::StorageObject(id) = *pointer.pointee else {
            return Err(context.reject("storage address lost its object row"));
        };
        Ok((pointer, context.row(id)?))
    }

    pub(super) fn storage_element_bytes(
        &self,
        element: &Type,
    ) -> Result<Option<u64>, LoweringErrors> {
        let Type::StorageObject(id) = element else {
            return Ok(None);
        };
        Ok(Some(self.storage_context()?.row(*id)?.size))
    }

    pub(super) fn storage_check_branch(
        &self,
        output: &mut dyn fmt::Write,
        block: BlockId,
        ordinal: usize,
        condition: &str,
    ) {
        writeln!(
            output,
            "  br i1 {condition}, label %storage_bb{}_op{}_continue, label %storage_bb{}_op{}_fail",
            block.0, ordinal, block.0, ordinal
        )
        .unwrap();
        writeln!(output, "storage_bb{}_op{}_fail:\n  call void @llvm.trap()\n  unreachable\nstorage_bb{}_op{}_continue:", block.0, ordinal, block.0, ordinal).unwrap();
    }

    pub(super) fn emit_storage_operation(
        &self,
        output: &mut dyn fmt::Write,
        block: BlockId,
        ordinal: usize,
        operation: &Operation,
    ) -> Result<bool, LoweringErrors> {
        let context = self.storage_context()?;
        context.charge(1)?;
        let prefix = self.storage_name(format_args!("%storage.bb{}.op{}", block.0, ordinal))?;
        match &operation.kind {
            OperationKind::Cast { kind: CastKind::SliceToGeneric, value, to } => {
                let (ValueBinding::Slice { data_name, length_name, ty: Type::Slice(from) },
                    Some(ValueBinding::Slice { data_name: result_data, length_name: result_length, ty }))
                    = (&self.bindings[value], self.bindings.get(&operation.results[0].id))
                else {
                    return Err(context.reject("slice exposure binding is missing"));
                };
                if ty != to {
                    return Err(context.reject("slice exposure result type differs"));
                }
                writeln!(output, "  {result_data} = addrspacecast ptr addrspace({}) {data_name} to ptr addrspace(0)", space_v18(from.address_space)).unwrap();
                writeln!(output, "  {result_length} = add i64 {length_name}, 0").unwrap();
            }
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value,
                to,
            } => {
                let (source, from) = self.value(*value);
                let result = self.value(operation.results[0].id).0;
                let from = storage_values_v18::ValueTypeV18(from);
                let to = storage_values_v18::ValueTypeV18(to);
                writeln!(output, "  {result} = addrspacecast {from} {source} to {to}")
                    .unwrap();
            }
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                ..
            } => {
                // Shared cast validation retains exact pointee/space and permits
                // only ReadWrite -> ReadOnly. The V18 spelling also covers Flat.
                let (source, ty) = self.value(*value);
                let result = self.value(operation.results[0].id).0;
                let ty = storage_values_v18::ValueTypeV18(ty);
                writeln!(
                    output,
                    "  {result} = select i1 true, {ty} {source}, {ty} {source}"
                )
                .unwrap();
            }
            OperationKind::Alloca {
                element: Type::StorageObject(id),
                count,
                alignment,
                ..
            } => {
                let row = context.row(*id)?;
                let result = self.value(operation.results[0].id).0;
                if let Some(count) = count {
                    let count = self.value(*count).0;
                    writeln!(
                        output,
                        "  {prefix}.bounded = icmp ule i64 {count}, {}",
                        u64::from(u32::MAX) / row.size
                    )
                    .unwrap();
                    writeln!(output, "  {prefix}.nonempty = icmp ne i64 {count}, 0\n  {prefix}.fits = and i1 {prefix}.bounded, {prefix}.nonempty").unwrap();
                    let condition = self.storage_name(format_args!("{prefix}.fits"))?;
                    self.storage_check_branch(output, block, ordinal, condition.as_str());
                    writeln!(output, "  {prefix}.bytes = mul i64 {count}, {}", row.size).unwrap();
                    writeln!(output, "  {result} = alloca i8, i64 {prefix}.bytes, align {alignment}, addrspace(5)").unwrap();
                } else {
                    writeln!(
                        output,
                        "  {result} = alloca [{} x i8], align {alignment}, addrspace(5)",
                        row.size
                    )
                    .unwrap();
                }
            }
            OperationKind::WorkgroupMemory(memory)
                if matches!(memory.element, Type::StorageObject(_)) =>
            {
                let kernel = self
                    .kernel
                    .ok_or_else(|| context.reject("storage LDS is not kernel-owned"))?;
                let result = self.value(operation.results[0].id).0;
                let symbol = lds_symbol(kernel, operation.results[0].id);
                writeln!(
                    output,
                    "  {result} = getelementptr i8, ptr addrspace(3) {symbol}, i64 0"
                )
                .unwrap();
            }
            OperationKind::Storage(Storage::Project { base, step }) => {
                let (pointer, row) = self.storage_address(*base)?;
                let source = self.value(*base).0;
                let result = self.value(operation.results[0].id).0;
                let space = space_v18(pointer.address_space);
                match step {
                    Projection::Field(index) => {
                        let offset = match &row.kind {
                            Kind::Record(fields) | Kind::Union(fields) => {
                                fields[*index as usize].offset
                            }
                            Kind::Slice { data, length, .. } => {
                                if *index == 0 {
                                    data.offset
                                } else {
                                    length.offset
                                }
                            }
                            _ => {
                                return Err(
                                    context.reject("storage field does not have a field row")
                                );
                            }
                        };
                        writeln!(output, "  {result} = getelementptr i8, ptr addrspace({space}) {source}, i64 {offset}").unwrap();
                    }
                    Projection::ArrayIndex(index) => {
                        let Kind::Array { length, stride, .. } = row.kind else {
                            return Err(context.reject("storage array projection has no array row"));
                        };
                        let index = self.value(*index).0;
                        writeln!(output, "  {prefix}.inside = icmp ult i64 {index}, {length}")
                            .unwrap();
                        let condition = self.storage_name(format_args!("{prefix}.inside"))?;
                        self.storage_check_branch(output, block, ordinal, condition.as_str());
                        writeln!(output, "  {prefix}.offset = mul i64 {index}, {stride}").unwrap();
                        writeln!(output, "  {result} = getelementptr i8, ptr addrspace({space}) {source}, i64 {prefix}.offset").unwrap();
                    }
                    Projection::Variant { index, access } => self.emit_storage_variant(
                        output,
                        block,
                        ordinal,
                        *base,
                        *index,
                        *access,
                        result,
                        prefix.as_str(),
                    )?,
                    Projection::VariantForWrite { .. } => writeln!(
                        output,
                        "  {result} = getelementptr i8, ptr addrspace({space}) {source}, i64 0"
                    )
                    .unwrap(),
                }
            }
            OperationKind::Storage(Storage::ReadValue { address, access }) => self
                .emit_storage_read(
                    output,
                    *address,
                    *access,
                    operation.results[0].id,
                    prefix.as_str(),
                )?,
            OperationKind::Storage(Storage::WriteValue {
                address,
                value,
                access,
            }) => self.emit_storage_write(output, *address, *value, *access, prefix.as_str())?,
            OperationKind::Storage(Storage::SetDiscriminant {
                address,
                variant,
                access,
            }) => self.emit_storage_discriminant(
                output,
                *address,
                *variant,
                *access,
                prefix.as_str(),
            )?,
            OperationKind::Storage(Storage::ReadDiscriminant { address, access }) => {
                let result = self.value(operation.results[0].id).0;
                self.emit_storage_read_discriminant(output, block, ordinal, *address, *access,
                    &result, prefix.as_str())?;
            }
            OperationKind::Storage(Storage::CopyObject {
                source,
                destination,
                source_access,
                destination_access,
                overlap,
            }) => self.emit_storage_copy(
                output,
                *source,
                *destination,
                *source_access,
                *destination_access,
                *overlap,
            )?,
            OperationKind::SliceData { slice } => {
                let ValueBinding::Slice {
                    data_name,
                    ty: Type::Slice(ty),
                    ..
                } = &self.bindings[slice]
                else {
                    return Err(context.reject("slice binding is missing"));
                };
                let result = self.value(operation.results[0].id).0;
                writeln!(
                    output,
                    "  {result} = getelementptr i8, ptr addrspace({}) {data_name}, i64 0",
                    space_v18(ty.address_space)
                )
                .unwrap();
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } if storage_preflight_v18::extended_type(self.value_type(*true_value)) => self
                .emit_storage_select(
                    output,
                    operation.results[0].id,
                    *condition,
                    *true_value,
                    *false_value,
                )?,
            OperationKind::Call { callee, arguments }
                if self
                    .module
                    .function(callee)
                    .is_some_and(|f| f.role == FunctionRole::InternalHelper)
                    && FloatOperation::from_intrinsic_call(callee, arguments).is_none()
                    && AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                        .is_none() =>
            {
                self.emit_storage_call(output, operation, callee, arguments, prefix.as_str())?
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn emit_storage_lds_declaration(
        &self,
        output: &mut dyn fmt::Write,
        memory: &fe2o3_kernel_ir::WorkgroupMemory,
        symbol: &str,
    ) -> Result<bool, LoweringErrors> {
        let Type::StorageObject(id) = memory.element else {
            return Ok(false);
        };
        let context = self.storage_context()?;
        let row = context.row(id)?;
        match memory.extent {
            WorkgroupMemoryExtent::Static(count) => {
                let bytes = row
                    .size
                    .checked_mul(u64::from(count))
                    .ok_or_else(|| context.reject("storage LDS extent overflow"))?;
                writeln!(
                    output,
                    "{symbol} = internal addrspace(3) global [{bytes} x i8] undef, align {}",
                    memory.alignment
                )
                .unwrap();
            }
            WorkgroupMemoryExtent::Dynamic | WorkgroupMemoryExtent::DynamicAtLeast(_) => writeln!(
                output,
                "{symbol} = external addrspace(3) global [0 x i8], align {}",
                memory.alignment
            )
            .unwrap(),
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "storage_operations_v18_tests.rs"]
mod tests;
