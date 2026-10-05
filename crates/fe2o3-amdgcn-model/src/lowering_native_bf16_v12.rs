// Private compiler continuation for the actual optimized V12 executable.
// This is not ordinary admission, memory/alias discharge, or launch authority.
type Bf16NativeBudgetV1<'a> = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'a>;
type Bf16NativeResourceV1 = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1;

#[derive(Debug)]
pub enum PrivateBf16NativeLlvmErrorV1 {
    Resource(Bf16NativeResourceV1),
    Context(&'static str),
    Lowering(LoweringErrors),
    Panicked,
}
impl fmt::Display for PrivateBf16NativeLlvmErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(f, "private BF16 LLVM resource refusal: {error}"),
            Self::Context(reason) => write!(f, "private BF16 LLVM context refusal: {reason}"),
            Self::Lowering(error) => write!(f, "{error}"),
            Self::Panicked => f.write_str("private BF16 LLVM construction panicked"),
        }
    }
}
impl std::error::Error for PrivateBf16NativeLlvmErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Lowering(error) => Some(error),
            _ => None,
        }
    }
}
impl From<Bf16NativeResourceV1> for PrivateBf16NativeLlvmErrorV1 {
    fn from(error: Bf16NativeResourceV1) -> Self {
        Self::Resource(error)
    }
}
type Bf16NativeResultV1<T> = Result<T, PrivateBf16NativeLlvmErrorV1>;
fn bf16_native_refuse_v1(reason: &'static str) -> PrivateBf16NativeLlvmErrorV1 {
    PrivateBf16NativeLlvmErrorV1::Context(reason)
}

// No public constructor, clone, receipt, or detached function/kernel identity.
struct NativeBf16HelperContextV1<'a> {
    owner: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    root: &'a Function,
    call_block: BlockId,
    call_ordinal: usize,
    helper: &'a Function,
    matrix: &'a MatrixOperation,
    workgroup: WorkgroupSize,
}
impl NativeBf16HelperContextV1<'_> {
    fn admits(&self, module: &Module, function: &Function, matrix: &MatrixOperation) -> bool {
        std::ptr::eq(self.owner.module(), module)
            && std::ptr::eq(self.helper, function)
            && std::ptr::eq(self.matrix, matrix)
    }

    // The immutable helper is acyclic, literal-branch-only, and exactly one
    // MFMA with its checked Return. Its sole root Call is workgroup-uniform.
    // This is not a result summary: unchanged analysis keeps its values varying.
    fn admits_root_call_diagnostic(
        &self,
        module: &Module,
        function: &Function,
        diagnostic: &fe2o3_kernel_analysis::Diagnostic,
    ) -> bool {
        std::ptr::eq(self.owner.module(), module)
            && std::ptr::eq(self.root, function)
            && matches!(
                diagnostic,
                fe2o3_kernel_analysis::Diagnostic::Unsupported {
                    block: Some(block),
                    operation_index: Some(ordinal),
                    reason: fe2o3_kernel_analysis::UnsupportedReason::CallWithoutSummary { callee },
                } if *block == self.call_block
                    && *ordinal == self.call_ordinal
                    && *callee == self.helper.id
            )
    }
}

fn bf16_native_edges_v1(
    term: &Terminator,
    budget: &mut Bf16NativeBudgetV1<'_>,
    mut visit: impl FnMut(BlockId, &[ValueId], &mut Bf16NativeBudgetV1<'_>) -> Bf16NativeResultV1<()>,
) -> Bf16NativeResultV1<()> {
    budget.charge_work(1)?;
    match term {
        Terminator::Branch { target, arguments } => visit(*target, arguments, budget)?,
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            visit(*then_target, then_arguments, budget)?;
            visit(*else_target, else_arguments, budget)?;
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            if cases.len() > 8 {
                return Err(bf16_native_refuse_v1("switch fanout"));
            }
            budget.charge_work(cases.len())?;
            for case in cases {
                visit(case.target, &case.arguments, budget)?;
            }
            visit(*default_target, default_arguments, budget)?;
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            if cases.len() > 8 {
                return Err(bf16_native_refuse_v1("integer switch fanout"));
            }
            budget.charge_work(cases.len())?;
            for case in cases {
                visit(case.target, &case.arguments, budget)?;
            }
            visit(*default_target, default_arguments, budget)?;
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}
fn bf16_native_blocks_v1(function: &Function) -> Bf16NativeResultV1<&[BasicBlock]> {
    let blocks = &function
        .body
        .as_ref()
        .ok_or_else(|| bf16_native_refuse_v1("missing body"))?
        .blocks;
    if blocks.is_empty() || blocks.len() > 32 {
        return Err(bf16_native_refuse_v1("closed 32-block profile"));
    }
    Ok(blocks)
}
fn bf16_native_acyclic_v1(
    function: &Function,
    budget: &mut Bf16NativeBudgetV1<'_>,
) -> Bf16NativeResultV1<()> {
    let blocks = bf16_native_blocks_v1(function)?;
    let mut incoming = [0usize; 32];
    let mut removed = [false; 32];
    budget.charge_work(blocks.len())?;
    for block in blocks {
        bf16_native_edges_v1(
            block
                .terminator
                .as_ref()
                .ok_or_else(|| bf16_native_refuse_v1("missing terminator"))?,
            budget,
            |target, _, budget| {
                budget.charge_work(blocks.len())?;
                let next = blocks
                    .iter()
                    .position(|b| b.id == target)
                    .ok_or_else(|| bf16_native_refuse_v1("foreign successor"))?;
                incoming[next] = incoming[next]
                    .checked_add(1)
                    .ok_or(Bf16NativeResourceV1::Arithmetic)?;
                Ok(())
            },
        )?;
    }
    budget.charge_work(blocks.len())?;
    for _ in 0..blocks.len() {
        budget.charge_work(blocks.len())?;
        let index = (0..blocks.len())
            .find(|i| !removed[*i] && incoming[*i] == 0)
            .ok_or_else(|| bf16_native_refuse_v1("cyclic control"))?;
        removed[index] = true;
        bf16_native_edges_v1(
            blocks[index].terminator.as_ref().unwrap(),
            budget,
            |target, _, budget| {
                budget.charge_work(blocks.len())?;
                let next = blocks
                    .iter()
                    .position(|b| b.id == target)
                    .ok_or_else(|| bf16_native_refuse_v1("foreign successor"))?;
                incoming[next] = incoming[next]
                    .checked_sub(1)
                    .ok_or(Bf16NativeResourceV1::Accounting)?;
                Ok(())
            },
        )?;
    }
    Ok(())
}
fn bf16_native_origin_v1(
    function: &Function,
    mut value: ValueId,
    budget: &mut Bf16NativeBudgetV1<'_>,
) -> Bf16NativeResultV1<ValueId> {
    let blocks = bf16_native_blocks_v1(function)?;
    for _ in 0..=32 {
        budget.charge_work(blocks.len())?;
        let mut parameter = None;
        for block in blocks {
            budget.charge_work(block.parameters.len())?;
            if let Some(index) = block.parameters.iter().position(|p| p.id == value) {
                if parameter.replace((block.id, index)).is_some() {
                    return Err(bf16_native_refuse_v1("duplicate block parameter"));
                }
            }
        }
        let Some((target, index)) = parameter else {
            return Ok(value);
        };
        let mut incoming = None;
        budget.charge_work(blocks.len())?;
        for block in blocks {
            bf16_native_edges_v1(
                block
                    .terminator
                    .as_ref()
                    .ok_or_else(|| bf16_native_refuse_v1("missing terminator"))?,
                budget,
                |actual, arguments, budget| {
                    budget.charge_work(1)?;
                    if actual == target {
                        let next = *arguments
                            .get(index)
                            .ok_or_else(|| bf16_native_refuse_v1("edge arity"))?;
                        if incoming.replace(next).is_some() {
                            return Err(bf16_native_refuse_v1(
                                "nonliteral multi-predecessor transport",
                            ));
                        }
                    }
                    Ok(())
                },
            )?;
        }
        value = incoming.ok_or_else(|| bf16_native_refuse_v1("missing parameter predecessor"))?;
    }
    Err(bf16_native_refuse_v1("transport depth"))
}

fn bf16_native_exact_trap_v1(
    function: &Function,
    budget: &mut Bf16NativeBudgetV1<'_>,
) -> Bf16NativeResultV1<bool> {
    // Exact eight-row intrinsic catalog and borrowed single capability check;
    // do not construct a declaration or allocate a diagnostic Print operand list.
    budget.charge_work(
        function
            .id
            .as_str()
            .len()
            .checked_add(2)
            .and_then(|n| n.checked_mul(8))
            .ok_or(Bf16NativeResourceV1::Arithmetic)?,
    )?;
    if !matches!(
        AmdGpuDiagnosticOperation::from_intrinsic_call(&function.id, &[]),
        Some(AmdGpuDiagnosticOperation::Trap)
    ) || function.body.is_some()
        || !function.signature.parameters.is_empty()
        || !function.signature.results.is_empty()
        || function.required_capabilities.len() != 1
    {
        return Ok(false);
    }
    let Some(TargetCapability::Extension { namespace, name }) =
        function.required_capabilities.iter().next()
    else {
        return Ok(false);
    };
    budget.charge_work(
        namespace
            .len()
            .checked_add(name.len())
            .and_then(|n| n.checked_add(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()))
            .and_then(|n| n.checked_add(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len()))
            .ok_or(Bf16NativeResourceV1::Arithmetic)?,
    )?;
    Ok(namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
        && name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME)
}

fn derive_native_bf16_context_v1<'a>(
    owner: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    requested_return: [u8; 4],
    budget: &mut Bf16NativeBudgetV1<'_>,
) -> Bf16NativeResultV1<NativeBf16HelperContextV1<'a>> {
    use fe2o3_kernel_ir::{
        AmdGpuDiagnosticOperation as DiagnosticOp, LaunchDomain, LaunchExtent,
        TensorLayoutContractV1,
    };
    budget.check_prior_denials_v1()?;
    budget.charge_work(8)?;
    let module = owner.module();
    if !matches!(requested_return, [0, 1, 2, 3] | [1, 0, 2, 3])
        || module.kernels.len() != 1
        || !(2..=3).contains(&module.functions.len())
    {
        return Err(bf16_native_refuse_v1("finite owner or Return roster"));
    }
    let kernel = &module.kernels[0];
    if !matches!(
        kernel.domain,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64)
        }
    ) || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
    {
        return Err(bf16_native_refuse_v1("exact complete wave64 geometry"));
    }
    let mut root = None;
    let mut helper = None;
    let mut trap = None;
    budget.charge_work(module.functions.len())?;
    for function in &module.functions {
        budget.charge_work(
            function
                .id
                .as_str()
                .len()
                .checked_add(kernel.entry.as_str().len())
                .ok_or(Bf16NativeResourceV1::Arithmetic)?,
        )?;
        match function.role {
            FunctionRole::KernelEntry if function.id == kernel.entry => {
                if root.replace(function).is_some() {
                    return Err(bf16_native_refuse_v1("duplicate root"));
                }
            }
            FunctionRole::InternalHelper => {
                if helper.replace(function).is_some() {
                    return Err(bf16_native_refuse_v1("extra helper"));
                }
            }
            FunctionRole::ExternalImport => {
                // Zero arguments excludes allocating Print descriptor variants.
                if !bf16_native_exact_trap_v1(function, budget)? || trap.replace(function).is_some()
                {
                    return Err(bf16_native_refuse_v1("nonexact Trap declaration"));
                }
            }
            _ => return Err(bf16_native_refuse_v1("unrelated function")),
        }
    }
    let root = root.ok_or_else(|| bf16_native_refuse_v1("missing root"))?;
    let helper = helper.ok_or_else(|| bf16_native_refuse_v1("missing helper"))?;
    let body = helper
        .body
        .as_ref()
        .ok_or_else(|| bf16_native_refuse_v1("helper body"))?;
    if helper.signature.parameters.len() != 12
        || body.parameters.len() != 12
        || helper.signature.results.len() != 4
    {
        return Err(bf16_native_refuse_v1("helper scalar ABI"));
    }
    budget.charge_work(16)?;
    for i in 0..12 {
        let ty = if i < 8 {
            Type::Scalar(ScalarType::Bf16)
        } else {
            Type::F32
        };
        if helper.signature.parameters[i] != ty {
            return Err(bf16_native_refuse_v1("helper input type"));
        }
    }
    if helper.signature.results.iter().any(|ty| *ty != Type::F32) {
        return Err(bf16_native_refuse_v1("helper result type"));
    }
    bf16_native_acyclic_v1(root, budget)?;
    bf16_native_acyclic_v1(helper, budget)?;
    let mut call = None;
    let mut matrix_site = None;
    let mut helper_return = None;
    let mut root_returns = 0usize;
    let mut traps = 0usize;
    let mut root_values = root.signature.parameters.len();
    for (slot, function) in [root, helper].into_iter().enumerate() {
        let blocks = bf16_native_blocks_v1(function)?;
        budget.charge_work(blocks.len())?;
        for block in blocks {
            if slot == 0 {
                root_values = root_values
                    .checked_add(block.parameters.len())
                    .ok_or(Bf16NativeResourceV1::Arithmetic)?;
            }
            budget.charge_work(block.operations.len())?;
            for (ordinal, operation) in block.operations.iter().enumerate() {
                budget.charge_work(
                    operation
                        .results
                        .len()
                        .checked_add(2)
                        .ok_or(Bf16NativeResourceV1::Arithmetic)?,
                )?;
                if slot == 0 {
                    root_values = root_values
                        .checked_add(operation.results.len())
                        .ok_or(Bf16NativeResourceV1::Arithmetic)?;
                }
                match &operation.kind {
                    OperationKind::Call { callee, arguments } => {
                        budget.charge_work(
                            callee
                                .as_str()
                                .len()
                                .checked_add(helper.id.as_str().len())
                                .ok_or(Bf16NativeResourceV1::Arithmetic)?,
                        )?;
                        if *callee == helper.id {
                            if slot != 0
                                || arguments.len() != 12
                                || operation.results.len() != 4
                                || operation.results.iter().any(|r| r.ty != Type::F32)
                                || call.replace((block.id, ordinal)).is_some()
                            {
                                return Err(bf16_native_refuse_v1("sole actual helper Call"));
                            }
                        } else {
                            budget.charge_work(
                                callee
                                    .as_str()
                                    .len()
                                    .checked_add(2)
                                    .and_then(|n| n.checked_mul(8))
                                    .ok_or(Bf16NativeResourceV1::Arithmetic)?,
                            )?;
                            if slot != 0
                                || !arguments.is_empty()
                                || !operation.results.is_empty()
                                || !matches!(
                                    DiagnosticOp::from_intrinsic_call(callee, arguments),
                                    Some(DiagnosticOp::Trap)
                                )
                            {
                                return Err(bf16_native_refuse_v1("extra call"));
                            }
                            traps = traps
                                .checked_add(1)
                                .ok_or(Bf16NativeResourceV1::Arithmetic)?;
                        }
                    }
                    OperationKind::Matrix(matrix) => {
                        let MatrixOperationKind::MultiplyAccumulate {
                            lhs,
                            rhs,
                            accumulator,
                            ..
                        } = &matrix.kind
                        else {
                            return Err(bf16_native_refuse_v1("different Matrix kind"));
                        };
                        budget.charge_work(20)?;
                        let expected =
                            MatrixOperation::multiply_accumulate(*lhs, *rhs, *accumulator)
                                .with_declared_tensor_layout(
                                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                                        .with_zero_filled_predicate_inputs(),
                                );
                        if slot != 1
                            || *matrix != expected
                            || operation.results.len() != 4
                            || operation.results.iter().any(|r| r.ty != Type::F32)
                            || matrix_site.replace((matrix, &operation.results)).is_some()
                        {
                            return Err(bf16_native_refuse_v1("sole exact MFMA contract"));
                        }
                        for (group, values) in [lhs, rhs, accumulator].into_iter().enumerate() {
                            for (i, value) in values.iter().enumerate() {
                                if bf16_native_origin_v1(helper, *value, budget)?
                                    != body.parameters[group * 4 + i]
                                {
                                    return Err(bf16_native_refuse_v1("MFMA formal operand role"));
                                }
                            }
                        }
                    }
                    _ if slot == 1 => return Err(bf16_native_refuse_v1("extra helper operation")),
                    _ => {}
                }
            }
            match block.terminator.as_ref() {
                Some(Terminator::Return { values }) if slot == 0 => {
                    if !values.is_empty() {
                        return Err(bf16_native_refuse_v1("root Return"));
                    }
                    root_returns = root_returns
                        .checked_add(1)
                        .ok_or(Bf16NativeResourceV1::Arithmetic)?;
                }
                Some(Terminator::Return { values }) => {
                    if values.len() != 4 || helper_return.replace(values).is_some() {
                        return Err(bf16_native_refuse_v1("helper Return roster"));
                    }
                }
                Some(Terminator::Branch { .. }) => {}
                Some(_) if slot == 0 => {}
                _ => return Err(bf16_native_refuse_v1("helper control")),
            }
        }
    }
    if root_returns != 1 || trap.is_some() != (traps != 0) {
        return Err(bf16_native_refuse_v1("root Return or Trap presence"));
    }
    let (call_block, call_ordinal) = call.ok_or_else(|| bf16_native_refuse_v1("missing Call"))?;
    let (matrix, results) = matrix_site.ok_or_else(|| bf16_native_refuse_v1("missing MFMA"))?;
    let returned = helper_return.ok_or_else(|| bf16_native_refuse_v1("missing helper Return"))?;
    budget.charge_work(4)?;
    for (i, value) in returned.iter().enumerate() {
        if bf16_native_origin_v1(helper, *value, budget)?
            != results[requested_return[i] as usize].id
        {
            return Err(bf16_native_refuse_v1("actual optimized Return permutation"));
        }
    }
    // Selected finite scan allowance is not a whole-engine meter. Uniformity's
    // existing tree/CFG containers and bounded relational engine stay excluded.
    let b = bf16_native_blocks_v1(root)?
        .len()
        .checked_add(1)
        .ok_or(Bf16NativeResourceV1::Arithmetic)?;
    let analysis_work = b
        .checked_mul(b)
        .and_then(|n| n.checked_mul(root_values.checked_add(1)?))
        .and_then(|n| n.checked_mul(64))
        .ok_or(Bf16NativeResourceV1::Arithmetic)?;
    budget.charge_work(analysis_work)?;
    let report = fe2o3_kernel_analysis::analyze_kernel_entry(module, root);
    budget.charge_work(
        root.id
            .as_str()
            .len()
            .checked_add(bf16_native_blocks_v1(root)?.len())
            .ok_or(Bf16NativeResourceV1::Arithmetic)?,
    )?;
    if report.function() != &root.id
        || report.block_control(call_block) > fe2o3_kernel_analysis::Variation::WorkgroupUniform
    {
        return Err(bf16_native_refuse_v1(
            "actual O Call lacks complete wave participation",
        ));
    }
    budget.charge_work(report.diagnostics().len())?;
    for diagnostic in report.diagnostics() {
        match diagnostic {
            fe2o3_kernel_analysis::Diagnostic::Unsupported {
                block: Some(block),
                operation_index: Some(index),
                reason: fe2o3_kernel_analysis::UnsupportedReason::CallWithoutSummary { callee },
            } => {
                budget.charge_work(
                    callee
                        .as_str()
                        .len()
                        .checked_add(helper.id.as_str().len())
                        .ok_or(Bf16NativeResourceV1::Arithmetic)?,
                )?;
                if *block != call_block || *index != call_ordinal || *callee != helper.id {
                    return Err(bf16_native_refuse_v1("unrelated incomplete uniformity"));
                }
            }
            _ => return Err(bf16_native_refuse_v1("incomplete uniformity")),
        }
    }
    // Unknown helper results remain varying. No analysis summary is fabricated.
    drop(report);
    Ok(NativeBf16HelperContextV1 {
        owner,
        root,
        call_block,
        call_ordinal,
        helper,
        matrix,
        workgroup: WorkgroupSize::new(64, 1, 1),
    })
}

fn bf16_native_frame_storage_v1() -> usize {
    std::mem::size_of::<NativeBf16HelperContextV1<'_>>()
        + std::mem::size_of::<fe2o3_kernel_analysis::AnalysisReport>()
        + std::mem::size_of::<([usize; 32], [bool; 32])>()
}

/// Compiler-internal, actual-owner BF16 helper continuation. It does not accept
/// a caller-created context/anchor. Generic native/legacy entries are unchanged.
/// The caller retains its checked B/O/source owner and prepays/owns LLVM text;
/// existing emitter containers, formatted errors and text policy remain separate.
/// Only the selected context/analysis headers and explicit proof scans are charged
/// here on the supplied original accumulating account. No launch is authenticated.
pub fn lower_private_bf16_canonical_v12_to_gfx942_llvm_ir_v1(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    requested_return: [u8; 4],
    budget: &mut Bf16NativeBudgetV1<'_>,
) -> Bf16NativeResultV1<String> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    let floor = budget.storage();
    let storage = bf16_native_frame_storage_v1();
    budget.reserve_storage(storage)?;
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = derive_native_bf16_context_v1(owner, requested_return, budget)?;
        lower_compiler_module_with_ordered_and_bf16_context(
            owner.module(),
            LoweringTarget::Gfx942XnackMinusV1,
            None,
            Some(SemanticAnchorInputV1::Native(owner)),
            true,
            None,
            Some(&context),
        )
        .map_err(PrivateBf16NativeLlvmErrorV1::Lowering)
    }));
    let result = match attempted {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(PrivateBf16NativeLlvmErrorV1::Panicked)
        }
    };
    // There is no callback and no escaping context/report/selected allocation.
    // LLVM text is in the caller's separately prepaid retained domain.
    let accounting = if budget.storage()
        == floor
            .checked_add(storage)
            .ok_or(Bf16NativeResourceV1::Arithmetic)?
    {
        budget.release_storage(storage)
    } else {
        Err(Bf16NativeResourceV1::Accounting)
    };
    if let Err(error) = budget.check_prior_denials_v1() {
        drop(result);
        return Err(error.into());
    }
    accounting?;
    result
}

include!("lowering_native_bf16_v12_tests.rs");
