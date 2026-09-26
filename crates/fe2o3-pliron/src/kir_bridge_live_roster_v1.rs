fn optimization_roster_metered_v1<const METER: bool, F>(
    context: &Context,
    root: Ptr<Operation>,
    source: &Module,
    origins: &KirBridgeOriginsV1,
    limit: usize,
    meter: &mut F,
) -> Result<crate::kir_optimization_map_v12::LiveRosterV12, crate::KirOptimizationMapErrorV12>
where
    F: FnMut(usize) -> Result<(), crate::KirOptimizationMapErrorV12> + ?Sized,
{
    use crate::kir_optimization_map_v12::{LiveKeyV12 as Key, LiveRosterV12};
    use crate::{KirOptimizationEndpointV12 as Endpoint, KirOptimizationMapErrorV12 as E};
    if METER {
        meter(
            source
                .functions
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(4))
                .ok_or(E::Arithmetic)?,
        )?;
    }
    if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
        return Err(E::Coverage);
    }
    let region = root.deref(context).get_region(0);
    let raw_region = region.deref(context);
    let mut root_blocks = raw_region.iter(context);
    let root_block = root_blocks.next().ok_or(E::Coverage)?;
    if root_blocks.next().is_some() {
        return Err(E::Coverage);
    }
    let raw_root_block = root_block.deref(context);
    let live_functions = index_live_functions(raw_root_block.iter(context), source, origins)
        .map_err(|_| E::Coverage)?;
    let mut roster = LiveRosterV12::new();
    roster.try_reserve_exact(limit).map_err(|_| E::Allocation)?;
    let mut push = |key, endpoint| {
        if roster.len() == limit {
            return Err(E::Limit);
        }
        roster.push((key, endpoint));
        Ok(())
    };
    let index = |n: usize| u32::try_from(n).map_err(|_| E::Arithmetic);
    for (function_index, source) in source.functions.iter().enumerate() {
        if METER {
            meter(1)?;
        }
        let Some(body) = &source.body else { continue };
        let function = index(function_index)?;
        let live_function = live_functions[function_index].ok_or(E::Coverage)?;
        if !Operation::is_op::<FuncOp>(live_function, context)
            || live_function.deref(context).num_regions() != 1
        {
            return Err(E::Coverage);
        }
        let region = live_function.deref(context).get_region(0);
        let raw_region = region.deref(context);
        for (block_index, live_block) in raw_region.iter(context).enumerate() {
            if METER {
                meter(1)?;
            }
            let block = index(block_index)?;
            let raw_block = live_block.deref(context);
            let offset = if block_index == 0 {
                body.parameters.len()
            } else {
                0
            };
            if raw_block.get_num_arguments() < offset {
                return Err(E::Coverage);
            }
            for (argument, value) in raw_block.arguments().enumerate() {
                if METER {
                    meter(1)?;
                }
                let endpoint = if argument < offset {
                    Endpoint::FunctionArgument {
                        function,
                        argument: index(argument)?,
                    }
                } else {
                    Endpoint::BlockArgument {
                        function,
                        block,
                        argument: index(argument - offset)?,
                    }
                };
                push(Key::Value(value), endpoint)?;
            }
            let mut operations = raw_block.iter(context).peekable();
            let mut operation_index = 0;
            while let Some(op) = operations.next() {
                if METER {
                    meter(1)?;
                }
                let coordinate = if operations.peek().is_some() {
                    KirBridgeCoordinateV1::Operation {
                        function,
                        block,
                        operation: index(operation_index)?,
                    }
                } else {
                    KirBridgeCoordinateV1::Terminator { function, block }
                };
                if op.deref(context).num_regions() != 0 {
                    return Err(E::UnsupportedMutation);
                }
                push(Key::Operation(op), Endpoint::Operation(coordinate))?;
                for (result, value) in op.deref(context).results().enumerate() {
                    if METER {
                        meter(1)?;
                    }
                    push(
                        Key::Value(value),
                        Endpoint::Result {
                            operation: coordinate,
                            result: index(result)?,
                        },
                    )?;
                }
                operation_index += 1;
            }
            if operation_index == 0 {
                return Err(E::Coverage);
            }
        }
    }
    Ok(roster)
}
