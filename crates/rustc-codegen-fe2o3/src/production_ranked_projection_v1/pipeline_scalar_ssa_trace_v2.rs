// Temporary bounded numeric observation only. No source/SSA/budget mutation.
const TRACE_EDGE_SCAN_V2: usize = 4096;
const TRACE_ARGUMENT_SCAN_V2: usize = 256;
const TRACE_ROWS_V2: usize = 64;

fn trace_block_argument_v2(
    index: &Index<'_>,
    local: usize,
    use_site: ScalarAssignmentSiteV1,
    value: SsaValueV1,
) {
    if std::env::var_os("FE2O3_TRACE_RANKED_CUSTODY_V1").is_none() {
        return;
    }
    let _ = write_block_argument_v2(
        true,
        &mut std::io::stderr().lock(),
        index,
        local,
        use_site,
        value,
    );
}

pub(super) fn write_block_argument_v2(
    enabled: bool,
    writer: &mut impl std::io::Write,
    index: &Index<'_>,
    local: usize,
    use_site: ScalarAssignmentSiteV1,
    value: SsaValueV1,
) -> std::io::Result<()> {
    if !enabled {
        return Ok(());
    }
    let SsaValueV1::BlockArgument { block, variable } = value else {
        return Ok(());
    };
    let Some(retained) = index.source.owner.plan_for_function(index.source.function) else {
        return writeln!(writer, "PIPELINE_SSA_PHI_V2 missing_plan=1");
    };
    let plan = retained.plan();
    let parameters = plan.transport_variables(block).unwrap_or(&[]);
    let position = parameters
        .iter()
        .take(TRACE_ARGUMENT_SCAN_V2)
        .position(|id| *id == variable);
    writeln!(
        writer,
        "PIPELINE_SSA_PHI_V2 function={} local={local} use_block={} use_statement={} argument_block={} variable={} position={} parameters={} parameter_scan_truncated={}",
        index.source.function.index(),
        use_site.block,
        use_site.statement,
        block.get(),
        variable.get(),
        position.unwrap_or(usize::MAX),
        parameters.len(),
        usize::from(parameters.len() > TRACE_ARGUMENT_SCAN_V2)
    )?;
    let edges = index.occurrences.successors();
    let mut rows = 0;
    let mut scanned = 0;
    let mut rows_truncated = false;
    for edge in edges.iter().take(TRACE_EDGE_SCAN_V2) {
        scanned += 1;
        if edge.edge().target().index() != block.get() {
            continue;
        }
        if rows == TRACE_ROWS_V2 {
            rows_truncated = true;
            break;
        }
        rows += 1;
        let arguments = plan.edge_arguments(edge.id());
        let incoming = arguments.and_then(|args| {
            args.iter()
                .take(TRACE_ARGUMENT_SCAN_V2)
                .find(|argument| argument.variable() == variable)
                .map(|argument| argument.value())
        });
        let (kind, first, second) = match incoming {
            Some(SsaValueV1::Definition(id)) => (1, id.get(), 0),
            Some(SsaValueV1::BlockArgument { block, variable }) => (2, block.get(), variable.get()),
            None => (0, 0, 0),
        };
        writeln!(
            writer,
            "PIPELINE_SSA_PHI_EDGE_V2 source={} ordinal={} target={} reachable={} arguments={} argument_scan_truncated={} value_kind={kind} value_first={first} value_second={second}",
            edge.id().source().get(),
            edge.id().ordinal(),
            edge.edge().target().index(),
            usize::from(plan.is_reachable(edge.id().source())),
            arguments.map_or(0, |args| args.len()),
            usize::from(arguments.is_some_and(|args| args.len() > TRACE_ARGUMENT_SCAN_V2))
        )?;
        if let Some(SsaValueV1::Definition(id)) = incoming {
            match index.definitions.get(id.get() as usize).copied().flatten() {
                Some(Origin::Argument { local, argument }) => writeln!(
                    writer,
                    "PIPELINE_SSA_PHI_DEFINITION_V2 id={} origin=1 local={local} argument={argument}",
                    id.get()
                )?,
                Some(Origin::Assignment { local, site }) => {
                    let assignment = index
                        .function
                        .blocks()
                        .get(site.block)
                        .and_then(|block| block.statements().get(site.statement))
                        .and_then(|statement| match statement.kind() {
                            SemanticStatementKindV1::Assign(assignment) => Some(assignment),
                            _ => None,
                        });
                    let operand =
                        assignment.and_then(|assignment| match assignment.value().kind() {
                            SemanticRvalueKindV1::Use(operand) => Some(operand),
                            _ => None,
                        });
                    let copy_local = operand.and_then(simple_operand_local).map(|id| id.index());
                    let literal = operand.and_then(|operand| match operand {
                        SemanticOperandV1::Constant(constant) => match constant.value() {
                            SemanticConstantValueV1::Scalar(value) => Some(value.bits()),
                            _ => None,
                        },
                        _ => None,
                    });
                    writeln!(
                        writer,
                        "PIPELINE_SSA_PHI_DEFINITION_V2 id={} origin=2 local={local} producer_block={} producer_statement={} copy_local={} literal_present={} literal_bits={}",
                        id.get(),
                        site.block,
                        site.statement,
                        copy_local.unwrap_or(u32::MAX),
                        usize::from(literal.is_some()),
                        literal.unwrap_or(0)
                    )?;
                }
                None => writeln!(
                    writer,
                    "PIPELINE_SSA_PHI_DEFINITION_V2 id={} origin=0",
                    id.get()
                )?,
            }
        }
    }
    writeln!(
        writer,
        "PIPELINE_SSA_PHI_END_V2 scanned={scanned} edges={} rows={rows} truncated={}",
        edges.len(),
        usize::from(rows_truncated || scanned < edges.len())
    )
}

pub(super) fn trace_induction_association_v2(
    inductions: &[ProjectedUniformInductionV1],
    local: usize,
    use_site: ScalarAssignmentSiteV1,
) {
    if std::env::var_os("FE2O3_TRACE_RANKED_CUSTODY_V1").is_none() {
        return;
    }
    let _ =
        write_induction_association_v2(&mut std::io::stderr().lock(), inductions, local, use_site);
}

fn write_induction_association_v2(
    writer: &mut impl std::io::Write,
    inductions: &[ProjectedUniformInductionV1],
    local: usize,
    use_site: ScalarAssignmentSiteV1,
) -> std::io::Result<()> {
    for (ordinal, induction) in inductions.iter().take(TRACE_ROWS_V2).enumerate() {
        writeln!(
            writer,
            "PIPELINE_SSA_PHI_INDUCTION_V2 ordinal={ordinal} local={local} use_block={} use_statement={} induction_local={} header={} body_entry={} latch={} exit={} contains_use={} local_matches={}",
            use_site.block,
            use_site.statement,
            induction.source_progress.induction.index(),
            induction.header,
            induction.body_entry,
            induction.latch,
            induction.exit,
            usize::from(
                induction.contains_block(use_site.block) || induction.header == use_site.block
            ),
            usize::from(induction.source_progress.induction.index() as usize == local)
        )?;
    }
    writeln!(
        writer,
        "PIPELINE_SSA_PHI_INDUCTIONS_END_V2 total={} shown={} truncated={}",
        inductions.len(),
        inductions.len().min(TRACE_ROWS_V2),
        usize::from(inductions.len() > TRACE_ROWS_V2)
    )
}
