use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use fe2o3_kernel_ir::{
    AddressSpace as KernelAddressSpace, Atomic, AtomicKind, Axis, BasicBlock, BinaryOp, BlockId,
    CastKind, ComparePredicate, Constant, DiagnosticCode as VerificationDiagnosticCode, Function,
    FunctionId, FunctionRole, IndexKind, IntrinsicKind, Kernel, KernelId, LaunchDomain,
    LaunchExtent, MemoryOrdering, Module, ModuleId, Operation, OperationKind, ScalarType,
    Signature, SynchronizationScope, TargetCapability, Terminator, Type, ValueId,
    VerificationErrors, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupMemoryExtent,
    WorkgroupSize, verify_module,
};

use crate::{AMDGPU_TRIPLE, AmdgcnIntrinsic, Dim};

const MAX_G1_WORKGROUP_SIZE: u32 = 1024;

/// Stable rejection categories for the first target-neutral AMDGPU lowering slice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LoweringDiagnosticCode {
    InputVerification(VerificationDiagnosticCode),
    MissingKernel,
    AmbiguousKernel,
    ConflictingSymbol,
    UnsafeSymbolName,
    UnsupportedLaunchDomain,
    MissingWorkgroupSize,
    UnsupportedWorkgroupSize,
    UnsupportedCapability,
    KernelEntryDeclaration,
    UnsupportedResults,
    UnsupportedParameter,
    UnsupportedType,
    UnsupportedAddressSpace,
    UnsupportedBlockArguments,
    UnsupportedOperation,
    UnsupportedAtomic,
    UnsupportedBarrier,
    UnsupportedWorkgroupMemory,
    UnsupportedWaveOperation,
    UnsupportedCast,
    UnsupportedConstant,
    UnsupportedTerminator,
}

/// A deterministic source location in the kernel IR.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoweringLocation {
    pub module: ModuleId,
    pub kernel: Option<KernelId>,
    pub function: Option<FunctionId>,
    pub block: Option<BlockId>,
    pub operation: Option<usize>,
}

impl LoweringLocation {
    fn module(module: &Module) -> Self {
        Self {
            module: module.id.clone(),
            kernel: None,
            function: None,
            block: None,
            operation: None,
        }
    }

    fn kernel(module: &Module, kernel: &Kernel) -> Self {
        Self {
            kernel: Some(kernel.id.clone()),
            ..Self::module(module)
        }
    }

    fn function(module: &Module, kernel: &Kernel, function: &Function) -> Self {
        Self {
            function: Some(function.id.clone()),
            ..Self::kernel(module, kernel)
        }
    }

    fn device_function(module: &Module, function: &Function) -> Self {
        Self {
            function: Some(function.id.clone()),
            ..Self::module(module)
        }
    }

    fn block(module: &Module, kernel: &Kernel, function: &Function, block: BlockId) -> Self {
        Self {
            block: Some(block),
            ..Self::function(module, kernel, function)
        }
    }

    fn operation(
        module: &Module,
        kernel: &Kernel,
        function: &Function,
        block: BlockId,
        operation: usize,
    ) -> Self {
        Self {
            operation: Some(operation),
            ..Self::block(module, kernel, function, block)
        }
    }

    fn device_block(module: &Module, function: &Function, block: BlockId) -> Self {
        Self {
            block: Some(block),
            ..Self::device_function(module, function)
        }
    }

    fn device_operation(
        module: &Module,
        function: &Function,
        block: BlockId,
        operation: usize,
    ) -> Self {
        Self {
            operation: Some(operation),
            ..Self::device_block(module, function, block)
        }
    }
}

impl fmt::Display for LoweringLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "module {}", self.module)?;
        if let Some(kernel) = &self.kernel {
            write!(formatter, ", kernel {kernel}")?;
        }
        if let Some(function) = &self.function {
            write!(formatter, ", function {function}")?;
        }
        if let Some(block) = self.block {
            write!(formatter, ", {block}")?;
        }
        if let Some(operation) = self.operation {
            write!(formatter, ", op {operation}")?;
        }
        Ok(())
    }
}

/// One stable, located lowering diagnostic.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoweringDiagnostic {
    pub location: LoweringLocation,
    pub code: LoweringDiagnosticCode,
    pub message: String,
}

impl fmt::Display for LoweringDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {:?}: {}",
            self.location, self.code, self.message
        )
    }
}

/// A deterministic set of errors produced before textual emission succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringErrors {
    diagnostics: Vec<LoweringDiagnostic>,
}

impl LoweringErrors {
    pub fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<LoweringDiagnostic> {
        self.diagnostics
    }

    pub fn contains(&self, code: LoweringDiagnosticCode) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }

    fn one(
        location: LoweringLocation,
        code: LoweringDiagnosticCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            diagnostics: vec![LoweringDiagnostic {
                location,
                code,
                message: message.into(),
            }],
        }
    }

    fn verification(errors: VerificationErrors) -> Self {
        let diagnostics = errors
            .into_diagnostics()
            .into_iter()
            .map(|diagnostic| LoweringDiagnostic {
                location: LoweringLocation {
                    module: diagnostic.location.module,
                    kernel: diagnostic.location.kernel,
                    function: diagnostic.location.function,
                    block: diagnostic.location.block,
                    operation: diagnostic.location.operation,
                },
                code: LoweringDiagnosticCode::InputVerification(diagnostic.code),
                message: diagnostic.message,
            })
            .collect();
        Self { diagnostics }
    }
}

impl fmt::Display for LoweringErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "AMDGPU lowering failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for LoweringErrors {}

/// Lowers one exact kernel to deterministic textual AMDGPU LLVM IR.
///
/// This is a deliberately small, non-production lowering seam. It always calls
/// [`verify_module`] before selecting the kernel, then rejects every valid IR construct outside
/// the documented G1 subset. The launch extent remains a host contract; LLVM IR records the
/// required workgroup size, while launch-grid selection remains outside this API.
pub fn lower_kernel_to_llvm_ir(
    module: &Module,
    kernel_id: &KernelId,
) -> Result<String, LoweringErrors> {
    verify_module(module).map_err(LoweringErrors::verification)?;

    let matches = module
        .kernels
        .iter()
        .filter(|kernel| &kernel.id == kernel_id)
        .collect::<Vec<_>>();
    let kernel = match matches.as_slice() {
        [] => {
            return Err(LoweringErrors::one(
                LoweringLocation::module(module),
                LoweringDiagnosticCode::MissingKernel,
                format!("kernel {kernel_id} is not in the module"),
            ));
        }
        [kernel] => *kernel,
        _ => {
            return Err(LoweringErrors::one(
                LoweringLocation::module(module),
                LoweringDiagnosticCode::AmbiguousKernel,
                format!("kernel identity {kernel_id} is ambiguous"),
            ));
        }
    };

    if !is_safe_symbol(kernel.id.as_str()) {
        return Err(LoweringErrors::one(
            LoweringLocation::kernel(module, kernel),
            LoweringDiagnosticCode::UnsafeSymbolName,
            "kernel identity is not a safe unquoted LLVM symbol",
        ));
    }

    let module_wave = validate_capabilities(
        LoweringLocation::module(module),
        &module.required_capabilities,
        "module",
    )?;
    let kernel_wave = validate_capabilities(
        LoweringLocation::kernel(module, kernel),
        &kernel.required_capabilities,
        "kernel",
    )?;

    let workgroup_size = validate_launch(module, kernel)?;
    let entry = module
        .functions
        .iter()
        .find(|function| function.id == kernel.entry)
        .expect("verify_module established the kernel entry");
    let body = entry.body.as_ref().ok_or_else(|| {
        LoweringErrors::one(
            LoweringLocation::function(module, kernel, entry),
            LoweringDiagnosticCode::KernelEntryDeclaration,
            "kernel entry must be a definition",
        )
    })?;

    let function_wave = validate_capabilities(
        LoweringLocation::function(module, kernel, entry),
        &entry.required_capabilities,
        "entry function",
    )?;
    let wave_widths = [module_wave, kernel_wave, function_wave]
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    if wave_widths.len() > 1 {
        return Err(LoweringErrors::one(
            LoweringLocation::function(module, kernel, entry),
            LoweringDiagnosticCode::UnsupportedCapability,
            format!("conflicting exact wave-width requirements: {wave_widths:?}"),
        ));
    }
    let wave_width = wave_widths.first().copied();
    if !entry.signature.results.is_empty() {
        return Err(LoweringErrors::one(
            LoweringLocation::function(module, kernel, entry),
            LoweringDiagnosticCode::UnsupportedResults,
            "G1 kernel entries must return void",
        ));
    }

    let mut lowerer = FunctionLowerer::new(module, kernel, entry, workgroup_size.x, wave_width);
    lowerer.validate_parameters()?;
    for block in &body.blocks {
        lowerer.validate_block(block)?;
    }
    lowerer.validate_block_arguments()?;
    lowerer.emit()
}

/// Lowers one verified kernel-IR module to one deterministic textual AMDGPU LLVM module.
///
/// This is an inert compiler-module construction primitive. It performs no linking, target
/// selection, optimization, publication, or code-object generation. Kernel entries are emitted
/// in kernel-identity order. Non-kernel definitions and external declarations are emitted once in
/// function-identity order, while each function body preserves its verified block and operation
/// order. All functions are preflighted before any output is returned.
///
/// The current bounded feature slice supports void or single-result scalar/pointer helper ABIs.
/// Slice ABIs remain kernel-entry-only. Calls to kernel entry functions and context-dependent
/// operations in helpers are rejected.
pub fn lower_compiler_module_to_llvm_ir(module: &Module) -> Result<String, LoweringErrors> {
    if module.kernels.is_empty() {
        return Err(LoweringErrors::one(
            LoweringLocation::module(module),
            LoweringDiagnosticCode::MissingKernel,
            "compiler-module lowering requires at least one kernel entry",
        ));
    }
    verify_module(module).map_err(LoweringErrors::verification)?;

    let module_wave = validate_capabilities(
        LoweringLocation::module(module),
        &module.required_capabilities,
        "module",
    )?;
    let mut kernels = module.kernels.iter().collect::<Vec<_>>();
    kernels.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));
    let mut functions = module.functions.iter().collect::<Vec<_>>();
    functions.sort_by(|lhs, rhs| lhs.id.cmp(&rhs.id));

    let mut entries = BTreeMap::<FunctionId, &Kernel>::new();
    let mut emitted_symbols = BTreeMap::<String, String>::new();
    for kernel in &kernels {
        if !is_safe_symbol(kernel.id.as_str()) {
            return Err(LoweringErrors::one(
                LoweringLocation::kernel(module, kernel),
                LoweringDiagnosticCode::UnsafeSymbolName,
                "kernel identity is not a safe unquoted LLVM symbol",
            ));
        }
        if let Some(previous) = entries.insert(kernel.entry.clone(), kernel) {
            return Err(LoweringErrors::one(
                LoweringLocation::kernel(module, kernel),
                LoweringDiagnosticCode::ConflictingSymbol,
                format!(
                    "kernel entry function {} is already emitted as kernel {}; one definition cannot back multiple exported entries",
                    kernel.entry, previous.id
                ),
            ));
        }
        reserve_emitted_symbol(
            &mut emitted_symbols,
            kernel.id.as_str(),
            format!("kernel {}", kernel.id),
            LoweringLocation::kernel(module, kernel),
        )?;
    }
    for kernel in &kernels {
        let entry = module
            .function(&kernel.entry)
            .expect("verify_module established the kernel entry");
        let body = entry.body.as_ref().expect("verified kernel entry body");
        for block in &body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                if !matches!(&operation.kind, OperationKind::WorkgroupMemory(_)) {
                    continue;
                }
                let result = operation
                    .results
                    .first()
                    .expect("verify_module established the LDS result");
                let emitted = lds_symbol(kernel, result.id);
                reserve_emitted_symbol(
                    &mut emitted_symbols,
                    emitted.strip_prefix('@').expect("LDS symbols start with @"),
                    format!("kernel {} LDS value {}", kernel.id, result.id),
                    LoweringLocation::operation(module, kernel, entry, block.id, operation_index),
                )?;
            }
        }
    }

    let mut call_symbols = BTreeMap::<FunctionId, String>::new();
    let mut declarations = Vec::new();
    let mut helper_definitions = Vec::new();
    for function in &functions {
        if entries.contains_key(&function.id) {
            continue;
        }
        let location = LoweringLocation::device_function(module, function);
        if !is_safe_symbol(function.id.as_str()) {
            return Err(LoweringErrors::one(
                location,
                LoweringDiagnosticCode::UnsafeSymbolName,
                "device function identity is not a safe unquoted LLVM symbol",
            ));
        }
        reserve_emitted_symbol(
            &mut emitted_symbols,
            function.id.as_str(),
            format!("device function {}", function.id),
            location.clone(),
        )?;
        validate_device_signature(module, function)?;
        call_symbols.insert(function.id.clone(), function.id.as_str().to_string());
        match function.role {
            FunctionRole::InternalHelper | FunctionRole::DeviceFfiExport => {
                helper_definitions.push(*function);
            }
            FunctionRole::ExternalImport => {
                if !function.required_capabilities.is_empty() {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedCapability,
                        "external declarations cannot carry target capability claims in this textual compiler-module slice",
                    ));
                }
                declarations.push(*function);
            }
            FunctionRole::KernelEntry => {
                unreachable!("verify_module rejects unreferenced KernelEntry definitions")
            }
        }
    }

    let mut kernel_lowerers = Vec::with_capacity(kernels.len());
    for kernel in &kernels {
        let workgroup_size = validate_launch(module, kernel)?;
        let entry = module
            .function(&kernel.entry)
            .expect("verify_module established the kernel entry");
        let kernel_wave = validate_capabilities(
            LoweringLocation::kernel(module, kernel),
            &kernel.required_capabilities,
            "kernel",
        )?;
        let function_wave = validate_capabilities(
            LoweringLocation::function(module, kernel, entry),
            &entry.required_capabilities,
            "entry function",
        )?;
        let wave_width = unique_wave_width(
            LoweringLocation::function(module, kernel, entry),
            [module_wave, kernel_wave, function_wave],
        )?;
        let mut lowerer = FunctionLowerer::compiler_module_kernel(
            module,
            kernel,
            entry,
            workgroup_size.x,
            wave_width,
            &call_symbols,
        );
        preflight_function(&mut lowerer)?;
        kernel_lowerers.push(lowerer);
    }

    let mut helper_lowerers = Vec::with_capacity(helper_definitions.len());
    for function in helper_definitions {
        let function_wave = validate_capabilities(
            LoweringLocation::device_function(module, function),
            &function.required_capabilities,
            "device function",
        )?;
        let wave_width = unique_wave_width(
            LoweringLocation::device_function(module, function),
            [module_wave, function_wave, None],
        )?;
        let mut lowerer = FunctionLowerer::compiler_module_device_function(
            module,
            function,
            wave_width,
            &call_symbols,
        );
        preflight_function(&mut lowerer)?;
        helper_lowerers.push(lowerer);
    }

    emit_compiler_module(&kernel_lowerers, &helper_lowerers, &declarations)
}

fn reserve_emitted_symbol(
    symbols: &mut BTreeMap<String, String>,
    symbol: &str,
    owner: String,
    location: LoweringLocation,
) -> Result<(), LoweringErrors> {
    if let Some(previous) = symbols.insert(symbol.to_string(), owner.clone()) {
        return Err(LoweringErrors::one(
            location,
            LoweringDiagnosticCode::ConflictingSymbol,
            format!("LLVM symbol {symbol:?} is claimed by both {previous} and {owner}"),
        ));
    }
    Ok(())
}

fn validate_device_signature(module: &Module, function: &Function) -> Result<(), LoweringErrors> {
    let location = LoweringLocation::device_function(module, function);
    for (index, ty) in function.signature.parameters.iter().enumerate() {
        validate_device_abi_type(ty, &location).map_err(|error| {
            LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedParameter,
                format!("unsupported device parameter {index}: {error}"),
            )
        })?;
    }
    if function.signature.results.len() > 1 {
        return Err(LoweringErrors::one(
            location,
            LoweringDiagnosticCode::UnsupportedResults,
            "device functions may return at most one scalar or pointer value",
        ));
    }
    if let Some(result) = function.signature.results.first() {
        validate_device_abi_type(result, &location).map_err(|error| {
            LoweringErrors::one(
                location,
                LoweringDiagnosticCode::UnsupportedResults,
                format!("unsupported device result: {error}"),
            )
        })?;
    }
    Ok(())
}

fn validate_device_abi_type(ty: &Type, location: &LoweringLocation) -> Result<(), String> {
    match ty {
        Type::Scalar(scalar) if supported_scalar(*scalar) => Ok(()),
        Type::Pointer(_) => validate_pointer(ty, location).map_err(|error| error.to_string()),
        _ => Err(format!("{ty:?}")),
    }
}

fn unique_wave_width(
    location: LoweringLocation,
    widths: [Option<WaveWidth>; 3],
) -> Result<Option<WaveWidth>, LoweringErrors> {
    let widths = widths.into_iter().flatten().collect::<BTreeSet<_>>();
    if widths.len() > 1 {
        return Err(LoweringErrors::one(
            location,
            LoweringDiagnosticCode::UnsupportedCapability,
            format!("conflicting exact wave-width requirements: {widths:?}"),
        ));
    }
    Ok(widths.first().copied())
}

fn preflight_function(lowerer: &mut FunctionLowerer<'_>) -> Result<(), LoweringErrors> {
    lowerer.validate_parameters()?;
    let body = lowerer.function.body.as_ref().expect("definition required");
    for block in &body.blocks {
        lowerer.validate_block(block)?;
    }
    lowerer.validate_block_arguments()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IntrinsicAttribute {
    ReadNone,
    Convergent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IntrinsicDeclaration {
    result: &'static str,
    arguments: &'static str,
    attribute: IntrinsicAttribute,
}

fn emit_compiler_module(
    kernels: &[FunctionLowerer<'_>],
    helpers: &[FunctionLowerer<'_>],
    declarations: &[&Function],
) -> Result<String, LoweringErrors> {
    let intrinsics = collect_intrinsic_declarations(kernels.iter().chain(helpers));
    let has_readnone = intrinsics
        .values()
        .any(|declaration| declaration.attribute == IntrinsicAttribute::ReadNone);
    let has_convergent = intrinsics
        .values()
        .any(|declaration| declaration.attribute == IntrinsicAttribute::Convergent);
    let readnone_attribute = has_readnone.then_some(kernels.len());
    let convergent_attribute = has_convergent.then_some(kernels.len() + usize::from(has_readnone));

    let mut output = String::new();
    writeln!(output, "target triple = \"{AMDGPU_TRIPLE}\"\n").unwrap();

    let mut has_lds = false;
    for lowerer in kernels {
        has_lds |= lowerer.emit_workgroup_memory_declarations(&mut output);
    }
    if has_lds {
        writeln!(output).unwrap();
    }

    for (symbol, declaration) in &intrinsics {
        let attribute = match declaration.attribute {
            IntrinsicAttribute::ReadNone => readnone_attribute.expect("readnone attribute"),
            IntrinsicAttribute::Convergent => convergent_attribute.expect("convergent attribute"),
        };
        writeln!(
            output,
            "declare {} @{symbol}({}) #{attribute}",
            declaration.result, declaration.arguments
        )
        .unwrap();
    }
    for function in declarations {
        writeln!(
            output,
            "declare {} @{}({})",
            llvm_result_type(&function.signature),
            function.id,
            llvm_parameter_types(&function.signature).join(", ")
        )
        .unwrap();
    }
    if !intrinsics.is_empty() || !declarations.is_empty() {
        writeln!(output).unwrap();
    }

    for (index, lowerer) in kernels.iter().enumerate() {
        lowerer.emit_compiler_module_definition(&mut output, Some(index), Some(index))?;
    }
    for lowerer in helpers {
        lowerer.emit_compiler_module_definition(&mut output, None, None)?;
    }

    for (index, lowerer) in kernels.iter().enumerate() {
        let wave_attribute = lowerer.wave_width.map_or("", wave_target_feature);
        let workgroup_x = lowerer
            .workgroup_x
            .expect("compiler-module kernel requires a workgroup size");
        writeln!(
            output,
            "attributes #{index} = {{ nounwind \"amdgpu-flat-work-group-size\"=\"{workgroup_x},{workgroup_x}\"{wave_attribute} }}"
        )
        .unwrap();
    }
    if let Some(index) = readnone_attribute {
        writeln!(
            output,
            "attributes #{index} = {{ nounwind readnone speculatable willreturn }}"
        )
        .unwrap();
    }
    if let Some(index) = convergent_attribute {
        writeln!(output, "attributes #{index} = {{ convergent nounwind }}").unwrap();
    }
    writeln!(output).unwrap();
    for (index, lowerer) in kernels.iter().enumerate() {
        let workgroup_x = lowerer
            .workgroup_x
            .expect("compiler-module kernel requires a workgroup size");
        writeln!(output, "!{index} = !{{i32 {workgroup_x}, i32 1, i32 1}}").unwrap();
    }
    Ok(output)
}

fn collect_intrinsic_declarations<'a>(
    lowerers: impl Iterator<Item = &'a FunctionLowerer<'a>>,
) -> BTreeMap<&'static str, IntrinsicDeclaration> {
    let mut declarations = BTreeMap::new();
    for lowerer in lowerers {
        let body = lowerer.function.body.as_ref().expect("definition required");
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            match &operation.kind {
                OperationKind::Intrinsic(_) => {
                    insert_intrinsic(
                        &mut declarations,
                        AmdgcnIntrinsic::WorkItemId(Dim::X),
                        "i32",
                        "",
                        IntrinsicAttribute::ReadNone,
                    );
                    insert_intrinsic(
                        &mut declarations,
                        AmdgcnIntrinsic::WorkGroupId(Dim::X),
                        "i32",
                        "",
                        IntrinsicAttribute::ReadNone,
                    );
                }
                OperationKind::WorkgroupBarrier(_) => insert_intrinsic(
                    &mut declarations,
                    AmdgcnIntrinsic::SBarrier,
                    "void",
                    "",
                    IntrinsicAttribute::Convergent,
                ),
                OperationKind::Wave(wave) => {
                    if matches!(
                        wave.kind,
                        WaveOperationKind::LaneId | WaveOperationKind::ShuffleIndex { .. }
                    ) {
                        insert_intrinsic(
                            &mut declarations,
                            AmdgcnIntrinsic::MbcntLo,
                            "i32",
                            "i32, i32",
                            IntrinsicAttribute::ReadNone,
                        );
                        if wave.width == WaveWidth::Wave64 {
                            insert_intrinsic(
                                &mut declarations,
                                AmdgcnIntrinsic::MbcntHi,
                                "i32",
                                "i32, i32",
                                IntrinsicAttribute::ReadNone,
                            );
                        }
                    }
                    if matches!(
                        wave.kind,
                        WaveOperationKind::Ballot { .. }
                            | WaveOperationKind::Any { .. }
                            | WaveOperationKind::All { .. }
                    ) {
                        let (result, intrinsic) = ballot_intrinsic(wave.width);
                        declarations.insert(
                            intrinsic,
                            IntrinsicDeclaration {
                                result,
                                arguments: "i1",
                                attribute: IntrinsicAttribute::Convergent,
                            },
                        );
                    }
                    if matches!(wave.kind, WaveOperationKind::ShuffleIndex { .. }) {
                        insert_intrinsic(
                            &mut declarations,
                            AmdgcnIntrinsic::DsBpermute,
                            "i32",
                            "i32, i32",
                            IntrinsicAttribute::Convergent,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    declarations
}

fn insert_intrinsic(
    declarations: &mut BTreeMap<&'static str, IntrinsicDeclaration>,
    intrinsic: AmdgcnIntrinsic,
    result: &'static str,
    arguments: &'static str,
    attribute: IntrinsicAttribute,
) {
    let previous = declarations.insert(
        intrinsic.llvm_name(),
        IntrinsicDeclaration {
            result,
            arguments,
            attribute,
        },
    );
    debug_assert!(previous.is_none_or(|previous| {
        previous
            == IntrinsicDeclaration {
                result,
                arguments,
                attribute,
            }
    }));
}

fn llvm_parameter_types(signature: &Signature) -> Vec<&'static str> {
    signature.parameters.iter().map(llvm_type).collect()
}

fn llvm_result_type(signature: &Signature) -> &'static str {
    match signature.results.as_slice() {
        [] => "void",
        [result] => llvm_type(result),
        _ => unreachable!("compiler-module preflight rejected multi-value returns"),
    }
}

fn wave_target_feature(width: WaveWidth) -> &'static str {
    match width {
        WaveWidth::Wave32 => " \"target-features\"=\"+wavefrontsize32,-wavefrontsize64\"",
        WaveWidth::Wave64 => " \"target-features\"=\"-wavefrontsize32,+wavefrontsize64\"",
    }
}

fn is_safe_symbol(symbol: &str) -> bool {
    let mut bytes = symbol.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validate_capabilities(
    location: LoweringLocation,
    capabilities: &BTreeSet<TargetCapability>,
    owner: &str,
) -> Result<Option<WaveWidth>, LoweringErrors> {
    let mut wave_width = None;
    for capability in capabilities {
        match capability {
            TargetCapability::WorkgroupMemory
            | TargetCapability::WorkgroupBarrier
            | TargetCapability::DynamicWorkgroupMemory
            | TargetCapability::Subgroups => {}
            TargetCapability::SubgroupSize(32 | 64) => {}
            TargetCapability::WaveWidth(width) => wave_width = Some(*width),
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } if supported_atomic_capability(*width_bits, *address_space, *max_scope) => {}
            _ => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedCapability,
                    format!("G1 does not lower {owner} capability {capability:?}"),
                ));
            }
        }
    }
    Ok(wave_width)
}

fn validate_launch(module: &Module, kernel: &Kernel) -> Result<WorkgroupSize, LoweringErrors> {
    match kernel.domain {
        LaunchDomain::D1 {
            x: LaunchExtent::Static(_) | LaunchExtent::Dynamic,
        } => {}
        LaunchDomain::D2 { .. } | LaunchDomain::D3 { .. } => {
            return Err(LoweringErrors::one(
                LoweringLocation::kernel(module, kernel),
                LoweringDiagnosticCode::UnsupportedLaunchDomain,
                "G1 supports only a 1D launch domain",
            ));
        }
    }

    let Some(size) = kernel.workgroup_size else {
        return Err(LoweringErrors::one(
            LoweringLocation::kernel(module, kernel),
            LoweringDiagnosticCode::MissingWorkgroupSize,
            "G1 requires a statically declared workgroup size",
        ));
    };
    if size.x > MAX_G1_WORKGROUP_SIZE || size.y != 1 || size.z != 1 {
        return Err(LoweringErrors::one(
            LoweringLocation::kernel(module, kernel),
            LoweringDiagnosticCode::UnsupportedWorkgroupSize,
            format!(
                "G1 requires workgroup dimensions (x, 1, 1) with x at most {MAX_G1_WORKGROUP_SIZE}"
            ),
        ));
    }
    Ok(size)
}

#[derive(Clone)]
enum ValueBinding {
    Value {
        llvm_name: String,
        ty: Type,
    },
    Slice {
        data_name: String,
        length_name: String,
        ty: Type,
    },
}

impl ValueBinding {
    fn value(&self) -> Option<(&str, &Type)> {
        match self {
            Self::Value { llvm_name, ty } => Some((llvm_name, ty)),
            Self::Slice { .. } => None,
        }
    }
}

struct FunctionLowerer<'a> {
    module: &'a Module,
    kernel: Option<&'a Kernel>,
    function: &'a Function,
    symbol: &'a str,
    workgroup_x: Option<u32>,
    wave_width: Option<WaveWidth>,
    call_symbols: Option<&'a BTreeMap<FunctionId, String>>,
    bindings: BTreeMap<ValueId, ValueBinding>,
}

impl<'a> FunctionLowerer<'a> {
    fn new(
        module: &'a Module,
        kernel: &'a Kernel,
        function: &'a Function,
        workgroup_x: u32,
        wave_width: Option<WaveWidth>,
    ) -> Self {
        Self {
            module,
            kernel: Some(kernel),
            function,
            symbol: kernel.id.as_str(),
            workgroup_x: Some(workgroup_x),
            wave_width,
            call_symbols: None,
            bindings: BTreeMap::new(),
        }
    }

    fn compiler_module_kernel(
        module: &'a Module,
        kernel: &'a Kernel,
        function: &'a Function,
        workgroup_x: u32,
        wave_width: Option<WaveWidth>,
        call_symbols: &'a BTreeMap<FunctionId, String>,
    ) -> Self {
        Self {
            module,
            kernel: Some(kernel),
            function,
            symbol: kernel.id.as_str(),
            workgroup_x: Some(workgroup_x),
            wave_width,
            call_symbols: Some(call_symbols),
            bindings: BTreeMap::new(),
        }
    }

    fn compiler_module_device_function(
        module: &'a Module,
        function: &'a Function,
        wave_width: Option<WaveWidth>,
        call_symbols: &'a BTreeMap<FunctionId, String>,
    ) -> Self {
        Self {
            module,
            kernel: None,
            function,
            symbol: function.id.as_str(),
            workgroup_x: None,
            wave_width,
            call_symbols: Some(call_symbols),
            bindings: BTreeMap::new(),
        }
    }

    fn function_location(&self) -> LoweringLocation {
        self.kernel.map_or_else(
            || LoweringLocation::device_function(self.module, self.function),
            |kernel| LoweringLocation::function(self.module, kernel, self.function),
        )
    }

    fn block_location(&self, block: BlockId) -> LoweringLocation {
        self.kernel.map_or_else(
            || LoweringLocation::device_block(self.module, self.function, block),
            |kernel| LoweringLocation::block(self.module, kernel, self.function, block),
        )
    }

    fn operation_location(&self, block: BlockId, operation: usize) -> LoweringLocation {
        self.kernel.map_or_else(
            || LoweringLocation::device_operation(self.module, self.function, block, operation),
            |kernel| {
                LoweringLocation::operation(self.module, kernel, self.function, block, operation)
            },
        )
    }

    fn validate_parameters(&mut self) -> Result<(), LoweringErrors> {
        let body = self.function.body.as_ref().expect("definition required");
        for (index, (value, ty)) in body
            .parameters
            .iter()
            .copied()
            .zip(&self.function.signature.parameters)
            .enumerate()
        {
            let location = self.function_location();
            match ty {
                Type::Scalar(scalar) => {
                    if !supported_scalar(*scalar) {
                        return Err(LoweringErrors::one(
                            location,
                            LoweringDiagnosticCode::UnsupportedType,
                            format!("unsupported kernel parameter {index}: {ty:?}"),
                        ));
                    }
                    self.bindings.insert(
                        value,
                        ValueBinding::Value {
                            llvm_name: format!("%arg{index}"),
                            ty: ty.clone(),
                        },
                    );
                }
                Type::Pointer(_) if self.kernel.is_none() => {
                    validate_pointer(ty, &location)?;
                    self.bindings.insert(
                        value,
                        ValueBinding::Value {
                            llvm_name: format!("%arg{index}"),
                            ty: ty.clone(),
                        },
                    );
                }
                Type::Slice(slice)
                    if self.kernel.is_some()
                        && slice.address_space == KernelAddressSpace::Global
                        && supported_memory_type(&slice.element) =>
                {
                    self.bindings.insert(
                        value,
                        ValueBinding::Slice {
                            data_name: format!("%arg{index}.data"),
                            length_name: format!("%arg{index}.len"),
                            ty: ty.clone(),
                        },
                    );
                }
                Type::Slice(slice) => {
                    let code = if slice.address_space != KernelAddressSpace::Global {
                        LoweringDiagnosticCode::UnsupportedAddressSpace
                    } else {
                        LoweringDiagnosticCode::UnsupportedType
                    };
                    return Err(LoweringErrors::one(
                        location,
                        code,
                        format!("unsupported kernel parameter {index}: {ty:?}"),
                    ));
                }
                _ => {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedParameter,
                        format!("unsupported kernel parameter {index}: {ty:?}"),
                    ));
                }
            }
        }
        for block in &body.blocks {
            for parameter in &block.parameters {
                let location = self.block_location(block.id);
                match &parameter.ty {
                    Type::Scalar(scalar) if supported_scalar(*scalar) => {
                        self.bindings.insert(
                            parameter.id,
                            ValueBinding::Value {
                                llvm_name: value_name(parameter.id),
                                ty: parameter.ty.clone(),
                            },
                        );
                    }
                    Type::Pointer(_) => {
                        validate_pointer(&parameter.ty, &location)?;
                        self.bindings.insert(
                            parameter.id,
                            ValueBinding::Value {
                                llvm_name: value_name(parameter.id),
                                ty: parameter.ty.clone(),
                            },
                        );
                    }
                    Type::Slice(slice)
                        if self.kernel.is_some()
                            && slice.address_space == KernelAddressSpace::Global
                            && supported_memory_type(&slice.element) =>
                    {
                        self.bindings.insert(
                            parameter.id,
                            ValueBinding::Slice {
                                data_name: format!("{}.data", value_name(parameter.id)),
                                length_name: format!("{}.len", value_name(parameter.id)),
                                ty: parameter.ty.clone(),
                            },
                        );
                    }
                    Type::Slice(slice) => {
                        let code = if slice.address_space != KernelAddressSpace::Global {
                            LoweringDiagnosticCode::UnsupportedAddressSpace
                        } else {
                            LoweringDiagnosticCode::UnsupportedType
                        };
                        return Err(LoweringErrors::one(
                            location,
                            code,
                            format!(
                                "unsupported block parameter {}: {:?}",
                                parameter.id, parameter.ty
                            ),
                        ));
                    }
                    _ => {
                        return Err(LoweringErrors::one(
                            location,
                            LoweringDiagnosticCode::UnsupportedType,
                            format!(
                                "unsupported block parameter {}: {:?}",
                                parameter.id, parameter.ty
                            ),
                        ));
                    }
                }
            }
        }
        for block in &body.blocks {
            for operation in &block.operations {
                for result in &operation.results {
                    let llvm_name = match &operation.kind {
                        OperationKind::Constant(constant) => {
                            constant_value(constant).unwrap_or_else(|| value_name(result.id))
                        }
                        _ => value_name(result.id),
                    };
                    self.bindings.insert(
                        result.id,
                        ValueBinding::Value {
                            llvm_name,
                            ty: result.ty.clone(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_block(&mut self, block: &BasicBlock) -> Result<(), LoweringErrors> {
        for (index, operation) in block.operations.iter().enumerate() {
            self.validate_operation(block.id, index, operation)?;
        }
        self.validate_terminator(
            block.id,
            block
                .terminator
                .as_ref()
                .expect("verify_module required it"),
        )
    }

    fn validate_block_arguments(&self) -> Result<(), LoweringErrors> {
        let body = self.function.body.as_ref().expect("definition required");
        let entry = body
            .blocks
            .first()
            .expect("verify_module required an entry block");
        for block in body
            .blocks
            .iter()
            .filter(|block| !block.parameters.is_empty())
        {
            let location = self.block_location(block.id);
            if block.id == entry.id {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBlockArguments,
                    "G1 cannot materialize entry-block parameters because the initial entry edge has no SSA arguments",
                ));
            }

            let incomings = self.incoming_edges(block.id);
            if incomings.is_empty() {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBlockArguments,
                    "G1 cannot materialize block parameters without an incoming CFG edge",
                ));
            }

            let mut predecessors = BTreeSet::new();
            for (predecessor, _) in incomings {
                if !predecessors.insert(predecessor) {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedBlockArguments,
                        format!(
                            "G1 cannot materialize block parameters for multiple edges from {predecessor}; LLVM phi nodes require one incoming value per predecessor"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_operation(
        &mut self,
        block: BlockId,
        index: usize,
        operation: &Operation,
    ) -> Result<(), LoweringErrors> {
        let location = self.operation_location(block, index);
        match &operation.kind {
            OperationKind::Constant(constant) => {
                validate_constant(constant).map_err(|message| {
                    LoweringErrors::one(
                        location.clone(),
                        LoweringDiagnosticCode::UnsupportedConstant,
                        message,
                    )
                })?;
            }
            OperationKind::Intrinsic(intrinsic)
                if self.kernel.is_some()
                    && intrinsic.kind
                        == (IntrinsicKind::InvocationIndex {
                            kind: IndexKind::Global,
                            axis: Axis::X,
                        }) => {}
            OperationKind::Binary { op, lhs, .. } => {
                let ty = self.value_type(*lhs);
                if !supported_binary(*op, ty) {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedOperation,
                        format!("G1 does not lower {op:?} for {ty:?}"),
                    ));
                }
            }
            OperationKind::Compare { lhs, .. } => {
                let ty = self.value_type(*lhs);
                if !ty
                    .as_scalar()
                    .is_some_and(|scalar| scalar == ScalarType::Bool || supported_integer(scalar))
                {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedOperation,
                        "G1 lowers only integer and boolean comparisons",
                    ));
                }
            }
            OperationKind::Cast { kind, value, to } => {
                let from = self.value_type(*value);
                validate_cast(*kind, from, to).map_err(|message| {
                    LoweringErrors::one(
                        location.clone(),
                        LoweringDiagnosticCode::UnsupportedCast,
                        message,
                    )
                })?;
            }
            OperationKind::SliceLength { slice } => {
                let binding = self.bindings.get(slice).expect("verified operand");
                let ValueBinding::Slice { .. } = binding else {
                    unreachable!("verify_module checked slice_length")
                };
            }
            OperationKind::SliceData { slice } => {
                let binding = self.bindings.get(slice).expect("verified operand");
                let ValueBinding::Slice { .. } = binding else {
                    unreachable!("verify_module checked slice_data")
                };
            }
            OperationKind::GetElementPointer { base, .. } => {
                validate_pointer(self.value_type(*base), &location)?;
            }
            OperationKind::Load { pointer, access } => {
                validate_memory_access(self.value_type(*pointer), access.address_space, &location)?;
            }
            OperationKind::Store {
                pointer, access, ..
            } => {
                validate_memory_access(self.value_type(*pointer), access.address_space, &location)?;
            }
            OperationKind::Fence(_) => {}
            OperationKind::WorkgroupBarrier(_) if self.kernel.is_some() => {}
            OperationKind::WorkgroupBarrier(_) => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBarrier,
                    "compiler-module helpers cannot contain kernel-context workgroup barriers",
                ));
            }
            OperationKind::WorkgroupMemory(memory) => {
                if self.kernel.is_none() {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedWorkgroupMemory,
                        "compiler-module helpers cannot own kernel-context LDS declarations",
                    ));
                }
                if !supported_memory_type(&memory.element) {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedWorkgroupMemory,
                        format!(
                            "AMDGPU LDS lowering does not support element type {:?}",
                            memory.element
                        ),
                    ));
                }
            }
            OperationKind::Wave(wave) => self.validate_wave(wave, &location)?,
            OperationKind::Atomic(atomic) => self.validate_atomic(atomic, &location)?,
            OperationKind::Barrier(_) => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBarrier,
                    "legacy barriers lack the convergence evidence required by AMDGPU lowering",
                ));
            }
            OperationKind::Alloca {
                address_space: KernelAddressSpace::Workgroup,
                ..
            } => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedWorkgroupMemory,
                    "workgroup Alloca is ambiguous; use explicit WorkgroupMemory",
                ));
            }
            OperationKind::Call { callee, arguments } if self.call_symbols.is_some() => {
                self.validate_call(callee, arguments, operation, &location)?;
            }
            OperationKind::Intrinsic(_)
            | OperationKind::Unary { .. }
            | OperationKind::Select { .. }
            | OperationKind::Call { .. }
            | OperationKind::Alloca { .. } => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedOperation,
                    format!("G1 does not lower {:?}", operation.kind),
                ));
            }
        }
        Ok(())
    }

    fn validate_call(
        &self,
        callee: &FunctionId,
        _arguments: &[ValueId],
        _operation: &Operation,
        location: &LoweringLocation,
    ) -> Result<(), LoweringErrors> {
        let call_symbols = self
            .call_symbols
            .expect("compiler-module call validation requires a symbol table");
        if call_symbols.contains_key(callee) {
            return Ok(());
        }

        Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedOperation,
            format!("compiler-module calls cannot target kernel entry function {callee}"),
        ))
    }

    fn validate_terminator(
        &self,
        block: BlockId,
        terminator: &Terminator,
    ) -> Result<(), LoweringErrors> {
        let location = self.block_location(block);
        match terminator {
            Terminator::Branch { .. } | Terminator::ConditionalBranch { .. } => Ok(()),
            Terminator::Return { values } if values.is_empty() => Ok(()),
            Terminator::Return { .. } if self.call_symbols.is_some() => Ok(()),
            Terminator::Unreachable => Ok(()),
            Terminator::Switch { .. }
            | Terminator::IntegerSwitch { .. }
            | Terminator::Return { .. } => Err(LoweringErrors::one(
                location,
                LoweringDiagnosticCode::UnsupportedTerminator,
                format!("G1 does not lower {terminator:?}"),
            )),
        }
    }

    fn incoming_edges(&self, target: BlockId) -> Vec<(BlockId, &[ValueId])> {
        let body = self.function.body.as_ref().expect("definition required");
        let mut incomings = Vec::new();
        for block in &body.blocks {
            match block.terminator.as_ref().expect("verified terminator") {
                Terminator::Branch {
                    target: edge_target,
                    arguments,
                } if *edge_target == target => incomings.push((block.id, arguments.as_slice())),
                Terminator::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                } => {
                    if *then_target == target {
                        incomings.push((block.id, then_arguments.as_slice()));
                    }
                    if *else_target == target {
                        incomings.push((block.id, else_arguments.as_slice()));
                    }
                }
                Terminator::Branch { .. }
                | Terminator::Switch { .. }
                | Terminator::IntegerSwitch { .. }
                | Terminator::Return { .. }
                | Terminator::Unreachable => {}
            }
        }
        incomings
    }

    fn value_type(&self, value: ValueId) -> &Type {
        match self
            .bindings
            .get(&value)
            .expect("verify_module checked value")
        {
            ValueBinding::Value { ty, .. } | ValueBinding::Slice { ty, .. } => ty,
        }
    }

    fn value(&self, value: ValueId) -> (&str, &Type) {
        self.bindings
            .get(&value)
            .and_then(ValueBinding::value)
            .expect("validated scalar or pointer value")
    }

    fn validate_wave(
        &self,
        wave: &WaveOperation,
        location: &LoweringLocation,
    ) -> Result<(), LoweringErrors> {
        let Some(workgroup_x) = self.workgroup_x else {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedWaveOperation,
                "compiler-module helpers cannot contain kernel-context wave operations",
            ));
        };
        if self.wave_width != Some(wave.width) {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedWaveOperation,
                format!(
                    "wave operation requires an exact {:?} capability on the module, kernel, or entry function",
                    wave.width
                ),
            ));
        }
        if !workgroup_x.is_multiple_of(wave.width.lanes()) {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedWaveOperation,
                format!(
                    "full-wave execution requires workgroup size {} to be a multiple of {}",
                    workgroup_x,
                    wave.width.lanes()
                ),
            ));
        }
        Ok(())
    }

    fn emit(&self) -> Result<String, LoweringErrors> {
        let mut output = String::new();
        writeln!(output, "target triple = \"{AMDGPU_TRIPLE}\"\n").unwrap();
        let has_workgroup_barrier = self.has_workgroup_barrier();
        let has_lane_id = self.has_wave_kind(|kind| {
            matches!(
                kind,
                WaveOperationKind::LaneId | WaveOperationKind::ShuffleIndex { .. }
            )
        });
        let has_ballot = self.has_wave_kind(|kind| {
            matches!(
                kind,
                WaveOperationKind::Ballot { .. }
                    | WaveOperationKind::Any { .. }
                    | WaveOperationKind::All { .. }
            )
        });
        let has_shuffle =
            self.has_wave_kind(|kind| matches!(kind, WaveOperationKind::ShuffleIndex { .. }));
        let has_convergent_operation = has_workgroup_barrier || self.has_wave_kind(|_| true);
        if self.emit_workgroup_memory_declarations(&mut output) {
            writeln!(output).unwrap();
        }
        writeln!(
            output,
            "declare i32 @{}() #1",
            AmdgcnIntrinsic::WorkItemId(Dim::X).llvm_name()
        )
        .unwrap();
        writeln!(
            output,
            "declare i32 @{}() #1",
            AmdgcnIntrinsic::WorkGroupId(Dim::X).llvm_name()
        )
        .unwrap();
        if has_workgroup_barrier {
            writeln!(
                output,
                "declare void @{}() #2",
                AmdgcnIntrinsic::SBarrier.llvm_name()
            )
            .unwrap();
        }
        if has_lane_id {
            writeln!(
                output,
                "declare i32 @{}(i32, i32) #1",
                AmdgcnIntrinsic::MbcntLo.llvm_name()
            )
            .unwrap();
            if self.wave_width == Some(WaveWidth::Wave64) {
                writeln!(
                    output,
                    "declare i32 @{}(i32, i32) #1",
                    AmdgcnIntrinsic::MbcntHi.llvm_name()
                )
                .unwrap();
            }
        }
        if has_ballot {
            let (ty, intrinsic) = match self.wave_width {
                Some(WaveWidth::Wave32) => ("i32", AmdgcnIntrinsic::Ballot32),
                Some(WaveWidth::Wave64) => ("i64", AmdgcnIntrinsic::Ballot64),
                None => unreachable!("wave preflight required an exact width"),
            };
            writeln!(output, "declare {ty} @{}(i1) #2", intrinsic.llvm_name()).unwrap();
        }
        if has_shuffle {
            writeln!(
                output,
                "declare i32 @{}(i32, i32) #2",
                AmdgcnIntrinsic::DsBpermute.llvm_name()
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        write!(output, "define amdgpu_kernel void @{}(", self.symbol).unwrap();
        let parameters = self.llvm_parameters()?;
        write!(output, "{}", parameters.join(", ")).unwrap();
        writeln!(output, ") #0 !reqd_work_group_size !0 {{").unwrap();

        self.emit_body(&mut output)?;
        writeln!(output, "}}\n").unwrap();
        let wave_attribute = self.wave_width.map_or("", |width| match width {
            WaveWidth::Wave32 => " \"target-features\"=\"+wavefrontsize32,-wavefrontsize64\"",
            WaveWidth::Wave64 => " \"target-features\"=\"-wavefrontsize32,+wavefrontsize64\"",
        });
        writeln!(
            output,
            "attributes #0 = {{ nounwind \"amdgpu-flat-work-group-size\"=\"{0},{0}\"{wave_attribute} }}",
            self.workgroup_x
                .expect("single-kernel emission requires a workgroup size")
        )
        .unwrap();
        writeln!(
            output,
            "attributes #1 = {{ nounwind readnone speculatable willreturn }}"
        )
        .unwrap();
        if has_convergent_operation {
            writeln!(output, "attributes #2 = {{ convergent nounwind }}").unwrap();
        }
        writeln!(output).unwrap();
        writeln!(
            output,
            "!0 = !{{i32 {}, i32 1, i32 1}}",
            self.workgroup_x
                .expect("single-kernel emission requires a workgroup size")
        )
        .unwrap();
        Ok(output)
    }

    fn emit_compiler_module_definition(
        &self,
        output: &mut String,
        kernel_attribute: Option<usize>,
        kernel_metadata: Option<usize>,
    ) -> Result<(), LoweringErrors> {
        let parameters = self.llvm_parameters()?.join(", ");
        if self.kernel.is_some() {
            writeln!(
                output,
                "define amdgpu_kernel void @{}({parameters}) #{} !reqd_work_group_size !{} {{",
                self.symbol,
                kernel_attribute.expect("kernel attribute index"),
                kernel_metadata.expect("kernel metadata index"),
            )
            .unwrap();
        } else {
            let result = llvm_result_type(&self.function.signature);
            let wave_attribute = self.wave_width.map_or("", wave_target_feature);
            let linkage = match self.function.role {
                FunctionRole::InternalHelper => "internal ",
                FunctionRole::DeviceFfiExport => "",
                FunctionRole::KernelEntry | FunctionRole::ExternalImport => {
                    unreachable!("helper definition has a definition role")
                }
            };
            writeln!(
                output,
                "define {linkage}{result} @{}({parameters}) nounwind{wave_attribute} {{",
                self.symbol,
            )
            .unwrap();
        }
        self.emit_body(output)?;
        writeln!(output, "}}\n").unwrap();
        Ok(())
    }

    fn emit_body(&self, output: &mut String) -> Result<(), LoweringErrors> {
        let body = self.function.body.as_ref().expect("definition required");
        for block in &body.blocks {
            writeln!(output, "{}:", block_label(block.id)).unwrap();
            self.emit_block_parameters(output, block);
            for operation in &block.operations {
                self.emit_operation(output, operation)?;
            }
            self.emit_terminator(
                output,
                block.terminator.as_ref().expect("verified terminator"),
            );
        }
        Ok(())
    }

    fn has_workgroup_barrier(&self) -> bool {
        self.function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(&operation.kind, OperationKind::WorkgroupBarrier(_)))
    }

    fn has_wave_kind(&self, predicate: impl Fn(&WaveOperationKind) -> bool) -> bool {
        self.function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(&operation.kind, OperationKind::Wave(wave) if predicate(&wave.kind))
            })
    }

    fn emit_workgroup_memory_declarations(&self, output: &mut String) -> bool {
        let mut emitted = false;
        let body = self.function.body.as_ref().expect("definition required");
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let OperationKind::WorkgroupMemory(memory) = &operation.kind else {
                continue;
            };
            emitted = true;
            let result = operation.results.first().expect("verified LDS result");
            let symbol = lds_symbol(
                self.kernel
                    .expect("workgroup memory declarations require a kernel"),
                result.id,
            );
            let element = llvm_type(&memory.element);
            match memory.extent {
                WorkgroupMemoryExtent::Static(elements) => writeln!(
                    output,
                    "{symbol} = internal addrspace(3) global [{elements} x {element}] undef, align {}",
                    memory.alignment
                )
                .unwrap(),
                WorkgroupMemoryExtent::Dynamic => writeln!(
                    output,
                    "{symbol} = external addrspace(3) global [0 x {element}], align {}",
                    memory.alignment
                )
                .unwrap(),
            }
        }
        emitted
    }

    fn emit_block_parameters(&self, output: &mut String, block: &BasicBlock) {
        let incomings = self.incoming_edges(block.id);
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            match self
                .bindings
                .get(&parameter.id)
                .expect("validated block parameter")
            {
                ValueBinding::Value { llvm_name, ty } => {
                    let values = incomings
                        .iter()
                        .map(|(predecessor, arguments)| {
                            let (argument, _) = self.value(arguments[parameter_index]);
                            format!("[ {argument}, %{} ]", block_label(*predecessor))
                        })
                        .collect::<Vec<_>>();
                    writeln!(
                        output,
                        "  {llvm_name} = phi {} {}",
                        llvm_type(ty),
                        values.join(", ")
                    )
                    .unwrap();
                }
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ..
                } => {
                    let data_values = incomings
                        .iter()
                        .map(|(predecessor, arguments)| {
                            let ValueBinding::Slice {
                                data_name: argument,
                                ..
                            } = self
                                .bindings
                                .get(&arguments[parameter_index])
                                .expect("verified branch argument")
                            else {
                                unreachable!("verify_module checked branch argument types")
                            };
                            format!("[ {argument}, %{} ]", block_label(*predecessor))
                        })
                        .collect::<Vec<_>>();
                    let length_values = incomings
                        .iter()
                        .map(|(predecessor, arguments)| {
                            let ValueBinding::Slice {
                                length_name: argument,
                                ..
                            } = self
                                .bindings
                                .get(&arguments[parameter_index])
                                .expect("verified branch argument")
                            else {
                                unreachable!("verify_module checked branch argument types")
                            };
                            format!("[ {argument}, %{} ]", block_label(*predecessor))
                        })
                        .collect::<Vec<_>>();
                    writeln!(
                        output,
                        "  {data_name} = phi ptr addrspace(1) {}",
                        data_values.join(", ")
                    )
                    .unwrap();
                    writeln!(
                        output,
                        "  {length_name} = phi i64 {}",
                        length_values.join(", ")
                    )
                    .unwrap();
                }
            }
        }
    }

    fn llvm_parameters(&self) -> Result<Vec<String>, LoweringErrors> {
        self.function
            .signature
            .parameters
            .iter()
            .enumerate()
            .map(|(index, ty)| match ty {
                Type::Scalar(scalar) => Ok(format!("{} %arg{index}", llvm_scalar(*scalar))),
                Type::Pointer(_) if self.kernel.is_none() => {
                    Ok(format!("{} %arg{index}", llvm_type(ty)))
                }
                Type::Slice(_) => Ok(format!(
                    "ptr addrspace(1) %arg{index}.data, i64 %arg{index}.len"
                )),
                _ => Err(LoweringErrors::one(
                    self.function_location(),
                    LoweringDiagnosticCode::UnsupportedParameter,
                    format!("unsupported kernel parameter {index}: {ty:?}"),
                )),
            })
            .collect()
    }

    fn emit_operation(
        &self,
        output: &mut String,
        operation: &Operation,
    ) -> Result<(), LoweringErrors> {
        let result_name = operation
            .results
            .first()
            .map(|result| value_name(result.id));
        match &operation.kind {
            OperationKind::Constant(_) => {}
            OperationKind::SliceLength { slice } => {
                let ValueBinding::Slice { length_name, .. } =
                    self.bindings.get(slice).expect("validated slice binding")
                else {
                    unreachable!()
                };
                writeln!(
                    output,
                    "  {} = add i64 {}, 0",
                    result_name.expect("validated result"),
                    length_name
                )
                .unwrap();
            }
            OperationKind::SliceData { slice } => {
                let ValueBinding::Slice { data_name, .. } =
                    self.bindings.get(slice).expect("validated slice binding")
                else {
                    unreachable!()
                };
                writeln!(
                    output,
                    "  {} = getelementptr i8, ptr addrspace(1) {}, i64 0",
                    result_name.expect("validated result"),
                    data_name
                )
                .unwrap();
            }
            OperationKind::Intrinsic(_) => {
                let result = result_name.expect("validated result");
                writeln!(
                    output,
                    "  {result}.local.i32 = call i32 @{}()",
                    AmdgcnIntrinsic::WorkItemId(Dim::X).llvm_name()
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.group.i32 = call i32 @{}()",
                    AmdgcnIntrinsic::WorkGroupId(Dim::X).llvm_name()
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.local = zext i32 {result}.local.i32 to i64"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.group = zext i32 {result}.group.i32 to i64"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.base = mul i64 {result}.group, {}",
                    self.workgroup_x
                        .expect("global invocation index requires a kernel workgroup size")
                )
                .unwrap();
                writeln!(output, "  {result} = add i64 {result}.base, {result}.local").unwrap();
            }
            OperationKind::Binary { op, lhs, rhs } => {
                let (lhs_name, lhs_ty) = self.value(*lhs);
                let (rhs_name, _) = self.value(*rhs);
                writeln!(
                    output,
                    "  {} = {} {} {}, {}",
                    result_name.expect("validated result"),
                    binary_opcode(*op, lhs_ty),
                    llvm_type(lhs_ty),
                    lhs_name,
                    rhs_name
                )
                .unwrap();
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => {
                let (lhs_name, lhs_ty) = self.value(*lhs);
                let (rhs_name, _) = self.value(*rhs);
                writeln!(
                    output,
                    "  {} = icmp {} {} {}, {}",
                    result_name.expect("validated result"),
                    compare_predicate(*predicate, lhs_ty),
                    llvm_type(lhs_ty),
                    lhs_name,
                    rhs_name
                )
                .unwrap();
            }
            OperationKind::Cast { kind, value, to } => {
                let (value_name, from) = self.value(*value);
                writeln!(
                    output,
                    "  {} = {} {} {} to {}",
                    result_name.expect("validated result"),
                    cast_opcode(*kind, from, to),
                    llvm_type(from),
                    value_name,
                    llvm_type(to)
                )
                .unwrap();
            }
            OperationKind::GetElementPointer { base, offset } => {
                let (base_name, base_ty) = self.value(*base);
                let (offset_name, offset_ty) = self.value(*offset);
                let Type::Pointer(pointer) = base_ty else {
                    unreachable!()
                };
                let address_space = llvm_address_space(pointer.address_space);
                writeln!(
                    output,
                    "  {} = getelementptr {}, ptr addrspace({}) {}, {} {}",
                    result_name.expect("validated result"),
                    llvm_type(&pointer.pointee),
                    address_space,
                    base_name,
                    llvm_type(offset_ty),
                    offset_name
                )
                .unwrap();
            }
            OperationKind::Load { pointer, access } => {
                let (pointer_name, pointer_ty) = self.value(*pointer);
                let Type::Pointer(pointer_ty) = pointer_ty else {
                    unreachable!()
                };
                let address_space = llvm_address_space(pointer_ty.address_space);
                let volatile = if access.volatile { " volatile" } else { "" };
                writeln!(
                    output,
                    "  {} = load{} {}, ptr addrspace({}) {}, align {}",
                    result_name.expect("validated result"),
                    volatile,
                    llvm_type(&pointer_ty.pointee),
                    address_space,
                    pointer_name,
                    access.alignment
                )
                .unwrap();
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            } => {
                let (pointer_name, pointer_ty) = self.value(*pointer);
                let (value_name, _) = self.value(*value);
                let Type::Pointer(pointer_ty) = pointer_ty else {
                    unreachable!()
                };
                let address_space = llvm_address_space(pointer_ty.address_space);
                let volatile = if access.volatile { " volatile" } else { "" };
                writeln!(
                    output,
                    "  store{} {} {}, ptr addrspace({}) {}, align {}",
                    volatile,
                    llvm_type(&pointer_ty.pointee),
                    value_name,
                    address_space,
                    pointer_name,
                    access.alignment
                )
                .unwrap();
            }
            OperationKind::Call { callee, arguments } => {
                let callee_function = self
                    .module
                    .function(callee)
                    .expect("verify_module checked the callee");
                let symbol = self
                    .call_symbols
                    .expect("compiler-module call emission requires a symbol table")
                    .get(callee)
                    .expect("compiler-module preflight rejected kernel-entry calls");
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        let (name, ty) = self.value(*argument);
                        format!("{} {name}", llvm_type(ty))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                match callee_function.signature.results.as_slice() {
                    [] => writeln!(output, "  call void @{symbol}({arguments})").unwrap(),
                    [result] => writeln!(
                        output,
                        "  {} = call {} @{symbol}({arguments})",
                        result_name.expect("verified call result"),
                        llvm_type(result)
                    )
                    .unwrap(),
                    _ => unreachable!("compiler-module preflight rejected multi-value returns"),
                }
            }
            OperationKind::Atomic(atomic) => {
                self.emit_atomic(output, operation, atomic);
            }
            OperationKind::Fence(fence) => {
                emit_fence(output, fence.memory_scope, fence.semantics.ordering);
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                match barrier.semantics.ordering {
                    MemoryOrdering::Acquire => {}
                    MemoryOrdering::Release | MemoryOrdering::AcquireRelease => {
                        emit_fence(output, barrier.memory_scope, MemoryOrdering::Release);
                    }
                    MemoryOrdering::SequentiallyConsistent => {
                        emit_fence(
                            output,
                            barrier.memory_scope,
                            MemoryOrdering::SequentiallyConsistent,
                        );
                    }
                    MemoryOrdering::Relaxed => {
                        unreachable!("kernel IR verification rejected a relaxed barrier")
                    }
                }
                writeln!(
                    output,
                    "  call void @{}()",
                    AmdgcnIntrinsic::SBarrier.llvm_name()
                )
                .unwrap();
                match barrier.semantics.ordering {
                    MemoryOrdering::Release => {}
                    MemoryOrdering::Acquire | MemoryOrdering::AcquireRelease => {
                        emit_fence(output, barrier.memory_scope, MemoryOrdering::Acquire);
                    }
                    MemoryOrdering::SequentiallyConsistent => {
                        emit_fence(
                            output,
                            barrier.memory_scope,
                            MemoryOrdering::SequentiallyConsistent,
                        );
                    }
                    MemoryOrdering::Relaxed => unreachable!(),
                }
            }
            OperationKind::WorkgroupMemory(memory) => {
                let result = operation.results.first().expect("verified LDS result");
                let result_name = result_name.expect("verified LDS result name");
                let elements = match memory.extent {
                    WorkgroupMemoryExtent::Static(elements) => elements,
                    WorkgroupMemoryExtent::Dynamic => 0,
                };
                let element = llvm_type(&memory.element);
                writeln!(
                    output,
                    "  {result_name} = getelementptr [{elements} x {element}], ptr addrspace(3) {}, i32 0, i32 0",
                    lds_symbol(
                        self.kernel
                            .expect("workgroup memory emission requires a kernel"),
                        result.id,
                    )
                )
                .unwrap();
            }
            OperationKind::Wave(wave) => {
                self.emit_wave(
                    output,
                    result_name.as_deref().expect("verified wave result"),
                    wave,
                );
            }
            _ => unreachable!("preflight rejected unsupported operation"),
        }
        Ok(())
    }

    fn emit_wave(&self, output: &mut String, result: &str, wave: &WaveOperation) {
        match wave.kind {
            WaveOperationKind::LaneId => self.emit_lane_id(output, result, wave.width),
            WaveOperationKind::Ballot { predicate } => {
                let predicate = self.value(predicate).0;
                let (ty, intrinsic) = ballot_intrinsic(wave.width);
                writeln!(
                    output,
                    "  {result} = call {ty} @{intrinsic}(i1 {predicate})"
                )
                .unwrap();
            }
            WaveOperationKind::Any { predicate } | WaveOperationKind::All { predicate } => {
                let predicate = self.value(predicate).0;
                let (ty, intrinsic) = ballot_intrinsic(wave.width);
                writeln!(
                    output,
                    "  {result}.mask = call {ty} @{intrinsic}(i1 {predicate})"
                )
                .unwrap();
                let comparison = if matches!(wave.kind, WaveOperationKind::Any { .. }) {
                    "ne"
                } else {
                    "eq"
                };
                let expected = if comparison == "ne" { "0" } else { "-1" };
                writeln!(
                    output,
                    "  {result} = icmp {comparison} {ty} {result}.mask, {expected}"
                )
                .unwrap();
            }
            WaveOperationKind::ShuffleIndex {
                value,
                source_lane,
                tile_width,
            } => {
                let value = self.value(value).0;
                let source_lane = self.value(source_lane).0;
                let lane = format!("{result}.lane");
                self.emit_lane_id(output, &lane, wave.width);
                writeln!(
                    output,
                    "  {result}.tile.base = and i32 {lane}, -{tile_width}"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.tile.relative = and i32 {source_lane}, {}",
                    tile_width - 1
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.source = or i32 {result}.tile.base, {result}.tile.relative"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.source.byte = shl i32 {result}.source, 2"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result} = call i32 @{}(i32 {result}.source.byte, i32 {value})",
                    AmdgcnIntrinsic::DsBpermute.llvm_name()
                )
                .unwrap();
            }
        }
    }

    fn emit_lane_id(&self, output: &mut String, result: &str, width: WaveWidth) {
        writeln!(
            output,
            "  {result}.lo = call i32 @{}(i32 -1, i32 0)",
            AmdgcnIntrinsic::MbcntLo.llvm_name()
        )
        .unwrap();
        match width {
            WaveWidth::Wave32 => {
                writeln!(output, "  {result} = add i32 {result}.lo, 0").unwrap();
            }
            WaveWidth::Wave64 => {
                writeln!(
                    output,
                    "  {result} = call i32 @{}(i32 -1, i32 {result}.lo)",
                    AmdgcnIntrinsic::MbcntHi.llvm_name()
                )
                .unwrap();
            }
        }
    }

    fn validate_atomic(
        &self,
        atomic: &Atomic,
        location: &LoweringLocation,
    ) -> Result<(), LoweringErrors> {
        let pointer = self.value_type(atomic.pointer);
        validate_memory_access(pointer, atomic.access.address_space, location)?;
        let Type::Pointer(pointer) = pointer else {
            unreachable!("kernel IR verification required an atomic pointer")
        };
        let Some(scalar) = pointer.pointee.as_scalar() else {
            unreachable!("kernel IR verification required a scalar atomic pointee")
        };

        if !matches!(
            scalar,
            ScalarType::I32 | ScalarType::U32 | ScalarType::I64 | ScalarType::U64
        ) {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedAtomic,
                format!(
                    "AMDGPU atomic lowering supports only 32-bit and 64-bit integers, found {scalar:?}"
                ),
            ));
        }
        if atomic.access.volatile {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedAtomic,
                "volatile scoped atomics are outside the supported AMDGPU subset",
            ));
        }
        if !supported_atomic_address_scope(atomic.access.address_space, atomic.scope) {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedAtomic,
                format!(
                    "AMDGPU atomic lowering does not support {:?} memory at {:?} scope",
                    atomic.access.address_space, atomic.scope
                ),
            ));
        }
        Ok(())
    }

    fn emit_atomic(&self, output: &mut String, operation: &Operation, atomic: &Atomic) {
        let (pointer_name, pointer_ty) = self.value(atomic.pointer);
        let Type::Pointer(pointer_ty) = pointer_ty else {
            unreachable!("atomic preflight required a pointer")
        };
        let value_type = llvm_type(&pointer_ty.pointee);
        let address_space = llvm_address_space(pointer_ty.address_space);
        let sync_scope = llvm_atomic_sync_scope(atomic.scope);
        let ordering = llvm_atomic_ordering(atomic.ordering);

        match atomic.kind {
            AtomicKind::Load => {
                let result = operation
                    .results
                    .first()
                    .expect("verified atomic load result");
                writeln!(
                    output,
                    "  {} = load atomic {value_type}, ptr addrspace({address_space}) {pointer_name}{sync_scope} {ordering}, align {}",
                    value_name(result.id),
                    atomic.access.alignment
                )
                .unwrap();
            }
            AtomicKind::Store => {
                let value = self
                    .value(atomic.value.expect("verified atomic store value"))
                    .0;
                writeln!(
                    output,
                    "  store atomic {value_type} {value}, ptr addrspace({address_space}) {pointer_name}{sync_scope} {ordering}, align {}",
                    atomic.access.alignment
                )
                .unwrap();
            }
            AtomicKind::CompareExchange => {
                let [old, succeeded] = operation.results.as_slice() else {
                    unreachable!("verified compare-exchange results")
                };
                let desired = self
                    .value(
                        atomic
                            .value
                            .expect("verified compare-exchange desired value"),
                    )
                    .0;
                let expected = self
                    .value(
                        atomic
                            .compare
                            .expect("verified compare-exchange expected value"),
                    )
                    .0;
                let failure_ordering = llvm_atomic_ordering(
                    atomic
                        .failure_ordering
                        .expect("verified compare-exchange failure ordering"),
                );
                let pair = format!("{}.cmpxchg", value_name(old.id));
                writeln!(
                    output,
                    "  {pair} = cmpxchg ptr addrspace({address_space}) {pointer_name}, {value_type} {expected}, {value_type} {desired}{sync_scope} {ordering} {failure_ordering}, align {}",
                    atomic.access.alignment
                )
                .unwrap();
                writeln!(
                    output,
                    "  {} = extractvalue {{ {value_type}, i1 }} {pair}, 0",
                    value_name(old.id)
                )
                .unwrap();
                writeln!(
                    output,
                    "  {} = extractvalue {{ {value_type}, i1 }} {pair}, 1",
                    value_name(succeeded.id)
                )
                .unwrap();
            }
            kind => {
                let result = operation
                    .results
                    .first()
                    .expect("verified atomic RMW result");
                let value = self
                    .value(atomic.value.expect("verified atomic RMW value"))
                    .0;
                let scalar = pointer_ty
                    .pointee
                    .as_scalar()
                    .expect("atomic preflight required a scalar");
                let opcode = llvm_atomic_rmw_opcode(kind, scalar);
                writeln!(
                    output,
                    "  {} = atomicrmw {opcode} ptr addrspace({address_space}) {pointer_name}, {value_type} {value}{sync_scope} {ordering}, align {}",
                    value_name(result.id),
                    atomic.access.alignment
                )
                .unwrap();
            }
        }
    }

    fn emit_terminator(&self, output: &mut String, terminator: &Terminator) {
        match terminator {
            Terminator::Branch { target, .. } => {
                writeln!(output, "  br label %{}", block_label(*target)).unwrap();
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } => {
                let (condition, _) = self.value(*condition);
                writeln!(
                    output,
                    "  br i1 {condition}, label %{}, label %{}",
                    block_label(*then_target),
                    block_label(*else_target)
                )
                .unwrap();
            }
            Terminator::Return { values } => match values.as_slice() {
                [] => writeln!(output, "  ret void").unwrap(),
                [value] => {
                    let (name, ty) = self.value(*value);
                    writeln!(output, "  ret {} {name}", llvm_type(ty)).unwrap();
                }
                _ => unreachable!("compiler-module preflight rejected multi-value returns"),
            },
            Terminator::Unreachable => writeln!(output, "  unreachable").unwrap(),
            _ => unreachable!("preflight rejected unsupported terminator"),
        }
    }
}

fn supported_scalar(scalar: ScalarType) -> bool {
    scalar == ScalarType::Bool || supported_integer(scalar) || scalar == ScalarType::F32
}

fn supported_integer(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::I8
            | ScalarType::I16
            | ScalarType::I32
            | ScalarType::I64
            | ScalarType::U8
            | ScalarType::U16
            | ScalarType::U32
            | ScalarType::U64
            | ScalarType::Index
    )
}

fn supported_memory_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(scalar) if supported_integer(*scalar) || *scalar == ScalarType::F32)
}

fn supported_atomic_capability(
    width_bits: u16,
    address_space: KernelAddressSpace,
    max_scope: SynchronizationScope,
) -> bool {
    matches!(width_bits, 32 | 64) && supported_atomic_address_scope(address_space, max_scope)
}

fn supported_atomic_address_scope(
    address_space: KernelAddressSpace,
    scope: SynchronizationScope,
) -> bool {
    match address_space {
        KernelAddressSpace::Workgroup => scope == SynchronizationScope::Workgroup,
        KernelAddressSpace::Global => matches!(
            scope,
            SynchronizationScope::Workgroup
                | SynchronizationScope::Device
                | SynchronizationScope::System
        ),
        KernelAddressSpace::Generic
        | KernelAddressSpace::Private
        | KernelAddressSpace::Constant => false,
    }
}

fn supported_binary(op: BinaryOp, ty: &Type) -> bool {
    matches!(op, BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply)
        && ty
            .as_scalar()
            .is_some_and(|scalar| supported_integer(scalar) || scalar == ScalarType::F32)
}

fn validate_pointer(ty: &Type, location: &LoweringLocation) -> Result<(), LoweringErrors> {
    let Type::Pointer(pointer) = ty else {
        unreachable!("verify_module checked GEP base")
    };
    if !matches!(
        pointer.address_space,
        KernelAddressSpace::Global | KernelAddressSpace::Workgroup
    ) {
        return Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedAddressSpace,
            format!(
                "G1 supports only global or workgroup pointers, found {:?}",
                pointer.address_space
            ),
        ));
    }
    if !supported_memory_type(&pointer.pointee) {
        return Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedType,
            format!("unsupported memory pointee type {:?}", pointer.pointee),
        ));
    }
    Ok(())
}

fn validate_memory_access(
    pointer: &Type,
    access_space: KernelAddressSpace,
    location: &LoweringLocation,
) -> Result<(), LoweringErrors> {
    validate_pointer(pointer, location)?;
    let Type::Pointer(pointer) = pointer else {
        unreachable!("validate_pointer required a pointer")
    };
    if access_space != pointer.address_space {
        return Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedAddressSpace,
            format!(
                "memory access names {access_space:?} but pointer uses {:?}",
                pointer.address_space
            ),
        ));
    }
    Ok(())
}

fn validate_constant(constant: &Constant) -> Result<(), String> {
    match constant {
        Constant::Bool(_)
        | Constant::I8(_)
        | Constant::I16(_)
        | Constant::I32(_)
        | Constant::I64(_)
        | Constant::U8(_)
        | Constant::U16(_)
        | Constant::U32(_)
        | Constant::U64(_)
        | Constant::Index(_) => Ok(()),
        Constant::F32Bits(bits) if !f32::from_bits(*bits).is_nan() => Ok(()),
        Constant::F32Bits(_) => Err("G1 rejects NaN f32 constants because LLVM's widened hexadecimal spelling does not preserve every payload".to_string()),
        _ => Err(format!("G1 does not lower constant {constant:?}")),
    }
}

fn validate_cast(kind: CastKind, from: &Type, to: &Type) -> Result<(), String> {
    let (Some(from_scalar), Some(to_scalar)) = (from.as_scalar(), to.as_scalar()) else {
        return Err(format!(
            "G1 casts require scalar types, found {from:?} to {to:?}"
        ));
    };
    if !supported_scalar(from_scalar) || !supported_scalar(to_scalar) {
        return Err(format!("G1 does not lower cast types {from:?} to {to:?}"));
    }
    let from_width = llvm_width(from_scalar);
    let to_width = llvm_width(to_scalar);
    let valid = match kind {
        CastKind::Truncate => {
            supported_integer(from_scalar) && supported_integer(to_scalar) && from_width > to_width
        }
        CastKind::ZeroExtend => {
            (from_scalar == ScalarType::Bool
                || (!from_scalar.is_signed_integer() && supported_integer(from_scalar)))
                && supported_integer(to_scalar)
                && from_width < to_width
        }
        CastKind::SignExtend => {
            from_scalar.is_signed_integer() && supported_integer(to_scalar) && from_width < to_width
        }
        CastKind::IntegerToFloat => supported_integer(from_scalar) && to_scalar == ScalarType::F32,
        CastKind::FloatToInteger => from_scalar == ScalarType::F32 && supported_integer(to_scalar),
        CastKind::Bitcast => {
            from_width == to_width && llvm_scalar(from_scalar) != llvm_scalar(to_scalar)
        }
        CastKind::FloatExtend | CastKind::FloatTruncate => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!("unsupported {kind:?} cast from {from:?} to {to:?}"))
    }
}

fn value_name(value: ValueId) -> String {
    format!("%v{}", value.0)
}

fn lds_symbol(kernel: &Kernel, value: ValueId) -> String {
    format!("@__fe2o3_lds_{}_{}", kernel.id.as_str(), value.0)
}

fn emit_fence(
    output: &mut String,
    scope: fe2o3_kernel_ir::SynchronizationScope,
    ordering: MemoryOrdering,
) {
    let ordering = match ordering {
        MemoryOrdering::Acquire => "acquire",
        MemoryOrdering::Release => "release",
        MemoryOrdering::AcquireRelease => "acq_rel",
        MemoryOrdering::SequentiallyConsistent => "seq_cst",
        MemoryOrdering::Relaxed => unreachable!("verification rejected a relaxed fence"),
    };
    match scope {
        fe2o3_kernel_ir::SynchronizationScope::Subgroup => {
            writeln!(output, "  fence syncscope(\"wavefront\") {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::Workgroup => {
            writeln!(output, "  fence syncscope(\"workgroup\") {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::Device => {
            writeln!(output, "  fence syncscope(\"agent\") {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::System => {
            writeln!(output, "  fence {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::Invocation => {
            unreachable!("verification rejected invocation-scoped synchronization")
        }
    }
}

fn llvm_atomic_sync_scope(scope: SynchronizationScope) -> &'static str {
    match scope {
        SynchronizationScope::Workgroup => " syncscope(\"workgroup\")",
        SynchronizationScope::Device => " syncscope(\"agent\")",
        SynchronizationScope::System => "",
        SynchronizationScope::Invocation | SynchronizationScope::Subgroup => {
            unreachable!("atomic preflight rejected unsupported synchronization scope")
        }
    }
}

fn llvm_atomic_ordering(ordering: MemoryOrdering) -> &'static str {
    match ordering {
        MemoryOrdering::Relaxed => "monotonic",
        MemoryOrdering::Acquire => "acquire",
        MemoryOrdering::Release => "release",
        MemoryOrdering::AcquireRelease => "acq_rel",
        MemoryOrdering::SequentiallyConsistent => "seq_cst",
    }
}

fn llvm_atomic_rmw_opcode(kind: AtomicKind, scalar: ScalarType) -> &'static str {
    match kind {
        AtomicKind::Exchange => "xchg",
        AtomicKind::Add => "add",
        AtomicKind::Subtract => "sub",
        AtomicKind::Min if scalar.is_signed_integer() => "min",
        AtomicKind::Min => "umin",
        AtomicKind::Max if scalar.is_signed_integer() => "max",
        AtomicKind::Max => "umax",
        AtomicKind::BitAnd => "and",
        AtomicKind::BitOr => "or",
        AtomicKind::BitXor => "xor",
        AtomicKind::Load | AtomicKind::Store | AtomicKind::CompareExchange => {
            unreachable!("non-RMW atomic kind")
        }
    }
}

fn ballot_intrinsic(width: WaveWidth) -> (&'static str, &'static str) {
    match width {
        WaveWidth::Wave32 => ("i32", AmdgcnIntrinsic::Ballot32.llvm_name()),
        WaveWidth::Wave64 => ("i64", AmdgcnIntrinsic::Ballot64.llvm_name()),
    }
}

fn block_label(block: BlockId) -> String {
    format!("bb{}", block.0)
}

fn llvm_type(ty: &Type) -> &'static str {
    match ty {
        Type::Scalar(scalar) => llvm_scalar(*scalar),
        Type::Pointer(pointer) if pointer.address_space == KernelAddressSpace::Global => {
            "ptr addrspace(1)"
        }
        Type::Pointer(pointer) if pointer.address_space == KernelAddressSpace::Workgroup => {
            "ptr addrspace(3)"
        }
        Type::Pointer(_) => unreachable!("preflight rejected unsupported address space"),
        Type::Unit | Type::Slice(_) => unreachable!("type is not a first-class G1 LLVM value"),
    }
}

fn llvm_address_space(address_space: KernelAddressSpace) -> u32 {
    match address_space {
        KernelAddressSpace::Global => 1,
        KernelAddressSpace::Workgroup => 3,
        _ => unreachable!("preflight rejected unsupported address space"),
    }
}

fn llvm_scalar(scalar: ScalarType) -> &'static str {
    match scalar {
        ScalarType::Bool => "i1",
        ScalarType::I8 | ScalarType::U8 => "i8",
        ScalarType::I16 | ScalarType::U16 => "i16",
        ScalarType::I32 | ScalarType::U32 => "i32",
        ScalarType::I64 | ScalarType::U64 | ScalarType::Index => "i64",
        ScalarType::F32 => "float",
        ScalarType::F16 | ScalarType::Bf16 | ScalarType::F64 => {
            unreachable!("preflight rejected unsupported scalar")
        }
    }
}

fn llvm_width(scalar: ScalarType) -> u16 {
    scalar.bit_width().unwrap_or(64)
}

fn constant_value(constant: &Constant) -> Option<String> {
    match constant {
        Constant::Bool(value) => Some(value.to_string()),
        Constant::I8(value) => Some(value.to_string()),
        Constant::I16(value) => Some(value.to_string()),
        Constant::I32(value) => Some(value.to_string()),
        Constant::I64(value) => Some(value.to_string()),
        Constant::U8(value) => Some(value.to_string()),
        Constant::U16(value) => Some(value.to_string()),
        Constant::U32(value) => Some(value.to_string()),
        Constant::U64(value) => Some(value.to_string()),
        Constant::Index(value) => Some(value.to_string()),
        Constant::F32Bits(bits) if !f32::from_bits(*bits).is_nan() => Some(format!(
            "0x{:016X}",
            f64::from(f32::from_bits(*bits)).to_bits()
        )),
        _ => None,
    }
}

fn binary_opcode(op: BinaryOp, ty: &Type) -> &'static str {
    let floating = ty.as_scalar() == Some(ScalarType::F32);
    match (op, floating) {
        (BinaryOp::Add, false) => "add",
        (BinaryOp::Subtract, false) => "sub",
        (BinaryOp::Multiply, false) => "mul",
        (BinaryOp::Add, true) => "fadd",
        (BinaryOp::Subtract, true) => "fsub",
        (BinaryOp::Multiply, true) => "fmul",
        _ => unreachable!("preflight rejected unsupported binary operation"),
    }
}

fn compare_predicate(predicate: ComparePredicate, ty: &Type) -> &'static str {
    let scalar = ty.as_scalar().expect("validated scalar comparison");
    let signed = scalar.is_signed_integer();
    match predicate {
        ComparePredicate::Equal => "eq",
        ComparePredicate::NotEqual => "ne",
        ComparePredicate::LessThan if signed => "slt",
        ComparePredicate::LessThan => "ult",
        ComparePredicate::LessThanOrEqual if signed => "sle",
        ComparePredicate::LessThanOrEqual => "ule",
        ComparePredicate::GreaterThan if signed => "sgt",
        ComparePredicate::GreaterThan => "ugt",
        ComparePredicate::GreaterThanOrEqual if signed => "sge",
        ComparePredicate::GreaterThanOrEqual => "uge",
    }
}

fn cast_opcode(kind: CastKind, from: &Type, to: &Type) -> &'static str {
    match kind {
        CastKind::Truncate => "trunc",
        CastKind::ZeroExtend => "zext",
        CastKind::SignExtend => "sext",
        CastKind::IntegerToFloat if from.as_scalar().is_some_and(ScalarType::is_signed_integer) => {
            "sitofp"
        }
        CastKind::IntegerToFloat => "uitofp",
        CastKind::FloatToInteger if to.as_scalar().is_some_and(ScalarType::is_signed_integer) => {
            "fptosi"
        }
        CastKind::FloatToInteger => "fptoui",
        CastKind::Bitcast => "bitcast",
        CastKind::FloatExtend | CastKind::FloatTruncate => {
            unreachable!("preflight rejected unsupported cast")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_symbols_are_intentionally_conservative() {
        for symbol in ["fill", "_fill_2", "Fill42"] {
            assert!(is_safe_symbol(symbol), "{symbol}");
        }
        for symbol in ["", "42fill", "fill.kernel", "fill-kernel", "fill\nret void"] {
            assert!(!is_safe_symbol(symbol), "{symbol}");
        }
    }

    #[test]
    fn constants_and_signedness_are_stable() {
        assert_eq!(constant_value(&Constant::I32(-7)).unwrap(), "-7");
        assert_eq!(
            constant_value(&Constant::F32Bits(1.0f32.to_bits())).unwrap(),
            "0x3FF0000000000000"
        );
        assert_eq!(
            compare_predicate(ComparePredicate::LessThan, &Type::Scalar(ScalarType::I32)),
            "slt"
        );
        assert_eq!(
            compare_predicate(ComparePredicate::LessThan, &Type::INDEX),
            "ult"
        );
    }
}
