//! Typed value access and ABI formatting inside the shared LLVM emitter.

use super::storage_operations_v18::space_v18;
use super::*;
use fe2o3_kernel_ir::{
    MemoryAccess, StorageLayoutIdV1, StorageLayoutKindV1 as Kind, VectorLayoutV12,
};

pub(super) struct ValueTypeV18<'a>(pub(super) &'a Type);
impl fmt::Display for ValueTypeV18<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Type::Scalar(s) => f.write_str(llvm_scalar(*s)),
            Type::Pointer(p) => write!(f, "ptr addrspace({})", space_v18(p.address_space)),
            Type::Slice(s) => write!(
                f,
                "{{ ptr addrspace({}), i64 }}",
                space_v18(s.address_space)
            ),
            Type::Vector(v) => write!(f, "<{} x {}>", v.lanes, llvm_scalar(v.element)),
            _ => Err(fmt::Error),
        }
    }
}

pub(super) struct ResultTypeV18<'a> {
    pub(super) function: &'a Function,
    pub(super) storage: bool,
}
impl fmt::Display for ResultTypeV18<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.storage {
            return fmt::Display::fmt(&llvm_result_type(self.function), f);
        }
        match self.function.signature.results.as_slice() {
            [] => f.write_str("void"),
            [ty] => fmt::Display::fmt(&ValueTypeV18(ty), f),
            types => {
                f.write_str("{ ")?;
                for (i, ty) in types.iter().enumerate() {
                    if i != 0 {
                        f.write_str(", ")?;
                    }
                    fmt::Display::fmt(&ValueTypeV18(ty), f)?;
                }
                f.write_str(" }")
            }
        }
    }
}

fn alignment(base: u32, offset: u64) -> u32 {
    if offset == 0 {
        base
    } else {
        base.min(1_u32 << offset.trailing_zeros().min(31))
    }
}

impl FunctionLowerer<'_> {
    fn storage_pointer_value(
        &self,
        output: &mut dyn fmt::Write,
        layout: StorageLayoutIdV1,
        address: &str,
        holder_space: KernelAddressSpace,
        access: MemoryAccess,
        value: &str,
        prefix: &str,
        write: bool,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let row = context.row(layout)?;
        let Kind::Pointer(pointer) = row.kind else {
            return Err(context.reject("pointer component has no pointer row"));
        };
        let checked = context.owner.verified_storage_module_ref_v1();
        context.charge(
            4 + fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1.len()
                + fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1.len()
                + value.len()
                + prefix.len(),
        )?;
        let target = storage_v1::StorageTargetContextV1::new(
            checked.storage().layouts(),
            self.target,
            fe2o3_amd_target::PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
        )
        .map_err(|_| context.reject("pointer target context refused"))?;
        let recipe = target
            .pointer_recipe(layout)
            .map_err(|_| context.reject("pointer representation refused"))?;
        let shape = storage_v1::StoragePointerShapeV1 {
            pointee: pointer.pointee,
            space: pointer.value_space,
            access: pointer.access,
        };
        let encoded = self.storage_name(format_args!("{prefix}.encoded"))?;
        let volatile = if access.volatile { "volatile " } else { "" };
        let holder = space_v18(holder_space);
        let stored = space_v18(pointer.encoded_space);
        if write {
            let operand = recipe
                .emit(
                    &target,
                    shape,
                    storage_v1::StoragePointerDirectionV1::ValueToStored,
                    value,
                    encoded.as_str(),
                    output,
                )
                .map_err(|_| context.reject("pointer encoding formatter refused"))?;
            writeln!(output, "  store {volatile}ptr addrspace({stored}) {}, ptr addrspace({holder}) {address}, align {}", operand.name(), access.alignment).unwrap();
        } else if pointer.value_space == pointer.encoded_space {
            writeln!(output, "  {value} = load {volatile}ptr addrspace({stored}), ptr addrspace({holder}) {address}, align {}", access.alignment).unwrap();
        } else {
            writeln!(output, "  {encoded} = load {volatile}ptr addrspace({stored}), ptr addrspace({holder}) {address}, align {}", access.alignment).unwrap();
            recipe
                .emit(
                    &target,
                    shape,
                    storage_v1::StoragePointerDirectionV1::StoredToValue,
                    encoded.as_str(),
                    value,
                    output,
                )
                .map_err(|_| context.reject("pointer decoding formatter refused"))?;
        }
        Ok(())
    }

    pub(super) fn emit_storage_read(
        &self,
        output: &mut dyn fmt::Write,
        address: ValueId,
        access: MemoryAccess,
        result: ValueId,
        prefix: &str,
    ) -> Result<(), LoweringErrors> {
        self.emit_storage_value(output, address, result, access, prefix, false)
    }
    pub(super) fn emit_storage_write(
        &self,
        output: &mut dyn fmt::Write,
        address: ValueId,
        value: ValueId,
        access: MemoryAccess,
        prefix: &str,
    ) -> Result<(), LoweringErrors> {
        self.emit_storage_value(output, address, value, access, prefix, true)
    }

    fn emit_storage_value(
        &self,
        output: &mut dyn fmt::Write,
        address: ValueId,
        value: ValueId,
        access: MemoryAccess,
        prefix: &str,
        write: bool,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let (pointer, row) = self.storage_address(address)?;
        let Type::StorageObject(layout) = *pointer.pointee else {
            unreachable!()
        };
        let base = self.value(address).0;
        let space = space_v18(pointer.address_space);
        let volatile = if access.volatile { "volatile " } else { "" };
        match &row.kind {
            Kind::Scalar(scalar) => {
                let name = self.value(value).0;
                if *scalar == ScalarType::Bool {
                    if write {
                        writeln!(output, "  {prefix}.byte = zext i1 {name} to i8\n  store {volatile}i8 {prefix}.byte, ptr addrspace({space}) {base}, align {}", access.alignment).unwrap();
                    } else {
                        writeln!(output, "  {prefix}.byte = load {volatile}i8, ptr addrspace({space}) {base}, align {}\n  {name} = trunc i8 {prefix}.byte to i1", access.alignment).unwrap();
                    }
                } else if write {
                    writeln!(
                        output,
                        "  store {volatile}{} {name}, ptr addrspace({space}) {base}, align {}",
                        llvm_scalar(*scalar),
                        access.alignment
                    )
                    .unwrap();
                } else {
                    writeln!(
                        output,
                        "  {name} = load {volatile}{}, ptr addrspace({space}) {base}, align {}",
                        llvm_scalar(*scalar),
                        access.alignment
                    )
                    .unwrap();
                }
            }
            Kind::Pointer(_) => self.storage_pointer_value(
                output,
                layout,
                base,
                pointer.address_space,
                access,
                self.value(value).0,
                prefix,
                write,
            )?,
            Kind::Slice { data, length, .. } => {
                let ValueBinding::Slice {
                    data_name,
                    length_name,
                    ..
                } = &self.bindings[&value]
                else {
                    return Err(context.reject("slice value binding is missing"));
                };
                let data_address = self.storage_name(format_args!("{prefix}.data.address"))?;
                let length_address = self.storage_name(format_args!("{prefix}.length.address"))?;
                writeln!(
                    output,
                    "  {data_address} = getelementptr i8, ptr addrspace({space}) {base}, i64 {}",
                    data.offset
                )
                .unwrap();
                let mut component = access;
                component.alignment = alignment(access.alignment, data.offset);
                self.storage_pointer_value(
                    output,
                    data.layout,
                    data_address.as_str(),
                    pointer.address_space,
                    component,
                    data_name,
                    prefix,
                    write,
                )?;
                writeln!(
                    output,
                    "  {length_address} = getelementptr i8, ptr addrspace({space}) {base}, i64 {}",
                    length.offset
                )
                .unwrap();
                let align = alignment(access.alignment, length.offset);
                if write {
                    writeln!(output, "  store {volatile}i64 {length_name}, ptr addrspace({space}) {length_address}, align {align}").unwrap();
                } else {
                    writeln!(output, "  {length_name} = load {volatile}i64, ptr addrspace({space}) {length_address}, align {align}").unwrap();
                }
            }
            Kind::Vector(vector) => {
                let (name, ty) = self.value(value);
                let bytes = u64::from(
                    vector
                        .element
                        .bit_width()
                        .expect("whole-byte vector scalar")
                        / 8,
                );
                for lane in 0..vector.lanes {
                    context.charge(1)?;
                    let physical = match vector.layout {
                        VectorLayoutV12::Contiguous => lane,
                        VectorLayoutV12::Interleaved { factor } => {
                            (lane % factor) * (vector.lanes / factor) + lane / factor
                        }
                    };
                    let offset = u64::from(physical) * bytes;
                    let align = alignment(access.alignment, offset);
                    writeln!(output, "  {prefix}.lane{lane}.address = getelementptr i8, ptr addrspace({space}) {base}, i64 {offset}").unwrap();
                    if write {
                        writeln!(
                            output,
                            "  {prefix}.lane{lane} = extractelement {} {name}, i32 {lane}",
                            ValueTypeV18(ty)
                        )
                        .unwrap();
                        writeln!(output, "  store {volatile}{} {prefix}.lane{lane}, ptr addrspace({space}) {prefix}.lane{lane}.address, align {align}", llvm_scalar(vector.element)).unwrap();
                    } else {
                        writeln!(output, "  {prefix}.lane{lane} = load {volatile}{}, ptr addrspace({space}) {prefix}.lane{lane}.address, align {align}", llvm_scalar(vector.element)).unwrap();
                        if lane + 1 == vector.lanes {
                            write!(output, "  {name}").unwrap();
                        } else {
                            write!(output, "  {prefix}.vector{lane}").unwrap();
                        }
                        write!(output, " = insertelement {} ", ValueTypeV18(ty)).unwrap();
                        if lane == 0 {
                            write!(output, "poison").unwrap();
                        } else {
                            write!(output, "{prefix}.vector{}", lane - 1).unwrap();
                        }
                        writeln!(
                            output,
                            ", {} {prefix}.lane{lane}, i32 {lane}",
                            llvm_scalar(vector.element)
                        )
                        .unwrap();
                    }
                }
            }
            _ => {
                return Err(context
                    .reject("aggregate objects must be projected or copied, not loaded as SSA"));
            }
        }
        Ok(())
    }

    pub(super) fn storage_llvm_parameters(&self) -> Result<Vec<String>, LoweringErrors> {
        let context = self.storage_context()?;
        self.function
            .signature
            .parameters
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                context.charge(1)?;
                if let (Some(roles), Some(kernel)) = (context.root_roles, self.kernel) {
                    if let RootParameterRoleV29::InlineObject(id) =
                        roles.parameter(context, kernel, self.function, index)?
                    {
                        let row = context.row(id)?;
                        return Ok(format!(
                            "ptr addrspace(4) byref([{} x i8]) align {} %arg{index}",
                            row.size, row.alignment,
                        ));
                    }
                }
                Ok(match ty {
                    Type::Slice(slice) => format!(
                        "ptr addrspace({}) %arg{index}.data, i64 %arg{index}.len",
                        space_v18(slice.address_space)
                    ),
                    _ => format!("{} %arg{index}", ValueTypeV18(ty)),
                })
            })
            .collect()
    }

    pub(super) fn emit_storage_block_parameters(
        &self,
        output: &mut dyn fmt::Write,
        block: &BasicBlock,
    ) {
        let context = self.storage_v18.expect("V18 block parameter dispatch");
        if context.charge(1).is_err() {
            return;
        }
        let incomings = self.incoming_edges(block.id);
        for (index, parameter) in block.parameters.iter().enumerate() {
            if context.charge(1).is_err() {
                return;
            }
            match &self.bindings[&parameter.id] {
                ValueBinding::Value { llvm_name, ty, .. } => {
                    write!(output, "  {llvm_name} = phi {} ", ValueTypeV18(ty)).unwrap();
                    for (n, (predecessor, ordinal, arguments)) in incomings.iter().enumerate() {
                        if context.charge(1).is_err() {
                            return;
                        }
                        if n != 0 {
                            write!(output, ", ").unwrap();
                        }
                        write!(
                            output,
                            "[ {}, %{} ]",
                            self.value(arguments[index]).0,
                            self.phi_predecessor_label(*predecessor, *ordinal, block.id)
                        )
                        .unwrap();
                    }
                    writeln!(output).unwrap();
                }
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ty: Type::Slice(slice),
                } => {
                    for data in [true, false] {
                        if data {
                            write!(
                                output,
                                "  {data_name} = phi ptr addrspace({}) ",
                                space_v18(slice.address_space)
                            )
                            .unwrap();
                        } else {
                            write!(output, "  {length_name} = phi i64 ").unwrap();
                        }
                        for (n, (predecessor, ordinal, arguments)) in incomings.iter().enumerate() {
                            if context.charge(1).is_err() {
                                return;
                            }
                            let ValueBinding::Slice {
                                data_name,
                                length_name,
                                ..
                            } = &self.bindings[&arguments[index]]
                            else {
                                unreachable!("verified slice phi")
                            };
                            if n != 0 {
                                write!(output, ", ").unwrap();
                            }
                            write!(
                                output,
                                "[ {}, %{} ]",
                                if data { data_name } else { length_name },
                                self.phi_predecessor_label(*predecessor, *ordinal, block.id)
                            )
                            .unwrap();
                        }
                        writeln!(output).unwrap();
                    }
                }
                _ => unreachable!("verified storage binding"),
            }
        }
    }

    pub(super) fn emit_storage_select(
        &self,
        output: &mut dyn fmt::Write,
        result: ValueId,
        condition: ValueId,
        yes: ValueId,
        no: ValueId,
    ) -> Result<(), LoweringErrors> {
        let condition = self.value(condition).0;
        match (
            &self.bindings[&result],
            &self.bindings[&yes],
            &self.bindings[&no],
        ) {
            (
                ValueBinding::Slice {
                    data_name: result_data,
                    length_name: result_length,
                    ty: Type::Slice(slice),
                },
                ValueBinding::Slice {
                    data_name: yes_data,
                    length_name: yes_length,
                    ..
                },
                ValueBinding::Slice {
                    data_name: no_data,
                    length_name: no_length,
                    ..
                },
            ) => {
                let space = space_v18(slice.address_space);
                writeln!(output, "  {result_data} = select i1 {condition}, ptr addrspace({space}) {yes_data}, ptr addrspace({space}) {no_data}\n  {result_length} = select i1 {condition}, i64 {yes_length}, i64 {no_length}").unwrap();
            }
            (
                ValueBinding::Value {
                    llvm_name: result,
                    ty,
                    ..
                },
                ValueBinding::Value { llvm_name: yes, .. },
                ValueBinding::Value { llvm_name: no, .. },
            ) => writeln!(
                output,
                "  {result} = select i1 {condition}, {} {yes}, {} {no}",
                ValueTypeV18(ty),
                ValueTypeV18(ty)
            )
            .unwrap(),
            _ => {
                return Err(self
                    .storage_context()?
                    .reject("select binding shape mismatch"));
            }
        }
        Ok(())
    }

    pub(super) fn emit_storage_call(
        &self,
        output: &mut dyn fmt::Write,
        operation: &Operation,
        callee: &FunctionId,
        arguments: &[ValueId],
        prefix: &str,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let function = self
            .module
            .function(callee)
            .ok_or_else(|| context.reject("storage call lost its callee"))?;
        if !self
            .call_symbols
            .is_some_and(|symbols| symbols.contains_key(callee))
        {
            return Err(context.reject("storage call has no current shared symbol"));
        }
        let result_type = ResultTypeV18 {
            function,
            storage: true,
        };
        if operation.results.is_empty() {
            write!(output, "  call {result_type} @{callee}(").unwrap();
        } else {
            write!(output, "  {prefix}.call = call {result_type} @{callee}(").unwrap();
        }
        for (index, argument) in arguments.iter().enumerate() {
            context.charge(1)?;
            if index != 0 {
                write!(output, ", ").unwrap();
            }
            match &self.bindings[argument] {
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ty: Type::Slice(slice),
                } => write!(
                    output,
                    "ptr addrspace({}) {data_name}, i64 {length_name}",
                    space_v18(slice.address_space)
                )
                .unwrap(),
                ValueBinding::Value { llvm_name, ty, .. } => {
                    write!(output, "{} {llvm_name}", ValueTypeV18(ty)).unwrap()
                }
                _ => unreachable!("verified storage call binding"),
            }
        }
        writeln!(output, ")").unwrap();
        for (index, result) in operation.results.iter().enumerate() {
            context.charge(1)?;
            let source = self.storage_name(format_args!("{prefix}.call"))?;
            match &self.bindings[&result.id] {
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ..
                } => {
                    if operation.results.len() == 1 {
                        writeln!(output, "  {data_name} = extractvalue {result_type} {source}, 0\n  {length_name} = extractvalue {result_type} {source}, 1").unwrap();
                    } else {
                        writeln!(output, "  {data_name} = extractvalue {result_type} {source}, {index}, 0\n  {length_name} = extractvalue {result_type} {source}, {index}, 1").unwrap();
                    }
                }
                ValueBinding::Value { llvm_name, ty, .. } => {
                    if operation.results.len() > 1 {
                        writeln!(
                            output,
                            "  {llvm_name} = extractvalue {result_type} {source}, {index}"
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            output,
                            "  {llvm_name} = select i1 true, {} {source}, {} {source}",
                            ValueTypeV18(ty),
                            ValueTypeV18(ty)
                        )
                        .unwrap();
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn emit_storage_return(
        &self,
        output: &mut dyn fmt::Write,
        block: BlockId,
        values: &[ValueId],
    ) {
        let context = self.storage_v18.expect("V18 return dispatch");
        if context.charge(1 + values.len()).is_err() {
            return;
        }
        let ty = ResultTypeV18 {
            function: self.function,
            storage: true,
        };
        if values.is_empty() {
            writeln!(output, "  ret void").unwrap();
            return;
        }
        if values.len() == 1 {
            match &self.bindings[&values[0]] {
                ValueBinding::Value { llvm_name, .. } => {
                    writeln!(output, "  ret {ty} {llvm_name}").unwrap()
                }
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ty: Type::Slice(slice),
                } => {
                    writeln!(output, "  %storage.return.{}.data = insertvalue {ty} poison, ptr addrspace({}) {data_name}, 0\n  %storage.return.{}.slice = insertvalue {ty} %storage.return.{}.data, i64 {length_name}, 1\n  ret {ty} %storage.return.{}.slice", block.0, space_v18(slice.address_space), block.0, block.0, block.0).unwrap();
                }
                _ => unreachable!("verified return binding"),
            }
            return;
        }
        for (index, value) in values.iter().enumerate() {
            let previous = if index == 0 { None } else { Some(index - 1) };
            match &self.bindings[value] {
                ValueBinding::Value {
                    llvm_name,
                    ty: value_ty,
                    ..
                } => {
                    write!(
                        output,
                        "  %storage.return.{}.{index} = insertvalue {ty} ",
                        block.0
                    )
                    .unwrap();
                    if let Some(previous) = previous {
                        write!(output, "%storage.return.{}.{previous}", block.0).unwrap();
                    } else {
                        write!(output, "poison").unwrap();
                    }
                    writeln!(output, ", {} {llvm_name}, {index}", ValueTypeV18(value_ty)).unwrap();
                }
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ty: Type::Slice(slice),
                } => {
                    write!(
                        output,
                        "  %storage.return.{}.{index}.data = insertvalue {ty} ",
                        block.0
                    )
                    .unwrap();
                    if let Some(previous) = previous {
                        write!(output, "%storage.return.{}.{previous}", block.0).unwrap();
                    } else {
                        write!(output, "poison").unwrap();
                    }
                    writeln!(
                        output,
                        ", ptr addrspace({}) {data_name}, {index}, 0",
                        space_v18(slice.address_space)
                    )
                    .unwrap();
                    writeln!(output, "  %storage.return.{}.{index} = insertvalue {ty} %storage.return.{}.{index}.data, i64 {length_name}, {index}, 1", block.0, block.0).unwrap();
                }
                _ => unreachable!("verified return binding"),
            }
        }
        writeln!(
            output,
            "  ret {ty} %storage.return.{}.{}",
            block.0,
            values.len() - 1
        )
        .unwrap();
    }
}

#[cfg(test)]
#[path = "storage_values_v18_tests.rs"]
mod tests;
