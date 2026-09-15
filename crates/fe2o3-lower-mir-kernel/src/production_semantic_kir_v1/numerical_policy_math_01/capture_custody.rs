// Field paths route retained values; only reference()/owned() authenticate issuers.
const MAX_MATH_CAPTURE_FIELDS_V1: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MathCapturePathV1 {
    pub(super) carrier: SemanticTypeIdV1,
    fields: [u32; MAX_MATH_CAPTURE_FIELDS_V1],
    len: u8,
}

impl MathCapturePathV1 {
    pub(super) fn fields(&self) -> &[u32] {
        &self.fields[..usize::from(self.len)]
    }

    fn checked(
        types: &[SemanticTypeDeclV1],
        carrier: SemanticTypeIdV1,
        projections: &[SemanticProjectionV1],
        reference: SemanticTypeIdV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if projections.is_empty() || projections.len() > MAX_MATH_CAPTURE_FIELDS_V1 {
            return Err(reject("Math capture field path exceeds the bounded route"));
        }
        let mut path = Self {
            carrier,
            fields: [0; MAX_MATH_CAPTURE_FIELDS_V1],
            len: projections.len() as u8,
        };
        let mut ty = carrier;
        for (slot, projection) in path.fields.iter_mut().zip(projections) {
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return Err(reject(
                    "Math capture requires exact aggregate fields, not pointer or index projections",
                ));
            };
            let fields = capture_fields(types, ty)?;
            ty = *fields
                .get(field as usize)
                .ok_or_else(|| reject("Math capture field is absent"))?;
            if ty != projection.result_type() {
                return Err(reject("Math capture field changed its nominal type edge"));
            }
            *slot = field;
        }
        if ty != reference {
            return Err(reject(
                "Math capture does not end at the exact shared reference",
            ));
        }
        Ok(path)
    }
}

pub(super) fn capture_fields(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<&[SemanticTypeIdV1], ProductionSemanticKirErrorV1> {
    match types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    {
        Some(SemanticTypeShapeV1::Tuple(tuple)) => Ok(tuple.fields()),
        Some(SemanticTypeShapeV1::Aggregate(aggregate)) => Ok(aggregate.fields()),
        _ => Err(reject(
            "Math capture carrier is not an exact tuple or aggregate",
        )),
    }
}

impl Resolver<'_, '_> {
    fn captured_reference_operand(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        tail: &[SemanticProjectionV1],
        reference: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(reject(
                "Math capture cannot issue a reference from a constant",
            ));
        };
        // Whole-carrier moves use the existing SSA move events. A projected move
        // needs separate partial-move lowering and is deliberately not added here.
        if matches!(operand, SemanticOperandV1::Move(_)) && !place.projections().is_empty() {
            return Err(reject(
                "Math capture projected moves require exact partial-move transport",
            ));
        }
        let len = place
            .projections()
            .len()
            .checked_add(tail.len())
            .filter(|len| *len <= MAX_MATH_CAPTURE_FIELDS_V1)
            .ok_or_else(|| reject("Math capture field path exceeds the bounded route"))?;
        self.graph.charge(len + 1)?;
        let mut projections = Vec::with_capacity(len);
        projections.extend_from_slice(place.projections());
        projections.extend_from_slice(tail);
        let value = self.graph.use_value(block, place.local().index())?;
        self.captured_reference(value, &projections, reference, role, depth + 1)
    }

    fn captured_reference(
        &mut self,
        value: SsaValueV1,
        projections: &[SemanticProjectionV1],
        reference: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        if projections.is_empty() {
            return self.reference(value, reference, role, depth + 1);
        }
        self.step(depth)?;
        self.graph.charge(projections.len())?;
        if !matches!(role, Role::Math | Role::Bound) {
            return Err(reject(
                "Math capture routing cannot introduce Context or policy authority",
            ));
        }
        let local = match value {
            SsaValueV1::BlockArgument { variable, .. } => variable.get(),
            _ => self.graph.definition(value)?.local,
        };
        let carrier = self
            .graph
            .body
            .locals()
            .get(local as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty();
        let path = MathCapturePathV1::checked(
            self.owner.source_semantic().types(),
            carrier,
            projections,
            reference,
        )?;
        let shape_work =
            math_capture_shape_work(self.owner.source_semantic().types(), carrier, path.fields())?;
        self.graph.charge(shape_work)?;
        let resolved = if let SsaValueV1::BlockArgument { block, variable } = value {
            let mut merged: Option<MathOwnerV1> = None;
            for incoming in self.graph.incoming(block.get(), variable.get())? {
                let candidate =
                    self.captured_reference(incoming, projections, reference, role, depth + 1)?;
                if let Some(previous) = &mut merged {
                    if !previous.same_owner(&candidate) {
                        return Err(reject(
                            "Math capture SSA join has distinct issuer definitions",
                        ));
                    }
                    self.graph.charge(candidate.loans.len())?;
                    previous.loans.extend(candidate.loans);
                } else {
                    merged = Some(candidate);
                }
            }
            merged.ok_or_else(|| reject("Math capture SSA argument lacks an original owner"))?
        } else {
            let site = self.graph.definition(value)?;
            let statement = site
                .statement
                .ok_or_else(|| reject("Math capture was returned by an unmodeled terminal"))?;
            let SemanticStatementKindV1::Assign(assignment) =
                self.graph.body.blocks()[site.block as usize].statements()[statement as usize]
                    .kind()
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if assignment.destination().ty() != carrier
                || assignment.value().result_type() != carrier
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    let operand = operand.clone();
                    self.captured_reference_operand(
                        site.block,
                        &operand,
                        projections,
                        reference,
                        role,
                        depth + 1,
                    )?
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    if !matches!(
                        aggregate.kind(),
                        SemanticAggregateKindV1::Tuple | SemanticAggregateKindV1::Aggregate
                    ) {
                        return Err(reject("Math capture cannot select enum or array authority"));
                    }
                    let fields = capture_fields(self.owner.source_semantic().types(), carrier)?;
                    if fields.len() != aggregate.operands().len() {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let field = path.fields()[0] as usize;
                    let operand = aggregate
                        .operands()
                        .get(field)
                        .filter(|operand| operand.ty() == projections[0].result_type())
                        .ok_or_else(|| reject("Math capture constructor operand changed its type"))?
                        .clone();
                    self.captured_reference_operand(
                        site.block,
                        &operand,
                        &projections[1..],
                        reference,
                        role,
                        depth + 1,
                    )?
                }
                _ => {
                    return Err(reject(
                        "Math capture lacks a retained aggregate constructor or SSA transfer",
                    ));
                }
            }
        };
        let transport = MathTransportV1 {
            contract: self.contract,
            bound: role == Role::Bound,
            capture: Some(path),
        };
        if let Some(previous) = self.transports.get(&local) {
            if !previous.same_binding(transport) {
                return Err(reject(
                    "Math capture local has conflicting checked field routes",
                ));
            }
        } else {
            // Charge the fixed path and descriptor before persisting it. This is
            // logical bounded state accounting, not a physical allocator limit.
            self.graph.charge(MAX_MATH_CAPTURE_FIELDS_V1 + 2)?;
            self.transports.insert(local, transport);
        }
        Ok(resolved)
    }
}
