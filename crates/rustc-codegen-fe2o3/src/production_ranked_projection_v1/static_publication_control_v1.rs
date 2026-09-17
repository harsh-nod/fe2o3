// Exact source control is separate from deterministic dependency summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticPublicationComparisonKindV1 {
    Equal,
    LessThan,
}

fn materialize_static_publication_comparison_v1(
    kind: StaticPublicationComparisonKindV1,
    lhs: ProductionRankedValueV1,
    rhs: ProductionRankedValueV1,
    true_block: u32,
    false_block: u32,
    true_arguments: Vec<ProductionRankedValueV1>,
    false_arguments: Vec<ProductionRankedValueV1>,
) -> ProductionRankedTerminatorV1 {
    match kind {
        StaticPublicationComparisonKindV1::Equal => ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs,
            rhs,
            true_block,
            false_block,
            true_arguments,
            false_arguments,
        },
        StaticPublicationComparisonKindV1::LessThan => {
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs,
                rhs,
                true_block,
                false_block,
                true_arguments,
                false_arguments,
            }
        }
    }
}

struct StaticPublicationControlProjectorV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    source: &'a StaticPublicationSourceV1,
    index_values: &'a [Option<ProjectedDisjointIndexV1>],
    extents: [ProductionRankedValueV1; 2],
    operations: &'a mut Vec<ProductionRankedOperationV1>,
    next_value: &'a mut u32,
    proofs: SemanticAssertProofsV1<'a>,
    calls: Vec<Option<usize>>,
    values: Vec<Option<ProductionRankedValueV1>>,
}

impl<'a> StaticPublicationControlProjectorV1<'a> {
    fn definition(
        &mut self,
        local: usize,
        use_site: ScalarAssignmentSiteV1,
    ) -> Result<Option<ScalarAssignmentSiteV1>, ProductionRankedProjectionErrorV1> {
        self.proofs.charge(1)?;
        if self.proofs.definition_counts.get(local).copied() != Some(1)
            || self.proofs.address_escaped.get(local).copied() != Some(false)
        {
            return Ok(None);
        }
        let site = self
            .proofs
            .assignments
            .get(local)
            .copied()
            .flatten()
            .or_else(|| {
                self.calls
                    .get(local)
                    .copied()
                    .flatten()
                    .map(|block| ScalarAssignmentSiteV1 {
                        block,
                        statement: self.function.blocks()[block].statements().len(),
                    })
            });
        let Some(site) = site else { return Ok(None) };
        if !indexed_atomic_block_acyclic_v1(&mut self.proofs, site.block)?
            || !self
                .proofs
                .assignment_dominates_use(site, use_site.block, use_site.statement)?
        {
            return Ok(None);
        }
        if let Some(block) = self.calls[local] {
            let SemanticTerminatorKindV1::Call(call) =
                self.function.blocks()[block].terminator().kind()
            else {
                return Err(static_publication_reject_v1());
            };
            let Some(destination) = call.destination() else {
                return Ok(None);
            };
            if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                || !self
                    .proofs
                    .block_dominates(destination.edge().target().index() as usize, use_site.block)?
            {
                return Ok(None);
            }
        }
        Ok(Some(site))
    }

    fn constant(
        &mut self,
        value: u64,
    ) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
        self.proofs.charge(1)?;
        reserve_operation(self.operations)?;
        let result = next_value_id(self.next_value)?;
        self.operations
            .push(ProductionRankedOperationV1::IndexConstant { result, value });
        Ok(ProductionRankedValueV1::Local(result))
    }

    fn metadata_root(
        &mut self,
        place: &SemanticPlaceV1,
        use_site: ScalarAssignmentSiteV1,
        depth: usize,
    ) -> Result<Option<usize>, ProductionRankedProjectionErrorV1> {
        self.proofs.charge(1)?;
        if depth >= 64 || !place.projections().is_empty() {
            return Ok(None);
        }
        for (root, source) in [self.source.payload, self.source.flags].iter().enumerate() {
            if place.local() == source.local && place.ty() == source.ty {
                return Ok(Some(root));
            }
        }
        let local = place.local().index() as usize;
        let Some(site) = self.definition(local, use_site)? else {
            return Ok(None);
        };
        if self
            .source
            .metadata_snapshot
            .is_some_and(|snapshot| snapshot.local == place.local())
            && place.ty() == self.source.payload.ty
        {
            return Ok(Some(0));
        }
        let Some(statement) = self.function.blocks()[site.block]
            .statements()
            .get(site.statement)
        else {
            return Ok(None);
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        let origin = match assignment.value().kind() {
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } => place,
            SemanticRvalueKindV1::Use(operand) => {
                let Some(place) = raw_operand_place(operand) else {
                    return Ok(None);
                };
                place
            }
            _ => return Ok(None),
        };
        let origin = origin.clone();
        let Some(root) = self.metadata_root(&origin, site, depth + 1)? else {
            return Ok(None);
        };
        if !consumed_read_only_shared_reference_v1(
            self.types,
            place.ty(),
            [self.source.payload, self.source.flags][root].ty,
        ) {
            return Ok(None);
        }
        Ok(Some(root))
    }

    fn operand(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
        depth: usize,
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        self.proofs.charge(1)?;
        if depth >= 64 || unsigned_index_bits_v1(self.types, operand.ty()).is_none() {
            return Ok(None);
        }
        if let SemanticOperandV1::Constant(constant) = operand {
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Ok(None);
            };
            let Ok(value) = u64::try_from(value.bits()) else {
                return Ok(None);
            };
            return self.constant(value).map(Some);
        }
        let Some(place) = raw_operand_place(operand) else {
            return Ok(None);
        };
        if !place.projections().is_empty() {
            return Ok(None);
        }
        let local = place.local().index() as usize;
        let Some(site) = self.definition(local, use_site)? else {
            return Ok(None);
        };
        // Every cache hit authenticates the exact source definition at this use.
        if let Some(value) = self.values[local] {
            return Ok(Some(value));
        }
        if let Some(index) = self.index_values.get(local).copied().flatten() {
            self.values[local] = Some(index.value);
            return Ok(Some(index.value));
        }
        let value = if let Some(block) = self.calls[local] {
            let SemanticTerminatorKindV1::Call(call) =
                self.function.blocks()[block].terminator().kind()
            else {
                return Err(static_publication_reject_v1());
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                        disjoint_slice,
                        raw_index,
                        ..
                    },
                ..
            }) = self.callables.get(call.callee().index() as usize)
            else {
                return Ok(None);
            };
            if *disjoint_slice != self.source.payload.ty
                || *raw_index != operand.ty()
                || call.arguments().len() != 1
                || unsigned_index_bits_v1(self.types, operand.ty()) != Some(64)
            {
                return Ok(None);
            }
            let Some(receiver) = raw_operand_place(&call.arguments()[0]).cloned() else {
                return Ok(None);
            };
            (self.metadata_root(&receiver, site, depth + 1)? == Some(0)).then_some(self.extents[0])
        } else {
            let SemanticStatementKindV1::Assign(assignment) =
                self.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                return Err(static_publication_reject_v1());
            };
            let value = assignment.value().clone();
            match value.kind() {
                SemanticRvalueKindV1::Use(operand) => self.operand(operand, site, depth + 1)?,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand,
                    ..
                } if value_preserving_unsigned_index_cast_v1(
                    self.types,
                    operand.ty(),
                    value.result_type(),
                ) =>
                {
                    self.operand(operand, site, depth + 1)?
                }
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: SemanticOperandV1::Copy(place),
                } if place.local() == self.source.flags.local
                    && place.ty() == self.source.flags.ty
                    && place.projections().is_empty()
                    && unsigned_index_bits_v1(self.types, value.result_type()) == Some(64) =>
                {
                    Some(self.extents[1])
                }
                SemanticRvalueKindV1::Binary {
                    operation,
                    left,
                    right,
                } => {
                    let Some(kind) = deterministic_index_binary_kind_v1(*operation) else {
                        return Ok(None);
                    };
                    let left_range =
                        self.proofs
                            .range_at_operand(left, site.block, site.statement)?;
                    let right_range =
                        self.proofs
                            .range_at_operand(right, site.block, site.statement)?;
                    let maximum = self.proofs.scalar_unsigned_maximum(value.result_type());
                    if SemanticAssertProofsV1::range_of_binary(
                        *operation,
                        left_range,
                        right_range,
                        maximum,
                    )?
                    .is_none()
                    {
                        return Ok(None);
                    }
                    let Some(lhs) = self.operand(left, site, depth + 1)? else {
                        return Ok(None);
                    };
                    let Some(rhs) = self.operand(right, site, depth + 1)? else {
                        return Ok(None);
                    };
                    reserve_operation(self.operations)?;
                    let result = next_value_id(self.next_value)?;
                    self.operations
                        .push(ProductionRankedOperationV1::IndexBinary {
                            result,
                            kind,
                            lhs,
                            rhs,
                        });
                    Some(ProductionRankedValueV1::Local(result))
                }
                _ => None,
            }
        };
        self.values[local] = value;
        Ok(value)
    }

    fn comparison(
        &mut self,
        operand: &SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
        depth: usize,
    ) -> Result<
        Option<(
            StaticPublicationComparisonKindV1,
            ProductionRankedValueV1,
            ProductionRankedValueV1,
            bool,
        )>,
        ProductionRankedProjectionErrorV1,
    > {
        self.proofs.charge(1)?;
        if depth >= 64
            || !matches!(
                self.types
                    .get(operand.ty().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            )
        {
            return Ok(None);
        }
        let Some(local) = simple_operand_local(operand) else {
            return Ok(None);
        };
        let Some(site) = self.definition(local.index() as usize, use_site)? else {
            return Ok(None);
        };
        let Some(statement) = self.function.blocks()[site.block]
            .statements()
            .get(site.statement)
        else {
            return Ok(None);
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        let value = assignment.value().clone();
        match value.kind() {
            SemanticRvalueKindV1::Use(next) => self.comparison(next, site, depth + 1),
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand,
            } => Ok(self
                .comparison(operand, site, depth + 1)?
                .map(|(kind, lhs, rhs, invert)| (kind, lhs, rhs, !invert))),
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } if left.ty() == right.ty() => {
                let (kind, swap, invert) = match operation {
                    SemanticBinaryOpV1::Equal => {
                        (StaticPublicationComparisonKindV1::Equal, false, false)
                    }
                    SemanticBinaryOpV1::NotEqual => {
                        (StaticPublicationComparisonKindV1::Equal, false, true)
                    }
                    SemanticBinaryOpV1::LessThan => {
                        (StaticPublicationComparisonKindV1::LessThan, false, false)
                    }
                    SemanticBinaryOpV1::GreaterOrEqual => {
                        (StaticPublicationComparisonKindV1::LessThan, false, true)
                    }
                    SemanticBinaryOpV1::GreaterThan => {
                        (StaticPublicationComparisonKindV1::LessThan, true, false)
                    }
                    SemanticBinaryOpV1::LessOrEqual => {
                        (StaticPublicationComparisonKindV1::LessThan, true, true)
                    }
                    _ => return Ok(None),
                };
                let Some(lhs) = self.operand(left, site, depth + 1)? else {
                    return Ok(None);
                };
                let Some(rhs) = self.operand(right, site, depth + 1)? else {
                    return Ok(None);
                };
                let (lhs, rhs) = if swap { (rhs, lhs) } else { (lhs, rhs) };
                Ok(Some((kind, lhs, rhs, invert)))
            }
            _ => Ok(None),
        }
    }

    fn controls(
        &mut self,
    ) -> Result<Vec<Option<ProjectedCfgTerminatorV1>>, ProductionRankedProjectionErrorV1> {
        self.proofs.charge(self.function.blocks().len())?;
        let mut controls = vec![None; self.function.blocks().len()];
        for (block, control) in controls.iter_mut().enumerate() {
            self.proofs.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = self.function.blocks()[block].terminator().kind()
            else {
                continue;
            };
            let discriminant = discriminant.clone();
            let targets = targets.clone();
            let site = ScalarAssignmentSiteV1 {
                block,
                statement: self.function.blocks()[block].statements().len(),
            };
            if let Some((kind, lhs, rhs, invert)) = self.comparison(&discriminant, site, 0)? {
                let mut branches = [targets.otherwise().target().index() as usize; 2];
                if targets.values().len() > 2 {
                    return Err(static_publication_reject_v1());
                }
                for target in targets.values() {
                    let Ok(value @ 0..=1) = usize::try_from(target.value()) else {
                        return Err(static_publication_reject_v1());
                    };
                    branches[value] = target.edge().target().index() as usize;
                }
                if targets.values().len() == 2
                    && !switch_fallback_is_empty_unreachable_v1(
                        self.function,
                        targets.otherwise().target().index() as usize,
                    )
                {
                    return Err(static_publication_reject_v1());
                }
                let (true_block, false_block) = if invert {
                    (branches[0], branches[1])
                } else {
                    (branches[1], branches[0])
                };
                *control = Some(ProjectedCfgTerminatorV1::PublicationComparison {
                    kind,
                    lhs,
                    rhs,
                    true_block,
                    false_block,
                });
            } else if let Some(value) = self.operand(&discriminant, site, 0)? {
                self.proofs.charge(targets.values().len())?;
                let mut exact = Vec::new();
                for target in targets.values() {
                    let literal = u64::try_from(target.value())
                        .map_err(|_| static_publication_reject_v1())?;
                    exact.push((
                        target.value(),
                        self.constant(literal)?,
                        target.edge().target().index() as usize,
                    ));
                }
                *control = Some(ProjectedCfgTerminatorV1::ExactSwitch(
                    ProjectedDeterministicSwitchV1 {
                        source_discriminant: discriminant,
                        discriminant: value,
                        targets: exact,
                        otherwise: targets.otherwise().target().index() as usize,
                        lane_uniform: false,
                    },
                ));
            }
        }
        Ok(controls)
    }
}

fn project_static_publication_control_v1(
    projector: &mut StaticPublicationProjectorV1<'_>,
    source: &StaticPublicationSourceV1,
    extents: [ProductionRankedValueV1; 2],
) -> Result<
    (
        ProductionRankedValueV1,
        Vec<Option<ProjectedCfgTerminatorV1>>,
    ),
    ProductionRankedProjectionErrorV1,
> {
    let mut proofs = SemanticAssertProofsV1::new(projector.types, projector.function)?;
    let count = projector.function.locals().len();
    proofs.charge(
        count
            .checked_mul(2)
            .ok_or_else(static_publication_reject_v1)?,
    )?;
    let mut calls = vec![None; count];
    for (block, body) in projector.function.blocks().iter().enumerate() {
        proofs.charge(1)?;
        if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind()
            && let Some(destination) = call.destination()
            && destination.place().projections().is_empty()
        {
            calls[destination.place().local().index() as usize] = Some(block);
        }
    }
    let mut exact = StaticPublicationControlProjectorV1 {
        types: projector.types,
        callables: projector.callables,
        function: projector.function,
        source,
        index_values: projector.index_values,
        extents,
        operations: projector.entry_operations,
        next_value: projector.next_value,
        proofs,
        calls,
        values: vec![None; count],
    };
    let controls = exact.controls()?;
    let mut index = None;
    for site in [source.producer, source.consumer] {
        let SemanticTerminatorKindV1::Call(call) =
            projector.function.blocks()[site.block].terminator().kind()
        else {
            return Err(static_publication_reject_v1());
        };
        let value = exact
            .operand(
                &call.arguments()[2],
                ScalarAssignmentSiteV1 {
                    block: site.block,
                    statement: projector.function.blocks()[site.block].statements().len(),
                },
                0,
            )?
            .ok_or_else(static_publication_reject_v1)?;
        if index
            .replace(value)
            .is_some_and(|previous| previous != value)
        {
            return Err(static_publication_reject_v1());
        }
    }
    Ok((index.ok_or_else(static_publication_reject_v1)?, controls))
}
