fn private_array_output_coordinate_key_v1(
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
) -> [usize; 3] {
    [
        coordinate.block.function.0 as usize,
        coordinate.block.block as usize,
        coordinate.operation as usize,
    ]
}

fn private_array_output_arithmetic_v1() -> ProductionSourceOutputErrorV1 {
    ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Arithmetic)
}

fn private_array_output_closed_operation_v1(kind: &OperationKind) -> bool {
    use OperationKind as Op;
    // Exhaustive over the current opcode roster. Empty local effects alone are
    // not a contract for calls, assembly, execution lifecycle or unit volatile IO.
    match kind {
        Op::Alloca {
            address_space: AddressSpace::Private,
            ..
        }
        | Op::Store { .. }
        | Op::Constant(_)
        | Op::Unary { .. }
        | Op::Binary { .. }
        | Op::Compare { .. }
        | Op::Cast { .. }
        | Op::Select { .. }
        | Op::SliceLength { .. }
        | Op::SliceData { .. }
        | Op::GetElementPointer { .. }
        | Op::VectorLayoutConvert(_)
        | Op::Wave(_) => true,
        Op::Intrinsic(intrinsic) => match intrinsic.kind {
            fe2o3_kernel_ir::IntrinsicKind::InvocationIndex { .. }
            | fe2o3_kernel_ir::IntrinsicKind::LaunchExtent { .. } => true,
        },
        Op::MemoryIntrinsic(memory) => match memory {
            fe2o3_kernel_ir::MemoryIntrinsicOperation::PointerDistance { .. } => true,
            fe2o3_kernel_ir::MemoryIntrinsicOperation::VolatileLoad { .. }
            | fe2o3_kernel_ir::MemoryIntrinsicOperation::VolatileStore { .. }
            | fe2o3_kernel_ir::MemoryIntrinsicOperation::CopyNonOverlapping { .. } => false,
        },
        Op::Matrix(matrix) => match &matrix.kind {
            fe2o3_kernel_ir::MatrixOperationKind::MultiplyAccumulate { .. }
            | fe2o3_kernel_ir::MatrixOperationKind::ScaledMultiplyAccumulate { .. } => true,
            fe2o3_kernel_ir::MatrixOperationKind::LdsLoad { .. }
            | fe2o3_kernel_ir::MatrixOperationKind::LdsStore { .. } => false,
        },
        Op::Execution(_)
        | Op::VerificationContract(_)
        | Op::VectorLoad(_)
        | Op::VectorStore(_)
        | Op::Call { .. }
        | Op::Alloca { .. }
        | Op::Load { .. }
        | Op::GuardedLoad { .. }
        | Op::GuardedStore { .. }
        | Op::Barrier(_)
        | Op::Atomic(_)
        | Op::Fence(_)
        | Op::WorkgroupBarrier(_)
        | Op::WorkgroupMemory(_)
        | Op::Gfx950LdsTranspose(_)
        | Op::Gfx942OrderedRegion(_)
        | Op::Gfx942OrderedProgram(_)
        | Op::Gfx942CompleteBodyDeclaration(_)
        | Op::Gfx942CompleteBodyStep(_)
        | Op::InlineAssembly(_) => false,
    }
}

fn private_array_output_closed_ranked_operation_v1(
    operation: &ProductionRankedOperationV1,
) -> bool {
    use ProductionRankedOperationV1 as Op;
    // Explicit passive grammar plus the one memory form joined below. New or
    // unrelated effect/proof/lifecycle forms require their own consumer.
    matches!(
        operation,
        Op::ExecutionLayout { .. }
            | Op::View { .. }
            | Op::ViewInSpace { .. }
            | Op::IndexConstant { .. }
            | Op::IndexUnsignedCast { .. }
            | Op::IndexUnknown { .. }
            | Op::InvocationIndex { .. }
            | Op::IndexBinary { .. }
            | Op::DeterministicJoin { .. }
            | Op::Dimension { .. }
            | Op::SemanticSymbol { .. }
            | Op::SemanticConstant { .. }
            | Op::SemanticBinary { .. }
            | Op::SemanticExpression { .. }
            | Op::Access { .. }
    )
}

fn private_array_output_vec_v1<T>(
    count: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(3).map_err(Error::Resource)?;
    let requested = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(private_array_output_arithmetic_v1)?;
    budget.reserve_storage(requested).map_err(Error::Resource)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
    budget.charge_work(3).map_err(Error::Resource)?;
    let actual = rows
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(private_array_output_arithmetic_v1)?;
    budget
        .reserve_storage(
            actual
                .checked_sub(requested)
                .ok_or_else(private_array_output_arithmetic_v1)?,
        )
        .map_err(Error::Resource)?;
    Ok(rows)
}

fn private_array_output_push_v1<T>(
    rows: &mut Vec<T>,
    item: T,
) -> Result<(), ProductionSourceOutputErrorV1> {
    // Every caller prepays the fixed push guard with its record construction.
    if rows.len() == rows.capacity() {
        return Err(ProductionSourceOutputErrorV1::Invalid(
            "private output census exceeded reserved capacity",
        ));
    }
    rows.push(item);
    Ok(())
}

fn private_array_output_workspace_v1<'a>(
    receipt: &'a ProductionMaterializedRankedModuleReceiptV1,
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<PrivateArrayOutputWorkspaceV1<'a>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(3).map_err(Error::Resource)?;
    if !std::ptr::eq(view.source(), &receipt.materialized) || !inventory.belongs_to(view.output()) {
        return Err(Error::InputCustody);
    }
    budget
        .reserve_storage(std::mem::size_of::<PrivateArrayOutputWorkspaceV1<'_>>())
        .map_err(Error::Resource)?;
    let count = receipt
        .materialized
        .correspondence
        .private_arrays
        .effects
        .len();
    let max_operations = receipt.materialized.limits.max_operations;
    let mut sources = 0usize;
    let mut definitions = 0usize;
    let mut operations = 0usize;
    let mut accesses = 0usize;
    for root in &receipt.roots {
        budget.charge_work(3).map_err(Error::Resource)?;
        if !root.executable_effect_sources.is_empty() {
            return Err(Error::Invalid(
                "private output generated effects are outside the closed subset",
            ));
        }
        sources = sources
            .checked_add(root.access_sources.len())
            .ok_or_else(private_array_output_arithmetic_v1)?;
        for block in root.lowering.kernel().blocks() {
            budget.charge_work(1).map_err(Error::Resource)?;
            for operation in block.operations() {
                budget.charge_work(10).map_err(Error::Resource)?;
                if !private_array_output_closed_ranked_operation_v1(operation) {
                    return Err(Error::Invalid(
                        "private output ranked opcode is outside the closed subset",
                    ));
                }
                operations = operations
                    .checked_add(1)
                    .ok_or_else(private_array_output_arithmetic_v1)?;
                if operations > max_operations {
                    return Err(Error::Invalid(
                        "private output ranked operation limit exceeded",
                    ));
                }
                if private_array_ranked_definition_v1(operation).is_some() {
                    definitions = definitions
                        .checked_add(1)
                        .ok_or_else(private_array_output_arithmetic_v1)?;
                }
                if matches!(operation, ProductionRankedOperationV1::Access { .. }) {
                    accesses = accesses
                        .checked_add(1)
                        .ok_or_else(private_array_output_arithmetic_v1)?;
                }
            }
        }
    }
    budget.charge_work(3).map_err(Error::Resource)?;
    if count > max_operations || sources > max_operations || accesses != sources {
        return Err(Error::Invalid(
            "private output ranked/source census is outside the closed subset",
        ));
    }
    let mut workspace = PrivateArrayOutputWorkspaceV1 {
        originals: private_array_output_vec_v1(count, budget)?,
        sources: private_array_output_vec_v1(sources, budget)?,
        definitions: private_array_output_vec_v1(definitions, budget)?,
        facts: private_array_output_vec_v1(count, budget)?,
        stores: private_array_output_vec_v1(count, budget)?,
        allocations: private_array_output_vec_v1(count, budget)?,
        ranked: private_array_output_vec_v1(count, budget)?,
    };
    for (index, effect) in receipt
        .materialized
        .correspondence
        .private_arrays
        .effects
        .iter()
        .enumerate()
    {
        budget.charge_work(2).map_err(Error::Resource)?;
        let key = source_output_array_key_v1(
            effect.owner,
            effect.function,
            effect.semantic_block,
            effect.semantic_statement,
            effect.role,
            effect.original_index.component(),
            budget,
        )?;
        private_array_output_push_v1(
            &mut workspace.originals,
            PrivateArrayOutputOriginalV1 { key, effect: index },
        )?;
    }
    for (root_index, root) in receipt.roots.iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (row_index, source) in root.access_sources.iter().enumerate() {
            budget.charge_work(6).map_err(Error::Resource)?;
            let statement = source.semantic_statement().ok_or(Error::Invalid(
                "private output terminator access is outside the closed subset",
            ))?;
            private_array_output_push_v1(
                &mut workspace.sources,
                PrivateArrayOutputSourceV1 {
                    key: [
                        root.selected_root.index() as usize,
                        source.semantic_block() as usize,
                        statement as usize,
                        source.semantic_access_ordinal() as usize,
                    ],
                    root: root_index,
                    row: row_index,
                    consumed: false,
                },
            )?;
        }
        for block in root.lowering.kernel().blocks() {
            budget.charge_work(1).map_err(Error::Resource)?;
            for operation in block.operations() {
                budget.charge_work(3).map_err(Error::Resource)?;
                if let Some(value) = private_array_ranked_definition_v1(operation) {
                    private_array_output_push_v1(
                        &mut workspace.definitions,
                        PrivateArrayOutputDefinitionV1 {
                            key: [root_index, value.get() as usize],
                            operation,
                        },
                    )?;
                }
            }
        }
    }
    budget.charge_work(3).map_err(Error::Resource)?;
    if workspace.originals.len() != count
        || workspace.sources.len() != sources
        || workspace.definitions.len() != definitions
    {
        return Err(Error::Invalid("private output counted inputs changed"));
    }
    let mut work = PrivateArrayOutputWorkV1 { budget };
    private_array_heapsort_v1(
        &mut workspace.originals,
        |row| row.key.map(|x| x as usize),
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    private_array_heapsort_v1(
        &mut workspace.sources,
        |row| row.key,
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    private_array_heapsort_v1(
        &mut workspace.definitions,
        |row| row.key,
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    for pair in workspace.originals.windows(2) {
        work.charge_private_array_work(7)?;
        if pair[0].key >= pair[1].key {
            return Err(Error::Invalid("private output duplicate source component"));
        }
    }
    for pair in workspace.sources.windows(2) {
        work.charge_private_array_work(4)?;
        if pair[0].key >= pair[1].key {
            return Err(Error::Invalid("private output duplicate ranked source"));
        }
    }
    for pair in workspace.definitions.windows(2) {
        work.charge_private_array_work(2)?;
        if pair[0].key >= pair[1].key {
            return Err(Error::Invalid("private output duplicate ranked definition"));
        }
    }
    private_array_output_components_v1(receipt, view, &mut workspace, work.budget)?;
    private_array_output_reverse_v1(
        &mut workspace,
        inventory,
        accesses,
        max_operations,
        work.budget,
    )?;
    Ok(workspace)
}

fn private_array_output_reverse_v1(
    workspace: &mut PrivateArrayOutputWorkspaceV1<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    accesses: usize,
    max_operations: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let mut work = PrivateArrayOutputWorkV1 { budget };
    work.charge_private_array_work(3)?;
    if workspace.facts.len() != workspace.originals.len()
        || workspace.ranked.len() != accesses
        || inventory.operations().len() > max_operations
    {
        return Err(Error::Invalid(
            "private output forward census is incomplete",
        ));
    }
    for source in &workspace.sources {
        work.charge_private_array_work(1)?;
        if !source.consumed {
            return Err(Error::Invalid("private output unconsumed ranked source"));
        }
    }
    private_array_heapsort_v1(
        &mut workspace.ranked,
        |row| *row,
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    for pair in workspace.ranked.windows(2) {
        work.charge_private_array_work(3)?;
        if pair[0] >= pair[1] {
            return Err(Error::Invalid("private output ranked access reused"));
        }
    }
    private_array_heapsort_v1(
        &mut workspace.stores,
        |row| row.key,
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    for pair in workspace.stores.windows(2) {
        work.charge_private_array_work(3)?;
        if pair[0].key >= pair[1].key {
            return Err(Error::Invalid("private output Store reused"));
        }
    }
    let mut stores = 0usize;
    let mut allocations = 0usize;
    let mut effects = 0usize;
    private_array_heapsort_v1(
        &mut workspace.allocations,
        |row| [row.slot, row.key[0], row.key[1], row.key[2]],
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    for pair in workspace.allocations.windows(2) {
        work.charge_private_array_work(4)?;
        if pair[0].slot == pair[1].slot && pair[0].key != pair[1].key {
            return Err(Error::Invalid(
                "private output slot split across allocations",
            ));
        }
    }
    private_array_heapsort_v1(
        &mut workspace.allocations,
        |row| [row.key[0], row.key[1], row.key[2], row.slot],
        &mut work,
        private_array_output_arithmetic_v1,
    )?;
    let mut distinct_allocations = 0usize;
    let mut prior: Option<&PrivateArrayOutputAllocationV1> = None;
    for allocation in &workspace.allocations {
        work.charge_private_array_work(6)?;
        if let Some(previous) = prior {
            if previous.key == allocation.key && previous.slot != allocation.slot {
                return Err(Error::Invalid(
                    "private output allocation shared by original slots",
                ));
            }
        }
        if prior.is_none_or(|previous| previous.key != allocation.key) {
            distinct_allocations = distinct_allocations
                .checked_add(1)
                .ok_or_else(private_array_output_arithmetic_v1)?;
        }
        prior = Some(allocation);
    }
    for operation in inventory.operations() {
        work.charge_private_array_work(4)?;
        if !private_array_output_closed_operation_v1(&operation.operation.kind)
            || !operation.compiler_ordering().is_empty()
        {
            return Err(Error::Invalid(
                "private output opcode or ordering is outside the closed subset",
            ));
        }
        if let OperationKind::Alloca {
            address_space: AddressSpace::Private,
            ..
        } = operation.operation.kind
        {
            work.charge_private_array_work(4)?;
            let key = private_array_output_coordinate_key_v1(operation.coordinate);
            private_array_binary_search_v1(&workspace.allocations, |row| row.key, key, &mut work)?
                .map_err(|_| Error::Invalid("private output unclaimed physical allocation"))?;
            work.charge_private_array_work(6)?;
            let [effect] =
                inventory
                    .effects()
                    .get(operation.effects.clone())
                    .ok_or(Error::Invalid(
                        "private output allocation effect range changed",
                    ))?
            else {
                return Err(Error::Invalid(
                    "private output allocation must have one exact effect",
                ));
            };
            if effect.coordinate.operation != operation.coordinate
                || effect.coordinate.effect != 0
                || !matches!(
                    effect.effect,
                    fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Allocate(AddressSpace::Private)
                )
            {
                return Err(Error::Invalid("private output allocation effect changed"));
            }
            allocations = allocations
                .checked_add(1)
                .ok_or_else(private_array_output_arithmetic_v1)?;
            effects = effects
                .checked_add(1)
                .ok_or_else(private_array_output_arithmetic_v1)?;
            continue;
        }
        let OperationKind::Store { value, .. } = operation.operation.kind else {
            if !operation.effects.is_empty() {
                return Err(Error::Invalid(
                    "private output non-Store memory effect is outside the closed subset",
                ));
            }
            continue;
        };
        work.charge_private_array_work(3)?;
        let key = private_array_output_coordinate_key_v1(operation.coordinate);
        let found =
            private_array_binary_search_v1(&workspace.stores, |row| row.key, key, &mut work)?
                .map_err(|_| Error::Invalid("private output unclaimed physical Store"))?;
        work.charge_private_array_work(16)?;
        let fact = workspace
            .facts
            .get(workspace.stores[found].fact)
            .ok_or(Error::Invalid("private output Store fact is absent"))?;
        let output = fact
            .output
            .as_ref()
            .ok_or(Error::Invalid("private output omitted Store was claimed"))?;
        let [effect] = inventory
            .effects()
            .get(operation.effects.clone())
            .ok_or(Error::Invalid("private output Store effect range changed"))?
        else {
            return Err(Error::Invalid(
                "private output Store must have one exact effect",
            ));
        };
        if output.access.operation != operation.coordinate
            || output.access.effect != 0
            || effect.coordinate != output.access
            || output.value != value
            || !matches!(
                effect.effect,
                fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Write(AddressSpace::Private)
            )
        {
            return Err(Error::Invalid(
                "private output Store operand or effect identity changed",
            ));
        }
        stores = stores
            .checked_add(1)
            .ok_or_else(private_array_output_arithmetic_v1)?;
        effects = effects
            .checked_add(1)
            .ok_or_else(private_array_output_arithmetic_v1)?;
    }
    work.charge_private_array_work(3)?;
    if stores != workspace.stores.len()
        || effects != inventory.effects().len()
        || allocations != distinct_allocations
    {
        return Err(Error::Invalid(
            "private output reverse Store census is incomplete",
        ));
    }
    Ok(())
}
