use alloc::vec::Vec;
use core::fmt::{self, Write as _};

use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BinaryOperationV2, CallTargetV2, CallingConventionV2, CastOperationV2,
    ComparePredicateV2, ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV2,
    FunctionKindV2, FunctionV2, GFX942_AMDHSA_DATA_LAYOUT_V1, GFX942_AMDHSA_TARGET_TRIPLE_V1,
    Gfx942HandoffV2, GlobalIdV2, GlobalLinkageV2, GlobalV2, HandoffIdentityV2, InstructionKindV2,
    InstructionV2, IntegerBinaryOperationV2, IntrinsicV2, ModuleFlagV1, NamedMetadataV1,
    ParameterAttributeV1, ReturnTypeV2, ScalarConstantV2, ScalarTypeV1, TerminatorV2, ValueIdV2,
    ValueTypeV2,
};

use crate::{MAX_LLVM_ASSEMBLY_BYTES_V2, SerializeErrorV2};

const GFX942_TARGET_FEATURES: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";

pub(crate) fn emit(
    handoff: &Gfx942HandoffV2,
    source_identity: HandoffIdentityV2,
) -> Result<Vec<u8>, SerializeErrorV2> {
    let module = handoff.module();
    let mut output = BoundedWriter::new();
    output.write(format_args!(
        "target triple = \"{GFX942_AMDHSA_TARGET_TRIPLE_V1}\"\n"
    ))?;
    output.write(format_args!(
        "target datalayout = \"{GFX942_AMDHSA_DATA_LAYOUT_V1}\"\n"
    ))?;

    if !module.globals().is_empty() {
        output.push("\n")?;
        for global in module.globals() {
            write_global(&mut output, global)?;
        }
        write_compiler_used(&mut output, module)?;
    }
    if !module.intrinsics().is_empty() {
        output.push("\n")?;
        for reference in module.intrinsics() {
            write_intrinsic_declaration(&mut output, reference.intrinsic())?;
        }
    }
    let workgroup_metadata_base = module.flags().len() + module.named_metadata().len() + 1;
    for (attribute_group, function) in module.functions().iter().enumerate() {
        output.push("\n")?;
        let workgroup_metadata = required_workgroup_size(function)
            .map(|shape| (workgroup_metadata_base + attribute_group, shape));
        write_function(
            &mut output,
            module,
            function,
            attribute_group,
            workgroup_metadata,
        )?;
    }

    output.push("\n")?;
    for (attribute_group, function) in module.functions().iter().enumerate() {
        write_function_attributes(&mut output, function, attribute_group)?;
    }
    write_metadata(&mut output, module, source_identity)?;
    output.finish()
}

struct BoundedWriter {
    bytes: Vec<u8>,
    overflowed: bool,
}

impl BoundedWriter {
    const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            overflowed: false,
        }
    }

    fn push(&mut self, value: &str) -> Result<(), SerializeErrorV2> {
        self.write_str(value).map_err(|_| limit_error())
    }

    fn write(&mut self, arguments: fmt::Arguments<'_>) -> Result<(), SerializeErrorV2> {
        self.write_fmt(arguments).map_err(|_| limit_error())
    }

    fn finish(self) -> Result<Vec<u8>, SerializeErrorV2> {
        if self.overflowed {
            Err(limit_error())
        } else {
            Ok(self.bytes)
        }
    }
}

impl fmt::Write for BoundedWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(new_len) = self.bytes.len().checked_add(value.len()) else {
            self.overflowed = true;
            return Err(fmt::Error);
        };
        if new_len > MAX_LLVM_ASSEMBLY_BYTES_V2 {
            self.overflowed = true;
            return Err(fmt::Error);
        }
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

const fn limit_error() -> SerializeErrorV2 {
    SerializeErrorV2::AssemblyBytesLimitExceeded {
        maximum: MAX_LLVM_ASSEMBLY_BYTES_V2,
    }
}

fn write_global(output: &mut BoundedWriter, global: &GlobalV2) -> Result<(), SerializeErrorV2> {
    output.write(format_args!("@{} = ", global.symbol()))?;
    match global.linkage() {
        GlobalLinkageV2::Internal => output.push("internal ")?,
        GlobalLinkageV2::External => output.push("external ")?,
    }
    write_global_address_space(output, global.address_space())?;
    output.push(if global.is_mutable() {
        "global "
    } else {
        "constant "
    })?;
    if let Some(bytes) = global.byte_initializer() {
        output.write(format_args!("[{} x i8] c\"", bytes.len()))?;
        for byte in bytes {
            output.write(format_args!("\\{byte:02X}"))?;
        }
        output.write(format_args!(
            "\", section \"{}\", align {}",
            global
                .section()
                .ok_or(SerializeErrorV2::InconsistentValidatedModel)?,
            global.alignment()
        ))?;
    } else if let Some(elements) = global.array_elements() {
        output.write(format_args!("[{elements} x "))?;
        write_scalar_type(output, global.value_type())?;
        output.write(format_args!("] undef, align {}", global.alignment()))?;
    } else {
        write_scalar_type(output, global.value_type())?;
        if let Some(initializer) = global.initializer() {
            output.push(" ")?;
            write_scalar_constant(output, initializer)?;
        }
    }
    output.push("\n")
}

fn write_compiler_used(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
) -> Result<(), SerializeErrorV2> {
    let retained = module
        .globals()
        .iter()
        .filter(|global| global.byte_initializer().is_some())
        .collect::<Vec<_>>();
    if retained.is_empty() {
        return Ok(());
    }
    output.write(format_args!(
        "@llvm.compiler.used = appending global [{} x ptr] [",
        retained.len()
    ))?;
    for (index, global) in retained.iter().enumerate() {
        if index != 0 {
            output.push(", ")?;
        }
        output.write(format_args!(
            "ptr addrspacecast (ptr addrspace(4) @{} to ptr)",
            global.symbol()
        ))?;
    }
    output.push("], section \"llvm.metadata\"\n")
}

fn write_global_address_space(
    output: &mut BoundedWriter,
    address_space: AddressSpaceV1,
) -> Result<(), SerializeErrorV2> {
    let number = address_space_number(address_space);
    if number != 0 {
        output.write(format_args!("addrspace({number}) "))?;
    }
    Ok(())
}

fn write_intrinsic_declaration(
    output: &mut BoundedWriter,
    intrinsic: IntrinsicV2,
) -> Result<(), SerializeErrorV2> {
    let (return_type, parameters) = intrinsic.signature();
    output.push("declare ")?;
    write_return_type(output, return_type)?;
    output.write(format_args!(" @{}(", intrinsic_name(intrinsic)))?;
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.push(", ")?;
        }
        write_value_type(output, *parameter)?;
    }
    output.push(")\n")
}

fn write_function(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
    function: &FunctionV2,
    attribute_group: usize,
    workgroup_metadata: Option<(usize, [u16; 3])>,
) -> Result<(), SerializeErrorV2> {
    let value_types = collect_value_types(function);
    let aliases = collect_aliases(function)?;

    output.push("define ")?;
    if function.kind() == FunctionKindV2::Helper {
        output.push("internal ")?;
    }
    write_calling_convention(output, function.calling_convention())?;
    write_return_type(output, function.return_type())?;
    output.write(format_args!(" @{}(", function.symbol()))?;
    for (index, parameter) in function.parameters().iter().enumerate() {
        if index != 0 {
            output.push(", ")?;
        }
        write_value_type(output, parameter.value().value_type())?;
        for attribute in parameter.attributes() {
            output.push(" ")?;
            write_parameter_attribute(output, *attribute)?;
        }
        output.write(format_args!(" %{}", parameter.name()))?;
    }
    output.write(format_args!(") #{attribute_group}"))?;
    if let Some((metadata, _)) = workgroup_metadata {
        output.write(format_args!(" !reqd_work_group_size !{metadata}"))?;
    }
    output.push(" {\n")?;

    let entry = function
        .blocks()
        .iter()
        .find(|block| block.id() == function.entry())
        .ok_or(SerializeErrorV2::InconsistentValidatedModel)?;
    write_block(output, module, function, entry, &value_types, &aliases)?;
    for block in function
        .blocks()
        .iter()
        .filter(|block| block.id() != function.entry())
    {
        write_block(output, module, function, block, &value_types, &aliases)?;
    }
    output.push("}\n")
}

fn write_calling_convention(
    output: &mut BoundedWriter,
    calling_convention: CallingConventionV2,
) -> Result<(), SerializeErrorV2> {
    output.push(match calling_convention {
        CallingConventionV2::C => "ccc ",
        CallingConventionV2::AmdGpuKernel => "amdgpu_kernel ",
    })
}

fn collect_value_types(function: &FunctionV2) -> Vec<(ValueIdV2, ValueTypeV2)> {
    function
        .parameters()
        .iter()
        .map(|parameter| (parameter.value().id(), parameter.value().value_type()))
        .chain(function.blocks().iter().flat_map(|block| {
            block.instructions().iter().filter_map(|instruction| {
                instruction
                    .result()
                    .map(|result| (result.id(), result.value_type()))
            })
        }))
        .collect()
}

#[derive(Clone, Copy)]
enum OperandAlias {
    Constant(ScalarConstantV2),
    VectorZero,
    Global(GlobalIdV2),
}

fn collect_aliases(
    function: &FunctionV2,
) -> Result<Vec<(ValueIdV2, OperandAlias)>, SerializeErrorV2> {
    let mut aliases = Vec::new();
    for instruction in function
        .blocks()
        .iter()
        .flat_map(|block| block.instructions())
    {
        let alias = match instruction.kind() {
            InstructionKindV2::Constant(value) => OperandAlias::Constant(*value),
            InstructionKindV2::VectorZero { .. } => OperandAlias::VectorZero,
            InstructionKindV2::GlobalAddress(global) => OperandAlias::Global(*global),
            _ => continue,
        };
        let result = instruction
            .result()
            .ok_or(SerializeErrorV2::InconsistentValidatedModel)?;
        aliases.push((result.id(), alias));
    }
    Ok(aliases)
}

fn write_block(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
    function: &FunctionV2,
    block: &fe2o3_llvm_handoff::BasicBlockV2,
    value_types: &[(ValueIdV2, ValueTypeV2)],
    aliases: &[(ValueIdV2, OperandAlias)],
) -> Result<(), SerializeErrorV2> {
    output.write(format_args!("bb{}:\n", block.id().get()))?;
    for instruction in block.instructions() {
        match instruction.kind() {
            InstructionKindV2::Constant(_)
            | InstructionKindV2::VectorZero { .. }
            | InstructionKindV2::GlobalAddress(_) => {}
            _ => write_instruction(output, module, function, instruction, value_types, aliases)?,
        }
    }
    write_terminator(
        output,
        module,
        function,
        block.terminator(),
        value_types,
        aliases,
    )
}

fn write_instruction(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
    function: &FunctionV2,
    instruction: &InstructionV2,
    value_types: &[(ValueIdV2, ValueTypeV2)],
    aliases: &[(ValueIdV2, OperandAlias)],
) -> Result<(), SerializeErrorV2> {
    match instruction.kind() {
        InstructionKindV2::Constant(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::GlobalAddress(_) => Err(SerializeErrorV2::InconsistentValidatedModel),
        InstructionKindV2::Binary {
            operation,
            left,
            right,
        } => {
            write_result(output, instruction)?;
            output.write(format_args!("{} ", binary_operation(*operation)))?;
            let value_type = value_type(value_types, *left)?;
            write_value_type(output, value_type)?;
            output.push(" ")?;
            write_operand(output, module, function, *left, aliases)?;
            output.push(", ")?;
            write_operand(output, module, function, *right, aliases)?;
            output.push("\n")
        }
        InstructionKindV2::Compare {
            predicate,
            left,
            right,
        } => {
            write_result(output, instruction)?;
            output.write(format_args!("{} ", compare_predicate(*predicate)))?;
            write_value_type(output, value_type(value_types, *left)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *left, aliases)?;
            output.push(", ")?;
            write_operand(output, module, function, *right, aliases)?;
            output.push("\n")
        }
        InstructionKindV2::Cast {
            operation,
            value,
            to,
        } => {
            write_result(output, instruction)?;
            output.write(format_args!("{} ", cast_operation(*operation)))?;
            write_value_type(output, value_type(value_types, *value)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *value, aliases)?;
            output.push(" to ")?;
            write_value_type(output, *to)?;
            output.push("\n")
        }
        InstructionKindV2::GetElementPtr { base, indices } => {
            write_result(output, instruction)?;
            output.push("getelementptr ")?;
            let base_type = value_type(value_types, *base)?;
            match base_type {
                ValueTypeV2::Pointer { pointee, .. } => write_scalar_type(output, pointee)?,
                ValueTypeV2::ArrayPointer {
                    element, elements, ..
                } => {
                    output.write(format_args!("[{elements} x "))?;
                    write_scalar_type(output, element)?;
                    output.push("]")?;
                }
                _ => return Err(SerializeErrorV2::InconsistentValidatedModel),
            }
            output.push(", ")?;
            write_value_type(output, base_type)?;
            output.push(" ")?;
            write_operand(output, module, function, *base, aliases)?;
            for index in indices {
                output.push(", ")?;
                write_value_type(output, value_type(value_types, *index)?)?;
                output.push(" ")?;
                write_operand(output, module, function, *index, aliases)?;
            }
            output.push("\n")
        }
        InstructionKindV2::Load {
            pointer,
            value_type: loaded,
            alignment,
        } => {
            write_result(output, instruction)?;
            output.push("load ")?;
            write_scalar_type(output, *loaded)?;
            output.push(", ")?;
            write_value_type(output, value_type(value_types, *pointer)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *pointer, aliases)?;
            output.write(format_args!(", align {alignment}\n"))
        }
        InstructionKindV2::VectorLoad4 {
            pointer,
            element_type,
            alignment,
        } => {
            write_result(output, instruction)?;
            output.push("load ")?;
            write_value_type(output, ValueTypeV2::fixed_vector(*element_type))?;
            output.push(", ")?;
            write_value_type(output, value_type(value_types, *pointer)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *pointer, aliases)?;
            output.write(format_args!(", align {alignment}\n"))
        }
        InstructionKindV2::Store {
            pointer,
            value,
            value_type: stored,
            alignment,
        } => {
            output.push("  store ")?;
            write_scalar_type(output, *stored)?;
            output.push(" ")?;
            write_operand(output, module, function, *value, aliases)?;
            output.push(", ")?;
            write_value_type(output, value_type(value_types, *pointer)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *pointer, aliases)?;
            output.write(format_args!(", align {alignment}\n"))
        }
        InstructionKindV2::Call { target, arguments } => {
            let (return_type, parameters) = call_signature(module, *target)?;
            if instruction.result().is_some() {
                write_result(output, instruction)?;
            } else {
                output.push("  ")?;
            }
            output.push("call ")?;
            if let CallTargetV2::Function(callee) = target {
                let callee = find_function(module, *callee)?;
                write_calling_convention(output, callee.calling_convention())?;
            }
            write_return_type(output, return_type)?;
            output.write(format_args!(" @{}(", call_target_name(module, *target)?))?;
            for (index, (argument, parameter_type)) in arguments.iter().zip(parameters).enumerate()
            {
                if index != 0 {
                    output.push(", ")?;
                }
                write_value_type(output, parameter_type)?;
                output.push(" ")?;
                write_operand(output, module, function, *argument, aliases)?;
            }
            output.push(")\n")
        }
        InstructionKindV2::Phi { incoming } => {
            write_result(output, instruction)?;
            output.push("phi ")?;
            let result_type = instruction
                .result()
                .ok_or(SerializeErrorV2::InconsistentValidatedModel)?
                .value_type();
            write_value_type(output, result_type)?;
            output.push(" ")?;
            for (index, (value, block)) in incoming.iter().enumerate() {
                if index != 0 {
                    output.push(", ")?;
                }
                output.push("[ ")?;
                write_operand(output, module, function, *value, aliases)?;
                output.write(format_args!(", %bb{} ]", block.get()))?;
            }
            output.push("\n")
        }
        InstructionKindV2::InsertElement {
            vector,
            element,
            index,
        } => {
            write_result(output, instruction)?;
            output.push("insertelement ")?;
            write_value_type(output, value_type(value_types, *vector)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *vector, aliases)?;
            output.push(", ")?;
            write_value_type(output, value_type(value_types, *element)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *element, aliases)?;
            output.push(", i32 ")?;
            write_operand(output, module, function, *index, aliases)?;
            output.push("\n")
        }
        InstructionKindV2::ExtractElement { vector, index } => {
            write_result(output, instruction)?;
            output.push("extractelement ")?;
            write_value_type(output, value_type(value_types, *vector)?)?;
            output.push(" ")?;
            write_operand(output, module, function, *vector, aliases)?;
            output.push(", i32 ")?;
            write_operand(output, module, function, *index, aliases)?;
            output.push("\n")
        }
    }
}

fn write_result(
    output: &mut BoundedWriter,
    instruction: &InstructionV2,
) -> Result<(), SerializeErrorV2> {
    let result = instruction
        .result()
        .ok_or(SerializeErrorV2::InconsistentValidatedModel)?;
    output.write(format_args!("  %v{} = ", result.id().get()))
}

fn call_signature(
    module: &ExecutableModuleV2,
    target: CallTargetV2,
) -> Result<(ReturnTypeV2, Vec<ValueTypeV2>), SerializeErrorV2> {
    match target {
        CallTargetV2::Function(id) => {
            let function = find_function(module, id)?;
            Ok((
                function.return_type(),
                function
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.value().value_type())
                    .collect(),
            ))
        }
        CallTargetV2::Intrinsic(intrinsic) => Ok(intrinsic.signature()),
    }
}

fn call_target_name(
    module: &ExecutableModuleV2,
    target: CallTargetV2,
) -> Result<&str, SerializeErrorV2> {
    match target {
        CallTargetV2::Function(id) => Ok(find_function(module, id)?.symbol()),
        CallTargetV2::Intrinsic(intrinsic) => Ok(intrinsic_name(intrinsic)),
    }
}

fn find_function(
    module: &ExecutableModuleV2,
    id: fe2o3_llvm_handoff::FunctionIdV2,
) -> Result<&FunctionV2, SerializeErrorV2> {
    module
        .functions()
        .iter()
        .find(|function| function.id() == id)
        .ok_or(SerializeErrorV2::InconsistentValidatedModel)
}

fn write_terminator(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
    function: &FunctionV2,
    terminator: &TerminatorV2,
    value_types: &[(ValueIdV2, ValueTypeV2)],
    aliases: &[(ValueIdV2, OperandAlias)],
) -> Result<(), SerializeErrorV2> {
    match terminator {
        TerminatorV2::Return(None) => output.push("  ret void\n"),
        TerminatorV2::Return(Some(value)) => {
            output.push("  ret ")?;
            let ReturnTypeV2::Value(return_type) = function.return_type() else {
                return Err(SerializeErrorV2::InconsistentValidatedModel);
            };
            write_value_type(output, return_type)?;
            output.push(" ")?;
            write_operand(output, module, function, *value, aliases)?;
            output.push("\n")
        }
        TerminatorV2::Branch(block) => {
            output.write(format_args!("  br label %bb{}\n", block.get()))
        }
        TerminatorV2::ConditionalBranch {
            condition,
            then_block,
            else_block,
        } => {
            if value_type(value_types, *condition)? != ValueTypeV2::Scalar(ScalarTypeV1::I1) {
                return Err(SerializeErrorV2::InconsistentValidatedModel);
            }
            output.push("  br i1 ")?;
            write_operand(output, module, function, *condition, aliases)?;
            output.write(format_args!(
                ", label %bb{}, label %bb{}\n",
                then_block.get(),
                else_block.get()
            ))
        }
        TerminatorV2::Unreachable => output.push("  unreachable\n"),
    }
}

fn write_operand(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
    function: &FunctionV2,
    value: ValueIdV2,
    aliases: &[(ValueIdV2, OperandAlias)],
) -> Result<(), SerializeErrorV2> {
    match aliases
        .iter()
        .find_map(|(id, alias)| (*id == value).then_some(*alias))
    {
        Some(OperandAlias::Constant(constant)) => write_scalar_constant(output, constant),
        Some(OperandAlias::VectorZero) => output.push("zeroinitializer"),
        Some(OperandAlias::Global(global)) => {
            let global = module
                .globals()
                .iter()
                .find(|candidate| candidate.id() == global)
                .ok_or(SerializeErrorV2::InconsistentValidatedModel)?;
            output.write(format_args!("@{}", global.symbol()))
        }
        None => {
            if let Some(parameter) = function
                .parameters()
                .iter()
                .find(|parameter| parameter.value().id() == value)
            {
                output.write(format_args!("%{}", parameter.name()))
            } else {
                output.write(format_args!("%v{}", value.get()))
            }
        }
    }
}

fn value_type(
    value_types: &[(ValueIdV2, ValueTypeV2)],
    value: ValueIdV2,
) -> Result<ValueTypeV2, SerializeErrorV2> {
    value_types
        .iter()
        .find_map(|(id, value_type)| (*id == value).then_some(*value_type))
        .ok_or(SerializeErrorV2::InconsistentValidatedModel)
}

fn write_function_attributes(
    output: &mut BoundedWriter,
    function: &FunctionV2,
    attribute_group: usize,
) -> Result<(), SerializeErrorV2> {
    output.write(format_args!("attributes #{attribute_group} = {{"))?;
    for attribute in function.attributes() {
        if matches!(attribute, FunctionAttributeV2::RequiredWorkgroupSize(_)) {
            continue;
        }
        output.push(" ")?;
        match attribute {
            FunctionAttributeV2::NoUnwind => output.push("nounwind")?,
            FunctionAttributeV2::AlwaysInline => output.push("alwaysinline")?,
            FunctionAttributeV2::NoInline => output.push("noinline")?,
            FunctionAttributeV2::ReadNone => output.push("memory(none)")?,
            FunctionAttributeV2::WillReturn => output.push("willreturn")?,
            FunctionAttributeV2::FlatWorkgroupSize(range) => write_flat_workgroup_size(
                output,
                u32::from(range.minimum()),
                u32::from(range.maximum()),
            )?,
            FunctionAttributeV2::WavesPerEu(range) => output.write(format_args!(
                "\"amdgpu-waves-per-eu\"=\"{},{}\"",
                range.minimum(),
                range.maximum()
            ))?,
            FunctionAttributeV2::DenormalFpMathF32Ieee => {
                output.push("\"denormal-fp-math-f32\"=\"ieee,ieee\"")?;
            }
            FunctionAttributeV2::UnsafeFpMathDisabled => {
                output.push("\"unsafe-fp-math\"=\"false\"")?;
            }
            FunctionAttributeV2::NoInfsFpMathDisabled => {
                output.push("\"no-infs-fp-math\"=\"false\"")?;
            }
            FunctionAttributeV2::NoNansFpMathDisabled => {
                output.push("\"no-nans-fp-math\"=\"false\"")?;
            }
            FunctionAttributeV2::NoSignedZerosFpMathDisabled => {
                output.push("\"no-signed-zeros-fp-math\"=\"false\"")?;
            }
            FunctionAttributeV2::ApproxFuncFpMathDisabled => {
                output.push("\"approx-func-fp-math\"=\"false\"")?;
            }
            FunctionAttributeV2::FpContractOff => output.push("\"fp-contract\"=\"off\"")?,
            FunctionAttributeV2::RequiredWorkgroupSize(_) => unreachable!("filtered above"),
        }
    }
    output.write(format_args!(
        " \"target-cpu\"=\"gfx942\" \"target-features\"=\"{GFX942_TARGET_FEATURES}\" }}\n"
    ))
}

fn write_flat_workgroup_size(
    output: &mut BoundedWriter,
    minimum: u32,
    maximum: u32,
) -> Result<(), SerializeErrorV2> {
    output.write(format_args!(
        "\"amdgpu-flat-work-group-size\"=\"{minimum},{maximum}\""
    ))
}

fn write_parameter_attribute(
    output: &mut BoundedWriter,
    attribute: ParameterAttributeV1,
) -> Result<(), SerializeErrorV2> {
    match attribute {
        ParameterAttributeV1::NoAlias => output.push("noalias"),
        ParameterAttributeV1::NoCapture => output.push("captures(none)"),
        ParameterAttributeV1::NonNull => output.push("nonnull"),
        ParameterAttributeV1::ReadOnly => output.push("readonly"),
        ParameterAttributeV1::WriteOnly => output.push("writeonly"),
        ParameterAttributeV1::Align(value) => output.write(format_args!("align {value}")),
        ParameterAttributeV1::Dereferenceable(value) => {
            output.write(format_args!("dereferenceable({value})"))
        }
    }
}

fn write_metadata(
    output: &mut BoundedWriter,
    module: &ExecutableModuleV2,
    handoff_identity: HandoffIdentityV2,
) -> Result<(), SerializeErrorV2> {
    let flags = module.flags();
    let named = module.named_metadata();
    output.push("\n")?;
    if !flags.is_empty() {
        output.push("!llvm.module.flags = !{")?;
        write_metadata_references(output, 0, flags.len())?;
        output.push("}\n")?;
    }
    for (index, metadata) in named.iter().enumerate() {
        output.write(format_args!(
            "!{} = !{{!{}}}\n",
            named_metadata_name(*metadata),
            flags.len() + index
        ))?;
    }
    let identity_node = flags.len() + named.len();
    output.write(format_args!(
        "!fe2o3.handoff.identity = !{{!{identity_node}}}\n"
    ))?;
    output.push("\n")?;
    for (index, flag) in flags.iter().enumerate() {
        output.write(format_args!("!{index} = "))?;
        match flag {
            ModuleFlagV1::CodeObjectVersion6 => {
                output.push("!{i32 1, !\"amdhsa_code_object_version\", i32 600}\n")?;
            }
            ModuleFlagV1::PicLevel2 => {
                output.push("!{i32 8, !\"PIC Level\", i32 2}\n")?;
            }
            ModuleFlagV1::WcharSize4 => {
                output.push("!{i32 1, !\"wchar_size\", i32 4}\n")?;
            }
        }
    }
    for (index, metadata) in named.iter().enumerate() {
        output.write(format_args!("!{} = ", flags.len() + index))?;
        match metadata {
            NamedMetadataV1::OpenClVersion2_0 | NamedMetadataV1::OpenClSpirVersion2_0 => {
                output.push("!{i32 2, i32 0}\n")?;
            }
            NamedMetadataV1::ProducerIdentity(identity) => {
                output.write(format_args!("!{{!\"sha256:{identity}\"}}\n"))?;
            }
        }
    }
    output.write(format_args!(
        "!{identity_node} = !{{!\"sha256:{handoff_identity}\"}}\n"
    ))?;
    let workgroup_metadata_base = identity_node + 1;
    for (function_index, function) in module.functions().iter().enumerate() {
        if let Some([x, y, z]) = required_workgroup_size(function) {
            output.write(format_args!(
                "!{} = !{{i32 {x}, i32 {y}, i32 {z}}}\n",
                workgroup_metadata_base + function_index
            ))?;
        }
    }
    Ok(())
}

fn required_workgroup_size(function: &FunctionV2) -> Option<[u16; 3]> {
    function
        .attributes()
        .iter()
        .find_map(|attribute| match attribute {
            FunctionAttributeV2::RequiredWorkgroupSize(shape) => Some(*shape),
            _ => None,
        })
}

fn write_metadata_references(
    output: &mut BoundedWriter,
    first: usize,
    count: usize,
) -> Result<(), SerializeErrorV2> {
    for offset in 0..count {
        if offset != 0 {
            output.push(", ")?;
        }
        output.write(format_args!("!{}", first + offset))?;
    }
    Ok(())
}

const fn named_metadata_name(metadata: NamedMetadataV1) -> &'static str {
    match metadata {
        NamedMetadataV1::OpenClVersion2_0 => "opencl.ocl.version",
        NamedMetadataV1::OpenClSpirVersion2_0 => "opencl.spir.version",
        NamedMetadataV1::ProducerIdentity(_) => "llvm.ident",
    }
}

fn write_return_type(
    output: &mut BoundedWriter,
    return_type: ReturnTypeV2,
) -> Result<(), SerializeErrorV2> {
    match return_type {
        ReturnTypeV2::Void => output.push("void"),
        ReturnTypeV2::Value(value_type) => write_value_type(output, value_type),
    }
}

fn write_value_type(
    output: &mut BoundedWriter,
    value_type: ValueTypeV2,
) -> Result<(), SerializeErrorV2> {
    match value_type {
        ValueTypeV2::Scalar(scalar) => write_scalar_type(output, scalar),
        ValueTypeV2::Vector { element, lanes } => {
            output.write(format_args!("<{lanes} x "))?;
            write_scalar_type(output, element)?;
            output.push(">")
        }
        ValueTypeV2::Pointer { address_space, .. }
        | ValueTypeV2::ArrayPointer { address_space, .. } => {
            let number = address_space_number(address_space);
            if number == 0 {
                output.push("ptr")
            } else {
                output.write(format_args!("ptr addrspace({number})"))
            }
        }
    }
}

fn write_scalar_type(
    output: &mut BoundedWriter,
    scalar_type: ScalarTypeV1,
) -> Result<(), SerializeErrorV2> {
    output.push(match scalar_type {
        ScalarTypeV1::I1 => "i1",
        ScalarTypeV1::I8 => "i8",
        ScalarTypeV1::I16 => "i16",
        ScalarTypeV1::I32 => "i32",
        ScalarTypeV1::I64 => "i64",
        ScalarTypeV1::F16 => "half",
        ScalarTypeV1::Bf16 => "bfloat",
        ScalarTypeV1::F32 => "float",
        ScalarTypeV1::F64 => "double",
    })
}

fn write_scalar_constant(
    output: &mut BoundedWriter,
    constant: ScalarConstantV2,
) -> Result<(), SerializeErrorV2> {
    match constant.scalar_type() {
        ScalarTypeV1::I1
        | ScalarTypeV1::I8
        | ScalarTypeV1::I16
        | ScalarTypeV1::I32
        | ScalarTypeV1::I64 => output.write(format_args!("{}", constant.bits())),
        ScalarTypeV1::F16 => {
            output.write(format_args!("bitcast (i16 {} to half)", constant.bits()))
        }
        ScalarTypeV1::Bf16 => {
            output.write(format_args!("bitcast (i16 {} to bfloat)", constant.bits()))
        }
        ScalarTypeV1::F32 => {
            output.write(format_args!("bitcast (i32 {} to float)", constant.bits()))
        }
        ScalarTypeV1::F64 => {
            output.write(format_args!("bitcast (i64 {} to double)", constant.bits()))
        }
    }
}

const fn address_space_number(address_space: AddressSpaceV1) -> u8 {
    match address_space {
        AddressSpaceV1::Flat => 0,
        AddressSpaceV1::Global => 1,
        AddressSpaceV1::Region => 2,
        AddressSpaceV1::Local => 3,
        AddressSpaceV1::Constant => 4,
        AddressSpaceV1::Private => 5,
    }
}

const fn intrinsic_name(intrinsic: IntrinsicV2) -> &'static str {
    match intrinsic {
        IntrinsicV2::AmdGpuWorkitemId(axis) => match axis {
            AxisV2::X => "llvm.amdgcn.workitem.id.x",
            AxisV2::Y => "llvm.amdgcn.workitem.id.y",
            AxisV2::Z => "llvm.amdgcn.workitem.id.z",
        },
        IntrinsicV2::AmdGpuWorkgroupId(axis) => match axis {
            AxisV2::X => "llvm.amdgcn.workgroup.id.x",
            AxisV2::Y => "llvm.amdgcn.workgroup.id.y",
            AxisV2::Z => "llvm.amdgcn.workgroup.id.z",
        },
        IntrinsicV2::AmdGpuBarrier => "llvm.amdgcn.s.barrier",
        IntrinsicV2::FmaF32 => "llvm.fma.f32",
        IntrinsicV2::SqrtF32 => "llvm.sqrt.f32",
        IntrinsicV2::Trap => "llvm.trap",
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k => "llvm.amdgcn.mfma.f32.16x16x16bf16.1k",
    }
}

const fn binary_operation(operation: BinaryOperationV2) -> &'static str {
    match operation {
        BinaryOperationV2::Integer(operation) => match operation {
            IntegerBinaryOperationV2::Add => "add",
            IntegerBinaryOperationV2::Subtract => "sub",
            IntegerBinaryOperationV2::Multiply => "mul",
            IntegerBinaryOperationV2::And => "and",
            IntegerBinaryOperationV2::Or => "or",
            IntegerBinaryOperationV2::Xor => "xor",
            IntegerBinaryOperationV2::ShiftLeft => "shl",
            IntegerBinaryOperationV2::LogicalShiftRight => "lshr",
            IntegerBinaryOperationV2::ArithmeticShiftRight => "ashr",
        },
        BinaryOperationV2::Float(operation) => match operation {
            FloatBinaryOperationV2::Add => "fadd",
            FloatBinaryOperationV2::Subtract => "fsub",
            FloatBinaryOperationV2::Multiply => "fmul",
            FloatBinaryOperationV2::Divide => "fdiv",
        },
    }
}

const fn compare_predicate(predicate: ComparePredicateV2) -> &'static str {
    match predicate {
        ComparePredicateV2::IntegerEqual => "icmp eq",
        ComparePredicateV2::IntegerNotEqual => "icmp ne",
        ComparePredicateV2::UnsignedLessThan => "icmp ult",
        ComparePredicateV2::UnsignedLessOrEqual => "icmp ule",
        ComparePredicateV2::SignedLessThan => "icmp slt",
        ComparePredicateV2::SignedLessOrEqual => "icmp sle",
        ComparePredicateV2::OrderedEqual => "fcmp oeq",
        ComparePredicateV2::OrderedNotEqual => "fcmp one",
        ComparePredicateV2::OrderedLessThan => "fcmp olt",
        ComparePredicateV2::OrderedLessOrEqual => "fcmp ole",
    }
}

const fn cast_operation(operation: CastOperationV2) -> &'static str {
    match operation {
        CastOperationV2::ZeroExtend => "zext",
        CastOperationV2::SignExtend => "sext",
        CastOperationV2::Truncate => "trunc",
        CastOperationV2::FloatExtend => "fpext",
        CastOperationV2::FloatTruncate => "fptrunc",
        CastOperationV2::UnsignedIntToFloat => "uitofp",
        CastOperationV2::SignedIntToFloat => "sitofp",
        CastOperationV2::FloatToUnsignedInt => "fptoui",
        CastOperationV2::FloatToSignedInt => "fptosi",
        CastOperationV2::PointerToInt => "ptrtoint",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_workgroup_range_one_to_sixty_four_has_no_serializer_guard() {
        let mut output = BoundedWriter::new();
        write_flat_workgroup_size(&mut output, 1, 64).unwrap();
        assert_eq!(
            output.finish().unwrap(),
            b"\"amdgpu-flat-work-group-size\"=\"1,64\""
        );
    }
}
