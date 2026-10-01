// Private scalar candidate custody. Source, launch, collective and memory
// obligations stay retained; this module grants no executable authority.
use scoped_tile_materialization_v29::*;

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile materialization remains gated")
)]
mod scoped_tile_materialization_v29 {
    use super::*;

    include!("production_scoped_tile_scalar_emitter_v29.rs");
    include!("production_scoped_tile_projection_v29.rs");
    include!("production_scoped_tile_projection_sources_v29.rs");

    #[cfg(test)]
    pub(super) mod tile_materialization_faults_v29 {
        use super::*;
        use std::{
            any::Any,
            cell::{Cell, RefCell},
        };

        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub(in super::super) enum Point {
            CopyTransfer,
            CopyAdopted,
            Emitted,
            V18Transfer,
            V18Adopted,
            Projected,
        }
        pub(in super::super) type History = (usize, usize, usize, Option<usize>);
        type Payload = Box<dyn Any + Send>;
        thread_local! {
            pub(in super::super) static ARMED: RefCell<Option<(Point, Payload)>> = const { RefCell::new(None) };
            pub(in super::super) static SEEN: Cell<Option<History>> = const { Cell::new(None) };
        }
        fn take(point: Point, budget: &ArgumentBudgetV1<'_>) -> Option<Payload> {
            let selected = ARMED.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot
                    .as_ref()
                    .is_some_and(|(expected, _)| *expected == point)
                {
                    slot.take().map(|(_, payload)| payload)
                } else {
                    None
                }
            });
            if selected.is_some() {
                SEEN.set(Some((
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )));
            }
            selected
        }
        pub(in super::super) fn transfer(
            point: Point,
            bytes: usize,
            budget: &ArgumentBudgetV1<'_>,
        ) -> usize {
            if take(point, budget).is_some() {
                assert!(matches!(point, Point::CopyTransfer | Point::V18Transfer));
                budget
                    .storage_limit()
                    .checked_add(1)
                    .unwrap()
                    .checked_sub(budget.storage())
                    .unwrap()
            } else {
                bytes
            }
        }
        pub(in super::super) fn panic(point: Point, budget: &ArgumentBudgetV1<'_>) {
            if let Some(payload) = take(point, budget) {
                std::panic::resume_unwind(payload);
            }
        }
    }

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
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum TileScalarPieceV29 {
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
        Anchor(TileScalarPointV29),
        ErasedValue(ValueId),
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum TileScalarStageV29 {
        Preserved,
        Erased,
        Prelude,
        Predicate,
        Read,
        ReadBlock,
        Join,
        Conditional,
        ReadBranch,
        Parts,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct TileScalarTaggedPieceV29 {
        pub(super) piece: TileScalarPieceV29,
        pub(super) component: Option<u32>,
        pub(super) stage: TileScalarStageV29,
    }
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct TileScalarOriginV29 {
        pub(super) source: TileScalarSourceV29,
        pub(super) first: usize,
        pub(super) count: usize,
    }
    pub(super) struct TileScalarRelationsV29 {
        pub(super) origins: Vec<TileScalarOriginV29>,
        pub(super) pieces: Vec<TileScalarTaggedPieceV29>,
    }
    impl TileScalarRelationsV29 {
        pub(super) fn storage(&self) -> Result<usize, ArgumentResourceV1> {
            argument_sum_v1(&[
                argument_product_v1(self.origins.capacity(), size_of::<TileScalarOriginV29>())?,
                argument_product_v1(
                    self.pieces.capacity(),
                    size_of::<TileScalarTaggedPieceV29>(),
                )?,
            ])
        }
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
        index: &AssertGraphIndexV1<'_>,
        function: usize,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> TileAttachmentCoreResultV29<(AttachmentBlockV29, &'a BasicBlock)> {
        let coordinate = index
            .block(
                AttachmentFunctionV29(tile_attachment_u32_v29(function)?),
                block,
                budget,
            )
            .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?;
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
            let SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                destination,
                transport: _,
            } = kind
            else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
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
            self.emitted_range(
                destination_range,
                *instance,
                physical.block,
                after_call,
                destination_end,
            )?;
            match destination {
                SemanticKirCallDestinationV1::Local => {
                    self.emit(destination_pointer, TileAttachmentLocationV29::NoOutput)?
                }
                SemanticKirCallDestinationV1::Retained { pointer, access: _ }
                | SemanticKirCallDestinationV1::Projected { pointer, access: _ } => {
                    self.definition(destination_pointer, pointer)?
                }
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
            let SemanticKirCallReturnKindV1::Call { transport, .. } = row.source.kind else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
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

    pub(super) struct ScopedTileScalarCandidateV29 {
        pub(super) input: PreparedScopedTileSourceV29,
        pub(super) output: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        pub(super) relations: TileScalarRelationsV29,
        pub(super) projections: TileScalarProjectionsV29,
        retained_storage: usize,
        pub(super) output_storage: usize,
    }
    pub(super) struct ScopedTileMaterializationFailureV29 {
        input: PreparedScopedTileSourceV29,
        pub(super) summary: ScopedTileFailureSummaryV29,
    }
    impl ScopedTileMaterializationFailureV29 {
        pub(super) fn into_input(self) -> PreparedScopedTileSourceV29 {
            self.input
        }
    }

    fn tile_scalar_v18_error_v29(
        error: fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18,
    ) -> ScopedTileFailureKindV29 {
        ScopedModuleErrorV29::Canonical(error).into()
    }

    // Both transfer APIs restore their incoming floor. Adopt each returned
    // receipt immediately, while all coexisting owners remain paid.
    fn tile_scalar_graph_v29(
        input: &PreparedScopedTileSourceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        (
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            TileScalarRelationsV29,
            usize,
        ),
        ScopedTileFailureKindV29,
    > {
        let floor = budget.storage();
        let (mut module, receipt) = input
            .pending
            .inner
            .pending
            .graph
            .copy_module_for_transformation_v18(budget)
            .map_err(tile_scalar_v18_error_v29)?;
        let copy_storage = receipt.retained_storage();
        #[cfg(test)]
        let copy_storage = tile_materialization_faults_v29::transfer(
            tile_materialization_faults_v29::Point::CopyTransfer,
            copy_storage,
            budget,
        );
        if let Err(error) = budget.reserve_storage(copy_storage) {
            drop(module);
            return Err(error.into());
        }
        #[cfg(test)]
        tile_materialization_faults_v29::panic(
            tile_materialization_faults_v29::Point::CopyAdopted,
            budget,
        );
        let relations = tile_scalar_build_v29(input, &mut module, budget)?;
        #[cfg(test)]
        tile_materialization_faults_v29::panic(
            tile_materialization_faults_v29::Point::Emitted,
            budget,
        );
        let relation_storage = relations.storage()?;
        let mutable_storage = budget
            .storage()
            .checked_sub(floor)
            .and_then(|bytes| bytes.checked_sub(relation_storage))
            .ok_or(ArgumentResourceV1::Accounting)?;
        let (output, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18
            ::from_module_ref_with_verification_budget_v18(
                &module, input.pending.inner.limits.storage_layout_limits(), budget,
            )
            .map_err(tile_scalar_v18_error_v29)?;
        let output_storage = receipt.retained_storage();
        #[cfg(test)]
        let output_charge = tile_materialization_faults_v29::transfer(
            tile_materialization_faults_v29::Point::V18Transfer,
            output_storage,
            budget,
        );
        #[cfg(not(test))]
        let output_charge = output_storage;
        if let Err(error) = budget.reserve_storage(output_charge) {
            drop(output);
            return Err(error.into());
        }
        #[cfg(test)]
        tile_materialization_faults_v29::panic(
            tile_materialization_faults_v29::Point::V18Adopted,
            budget,
        );
        drop(module);
        budget.release_storage(mutable_storage)?;
        Ok((output, relations, output_storage))
    }

    pub(super) fn materialize_scoped_tile_source_v29(
        input: PreparedScopedTileSourceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ScopedTileScalarCandidateV29, ScopedTileMaterializationFailureV29> {
        let mut phase = ScopedTileFailurePhaseV29::Materialization;
        let failure = |input, phase, kind| ScopedTileMaterializationFailureV29 {
            input,
            summary: ScopedTileFailureSummaryV29 { phase, kind },
        };
        if let Err(kind) = scoped_tile_floor_v29(&input.pending, input.adopted_storage(), budget) {
            return Err(failure(input, phase, kind));
        }
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_tile_attempt_v29(budget, |budget| {
                input.replay_inner_v29(budget)?;
                let (output, relations, output_storage) = tile_scalar_graph_v29(&input, budget)?;
                phase = ScopedTileFailurePhaseV29::Correspondence;
                replay_tile_scalar_graph_v29(&input, &output, &relations, budget)?;
                let projections =
                    project_scoped_tile_attachments_v29(&input, &output, &relations, budget)?;
                #[cfg(test)]
                tile_materialization_faults_v29::panic(
                    tile_materialization_faults_v29::Point::Projected,
                    budget,
                );
                replay_scoped_tile_attachments_v29(
                    &input,
                    &output,
                    &relations,
                    &projections,
                    budget,
                )?;
                let retained_storage = argument_sum_v1(&[
                    input.adopted_storage(),
                    output_storage,
                    relations.storage()?,
                    projections.storage()?,
                ])?;
                let extra = retained_storage
                    .checked_sub(input.adopted_storage())
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if budget.storage() != argument_sum_v1(&[floor, extra])? {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((
                    output,
                    relations,
                    projections,
                    retained_storage,
                    output_storage,
                ))
            })
        }));
        match result {
            Ok(Ok((output, relations, projections, retained_storage, output_storage))) => {
                Ok(ScopedTileScalarCandidateV29 {
                    input,
                    output,
                    relations,
                    projections,
                    retained_storage,
                    output_storage,
                })
            }
            Ok(Err(kind)) => Err(failure(input, phase, kind)),
            Err(payload) => {
                let adopted = input.adopted_storage();
                drop(input);
                // The attempt already dropped generated owners and restored F0.
                // A consumed input cannot be returned across unwinding.
                if budget.work_ledger_identity_v1() == ledger {
                    if let Some(extra) = floor
                        .checked_sub(adopted)
                        .and_then(|target| budget.storage().checked_sub(target))
                    {
                        let _ = budget.release_storage(extra);
                    }
                }
                std::panic::resume_unwind(payload)
            }
        }
    }

    impl ScopedTileScalarCandidateV29 {
        pub(super) fn adopted_storage(&self) -> usize {
            self.retained_storage
        }

        pub(super) fn replay_with_budget(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ScopedTileFailureKindV29> {
            let expected = argument_sum_v1(&[
                self.input.adopted_storage(),
                self.output_storage,
                self.relations.storage()?,
                self.projections.storage()?,
            ])?;
            if expected != self.retained_storage {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            scoped_tile_floor_v29(&self.input.pending, expected, budget)?;
            let floor = budget.storage();
            scoped_tile_attempt_v29(budget, |budget| {
                replay_tile_scalar_graph_v29(&self.input, &self.output, &self.relations, budget)?;
                replay_scoped_tile_attachments_v29(
                    &self.input,
                    &self.output,
                    &self.relations,
                    &self.projections,
                    budget,
                )?;
                if budget.storage() != floor {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok(())
            })
        }
    }
}
