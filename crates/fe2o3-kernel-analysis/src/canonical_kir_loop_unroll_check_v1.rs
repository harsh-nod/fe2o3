use super::*;

fn bi(i: &Inventory<'_>, b: Block) -> Result<usize> {
    let f = i
        .functions()
        .get(b.function.0 as usize)
        .ok_or(Error::Mismatch("function"))?;
    let at = f
        .blocks
        .start
        .checked_add(b.block as usize)
        .ok_or(Resource::Arithmetic)?;
    if !f.blocks.contains(&at) || i.blocks()[at].coordinate != b {
        return Err(Error::Mismatch("block coordinate"));
    }
    Ok(at)
}
fn owner_block(d: Definition) -> Option<Block> {
    match d {
        Definition::FunctionArgument { .. } => None,
        Definition::BlockArgument { block, .. } => Some(block),
        Definition::Result { operation, .. } => Some(operation.block),
    }
}
fn scalar(t: &Type) -> bool {
    matches!(
        t,
        Type::Scalar(
            ScalarType::Bool
                | ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::I128
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
                | ScalarType::U128
        )
    )
}
fn closed_type(t: &Type) -> bool {
    match t {
        Type::Scalar(ScalarType::Index) => true,
        Type::Scalar(_) => scalar(t),
        Type::Pointer(p) => scalar(&p.pointee),
        Type::Slice(s) => scalar(&s.element),
        Type::Unit | Type::Vector(_) | Type::Execution(_) => false,
    }
}
fn closed_op(k: &OperationKind) -> bool {
    match k {
        OperationKind::Constant(c) => scalar(&c.ty()),
        OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Select { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. } => true,
        OperationKind::Cast { kind, to, .. } => {
            scalar(to)
                && matches!(
                    kind,
                    CastKind::Truncate
                        | CastKind::ZeroExtend
                        | CastKind::SignExtend
                        | CastKind::Bitcast
                )
        }
        OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => {
            !access.volatile
                && matches!(
                    access.address_space,
                    AddressSpace::Private | AddressSpace::Global
                )
        }
        OperationKind::Execution(_)
        | OperationKind::VerificationContract(_)
        | OperationKind::VectorLoad(_)
        | OperationKind::VectorStore(_)
        | OperationKind::VectorLayoutConvert(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Call { .. }
        | OperationKind::Alloca { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Wave(_)
        | OperationKind::InlineAssembly(_)
        | OperationKind::Gfx942OrderedRegion(_)
        | OperationKind::Gfx942OrderedProgram(_)
        | OperationKind::Gfx942CompleteBodyDeclaration(_)
        | OperationKind::Gfx942CompleteBodyStep(_)
        | OperationKind::Gfx942PhysicalEntryDeclaration(_)
        | OperationKind::Gfx942PhysicalEntryStep(_)
        | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
        | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
        | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
        | OperationKind::Gfx942PhysicalLdsExchangeStep(_) => false,
    }
}

// This eligibility scan is deliberately independent of the optimizer selector.
fn select(
    i: &Inventory<'_>,
    facts: &Facts<'_, '_, '_>,
    limits: Limits,
    members: &mut [bool],
    coverage: &[usize],
    meter: &mut Meter<'_, '_>,
) -> Result<Option<Selection>> {
    let loops = facts.loops();
    for (ordinal, row) in facts.rows().iter().enumerate() {
        meter.work(20)?;
        let Outcome::Guarded(g) = row.outcome() else {
            continue;
        };
        let Distance::Literal(n) = g.guard_distance() else {
            continue;
        };
        if n > u64::from(limits.max_iterations)
            || n > 8
            || g.iteration_scope() != Completion::NormalHeaderCompletion
            || g.guarded_update()
                != (if n == 0 {
                    Update::NoUpdate
                } else {
                    Update::NonWrapping
                })
            || !matches!(
                row.recurrence().scalar(),
                ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64
            )
        {
            continue;
        }
        let l = meter.derive(|b| Ok(loops.natural_loop(row.loop_ordinal(), b)?))?;
        let Some(pre) = l.unconditional_preheader() else {
            continue;
        };
        if !l.is_single_entry() {
            continue;
        }
        let external = meter.derive(|b| Ok(loops.external_header_edges(row.loop_ordinal(), b)?))?;
        let latches = meter.derive(|b| Ok(loops.latch_edges(row.loop_ordinal(), b)?))?;
        if external != [pre] || latches != [row.recurrence().backedge()] {
            continue;
        }
        meter.work(members.len())?;
        members.fill(false);
        for block in meter.derive(|b| Ok(loops.members(row.loop_ordinal(), b)?))? {
            meter.work(2)?;
            members[bi(i, *block)?] = true;
        }
        let mut allowed = true;
        for (at, block) in i.blocks().iter().enumerate() {
            meter.work(2)?;
            if !members[at] {
                continue;
            }
            if coverage[at] != 1 {
                allowed = false;
            }
            if !matches!(
                block.terminator,
                Terminator::Branch { .. } | Terminator::ConditionalBranch { .. }
            ) {
                allowed = false;
            }
            for d in block.parameters.clone() {
                meter.work(2)?;
                allowed &= closed_type(i.definitions()[d].ty);
            }
            for op in block.operations.clone() {
                meter.work(2)?;
                let op = &i.operations()[op];
                allowed &= closed_op(&op.operation.kind);
                for d in op.results.clone() {
                    meter.work(2)?;
                    allowed &= closed_type(i.definitions()[d].ty);
                }
                for u in op.operands.clone() {
                    meter.work(3)?;
                    let ty = i.definitions()[i.uses()[u].definition].ty;
                    allowed &= closed_type(ty);
                    if matches!(
                        op.operation.kind,
                        OperationKind::Unary { .. }
                            | OperationKind::Binary { .. }
                            | OperationKind::Compare { .. }
                            | OperationKind::Cast { .. }
                            | OperationKind::Select { .. }
                    ) {
                        allowed &= scalar(ty);
                    }
                }
            }
        }
        for edge in i.edges() {
            meter.work(4)?;
            let from = members[bi(i, edge.coordinate.source)?];
            let to = members[bi(i, edge.target)?];
            if (from && !to && edge.coordinate != g.exit_edge())
                || (!from && to && edge.coordinate != pre)
            {
                allowed = false;
            }
        }
        for use_ in i.uses() {
            meter.work(4)?;
            let at = match use_.coordinate {
                fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                    operation,
                    ..
                } => operation.block,
                fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand {
                    block, ..
                } => block,
            };
            if !members[bi(i, at)?]
                && let Some(d) = owner_block(i.definitions()[use_.definition].coordinate)
                && members[bi(i, d)?]
            {
                allowed = false;
            }
        }
        if allowed {
            return Ok(Some(Selection {
                fact: ordinal,
                iterations: n as u8,
            }));
        }
    }
    meter.work(members.len())?;
    members.fill(false);
    Ok(None)
}

fn bound(kind: &'static str, actual: usize, limit: usize) -> Result<()> {
    if actual > limit {
        Err(Error::OutputLimit {
            kind,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}
fn counts(i: &Inventory<'_>, limits: Limits) -> Result<()> {
    for (kind, n, cap) in [
        ("functions", i.functions().len(), limits.loops.functions),
        ("blocks", i.blocks().len(), limits.loops.blocks),
        (
            "definitions",
            i.definitions().len(),
            limits.loops.definitions,
        ),
        ("operations", i.operations().len(), limits.loops.operations),
        ("edges", i.edges().len(), limits.loops.edges),
        ("uses", i.uses().len(), limits.max_output_operand_uses),
        (
            "edge arguments",
            i.edge_arguments().len(),
            limits.max_output_edge_arguments,
        ),
    ] {
        bound(kind, n, cap)?;
    }
    Ok(())
}
fn take<C: Copy + Eq>(
    rows: &[Row<C>],
    cursor: &mut usize,
    expected: Row<C>,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(2)?;
    if rows.get(*cursor) != Some(&expected) {
        return Err(Error::Mismatch("complete ordered origin"));
    }
    *cursor = cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(())
}
fn column(copy: CopyRole) -> Result<usize> {
    match copy {
        CopyRole::Retained => Ok(0),
        CopyRole::Header(n) | CopyRole::Body(n) if n <= 8 => Ok(n as usize),
        _ => Err(Error::Mismatch("copy ordinal")),
    }
}

struct Scratch {
    members: Vec<bool>,
    coverage: Vec<usize>,
    blocks: Vec<[Option<(Block, BlockId)>; 9]>,
    values: Vec<[Option<ValueId>; 9]>,
    cursors: [usize; 6],
    next_block: u32,
    next_id: u64,
    next_value: u64,
}
impl Scratch {
    fn new(i: &Inventory<'_>, m: &mut Meter<'_, '_>) -> Result<(Self, usize)> {
        m.reserve(size_of::<Self>())?;
        let (mut members, a) = m.table(i.blocks().len())?;
        let (mut blocks, b) = m.table(i.blocks().len())?;
        let (mut values, c) = m.table(i.definitions().len())?;
        let (mut coverage, d) = m.table(i.blocks().len())?;
        for _ in i.blocks() {
            m.push(&mut members, false)?;
            m.push(&mut coverage, 0usize)?;
            m.push(&mut blocks, [None; 9])?;
        }
        for _ in i.definitions() {
            m.push(&mut values, [None; 9])?;
        }
        let bytes = [size_of::<Self>(), a, b, c, d]
            .into_iter()
            .try_fold(0usize, |s, n| s.checked_add(n).ok_or(Resource::Arithmetic))?;
        Ok((
            Self {
                members,
                coverage,
                blocks,
                values,
                cursors: [0; 6],
                next_block: 0,
                next_id: 0,
                next_value: 0,
            },
            bytes,
        ))
    }
}

pub(super) fn verify(
    i: &Inventory<'_>,
    o: &Inventory<'_>,
    facts: &Facts<'_, '_, '_>,
    rows: Origins<'_>,
    limits: Limits,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    m.work(12)?;
    counts(o, limits)?;
    let total = [
        rows.blocks.len(),
        rows.definitions.len(),
        rows.operations.len(),
        rows.terminators.len(),
        rows.edges.len(),
        rows.arguments.len(),
    ]
    .into_iter()
    .try_fold(0usize, |s, n| s.checked_add(n).ok_or(Resource::Arithmetic))?;
    bound("origin rows", total, limits.max_origin_rows)?;
    // Prepay deep unchanged metadata/payload comparisons; dynamic lists have
    // further per-occurrence charges below. This is not a capacity reservation.
    m.work(
        i.owner()
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(o.owner().canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let (mut scratch, bytes) = Scratch::new(i, m)?;
    for ordinal in 0..facts.loops().loop_count() {
        for block in m.derive(|b| Ok(facts.loops().members(ordinal, b)?))? {
            m.work(3)?;
            let at = bi(i, *block)?;
            scratch.coverage[at] = scratch.coverage[at]
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
        }
    }
    let selected = select(i, facts, limits, &mut scratch.members, &scratch.coverage, m)?;
    if selected != rows.selection {
        return Err(Error::Mismatch("first qualifying literal fact"));
    }
    let im = i.owner().module();
    let om = o.owner().module();
    if im.id != om.id
        || im.required_capabilities != om.required_capabilities
        || im.kernels != om.kernels
        || im.functions.len() != om.functions.len()
    {
        return Err(Error::Mismatch("module target/kernel/function roster"));
    }
    let header = if let Some(s) = selected {
        Some(
            m.derive(|b| {
                Ok(facts
                    .loops()
                    .natural_loop(facts.rows()[s.fact].loop_ordinal(), b)?)
            })?
            .header(),
        )
    } else {
        None
    };
    for (f, of) in i.functions().iter().zip(o.functions()) {
        m.work(6)?;
        let a = f.function;
        let b = of.function;
        if a.id != b.id
            || a.signature != b.signature
            || a.role != b.role
            || a.required_capabilities != b.required_capabilities
            || a.body.as_ref().map(|x| &x.parameters) != b.body.as_ref().map(|x| &x.parameters)
        {
            return Err(Error::Mismatch("function metadata"));
        }
        scratch.next_id = 0;
        scratch.next_value = 0;
        scratch.next_block = 0;
        for block in f.blocks.clone() {
            m.work(2)?;
            scratch.next_id = scratch
                .next_id
                .max(u64::from(i.blocks()[block].block.id.0) + 1);
        }
        for d in f.definitions.clone() {
            m.work(2)?;
            if let Some(v) = i.definitions()[d].value {
                scratch.next_value = scratch.next_value.max(u64::from(v.0) + 1);
            }
        }
        for d in f.definitions.clone() {
            m.work(1)?;
            if matches!(
                i.definitions()[d].coordinate,
                Definition::FunctionArgument { .. }
            ) {
                let input = i.definitions()[d].coordinate;
                take(
                    rows.definitions,
                    &mut scratch.cursors[1],
                    Row {
                        input,
                        output: Some(input),
                        copy: CopyRole::Retained,
                    },
                    m,
                )?;
                scratch.values[d][0] = i.definitions()[d].value;
            }
        }
        for at in f.blocks.clone() {
            m.work(2)?;
            let block = i.blocks()[at].coordinate;
            if Some(block) == header {
                let s = selected.ok_or(Error::Mismatch("selection"))?;
                for n in 0..=s.iterations {
                    layout(i, o, rows, &mut scratch, at, CopyRole::Header(n), m)?;
                    if n < s.iterations {
                        for inner in f.blocks.clone() {
                            m.work(1)?;
                            if scratch.members[inner] && inner != at {
                                layout(i, o, rows, &mut scratch, inner, CopyRole::Body(n), m)?;
                            }
                        }
                    }
                }
                if s.iterations == 0 {
                    for inner in f.blocks.clone() {
                        m.work(1)?;
                        if scratch.members[inner] && inner != at {
                            layout(i, o, rows, &mut scratch, inner, CopyRole::OmittedBody, m)?;
                        }
                    }
                }
            } else if !scratch.members[at] {
                layout(i, o, rows, &mut scratch, at, CopyRole::Retained, m)?;
            }
        }
        if scratch.next_block as usize != of.blocks.len() {
            return Err(Error::Mismatch("output block coverage"));
        }
    }
    // Layout/definitions are established before any use, including forward
    // textual block order and backedge payloads. Never trust a row as a map.
    for row in rows.blocks {
        m.work(3)?;
        payload(i, o, facts, rows, &mut scratch, *row, m)?;
    }
    if scratch.cursors
        != [
            rows.blocks.len(),
            rows.definitions.len(),
            rows.operations.len(),
            rows.terminators.len(),
            rows.edges.len(),
            rows.arguments.len(),
        ]
    {
        return Err(Error::Mismatch("unused origin tail"));
    }
    if selected.is_none()
        && i.owner().canonical().canonical_bytes() != o.owner().canonical().canonical_bytes()
    {
        return Err(Error::Mismatch("exact unsupported no-op"));
    }
    drop(scratch);
    m.release(bytes)?;
    Ok(())
}

fn layout(
    i: &Inventory<'_>,
    o: &Inventory<'_>,
    rows: Origins<'_>,
    s: &mut Scratch,
    at: usize,
    copy: CopyRole,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    m.work(5)?;
    let block = &i.blocks()[at];
    let output = if copy == CopyRole::OmittedBody {
        None
    } else {
        let b = Block {
            function: block.coordinate.function,
            block: s.next_block,
        };
        s.next_block = s.next_block.checked_add(1).ok_or(Resource::Arithmetic)?;
        Some(b)
    };
    take(
        rows.blocks,
        &mut s.cursors[0],
        Row {
            input: block.coordinate,
            output,
            copy,
        },
        m,
    )?;
    if let Some(out) = output {
        let actual = &o.blocks()[bi(o, out)?];
        let expected_id = if copy == CopyRole::Retained {
            block.block.id
        } else {
            let id = BlockId(u32::try_from(s.next_id).map_err(|_| Resource::Arithmetic)?);
            s.next_id += 1;
            id
        };
        if actual.block.id != expected_id
            || actual.parameters.len() != block.parameters.len()
            || actual.operations.len() != block.operations.len()
        {
            return Err(Error::Mismatch("block copy shape/fresh ID"));
        }
        s.blocks[at][column(copy)?] = Some((out, expected_id));
    }
    for d in block.parameters.clone().chain(
        block
            .operations
            .clone()
            .flat_map(|op| i.operations()[op].results.clone()),
    ) {
        m.work(4)?;
        let input = i.definitions()[d].coordinate;
        let mapped = output.map(|out| match input {
            Definition::BlockArgument { argument, .. } => Definition::BlockArgument {
                block: out,
                argument,
            },
            Definition::Result { operation, result } => Definition::Result {
                operation: Site {
                    block: out,
                    operation: operation.operation,
                },
                result,
            },
            Definition::FunctionArgument { .. } => input,
        });
        take(
            rows.definitions,
            &mut s.cursors[1],
            Row {
                input,
                output: mapped,
                copy,
            },
            m,
        )?;
        if let Some(mapped) = mapped {
            let ob = &o.blocks()[bi(o, output.ok_or(Error::Mismatch("definition output"))?)?];
            let od = match mapped {
                Definition::BlockArgument { argument, .. } => {
                    ob.parameters.start + argument as usize
                }
                Definition::Result { operation, result } => {
                    o.operations()[ob.operations.start + operation.operation as usize]
                        .results
                        .start
                        + result as usize
                }
                Definition::FunctionArgument { .. } => {
                    return Err(Error::Mismatch("block definition"));
                }
            };
            let actual = o
                .definitions()
                .get(od)
                .ok_or(Error::Mismatch("definition roster"))?;
            let value = if copy == CopyRole::Retained {
                i.definitions()[d].value.ok_or(Error::Mismatch("value"))?
            } else {
                let v = ValueId(u32::try_from(s.next_value).map_err(|_| Resource::Arithmetic)?);
                s.next_value += 1;
                v
            };
            if actual.coordinate != mapped
                || actual.value != Some(value)
                || actual.ty != i.definitions()[d].ty
            {
                return Err(Error::Mismatch("definition type/fresh value"));
            }
            let col = column(copy)?;
            s.values[d][col] = Some(value);
        }
    }
    Ok(())
}

fn value(
    i: &Inventory<'_>,
    s: &Scratch,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    v: ValueId,
    copy: CopyRole,
    m: &mut Meter<'_, '_>,
) -> Result<ValueId> {
    let d = m
        .derive(|b| Ok(i.definition_index_for_value(function, v, b)?))?
        .ok_or(Error::Mismatch("actual operand definition"))?;
    m.work(3)?;
    let col = if let Some(b) = owner_block(i.definitions()[d].coordinate) {
        if s.members[bi(i, b)?] {
            column(copy)?
        } else {
            0
        }
    } else {
        0
    };
    s.values[d][col].ok_or(Error::Mismatch("mapped operand"))
}

fn kind(
    i: &Inventory<'_>,
    s: &Scratch,
    site: Site,
    copy: CopyRole,
    a: &OperationKind,
    b: &OperationKind,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    m.work(3)?;
    let mut same =
        |x, y| -> Result<bool> { Ok(value(i, s, site.block.function, x, copy, m)? == y) };
    let ok = match (a, b) {
        (OperationKind::Constant(a), OperationKind::Constant(b)) => a == b,
        (
            OperationKind::Unary { op: a, operand: x },
            OperationKind::Unary { op: b, operand: y },
        ) => a == b && same(*x, *y)?,
        (
            OperationKind::Binary {
                op: a,
                lhs: x,
                rhs: y,
            },
            OperationKind::Binary {
                op: b,
                lhs: u,
                rhs: v,
            },
        ) => a == b && same(*x, *u)? && same(*y, *v)?,
        (
            OperationKind::Compare {
                predicate: a,
                lhs: x,
                rhs: y,
            },
            OperationKind::Compare {
                predicate: b,
                lhs: u,
                rhs: v,
            },
        ) => a == b && same(*x, *u)? && same(*y, *v)?,
        (
            OperationKind::Cast {
                kind: a,
                value: x,
                to: t,
            },
            OperationKind::Cast {
                kind: b,
                value: y,
                to: u,
            },
        ) => a == b && t == u && same(*x, *y)?,
        (
            OperationKind::Select {
                condition: a,
                true_value: b,
                false_value: c,
            },
            OperationKind::Select {
                condition: x,
                true_value: y,
                false_value: z,
            },
        ) => same(*a, *x)? && same(*b, *y)? && same(*c, *z)?,
        (OperationKind::SliceLength { slice: x }, OperationKind::SliceLength { slice: y })
        | (OperationKind::SliceData { slice: x }, OperationKind::SliceData { slice: y }) => {
            same(*x, *y)?
        }
        (
            OperationKind::GetElementPointer { base: a, offset: b },
            OperationKind::GetElementPointer { base: x, offset: y },
        ) => same(*a, *x)? && same(*b, *y)?,
        (
            OperationKind::Load {
                pointer: a,
                access: x,
            },
            OperationKind::Load {
                pointer: b,
                access: y,
            },
        ) => x == y && same(*a, *b)?,
        (
            OperationKind::Store {
                pointer: a,
                value: b,
                access: x,
            },
            OperationKind::Store {
                pointer: c,
                value: d,
                access: y,
            },
        ) => x == y && same(*a, *c)? && same(*b, *d)?,
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(Error::Mismatch("cloned operation payload/operand order"))
    }
}

fn payload(
    i: &Inventory<'_>,
    o: &Inventory<'_>,
    facts: &Facts<'_, '_, '_>,
    rows: Origins<'_>,
    s: &mut Scratch,
    row: Row<Block>,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    let at = bi(i, row.input)?;
    let block = &i.blocks()[at];
    let ob = row
        .output
        .map(|b| bi(o, b))
        .transpose()?
        .map(|n| &o.blocks()[n]);
    for op in block.operations.clone() {
        m.work(3)?;
        let old = &i.operations()[op];
        let output = row.output.map(|block| Site {
            block,
            operation: old.coordinate.operation,
        });
        take(
            rows.operations,
            &mut s.cursors[2],
            Row {
                input: old.coordinate,
                output,
                copy: row.copy,
            },
            m,
        )?;
        if let Some(ob) = ob {
            let new = o
                .operations()
                .get(ob.operations.start + old.coordinate.operation as usize)
                .ok_or(Error::Mismatch("operation roster"))?;
            if old.results.len() != new.results.len() {
                return Err(Error::Mismatch("result count"));
            }
            if row.copy == CopyRole::Retained {
                if old.operation != new.operation {
                    return Err(Error::Mismatch("outside operation"));
                }
            } else {
                kind(
                    i,
                    s,
                    old.coordinate,
                    row.copy,
                    &old.operation.kind,
                    &new.operation.kind,
                    m,
                )?;
            }
        }
    }
    take(
        rows.terminators,
        &mut s.cursors[3],
        Row {
            input: row.input,
            output: row.output,
            copy: row.copy,
        },
        m,
    )?;
    let selection = rows.selection;
    let selected_edge = if let CopyRole::Header(n) = row.copy {
        let selected = selection.ok_or(Error::Mismatch("header fact"))?;
        let Outcome::Guarded(g) = facts.rows()[selected.fact].outcome() else {
            return Err(Error::Mismatch("guard"));
        };
        Some(if n < selected.iterations {
            g.body_edge()
        } else {
            g.exit_edge()
        })
    } else {
        None
    };
    if let Some(ob) = ob {
        match (block.terminator, ob.terminator) {
            (
                Terminator::ConditionalBranch { condition: a, .. },
                Terminator::ConditionalBranch { condition: b, .. },
            ) if selected_edge.is_none() => {
                if value(i, s, row.input.function, *a, row.copy, m)? != *b {
                    return Err(Error::Mismatch("condition"));
                }
            }
            (_, Terminator::Branch { .. }) if selected_edge.is_some() => {}
            (Terminator::Branch { .. }, Terminator::Branch { .. }) => {}
            (a, b) if row.copy == CopyRole::Retained && a == b => {}
            _ => return Err(Error::Mismatch("terminator shape")),
        }
    }
    let mut successors = 0usize;
    for e in block.edges.clone() {
        m.work(4)?;
        let edge = &i.edges()[e];
        let output = row
            .output
            .filter(|_| selected_edge.is_none_or(|chosen| chosen == edge.coordinate))
            .map(|source| Edge {
                source,
                successor: if selected_edge.is_some() {
                    0
                } else {
                    edge.coordinate.successor
                },
            });
        take(
            rows.edges,
            &mut s.cursors[4],
            Row {
                input: edge.coordinate,
                output,
                copy: row.copy,
            },
            m,
        )?;
        if let Some(output) = output {
            successors += 1;
            let ob = ob.ok_or(Error::Mismatch("edge block"))?;
            let oe = o
                .edges()
                .get(ob.edges.start + output.successor as usize)
                .filter(|e| e.coordinate == output)
                .ok_or(Error::Mismatch("output edge occurrence"))?;
            let dest = bi(i, edge.target)?;
            let col = if s.members[dest] {
                match row.copy {
                    CopyRole::Retained => 0,
                    CopyRole::Header(n) => n as usize,
                    CopyRole::Body(n) => {
                        let selected = selection.ok_or(Error::Mismatch("latch selection"))?;
                        if edge.coordinate == facts.rows()[selected.fact].recurrence().backedge() {
                            n as usize + 1
                        } else {
                            n as usize
                        }
                    }
                    CopyRole::OmittedBody => return Err(Error::Mismatch("omitted edge")),
                }
            } else {
                0
            };
            let (target, id) = s.blocks[dest][col].ok_or(Error::Mismatch("mapped destination"))?;
            if oe.target != target
                || oe.target_id != id
                || oe.arguments.len() != edge.arguments.len()
            {
                return Err(Error::Mismatch("edge target/arity"));
            }
        }
        for a in edge.bindings.clone() {
            m.work(3)?;
            let old = &i.edge_arguments()[a];
            let mapped = output.map(|edge| Argument {
                edge,
                argument: old.coordinate.argument,
            });
            take(
                rows.arguments,
                &mut s.cursors[5],
                Row {
                    input: old.coordinate,
                    output: mapped,
                    copy: row.copy,
                },
                m,
            )?;
            if let Some(mapped) = mapped {
                let ob = ob.ok_or(Error::Mismatch("argument block"))?;
                let oe = &o.edges()[ob.edges.start + mapped.edge.successor as usize];
                let oa = o
                    .edge_arguments()
                    .get(oe.bindings.start + mapped.argument as usize)
                    .filter(|a| a.coordinate == mapped)
                    .ok_or(Error::Mismatch("argument occurrence"))?;
                if value(i, s, row.input.function, old.value, row.copy, m)? != oa.value {
                    return Err(Error::Mismatch("edge argument value"));
                }
            }
        }
    }
    if let Some(ob) = ob
        && ob.edges.len() != successors
    {
        return Err(Error::Mismatch("edge coverage"));
    }
    Ok(())
}
