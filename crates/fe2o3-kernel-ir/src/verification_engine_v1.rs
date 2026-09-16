use std::collections::BTreeSet;
use std::fmt;

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    DiagnosticCode, Function, FunctionId, FunctionRole, Kernel, LaunchExtent, Module,
    OperationKind, TargetCapability, Type, VerificationDiagnosticCollectorV1,
    VerificationDiagnosticLocationV1, VerificationErrors, VerificationModuleStateV1,
    VerifiedKernelIrModuleV1, target_capability_is_supported_owned_with_budget_v1,
    verification_type_facts_v15,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum MeteredKernelIrVerificationErrorV1 {
    Verification(VerificationErrors),
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
}

impl From<CanonicalKernelIrVerificationResourceErrorV1> for MeteredKernelIrVerificationErrorV1 {
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

/// Verifies an exact decoded V12 module under a shared work and local storage
/// budget. The exact decoder's depth checks are a required precondition for
/// bounded diagnostic formatting of recursive types.
pub(crate) fn verify_exact_decoded_module_with_budget_v1<'module>(
    module: &'module Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerifiedKernelIrModuleV1<'module>, MeteredKernelIrVerificationErrorV1> {
    verify_depth_bounded_module_with_budget_v1(module, supported_capabilities, budget)
}

/// Verifies an in-memory module after the caller has established the same
/// recursive-type depth bound enforced by exact V12 decoding.
pub(crate) fn verify_depth_bounded_module_with_budget_v1<'module>(
    module: &'module Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerifiedKernelIrModuleV1<'module>, MeteredKernelIrVerificationErrorV1> {
    let mut count = VerificationDiagnosticCollectorV1::count();
    run_verification_pass_v1(module, supported_capabilities, &mut count, budget)?;
    let diagnostic_count = count
        .counted()
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
    if diagnostic_count == 0 {
        count.abandon(budget)?;
        return Ok(VerifiedKernelIrModuleV1::new_verified_v1(module));
    }

    let mut diagnostics = VerificationDiagnosticCollectorV1::materialize(diagnostic_count, budget)?;
    if let Err(error) =
        run_verification_pass_v1(module, supported_capabilities, &mut diagnostics, budget)
    {
        let _ = diagnostics.abandon(budget);
        return Err(error.into());
    }
    let diagnostics = diagnostics.finish_materialized(budget)?;
    Err(MeteredKernelIrVerificationErrorV1::Verification(
        VerificationErrors::from_sorted_diagnostics_v1(diagnostics),
    ))
}

fn run_verification_pass_v1(
    module: &Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let module_state = VerificationModuleStateV1::build(module, budget)?;
    let result = run_verification_pass_inner_v1(
        module,
        &module_state,
        supported_capabilities,
        diagnostics,
        budget,
    );
    let released = module_state.release(budget);
    result.and(released)
}

fn run_verification_pass_inner_v1<'module>(
    module: &'module Module,
    module_state: &VerificationModuleStateV1<'module>,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    if module.id.as_str().is_empty() {
        emit_fixed_v1(
            diagnostics,
            module_diagnostic_location_v1(module, budget)?,
            DiagnosticCode::InvalidIdentity,
            "module identity must not be empty",
            budget,
        )?;
    }
    if let Some(supported) = supported_capabilities {
        // Preserve the legacy verifier's validation of the target roster
        // itself. A valid member is necessarily supported by the same roster,
        // so this pass needs no nested support query.
        verify_declared_capabilities_v1(
            module,
            supported,
            None,
            module_diagnostic_location_v1(module, budget)?,
            diagnostics,
            budget,
        )?;
    }
    verify_declared_capabilities_v1(
        module,
        &module.required_capabilities,
        supported_capabilities,
        module_diagnostic_location_v1(module, budget)?,
        diagnostics,
        budget,
    )?;

    budget.charge_work(module_state.function_rows().len())?;
    for pair in module_state.function_rows().windows(2) {
        let left = pair[0];
        let right = pair[1];
        module_state.charge_function_duplicate_comparison(
            &left.function.id,
            &right.function.id,
            budget,
        )?;
        if left.function.id != right.function.id {
            continue;
        }
        emit_identifier_v1(
            diagnostics,
            function_diagnostic_location_v1(module, right.function, budget)?,
            DiagnosticCode::DuplicateFunction,
            "function ",
            &right.function.id,
            " is defined more than once",
            budget,
        )?;
        if left.function.role != right.function.role {
            emit_dynamic_v1(
                diagnostics,
                function_diagnostic_location_v1(module, right.function, budget)?,
                DiagnosticCode::ConflictingFunctionRole,
                identifier_message_work_v1(right.function.id.as_str().len(), 256)?,
                format_args!(
                    "function {} has conflicting roles {:?} and {:?}",
                    right.function.id, left.function.role, right.function.role
                ),
                budget,
            )?;
        }
    }

    budget.charge_work(module.functions.len())?;
    for function in &module.functions {
        verify_function_header_v1(
            module,
            function,
            supported_capabilities,
            diagnostics,
            budget,
        )?;
        crate::run_verification_function_pass_v1(
            module,
            function,
            module_state,
            supported_capabilities,
            diagnostics,
            budget,
        )?;
    }

    budget.charge_work(module_state.kernel_rows().len())?;
    for pair in module_state.kernel_rows().windows(2) {
        let left = pair[0];
        let right = pair[1];
        module_state.charge_kernel_duplicate_comparison(left.kernel, right.kernel, budget)?;
        if left.kernel.id == right.kernel.id {
            emit_dynamic_v1(
                diagnostics,
                kernel_diagnostic_location_v1(module, right.kernel, budget)?,
                DiagnosticCode::DuplicateKernel,
                identifier_message_work_v1(right.kernel.id.as_str().len(), 128)?,
                format_args!("kernel {} is declared more than once", right.kernel.id),
                budget,
            )?;
        }
    }

    let mut traversal = VerificationKernelTraversalV1::new();
    let kernel_result = (|| {
        budget.charge_work(module.kernels.len())?;
        for kernel in &module.kernels {
            verify_kernel_v1(
                module,
                kernel,
                module_state,
                &mut traversal,
                supported_capabilities,
                diagnostics,
                budget,
            )?;
        }
        Ok(())
    })();
    let traversal_release = traversal.release(budget);
    kernel_result.and(traversal_release)?;
    budget.charge_work(module.functions.len())?;
    for function in &module.functions {
        if function.role == FunctionRole::KernelEntry
            && !module_state.entry_is_referenced(&function.id, budget)?
        {
            emit_fixed_v1(
                diagnostics,
                function_diagnostic_location_v1(module, function, budget)?,
                DiagnosticCode::InvalidFunctionRole,
                "KernelEntry function is not referenced by any kernel record",
                budget,
            )?;
        }
    }
    Ok(())
}

fn verify_function_header_v1(
    module: &Module,
    function: &Function,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let location = function_diagnostic_location_v1(module, function, budget)?;
    if function.id.as_str().is_empty() {
        emit_fixed_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::InvalidIdentity,
            "function identity must not be empty",
            budget,
        )?;
    }
    verify_declared_capabilities_v1(
        module,
        &function.required_capabilities,
        supported_capabilities,
        clone_diagnostic_location_v1(&location, budget)?,
        diagnostics,
        budget,
    )?;
    budget.charge_work(
        function
            .signature
            .parameters
            .len()
            .checked_add(function.signature.results.len())
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    for ty in function
        .signature
        .parameters
        .iter()
        .chain(&function.signature.results)
    {
        if verify_type_v12_with_budget_v1(ty, &location, diagnostics, budget)? {
            emit_fixed_v1(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?,
                DiagnosticCode::InvalidSemanticOperation,
                "execution roles cannot cross a function signature",
                budget,
            )?;
        }
    }

    // Reserved declaration checks are allocation-free in the shared
    // descriptor helpers used by operation capability visitation.
    crate::verify_reserved_function_declaration_v1(function, &location, diagnostics, budget)?;

    let role_requires_body = function.role != FunctionRole::ExternalImport;
    budget.charge_work(1)?;
    if role_requires_body != function.body.is_some() {
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::InvalidFunctionRole,
            256,
            format_args!(
                "function role {:?} is incompatible with a {} body",
                function.role,
                if function.body.is_some() {
                    "present"
                } else {
                    "missing"
                }
            ),
            budget,
        )?;
    }
    let Some(body) = &function.body else {
        return Ok(());
    };
    budget.charge_work(1)?;
    if body.parameters.len() != function.signature.parameters.len() {
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::SignatureMismatch,
            256,
            format_args!(
                "body defines {} parameter values but signature has {} parameters",
                body.parameters.len(),
                function.signature.parameters.len()
            ),
            budget,
        )?;
    }
    if body.blocks.is_empty() {
        emit_fixed_v1(
            diagnostics,
            location,
            DiagnosticCode::EmptyFunction,
            "defined function must contain an entry block",
            budget,
        )?;
    }
    Ok(())
}

fn verify_kernel_v1<'module>(
    module: &'module Module,
    kernel: &'module Kernel,
    module_state: &VerificationModuleStateV1<'module>,
    traversal: &mut VerificationKernelTraversalV1,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let location = kernel_diagnostic_location_v1(module, kernel, budget)?;
    if kernel.id.as_str().is_empty() {
        emit_fixed_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::InvalidIdentity,
            "kernel identity must not be empty",
            budget,
        )?;
    }
    verify_declared_capabilities_v1(
        module,
        &kernel.required_capabilities,
        supported_capabilities,
        clone_diagnostic_location_v1(&location, budget)?,
        diagnostics,
        budget,
    )?;
    budget.charge_work(kernel.domain.rank() as usize)?;
    for extent in kernel.domain.extents() {
        if matches!(extent, LaunchExtent::Static(0)) {
            emit_fixed_v1(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?,
                DiagnosticCode::InvalidLaunchDomain,
                "static launch extents must be non-zero",
                budget,
            )?;
        }
    }
    budget.charge_work(1)?;
    if let Some(size) = kernel.workgroup_size {
        if size.x == 0 || size.y == 0 || size.z == 0 {
            emit_fixed_v1(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?,
                DiagnosticCode::InvalidWorkgroupSize,
                "workgroup dimensions must be non-zero",
                budget,
            )?;
        }
        if (kernel.domain.rank() == 1 && (size.y != 1 || size.z != 1))
            || (kernel.domain.rank() == 2 && size.z != 1)
        {
            emit_fixed_v1(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?,
                DiagnosticCode::InvalidWorkgroupSize,
                "inactive workgroup dimensions must be one",
                budget,
            )?;
        }
    }
    let Some(entry_row) = module_state.find_function_row(&kernel.entry, budget)? else {
        emit_dynamic_v1(
            diagnostics,
            location,
            DiagnosticCode::UnknownKernelEntry,
            identifier_message_work_v1(kernel.entry.as_str().len(), 128)?,
            format_args!("entry function {} is not in the module", kernel.entry),
            budget,
        )?;
        return Ok(());
    };
    let entry = entry_row.function;
    if entry.role != FunctionRole::KernelEntry {
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::ConflictingFunctionRole,
            identifier_message_work_v1(
                kernel
                    .id
                    .as_str()
                    .len()
                    .checked_add(entry.id.as_str().len())
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
                256,
            )?,
            format_args!(
                "kernel {} references function {} with role {:?}, expected KernelEntry",
                kernel.id, entry.id, entry.role
            ),
            budget,
        )?;
    }
    if entry.body.is_none() {
        emit_dynamic_v1(
            diagnostics,
            location,
            DiagnosticCode::KernelEntryDeclaration,
            identifier_message_work_v1(entry.id.as_str().len(), 128)?,
            format_args!("entry function {} has no body", entry.id),
            budget,
        )?;
        return Ok(());
    }
    if !entry.signature.results.is_empty() {
        emit_fixed_v1(
            diagnostics,
            kernel_diagnostic_location_v1(module, kernel, budget)?,
            DiagnosticCode::KernelReturnsValue,
            "kernel entry functions must not return values",
            budget,
        )?;
    }
    verify_reachable_intrinsic_axes_v1(
        module,
        kernel,
        entry_row.input_ordinal,
        module_state,
        traversal,
        diagnostics,
        budget,
    )
}

/// One lazy scratch owner for all kernel closures in a verification pass.
/// Generation tags avoid clearing every function for each independent kernel.
struct VerificationKernelTraversalV1 {
    visited_generations: Vec<usize>,
    pending: Vec<usize>,
    generation: usize,
    retained_storage: usize,
}

impl VerificationKernelTraversalV1 {
    const fn new() -> Self {
        Self {
            visited_generations: Vec::new(),
            pending: Vec::new(),
            generation: 0,
            retained_storage: 0,
        }
    }

    fn begin_kernel(
        &mut self,
        function_count: usize,
        entry_ordinal: usize,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let initialize = self.visited_generations.is_empty();
        let initialization_work = if initialize { function_count } else { 0 };
        let retained_storage = function_count
            .checked_mul(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        // Initial zero-fill once; every closure advances its generation,
        // marks the authenticated entry, and publishes it before traversal.
        budget.charge_work(
            initialization_work
                .checked_add(3)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
        )?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        if entry_ordinal >= function_count
            || !self.pending.is_empty()
            || (!initialize && self.visited_generations.len() != function_count)
        {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }
        if initialize {
            budget.reserve_storage(retained_storage)?;
            let allocation = (|| {
                let mut pending = Vec::new();
                pending
                    .try_reserve_exact(function_count)
                    .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
                let mut visited_generations = Vec::new();
                visited_generations
                    .try_reserve_exact(function_count)
                    .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
                if pending.capacity() != function_count
                    || visited_generations.capacity() != function_count
                {
                    return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
                }
                visited_generations.resize(function_count, 0);
                Ok((visited_generations, pending))
            })();
            let (visited_generations, pending) = match allocation {
                Ok(owners) => owners,
                Err(error) => {
                    // Partial allocation owners have dropped before release.
                    budget.release_storage(retained_storage)?;
                    return Err(error);
                }
            };
            self.visited_generations = visited_generations;
            self.pending = pending;
            self.retained_storage = retained_storage;
        }
        self.generation = generation;
        self.visited_generations[entry_ordinal] = generation;
        self.pending.push(entry_ordinal);
        Ok(())
    }

    fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        drop(self.visited_generations);
        drop(self.pending);
        budget.release_storage(self.retained_storage)
    }
}

fn verify_reachable_intrinsic_axes_v1<'module>(
    module: &'module Module,
    kernel: &'module Kernel,
    entry_ordinal: usize,
    module_state: &VerificationModuleStateV1<'module>,
    traversal: &mut VerificationKernelTraversalV1,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let function_count = module.functions.len();
    traversal.begin_kernel(function_count, entry_ordinal, budget)?;
    while !traversal.pending.is_empty() {
        budget.charge_work(1)?;
        let function_ordinal = traversal
            .pending
            .pop()
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        let function = module
            .functions
            .get(function_ordinal)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        let Some(body) = &function.body else {
            continue;
        };
        budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            budget.charge_work(block.operations.len())?;
            for (operation_index, operation) in block.operations.iter().enumerate() {
                if let OperationKind::Intrinsic(intrinsic) = &operation.kind
                    && !kernel.domain.contains_axis(intrinsic.kind.axis())
                {
                    let axis = intrinsic.kind.axis();
                    let mut location = function_diagnostic_location_v1(module, function, budget)?;
                    budget.charge_work(1)?;
                    location.kernel = Some(&kernel.id);
                    emit_dynamic_v1(
                        diagnostics,
                        location.at_block(block.id).at_operation(operation_index),
                        DiagnosticCode::InvalidLaunchDomain,
                        identifier_message_work_v1(kernel.id.as_str().len(), 256)?,
                        format_args!(
                            "axis {axis:?} is outside the {}D launch domain of kernel {}",
                            kernel.domain.rank(),
                            kernel.id
                        ),
                        budget,
                    )?;
                }
                if let OperationKind::Call { callee, .. } = &operation.kind
                    && let Some(row) = module_state.find_function_row(callee, budget)?
                {
                    if row.input_ordinal >= function_count {
                        return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
                    }
                    if traversal.visited_generations[row.input_ordinal] != traversal.generation {
                        // Each ordinal is marked before publication, so the
                        // reusable pending buffer cannot exceed function_count.
                        budget.charge_work(2)?;
                        traversal.visited_generations[row.input_ordinal] = traversal.generation;
                        traversal.pending.push(row.input_ordinal);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn verify_declared_capabilities_v1(
    _module: &Module,
    capabilities: &BTreeSet<TargetCapability>,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    location: VerificationDiagnosticLocationV1<'_>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(capabilities.len())?;
    let mut has_dynamic_workgroup = false;
    let mut has_workgroup = false;
    let mut first_wave = None;
    let mut wave_count = 0_usize;
    let mut conflicting_subgroup = false;
    for capability in capabilities {
        match capability {
            TargetCapability::DynamicWorkgroupMemory => has_dynamic_workgroup = true,
            TargetCapability::WorkgroupMemory => has_workgroup = true,
            TargetCapability::WaveWidth(width) => {
                first_wave.get_or_insert(*width);
                wave_count = wave_count
                    .checked_add(1)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            }
            _ => {}
        }
    }
    if let Some(wave) = first_wave {
        budget.charge_work(capabilities.len())?;
        conflicting_subgroup = capabilities.iter().any(|capability| {
            matches!(capability, TargetCapability::SubgroupSize(size) if *size != wave.lanes())
        });
    }
    if has_dynamic_workgroup && !has_workgroup {
        emit_fixed_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::InvalidCapability,
            "dynamic workgroup memory requires the base workgroup-memory capability",
            budget,
        )?;
    }
    if wave_count > 1 {
        let work = capabilities
            .len()
            .checked_mul(64)
            .and_then(|work| work.checked_add(128))
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::InvalidCapability,
            work,
            format_args!(
                "conflicting exact wave-width requirements: {:?}",
                WaveWidthsDebugV1(capabilities)
            ),
            budget,
        )?;
    }
    if conflicting_subgroup {
        let wave = first_wave.ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(&location, budget)?,
            DiagnosticCode::InvalidCapability,
            128,
            format_args!(
                "wave width {} conflicts with the declared subgroup size",
                wave.lanes()
            ),
            budget,
        )?;
    }

    budget.charge_work(capabilities.len())?;
    for capability in capabilities {
        let invalid = match capability {
            TargetCapability::SubgroupSize(size) => *size == 0 || !size.is_power_of_two(),
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => {
                !matches!(*width_bits, 8 | 16 | 32 | 64)
                    || !matches!(
                        address_space,
                        crate::AddressSpace::Workgroup
                            | crate::AddressSpace::Global
                            | crate::AddressSpace::Generic
                    )
                    || *max_scope == crate::SynchronizationScope::Invocation
                    || (*address_space == crate::AddressSpace::Workgroup
                        && max_scope.rank() > crate::SynchronizationScope::Workgroup.rank())
            }
            TargetCapability::Extension { namespace, name } => {
                namespace.is_empty() || name.is_empty()
            }
            _ => false,
        };
        let message_work = capability_message_work_v1(capability)?;
        if invalid {
            emit_dynamic_v1(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?,
                DiagnosticCode::InvalidCapability,
                message_work,
                format_args!("malformed target capability: {capability:?}"),
                budget,
            )?;
        } else if let Some(supported) = supported_capabilities
            && !target_capability_is_supported_owned_with_budget_v1(capability, supported, budget)?
        {
            emit_dynamic_v1(
                diagnostics,
                clone_diagnostic_location_v1(&location, budget)?,
                DiagnosticCode::UnsupportedCapability,
                message_work,
                format_args!("target does not support required capability {capability:?}"),
                budget,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn verify_type_v12_with_budget_v1(
    ty: &Type,
    location: &VerificationDiagnosticLocationV1<'_>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    let facts = verification_type_facts_v15(ty, budget)?;
    if facts.invalid_execution_role {
        emit_fixed_v1(
            diagnostics,
            clone_diagnostic_location_v1(location, budget)?,
            DiagnosticCode::InvalidSemanticOperation,
            "execution roles must have valid geometry and cannot be nested in memory types",
            budget,
        )?;
    }
    if let Some(error) = facts.vector_error {
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(location, budget)?,
            DiagnosticCode::InvalidVectorOperation,
            256,
            format_args!("{error}"),
            budget,
        )?;
    }
    Ok(facts.contains_execution_role)
}

pub(crate) fn emit_fixed_v1(
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    location: VerificationDiagnosticLocationV1<'_>,
    code: DiagnosticCode,
    message: &'static str,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    diagnostics.emit(
        location,
        code,
        message.len(),
        format_args!("{message}"),
        budget,
    )
}

pub(crate) fn clone_diagnostic_location_v1<'m>(
    location: &VerificationDiagnosticLocationV1<'m>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerificationDiagnosticLocationV1<'m>, CanonicalKernelIrVerificationResourceErrorV1> {
    // Copy the five borrowed location fields; error publication owns ID bytes.
    budget.charge_work(5)?;
    Ok(*location)
}

pub(crate) fn module_diagnostic_location_v1<'m>(
    module: &'m Module,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerificationDiagnosticLocationV1<'m>, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(5)?;
    Ok(VerificationDiagnosticLocationV1::module(module))
}

pub(crate) fn function_diagnostic_location_v1<'m>(
    module: &'m Module,
    function: &'m Function,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerificationDiagnosticLocationV1<'m>, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(5)?;
    Ok(VerificationDiagnosticLocationV1::function(module, function))
}

pub(crate) fn kernel_diagnostic_location_v1<'m>(
    module: &'m Module,
    kernel: &'m Kernel,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerificationDiagnosticLocationV1<'m>, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(5)?;
    Ok(VerificationDiagnosticLocationV1::kernel(module, kernel))
}

pub(crate) fn emit_dynamic_v1(
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    location: VerificationDiagnosticLocationV1<'_>,
    code: DiagnosticCode,
    message_work_upper: usize,
    arguments: fmt::Arguments<'_>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    diagnostics.emit(location, code, message_work_upper, arguments, budget)
}

fn emit_identifier_v1(
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    location: VerificationDiagnosticLocationV1<'_>,
    code: DiagnosticCode,
    prefix: &'static str,
    identifier: &FunctionId,
    suffix: &'static str,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    emit_dynamic_v1(
        diagnostics,
        location,
        code,
        identifier_message_work_v1(identifier.as_str().len(), prefix.len() + suffix.len())?,
        format_args!("{prefix}{identifier}{suffix}"),
        budget,
    )
}

pub(crate) fn identifier_message_work_v1(
    identifier_bytes: usize,
    fixed: usize,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    identifier_bytes
        .checked_add(fixed)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
}

fn capability_message_work_v1(
    capability: &TargetCapability,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    match capability {
        TargetCapability::Extension { namespace, name } => namespace
            .len()
            .checked_add(name.len())
            .and_then(|bytes| bytes.checked_mul(8))
            .and_then(|bytes| bytes.checked_add(512))
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic),
        _ => Ok(512),
    }
}

struct WaveWidthsDebugV1<'a>(&'a BTreeSet<TargetCapability>);

impl fmt::Debug for WaveWidthsDebugV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_list()
            .entries(self.0.iter().filter_map(|capability| match capability {
                TargetCapability::WaveWidth(width) => Some(width),
                _ => None,
            }))
            .finish()
    }
}

#[cfg(test)]
#[path = "verification_engine_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "verification_kernel_traversal_v1_tests.rs"]
mod kernel_traversal_tests;
