// Private source-derived memory versions. This does not remove ranked accesses
// or replace the production concurrency and source-correspondence checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypedGlobalMemoryVersionV1 {
    Initial,
    Store { semantic_block: usize },
    Conflict,
}

#[derive(Clone, Copy)]
struct TypedGlobalMemoryEventV1 {
    origin: u64,
    semantic_block: usize,
    access: AccessKindAttr,
    bounds: (ProductionRankedValueV1, ProductionRankedValueV1),
    mutable: bool,
}

type TypedGlobalMemoryVersionsV1 = HashMap<(usize, usize), TypedGlobalMemoryVersionV1>;

fn typed_global_memory_successors_v1(
    terminator: &ProductionRankedTerminatorV1,
) -> Result<[Option<usize>; 2], ProductionRankedProjectionErrorV1> {
    use ProductionRankedTerminatorV1 as T;
    Ok(match terminator {
        T::IndexLessThan {
            true_block,
            false_block,
            ..
        }
        | T::IndexEqual {
            true_block,
            false_block,
            ..
        } => [Some(*true_block as usize), Some(*false_block as usize)],
        T::AnalysisSplit {
            first_block,
            second_block,
            ..
        } => [Some(*first_block as usize), Some(*second_block as usize)],
        T::Branch { target } => [Some(*target as usize), None],
        T::Return | T::Trap => [None, None],
        _ => {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "mutable global memory versions do not support block-argument control flow",
            ));
        }
    })
}

fn typed_global_memory_order_v1(
    blocks: &[ProductionRankedBlockV1],
    work: &mut usize,
) -> Result<Vec<usize>, ProductionRankedProjectionErrorV1> {
    charge_capability_dataflow_work_v1(work, blocks.len())?;
    let mut incoming = vec![0usize; blocks.len()];
    for block in blocks {
        charge_capability_dataflow_work_v1(work, 1)?;
        if block.index_argument_count() != 0 {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "mutable global memory versions do not support block arguments",
            ));
        }
        for target in typed_global_memory_successors_v1(block.terminator())?
            .into_iter()
            .flatten()
        {
            charge_capability_dataflow_work_v1(work, 1)?;
            let count =
                incoming
                    .get_mut(target)
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "mutable global CFG edge is out of bounds",
                    ))?;
            *count += 1;
        }
    }
    charge_capability_dataflow_work_v1(work, blocks.len())?;
    let mut ready = incoming
        .iter()
        .enumerate()
        .filter_map(|(block, count)| (*count == 0).then_some(block))
        .collect::<VecDeque<_>>();
    charge_capability_dataflow_work_v1(work, blocks.len())?;
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_front() {
        charge_capability_dataflow_work_v1(work, 1)?;
        order.push(block);
        for target in typed_global_memory_successors_v1(blocks[block].terminator())?
            .into_iter()
            .flatten()
        {
            charge_capability_dataflow_work_v1(work, 1)?;
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.push_back(target);
            }
        }
    }
    if order.len() != blocks.len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "mutable global memory versions require an acyclic execution CFG",
        ));
    }
    Ok(order)
}

fn typed_global_reaching_version_v1(
    blocks: &[ProductionRankedBlockV1],
    order: &[usize],
    events: &HashMap<(usize, usize), TypedGlobalMemoryEventV1>,
    read_site: (usize, usize),
    read: TypedGlobalMemoryEventV1,
    work: &mut usize,
) -> Result<TypedGlobalMemoryVersionV1, ProductionRankedProjectionErrorV1> {
    charge_capability_dataflow_work_v1(work, blocks.len())?;
    let mut versions = vec![None; blocks.len()];
    charge_capability_dataflow_work_v1(work, blocks.len())?;
    let mut unguarded = vec![false; blocks.len()];
    if blocks.is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "mutable global CFG is empty",
        ));
    }
    versions[0] = Some(TypedGlobalMemoryVersionV1::Initial);
    unguarded[0] = true;
    for &block in order {
        charge_capability_dataflow_work_v1(work, 1)?;
        let mut version = versions[block];
        for (operation, _) in blocks[block].operations().iter().enumerate() {
            charge_capability_dataflow_work_v1(work, 1)?;
            if (block, operation) == read_site {
                return match version {
                    Some(
                        TypedGlobalMemoryVersionV1::Initial
                        | TypedGlobalMemoryVersionV1::Store { .. },
                    ) if !unguarded[block] => Ok(version.expect("version matched")),
                    _ => Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "mutable global load lacks one guarded reaching memory version",
                    )),
                };
            }
            if version.is_some()
                && let Some(event) = events.get(&(block, operation))
                && event.origin == read.origin
                && event.access == AccessKindAttr::Write
            {
                version = Some(TypedGlobalMemoryVersionV1::Store {
                    semantic_block: event.semantic_block,
                });
            }
        }
        let exact_bounds = matches!(blocks[block].terminator(),
            ProductionRankedTerminatorV1::IndexLessThan { lhs, rhs, .. }
                if (*lhs, *rhs) == read.bounds);
        for (edge, target) in typed_global_memory_successors_v1(blocks[block].terminator())?
            .into_iter()
            .enumerate()
            .filter_map(|(edge, target)| target.map(|target| (edge, target)))
        {
            charge_capability_dataflow_work_v1(work, 1)?;
            // Independently check domination on the unpruned graph: removing
            // true bounds edges must make the actual read unreachable.
            if !(exact_bounds && edge == 0) {
                unguarded[target] |= unguarded[block];
            }
            // A successful read fixes this immutable SSA comparison to true.
            // This also excludes failed earlier stores at the identical index
            // and physical extent; unrelated guards are never assumed.
            if exact_bounds && edge == 1 {
                continue;
            }
            if let Some(version) = version {
                versions[target] = Some(match versions[target] {
                    None => version,
                    Some(previous) if previous == version => version,
                    Some(_) => TypedGlobalMemoryVersionV1::Conflict,
                });
            }
        }
    }
    Err(ProductionRankedProjectionErrorV1::Incomplete(
        "mutable global read site is absent",
    ))
}

fn typed_global_memory_versions_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    intrinsic: &IntrinsicProjectionV1,
    blocks: &[ProductionRankedBlockV1],
    sources: &[ProjectedAccessSourceV1],
    work: &mut usize,
) -> Result<TypedGlobalMemoryVersionsV1, ProductionRankedProjectionErrorV1> {
    let mut needs_versions = false;
    for source in sources {
        charge_capability_dataflow_work_v1(work, 1)?;
        if source.access == AccessKindAttr::Read
            && let Some(site) = source.semantic_site
            && site.statement.is_none()
            && intrinsic
                .global_views
                .get(site.block)
                .copied()
                .flatten()
                .is_some_and(|bound| bound.allocation.writable)
        {
            needs_versions = true;
        }
    }
    if !needs_versions {
        return Ok(HashMap::new());
    }

    let mut views = HashMap::new();
    let mut classes = HashMap::new();
    let mut origins = HashMap::new();
    let mut invocation_indices = HashSet::new();
    for block in blocks {
        for operation in block.operations() {
            charge_capability_dataflow_work_v1(work, 1)?;
            use ProductionRankedOperationV1 as Op;
            match operation {
                Op::View {
                    result,
                    allocation_origin,
                    noalias_class,
                    writable,
                    shape,
                    dynamic_extents,
                    ..
                }
                | Op::ViewInSpace {
                    result,
                    allocation_origin,
                    noalias_class,
                    writable,
                    shape,
                    dynamic_extents,
                    memory_space: MemorySpaceAttr::Global,
                    ..
                } => {
                    if *allocation_origin == 0
                        || *noalias_class == 0
                        || (*writable && *noalias_class <= 1)
                        || shape.as_slice() != [DYNAMIC_EXTENT]
                        || dynamic_extents.len() != 1
                        || views
                            .insert(
                                ProductionRankedValueV1::Local(*result),
                                (
                                    *allocation_origin,
                                    *noalias_class,
                                    *writable,
                                    dynamic_extents[0],
                                ),
                            )
                            .is_some()
                    {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "mutable global memory requires exact nonaliasing dynamic views",
                        ));
                    }
                    if *noalias_class > 1
                        && classes
                            .insert(*noalias_class, *allocation_origin)
                            .is_some_and(|previous| previous != *allocation_origin)
                    {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "mutable global allocation alias classes overlap",
                        ));
                    }
                    if origins
                        .insert(
                            *allocation_origin,
                            (*noalias_class, *writable, dynamic_extents[0]),
                        )
                        .is_some_and(|previous| {
                            previous != (*noalias_class, *writable, dynamic_extents[0])
                        })
                    {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "mutable global allocation has inconsistent view identities",
                        ));
                    }
                }
                Op::InvocationIndex {
                    result,
                    dimension: 0,
                    ..
                } => {
                    invocation_indices.insert(ProductionRankedValueV1::Local(*result));
                }
                Op::ExecutionLayout {
                    global_extents,
                    workgroup_extents,
                    ..
                } if global_extents[1..] == [1, 1] && workgroup_extents[1..] == [1, 1] => {}
                Op::IndexConstant { .. }
                | Op::IndexUnsignedCast { .. }
                | Op::IndexUnknown { .. }
                | Op::IndexBinary { .. }
                | Op::DeterministicJoin { .. }
                | Op::Dimension { .. }
                | Op::Access { .. } => {}
                _ => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "mutable global memory versions reject unsupported effects or synchronization",
                    ));
                }
            }
        }
    }
    let mut by_site = HashMap::new();
    for source in sources {
        charge_capability_dataflow_work_v1(work, 1)?;
        if by_site
            .insert((source.block, source.operation), source)
            .is_some()
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "mutable global access source is ambiguous",
            ));
        }
    }
    let mut events = HashMap::new();
    let mut mutable_coordinates = HashMap::new();
    let mut semantic_sites = HashSet::new();
    for (block, body) in blocks.iter().enumerate() {
        for (operation, op) in body.operations().iter().enumerate() {
            charge_capability_dataflow_work_v1(work, 1)?;
            let ProductionRankedOperationV1::Access {
                kind,
                view,
                indices,
            } = op
            else {
                continue;
            };
            let source = by_site.get(&(block, operation)).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "mutable global access lacks exact source custody",
                ),
            )?;
            let (call, _, bound) = typed_global_source_call_v1(
                function, callables, intrinsic, source, *view, indices, *kind,
            )
            .map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
            let Some(&(origin, class, writable, extent)) = views.get(view) else {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "mutable global access view is absent",
                ));
            };
            let site = source.semantic_site.expect("source call authenticated");
            if !semantic_sites.insert(site.block)
                || !matches!(kind, AccessKindAttr::Read | AccessKindAttr::Write)
                || origin != bound.allocation.allocation_origin
                || class != bound.allocation.noalias_class
                || writable != bound.allocation.writable
                || indices.len() != 1
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "mutable global access or allocation was substituted",
                ));
            }
            if writable {
                let effect = if *kind == AccessKindAttr::Read {
                    &intrinsic.direct_read_effects[site.block]
                } else {
                    &intrinsic.direct_write_effects[site.block]
                };
                let index = call
                    .arguments()
                    .get(1)
                    .and_then(simple_operand_local)
                    .and_then(|local| intrinsic.index_values.get(local.index() as usize))
                    .copied()
                    .flatten();
                if bound.contract
                    != SemanticCapabilityMemoryContractV1::global_exclusive_read_write()
                    || effect.as_ref().is_none_or(|effect| {
                        effect.comparisons.as_slice() != [(indices[0], extent)]
                            || effect.checked_success.is_some()
                    })
                    || index.is_none_or(|index| {
                        index.mapping != SemanticDisjointIndexSpaceV1::Index1d
                            || index.precondition.is_some()
                            || index.availability.is_some()
                            || index.value != indices[0]
                    })
                    || !invocation_indices.contains(&indices[0])
                    || mutable_coordinates
                        .insert(origin, (indices[0], extent))
                        .is_some_and(|previous| previous != (indices[0], extent))
                {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "mutable global memory requires one exact invocation-owned coordinate per allocation",
                    ));
                }
            }
            events.insert(
                (block, operation),
                TypedGlobalMemoryEventV1 {
                    origin,
                    semantic_block: site.block,
                    access: *kind,
                    bounds: (indices[0], extent),
                    mutable: writable,
                },
            );
        }
    }
    if events.len() != by_site.len() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "mutable global source roster contains an unmodeled effect",
        ));
    }
    let order = typed_global_memory_order_v1(blocks, work)?;
    let mut versions = HashMap::new();
    // Ranked order, not hash-map iteration, defines resource charging and replay.
    for &block in &order {
        for operation in 0..blocks[block].operations().len() {
            charge_capability_dataflow_work_v1(work, 1)?;
            if let Some(&read) = events.get(&(block, operation))
                && read.mutable
                && read.access == AccessKindAttr::Read
            {
                let version = typed_global_reaching_version_v1(
                    blocks,
                    &order,
                    &events,
                    (block, operation),
                    read,
                    work,
                )?;
                versions.insert((block, operation), version);
            }
        }
    }
    Ok(versions)
}
