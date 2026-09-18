use super::*;

#[derive(Clone, Copy)]
struct Address {
    allocation: usize,
    start: usize,
    length: usize,
    offset: usize,
    alignment: u32,
    stride: usize,
}

/// A temporary census of this exact borrowed inventory, not a transferable
/// private-memory certificate. Source/N and optimizer equivalence are separate
/// prerequisites of the enclosing consuming admission transaction.
pub(super) struct PrivateMemory<'a, 'g> {
    inventory: &'a CanonicalKirInventoryV1<'g>,
    definitions: Vec<Option<Address>>,
    operations: Vec<bool>,
    latest_stores: Vec<Option<usize>>,
}

impl PrivateMemory<'_, '_> {
    pub(super) fn definition(&self, index: usize) -> bool {
        self.definitions.get(index).is_some_and(Option::is_some)
    }
    pub(super) fn operation(&self, index: usize) -> bool {
        self.operations.get(index).copied().unwrap_or(false)
    }
    pub(super) fn is_for(&self, inventory: &CanonicalKirInventoryV1<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
}

fn is_private(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private)
}

fn index(
    inventory: &CanonicalKirInventoryV1<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    value: ValueId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<usize> {
    inventory
        .definition_index_for_value(function, value, budget)
        .map_err(inventory_error)?
        .ok_or_else(|| refused("private", "exact function-local definition"))
}

pub(super) fn check<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV1<'g>,
    max_cells: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<PrivateMemory<'a, 'g>> {
    charge(budget, 2)?;
    budget
        .reserve_storage(std::mem::size_of::<&CanonicalKirInventoryV1<'_>>())
        .map_err(E::Resource)?;
    let mut constants = scratch::<Option<u64>>(inventory.definitions().len(), budget)?;
    let mut addresses = scratch::<Option<Address>>(inventory.definitions().len(), budget)?;
    let mut operations = scratch::<bool>(inventory.operations().len(), budget)?;
    let mut latest_stores = scratch::<Option<usize>>(inventory.operations().len(), budget)?;
    charge(
        budget,
        inventory
            .definitions()
            .len()
            .checked_mul(2)
            .and_then(|n| {
                inventory
                    .operations()
                    .len()
                    .checked_mul(2)
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(arithmetic)?,
    )?;
    constants.resize(inventory.definitions().len(), None);
    addresses.resize(inventory.definitions().len(), None);
    operations.resize(inventory.operations().len(), false);
    latest_stores.resize(inventory.operations().len(), None);
    for row in inventory.operations() {
        charge(budget, 2)?;
        if let OperationKind::Constant(Constant::Index(value)) = row.operation.kind
            && row.results.len() == 1
        {
            constants[row.results.start] = Some(value);
        }
    }
    let mut cells = 0usize;
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(budget, 3)?;
        let OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } = &row.operation.kind
        else {
            continue;
        };
        if *address_space != AddressSpace::Private
            || !matches!(element, Type::Scalar(_))
            || *alignment == 0
            || row.results.len() != 1
            || row.effects.len() != 1
        {
            return Err(refused("private", "one exact scalar private allocation"));
        }
        charge(budget, 3)?;
        let Type::Scalar(scalar) = element else {
            unreachable!()
        };
        let stride = usize::from(
            scalar
                .bit_width()
                .ok_or_else(|| refused("private", "fixed-width scalar allocation layout"))?
                .div_ceil(8),
        );
        let length = match count {
            None => 1,
            Some(count) => {
                let definition = index(inventory, row.coordinate.block.function, *count, budget)?;
                usize::try_from(
                    constants[definition]
                        .ok_or_else(|| refused("private", "constant allocation extent"))?,
                )
                .map_err(|_| arithmetic())?
            }
        };
        charge(budget, 5)?;
        let end = cells.checked_add(length).ok_or_else(arithmetic)?;
        if length == 0 || end > max_cells {
            return Err(refused("private", "bounded nonzero allocation extent"));
        }
        charge(budget, 1)?;
        length.checked_mul(stride).ok_or_else(arithmetic)?;
        if !matches!(
            inventory.effects()[row.effects.start].effect,
            fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Allocate(AddressSpace::Private)
        ) {
            return Err(refused("private", "exact allocation effect"));
        }
        addresses[row.results.start] = Some(Address {
            allocation: ordinal,
            start: cells,
            length,
            offset: 0,
            alignment: *alignment,
            stride,
        });
        operations[ordinal] = true;
        cells = end;
    }
    // Only direct constant element addresses are admitted. In particular phi,
    // pointer casts, integer-derived pointers and nested/dynamic GEPs do not
    // acquire provenance by sharing a numeric address or a private type.
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(
            budget,
            row.results.len().checked_add(2).ok_or_else(arithmetic)?,
        )?;
        if !row
            .results
            .clone()
            .any(|i| is_private(inventory.definitions()[i].ty))
        {
            continue;
        }
        if matches!(row.operation.kind, OperationKind::Alloca { .. }) {
            continue;
        }
        let OperationKind::GetElementPointer { base, offset } = row.operation.kind else {
            return Err(refused(
                "private",
                "direct allocation or constant element address",
            ));
        };
        let base_index = index(inventory, row.coordinate.block.function, base, budget)?;
        let offset_index = index(inventory, row.coordinate.block.function, offset, budget)?;
        charge(budget, 5)?;
        let base =
            addresses[base_index].ok_or_else(|| refused("private", "known allocation base"))?;
        let allocation = &inventory.operations()[base.allocation];
        if base.offset != 0 || allocation.results.start != base_index || row.results.len() != 1 {
            return Err(refused("private", "direct allocation base only"));
        }
        let offset = usize::try_from(
            constants[offset_index]
                .ok_or_else(|| refused("private", "constant exact element offset"))?,
        )
        .map_err(|_| arithmetic())?;
        if offset >= base.length {
            return Err(refused("private", "element offset within allocation"));
        }
        addresses[row.results.start] = Some(Address { offset, ..base });
        operations[ordinal] = true;
    }
    for (ordinal, definition) in inventory.definitions().iter().enumerate() {
        charge(budget, 2)?;
        if is_private(definition.ty) && addresses[ordinal].is_none() {
            return Err(refused(
                "private",
                "no private parameters or transported unknown pointers",
            ));
        }
    }
    let mut latest = scratch::<Option<usize>>(cells, budget)?;
    charge(budget, cells)?;
    latest.resize(cells, None);
    for block in inventory.blocks() {
        charge(budget, cells.checked_add(1).ok_or_else(arithmetic)?)?;
        latest.fill(None);
        for ordinal in block.operations.clone() {
            let row = &inventory.operations()[ordinal];
            charge(budget, 3)?;
            let memory = match row.operation.kind {
                OperationKind::Load { pointer, access }
                    if access.address_space == AddressSpace::Private =>
                {
                    Some((pointer, access, false))
                }
                OperationKind::Store {
                    pointer, access, ..
                } if access.address_space == AddressSpace::Private => Some((pointer, access, true)),
                _ => None,
            };
            if let Some((pointer, access, write)) = memory {
                let definition = index(inventory, row.coordinate.block.function, pointer, budget)?;
                charge(budget, 6)?;
                let address = addresses[definition]
                    .ok_or_else(|| refused("private", "known memory address"))?;
                if access.volatile || access.alignment == 0 || row.effects.len() != 1 {
                    return Err(refused("private", "one ordinary nonvolatile memory effect"));
                }
                charge(budget, 4)?;
                let byte_offset = address
                    .offset
                    .checked_mul(address.stride)
                    .ok_or_else(arithmetic)?;
                if access.alignment > address.alignment
                    || byte_offset % access.alignment as usize != 0
                {
                    return Err(refused(
                        "private",
                        "access alignment follows allocation and element offset",
                    ));
                }
                if !matches!(
                    (write, inventory.effects()[row.effects.start].effect),
                    (
                        true,
                        fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Write(AddressSpace::Private)
                    ) | (
                        false,
                        fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Read(AddressSpace::Private)
                    )
                ) {
                    return Err(refused("private", "exact memory effect"));
                }
                let cell = address
                    .start
                    .checked_add(address.offset)
                    .ok_or_else(arithmetic)?;
                if write {
                    latest[cell] = Some(ordinal);
                } else if latest[cell].is_none() {
                    return Err(refused(
                        "private",
                        "Load requires a same-block latest Store",
                    ));
                } else {
                    latest_stores[ordinal] = latest[cell];
                }
                operations[ordinal] = true;
            }
            for operand in &inventory.uses()[row.operands.clone()] {
                charge(budget, 3)?;
                if addresses[operand.definition].is_none() {
                    continue;
                }
                let permitted = match row.operation.kind {
                    OperationKind::GetElementPointer { base, .. } => {
                        base == operand.value && operations[ordinal]
                    }
                    OperationKind::Load { pointer, .. } => {
                        pointer == operand.value && operations[ordinal]
                    }
                    OperationKind::Store { pointer, value, .. } => {
                        pointer == operand.value && value != operand.value && operations[ordinal]
                    }
                    _ => false,
                };
                if !permitted {
                    return Err(refused("private", "private pointer does not escape"));
                }
            }
        }
        for operand in &inventory.uses()[block.terminator_uses.clone()] {
            charge(budget, 2)?;
            if addresses[operand.definition].is_some() {
                return Err(refused("private", "no private pointer control transport"));
            }
        }
    }
    Ok(PrivateMemory {
        inventory,
        definitions: addresses,
        operations,
        latest_stores,
    })
}

#[derive(Clone, Copy)]
struct SourceKillSite {
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    local: SemanticLocalIdV1,
    statement: u32,
}

impl SourceKillSite {
    fn key(self) -> [u32; 4] {
        [
            self.function.index(),
            self.block.index(),
            self.local.index(),
            self.statement,
        ]
    }
}

fn visit_statement_kills(
    kind: &SemanticStatementKindV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    mut visit: impl FnMut(SemanticLocalIdV1, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    charge(budget, 2)?;
    match kind {
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => visit(*local, budget),
        SemanticStatementKindV1::Deinitialize(place) => visit(place.local(), budget),
        SemanticStatementKindV1::Assign(assignment) => {
            private_array_visit_rvalue_operands_v1(assignment.value().kind(), |operand, _| {
                charge(budget, 1)?;
                if let SemanticOperandV1::Move(place) = operand {
                    visit(place.local(), budget)?;
                }
                Ok(())
            })
        }
        SemanticStatementKindV1::Store(store) => {
            charge(budget, 1)?;
            if let SemanticOperandV1::Move(place) = store.value() {
                visit(place.local(), budget)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn visit_source_kills(
    source: &AdmittedInertSemanticMirV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    mut visit: impl FnMut(SourceKillSite, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    for (function, declaration) in source.functions().iter().enumerate() {
        charge(budget, 3)?;
        let function =
            SemanticFunctionIdV1::from_index(u32::try_from(function).map_err(|_| arithmetic())?);
        for (block, body) in declaration.blocks().iter().enumerate() {
            charge(budget, 3)?;
            let block =
                SemanticBlockIdV1::from_index(u32::try_from(block).map_err(|_| arithmetic())?);
            for (statement, value) in body.statements().iter().enumerate() {
                charge(budget, 3)?;
                let statement = u32::try_from(statement).map_err(|_| arithmetic())?;
                visit_statement_kills(value.kind(), budget, |local, budget| {
                    visit(
                        SourceKillSite {
                            function,
                            block,
                            local,
                            statement,
                        },
                        budget,
                    )
                })?;
            }
        }
    }
    Ok(())
}

// Sparse syntax-only kills share the immutable semantic owner, while each
// Store/Load keeps its exact root-qualified physical/source span join below.
// Duplicate Move occurrences are harmless. The legacy private-array ranked
// relation still has its separate per-Read effect and interval scans.
fn source_kill_index(
    source: &AdmittedInertSemanticMirV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<SourceKillSite>> {
    let mut count = 0usize;
    visit_source_kills(source, budget, |_, budget| {
        charge(budget, 2)?;
        count = count.checked_add(1).ok_or_else(arithmetic)?;
        Ok(())
    })?;
    let mut rows = scratch::<SourceKillSite>(count, budget)?;
    visit_source_kills(source, budget, |site, budget| {
        charge(budget, 3)?;
        if rows.len() == count {
            return Err(arithmetic());
        }
        rows.push(site);
        Ok(())
    })?;
    charge(budget, 1)?;
    if rows.len() != count {
        return Err(arithmetic());
    }
    sort_source_kills(&mut rows, budget)?;
    Ok(rows)
}

fn sort_source_kills(rows: &mut [SourceKillSite], budget: &mut AssertOriginBudgetV1<'_>) -> R<()> {
    assert_origin_sort_v1(rows, budget, |left, right, budget| {
        budget.charge_work(4)?;
        Ok(left.key().cmp(&right.key()))
    })
    .map_err(|error| E::SourceOutput(ProductionSourceOutputErrorV1::SourceOrigin(error)))
}

fn source_interval_is_killed(
    rows: &[SourceKillSite],
    first: SourceKillSite,
    last: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 2)?;
    let target = first.key();
    let (mut start, mut end) = (0, rows.len());
    while start < end {
        charge(budget, 6)?;
        let middle = start + (end - start) / 2;
        if rows[middle].key() < target {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    charge(budget, 5)?;
    Ok(rows.get(start).is_some_and(|row| {
        (row.function, row.block, row.local) == (first.function, first.block, first.local)
            && row.statement <= last
    }))
}

fn record_source_statement(
    site: &mut Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>,
    incoming: (SemanticFunctionIdV1, SemanticBlockIdV1, u32),
) -> R<()> {
    // Root-qualified occurrences of one retained helper share its physical
    // instructions. Complete source/N replay establishes their common body.
    if site.is_some_and(|previous| previous != incoming) {
        return Err(refused("private source", "unique source statement span"));
    }
    *site = Some(incoming);
    Ok(())
}

// Physical Store/Load validity does not resurrect a source lifetime killed by
// a statement that emits no memory instruction. This initial source rule is
// deliberately conservative for Moves and partial deinitialization.
pub(super) fn source_lifetimes(
    source: &ProductionSemanticKirOwnerV1,
    proof: &PrivateMemory<'_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let sites = source_statement_sites_v1(source, proof.inventory, budget)?;
    source_lifetimes_from_sites(source.semantic().semantic(), proof, &sites, budget)
}

pub(super) fn source_statement_sites_v1(
    source: &ProductionSemanticKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>>> {
    let mut sites = scratch::<Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>>(
        inventory.operations().len(),
        budget,
    )?;
    charge(budget, inventory.operations().len())?;
    sites.resize(inventory.operations().len(), None);
    let origins = source
        .pre_ranked_assert_origins()
        .ok_or_else(|| refused("private source", "sealed source function mapping"))?;
    for span in source.correspondence.statement_operation_spans() {
        charge(budget, 3)?;
        let found = assert_origin_find_v1(&origins.origins.functions, budget, |entry, budget| {
            budget.charge_work(2)?;
            Ok((entry.owner, entry.function)
                .cmp(&(span.correspondence_owner(), span.semantic_function())))
        })
        .map_err(|e| E::SourceOutput(ProductionSourceOutputErrorV1::SourceOrigin(e)))?
        .ok_or_else(|| refused("private source", "exact source function"))?;
        let block = inventory
            .block_for_id(
                origins.origins.functions[found].canonical,
                span.kernel_ir_block(),
                budget,
            )
            .map_err(inventory_error)?
            .ok_or_else(|| refused("private source", "exact statement block"))?;
        let start = block
            .operations
            .start
            .checked_add(span.first_operation_ordinal() as usize)
            .ok_or_else(arithmetic)?;
        let end = start
            .checked_add(span.operation_count() as usize)
            .ok_or_else(arithmetic)?;
        if end > block.operations.end {
            return Err(refused("private source", "bounded statement span"));
        }
        for site in &mut sites[start..end] {
            charge(budget, 2)?;
            record_source_statement(
                site,
                (
                    span.semantic_function(),
                    span.semantic_block(),
                    span.statement_ordinal(),
                ),
            )?;
        }
    }
    Ok(sites)
}

pub(super) fn source_lifetimes_from_sites(
    semantic: &AdmittedInertSemanticMirV1,
    proof: &PrivateMemory<'_, '_>,
    sites: &[Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let mut kills = None;
    for (read, store) in proof.latest_stores.iter().enumerate() {
        charge(budget, 4)?;
        let Some(store) = store else {
            continue;
        };
        let Some((function, block, first)) = sites[*store] else {
            return Err(refused(
                "private source",
                "initializing Store has an actual source statement",
            ));
        };
        let Some((read_function, read_block, last)) = sites[read] else {
            return Err(refused(
                "private source",
                "Load has an actual source statement",
            ));
        };
        if (function, block) != (read_function, read_block) || first > last {
            return Err(refused(
                "private source",
                "same-block ordered source Store/Load",
            ));
        }
        let function_id = function;
        let function = semantic
            .functions()
            .get(function.index() as usize)
            .ok_or_else(|| refused("private source", "source function exists"))?;
        let statements = function
            .blocks()
            .get(block.index() as usize)
            .ok_or_else(|| refused("private source", "source block exists"))?
            .statements();
        charge(budget, 2)?;
        let destination = match statements
            .get(first as usize)
            .map(|statement| statement.kind())
        {
            Some(SemanticStatementKindV1::Assign(assignment)) => assignment.destination(),
            Some(SemanticStatementKindV1::Store(store)) => store.destination(),
            _ => {
                return Err(refused(
                    "private source",
                    "initializing source assignment or Store",
                ));
            }
        };
        charge(budget, 3)?;
        let declaration = function
            .locals()
            .get(destination.local().index() as usize)
            .and_then(|local| semantic.types().get(local.ty().index() as usize))
            .ok_or_else(|| refused("private source", "source storage declaration"))?;
        let local = source_storage_local(destination, declaration.shape(), budget)?;
        if last as usize >= statements.len() {
            return Err(refused("private source", "source statement interval"));
        }
        if kills.is_none() {
            kills = Some(source_kill_index(semantic, budget)?);
        }
        if source_interval_is_killed(
            kills.as_deref().unwrap(),
            SourceKillSite {
                function: function_id,
                block,
                local,
                statement: first,
            },
            last,
            budget,
        )? {
            return Err(refused(
                "private source",
                "no source lifetime or Move invalidation between Store and Load",
            ));
        }
    }
    Ok(())
}

fn source_storage_local(
    destination: &SemanticPlaceV1,
    declared: &SemanticTypeShapeV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<SemanticLocalIdV1> {
    charge(budget, 4)?;
    let direct = match (declared, destination.projections()) {
        (SemanticTypeShapeV1::Scalar(_), []) | (SemanticTypeShapeV1::Array { .. }, []) => true,
        (SemanticTypeShapeV1::Array { element, .. }, [projection]) => {
            projection.result_type() == *element
                && matches!(
                    projection.kind(),
                    SemanticProjectionKindV1::Index(_)
                        | SemanticProjectionKindV1::ConstantIndex { .. }
                )
        }
        _ => false,
    };
    if !direct {
        return Err(refused(
            "private source",
            "direct scalar local or fixed-array element destination",
        ));
    }
    Ok(destination.local())
}

include!("production_checked_output_erased_private_lifetimes_v1.rs");
include!("production_checked_output_redundant_store_lifetimes_v1.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{AccessMode, BasicBlock, MemoryAccess, Signature, ValueDef};
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAssignmentV1, SemanticMemoryStoreV1, SemanticRvalueV1,
    };

    #[test]
    fn shared_helper_source_sites_coalesce_only_identical_statement_coordinates() {
        let original = (
            SemanticFunctionIdV1::from_index(2),
            SemanticBlockIdV1::from_index(3),
            4,
        );
        let mut site = None;
        record_source_statement(&mut site, original).unwrap();
        record_source_statement(&mut site, original).unwrap();
        assert_eq!(site, Some(original));
        for conflicting in [
            (SemanticFunctionIdV1::from_index(1), original.1, original.2),
            (original.0, SemanticBlockIdV1::from_index(2), original.2),
            (original.0, original.1, 3),
        ] {
            assert!(matches!(
                record_source_statement(&mut site, conflicting),
                Err(E::Unsupported {
                    phase: "private source",
                    detail: "unique source statement span",
                })
            ));
            assert_eq!(site, Some(original));
        }
    }

    // These are independently verified native hostile components, not claimed
    // source-owned positive fixtures. The sibling general tests own that lane.
    fn component() -> Module {
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        );
        let mut block = BasicBlock::new(BlockId(41));
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(0), Type::Scalar(ScalarType::Index)),
                OperationKind::Constant(Constant::Index(2)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), pointer.clone()),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: Some(ValueId(0)),
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::Scalar(ScalarType::Index)),
                OperationKind::Constant(Constant::Index(0)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(1),
                    offset: ValueId(2),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(99)),
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(3),
                    value: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
                OperationKind::Load {
                    pointer: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ];
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(5)],
        });
        let mut module = Module::new("private-memory-census-component");
        module.functions.push(Function::internal_helper(
            "f",
            Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
            vec![],
            vec![block],
        ));
        module
    }

    fn check_component(module: Module, expected: Option<&'static str>) {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 16 * 1024 * 1024);
        let (owner, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        let result = check(&inventory, 1024, &mut budget);
        match expected {
            None => {
                let proof = result.unwrap();
                assert!(proof.is_for(&inventory));
                assert!(
                    proof.operation(1)
                        && proof.operation(3)
                        && proof.operation(5)
                        && proof.operation(6)
                );
                drop(proof);
            }
            Some(expected) => assert!(
                matches!(result, Err(E::Unsupported { phase: "private", detail }) if detail == expected)
            ),
        }
        // This worker transfers its scratch to the enclosing transaction, which
        // drops all indices before releasing it (covered by admission tests).
        assert!(budget.storage() > floor);
        budget.release_storage(budget.storage() - floor).unwrap();
    }

    #[test]
    fn private_census_checks_independently_verified_store_load_and_latest_cell() {
        check_component(component(), None);
        let mut uninitialized = component();
        uninitialized.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .swap(5, 6);
        check_component(
            uninitialized,
            Some("Load requires a same-block latest Store"),
        );
        let mut other_cell = component();
        let operations = &mut other_cell.functions[0].body.as_mut().unwrap().blocks[0].operations;
        operations[2].kind = OperationKind::Constant(Constant::Index(1));
        let OperationKind::Store { pointer, .. } = &mut operations[5].kind else {
            unreachable!()
        };
        *pointer = ValueId(1);
        check_component(other_cell, Some("Load requires a same-block latest Store"));
    }

    #[test]
    fn private_census_rejects_out_of_bounds_and_pointer_escape() {
        let mut outside = component();
        outside.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind =
            OperationKind::Constant(Constant::Index(2));
        check_component(outside, Some("element offset within allocation"));
        let mut escaped = component();
        escaped.functions[0].signature = Signature::new(
            vec![],
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )],
        );
        escaped.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::Return {
                values: vec![ValueId(3)],
            });
        check_component(escaped, Some("no private pointer control transport"));
    }

    #[test]
    fn private_census_refuses_dynamic_address_and_cross_block_initialization() {
        let mut dynamic = component();
        dynamic.functions[0]
            .signature
            .parameters
            .push(Type::Scalar(ScalarType::Index));
        let body = dynamic.functions[0].body.as_mut().unwrap();
        body.parameters.push(ValueId(9));
        body.blocks[0].operations[3].kind = OperationKind::GetElementPointer {
            base: ValueId(1),
            offset: ValueId(9),
        };
        check_component(dynamic, Some("constant exact element offset"));
        let mut crossing = component();
        let body = crossing.functions[0].body.as_mut().unwrap();
        let mut second = BasicBlock::new(BlockId(77));
        second.operations = body.blocks[0].operations.split_off(6);
        second.terminator = body.blocks[0].terminator.take();
        body.blocks[0].terminator = Some(Terminator::Branch {
            target: second.id,
            arguments: vec![],
        });
        body.blocks.push(second);
        check_component(crossing, Some("Load requires a same-block latest Store"));
    }

    #[test]
    fn private_census_rejects_unproved_base_and_element_alignment() {
        for (base, offset) in [(4, 0), (8, 1)] {
            let mut module = component();
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            let OperationKind::Alloca { alignment, .. } = &mut operations[1].kind else {
                unreachable!()
            };
            *alignment = base;
            operations[2].kind = OperationKind::Constant(Constant::Index(offset));
            for ordinal in [5, 6] {
                let (OperationKind::Load { access, .. } | OperationKind::Store { access, .. }) =
                    &mut operations[ordinal].kind
                else {
                    unreachable!()
                };
                access.alignment = 8;
            }
            check_component(
                module,
                Some("access alignment follows allocation and element offset"),
            );
        }
    }

    #[test]
    fn private_census_entry_work_and_header_storage_boundaries_are_paid() {
        let mut preparation = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut preparation, 16 * 1024 * 1024);
        let (owner, os) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&component(), &mut budget).unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        let mut short_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
        let mut short = AssertOriginBudgetV1::new(&mut short_work, floor + 1024);
        short.reserve_storage(floor).unwrap();
        assert!(matches!(
            check(&inventory, 1024, &mut short),
            Err(E::Resource(_))
        ));
        assert_eq!(short.work(), 0);
        assert_eq!(short.storage(), floor);
        let mut exact_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(2);
        let mut short = AssertOriginBudgetV1::new(
            &mut exact_work,
            floor + std::mem::size_of::<&CanonicalKirInventoryV1<'_>>() - 1,
        );
        short.reserve_storage(floor).unwrap();
        assert!(matches!(
            check(&inventory, 1024, &mut short),
            Err(E::Resource(_))
        ));
        assert_eq!(short.work(), 2);
        assert_eq!(short.storage(), floor);
    }

    #[test]
    fn private_source_lifetime_key_rejects_indirect_destination_components() {
        let ty = SemanticTypeIdV1::from_index(1);
        let shape = SemanticTypeShapeV1::Array {
            element: ty,
            length: 8,
        };
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1024);
        let local = SemanticLocalIdV1::from_index(2);
        let direct = SemanticPlaceV1::new(
            local,
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 8,
                        from_end: false,
                    },
                    ty,
                )
                .unwrap(),
            ],
            ty,
        )
        .unwrap();
        assert_eq!(
            source_storage_local(&direct, &shape, &mut budget).unwrap(),
            local
        );
        let indirect = SemanticPlaceV1::new(
            local,
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty).unwrap()],
            ty,
        )
        .unwrap();
        assert!(matches!(
            source_storage_local(&indirect, &shape, &mut budget),
            Err(E::Unsupported {
                phase: "private source",
                detail: "direct scalar local or fixed-array element destination"
            })
        ));
        let nested = SemanticPlaceV1::new(
            local,
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty).unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 8,
                        from_end: false,
                    },
                    ty,
                )
                .unwrap(),
            ],
            ty,
        )
        .unwrap();
        assert!(source_storage_local(&nested, &shape, &mut budget).is_err());
        assert_eq!(budget.work(), 12);
        assert_eq!(budget.storage(), 0);
    }

    fn kill(statement: u32) -> SourceKillSite {
        SourceKillSite {
            function: SemanticFunctionIdV1::from_index(1),
            block: SemanticBlockIdV1::from_index(2),
            local: SemanticLocalIdV1::from_index(3),
            statement,
        }
    }

    #[test]
    fn private_kill_index_preserves_inclusive_intervals_and_qualified_keys() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1024);
        let mut rows = [kill(9), kill(4), kill(9)];
        sort_source_kills(&mut rows, &mut budget).unwrap();
        for _ in 0..8 {
            for (first, last, expected) in [
                (4, 4, true),
                (1, 4, true),
                (9, 12, true),
                (5, 8, false),
                (10, 12, false),
            ] {
                assert_eq!(
                    source_interval_is_killed(&rows, kill(first), last, &mut budget).unwrap(),
                    expected
                );
            }
        }
        for foreign in [
            SourceKillSite {
                function: SemanticFunctionIdV1::from_index(0),
                ..kill(4)
            },
            SourceKillSite {
                block: SemanticBlockIdV1::from_index(0),
                ..kill(4)
            },
            SourceKillSite {
                local: SemanticLocalIdV1::from_index(0),
                ..kill(4)
            },
        ] {
            assert!(!source_interval_is_killed(&rows, foreign, 12, &mut budget).unwrap());
        }
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn private_kill_index_collects_the_existing_source_invalidation_grammar() {
        let ty = SemanticTypeIdV1::from_index(1);
        let local = kill(0).local;
        let place = SemanticPlaceV1::new(local, vec![], ty).unwrap();
        let kinds = [
            SemanticStatementKindV1::StorageLive(local),
            SemanticStatementKindV1::StorageDead(local),
            SemanticStatementKindV1::Deinitialize(place.clone()),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place.clone(),
                SemanticRvalueV1::new(
                    ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place.clone())),
                ),
            )),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place.clone(),
                SemanticOperandV1::Move(place.clone()),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place.clone(),
                SemanticRvalueV1::new(
                    ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)),
                ),
            )),
            SemanticStatementKindV1::Nop,
        ];
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1024);
        let mut count = 0;
        for kind in &kinds {
            visit_statement_kills(kind, &mut budget, |found, _| {
                assert_eq!(found, local);
                count += 1;
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(count, 5);
        assert_eq!(budget.work(), 17);
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn private_kill_index_search_charges_exact_and_one_short_work() {
        for limit in [12, 13] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1024);
            let result = source_interval_is_killed(&[kill(4)], kill(4), 4, &mut budget);
            if limit == 13 {
                assert!(result.unwrap());
                assert_eq!(budget.work(), 13);
            } else {
                assert!(matches!(result, Err(E::Resource(_))));
                assert_eq!(budget.work(), 8);
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}
