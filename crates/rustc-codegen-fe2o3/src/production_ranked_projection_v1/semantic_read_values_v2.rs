// Bind source values to original memory events, never replacement argument symbols.
impl<'a> GpuSemanticExpressionResolverV2<'a> {
    fn with_ranked_reads(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        blocks: &[ProductionRankedBlockV1],
        sources: &[ProjectedAccessSourceV1],
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        type Error = ProductionRankedProjectionErrorV1;
        if sources.len() > MAX_RANKED_BOUNDS_OPERATIONS {
            return Err(Error::Unsupported(
                "source read bindings exceed the ranked operation limit",
            ));
        }
        let mut resolver = Self::new(types, function)?;
        resolver.callables = callables;
        let mut origins = HashMap::new();
        for operation in blocks.iter().flat_map(|block| block.operations()) {
            if let ProductionRankedOperationV1::View {
                result,
                allocation_origin,
                ..
            }
            | ProductionRankedOperationV1::ViewInSpace {
                result,
                allocation_origin,
                ..
            } = operation
            {
                origins.insert(ProductionRankedValueV1::Local(*result), *allocation_origin);
            }
        }
        // Group once: repeated reads in one statement keep their original order,
        // without rescanning every source event for each individual load.
        let mut groups = BTreeMap::<_, Vec<_>>::new();
        for source in sources {
            if source.access != AccessKindAttr::Read
                || source.memory_space != MemorySpaceAttr::Global
            {
                continue;
            }
            if let Some(site) = source.semantic_site {
                groups
                    .entry((site.block, site.statement))
                    .or_default()
                    .push(source);
            }
        }
        for ((block, statement), sources) in groups {
            let source_block = function.blocks().get(block).ok_or(Error::Unsupported(
                "a source read binding is outside the semantic CFG",
            ))?;
            if let Some(statement) = statement {
                let Some(statement) = source_block.statements().get(statement) else {
                    return Err(Error::Unsupported(
                        "a source read binding is outside its semantic block",
                    ));
                };
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let mut places = Vec::new();
                semantic_rvalue_read_places_v2(assignment.value(), &mut places)?;
                if places.len() != sources.len() {
                    continue;
                }
                for ((place, mode), source) in places.into_iter().zip(sources) {
                    let scalar = resolver.scalar_v2(place.ty()).map_err(Error::Incomplete)?;
                    let load = ranked_read_value_v2(blocks, &origins, source, scalar, mode)?;
                    if resolver
                        .place_loads
                        .insert(place as *const _, load.clone())
                        .is_some()
                    {
                        return Err(Error::Incomplete(
                            "one semantic scalar read place maps to multiple ranked reads",
                        ));
                    }
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(_))
                        && resolver
                            .loads
                            .insert(assignment.value() as *const _, load)
                            .is_some()
                    {
                        return Err(Error::Incomplete(
                            "one semantic scalar load maps to multiple ranked reads",
                        ));
                    }
                }
                continue;
            }
            let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad { element },
                ..
            }) = callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            if sources.len() != 1
                || call.arguments().len() != 2
                || !call.variadic_argument_abis().is_empty()
                || sources[0].source != source_block.terminator().source()
            {
                return Err(Error::Incomplete(
                    "volatile call read has no unique exact source event",
                ));
            }
            let destination = call.destination().ok_or(Error::Incomplete(
                "volatile call read has no scalar destination",
            ))?;
            let place = destination.place();
            let local = place.local().index() as usize;
            let declaration = function.locals().get(local).ok_or(Error::Incomplete(
                "volatile call result is outside the local table",
            ))?;
            if !place.projections().is_empty()
                || place.ty() != *element
                || declaration.ty() != *element
                || matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
                || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            {
                return Err(Error::Incomplete(
                    "volatile call result type or destination changed",
                ));
            }
            if resolver.source_proof.definition_counts.get(local) != Some(&1)
                || resolver.source_proof.address_escaped.get(local) != Some(&false)
            {
                return Err(Error::Incomplete(
                    "volatile call result is not uniquely defined and unescaped",
                ));
            }
            // Receiver provenance and bounds were authenticated by the source
            // projector. This only joins that event to its retained scalar result.
            let scalar = resolver.scalar_v2(*element).map_err(Error::Incomplete)?;
            let load = ranked_read_value_v2(
                blocks,
                &origins,
                sources[0],
                scalar,
                fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile,
            )?;
            if resolver
                .call_loads
                .insert(
                    place.local().index(),
                    (load, (block, destination.edge().target().index() as usize)),
                )
                .is_some()
            {
                return Err(Error::Incomplete(
                    "one volatile call result maps to multiple ranked reads",
                ));
            }
        }
        Ok(resolver)
    }

    fn resolve_source_write_v2(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let block = self
            .function
            .blocks()
            .get(site.block)
            .ok_or("GPU write source block is out of bounds")?;
        if let Some(statement) = site.statement {
            return self.resolve_store_v2(
                block
                    .statements()
                    .get(statement)
                    .ok_or("GPU write source statement is out of bounds")?
                    .kind(),
                ScalarAssignmentSiteV1 {
                    block: site.block,
                    statement,
                },
            );
        }
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err("GPU write source terminator is not a typed write call");
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                    element, kind, ..
                },
            ..
        }) = self.callables.get(call.callee().index() as usize)
        else {
            return Err("GPU write source call has no authenticated scalar payload contract");
        };
        let arity = match kind {
            SemanticWriteOnlyDisjointWriteKindV1::Thread { .. } => 3,
            SemanticWriteOnlyDisjointWriteKindV1::GridExclusive
            | SemanticWriteOnlyDisjointWriteKindV1::Block { .. } => 4,
            SemanticWriteOnlyDisjointWriteKindV1::Tiled2d { .. }
            | SemanticWriteOnlyDisjointWriteKindV1::RowStriped2d { .. } => 7,
        };
        if call.arguments().len() != arity || !call.variadic_argument_abis().is_empty() {
            return Err("GPU typed write call payload arity changed");
        }
        let payload = &call.arguments()[arity - 1];
        if payload.ty() != *element {
            return Err("GPU typed write call payload type changed");
        }
        self.scalar_v2(*element)?;
        self.resolve_operand_v2(
            payload,
            0,
            ScalarAssignmentSiteV1 {
                block: site.block,
                statement: block.statements().len(),
            },
        )
    }

    fn charge_source_query_v2(&mut self) -> Result<(), &'static str> {
        // Dominance charges traversal; also account for its per-query visited map.
        self.source_proof
            .charge(self.function.blocks().len())
            .map_err(|_| "GPU semantic source dependence exceeds its analysis budget")
    }

    fn require_source_value_graph_v2(&mut self) -> Result<(), &'static str> {
        if self.source_acyclic.is_none() {
            // A unique syntactic definition is not a unique dynamic definition in
            // a loop. Keep substitution acyclic until source SSA instances are bound.
            self.charge_source_query_v2()?;
            let edges = self
                .source_proof
                .graph
                .successors
                .iter()
                .map(Vec::len)
                .sum::<usize>();
            self.source_proof
                .charge(2 * self.function.blocks().len() + 2 * edges)
                .map_err(|_| "GPU semantic source dependence exceeds its analysis budget")?;
            let graph = &self.source_proof.graph;
            let mut incoming = vec![0; graph.successors.len()];
            for (block, targets) in graph.successors.iter().enumerate() {
                if graph.reachable[block] {
                    for &target in targets {
                        incoming[target] += 1;
                    }
                }
            }
            let mut pending = incoming
                .iter()
                .enumerate()
                .filter_map(|(block, &count)| {
                    (count == 0 && graph.reachable[block]).then_some(block)
                })
                .collect::<Vec<_>>();
            let mut visited = 0;
            while let Some(block) = pending.pop() {
                visited += 1;
                for &target in &graph.successors[block] {
                    incoming[target] -= 1;
                    if incoming[target] == 0 {
                        pending.push(target);
                    }
                }
            }
            let acyclic = visited
                == graph
                    .reachable
                    .iter()
                    .filter(|&&reachable| reachable)
                    .count();
            self.source_acyclic = Some(acyclic);
        }
        if self.source_acyclic == Some(true) {
            Ok(())
        } else {
            Err("GPU scalar substitution requires acyclic source definitions")
        }
    }
}

fn check_source_value_analysis_size_v2(
    function: &SemanticFunctionDeclV1,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    type Error = ProductionRankedProjectionErrorV1;
    if function.blocks().len() > MAX_RANKED_BOUNDS_BLOCKS
        || function.locals().len() > MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1
    {
        return Err(Error::Unsupported(
            "source value analysis exceeds its table limits",
        ));
    }
    let mut count = 0usize;
    let mut edges = 0usize;
    for block in function.blocks() {
        count = count
            .checked_add(block.statements().len())
            .and_then(|n| n.checked_add(1))
            .filter(|&n| n <= MAX_RANKED_BOUNDS_OPERATIONS)
            .ok_or(Error::Unsupported(
                "source value analysis exceeds its operation limit",
            ))?;
        let unwind = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => Some(call.unwind()),
            SemanticTerminatorKindV1::TailCall(call) => Some(call.unwind()),
            SemanticTerminatorKindV1::Drop { unwind, .. }
            | SemanticTerminatorKindV1::Assert { unwind, .. } => Some(*unwind),
            _ => None,
        };
        if matches!(unwind, Some(SemanticUnwindActionV1::Cleanup(_))) {
            return Err(Error::Incomplete(
                "source value availability does not support cleanup edges",
            ));
        }
        block.terminator().kind().try_for_each_edge(|_| {
            edges = edges
                .checked_add(1)
                .filter(|&n| n <= MAX_RANKED_BOUNDS_EDGES)
                .ok_or(Error::Unsupported(
                    "source value analysis exceeds its raw edge limit",
                ))?;
            Ok(())
        })?;
    }
    // Reserve table initialization, scans, and sorting work before allocating
    // the shared graph/inventory. Counts above are bounded before multiplication.
    let sort_depth = |n: usize| if n < 2 { 0 } else { n.ilog2() as usize + 1 };
    let construction = function.locals().len() * 8
        + function.blocks().len() * 8
        + count * (16 + 2 * sort_depth(2 * count))
        + edges * (8 + sort_depth(edges));
    let mut work = 0;
    project_loop_graph_charge_v1(&mut work, construction)?;
    Ok(work)
}

fn ranked_read_value_v2(
    blocks: &[ProductionRankedBlockV1],
    origins: &HashMap<ProductionRankedValueV1, u64>,
    source: &ProjectedAccessSourceV1,
    scalar: ProductionSemanticScalarTypeV2,
    read_mode: fe2o3_pliron::ProductionSemanticReadModeV2,
) -> Result<ProductionSemanticLoadV2, ProductionRankedProjectionErrorV1> {
    type Error = ProductionRankedProjectionErrorV1;
    let operation = blocks
        .get(source.block)
        .and_then(|block| block.operations().get(source.operation))
        .ok_or(Error::Unsupported(
            "a projected load correspondence does not identify one ranked read",
        ))?;
    let (kind, view, indices) = match operation {
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } => (*kind, *view, indices.clone()),
        ProductionRankedOperationV1::PredicatedAccess {
            kind, view, index, ..
        } => (*kind, *view, vec![*index]),
        _ => {
            return Err(Error::Unsupported(
                "a projected load correspondence does not identify one ranked read",
            ));
        }
    };
    if kind != AccessKindAttr::Read {
        return Err(Error::Unsupported(
            "a projected scalar load changed ranked access kind",
        ));
    }
    Ok(ProductionSemanticLoadV2 {
        block: u32::try_from(source.block).map_err(|_| {
            Error::Unsupported("a projected load block exceeds the ranked identity width")
        })?,
        operation: u32::try_from(source.operation).map_err(|_| {
            Error::Unsupported("a projected load operation exceeds the ranked identity width")
        })?,
        scalar,
        read_mode,
        allocation_origin: origins.get(&view).copied().ok_or(Error::Unsupported(
            "a projected load view has no exact allocation origin",
        ))?,
        view,
        indices: indices.into_boxed_slice(),
    })
}
