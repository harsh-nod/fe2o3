use super::*;

#[derive(Clone, Copy)]
pub(super) struct Emission {
    pub input: usize,
    pub output: Option<Block>,
    pub id: Option<BlockId>,
    pub copy: CopyRole,
}
pub(super) struct Plan {
    pub selection: Option<Selection>,
    pub header: Option<usize>,
    pub body: Option<Edge>,
    pub exit: Option<Edge>,
    pub latch: Option<Edge>,
    pub members: Vec<bool>,
    coverage: Vec<usize>,
    pub emissions: Vec<Emission>,
    pub blocks: Vec<[Option<BlockId>; 9]>,
    pub values: Vec<[Option<ValueId>; 9]>,
    pub rows: [usize; 6],
    output_counts: [usize; 6],
    next_block: u32,
    next_id: u64,
    next_value: u64,
}

pub(super) fn block_index(i: &Inventory<'_>, b: Block) -> Result<usize> {
    let f = i
        .functions()
        .get(b.function.0 as usize)
        .ok_or(Error::Recipe("function"))?;
    let at = f
        .blocks
        .start
        .checked_add(b.block as usize)
        .ok_or(Resource::Arithmetic)?;
    if !f.blocks.contains(&at) || i.blocks()[at].coordinate != b {
        return Err(Error::Recipe("block"));
    }
    Ok(at)
}
pub(super) fn definition_block(d: Definition) -> Option<Block> {
    match d {
        Definition::FunctionArgument { .. } => None,
        Definition::BlockArgument { block, .. } => Some(block),
        Definition::Result { operation, .. } => Some(operation.block),
    }
}
pub(super) fn col(c: CopyRole) -> Result<usize> {
    match c {
        CopyRole::Retained => Ok(0),
        CopyRole::Header(n) | CopyRole::Body(n) if n <= 8 => Ok(n as usize),
        _ => Err(Error::Recipe("copy")),
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
// Index is opaque transport only: scalar arithmetic/constants/casts below
// still reject it. SliceLength's existing result type is necessarily Index.
fn ty(t: &Type) -> bool {
    match t {
        Type::Scalar(ScalarType::Index) => true,
        Type::Scalar(_) => scalar(t),
        Type::Pointer(p) => scalar(&p.pointee),
        Type::Slice(s) => scalar(&s.element),
        Type::Unit | Type::Vector(_) | Type::Execution(_) => false,
    }
}
fn allowed(k: &OperationKind) -> bool {
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
        | OperationKind::Gfx942CompleteBodyStep(_) => false,
    }
}

fn candidate(
    i: &Inventory<'_>,
    f: &Facts<'_, '_, '_>,
    ordinal: usize,
    p: &Plan,
    m: &mut Meter<'_, '_>,
) -> Result<bool> {
    let row = f.rows()[ordinal];
    let Outcome::Guarded(g) = row.outcome() else {
        return Ok(false);
    };
    let loops = f.loops();
    let natural = m.derive(|b| Ok(loops.natural_loop(row.loop_ordinal(), b)?))?;
    let Some(pre) = natural.unconditional_preheader() else {
        return Ok(false);
    };
    if !natural.is_single_entry()
        || m.derive(|b| Ok(loops.external_header_edges(row.loop_ordinal(), b)?))? != [pre]
        || m.derive(|b| Ok(loops.latch_edges(row.loop_ordinal(), b)?))?
            != [row.recurrence().backedge()]
    {
        return Ok(false);
    }
    for (at, b) in i.blocks().iter().enumerate() {
        m.work(2)?;
        if !p.members[at] {
            continue;
        }
        if p.coverage[at] != 1 {
            return Ok(false);
        }
        if !matches!(
            b.terminator,
            Terminator::Branch { .. } | Terminator::ConditionalBranch { .. }
        ) {
            return Ok(false);
        }
        for d in b.parameters.clone() {
            m.work(2)?;
            if !ty(i.definitions()[d].ty) {
                return Ok(false);
            }
        }
        for op in b.operations.clone() {
            m.work(2)?;
            let op = &i.operations()[op];
            if !allowed(&op.operation.kind) {
                return Ok(false);
            }
            for d in op.results.clone() {
                m.work(2)?;
                if !ty(i.definitions()[d].ty) {
                    return Ok(false);
                }
            }
            for use_ in op.operands.clone() {
                m.work(3)?;
                let t = i.definitions()[i.uses()[use_].definition].ty;
                if !ty(t) {
                    return Ok(false);
                }
                if matches!(
                    op.operation.kind,
                    OperationKind::Unary { .. }
                        | OperationKind::Binary { .. }
                        | OperationKind::Compare { .. }
                        | OperationKind::Cast { .. }
                        | OperationKind::Select { .. }
                ) && !scalar(t)
                {
                    return Ok(false);
                }
            }
        }
    }
    for e in i.edges() {
        m.work(4)?;
        let from = p.members[block_index(i, e.coordinate.source)?];
        let to = p.members[block_index(i, e.target)?];
        if from && !to && e.coordinate != g.exit_edge() || !from && to && e.coordinate != pre {
            return Ok(false);
        }
    }
    for u in i.uses() {
        m.work(4)?;
        let use_block = match u.coordinate {
            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                operation, ..
            } => operation.block,
            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand { block, .. } => block,
        };
        if let Some(def) = definition_block(i.definitions()[u.definition].coordinate)
            && p.members[block_index(i, def)?]
            && !p.members[block_index(i, use_block)?]
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn cap(kind: &'static str, actual: usize, limit: usize) -> Result<()> {
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
fn add(a: &mut usize, b: usize) -> Result<()> {
    *a = a.checked_add(b).ok_or(Resource::Arithmetic)?;
    Ok(())
}
fn scaled(a: &mut usize, n: usize, copies: usize) -> Result<()> {
    add(a, n.checked_mul(copies).ok_or(Resource::Arithmetic)?)
}

// Count complete lineage before allocating emissions. Omitted zero-trip body
// rows remain present even though they contribute no output definitions/uses.
fn preflight(i: &Inventory<'_>, p: &mut Plan, m: &mut Meter<'_, '_>) -> Result<()> {
    for d in i.definitions() {
        m.work(1)?;
        if matches!(d.coordinate, Definition::FunctionArgument { .. }) {
            add(&mut p.rows[1], 1)?;
            add(&mut p.output_counts[1], 1)?;
        }
    }
    for (at, b) in i.blocks().iter().enumerate() {
        m.work(7)?;
        let trips = usize::from(p.selection.map_or(0, |s| s.iterations));
        let header = p.header == Some(at);
        let (copies, outputs) = if !p.members[at] {
            (1, 1)
        } else if header {
            let n = trips.checked_add(1).ok_or(Resource::Arithmetic)?;
            (n, n)
        } else {
            (trips.max(1), trips)
        };
        add(&mut p.rows[0], copies)?;
        add(&mut p.rows[3], copies)?;
        scaled(&mut p.rows[1], b.parameters.len(), copies)?;
        scaled(&mut p.rows[2], b.operations.len(), copies)?;
        add(&mut p.output_counts[0], outputs)?;
        scaled(&mut p.output_counts[1], b.parameters.len(), outputs)?;
        scaled(&mut p.output_counts[2], b.operations.len(), outputs)?;
        for op in b.operations.clone() {
            m.work(3)?;
            let op = &i.operations()[op];
            scaled(&mut p.rows[1], op.results.len(), copies)?;
            scaled(&mut p.output_counts[1], op.results.len(), outputs)?;
            scaled(&mut p.output_counts[4], op.operands.len(), outputs)?;
        }
        for e in b.edges.clone() {
            m.work(5)?;
            let e = &i.edges()[e];
            add(&mut p.rows[4], copies)?;
            scaled(&mut p.rows[5], e.arguments.len(), copies)?;
            let kept = if !header {
                outputs
            } else if Some(e.coordinate) == p.body {
                trips
            } else if Some(e.coordinate) == p.exit {
                1
            } else {
                0
            };
            add(&mut p.output_counts[3], kept)?;
            scaled(&mut p.output_counts[5], e.arguments.len(), kept)?;
            if header {
                scaled(&mut p.output_counts[4], e.arguments.len(), kept)?;
            }
        }
        if !header {
            scaled(&mut p.output_counts[4], b.terminator_uses.len(), outputs)?;
        }
    }
    Ok(())
}
fn output_caps(p: &Plan, limits: Limits) -> Result<()> {
    for (kind, n, limit) in [
        ("blocks", p.output_counts[0], limits.loops.blocks),
        ("definitions", p.output_counts[1], limits.loops.definitions),
        ("operations", p.output_counts[2], limits.loops.operations),
        ("edges", p.output_counts[3], limits.loops.edges),
        ("uses", p.output_counts[4], limits.max_output_operand_uses),
        (
            "edge arguments",
            p.output_counts[5],
            limits.max_output_edge_arguments,
        ),
    ] {
        cap(kind, n, limit)?;
    }
    Ok(())
}

pub(super) fn plan(
    i: &Inventory<'_>,
    facts: &Facts<'_, '_, '_>,
    limits: Limits,
    m: &mut Meter<'_, '_>,
) -> Result<(Plan, usize)> {
    m.reserve(size_of::<Plan>())?;
    let (mut members, a) = m.table(i.blocks().len())?;
    let (mut blocks, b) = m.table(i.blocks().len())?;
    let (mut values, c) = m.table(i.definitions().len())?;
    let (mut coverage, coverage_bytes) = m.table(i.blocks().len())?;
    for _ in i.blocks() {
        m.push(&mut members, false)?;
        m.push(&mut coverage, 0usize)?;
        m.push(&mut blocks, [None; 9])?;
    }
    for _ in i.definitions() {
        m.push(&mut values, [None; 9])?;
    }
    let mut p = Plan {
        selection: None,
        header: None,
        body: None,
        exit: None,
        latch: None,
        members,
        coverage,
        blocks,
        values,
        emissions: Vec::new(),
        rows: [0; 6],
        output_counts: [0; 6],
        next_block: 0,
        next_id: 0,
        next_value: 0,
    };
    for ordinal in 0..facts.loops().loop_count() {
        for block in m.derive(|b| Ok(facts.loops().members(ordinal, b)?))? {
            m.work(3)?;
            let at = block_index(i, *block)?;
            p.coverage[at] = p.coverage[at].checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    for (n, row) in facts.rows().iter().enumerate() {
        m.work(20)?;
        let Outcome::Guarded(g) = row.outcome() else {
            continue;
        };
        let Distance::Literal(trips) = g.guard_distance() else {
            continue;
        };
        if trips > 8
            || trips > u64::from(limits.max_iterations)
            || g.iteration_scope() != Completion::NormalHeaderCompletion
            || g.guarded_update()
                != (if trips == 0 {
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
        m.work(p.members.len())?;
        p.members.fill(false);
        for b in m.derive(|b| Ok(facts.loops().members(row.loop_ordinal(), b)?))? {
            m.work(2)?;
            p.members[block_index(i, *b)?] = true;
        }
        if !candidate(i, facts, n, &p, m)? {
            continue;
        }
        p.selection = Some(Selection {
            fact: n,
            iterations: trips as u8,
        });
        p.header = Some(block_index(
            i,
            m.derive(|b| Ok(facts.loops().natural_loop(row.loop_ordinal(), b)?))?
                .header(),
        )?);
        p.body = Some(g.body_edge());
        p.exit = Some(g.exit_edge());
        p.latch = Some(row.recurrence().backedge());
        break;
    }
    if p.selection.is_none() {
        m.work(p.members.len())?;
        p.members.fill(false);
    }
    preflight(i, &mut p, m)?;
    let block_bound = p.rows[0];
    let row_bound = p
        .rows
        .into_iter()
        .try_fold(0usize, |s, n| s.checked_add(n).ok_or(Resource::Arithmetic))?;
    cap("origin rows", row_bound, limits.max_origin_rows)?;
    output_caps(&p, limits)?;
    let (emissions, d) = m.table(block_bound)?;
    p.emissions = emissions;
    p.rows = [0; 6];
    p.output_counts = [0; 6];
    // Counts are blocks, definitions, operations, edges, uses, edge arguments.
    for f in i.functions() {
        m.work(2)?;
        p.next_block = 0;
        p.next_id = 0;
        p.next_value = 0;
        for at in f.blocks.clone() {
            m.work(2)?;
            p.next_id = p.next_id.max(u64::from(i.blocks()[at].block.id.0) + 1);
        }
        for at in f.definitions.clone() {
            m.work(2)?;
            if let Some(v) = i.definitions()[at].value {
                p.next_value = p.next_value.max(u64::from(v.0) + 1);
            }
        }
        for at in f.definitions.clone() {
            m.work(1)?;
            if matches!(
                i.definitions()[at].coordinate,
                Definition::FunctionArgument { .. }
            ) {
                p.values[at][0] = i.definitions()[at].value;
                add(&mut p.rows[1], 1)?;
                add(&mut p.output_counts[1], 1)?;
            }
        }
        for at in f.blocks.clone() {
            m.work(2)?;
            if p.header == Some(at) {
                let trips = p.selection.ok_or(Error::Recipe("selection"))?.iterations;
                for n in 0..=trips {
                    emit(i, &mut p, at, CopyRole::Header(n), m)?;
                    if n < trips {
                        for inner in f.blocks.clone() {
                            m.work(1)?;
                            if p.members[inner] && inner != at {
                                emit(i, &mut p, inner, CopyRole::Body(n), m)?;
                            }
                        }
                    }
                }
                if trips == 0 {
                    for inner in f.blocks.clone() {
                        m.work(1)?;
                        if p.members[inner] && inner != at {
                            emit(i, &mut p, inner, CopyRole::OmittedBody, m)?;
                        }
                    }
                }
            } else if !p.members[at] {
                emit(i, &mut p, at, CopyRole::Retained, m)?;
            }
        }
    }
    output_caps(&p, limits)?;
    let exact = p
        .rows
        .into_iter()
        .try_fold(0usize, |s, n| s.checked_add(n).ok_or(Resource::Arithmetic))?;
    cap("origin rows", exact, limits.max_origin_rows)?;
    if exact != row_bound || p.emissions.len() != block_bound {
        return Err(Error::Recipe("exact emission preflight"));
    }
    let bytes = [size_of::<Plan>(), a, b, c, d, coverage_bytes]
        .into_iter()
        .try_fold(0usize, |s, n| s.checked_add(n).ok_or(Resource::Arithmetic))?;
    Ok((p, bytes))
}

fn emit(
    i: &Inventory<'_>,
    p: &mut Plan,
    at: usize,
    copy: CopyRole,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    m.work(7)?;
    let b = &i.blocks()[at];
    let omitted = copy == CopyRole::OmittedBody;
    let (output, id) = if omitted {
        (None, None)
    } else {
        let coordinate = Block {
            function: b.coordinate.function,
            block: p.next_block,
        };
        p.next_block = p.next_block.checked_add(1).ok_or(Resource::Arithmetic)?;
        let id = if copy == CopyRole::Retained {
            b.block.id
        } else {
            let id = BlockId(u32::try_from(p.next_id).map_err(|_| Resource::Arithmetic)?);
            p.next_id += 1;
            id
        };
        p.blocks[at][col(copy)?] = Some(id);
        (Some(coordinate), Some(id))
    };
    m.push(
        &mut p.emissions,
        Emission {
            input: at,
            output,
            id,
            copy,
        },
    )?;
    add(&mut p.rows[0], 1)?;
    add(&mut p.rows[3], 1)?;
    add(&mut p.rows[2], b.operations.len())?;
    if !omitted {
        add(&mut p.output_counts[0], 1)?;
        add(&mut p.output_counts[2], b.operations.len())?;
    }
    for d in b.parameters.clone().chain(
        b.operations
            .clone()
            .flat_map(|op| i.operations()[op].results.clone()),
    ) {
        m.work(3)?;
        add(&mut p.rows[1], 1)?;
        if !omitted {
            add(&mut p.output_counts[1], 1)?;
            let v = if copy == CopyRole::Retained {
                i.definitions()[d]
                    .value
                    .ok_or(Error::Recipe("definition"))?
            } else {
                let v = ValueId(u32::try_from(p.next_value).map_err(|_| Resource::Arithmetic)?);
                p.next_value += 1;
                v
            };
            p.values[d][col(copy)?] = Some(v);
        }
    }
    if !omitted {
        for op in b.operations.clone() {
            m.work(1)?;
            add(&mut p.output_counts[4], i.operations()[op].operands.len())?;
        }
    }
    let selected = if let CopyRole::Header(n) = copy {
        if n < p.selection.ok_or(Error::Recipe("trip count"))?.iterations {
            p.body
        } else {
            p.exit
        }
    } else {
        None
    };
    for e in b.edges.clone() {
        m.work(3)?;
        let e = &i.edges()[e];
        add(&mut p.rows[4], 1)?;
        add(&mut p.rows[5], e.arguments.len())?;
        if !omitted && selected.is_none_or(|chosen| chosen == e.coordinate) {
            add(&mut p.output_counts[3], 1)?;
            add(&mut p.output_counts[5], e.arguments.len())?;
            if selected.is_some() {
                add(&mut p.output_counts[4], e.arguments.len())?;
            }
        }
    }
    if !omitted && selected.is_none() {
        add(&mut p.output_counts[4], b.terminator_uses.len())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "loop_unroll_complete_body_v19_tests.rs"]
mod complete_body_v19_tests;
