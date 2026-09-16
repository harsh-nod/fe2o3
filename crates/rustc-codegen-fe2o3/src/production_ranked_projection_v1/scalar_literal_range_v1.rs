impl SemanticAssertProofsV1<'_> {
    /// Walk every reaching path backwards, stopping only at its last exact
    /// literal assignment. This is a finite join, not loop invariant inference.
    fn reaching_scalar_literal_range_v1(
        &mut self,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        if self.address_escaped.get(local).copied() != Some(false) {
            return Ok(None);
        }
        let Some(declaration) = self.function.locals().get(local) else {
            return Ok(None);
        };
        let ty = declaration.ty();
        let Some(bits) = self.unsigned_integer_bits(ty) else {
            return Ok(None);
        };
        let Some(maximum) = self.scalar_unsigned_maximum(ty) else {
            return Ok(None);
        };
        let size = u64::from(bits.div_ceil(8));
        if self.types[ty.index() as usize].layout().size_bytes() != Some(size) {
            return Ok(None);
        }
        let block_count = self.function.blocks().len();
        if use_site.block >= block_count
            || use_site.statement > self.function.blocks()[use_site.block].statements().len()
        {
            return Ok(None);
        }
        self.charge(block_count)?;
        let mut colors = Vec::new();
        let mut pending = Vec::new();
        colors.try_reserve_exact(block_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "literal range traversal color storage cannot be reserved",
            )
        })?;
        pending.try_reserve_exact(block_count).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "literal range traversal stack storage cannot be reserved",
            )
        })?;
        colors.resize(block_count, 0_u8);
        pending.push((use_site.block, use_site.statement, None::<usize>));
        let mut result: Option<UnsignedRangeProofV1> = None;
        while let Some((block, end, next_predecessor)) = pending.last().copied() {
            self.charge(1)?;
            if let Some(next) = next_predecessor {
                if next == self.graph.predecessors[block].len() {
                    colors[block] = 2;
                    pending.pop();
                    continue;
                }
                pending.last_mut().expect("nonempty literal traversal").2 = Some(next + 1);
                let predecessor = self.graph.predecessors[block][next];
                self.charge(1)?;
                // Calls can replace the local after the block's last statement.
                if let SemanticTerminatorKindV1::Call(call) =
                    self.function.blocks()[predecessor].terminator().kind()
                    && call.destination().is_some_and(|destination| {
                        local_definition_index(destination.place()) == Some(local)
                    })
                {
                    return Ok(None);
                }
                match colors[predecessor] {
                    1 => return Ok(None),
                    2 => continue,
                    _ => pending.push((
                        predecessor,
                        self.function.blocks()[predecessor].statements().len(),
                        None,
                    )),
                }
                continue;
            }
            colors[block] = 1;
            let mut literal = None;
            for index in (0..end).rev() {
                self.charge(1)?;
                let kind = self.function.blocks()[block].statements()[index].kind();
                if matches!(kind,
                    SemanticStatementKindV1::StorageLive(value)
                    | SemanticStatementKindV1::StorageDead(value)
                    if value.index() as usize == local)
                {
                    return Ok(None);
                }
                let mut defines_local = false;
                visit_statement_definition_places(kind, &mut |place| {
                    defines_local |= local_definition_index(place) == Some(local);
                });
                if !defines_local {
                    continue;
                }
                let SemanticStatementKindV1::Assign(assignment) = kind else {
                    return Ok(None);
                };
                if !assignment.destination().projections().is_empty()
                    || assignment.destination().ty() != ty
                    || assignment.value().result_type() != ty
                {
                    return Ok(None);
                }
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) =
                    assignment.value().kind()
                else {
                    return Ok(None);
                };
                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                    return Ok(None);
                };
                if constant.ty() != ty
                    || u64::from(value.size_bytes()) != size
                    || value.bits() > maximum
                {
                    return Ok(None);
                }
                literal = Some(value.bits());
                break;
            }
            if let Some(value) = literal {
                result = Some(match result {
                    Some(range) => UnsignedRangeProofV1 {
                        minimum: range.minimum.min(value),
                        maximum: range.maximum.max(value),
                    },
                    None => UnsignedRangeProofV1::exact(value),
                });
                colors[block] = 2;
                pending.pop();
            } else if block == self.graph.entry || self.graph.predecessors[block].is_empty() {
                return Ok(None);
            } else {
                pending.last_mut().expect("nonempty literal traversal").2 = Some(0);
            }
        }
        Ok(result)
    }
}
