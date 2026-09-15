use super::*;
type Result<T> = std::result::Result<T, SemanticMirErrorV1>;
fn require(ok: bool) -> Result<()> {
    ok.then_some(())
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)
}
fn spend(work: &mut u64, n: usize) -> Result<()> {
    let before = *work;
    let n = u64::try_from(n).map_err(|_| SemanticMirErrorV1::InvalidFunctionAbi)?;
    *work = before.saturating_sub(n);
    if n > before {
        return Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: n,
            max: before,
        });
    }
    Ok(())
}
fn call(
    functions: &[SemanticFunctionDeclV1],
    s: SemanticOwnedSourceCallSiteV1,
) -> Result<&SemanticDirectCallV1> {
    let b = functions
        .get(s.function.index() as usize)
        .and_then(|f| f.blocks().get(s.block.index() as usize))
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let SemanticTerminatorKindV1::Call(c) = b.terminator().kind() else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(
        c.unwind() == SemanticUnwindActionV1::Unreachable && c.variadic_argument_abis().is_empty(),
    )?;
    let d = c
        .destination()
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(
        d.edge().role() == SemanticEdgeRoleV1::CallReturn && d.place().projections().is_empty(),
    )?;
    Ok(c)
}
fn transpose(
    callables: &[SemanticCallableDeclV1],
    call: &SemanticDirectCallV1,
) -> Result<(
    SemanticExecutionCapabilityContractV1,
    SemanticGfx950TransposeContractV1,
)> {
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        ..
    }) = callables.get(call.callee().index() as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t) = contract.operation() else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    Ok((*contract, t))
}
fn destination(c: &SemanticDirectCallV1) -> &SemanticPlaceV1 {
    c.destination().unwrap().place()
}
fn moved(op: &SemanticOperandV1, base: SemanticLocalIdV1) -> bool {
    matches!(op,SemanticOperandV1::Move(p) if p.local()==base && p.projections().is_empty())
}

// Representability only: an erased source operand stays a typed constant.
// The live source/SSA consumer must still prove the original producer link;
// this footer cannot turn the constant into an initialized tile or allocation.
fn tile_operand(
    types: &[SemanticTypeDeclV1],
    operand: &SemanticOperandV1,
    producer: &SemanticPlaceV1,
) -> bool {
    if operand.ty() != producer.ty() {
        return false;
    }
    match operand {
        SemanticOperandV1::Move(_) => moved(operand, producer.local()),
        SemanticOperandV1::Constant(constant) => {
            matches!(constant.value(), SemanticConstantValueV1::ZeroSized)
                && types.get(producer.ty().index() as usize).is_some_and(|ty| {
                    ty.layout().size_bytes() == Some(0)
                        && ty.layout().alignment_bytes() == 1
                        && !ty.layout().is_uninhabited()
                })
        }
        SemanticOperandV1::Copy(_) => false,
    }
}

fn workgroup_operand(
    operand: &SemanticOperandV1,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
) -> bool {
    matches!(operand, SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
        if place.local() == local && place.ty() == ty && place.projections().is_empty())
}

pub(super) fn rows(
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    rows: &[SemanticTransposeOwnedFlowV1],
    work: &mut u64,
    limits: SemanticMirLimitsV1,
) -> Result<()> {
    enforce_count(SemanticMirResourceV1::Blocks, rows.len(), limits)?;
    let mut total_borrows = 0usize;
    for (index, row) in rows.iter().enumerate() {
        spend(work, 1)?;
        if index != 0 {
            require(rows[index - 1].canonical_key() < row.canonical_key())?;
        }
        total_borrows = total_borrows
            .checked_add(row.workgroup_borrows.len())
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        enforce_count(SemanticMirResourceV1::CallArguments, total_borrows, limits)?;
        let s = row.sites;
        for (body, expected) in
            row.bodies
                .iter()
                .zip([s.issue.function, s.closure_call.function, s.stage.function])
        {
            require(body.function == expected)?;
            let f = functions
                .get(body.function.index() as usize)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            let (hash, bytes) = canonical_semantic_source_body_sha256_v25(f, *work / 2)?;
            spend(
                work,
                bytes
                    .checked_mul(2)
                    .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
            )?;
            require(hash == body.identity)?;
        }
        let issue = call(functions, s.issue)?;
        let stage = call(functions, s.stage)?;
        let publish = call(functions, s.publish)?;
        let matrix = call(functions, s.matrix_call)?;
        let invoke = call(functions, s.closure_call)?;
        let (ie, it) = transpose(callables, issue)?;
        let (se, st) = transpose(callables, stage)?;
        let (pe, pt) = transpose(callables, publish)?;
        let SemanticGfx950TransposeOperationV1::Issue {
            partition_reference,
            tile,
            ..
        } = it.operation()
        else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        let SemanticGfx950TransposeOperationV1::Stage {
            input_tile,
            output_tile,
            view_reference,
            index: integer,
            ..
        } = st.operation()
        else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        let SemanticGfx950TransposeOperationV1::Publish {
            input_tile: published_input,
            input_workgroup,
            transition,
            ..
        } = pt.operation()
        else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        require(
            ie.provenance() == se.provenance()
                && ie.provenance() == pe.provenance()
                && ie.workgroup_brand() == se.workgroup_brand()
                && ie.workgroup_brand() == pe.workgroup_brand()
                && ie.epoch_before() == se.epoch_before()
                && ie.epoch_before() == pe.epoch_before()
                && ie.epoch_after().is_none()
                && se.epoch_after().is_none()
                && pe.epoch_after().is_some()
                && it.format() == st.format()
                && it.format() == pt.format()
                && it.subgroup_brand() == st.subgroup_brand()
                && it.subgroup_brand() == pt.subgroup_brand()
                && tile == input_tile
                && output_tile == published_input,
        )?;
        require(
            issue.arguments().len() == 1
                && issue.arguments()[0].ty() == partition_reference
                && destination(issue).ty() == tile
                && stage.arguments().len() == 4
                && stage.arguments()[0].ty() == tile
                && stage.arguments()[1].ty() == view_reference
                && stage.arguments()[2].ty() == integer
                && stage.arguments()[3].ty() == integer
                && destination(stage).ty() == output_tile
                && publish.arguments().len() == 2
                && publish.arguments()[0].ty() == output_tile
                && publish.arguments()[1].ty() == input_workgroup
                && destination(publish).ty() == transition,
        )?;
        require(
            callables.get(matrix.callee().index() as usize)
                == Some(&SemanticCallableDeclV1::Defined {
                    function: s.closure_call.function,
                })
                && callables.get(invoke.callee().index() as usize)
                    == Some(&SemanticCallableDeclV1::Defined {
                        function: s.stage.function,
                    })
                && destination(matrix).ty() == output_tile
                && destination(invoke).ty() == output_tile,
        )?;
        let outer = &functions[s.issue.function.index() as usize];
        let capture = outer
            .blocks()
            .get(s.capture.block.index() as usize)
            .and_then(|b| b.statements().get(s.capture.statement as usize))
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        let SemanticStatementKindV1::Assign(capture) = capture.kind() else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        let SemanticRvalueKindV1::Aggregate(fields) = capture.value().kind() else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        require(
            capture.destination().projections().is_empty()
                && matches!(
                    fields.kind(),
                    SemanticAggregateKindV1::Aggregate | SemanticAggregateKindV1::Tuple
                )
                && fields
                    .operands()
                    .get(s.capture_field as usize)
                    .is_some_and(|op| tile_operand(types, op, destination(issue))),
        )?;
        require(
            matrix
                .arguments()
                .iter()
                .any(|op| moved(op, capture.destination().local())),
        )?;
        require(
            tile_operand(types, &publish.arguments()[0], destination(matrix))
                && workgroup_operand(&publish.arguments()[1], s.workgroup_local, input_workgroup)
                && outer
                    .locals()
                    .get(s.workgroup_local.index() as usize)
                    .is_some_and(|l| l.ty() == input_workgroup)
                && matrix.destination().unwrap().edge().target() == s.publish.block
                && outer.blocks()[s.publish.block.index() as usize]
                    .statements()
                    .is_empty()
                && outer.entry() != s.publish.block,
        )?;
        for (from, b) in outer.blocks().iter().enumerate() {
            spend(work, 1)?;
            b.terminator()
                .kind()
                .try_for_each_edge::<SemanticMirErrorV1>(|edge| {
                    spend(work, 1)?;
                    if edge.target() == s.publish.block {
                        require(
                            from == s.matrix_call.block.index() as usize
                                && edge.role() == SemanticEdgeRoleV1::CallReturn,
                        )?;
                    }
                    Ok(())
                })?;
        }
        for (i, borrow) in row.workgroup_borrows.iter().enumerate() {
            spend(work, 1)?;
            require(
                borrow.function == s.issue.function
                    && (i == 0 || row.workgroup_borrows[i - 1] < *borrow),
            )?;
            call(functions, *borrow)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "operand_tests.rs"]
mod operand_tests;

#[cfg(test)]
#[path = "limit_tests.rs"]
mod limit_tests;
