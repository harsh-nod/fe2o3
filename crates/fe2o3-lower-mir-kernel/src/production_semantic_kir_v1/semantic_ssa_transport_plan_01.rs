fn promoted_transport_descriptor_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    local: u32,
    compiler_issued_bindings: &BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
    shared_promoted: &BTreeSet<u32>,
    capability_origins: &mut SemanticCapabilityOriginResolverV1<'_>,
    direct_parameters: &BTreeMap<u32, Type>,
) -> Result<(SemanticTypeIdV1, SemanticPromotedTransportV1), ProductionSemanticKirErrorV1> {
    let mut current = local;
    let mut visited = BTreeSet::new();
    loop {
        capability_origins.charge_work(1)?;
        if !visited.insert(current) || visited.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "transparent-borrow SSA transport is cyclic or too deep",
            ));
        }
        let declaration = function
            .locals()
            .get(current as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if let Some(binding) = compiler_issued_bindings.get(&declaration.ty()).copied() {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(binding),
            ));
        }
        if let Some(binding) = capability_origins.resolve(SemanticLocalIdV1::from_index(current))? {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(binding),
            ));
        }
        if matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
            && direct_parameters.contains_key(&current)
        {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::DirectParameter {
                    parameter_local: current,
                },
            ));
        }

        let Some(definition) =
            capability_origins.transparent_definition(SemanticLocalIdV1::from_index(current))
        else {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary),
            ));
        };
        let source = match definition {
            SemanticTransparentDefinitionV1::Borrow { source, .. }
            | SemanticTransparentDefinitionV1::ValueAlias { source, .. } => source,
        };
        if !shared_promoted.contains(&source) {
            return Ok((
                declaration.ty(),
                SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary),
            ));
        }
        match definition {
            SemanticTransparentDefinitionV1::Borrow { referent_type, .. } => {
                let Some(reference) =
                    types
                        .get(declaration.ty().index() as usize)
                        .and_then(|declaration| match declaration.shape() {
                            SemanticTypeShapeV1::Pointer(pointer) => Some(pointer),
                            _ => None,
                        })
                else {
                    return Ok((
                        declaration.ty(),
                        SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary),
                    ));
                };
                if reference.pointee() != referent_type {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "transparent-borrow SSA transport pointee type changed",
                    ));
                }
            }
            SemanticTransparentDefinitionV1::ValueAlias { source_type, .. } => {
                if declaration.ty() != source_type {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "transparent value-alias SSA transport type changed",
                    ));
                }
            }
        }
        current = source;
    }
}

impl SemanticControlFlowSsaPlanV1 {
    fn analyze(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        semantic_function: SemanticFunctionIdV1,
        semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
        option_dominance: &SemanticOptionDominanceV1,
        direct_parameters: &BTreeMap<u32, Type>,
        max_analysis_work: usize,
        max_analysis_storage: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if semantic_ssa.function() != semantic_function
            || semantic_ssa.function_identity() != function.identity()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let compiler_issued_bindings =
            compiler_issued_ssa_bindings_v1(types, callables, function, semantic_function)?;
        let shared = semantic_ssa.plan();
        let retained_cross_edge = semantic_ssa
            .retained_cross_edge_variables()
            .iter()
            .map(|local| local.get())
            .collect::<BTreeSet<_>>();
        let shared_promoted = shared
            .promoted_variables()
            .iter()
            .map(|variable| variable.get())
            .collect::<BTreeSet<_>>();
        let private_slot_candidates =
            private_slot_candidate_locals_v1(function, &shared_promoted, &retained_cross_edge);
        let mut retained_local_slots = BTreeMap::new();
        let mut unsupported_retained_locals = Vec::new();
        for local in private_slot_candidates {
            let declaration = function
                .locals()
                .get(local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let Some((kernel_type, alignment)) =
                retained_local_slot_type_v1(types, declaration.ty())
            else {
                unsupported_retained_locals.push((local, declaration.ty().index()));
                continue;
            };
            retained_local_slots.insert(
                local,
                SemanticRetainedLocalSlotPlanV1 {
                    semantic_type: declaration.ty(),
                    kernel_type,
                    alignment,
                },
            );
        }
        if !unsupported_retained_locals.is_empty() {
            const MAX_RETAINED_LOCAL_DIAGNOSTICS_V1: usize = 32;
            let retained_count = unsupported_retained_locals.len();
            unsupported_retained_locals.truncate(MAX_RETAINED_LOCAL_DIAGNOSTICS_V1);
            let unsupported_retained_locals = unsupported_retained_locals
                .into_iter()
                .map(|(local, ty)| {
                    (
                        local,
                        ty,
                        first_retained_local_cause_v1(
                            function,
                            local,
                            callables,
                            &compiler_issued_bindings,
                        ),
                    )
                })
                .collect();
            return Err(ProductionSemanticKirErrorV1::RetainedLocalStorage {
                function: semantic_function.index(),
                retained_locals: unsupported_retained_locals,
                retained_count,
            });
        }
        let implicit_entry_locals = semantic_ssa
            .implicit_entry_variables()
            .iter()
            .map(|variable| variable.get())
            .collect::<BTreeSet<_>>();
        if implicit_entry_locals.len() != semantic_ssa.implicit_entry_variables().len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        for local in &implicit_entry_locals {
            let declaration = function
                .locals()
                .get(*local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !shared_promoted.contains(local)
                || declaration.role() != SemanticLocalRoleV1::Temporary
                || !authenticated_ambient_workgroup_lds_scope_zst_v1(
                    types,
                    callables,
                    declaration.ty(),
                )
                || compiler_issued_bindings.get(&declaration.ty())
                    != Some(&SemanticPromotedBindingV1::WorkgroupLdsScope)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        let entry = function.entry().index();
        if shared
            .transport_variables(SsaBlockIdV1::new(entry))
            .is_some_and(|variables| !variables.is_empty())
        {
            return Err(unsupported(
                semantic_function.index(),
                Some(entry),
                None,
                "cyclic entry SSA requires a synthetic Kernel IR preheader",
            ));
        }
        let mut transported = BTreeSet::new();
        for block in shared.reverse_postorder() {
            if block.get() == entry {
                continue;
            }
            let variables = shared
                .transport_variables(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            for variable in variables {
                if !shared_promoted.contains(&variable.get()) {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                transported.insert(variable.get());
            }
        }
        let mut capability_origins = SemanticCapabilityOriginResolverV1::new(
            types,
            callables,
            function,
            option_dominance,
            &shared_promoted,
            max_analysis_work,
            max_analysis_storage,
        )?;
        let mut promoted = BTreeMap::new();
        for local in transported {
            let declaration = function
                .locals()
                .get(local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let (transport_semantic_type, binding) = promoted_transport_descriptor_v1(
                types,
                function,
                local,
                &compiler_issued_bindings,
                &shared_promoted,
                &mut capability_origins,
                direct_parameters,
            )?;
            let kernel_types =
                binding.transport_types(types, transport_semantic_type, direct_parameters)?;
            if kernel_types.is_empty()
                && !matches!(
                    binding,
                    SemanticPromotedTransportV1::Semantic(
                        SemanticPromotedBindingV1::MathContext
                            | SemanticPromotedBindingV1::CollectiveContext
                            | SemanticPromotedBindingV1::WorkgroupLdsScope
                            | SemanticPromotedBindingV1::MatrixContext
                            | SemanticPromotedBindingV1::GridLeader { .. }
                    )
                )
            {
                return Err(unsupported(
                    semantic_function.index(),
                    None,
                    None,
                    "SSA transport produced no Kernel IR block-parameter types",
                ));
            }
            promoted.insert(
                local,
                SemanticPromotedLocalV1 {
                    semantic_type: declaration.ty(),
                    transport_semantic_type,
                    transport: binding,
                    kernel_types: kernel_types.into_boxed_slice(),
                },
            );
        }
        let mut live_in = BTreeMap::new();
        for block in 0..function.blocks().len() as u32 {
            let block_id = SsaBlockIdV1::new(block);
            let locals = if block == entry || !shared.is_reachable(block_id) {
                Vec::new()
            } else {
                shared
                    .transport_variables(block_id)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .iter()
                    .map(|variable| variable.get())
                    .filter(|local| promoted.contains_key(local))
                    .collect()
            };
            live_in.insert(block, locals);
        }
        let mut edge_arguments = BTreeMap::new();
        let mut edge_definitions = BTreeMap::new();
        for block in shared.reverse_postorder() {
            let source = function
                .blocks()
                .get(block.get() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            for ordinal in 0..source.terminator().kind().edge_count() {
                let edge = fe2o3_mir_model::SsaEdgeIdV1::new(*block, ordinal as u32);
                let arguments = shared
                    .edge_arguments(edge)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .iter()
                    .copied()
                    .filter(|argument| promoted.contains_key(&argument.variable().get()))
                    .collect();
                let definitions = shared
                    .edge_definitions(edge)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .iter()
                    .copied()
                    .collect();
                edge_definitions.insert((block.get(), ordinal as u32), definitions);
                edge_arguments.insert((block.get(), ordinal as u32), arguments);
            }
        }

        let entry_definitions = shared
            .entry_definitions()
            .iter()
            .copied()
            .map(|definition| (definition.variable().get(), definition.value()))
            .collect();
        let mut block_entry_values = BTreeMap::new();
        let mut definition_values = BTreeMap::<(u32, u32), Vec<SsaValueV1>>::new();
        for block in shared.reverse_postorder() {
            let mut seen = BTreeSet::new();
            for (_, event) in shared
                .resolved_events(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            {
                let (variable, entry_value, definition) = match *event {
                    SsaResolvedEventV1::Use { variable, value } => (variable, Some(value), None),
                    SsaResolvedEventV1::Define { variable, value } => (variable, None, Some(value)),
                    SsaResolvedEventV1::Kill { variable, previous } => (variable, previous, None),
                };
                let local = variable.get();
                if !shared_promoted.contains(&local) {
                    continue;
                }
                if seen.insert(local)
                    && let Some(value) = entry_value
                {
                    block_entry_values.insert((block.get(), local), value);
                }
                if let Some(value) = definition {
                    definition_values
                        .entry((block.get(), local))
                        .or_default()
                        .push(value);
                }
            }
        }
        let reachable = shared
            .reverse_postorder()
            .iter()
            .map(|block| block.get())
            .collect::<BTreeSet<_>>();
        let retained_initialized_at_entry = retained_local_initialization_entries_v1(
            function,
            &retained_local_slots,
            &reachable,
            max_analysis_work,
            max_analysis_storage,
        )?;
        Ok(Self {
            compiler_issued_bindings,
            implicit_entry_locals,
            ssa_value_locals: shared_promoted,
            promoted,
            live_in,
            block_entry_values,
            entry_definitions,
            definition_values,
            edge_definitions,
            edge_arguments,
            retained_local_slots,
            retained_initialized_at_entry,
        })
    }

    fn live_in(&self, block: u32) -> &[u32] {
        self.live_in.get(&block).map_or(&[], Vec::as_slice)
    }

    fn entry_value(
        &self,
        function: &SemanticFunctionDeclV1,
        block: u32,
        local: u32,
    ) -> Option<SsaValueV1> {
        if block == function.entry().index() {
            return self.entry_definitions.get(&local).copied();
        }
        if self.live_in(block).contains(&local) {
            return Some(SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(block),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(local),
            });
        }
        self.block_entry_values.get(&(block, local)).copied()
    }
}
