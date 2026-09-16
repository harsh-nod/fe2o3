// Indexed atomics retain their checked slice origin independently of ordinary borrows.
use fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1;
include!("indexed_atomic_dead_cast_v1.rs");

#[derive(Clone, Debug)]
struct IndexedAtomicBorrowOriginV1 {
    place: SemanticPlaceV1,
    guard: ProjectedBoundsCheckV1,
    borrow: ScalarAssignmentSiteV1,
}

#[derive(Clone, Debug)]
struct IndexedAtomicUseV1 {
    block: usize,
    statement: usize,
    access: AccessKindAttr,
    address: SemanticPlaceV1,
    atomic: SemanticAtomicAccessV1,
    origin: IndexedAtomicBorrowOriginV1,
}

#[derive(Clone, Debug, Default)]
struct AuthenticatedAtomicAllocationsV1 {
    indexed_uses: Vec<IndexedAtomicUseV1>,
    indexed_borrows: Vec<IndexedAtomicBorrowOriginV1>,
    indexed_locals: Vec<SemanticLocalIdV1>,
    coherent: Vec<u64>,
    atomic_only_roots: Vec<u64>,
    work: std::cell::Cell<usize>,
}

impl AuthenticatedAtomicAllocationsV1 {
    fn charge(&self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        let mut work = self.work.get();
        project_loop_graph_charge_v1(&mut work, amount)?;
        self.work.set(work);
        Ok(())
    }

    fn authorizes_bounds(
        &self,
        check: ProjectedBoundsCheckV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.charge(self.indexed_uses.len())?;
        Ok(self
            .indexed_uses
            .iter()
            .any(|usage| usage.origin.guard == check))
    }

    fn contains_local(
        &self,
        local: SemanticLocalIdV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.charge(self.indexed_locals.len())?;
        Ok(self.indexed_locals.contains(&local))
    }

    fn atomic_only(&self, allocation: u64) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.charge(self.atomic_only_roots.len())?;
        Ok(self.atomic_only_roots.contains(&allocation))
    }

    fn permits_address(
        &self,
        block: usize,
        place: &SemanticPlaceV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.charge(
            self.indexed_borrows
                .len()
                .saturating_mul(place.projections().len() + 1),
        )?;
        Ok(self
            .indexed_borrows
            .iter()
            .any(|origin| origin.borrow.block == block && origin.place == *place))
    }

    fn validate_statement(
        &self,
        block: usize,
        statement: usize,
        record: &SemanticStatementV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.charge(self.indexed_uses.len())?;
        let expected = self
            .indexed_uses
            .iter()
            .find(|usage| usage.block == block && usage.statement == statement);
        if let Some(expected) = expected {
            let Some((address, atomic, access)) = indexed_atomic_effect_v1(record) else {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an indexed atomic use lost its exact semantic statement",
                ));
            };
            if expected.address != *address
                || expected.atomic != atomic
                || expected.access != access
            {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an indexed atomic statement changed its exact address, kind, ordering, or scope",
                ));
            }
        }
        Ok(())
    }

    fn usage(
        &self,
        block: usize,
        place: &SemanticPlaceV1,
        access: AccessKindAttr,
        atomic: SemanticAtomicAccessV1,
    ) -> Result<&IndexedAtomicUseV1, ProductionRankedProjectionErrorV1> {
        self.charge(
            self.indexed_uses
                .len()
                .saturating_mul(place.projections().len() + 1),
        )?;
        let mut matches = self.indexed_uses.iter().filter(|usage| {
            usage.block == block
                && usage.address == *place
                && usage.access == access
                && usage.atomic == atomic
        });
        let first = matches
            .next()
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "an indexed atomic effect changed its authenticated address, kind, or ordering",
            ))?;
        if matches.any(|usage| {
            usage.origin.place != first.origin.place || usage.origin.guard != first.origin.guard
        }) {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "indexed atomic uses disagree on their retained origin",
            ));
        }
        Ok(first)
    }

    fn authorizes_root(
        &self,
        block: usize,
        place: &SemanticPlaceV1,
        access: AccessKindAttr,
        atomic: SemanticAtomicAccessV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.charge(
            self.indexed_uses
                .len()
                .saturating_mul(place.projections().len() + 1),
        )?;
        Ok(atomic.scope() == SemanticAtomicScopeV1::System
            && self.indexed_uses.iter().any(|usage| {
                usage.origin.place == *place
                    && usage.origin.guard.access_block == block
                    && usage.access == access
                    && usage.atomic == atomic
            }))
    }
}

fn indexed_atomic_effect_v1(
    record: &SemanticStatementV1,
) -> Option<(&SemanticPlaceV1, SemanticAtomicAccessV1, AccessKindAttr)> {
    match record.kind() {
        SemanticStatementKindV1::Store(store) => store
            .atomic()
            .map(|atomic| (store.destination(), atomic, AccessKindAttr::AtomicWrite)),
        SemanticStatementKindV1::Assign(assignment) => match assignment.value().kind() {
            SemanticRvalueKindV1::Load(load) => load
                .atomic()
                .map(|atomic| (load.source(), atomic, AccessKindAttr::AtomicRead)),
            _ => None,
        },
        SemanticStatementKindV1::AtomicRmw(atomic) => Some((
            atomic.address(),
            atomic.access(),
            AccessKindAttr::AtomicReadModifyWrite,
        )),
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => Some((
            atomic.address(),
            atomic.success(),
            AccessKindAttr::AtomicReadModifyWrite,
        )),
        _ => None,
    }
}

// A single static assignment inside a cycle is not dynamically immutable.
// This first indexed route accepts only acyclic pointer/index definitions.
fn indexed_atomic_block_acyclic_v1(
    proofs: &mut SemanticAssertProofsV1<'_>,
    definition: usize,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let count = proofs.graph.successors.len();
    proofs.charge(count)?;
    let mut seen = vec![false; count];
    let mut pending = VecDeque::new();
    for next in proofs.graph.successors[definition].clone() {
        proofs.charge(1)?;
        if next == definition {
            return Ok(false);
        }
        if !seen[next] {
            seen[next] = true;
            pending.push_back(next);
        }
    }
    while let Some(block) = pending.pop_front() {
        proofs.charge(1)?;
        for next in proofs.graph.successors[block].clone() {
            proofs.charge(1)?;
            if next == definition {
                return Ok(false);
            }
            if !seen[next] {
                seen[next] = true;
                pending.push_back(next);
            }
        }
    }
    Ok(true)
}

fn indexed_atomic_index_stable_v1(
    proofs: &mut SemanticAssertProofsV1<'_>,
    local: SemanticLocalIdV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let index = local.index() as usize;
    if proofs.address_escaped.get(index).copied() != Some(false) {
        return Ok(false);
    }
    let count = proofs.definition_counts.get(index).copied();
    if matches!(
        proofs
            .function
            .locals()
            .get(index)
            .map(|local| local.role()),
        Some(SemanticLocalRoleV1::Argument(_))
    ) {
        return Ok(count == Some(0));
    }
    if count != Some(1) {
        return Ok(false);
    }
    // The definition inventory includes Call destinations as well as assignments.
    // An absent assignment record must not imply dynamic stability.
    let mut definition = None;
    for block in 0..proofs.block_definitions.len() {
        proofs.charge(proofs.block_definitions[block].len() + 1)?;
        if proofs.block_definitions[block].contains(&index) && definition.replace(block).is_some() {
            return Ok(false);
        }
    }
    let Some(definition) = definition else {
        return Ok(false);
    };
    indexed_atomic_block_acyclic_v1(proofs, definition)
}

fn core_atomic_u32_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|ty| {
        ty.rust_type_kind()
            == fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::CoreAtomicU32
            && matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
            && ty.layout().size_bytes() == Some(4)
            && ty.layout().alignment_bytes() == 4
    })
}

fn atomic_slice_argument_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    local: SemanticLocalIdV1,
) -> Option<SemanticTypeIdV1> {
    let declaration = function.locals().get(local.index() as usize)?;
    let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
        return None;
    };
    let argument = argument as usize;
    if function.abi().source_argument_ownership().get(argument)
        != Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
        || function.abi().source_input_types().get(argument) != Some(&declaration.ty())
    {
        return None;
    }
    let source = types.get(declaration.ty().index() as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = source.shape() else {
        return None;
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
        || pointer.pointer_width_bits() != 64
        || pointer.address_space() != 0
    {
        return None;
    }
    let pointee = function
        .abi()
        .adjusted_arguments()
        .get(argument)?
        .value()
        .pointee_override()
        .or(source.abi_properties().first_pointee())?;
    if pointee.kind() != (SemanticAbiPointeeKindV1::SharedReference { frozen: false }) {
        return None;
    }
    let SemanticTypeShapeV1::Slice { element } =
        types.get(pointer.pointee().index() as usize)?.shape()
    else {
        return None;
    };
    core_atomic_u32_v1(types, *element).then_some(*element)
}

fn atomic_u32_pointer_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<&fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1> {
    let SemanticTypeShapeV1::Pointer(pointer) = types.get(ty.index() as usize)?.shape() else {
        return None;
    };
    (pointer.pointer_width_bits() == 64
        && pointer.address_space() == 0
        && pointer.metadata() == SemanticPointerMetadataV1::None)
        .then_some(pointer)
}

fn atomic_u32_scalar_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            bits: 32,
            signed: false
        }))
    )
}

fn zero_atomic_field_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
) -> bool {
    let Some(local) = function.locals().get(place.local().index() as usize) else {
        return false;
    };
    let Some(pointer) = atomic_u32_pointer_v1(types, local.ty()) else {
        return false;
    };
    let [deref, field] = place.projections() else {
        return false;
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || !core_atomic_u32_v1(types, pointer.pointee())
        || deref.kind() != SemanticProjectionKindV1::Dereference
        || deref.result_type() != pointer.pointee()
        || field.kind() != SemanticProjectionKindV1::Field(0)
    {
        return false;
    }
    let atomic = &types[pointer.pointee().index() as usize];
    let SemanticTypeShapeV1::Aggregate(fields) = atomic.shape() else {
        return false;
    };
    let fe2o3_mir_model::semantic_mir_v1::SemanticFieldsShapeV1::Arbitrary {
        source_order_offsets_bytes,
        ..
    } = atomic.layout().fields()
    else {
        return false;
    };
    fields.fields() == [place.ty()]
        && source_order_offsets_bytes.as_ref() == [0]
        && types.get(place.ty().index() as usize).is_some_and(|field| {
            field.layout().size_bytes() == Some(4) && field.layout().alignment_bytes() == 4
        })
}

fn indexed_atomic_origin_v1(
    proofs: &mut SemanticAssertProofsV1<'_>,
    bounds: &[ProjectedBoundsCheckV1],
    address: &SemanticPlaceV1,
    use_site: ScalarAssignmentSiteV1,
) -> Result<
    Option<(IndexedAtomicBorrowOriginV1, Vec<SemanticLocalIdV1>)>,
    ProductionRankedProjectionErrorV1,
> {
    let [deref] = address.projections() else {
        return Ok(None);
    };
    if deref.kind() != SemanticProjectionKindV1::Dereference
        || !atomic_u32_scalar_v1(proofs.types, address.ty())
    {
        return Ok(None);
    }
    let mut local = address.local();
    let mut use_site = use_site;
    let mut chain = Vec::new();
    let mut field_address = false;
    let mut pointer_cast = false;
    loop {
        proofs.charge(chain.len() + 1)?;
        if chain.contains(&local) || chain.len() >= MAX_PROJECTED_OPERATIONS_V1 {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "cyclic indexed atomic pointer provenance",
            ));
        }
        chain.push(local);
        let index = local.index() as usize;
        if proofs.address_escaped.get(index).copied() != Some(false) {
            return Ok(None);
        }
        let Some(definition) = proofs.assignments.get(index).copied().flatten() else {
            return Ok(None);
        };
        if !proofs.assignment_dominates_use(definition, use_site.block, use_site.statement)? {
            return Ok(None);
        }
        if !indexed_atomic_block_acyclic_v1(proofs, definition.block)? {
            return Ok(None);
        }
        let SemanticStatementKindV1::Assign(assignment) =
            proofs.function.blocks()[definition.block].statements()[definition.statement].kind()
        else {
            return Ok(None);
        };
        match assignment.value().kind() {
            SemanticRvalueKindV1::Use(operand)
                if operand.ty() == assignment.value().result_type() =>
            {
                let Some(source) = simple_operand_local(operand) else {
                    return Ok(None);
                };
                if atomic_u32_pointer_v1(proofs.types, operand.ty()).is_none() {
                    return Ok(None);
                }
                local = source;
            }
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Pointer,
                operand,
            } if !pointer_cast && !field_address => {
                let Some(destination) =
                    atomic_u32_pointer_v1(proofs.types, assignment.value().result_type())
                else {
                    return Ok(None);
                };
                let Some(source) = atomic_u32_pointer_v1(proofs.types, operand.ty()) else {
                    return Ok(None);
                };
                if destination.kind() != SemanticPointerKindV1::Raw
                    || source.kind() != SemanticPointerKindV1::Raw
                    || !atomic_u32_scalar_v1(proofs.types, destination.pointee())
                    || proofs
                        .types
                        .get(source.pointee().index() as usize)
                        .is_none_or(|ty| ty.layout().size_bytes() != Some(4))
                {
                    return Ok(None);
                }
                let Some(source) = simple_operand_local(operand) else {
                    return Ok(None);
                };
                pointer_cast = true;
                local = source;
            }
            SemanticRvalueKindV1::AddressOf { place, .. }
                if pointer_cast
                    && !field_address
                    && zero_atomic_field_v1(proofs.types, proofs.function, place) =>
            {
                let Some(pointer) =
                    atomic_u32_pointer_v1(proofs.types, assignment.value().result_type())
                else {
                    return Ok(None);
                };
                if pointer.kind() != SemanticPointerKindV1::Raw || pointer.pointee() != place.ty() {
                    return Ok(None);
                }
                field_address = true;
                local = place.local();
            }
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } if pointer_cast && field_address => {
                let Some(element) =
                    atomic_slice_argument_v1(proofs.types, proofs.function, place.local())
                else {
                    return Ok(None);
                };
                let Some(pointer) =
                    atomic_u32_pointer_v1(proofs.types, assignment.value().result_type())
                else {
                    return Ok(None);
                };
                if pointer.kind() != SemanticPointerKindV1::Reference
                    || pointer.mutability() != SemanticMutabilityV1::Immutable
                    || pointer.pointee() != element
                    || place.ty() != element
                    || proofs.definition_counts[place.local().index() as usize] != 0
                    || proofs.address_escaped[place.local().index() as usize]
                {
                    return Ok(None);
                }
                let [deref, index] = place.projections() else {
                    return Ok(None);
                };
                if deref.kind() != SemanticProjectionKindV1::Dereference {
                    return Ok(None);
                }
                let identity = match index.kind() {
                    SemanticProjectionKindV1::Index(index) => {
                        if !indexed_atomic_index_stable_v1(proofs, index)? {
                            return Ok(None);
                        }
                        ProjectedBoundsIndexIdentityV1::Local(index)
                    }
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        from_end: false,
                        ..
                    } => ProjectedBoundsIndexIdentityV1::Literal(offset),
                    _ => return Ok(None),
                };
                let mut selected = None;
                for guard in bounds {
                    proofs.charge(1)?;
                    if guard.extent_source == ProjectedBoundsExtentSourceV1::Slice(place.local())
                        && guard.index_identity == identity
                        && proofs.block_dominates(guard.access_block, definition.block)?
                        && selected.replace(*guard).is_some()
                    {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "ambiguous dominating indexed atomic bounds guards",
                        ));
                    }
                }
                let Some(guard) = selected else {
                    return Ok(None);
                };
                return Ok(Some((
                    IndexedAtomicBorrowOriginV1 {
                        place: place.clone(),
                        guard,
                        borrow: definition,
                    },
                    chain,
                )));
            }
            _ => return Ok(None),
        }
        use_site = definition;
    }
}

fn authenticated_atomic_allocations_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    bounds: &[ProjectedBoundsCheckV1],
    contracts: &[Option<AllocationContractV1>],
) -> Result<AuthenticatedAtomicAllocationsV1, ProductionRankedProjectionErrorV1> {
    let mut result = AuthenticatedAtomicAllocationsV1::default();
    // Preserve the independently authenticated singleton route without granting slice authority.
    result.coherent.extend(
        contracts
            .iter()
            .flatten()
            .filter(|contract| contract.singleton_object)
            .map(|contract| contract.allocation_origin),
    );
    result.charge(contracts.len())?;
    for (index, _) in function.locals().iter().enumerate() {
        result.charge(1)?;
        if atomic_slice_argument_v1(types, function, SemanticLocalIdV1::from_index(index as u32))
            .is_some()
        {
            let contract = contracts.get(index).copied().flatten().ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "a nominal atomic slice has no authenticated allocation root",
                ),
            )?;
            result.atomic_only_roots.push(contract.allocation_origin);
        }
    }
    if result.atomic_only_roots.is_empty() {
        return Ok(result);
    }
    let mut proofs = SemanticAssertProofsV1::new(types, function)?;
    proofs.work = result.work.get();
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, record) in body.statements().iter().enumerate() {
            proofs.charge(1)?;
            let Some((address, atomic, access)) = indexed_atomic_effect_v1(record) else {
                continue;
            };
            let Some((origin, chain)) = indexed_atomic_origin_v1(
                &mut proofs,
                bounds,
                address,
                ScalarAssignmentSiteV1 { block, statement },
            )?
            else {
                continue;
            };
            if atomic.scope() != SemanticAtomicScopeV1::System {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "indexed atomics require the authenticated System scope",
                ));
            }
            let contract = contracts
                .get(origin.place.local().index() as usize)
                .copied()
                .flatten()
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "indexed atomics lost their source allocation",
                ))?;
            if !contract.writable || contract.singleton_object {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "indexed atomics have an incompatible allocation contract",
                ));
            }
            result.coherent.push(contract.allocation_origin);
            if result
                .indexed_locals
                .len()
                .saturating_add(chain.len())
                .saturating_add(result.indexed_uses.len().saturating_mul(3))
                > MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1
            {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "indexed atomic custody storage exceeds its existing limit",
                ));
            }
            proofs.charge(chain.len())?;
            result.indexed_locals.extend(chain);
            result.indexed_borrows.push(origin.clone());
            result.indexed_uses.push(IndexedAtomicUseV1 {
                block,
                statement,
                access,
                address: address.clone(),
                atomic,
                origin,
            });
        }
    }
    let rows = result
        .coherent
        .len()
        .saturating_add(result.indexed_locals.len());
    proofs.charge(rows.saturating_mul(rows.checked_ilog2().unwrap_or(0) as usize + 2))?;
    result.work.set(proofs.work);
    result.coherent.sort_unstable();
    result.coherent.dedup();
    result
        .indexed_locals
        .sort_unstable_by_key(|local| local.index());
    result.indexed_locals.dedup();
    reject_indexed_atomic_escapes_v1(types, function, &result)?;
    Ok(result)
}

#[derive(Debug)]
pub(crate) struct IndexedAtomicEscapeV1 {
    block: usize,
    statement: Option<usize>,
    local: u32,
    context: &'static str,
}

impl std::fmt::Display for IndexedAtomicEscapeV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "semantic-to-ranked projection rejected an indexed atomic pointer escape: bb{}, statement {:?}, local _{}, {}",
            self.block, self.statement, self.local, self.context
        )
    }
}

fn reject_indexed_atomic_escapes_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    inventory: &AuthenticatedAtomicAllocationsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let site = std::cell::Cell::new((0_usize, None, "unknown use"));
    let reject_operand =
        |operand: &SemanticOperandV1| -> Result<(), ProductionRankedProjectionErrorV1> {
            if let Some(place) = raw_operand_place(operand)
                && inventory.contains_local(place.local())?
            {
                let (block, statement, context) = site.get();
                return Err(ProductionRankedProjectionErrorV1::IndexedAtomicEscape(
                    IndexedAtomicEscapeV1 {
                        block,
                        statement,
                        local: place.local().index(),
                        context,
                    },
                ));
            }
            Ok(())
        };
    for (block, body) in function.blocks().iter().enumerate() {
        for (position, statement) in body.statements().iter().enumerate() {
            inventory.charge(1)?;
            site.set((block, Some(position), "memory value operand"));
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    let context = match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(_) => "copy/move operand",
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Pointer,
                            ..
                        } => "pointer cast operand",
                        SemanticRvalueKindV1::Cast { .. } => "non-pointer cast operand",
                        _ => "other rvalue operand",
                    };
                    site.set((block, Some(position), context));
                    let transporting = assignment.destination().projections().is_empty()
                        && inventory.contains_local(assignment.destination().local())?
                        && matches!(
                            assignment.value().kind(),
                            SemanticRvalueKindV1::Use(_)
                                | SemanticRvalueKindV1::Cast {
                                    kind: SemanticCastKindV1::Pointer,
                                    ..
                                }
                        );
                    let dead_cast = !transporting
                        && indexed_atomic_dead_cast_v1(
                            types,
                            function,
                            inventory,
                            ScalarAssignmentSiteV1 {
                                block,
                                statement: position,
                            },
                            assignment,
                        )?;
                    if !transporting && !dead_cast {
                        assignment
                            .value()
                            .kind()
                            .try_visit_operands(&reject_operand)?;
                    }
                }
                SemanticStatementKindV1::Store(store) => reject_operand(store.value())?,
                SemanticStatementKindV1::AtomicRmw(atomic) => reject_operand(atomic.value())?,
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    reject_operand(atomic.expected())?;
                    reject_operand(atomic.replacement())?;
                }
                _ => {}
            }
        }
        site.set((block, None, "call argument"));
        if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() {
            for argument in call.arguments() {
                reject_operand(argument)?;
            }
        }
        if let SemanticTerminatorKindV1::TailCall(call) = body.terminator().kind() {
            for argument in call.arguments() {
                reject_operand(argument)?;
            }
        }
    }
    Ok(())
}

const fn atomic_ordering_v1(ordering: SemanticAtomicOrderingV1) -> AtomicOrderingAttr {
    match ordering {
        SemanticAtomicOrderingV1::Relaxed => AtomicOrderingAttr::Relaxed,
        SemanticAtomicOrderingV1::Release => AtomicOrderingAttr::Release,
        SemanticAtomicOrderingV1::Acquire => AtomicOrderingAttr::Acquire,
        SemanticAtomicOrderingV1::AcquireRelease => AtomicOrderingAttr::AcquireRelease,
        SemanticAtomicOrderingV1::SequentiallyConsistent => {
            AtomicOrderingAttr::SequentiallyConsistent
        }
    }
}

const fn atomic_scope_v1(scope: SemanticAtomicScopeV1) -> AtomicScopeAttr {
    match scope {
        SemanticAtomicScopeV1::SingleThread => AtomicScopeAttr::SingleThread,
        SemanticAtomicScopeV1::Workgroup => AtomicScopeAttr::Workgroup,
        SemanticAtomicScopeV1::Agent => AtomicScopeAttr::Agent,
        SemanticAtomicScopeV1::Device => AtomicScopeAttr::Device,
        SemanticAtomicScopeV1::System => AtomicScopeAttr::System,
    }
}
