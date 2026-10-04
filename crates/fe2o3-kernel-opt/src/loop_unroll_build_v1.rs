use super::*;
use selection::{Emission, Plan, block_index, col, definition_block};

fn pay(extra: &mut usize, bytes: usize) -> Result<()> {
    *extra = extra.checked_add(bytes).ok_or(Resource::Arithmetic)?;
    Ok(())
}
fn table<T>(n: usize, extra: &mut usize, m: &mut Meter<'_, '_>) -> Result<Vec<T>> {
    let (v, bytes) = m.table(n)?;
    pay(extra, bytes)?;
    Ok(v)
}
fn value(
    i: &Inventory<'_>,
    p: &Plan,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    v: ValueId,
    copy: CopyRole,
    m: &mut Meter<'_, '_>,
) -> Result<ValueId> {
    let at = m
        .derive(|b| Ok(i.definition_index_for_value(function, v, b)?))?
        .ok_or(Error::Recipe("operand definition"))?;
    m.work(3)?;
    let column = if let Some(b) = definition_block(i.definitions()[at].coordinate) {
        if p.members[block_index(i, b)?] {
            col(copy)?
        } else {
            0
        }
    } else {
        0
    };
    p.values[at][column].ok_or(Error::Recipe("planned operand value"))
}
fn destination(i: &Inventory<'_>, p: &Plan, e: Edge, copy: CopyRole) -> Result<BlockId> {
    let b = &i.blocks()[block_index(i, e.source)?];
    let edge = i
        .edges()
        .get(b.edges.start + e.successor as usize)
        .filter(|row| row.coordinate == e)
        .ok_or(Error::Recipe("actual edge"))?;
    let at = block_index(i, edge.target)?;
    let column = if p.members[at] {
        if Some(e) == p.latch {
            col(copy)?.checked_add(1).ok_or(Resource::Arithmetic)?
        } else {
            col(copy)?
        }
    } else {
        0
    };
    p.blocks[at]
        .get(column)
        .copied()
        .flatten()
        .ok_or(Error::Recipe("planned target"))
}
fn ty(t: &Type, extra: &mut usize, m: &mut Meter<'_, '_>) -> Result<Type> {
    m.work(2)?;
    match t {
        Type::Scalar(s) => Ok(Type::Scalar(*s)),
        Type::Pointer(p) => {
            m.reserve(size_of::<Type>())?;
            pay(extra, size_of::<Type>())?;
            let Type::Scalar(s) = p.pointee.as_ref() else {
                return Err(Error::Recipe("scalar pointee"));
            };
            Ok(Type::Pointer(fe2o3_kernel_ir::PointerType {
                pointee: Box::new(Type::Scalar(*s)),
                address_space: p.address_space,
                access: p.access,
            }))
        }
        Type::Slice(s) => {
            m.reserve(size_of::<Type>())?;
            pay(extra, size_of::<Type>())?;
            let Type::Scalar(t) = s.element.as_ref() else {
                return Err(Error::Recipe("scalar slice element"));
            };
            Ok(Type::Slice(fe2o3_kernel_ir::SliceType {
                element: Box::new(Type::Scalar(*t)),
                address_space: s.address_space,
                access: s.access,
            }))
        }
        _ => Err(Error::Recipe("closed clone type")),
    }
}
fn kind(
    i: &Inventory<'_>,
    p: &Plan,
    site: Site,
    copy: CopyRole,
    k: &OperationKind,
    extra: &mut usize,
    m: &mut Meter<'_, '_>,
) -> Result<OperationKind> {
    m.work(3)?;
    let function = site.block.function;
    let mut map = |v| value(i, p, function, v, copy, m);
    Ok(match k {
        OperationKind::Constant(c) => OperationKind::Constant(c.clone()),
        OperationKind::Unary { op, operand } => OperationKind::Unary {
            op: *op,
            operand: map(*operand)?,
        },
        OperationKind::Binary { op, lhs, rhs } => OperationKind::Binary {
            op: *op,
            lhs: map(*lhs)?,
            rhs: map(*rhs)?,
        },
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => OperationKind::Compare {
            predicate: *predicate,
            lhs: map(*lhs)?,
            rhs: map(*rhs)?,
        },
        OperationKind::Cast { kind, value, to } => {
            let value = map(*value)?;
            OperationKind::Cast {
                kind: *kind,
                value,
                to: ty(to, extra, m)?,
            }
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => OperationKind::Select {
            condition: map(*condition)?,
            true_value: map(*true_value)?,
            false_value: map(*false_value)?,
        },
        OperationKind::SliceLength { slice } => OperationKind::SliceLength {
            slice: map(*slice)?,
        },
        OperationKind::SliceData { slice } => OperationKind::SliceData {
            slice: map(*slice)?,
        },
        OperationKind::GetElementPointer { base, offset } => OperationKind::GetElementPointer {
            base: map(*base)?,
            offset: map(*offset)?,
        },
        OperationKind::Load { pointer, access } => OperationKind::Load {
            pointer: map(*pointer)?,
            access: *access,
        },
        OperationKind::Store {
            pointer,
            value,
            access,
        } => OperationKind::Store {
            pointer: map(*pointer)?,
            value: map(*value)?,
            access: *access,
        },
        _ => return Err(Error::Recipe("closed cloned operation")),
    })
}
fn arguments(
    i: &Inventory<'_>,
    p: &Plan,
    e: Edge,
    copy: CopyRole,
    extra: &mut usize,
    m: &mut Meter<'_, '_>,
) -> Result<Vec<ValueId>> {
    let block = &i.blocks()[block_index(i, e.source)?];
    let edge = i
        .edges()
        .get(block.edges.start + e.successor as usize)
        .filter(|r| r.coordinate == e)
        .ok_or(Error::Recipe("argument edge"))?;
    let mut result = table(edge.arguments.len(), extra, m)?;
    for v in edge.arguments {
        let v = value(i, p, e.source.function, *v, copy, m)?;
        m.push(&mut result, v)?;
    }
    Ok(result)
}
fn terminator(
    i: &Inventory<'_>,
    p: &Plan,
    emission: Emission,
    extra: &mut usize,
    m: &mut Meter<'_, '_>,
) -> Result<Terminator> {
    m.work(4)?;
    let b = &i.blocks()[emission.input];
    let edge = |successor| Edge {
        source: b.coordinate,
        successor,
    };
    if let CopyRole::Header(n) = emission.copy {
        let selected = if n < p.selection.ok_or(Error::Recipe("trip count"))?.iterations {
            p.body
        } else {
            p.exit
        }
        .ok_or(Error::Recipe("header edge"))?;
        return Ok(Terminator::Branch {
            target: destination(i, p, selected, emission.copy)?,
            arguments: arguments(i, p, selected, emission.copy, extra, m)?,
        });
    }
    match b.terminator {
        Terminator::Branch { .. } => Ok(Terminator::Branch {
            target: destination(i, p, edge(0), emission.copy)?,
            arguments: arguments(i, p, edge(0), emission.copy, extra, m)?,
        }),
        Terminator::ConditionalBranch { condition, .. } => Ok(Terminator::ConditionalBranch {
            condition: value(i, p, b.coordinate.function, *condition, emission.copy, m)?,
            then_target: destination(i, p, edge(0), emission.copy)?,
            then_arguments: arguments(i, p, edge(0), emission.copy, extra, m)?,
            else_target: destination(i, p, edge(1), emission.copy)?,
            else_arguments: arguments(i, p, edge(1), emission.copy, extra, m)?,
        }),
        _ => Err(Error::Recipe("closed body terminator")),
    }
}
fn cloned(
    i: &Inventory<'_>,
    p: &Plan,
    e: Emission,
    extra: &mut usize,
    m: &mut Meter<'_, '_>,
) -> Result<BasicBlock> {
    let b = &i.blocks()[e.input];
    let mut parameters = table(b.parameters.len(), extra, m)?;
    for d in b.parameters.clone() {
        m.work(2)?;
        let v = ValueDef {
            id: p.values[d][col(e.copy)?].ok_or(Error::Recipe("parameter ID"))?,
            ty: ty(i.definitions()[d].ty, extra, m)?,
        };
        m.push(&mut parameters, v)?;
    }
    let mut operations = table(b.operations.len(), extra, m)?;
    for op in b.operations.clone() {
        m.work(2)?;
        let op = &i.operations()[op];
        let mut results = table(op.results.len(), extra, m)?;
        for d in op.results.clone() {
            m.work(2)?;
            let v = ValueDef {
                id: p.values[d][col(e.copy)?].ok_or(Error::Recipe("result ID"))?,
                ty: ty(i.definitions()[d].ty, extra, m)?,
            };
            m.push(&mut results, v)?;
        }
        let kind = kind(i, p, op.coordinate, e.copy, &op.operation.kind, extra, m)?;
        m.push(&mut operations, Operation { results, kind })?;
    }
    Ok(BasicBlock {
        id: e.id.ok_or(Error::Recipe("block ID"))?,
        parameters,
        operations,
        terminator: Some(terminator(i, p, e, extra, m)?),
    })
}

pub(super) fn materialize(
    i: &Inventory<'_>,
    _facts: &Facts<'_, '_, '_>,
    p: &Plan,
    candidate: &mut Module,
    m: &mut Meter<'_, '_>,
) -> Result<(Rows, usize)> {
    // Rows' six Vec headers are already included in the owning addition.
    let (blocks, _) = m.table(p.rows[0])?;
    let (definitions, _) = m.table(p.rows[1])?;
    let (operations, _) = m.table(p.rows[2])?;
    let (terminators, _) = m.table(p.rows[3])?;
    let (edges, _) = m.table(p.rows[4])?;
    let (arguments, _) = m.table(p.rows[5])?;
    let mut rows = Rows {
        selection: p.selection,
        blocks,
        definitions,
        operations,
        terminators,
        edges,
        arguments,
    };
    let mut cursor = 0usize;
    for f in i.functions() {
        m.work(2)?;
        for d in f.definitions.clone() {
            m.work(1)?;
            let input = i.definitions()[d].coordinate;
            if matches!(input, Definition::FunctionArgument { .. }) {
                m.push(
                    &mut rows.definitions,
                    Row {
                        input,
                        output: Some(input),
                        copy: CopyRole::Retained,
                    },
                )?;
            }
        }
        while let Some(e) = p.emissions.get(cursor).copied() {
            m.work(2)?;
            if i.blocks()[e.input].coordinate.function != f.coordinate {
                break;
            }
            origin_rows(i, p, e, &mut rows, m)?;
            cursor += 1;
        }
    }
    if cursor != p.emissions.len() {
        return Err(Error::Recipe("emission coverage"));
    }
    let mut extra = 0usize;
    let Some(header) = p.header else {
        return Ok((rows, extra));
    };
    let f = i.blocks()[header].coordinate.function;
    let target = candidate
        .functions
        .get_mut(f.0 as usize)
        .and_then(|f| f.body.as_mut())
        .ok_or(Error::Recipe("mutable function body"))?;
    let mut count = 0usize;
    for e in &p.emissions {
        m.work(2)?;
        if i.blocks()[e.input].coordinate.function == f && e.output.is_some() {
            count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    let temporary = size_of::<Vec<BasicBlock>>()
        .checked_add(size_of::<std::vec::IntoIter<BasicBlock>>())
        .ok_or(Resource::Arithmetic)?;
    m.reserve(temporary)?;
    pay(&mut extra, temporary)?;
    let mut new = table(count, &mut extra, m)?;
    m.work(i.owner().canonical().canonical_bytes().len())?;
    let old = std::mem::take(&mut target.blocks);
    let mut old = old.into_iter();
    let mut old_position = 0usize;
    for e in &p.emissions {
        m.work(2)?;
        let input = &i.blocks()[e.input];
        if input.coordinate.function != f || e.output.is_none() {
            continue;
        }
        let mut output = if e.copy == CopyRole::Retained {
            while old_position < input.coordinate.block as usize {
                m.work(1)?;
                drop(old.next().ok_or(Error::Recipe("old block gap"))?);
                old_position += 1;
            }
            old_position += 1;
            old.next().ok_or(Error::Recipe("old retained block"))?
        } else {
            cloned(i, p, *e, &mut extra, m)?
        };
        if e.copy == CopyRole::Retained {
            for edge in input.edges.clone() {
                m.work(2)?;
                let edge = &i.edges()[edge];
                if p.members[block_index(i, edge.target)?] {
                    let Some(Terminator::Branch { target, .. }) = &mut output.terminator else {
                        return Err(Error::Recipe("actual unconditional preheader"));
                    };
                    *target = destination(i, p, edge.coordinate, e.copy)?;
                }
            }
        }
        m.push(&mut new, output)?;
    }
    for block in old {
        m.work(1)?;
        drop(block);
    }
    if new.len() != count {
        return Err(Error::Recipe("materialized output count"));
    }
    target.blocks = new;
    Ok((rows, extra))
}

fn origin_rows(
    i: &Inventory<'_>,
    p: &Plan,
    e: Emission,
    rows: &mut Rows,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    m.work(3)?;
    let b = &i.blocks()[e.input];
    m.push(
        &mut rows.blocks,
        Row {
            input: b.coordinate,
            output: e.output,
            copy: e.copy,
        },
    )?;
    m.push(
        &mut rows.terminators,
        Row {
            input: b.coordinate,
            output: e.output,
            copy: e.copy,
        },
    )?;
    for d in b.parameters.clone().chain(
        b.operations
            .clone()
            .flat_map(|op| i.operations()[op].results.clone()),
    ) {
        m.work(3)?;
        let input = i.definitions()[d].coordinate;
        let output = e.output.map(|block| match input {
            Definition::BlockArgument { argument, .. } => {
                Definition::BlockArgument { block, argument }
            }
            Definition::Result { operation, result } => Definition::Result {
                operation: Site {
                    block,
                    operation: operation.operation,
                },
                result,
            },
            Definition::FunctionArgument { .. } => input,
        });
        m.push(
            &mut rows.definitions,
            Row {
                input,
                output,
                copy: e.copy,
            },
        )?;
    }
    for op in b.operations.clone() {
        m.work(2)?;
        let input = i.operations()[op].coordinate;
        m.push(
            &mut rows.operations,
            Row {
                input,
                output: e.output.map(|block| Site {
                    block,
                    operation: input.operation,
                }),
                copy: e.copy,
            },
        )?;
    }
    let selected = if let CopyRole::Header(n) = e.copy {
        if n < p.selection.ok_or(Error::Recipe("trip count"))?.iterations {
            p.body
        } else {
            p.exit
        }
    } else {
        None
    };
    for edge in b.edges.clone() {
        m.work(3)?;
        let edge = &i.edges()[edge];
        let output = e
            .output
            .filter(|_| selected.is_none_or(|s| s == edge.coordinate))
            .map(|source| Edge {
                source,
                successor: if selected.is_some() {
                    0
                } else {
                    edge.coordinate.successor
                },
            });
        m.push(
            &mut rows.edges,
            Row {
                input: edge.coordinate,
                output,
                copy: e.copy,
            },
        )?;
        for arg in edge.bindings.clone() {
            m.work(2)?;
            let input = i.edge_arguments()[arg].coordinate;
            m.push(
                &mut rows.arguments,
                Row {
                    input,
                    output: output.map(|edge| Argument {
                        edge,
                        argument: input.argument,
                    }),
                    copy: e.copy,
                },
            )?;
        }
    }
    Ok(())
}
