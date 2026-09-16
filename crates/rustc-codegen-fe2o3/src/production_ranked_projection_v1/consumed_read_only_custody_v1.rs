// The consuming terminal ends source-allocation custody on every successor
// path, including backedges. This does not infer authority from MIR Copy/Move.
fn consumed_read_only_successor_custody_v1(
    function: &SemanticFunctionDeclV1,
    roots: &[Option<ConsumedReadOnlyRootV1>],
    work: &mut usize,
) -> Result<Vec<Option<Vec<bool>>>, ProductionRankedProjectionErrorV1> {
    charge_capability_dataflow_work_v1(work, roots.len())?;
    let mut result = vec![None; roots.len()];
    for root in roots.iter().flatten() {
        charge_capability_dataflow_work_v1(work, function.blocks().len())?;
        let mut reached = vec![false; function.blocks().len()];
        let mut pending = VecDeque::new();
        let mut source = root.conversion_block;
        loop {
            charge_capability_dataflow_work_v1(work, 1)?;
            let block = function.blocks().get(source).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "readonly custody source is outside the original CFG",
                ),
            )?;
            block
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionRankedProjectionErrorV1>(|edge| {
                    charge_capability_dataflow_work_v1(work, 1)?;
                    let target = edge.target().index() as usize;
                    let visited = reached.get_mut(target).ok_or(
                        ProductionRankedProjectionErrorV1::Incomplete(
                            "readonly custody successor is outside the original CFG",
                        ),
                    )?;
                    if !*visited {
                        *visited = true;
                        pending.push_back(target);
                    }
                    Ok(())
                })?;
            let Some(next) = pending.pop_front() else {
                break;
            };
            source = next;
        }
        result[root.argument as usize] = Some(reached);
    }
    Ok(result)
}
