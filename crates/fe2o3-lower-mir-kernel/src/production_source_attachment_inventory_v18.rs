// Genuine input-only original attachment census, factored from the tile projection host.
// No tile emitter, output correspondence, optimizer or launch endpoint is admitted here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
enum ScopedTileFailureKindV29 {
    MissingDonor,
    Resource(ArgumentResourceV1),
    Source,
    SemanticSsa,
    SourceLaunch,
    Canonical,
    Occurrences,
    Census,
    Geometry,
    NoTileOccurrences,
    ReplayMismatch,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl From<ArgumentResourceV1> for ScopedTileFailureKindV29 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl From<ProductionSemanticKirErrorV1> for ScopedTileFailureKindV29 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        let summary = match &error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource)
            | ProductionSemanticKirErrorV1::AssertOrigin(
                SemanticKirAssertOriginErrorV1::Resource(resource),
            ) => Self::Resource(*resource),
            ProductionSemanticKirErrorV1::SemanticSsa(_) => Self::SemanticSsa,
            _ => Self::Source,
        };
        // Raw errors may own strings, type trees or provenance after rollback.
        drop(error);
        summary
    }
}

use source_attachment_inventory_v18::*;

mod source_attachment_inventory_v18 {
    use super::*;
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct TileScalarPointV29 {
        pub(super) function: usize,
        pub(super) block: usize,
        pub(super) operation: usize,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum TileScalarSourceV29 {
        FunctionParameter {
            function: usize,
            parameter: usize,
        },
        Block {
            function: usize,
            block: usize,
        },
        BlockParameter {
            function: usize,
            block: usize,
            parameter: usize,
        },
        Operation(TileScalarPointV29),
        Result {
            operation: TileScalarPointV29,
            result: usize,
        },
        Terminator {
            function: usize,
            block: usize,
        },
        Edge {
            function: usize,
            block: usize,
            edge: usize,
        },
    }
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as AttachmentBlockV29,
        CanonicalKirEdgeArgumentCoordinateV1 as AttachmentEdgeArgumentV29,
        CanonicalKirEdgeCoordinateV1 as AttachmentEdgeV29,
        CanonicalKirFunctionCoordinateV1 as AttachmentFunctionV29,
        CanonicalKirUseCoordinateV1 as AttachmentUseV29,
    };
    type TileAttachmentCoreResultV29<T> = Result<T, ScopedTileFailureKindV29>;

    fn tile_attachment_component_v29(
        mut key: TileAttachmentKeyV29,
        component: usize,
    ) -> TileAttachmentKeyV29 {
        key.component = component;
        key
    }
    fn tile_attachment_u32_v29(value: usize) -> TileAttachmentCoreResultV29<u32> {
        u32::try_from(value).map_err(|_| ArgumentResourceV1::Arithmetic.into())
    }
    fn tile_attachment_block_v29<'a>(
        graph: &'a Module,
        index: TileAttachmentIndexV18<'_, '_>,
        function: usize,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentCoreResultV29<(AttachmentBlockV29, &'a BasicBlock)> {
        let coordinate = index.block(
            AttachmentFunctionV29(tile_attachment_u32_v29(function)?),
            block,
            budget,
        )?;
        let block = graph
            .functions
            .get(function)
            .and_then(|f| f.body.as_ref())
            .and_then(|b| b.blocks.get(coordinate.block as usize))
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        Ok((coordinate, block))
    }
    fn tile_attachment_values_v29<'a>(
        coordinates: &'a OwnedInstanceCoordinatesV1,
        range: &std::ops::Range<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentCoreResultV29<&'a [ValueId]> {
        let count = range
            .end
            .checked_sub(range.start)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(argument_sum_v1(&[count, 1])?)?;
        coordinates
            .values
            .rows
            .get(range.clone())
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
    }
    fn tile_attachment_control_v29<'a>(
        coordinates: &'a OwnedInstanceCoordinatesV1,
        instance: ProductionCallInstanceIdV1,
        original_block: BlockId,
        semantic_block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentCoreResultV29<&'a InstanceControlV1> {
        budget.charge_work(argument_sum_v1(&[coordinates.controls.rows.len(), 1])?)?;
        let mut rows = coordinates.controls.rows.iter().filter(|row| {
            row.instance == instance
                && row.original_block == original_block
                && row.semantic_block == Some(semantic_block)
                && matches!(
                    row.origin,
                    InstanceControlOriginV1::Retained
                        | InstanceControlOriginV1::ExpandedReturn { .. }
                )
        });
        let row = rows
            .next()
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if rows.next().is_some() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(row)
    }
    fn tile_attachment_call_span_v29(
        coordinates: &OwnedInstanceCoordinatesV1,
        anchor: &InstanceCallAnchorV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentCoreResultV29<InstancePhysicalSpanV1> {
        budget.charge_work(argument_sum_v1(&[coordinates.spans.rows.len(), 1])?)?;
        let mut rows = coordinates.spans.rows.iter().filter_map(|row| {
            if row.instance != anchor.instance {
                return None;
            }
            match row.source {
                InstanceSpanSourceV1::Terminator(source)
                    if source.correspondence_owner == anchor.source.correspondence_owner
                        && source.semantic_function == anchor.source.semantic_function
                        && source.semantic_block == anchor.source.semantic_block =>
                {
                    Some(InstanceSpanSourceV1::Terminator(source).coordinates().2)
                }
                _ => None,
            }
        });
        let row = rows
            .next()
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if rows.next().is_some() || row.block != anchor.physical.block {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(row)
    }

    impl TileAttachmentWalkV29<'_, '_, '_, '_> {
        fn core_definitions_v29(
            &mut self,
            key: TileAttachmentKeyV29,
            values: &[ValueId],
        ) -> TileAttachmentCoreResultV29<()> {
            if values.is_empty() {
                return self.emit(key, TileAttachmentLocationV29::NoOutput);
            }
            for (component, value) in values.iter().enumerate() {
                self.definition(tile_attachment_component_v29(key, component), *value)?;
            }
            Ok(())
        }
        fn core_return_uses_v29(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
            expected: &[ValueId],
        ) -> TileAttachmentCoreResultV29<()> {
            let (coordinate, actual) = tile_attachment_block_v29(
                self.graph,
                self.index,
                self.root.function_ordinal,
                block,
                self.budget,
            )?;
            let terminator = actual
                .terminator
                .as_ref()
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            self.budget
                .charge_work(argument_sum_v1(&[expected.len(), 1])?)?;
            match terminator {
                Terminator::Return { values } if values == expected => {
                    for component in 0..expected.len() {
                        self.canonical_use(
                            tile_attachment_component_v29(key, component),
                            AttachmentUseV29::TerminatorOperand {
                                block: coordinate,
                                operand: tile_attachment_u32_v29(component)?,
                            },
                        )?;
                    }
                }
                Terminator::Branch { arguments, .. } if arguments == expected => {
                    for component in 0..expected.len() {
                        self.canonical_edge_argument(
                            tile_attachment_component_v29(key, component),
                            AttachmentEdgeArgumentV29 {
                                edge: AttachmentEdgeV29 {
                                    source: coordinate,
                                    successor: 0,
                                },
                                argument: tile_attachment_u32_v29(component)?,
                            },
                        )?;
                    }
                }
                _ => return Err(ScopedTileFailureKindV29::ReplayMismatch),
            }
            if expected.is_empty() {
                self.emit(key, TileAttachmentLocationV29::NoOutput)?;
            }
            Ok(())
        }
        fn core_control_v29(
            &mut self,
            ordinal: usize,
            row: &InstanceControlV1,
        ) -> TileAttachmentCoreResultV29<()> {
            use TileAttachmentFamilyV29 as Family;
            use TileAttachmentFieldV29 as Field;
            let InstanceControlV1 {
                instance,
                original_block: _,
                semantic_block: _,
                physical_block,
                origin: _,
                return_values,
                expected_branch,
            } = row;
            let instance = instance.index();
            let key = |field| self.key(Family::InstanceControls, instance, ordinal, field);
            let block_key = key(Field::PhysicalBlock);
            let term_key = key(Field::Terminator);
            let edge_key = key(Field::Edge);
            let argument_key = key(Field::EdgeArgument);
            let definition_key = key(Field::ReturnDefinition);
            let use_key = key(Field::ReturnUse);
            let target_key = key(Field::ExpectedTarget);
            let expected_key = key(Field::ExpectedArgument);
            self.block(block_key, *physical_block)?;
            self.terminator(term_key, *physical_block)?;
            let (coordinate, block) = tile_attachment_block_v29(
                self.graph,
                self.index,
                self.root.function_ordinal,
                *physical_block,
                self.budget,
            )?;
            let terminator = block
                .terminator
                .as_ref()
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let mut edge = 0usize;
            terminator.try_visit_edges_v1(|_, _| -> TileAttachmentCoreResultV29<()> {
                self.budget.charge_work(1)?;
                self.edge(
                    tile_attachment_component_v29(edge_key, edge),
                    *physical_block,
                    edge,
                )?;
                edge = argument_sum_v1(&[edge, 1])?;
                Ok(())
            })?;
            if edge == 0 {
                self.emit(edge_key, TileAttachmentLocationV29::NoOutput)?;
            }
            let (mut edge, mut component) = (0usize, 0usize);
            terminator.try_visit_edges_v1(|_, arguments| -> TileAttachmentCoreResultV29<()> {
                self.budget.charge_work(1)?;
                for argument in 0..arguments.len() {
                    self.canonical_edge_argument(
                        tile_attachment_component_v29(argument_key, component),
                        AttachmentEdgeArgumentV29 {
                            edge: AttachmentEdgeV29 {
                                source: coordinate,
                                successor: tile_attachment_u32_v29(edge)?,
                            },
                            argument: tile_attachment_u32_v29(argument)?,
                        },
                    )?;
                    component = argument_sum_v1(&[component, 1])?;
                }
                edge = argument_sum_v1(&[edge, 1])?;
                Ok(())
            })?;
            if component == 0 {
                self.emit(argument_key, TileAttachmentLocationV29::NoOutput)?;
            }
            if let Some(range) = return_values {
                let values =
                    tile_attachment_values_v29(&self.root.coordinates, range, self.budget)?;
                self.core_definitions_v29(definition_key, values)?;
                self.core_return_uses_v29(use_key, *physical_block, values)?;
            } else {
                self.emit(definition_key, TileAttachmentLocationV29::NoOutput)?;
                self.emit(use_key, TileAttachmentLocationV29::NoOutput)?;
            }
            if let Some((target, range)) = expected_branch {
                self.block(target_key, *target)?;
                let values =
                    tile_attachment_values_v29(&self.root.coordinates, range, self.budget)?;
                self.core_definitions_v29(expected_key, values)?;
            } else {
                self.emit(target_key, TileAttachmentLocationV29::NoOutput)?;
                self.emit(expected_key, TileAttachmentLocationV29::NoOutput)?;
            }
            Ok(())
        }
        fn core_call_v29(
            &mut self,
            ordinal: usize,
            row: &InstanceCallAnchorV1,
        ) -> TileAttachmentCoreResultV29<()> {
            use TileAttachmentFamilyV29 as Family;
            use TileAttachmentFieldV29 as Field;
            let InstanceCallAnchorV1 {
                instance,
                source,
                physical,
                arguments,
                results,
                removed,
            } = row;
            let SemanticKirCallReturnV1 {
                correspondence_owner: _,
                semantic_function: _,
                semantic_block: _,
                kind,
            } = *source;
            let (arguments_first, call_operation, destination_end, destination, normal) = match kind
            {
                SemanticKirCallReturnKindV1::Call {
                    arguments_first,
                    call_operation,
                    destination_end,
                    destination,
                    ..
                } => (
                    arguments_first,
                    call_operation,
                    destination_end,
                    Some(destination),
                    true,
                ),
                SemanticKirCallReturnKindV1::NoNormalReturnCall {
                    arguments_first,
                    call_operation,
                    destination,
                } => (
                    arguments_first,
                    call_operation,
                    call_operation
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                    destination,
                    false,
                ),
                SemanticKirCallReturnKindV1::Return { .. } => {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
            };
            let span = tile_attachment_call_span_v29(&self.root.coordinates, row, self.budget)?;
            let span_end = span
                .first
                .checked_add(span.count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if span.first > arguments_first
                || arguments_first > call_operation
                || destination_end <= call_operation
                || destination_end > span_end
            {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let after_call = call_operation
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let key = |field| self.key(Family::InstanceCalls, instance.index(), ordinal, field);
            let argument_preparation = key(Field::ArgumentPreparation);
            let call_site = key(Field::CallSite);
            let destination_preparation = key(Field::DestinationPreparation);
            let destination_range = key(Field::DestinationRange);
            let destination_pointer = key(Field::DestinationPointer);
            let argument_definition = key(Field::ArgumentDefinition);
            let argument_use = key(Field::ArgumentUse);
            let result_definition = key(Field::ResultDefinition);
            self.emitted_range(
                argument_preparation,
                *instance,
                physical.block,
                arguments_first,
                call_operation,
            )?;
            if *removed {
                self.emit(call_site, TileAttachmentLocationV29::Tombstone)?;
            } else {
                self.emitted_operation(call_site, *instance, physical.block, call_operation)?;
            }
            self.emitted_range(
                destination_preparation,
                *instance,
                physical.block,
                span.first,
                arguments_first,
            )?;
            if normal {
                self.emitted_range(
                    destination_range,
                    *instance,
                    physical.block,
                    after_call,
                    destination_end,
                )?;
            } else {
                self.emit(destination_range, TileAttachmentLocationV29::NoOutput)?;
            }
            match destination {
                None | Some(SemanticKirCallDestinationV1::Local) => {
                    self.emit(destination_pointer, TileAttachmentLocationV29::NoOutput)?
                }
                Some(
                    SemanticKirCallDestinationV1::Retained { pointer, access: _ }
                    | SemanticKirCallDestinationV1::Projected { pointer, access: _ },
                ) => self.definition(destination_pointer, pointer)?,
            }
            let arguments =
                tile_attachment_values_v29(&self.root.coordinates, arguments, self.budget)?;
            self.core_definitions_v29(argument_definition, arguments)?;
            if arguments.is_empty() {
                self.emit(argument_use, TileAttachmentLocationV29::NoOutput)?;
            }
            for component in 0..arguments.len() {
                let key = tile_attachment_component_v29(argument_use, component);
                if *removed {
                    self.emit(key, TileAttachmentLocationV29::Tombstone)?;
                } else {
                    self.emitted_use(
                        key,
                        *instance,
                        physical.block,
                        call_operation,
                        tile_attachment_u32_v29(component)?,
                    )?;
                }
            }
            let results = tile_attachment_values_v29(&self.root.coordinates, results, self.budget)?;
            self.core_definitions_v29(result_definition, results)?;
            Ok(())
        }
        fn core_return_v29(
            &mut self,
            ordinal: usize,
            row: &InstanceReturnAnchorV1,
        ) -> TileAttachmentCoreResultV29<()> {
            use TileAttachmentFamilyV29 as Family;
            use TileAttachmentFieldV29 as Field;
            let InstanceReturnAnchorV1 {
                instance,
                source,
                original_block,
            } = row;
            let SemanticKirCallReturnKindV1::Return { components } = source.kind else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            let coordinates = &self.root.coordinates;
            let control = tile_attachment_control_v29(
                coordinates,
                *instance,
                *original_block,
                source.semantic_block,
                self.budget,
            )?;
            let range = components.range()?;
            let components = coordinates
                .components
                .rows
                .get(range)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let returned = tile_attachment_values_v29(
                coordinates,
                control
                    .return_values
                    .as_ref()
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?,
                self.budget,
            )?;
            if components.len() != returned.len() {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let key = |field| self.key(Family::InstanceReturns, instance.index(), ordinal, field);
            let site = key(Field::ReturnSite);
            let input_key = key(Field::ReturnComponentInput);
            let conversion_key = key(Field::ReturnComponentConversion);
            let output_key = key(Field::ReturnComponentOutput);
            let use_key = key(Field::ReturnComponentUse);
            self.terminator(site, control.physical_block)?;
            if components.is_empty() {
                self.emit(input_key, TileAttachmentLocationV29::NoOutput)?;
            }
            for (component, row) in components.iter().enumerate() {
                let CallResultComponentV1::Return {
                    input,
                    conversion: _,
                } = *row
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                self.definition(tile_attachment_component_v29(input_key, component), input)?;
            }
            if components.is_empty() {
                self.emit(conversion_key, TileAttachmentLocationV29::NoOutput)?;
            }
            for (component, row) in components.iter().enumerate() {
                let CallResultComponentV1::Return {
                    input: _,
                    conversion,
                } = *row
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                let key = tile_attachment_component_v29(conversion_key, component);
                match conversion {
                    Some(operation) => {
                        self.emitted_operation(key, *instance, *original_block, operation)?
                    }
                    None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                }
            }
            self.core_definitions_v29(output_key, returned)?;
            self.core_return_uses_v29(use_key, control.physical_block, returned)
        }
        fn core_transport_v29(
            &mut self,
            ordinal: usize,
            row: &InstanceCallAnchorV1,
        ) -> TileAttachmentCoreResultV29<()> {
            use TileAttachmentFamilyV29 as Family;
            use TileAttachmentFieldV29 as Field;
            let transport = match row.source.kind {
                SemanticKirCallReturnKindV1::Call { transport, .. } => transport,
                SemanticKirCallReturnKindV1::NoNormalReturnCall { .. } => {
                    CallComponentSpanV1::EMPTY
                }
                SemanticKirCallReturnKindV1::Return { .. } => {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
            };
            let coordinates = &self.root.coordinates;
            let components = coordinates
                .components
                .rows
                .get(transport.range()?)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let key = |field| {
                self.key(
                    Family::InstanceReturns,
                    row.instance.index(),
                    ordinal,
                    field,
                )
            };
            let conversion_key = key(Field::TransportComponentConversion);
            let output_key = key(Field::TransportComponentOutput);
            let use_key = key(Field::TransportComponentUse);
            if components.is_empty() {
                for key in [conversion_key, output_key, use_key] {
                    self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                }
                return Ok(());
            }
            let control = tile_attachment_control_v29(
                coordinates,
                row.instance,
                row.physical.block,
                row.source.semantic_block,
                self.budget,
            )?;
            let (block_coordinate, block) = tile_attachment_block_v29(
                self.graph,
                self.index,
                self.root.function_ordinal,
                control.physical_block,
                self.budget,
            )?;
            let Some(Terminator::Branch { arguments, .. }) = &block.terminator else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            for (component, row_component) in components.iter().enumerate() {
                let CallResultComponentV1::Transport {
                    slot: _,
                    conversion,
                } = *row_component
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                let key = tile_attachment_component_v29(conversion_key, component);
                match conversion {
                    Some(operation) => {
                        self.emitted_operation(key, row.instance, row.physical.block, operation)?
                    }
                    None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                }
            }
            for (component, row_component) in components.iter().enumerate() {
                let CallResultComponentV1::Transport {
                    slot,
                    conversion: _,
                } = *row_component
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                let value = arguments
                    .get(slot as usize)
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                self.definition(tile_attachment_component_v29(output_key, component), *value)?;
            }
            for (component, row_component) in components.iter().enumerate() {
                let CallResultComponentV1::Transport {
                    slot,
                    conversion: _,
                } = *row_component
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                self.canonical_edge_argument(
                    tile_attachment_component_v29(use_key, component),
                    AttachmentEdgeArgumentV29 {
                        edge: AttachmentEdgeV29 {
                            source: block_coordinate,
                            successor: 0,
                        },
                        argument: slot,
                    },
                )?;
            }
            Ok(())
        }
        fn walk_core_v29(&mut self) -> TileAttachmentCoreResultV29<()> {
            use TileAttachmentFamilyV29 as Family;
            use TileAttachmentFieldV29 as Field;
            let root = self.root;
            let OwnedInstanceCoordinatesV1 {
                semantic_sha256: _,
                ssa: _,
                root: _,
                sources,
                seeds,
                spans,
                controls,
                anchors,
                returns,
                components: _,
                values: _,
                inline: _, // Original source/spans only; no SSA value attachments.
                storage: _,
            } = &root.coordinates;
            for source in &sources.rows {
                self.budget
                    .charge_work(argument_sum_v1(&[spans.rows.len(), 1])?)?;
                for (row, span) in spans
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.instance == source.instance)
                {
                    let key = self.key(
                        Family::InstanceSpans,
                        source.instance.index(),
                        row,
                        Field::Span,
                    );
                    self.mapped_span(key, row)?;
                    let InstanceMappedSpanV1 {
                        instance: _,
                        source: _,
                        segments: _,
                        removed_call: _,
                    } = span;
                }
            }
            for source in &sources.rows {
                self.budget
                    .charge_work(argument_sum_v1(&[seeds.rows.len(), 1])?)?;
                for (row, seed) in seeds
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.instance == source.instance)
                {
                    let InstanceSeedV1 {
                        instance,
                        container: _,
                        function_name: _,
                        parameters,
                        inline: _,
                    } = seed;
                    let key = self.key(
                        Family::InstanceSeeds,
                        instance.index(),
                        row,
                        Field::Parameters,
                    );
                    let values =
                        tile_attachment_values_v29(&root.coordinates, parameters, self.budget)?;
                    self.core_definitions_v29(key, values)?;
                }
            }
            for source in &sources.rows {
                self.budget
                    .charge_work(argument_sum_v1(&[controls.rows.len(), 1])?)?;
                for (row, control) in controls
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.instance == source.instance)
                {
                    self.core_control_v29(row, control)?;
                }
            }
            for source in &sources.rows {
                self.budget
                    .charge_work(argument_sum_v1(&[anchors.rows.len(), 1])?)?;
                for (row, anchor) in anchors
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.instance == source.instance)
                {
                    self.core_call_v29(row, anchor)?;
                }
            }
            let count = returns.rows.len().max(anchors.rows.len());
            for source in &sources.rows {
                self.budget.charge_work(argument_sum_v1(&[count, 1])?)?;
                for row in 0..count {
                    if let Some(ret) = returns
                        .rows
                        .get(row)
                        .filter(|r| r.instance == source.instance)
                    {
                        self.core_return_v29(row, ret)?;
                    }
                    if let Some(call) = anchors
                        .rows
                        .get(row)
                        .filter(|r| r.instance == source.instance)
                    {
                        self.core_transport_v29(row, call)?;
                    }
                }
            }
            Ok(())
        }
    }

    // Input-only attachment normalization. No output graph, emitter plan or origin
    // table is available to this visitor.

    pub(super) fn tile_store_payload_operand_v18(
        operation: &Operation,
        expected_pointer: ValueId,
        expected_value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentResultV29<u32> {
        budget.charge_work(1)?;
        let (actual_pointer, actual_value, expected, count) = match &operation.kind {
            OperationKind::Store { pointer, value, .. } => {
                (*pointer, *value, [*pointer, *value, ValueId(0)], 2usize)
            }
            OperationKind::GuardedStore {
                pointer,
                predicate,
                value,
                ..
            } => (*pointer, *value, [*pointer, *predicate, *value], 3usize),
            _ => return Err(ScopedTileFailureKindV29::ReplayMismatch),
        };
        if actual_pointer != expected_pointer
            || actual_value != expected_value
            || !operation.results.is_empty()
        {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        // Validate every actual operand occurrence, including equal-valued operands.
        let mut visited = 0usize;
        let mut rhs = None;
        operation
            .kind
            .try_visit_operands(|operand| -> TileAttachmentResultV29<()> {
                budget.charge_work(1)?;
                if visited >= count || expected[visited] != operand {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                rhs = Some(tile_attachment_u32_v29(visited)?);
                visited = visited
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                Ok(())
            })?;
        if visited != count {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        rhs.ok_or(ScopedTileFailureKindV29::ReplayMismatch)
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum TileAttachmentFamilyV29 {
        InstanceSpans,
        InstanceSeeds,
        InstanceControls,
        InstanceCalls,
        InstanceReturns,
        RawSidecar,
        SourceSlot,
        MemoryAnchor,
        PrivateArray,
        Lifecycle,
        Assertion,
        TerminalFailure,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum TileAttachmentFieldV29 {
        Span,
        Parameters,
        PhysicalBlock,
        Terminator,
        Edge,
        EdgeArgument,
        ReturnDefinition,
        ReturnUse,
        ExpectedTarget,
        ExpectedArgument,
        ArgumentPreparation,
        CallSite,
        DestinationPreparation,
        DestinationRange,
        DestinationPointer,
        ArgumentDefinition,
        ArgumentUse,
        ResultDefinition,
        ReturnSite,
        ReturnComponentInput,
        ReturnComponentConversion,
        ReturnComponentOutput,
        ReturnComponentUse,
        TransportComponentConversion,
        TransportComponentOutput,
        TransportComponentUse,
        RawBlock,
        RawStatementSpan,
        RawTerminatorSpan,
        RawSyntheticSpan,
        RawInvocationSpan,
        RawInvocationPreheader,
        RawInvocationEntry,
        RawInvocationTerminator,
        RawInvocationEdge,
        RawInvocationArgument,
        RawInvocationInputMap,
        RawInvocationInput,
        RawInvocationParameter,
        RawInvocationOutput,
        RawInvocationConversion,
        RawParameter,
        RawParameterComponent,
        RawIgnoredParameter,
        RawGeneratedInput,
        RawGeneratedOutput,
        RawCallArguments,
        RawCallOperation,
        RawCallDestination,
        RawCallDestinationPointer,
        RawNoNormalReturnTerminator,
        RawReturnTerminator,
        RawReturnInput,
        RawReturnConversion,
        RawTransportArgument,
        RawTransportConversion,
        SlotRawPointer,
        SlotPointer,
        SlotCount,
        SlotCountLocation,
        SlotAllocation,
        MemoryPosition,
        MemoryPointer,
        MemoryLoadResult,
        MemoryStoreValue,
        MemoryStoreUse,
        ArrayCountLocation,
        ArrayAllocation,
        ArrayPointer,
        ArrayCount,
        ArraySourceRange,
        ArrayOriginalIndex,
        ArrayDirectDefinition,
        ArrayLiteralValue,
        ArrayLiteralDefinition,
        ArrayOffsetLocation,
        ArrayGepLocation,
        ArrayMemoryLocation,
        ArrayOffset,
        ArrayGep,
        LifecycleOriginalGap,
        LifecycleBeforeGap,
        LifecycleOperation,
        LifecycleOperand,
        LifecycleResult,
        AssertSourceRange,
        AssertSourceBlock,
        AssertSuccessBlock,
        AssertFailureBlock,
        AssertCapturedCondition,
        AssertCapturedArgument,
        AssertConditionUse,
        AssertConditionDefinition,
        AssertSuccessEdge,
        AssertFailureEdge,
        AssertSuccessArgument,
        FailureSourceBlock,
        FailureSourceEdge,
        FailureOriginalTarget,
        FailureOriginalDiagnostic,
        FailureBlock,
        FailureGap,
        FailureCleanup,
        FailureOperand,
        FailureDiagnostic,
        FailureTerminator,
        ObjectOperand,
        ObjectOperandUse,
        ObjectResult,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct TileAttachmentKeyV29 {
        pub(super) root: usize,
        pub(super) family: TileAttachmentFamilyV29,
        // Original source instance ID, never the compact sidecar ordinal.
        pub(super) instance: usize,
        pub(super) row: usize,
        pub(super) field: TileAttachmentFieldV29,
        pub(super) component: usize,
        pub(super) part: usize,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum TileAttachmentLocationV29 {
        Origin(TileScalarSourceV29),
        Gap(TileScalarPointV29),
        Use(fe2o3_kernel_ir::CanonicalKirUseCoordinateV1),
        EdgeArgument(fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1),
        Tombstone,
        NoOutput,
    }
    type TileAttachmentResultV29<T> = Result<T, ScopedTileFailureKindV29>;

    #[derive(Clone, Copy)]
    enum TileAttachmentIndexV18<'i, 'g> {
        Legacy(&'i AssertGraphIndexV1<'g>),
        Inventory(&'i fe2o3_kernel_analysis::CanonicalKirInventoryV18<'g>),
    }

    fn tile_attachment_inventory_error_v18(
        error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
    ) -> ScopedTileFailureKindV29 {
        match error {
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
            _ => ScopedTileFailureKindV29::ReplayMismatch,
        }
    }

    impl TileAttachmentIndexV18<'_, '_> {
        fn definition(
            self,
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            value: ValueId,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> TileAttachmentResultV29<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1> {
            match self {
                Self::Legacy(index) => Ok(index
                    .definition(function, value, budget)
                    .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?
                    .coordinate),
                Self::Inventory(index) => index
                    .definition_for_value(function, value, budget)
                    .map_err(tile_attachment_inventory_error_v18)?
                    .map(|row| row.coordinate)
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch),
            }
        }

        fn block(
            self,
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            block: BlockId,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> TileAttachmentResultV29<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1> {
            match self {
                Self::Legacy(index) => Ok(index
                    .block(function, block, budget)
                    .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?),
                Self::Inventory(index) => index
                    .block_for_id(function, block, budget)
                    .map_err(tile_attachment_inventory_error_v18)?
                    .map(|row| row.coordinate)
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch),
            }
        }
    }

    struct TileAttachmentWalkV29<'a, 'i, 'b, 'work> {
        pending: &'a ProductionPendingScopedSourceOwnerV29,
        root_ordinal: usize,
        root: &'a ScopedModuleRootV29,
        graph: &'a Module,
        index: TileAttachmentIndexV18<'i, 'a>,
        budget: &'b mut ArgumentBudgetV1<'work>,
        visit: &'b mut dyn FnMut(
            TileAttachmentKeyV29,
            TileAttachmentLocationV29,
            &mut ArgumentBudgetV1<'work>,
        ) -> TileAttachmentResultV29<()>,
    }
    impl TileAttachmentWalkV29<'_, '_, '_, '_> {
        fn key(
            &self,
            family: TileAttachmentFamilyV29,
            instance: usize,
            row: usize,
            field: TileAttachmentFieldV29,
        ) -> TileAttachmentKeyV29 {
            TileAttachmentKeyV29 {
                root: self.root_ordinal,
                family,
                instance,
                row,
                field,
                component: 0,
                part: 0,
            }
        }
        fn emit(
            &mut self,
            key: TileAttachmentKeyV29,
            location: TileAttachmentLocationV29,
        ) -> TileAttachmentResultV29<()> {
            self.budget.charge_work(1)?;
            (self.visit)(key, location, self.budget)
        }
        fn body(&self) -> TileAttachmentResultV29<&FunctionBody> {
            self.graph
                .functions
                .get(self.root.function_ordinal)
                .and_then(|f| f.body.as_ref())
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
        }
        fn block_ordinal(&mut self, block: BlockId) -> TileAttachmentResultV29<usize> {
            self.budget.charge_work(self.body()?.blocks.len())?;
            self.body()?
                .blocks
                .iter()
                .position(|b| b.id == block)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
        }
        fn block(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
        ) -> TileAttachmentResultV29<()> {
            let block = self.block_ordinal(block)?;
            self.emit(
                key,
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Block {
                    function: self.root.function_ordinal,
                    block,
                }),
            )
        }
        fn terminator(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
        ) -> TileAttachmentResultV29<()> {
            let block = self.block_ordinal(block)?;
            self.emit(
                key,
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Terminator {
                    function: self.root.function_ordinal,
                    block,
                }),
            )
        }
        fn edge(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
            edge: usize,
        ) -> TileAttachmentResultV29<()> {
            let block = self.block_ordinal(block)?;
            self.emit(
                key,
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Edge {
                    function: self.root.function_ordinal,
                    block,
                    edge,
                }),
            )
        }
        fn definition(
            &mut self,
            key: TileAttachmentKeyV29,
            value: ValueId,
        ) -> TileAttachmentResultV29<()> {
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(self.root.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let definition = self.index.definition(function, value, self.budget)?;
            self.canonical_definition(key, definition)
        }
        fn canonical_definition(
            &mut self,
            key: TileAttachmentKeyV29,
            definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
        ) -> TileAttachmentResultV29<()> {
            use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as D;
            let source = match definition {
                D::FunctionArgument { function, argument } => {
                    TileScalarSourceV29::FunctionParameter {
                        function: function.0 as usize,
                        parameter: argument as usize,
                    }
                }
                D::BlockArgument { block, argument } => TileScalarSourceV29::BlockParameter {
                    function: block.function.0 as usize,
                    block: block.block as usize,
                    parameter: argument as usize,
                },
                D::Result { operation, result } => TileScalarSourceV29::Result {
                    operation: TileScalarPointV29 {
                        function: operation.block.function.0 as usize,
                        block: operation.block.block as usize,
                        operation: operation.operation as usize,
                    },
                    result: result as usize,
                },
            };
            self.emit(key, TileAttachmentLocationV29::Origin(source))
        }
        fn canonical_use(
            &mut self,
            key: TileAttachmentKeyV29,
            used: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
        ) -> TileAttachmentResultV29<()> {
            self.emit(key, TileAttachmentLocationV29::Use(used))
        }
        fn canonical_edge(
            &mut self,
            key: TileAttachmentKeyV29,
            edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
        ) -> TileAttachmentResultV29<()> {
            self.emit(
                key,
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Edge {
                    function: edge.source.function.0 as usize,
                    block: edge.source.block as usize,
                    edge: edge.successor as usize,
                }),
            )
        }
        fn canonical_edge_argument(
            &mut self,
            key: TileAttachmentKeyV29,
            argument: fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1,
        ) -> TileAttachmentResultV29<()> {
            self.emit(key, TileAttachmentLocationV29::EdgeArgument(argument))
        }
        fn operation_v(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
            ordinal: u32,
        ) -> TileAttachmentResultV29<()> {
            let block = self.block_ordinal(block)?;
            if ordinal as usize >= self.body()?.blocks[block].operations.len() {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            self.emit(
                key,
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(
                    TileScalarPointV29 {
                        function: self.root.function_ordinal,
                        block,
                        operation: ordinal as usize,
                    },
                )),
            )
        }
        fn lifecycle_ordinal(
            &mut self,
            block: BlockId,
            p: u32,
            gap: bool,
        ) -> TileAttachmentResultV29<u32> {
            let mut ordinal = p;
            for insertion in &self.root.insertions {
                self.budget.charge_work(1)?;
                if insertion.before.block == block
                    && (insertion.before.first < p || (!gap && insertion.before.first == p))
                {
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            let terminal = terminal_failure_ordinal_v18(
                self.root.terminal_failures.as_ref(),
                block,
                p,
                gap,
                self.budget,
            )?;
            ordinal
                .checked_add(
                    terminal
                        .checked_sub(p)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )
                .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
        }
        fn operation_c(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
            p: u32,
        ) -> TileAttachmentResultV29<()> {
            let ordinal = self.lifecycle_ordinal(block, p, false)?;
            self.operation_v(key, block, ordinal)
        }
        fn gap_c(
            &mut self,
            key: TileAttachmentKeyV29,
            block: BlockId,
            p: u32,
        ) -> TileAttachmentResultV29<()> {
            let ordinal = self.lifecycle_ordinal(block, p, true)?;
            let block = self.block_ordinal(block)?;
            if ordinal as usize > self.body()?.blocks[block].operations.len() {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            self.emit(
                key,
                TileAttachmentLocationV29::Gap(TileScalarPointV29 {
                    function: self.root.function_ordinal,
                    block,
                    operation: ordinal as usize,
                }),
            )
        }

        fn emitted_point(
            &mut self,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            p: u32,
            gap: bool,
        ) -> TileAttachmentResultV29<Option<(BlockId, u32)>> {
            ScopedEmittedPointsV29 {
                coordinates: &self.root.coordinates,
                relocation: &self.root.slot_relocation,
                budget: self.budget,
            }
            .emitted_point(instance, block, p, gap)
        }
        fn emitted_operation(
            &mut self,
            key: TileAttachmentKeyV29,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            p: u32,
        ) -> TileAttachmentResultV29<()> {
            match self.emitted_point(instance, block, p, false)? {
                Some((block, p)) => self.operation_c(key, block, p),
                None => self.emit(key, TileAttachmentLocationV29::Tombstone),
            }
        }
        fn emitted_gap(
            &mut self,
            key: TileAttachmentKeyV29,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            p: u32,
        ) -> TileAttachmentResultV29<()> {
            let (block, p) = self
                .emitted_point(instance, block, p, true)?
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            self.gap_c(key, block, p)
        }
        fn emitted_range(
            &mut self,
            mut key: TileAttachmentKeyV29,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            first: u32,
            end: u32,
        ) -> TileAttachmentResultV29<()> {
            if end < first {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            if first == end {
                return self.emitted_gap(key, instance, block, first);
            }
            for p in first..end {
                self.emitted_operation(key, instance, block, p)?;
                key.part = key
                    .part
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            Ok(())
        }
        fn emitted_use(
            &mut self,
            key: TileAttachmentKeyV29,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            p: u32,
            operand: u32,
        ) -> TileAttachmentResultV29<()> {
            let Some((block, p)) = self.emitted_point(instance, block, p, false)? else {
                return self.emit(key, TileAttachmentLocationV29::Tombstone);
            };
            let operation = self.lifecycle_ordinal(block, p, false)?;
            let block = u32::try_from(self.block_ordinal(block)?)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(self.root.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            self.canonical_use(
                key,
                fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 { function, block },
                        operation,
                    },
                    operand,
                },
            )
        }
        fn emitted_store_payload_use_v18(
            &mut self,
            key: TileAttachmentKeyV29,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            position: u32,
            pointer: ValueId,
            value: ValueId,
        ) -> TileAttachmentResultV29<()> {
            let (physical_block, physical_position) = self
                .emitted_point(instance, block, position, false)?
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let ordinal = self.lifecycle_ordinal(physical_block, physical_position, false)?;
            let block_ordinal = self.block_ordinal(physical_block)?;
            let operation = self
                .graph
                .functions
                .get(self.root.function_ordinal)
                .and_then(|function| function.body.as_ref())
                .and_then(|body| body.blocks.get(block_ordinal))
                .and_then(|block| block.operations.get(ordinal as usize))
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let operand = tile_store_payload_operand_v18(operation, pointer, value, self.budget)?;
            self.emitted_use(key, instance, block, position, operand)
        }

        fn check_emitted_object_v29(
            &mut self,
            instance: ProductionCallInstanceIdV1,
            block: BlockId,
            position: u32,
            payload: &ScopedObjectPayloadV29,
        ) -> TileAttachmentResultV29<()> {
            let (block, position) = self
                .emitted_point(instance, block, position, false)?
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let position = self.lifecycle_ordinal(block, position, false)?;
            let block = self.block_ordinal(block)?;
            let operation = self
                .graph
                .functions
                .get(self.root.function_ordinal)
                .and_then(|function| function.body.as_ref())
                .and_then(|body| body.blocks.get(block))
                .and_then(|block| block.operations.get(position as usize))
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            payload.check_operation(operation, self.budget)?;
            Ok(())
        }

        fn mapped_span(
            &mut self,
            mut key: TileAttachmentKeyV29,
            row: usize,
        ) -> TileAttachmentResultV29<()> {
            let span = *self
                .root
                .coordinates
                .spans
                .rows
                .get(row)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            if span.segments == [None, None] {
                return self.emit(
                    key,
                    if span.removed_call.is_some() {
                        TileAttachmentLocationV29::Tombstone
                    } else {
                        TileAttachmentLocationV29::NoOutput
                    },
                );
            }
            for segment in span.segments.into_iter().flatten() {
                let end = segment
                    .first
                    .checked_add(segment.count)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                if segment.count == 0 {
                    self.gap_c(key, segment.block, segment.first)?;
                    key.part = key
                        .part
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                for p in segment.first..=end {
                    for insertion in &self.root.insertions {
                        self.budget.charge_work(1)?;
                        if insertion.source_span == row
                            && insertion.before.block == segment.block
                            && insertion.before.first == p
                        {
                            self.operation_v(key, insertion.after.block, insertion.after.first)?;
                            key.part = key
                                .part
                                .checked_add(1)
                                .ok_or(ArgumentResourceV1::Arithmetic)?;
                        }
                    }
                    if p < end {
                        self.operation_c(key, segment.block, p)?;
                        key.part = key
                            .part
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                }
            }
            Ok(())
        }
    }

    pub(super) fn visit_source_attachment_inventory_v18<'work>(
        pending: &ProductionPendingScopedSourceOwnerV29,
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        mut visit: impl FnMut(
            TileAttachmentKeyV29,
            TileAttachmentLocationV29,
            &mut ArgumentBudgetV1<'work>,
        ) -> TileAttachmentResultV29<()>,
    ) -> TileAttachmentResultV29<()> {
        budget.charge_work(2)?;
        if !inventory.belongs_to(&pending.inner.pending.graph)
            || pending.inner.pending.ledger != budget.work_ledger_identity_v1()
        {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        visit_pending_attachment_sources_v18(
            pending,
            TileAttachmentIndexV18::Inventory(inventory),
            budget,
            &mut visit,
        )
    }

    fn visit_pending_attachment_sources_v18<'a, 'work>(
        pending: &'a ProductionPendingScopedSourceOwnerV29,
        index: TileAttachmentIndexV18<'_, 'a>,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: &mut impl FnMut(
            TileAttachmentKeyV29,
            TileAttachmentLocationV29,
            &mut ArgumentBudgetV1<'work>,
        ) -> TileAttachmentResultV29<()>,
    ) -> TileAttachmentResultV29<()> {
        let graph = pending.inner.pending.graph.module();
        for (root_ordinal, root) in pending.inner.pending.roots.iter().enumerate() {
            let ScopedModuleRootV29 {
                function_ordinal: _,
                sidecars: _,
                active_instances: _,
                coordinates: _,
                // Original rvalue locators are queried against the original
                // inventory, never projected as optimizer occurrence evidence.
                rvalue_results: _,
                slot_relocation: _,
                source_slots: _,
                insertions: _,
                terminal_failures: _,
                declarations: _,
                private_payload: _,
                requires_context_issue: _,
                inherited_emission_storage: _,
                inherited_assembly_storage: _,
            } = root;
            budget.charge_work(1)?;
            let mut walk = TileAttachmentWalkV29 {
                pending,
                root_ordinal,
                root,
                graph,
                index,
                budget,
                visit,
            };
            walk.walk_core_v29()?;
            walk.walk_sidecars_v29()?;
        }
        Ok(())
    }

    impl TileAttachmentWalkV29<'_, '_, '_, '_> {
        fn raw_call_v29(
            &mut self,
            instance: ProductionCallInstanceIdV1,
            row_ordinal: usize,
            sidecar: &PendingInstanceSidecarsV29,
            source: SemanticKirCallReturnV1,
        ) -> TileAttachmentResultV29<()> {
            use TileAttachmentFamilyV29 as Family;
            use TileAttachmentFieldV29 as Field;
            let SemanticKirCallReturnV1 {
                correspondence_owner: _,
                semantic_function: _,
                semantic_block,
                kind,
            } = source;
            let block = self.raw_block_v29(sidecar, semantic_block)?;
            let component_span = match kind {
                SemanticKirCallReturnKindV1::Call { transport, .. } => transport,
                SemanticKirCallReturnKindV1::Return { components } => components,
                SemanticKirCallReturnKindV1::NoNormalReturnCall { .. } => {
                    CallComponentSpanV1::EMPTY
                }
            };
            let components = sidecar
                .call_returns
                .components
                .rows
                .get(component_span.range()?)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            self.budget.charge_work(components.len())?;
            match kind {
                SemanticKirCallReturnKindV1::NoNormalReturnCall {
                    arguments_first,
                    call_operation,
                    destination,
                } => {
                    self.budget
                        .charge_work(sidecar.terminator_operation_spans.len())?;
                    let mut spans = sidecar
                        .terminator_operation_spans
                        .iter()
                        .filter(|span| span.semantic_block == semantic_block);
                    let span = spans
                        .next()
                        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                    if spans.next().is_some() || span.kernel_ir_block != block {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    }
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallArguments,
                    );
                    self.emitted_range(key, instance, block, arguments_first, call_operation)?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallOperation,
                    );
                    self.emitted_operation(key, instance, block, call_operation)?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallDestination,
                    );
                    self.emitted_range(
                        key,
                        instance,
                        block,
                        span.first_operation_ordinal,
                        arguments_first,
                    )?;
                    self.emit(
                        tile_attachment_component_v29(key, 1),
                        TileAttachmentLocationV29::NoOutput,
                    )?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallDestinationPointer,
                    );
                    match destination {
                        None | Some(SemanticKirCallDestinationV1::Local) => {
                            self.emit(key, TileAttachmentLocationV29::NoOutput)?
                        }
                        Some(
                            SemanticKirCallDestinationV1::Retained { pointer, .. }
                            | SemanticKirCallDestinationV1::Projected { pointer, .. },
                        ) => self.definition(key, pointer)?,
                    }
                    for field in [Field::RawTransportArgument, Field::RawTransportConversion] {
                        let key =
                            self.key(Family::RawSidecar, instance.index(), row_ordinal, field);
                        self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                    }
                    let control = tile_attachment_control_v29(
                        &self.root.coordinates,
                        instance,
                        block,
                        semantic_block,
                        self.budget,
                    )?;
                    let (_, physical) = tile_attachment_block_v29(
                        self.graph,
                        self.index,
                        self.root.function_ordinal,
                        control.physical_block,
                        self.budget,
                    )?;
                    if !matches!(physical.terminator, Some(Terminator::Unreachable)) {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    }
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawNoNormalReturnTerminator,
                    );
                    self.terminator(key, control.physical_block)?;
                }
                SemanticKirCallReturnKindV1::Call {
                    arguments_first,
                    call_operation,
                    destination_end,
                    destination,
                    transport: _,
                } => {
                    self.budget
                        .charge_work(sidecar.terminator_operation_spans.len())?;
                    let mut spans = sidecar
                        .terminator_operation_spans
                        .iter()
                        .filter(|span| span.semantic_block == semantic_block);
                    let span = spans
                        .next()
                        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                    if spans.next().is_some() || span.kernel_ir_block != block {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    }
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallArguments,
                    );
                    self.emitted_range(key, instance, block, arguments_first, call_operation)?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallOperation,
                    );
                    self.emitted_operation(key, instance, block, call_operation)?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallDestination,
                    );
                    self.emitted_range(
                        key,
                        instance,
                        block,
                        span.first_operation_ordinal,
                        arguments_first,
                    )?;
                    let first = call_operation
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    self.emitted_range(
                        tile_attachment_component_v29(key, 1),
                        instance,
                        block,
                        first,
                        destination_end,
                    )?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawCallDestinationPointer,
                    );
                    match destination {
                        SemanticKirCallDestinationV1::Local => {
                            self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                        }
                        SemanticKirCallDestinationV1::Retained { pointer, access: _ }
                        | SemanticKirCallDestinationV1::Projected { pointer, access: _ } => {
                            self.definition(key, pointer)?;
                        }
                    }
                    let argument_key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawTransportArgument,
                    );
                    let conversion_key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawTransportConversion,
                    );
                    if components.is_empty() {
                        self.emit(argument_key, TileAttachmentLocationV29::NoOutput)?;
                        self.emit(conversion_key, TileAttachmentLocationV29::NoOutput)?;
                        return Ok(());
                    }
                    let control = tile_attachment_control_v29(
                        &self.root.coordinates,
                        instance,
                        block,
                        semantic_block,
                        self.budget,
                    )?;
                    let (coordinate, physical) = tile_attachment_block_v29(
                        self.graph,
                        self.index,
                        self.root.function_ordinal,
                        control.physical_block,
                        self.budget,
                    )?;
                    let Some(Terminator::Branch { arguments, .. }) = &physical.terminator else {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    };
                    for (component, item) in components.iter().enumerate() {
                        let CallResultComponentV1::Transport {
                            slot,
                            conversion: _,
                        } = *item
                        else {
                            return Err(ScopedTileFailureKindV29::ReplayMismatch);
                        };
                        if arguments.get(slot as usize).is_none() {
                            return Err(ScopedTileFailureKindV29::ReplayMismatch);
                        }
                        self.canonical_edge_argument(
                            tile_attachment_component_v29(argument_key, component),
                            AttachmentEdgeArgumentV29 {
                                edge: AttachmentEdgeV29 {
                                    source: coordinate,
                                    successor: 0,
                                },
                                argument: slot,
                            },
                        )?;
                    }
                    for (component, item) in components.iter().enumerate() {
                        let CallResultComponentV1::Transport {
                            slot: _,
                            conversion,
                        } = *item
                        else {
                            return Err(ScopedTileFailureKindV29::ReplayMismatch);
                        };
                        let key = tile_attachment_component_v29(conversion_key, component);
                        match conversion {
                            Some(p) => self.emitted_operation(key, instance, block, p)?,
                            None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                        }
                    }
                }
                SemanticKirCallReturnKindV1::Return { components: _ } => {
                    let control = tile_attachment_control_v29(
                        &self.root.coordinates,
                        instance,
                        block,
                        semantic_block,
                        self.budget,
                    )?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawReturnTerminator,
                    );
                    self.terminator(key, control.physical_block)?;
                    let input_key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawReturnInput,
                    );
                    let conversion_key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row_ordinal,
                        Field::RawReturnConversion,
                    );
                    if components.is_empty() {
                        self.emit(input_key, TileAttachmentLocationV29::NoOutput)?;
                        self.emit(conversion_key, TileAttachmentLocationV29::NoOutput)?;
                    }
                    for (component, item) in components.iter().enumerate() {
                        let CallResultComponentV1::Return {
                            input,
                            conversion: _,
                        } = *item
                        else {
                            return Err(ScopedTileFailureKindV29::ReplayMismatch);
                        };
                        self.definition(
                            tile_attachment_component_v29(input_key, component),
                            input,
                        )?;
                    }
                    for (component, item) in components.iter().enumerate() {
                        let CallResultComponentV1::Return {
                            input: _,
                            conversion,
                        } = *item
                        else {
                            return Err(ScopedTileFailureKindV29::ReplayMismatch);
                        };
                        let key = tile_attachment_component_v29(conversion_key, component);
                        match conversion {
                            Some(p) => self.emitted_operation(key, instance, block, p)?,
                            None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                        }
                    }
                }
            }
            Ok(())
        }
    }

    include!("production_scoped_tile_projection_sources_v29.rs");
}
