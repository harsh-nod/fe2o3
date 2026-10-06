impl TileAttachmentWalkV29<'_, '_, '_, '_> {
    fn walk_terminal_failure_attachments_v18(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        let Some(relation) = &root.terminal_failures else {
            return Ok(());
        };
        self.budget.charge_work(1)?;
        if relation.origins.rows.len() != relation.closures.len() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        for (row, (origin, closure)) in relation
            .origins
            .rows
            .iter()
            .zip(&relation.closures)
            .enumerate()
        {
            self.budget.charge_work(3)?;
            if closure.origin != row {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let instance = origin.instance.index();
            if root
                .active_instances
                .sidecar_ordinal(
                    instance,
                    root.coordinates.sources.rows.len(),
                    &root.sidecars.rows,
                    self.budget,
                )?
                .is_none()
            {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let (source_block, source_edge, original_target, original_diagnostic) =
                match origin.site {
                    TerminalFailureSiteV18::Edge {
                        block,
                        successor,
                        target,
                    } => (block, Some(successor), Some(target), 0),
                    TerminalFailureSiteV18::Operation { block, operation } => {
                        (block, None, None, operation)
                    }
                };
            let (source, _) = tile_attachment_block_v29(
                self.graph,
                self.index,
                root.function_ordinal,
                source_block,
                self.budget,
            )?;
            let (terminal, body) = tile_attachment_block_v29(
                self.graph,
                self.index,
                root.function_ordinal,
                closure.block,
                self.budget,
            )?;
            let original = if let Some(target) = original_target {
                tile_attachment_block_v29(
                    self.graph,
                    self.index,
                    root.function_ordinal,
                    target,
                    self.budget,
                )?
                .0
            } else {
                terminal
            };
            let root_ordinal = self.root_ordinal;
            let key = |field| TileAttachmentKeyV29 {
                root: root_ordinal,
                family: Family::TerminalFailure,
                instance,
                row,
                field,
                component: 0,
                part: 0,
            };
            let block = |coordinate: AttachmentBlockV29| {
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Block {
                    function: root.function_ordinal,
                    block: coordinate.block as usize,
                })
            };
            self.emit(key(Field::FailureSourceBlock), block(source))?;
            self.emit(
                key(Field::FailureSourceEdge),
                match source_edge {
                    Some(edge) => TileAttachmentLocationV29::Origin(TileScalarSourceV29::Edge {
                        function: root.function_ordinal,
                        block: source.block as usize,
                        edge: edge as usize,
                    }),
                    None => TileAttachmentLocationV29::NoOutput,
                },
            )?;
            self.emit(
                key(Field::FailureOriginalTarget),
                if original_target.is_some() {
                    block(original)
                } else {
                    TileAttachmentLocationV29::NoOutput
                },
            )?;
            let diagnostic = if closure.generated {
                original_diagnostic
            } else {
                closure.diagnostic
            };
            self.emit(
                key(Field::FailureOriginalDiagnostic),
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(
                    TileScalarPointV29 {
                        function: root.function_ordinal,
                        block: original.block as usize,
                        operation: diagnostic as usize,
                    },
                )),
            )?;
            self.emit(key(Field::FailureBlock), block(terminal))?;
            self.emit(
                key(Field::FailureGap),
                TileAttachmentLocationV29::Gap(TileScalarPointV29 {
                    function: root.function_ordinal,
                    block: terminal.block as usize,
                    operation: closure.first as usize,
                }),
            )?;
            let range = closure.first as usize..closure.diagnostic as usize;
            let cleanup = body
                .operations
                .get(range.clone())
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            self.budget.charge_work(cleanup.len())?;
            if cleanup.is_empty() {
                self.emit(
                    key(Field::FailureCleanup),
                    TileAttachmentLocationV29::NoOutput,
                )?;
                self.emit(
                    key(Field::FailureOperand),
                    TileAttachmentLocationV29::NoOutput,
                )?;
            }
            for (component, operation) in cleanup.iter().enumerate() {
                let mut operation_key = key(Field::FailureCleanup);
                operation_key.component = component;
                self.emit(
                    operation_key,
                    TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(
                        TileScalarPointV29 {
                            function: root.function_ordinal,
                            block: terminal.block as usize,
                            operation: range.start + component,
                        },
                    )),
                )?;
                let OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd {
                    workgroup,
                    discarded,
                }) = &operation.kind
                else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                self.budget
                    .charge_work(argument_sum_v1(&[discarded.len(), 1])?)?;
                for (part, value) in std::iter::once(workgroup)
                    .chain(discarded.iter())
                    .enumerate()
                {
                    let mut operand_key = key(Field::FailureOperand);
                    operand_key.component = component;
                    operand_key.part = part;
                    self.definition(operand_key, *value)?;
                }
            }
            self.emit(
                key(Field::FailureDiagnostic),
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(
                    TileScalarPointV29 {
                        function: root.function_ordinal,
                        block: terminal.block as usize,
                        operation: closure.diagnostic as usize,
                    },
                )),
            )?;
            self.emit(
                key(Field::FailureTerminator),
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Terminator {
                    function: root.function_ordinal,
                    block: terminal.block as usize,
                }),
            )?;
        }
        Ok(())
    }
}
