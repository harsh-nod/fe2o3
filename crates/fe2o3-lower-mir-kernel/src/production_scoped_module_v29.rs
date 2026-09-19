// Complete graph custody pending source replay, assertion sealing and discharge.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
struct PendingScopedModuleV29 {
    graph: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15,
    graph_storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV15,
    roots: Vec<ScopedModuleRootV29>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    retained_storage: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
struct ScopedModuleRootV29 {
    function_ordinal: usize,
    sidecars: InstanceRowsV1<PendingInstanceSidecarsV29>,
    coordinates: OwnedInstanceCoordinatesV1,
    slot_relocation: Option<scoped_slot_relocation_v29::RelocationV29>,
    source_slots: OwnedScopedSourceSlotsV29,
    insertions: Vec<LifecycleInsertionV29>,
    declarations: Vec<ScopedDeclarationUseV29>,
    // Cumulative through this root, not an independently summable per-root cost.
    private_payload: PrivateArrayPayloadV1,
    requires_context_issue: bool,
    inherited_emission_storage: usize,
    inherited_assembly_storage: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedDeclarationKindV29 {
    Diagnostic,
    Float,
}

#[derive(Debug)]
struct ScopedDeclarationUseV29 {
    instance: usize,
    kind: ScopedDeclarationKindV29,
    id: FunctionId,
    function_ordinal: usize,
}

#[derive(Debug)]
enum ScopedModuleErrorV29 {
    Source(ProductionSemanticKirErrorV1),
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV15),
}

impl From<ProductionSemanticKirErrorV1> for ScopedModuleErrorV29 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Source(error)
    }
}

impl From<ArgumentResourceV1> for ScopedModuleErrorV29 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

fn scoped_module_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "scoped module differs from its complete source roster",
    )
}

fn scoped_capability_width_v29(
    capability: &fe2o3_kernel_ir::TargetCapability,
) -> Result<usize, ArgumentResourceV1> {
    match capability {
        fe2o3_kernel_ir::TargetCapability::Extension { namespace, name } => {
            argument_sum_v1(&[16, namespace.len(), name.len()])
        }
        _ => Ok(16),
    }
}

fn scoped_copy_string_v29(
    value: &str,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<String, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[value.len(), 2])?)?;
    budget.reserve_storage(value.len())?;
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(
        copy.capacity()
            .checked_sub(value.len())
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    copy.push_str(value);
    Ok(copy)
}

fn scoped_module_capabilities_v29(
    output: &mut BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    source: &BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    let mut width = 16;
    for capability in output.iter().chain(source) {
        budget.charge_work(3)?;
        width = width.max(scoped_capability_width_v29(capability)?);
    }
    for capability in source {
        budget.charge_work(argument_product_v1(
            argument_product_v1(argument_sum_v1(&[output.len(), 1])?, 2)?,
            width,
        )?)?;
        if output.contains(capability) {
            continue;
        }
        budget.reserve_storage(size_of::<fe2o3_kernel_ir::TargetCapability>())?;
        let copy = match capability {
            fe2o3_kernel_ir::TargetCapability::Extension { namespace, name } => {
                fe2o3_kernel_ir::TargetCapability::Extension {
                    namespace: scoped_copy_string_v29(namespace, budget)?,
                    name: scoped_copy_string_v29(name, budget)?,
                }
            }
            other => other.clone(),
        };
        output.insert(copy);
    }
    Ok(())
}

fn scoped_declaration_equal_v29(
    left: &Function,
    right: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        left.id.as_str().len(),
        right.id.as_str().len(),
        8,
    ])?)?;
    if left.id != right.id
        || left.role != fe2o3_kernel_ir::FunctionRole::ExternalImport
        || right.role != fe2o3_kernel_ir::FunctionRole::ExternalImport
        || left.body.is_some()
        || right.body.is_some()
        || left.signature.parameters.len() != right.signature.parameters.len()
        || left.signature.results.len() != right.signature.results.len()
        || left.required_capabilities.len() != right.required_capabilities.len()
    {
        return Ok(false);
    }
    for (left, right) in left
        .signature
        .parameters
        .iter()
        .chain(&left.signature.results)
        .zip(
            right
                .signature
                .parameters
                .iter()
                .chain(&right.signature.results),
        )
    {
        if !call_splice_type_eq_v1(left, right, budget)
            .map_err(|error| pending_scope_correspondence_error_v29(error.into()))?
        {
            return Ok(false);
        }
    }
    for (left, right) in left
        .required_capabilities
        .iter()
        .zip(&right.required_capabilities)
    {
        budget.charge_work(argument_sum_v1(&[
            scoped_capability_width_v29(left)?,
            scoped_capability_width_v29(right)?,
        ])?)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

fn scoped_declaration_v29(
    module: &mut Module,
    key: FunctionId,
    declaration: Function,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(FunctionId, usize), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        key.as_str().len(),
        declaration.id.as_str().len(),
        4,
    ])?)?;
    if key != declaration.id
        || declaration.role != fe2o3_kernel_ir::FunctionRole::ExternalImport
        || declaration.body.is_some()
    {
        return Err(scoped_module_error_v29());
    }
    for function in &module.functions {
        budget.charge_work(argument_sum_v1(&[
            function.id.as_str().len(),
            key.as_str().len(),
            1,
        ])?)?;
    }
    let ordinal = match module
        .functions
        .iter()
        .position(|function| function.id == key)
    {
        Some(ordinal) => {
            if ordinal < module.kernels.len()
                || !scoped_declaration_equal_v29(&module.functions[ordinal], &declaration, budget)?
            {
                return Err(scoped_module_error_v29());
            }
            ordinal
        }
        None => {
            let count = argument_sum_v1(&[module.functions.len(), 1])?;
            enforce_limit(
                ProductionSemanticKirResourceV1::Functions,
                count,
                limits.max_functions,
            )?;
            scoped_module_capabilities_v29(
                &mut module.required_capabilities,
                &declaration.required_capabilities,
                budget,
            )?;
            let ordinal = module.functions.len();
            emission_push_v1(&mut module.functions, declaration, budget)?;
            ordinal
        }
    };
    Ok((key, ordinal))
}

fn scoped_module_name_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<String, ProductionSemanticKirErrorV1> {
    const PREFIX: &str = "fe2o3::semantic::";
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut complete = String::new();
    budget.charge_work(96 + PREFIX.len())?;
    budget.reserve_storage(64 + PREFIX.len())?;
    complete
        .try_reserve_exact(64 + PREFIX.len())
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(
        complete
            .capacity()
            .checked_sub(64 + PREFIX.len())
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    complete.push_str(PREFIX);
    for byte in source.owner.source_semantic_sha256() {
        complete.push(char::from(HEX[usize::from(byte >> 4)]));
        complete.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(complete)
}

fn scoped_module_roots_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<OwnedLifecycleInsertedRootV29>, ProductionSemanticKirErrorV1> {
    let count = source.launch.roots().len();
    let mut roots = emission_vec_v1(count, budget)?;
    let mut closure = ReachableClosureBudgetV1::new(limits.max_blocks);
    let mut private_work = PrivateArrayLazyBudgetV1::new(count, limits.max_operations);
    let mut private_payload = PrivateArrayPayloadV1::default();
    for ordinal in 0..count {
        budget.charge_work(1)?;
        let pending = emit_pending_source_root_v29(
            source,
            ordinal,
            limits,
            &mut closure,
            &mut private_work,
            private_payload,
            budget,
        )?;
        private_payload = pending.private_payload;
        let mut donor = Some(pending);
        roots.push(insert_pending_lifecycle_v29(&mut donor, limits, budget)?);
    }
    Ok(roots)
}

fn scoped_module_candidate_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    emitted: Vec<OwnedLifecycleInsertedRootV29>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(Module, Vec<ScopedModuleRootV29>), ProductionSemanticKirErrorV1> {
    if source.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(3)?;
    let count = source.launch.roots().len();
    if count == 0 || emitted.len() != count {
        return Err(scoped_module_error_v29());
    }
    let mut blocks = 0;
    let mut operations = 0;
    let mut statements = 0;
    for (ordinal, emitted) in emitted.iter().enumerate() {
        let root = &emitted.root;
        budget.charge_work(8)?;
        if root.ledger != source.ledger
            || root.pending.coordinates.root != source.launch.roots()[ordinal].selected_root()
            || root.pending.coordinates.semantic_sha256 != *source.owner.source_semantic_sha256()
            || root.pending.coordinates.ssa != source.owner.identity()
            || root.kernel.entry != root.pending.function.id
        {
            return Err(scoped_module_error_v29());
        }
        let body = root
            .pending
            .function
            .body
            .as_ref()
            .ok_or_else(scoped_module_error_v29)?;
        budget.charge_work(body.blocks.len())?;
        blocks = argument_sum_v1(&[blocks, body.blocks.len()])?;
        for block in &body.blocks {
            operations = argument_sum_v1(&[operations, block.operations.len()])?;
        }
        for row in &root.pending.coordinates.sources.rows {
            budget.charge_work(1)?;
            let function = source
                .owner
                .source_semantic()
                .functions()
                .get(row.function.index() as usize)
                .ok_or_else(scoped_module_error_v29)?;
            budget.charge_work(function.blocks().len())?;
            for block in function.blocks() {
                statements = argument_sum_v1(&[statements, block.statements().len()])?;
            }
        }
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Blocks,
        blocks,
        limits.max_blocks,
    )?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        operations,
        limits.max_operations,
    )?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Statements,
        statements,
        limits.max_statements,
    )?;
    let mut module = Module::new(scoped_module_name_v29(source, budget)?);
    module.functions = emission_vec_v1(count, budget)?;
    module.kernels = emission_vec_v1(count, budget)?;
    let mut roots = emission_vec_v1(count, budget)?;
    let scratch = argument_product_v1(
        emitted.capacity(),
        size_of::<OwnedLifecycleInsertedRootV29>(),
    )?;
    budget.charge_work(argument_product_v1(count, 8)?)?;
    for (ordinal, inserted) in emitted.into_iter().enumerate() {
        let OwnedLifecycleInsertedRootV29 { root, insertions } = inserted;
        let OwnedPendingScopedRootV29 {
            pending,
            kernel,
            private_payload,
            source_slots,
            requires_context_issue,
            ledger: _,
            retained_emission_storage,
        } = root;
        let PendingScopedRootEmissionV29 {
            function,
            sidecars,
            coordinates,
            slot_relocation,
            additional_storage_bytes,
        } = pending;
        for prior in &module.kernels {
            budget.charge_work(argument_sum_v1(&[
                prior.id.as_str().len(),
                kernel.id.as_str().len(),
                prior.entry.as_str().len(),
                kernel.entry.as_str().len(),
                2,
            ])?)?;
            if prior.id == kernel.id || prior.entry == kernel.entry {
                return Err(scoped_module_error_v29());
            }
        }
        scoped_module_capabilities_v29(
            &mut module.required_capabilities,
            &function.required_capabilities,
            budget,
        )?;
        scoped_module_capabilities_v29(
            &mut module.required_capabilities,
            &kernel.required_capabilities,
            budget,
        )?;
        for sidecar in &sidecars.rows {
            budget.charge_work(1)?;
            scoped_module_capabilities_v29(
                &mut module.required_capabilities,
                &sidecar.operation_capabilities,
                budget,
            )?;
        }
        module.functions.push(function);
        module.kernels.push(kernel);
        roots.push(ScopedModuleRootV29 {
            function_ordinal: ordinal,
            sidecars,
            coordinates,
            slot_relocation,
            source_slots,
            insertions,
            declarations: Vec::new(),
            private_payload,
            requires_context_issue,
            inherited_emission_storage: retained_emission_storage,
            inherited_assembly_storage: additional_storage_bytes,
        });
    }
    budget.release_storage(scratch)?;
    // Declaration values move into the graph; original keys remain as explicit
    // per-instance routes. Every other sidecar and its coordinate system stays intact.
    for root in &mut roots {
        budget.charge_work(argument_sum_v1(&[root.sidecars.rows.len(), 1])?)?;
        for (instance, sidecar) in root.sidecars.rows.iter_mut().enumerate() {
            for (kind, declarations) in [
                (
                    ScopedDeclarationKindV29::Diagnostic,
                    &mut sidecar.diagnostic_declarations,
                ),
                (
                    ScopedDeclarationKindV29::Float,
                    &mut sidecar.float_declarations,
                ),
            ] {
                budget.charge_work(declarations.len())?;
                for (key, declaration) in std::mem::take(declarations) {
                    let (id, function_ordinal) =
                        scoped_declaration_v29(&mut module, key, declaration, limits, budget)?;
                    emission_push_v1(
                        &mut root.declarations,
                        ScopedDeclarationUseV29 {
                            instance,
                            kind,
                            id,
                            function_ordinal,
                        },
                        budget,
                    )?;
                }
            }
        }
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Functions,
        module.functions.len(),
        limits.max_functions,
    )?;
    Ok((module, roots))
}

/// Success keeps its complete reservation live, like the pending root owner.
/// Original emission envelopes conservatively coexist with the genuine V15
/// receipt: dropping the raw candidate does not refund still-owned source rows.
/// Failure/panic drops the attempted graph and source rows before restoring the
/// caller's floor; returned V15 verifier diagnostics retain their existing
/// caller-owned accounting contract.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
fn admit_pending_scoped_module_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingScopedModuleV29, ScopedModuleErrorV29> {
    if source.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let emitted = scoped_module_roots_v29(source, limits, budget)?;
        let (candidate, roots) = scoped_module_candidate_v29(source, emitted, limits, budget)?;
        let (graph, graph_storage) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
                &candidate, budget,
            ).map_err(ScopedModuleErrorV29::Canonical)?;
        budget.reserve_storage(graph_storage.retained_storage())?;
        drop(candidate);
        let retained_storage = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(PendingScopedModuleV29 {
            graph,
            graph_storage,
            roots,
            ledger: source.ledger,
            retained_storage,
        })
    }));
    match result {
        Ok(Ok(owner)) => Ok(owner),
        other => {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            match other {
                Ok(Err(error)) => Err(error),
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}
