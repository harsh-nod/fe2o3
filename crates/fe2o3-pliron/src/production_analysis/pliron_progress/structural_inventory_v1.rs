#[derive(Default)]
struct StructuralInventoryV1 {
    regions: usize,
    blocks: usize,
    operations: usize,
    operands: usize,
    results: usize,
    attributes: usize,
    block_arguments: usize,
    edges: usize,
    root_blocks: Vec<Ptr<BasicBlock>>,
    root_operation_blocks: HashMap<Ptr<Operation>, usize>,
}

impl StructuralInventoryV1 {
    fn structural_work(&self) -> Option<usize> {
        self.regions
            .checked_add(self.blocks)
            .and_then(|n| n.checked_add(self.operations))
            .and_then(|n| n.checked_add(self.operands))
            .and_then(|n| n.checked_add(self.results))
            .and_then(|n| n.checked_add(self.attributes))
            .and_then(|n| n.checked_add(self.block_arguments))
            .and_then(|n| n.checked_add(self.edges))
    }

    #[cfg(test)]
    fn verification_work(&self) -> usize {
        self.structural_work()
            .and_then(|work| {
                self.operands
                    .checked_mul(self.operations)
                    .and_then(|dominance| work.checked_add(dominance))
            })
            .unwrap_or(usize::MAX)
    }
}

fn bounded_structural_inventory(
    context: &Context,
    function: &FuncOp,
) -> Result<StructuralInventoryV1, PlironProgressFindingV1> {
    let root = function.get_operation();
    let root_region = {
        let root_ref = root.try_deref(context).map_err(|error| {
            structural_rejection(format!(
                "the function root cannot be borrowed from the supplied context: {}",
                bounded_display_detail(error.disp(context))
            ))
        })?;
        let mut regions = root_ref.regions();
        let Some(root_region) = regions.next() else {
            return Err(structural_rejection(
                "the function root has no body region".to_owned(),
            ));
        };
        root_region
    };
    let mut inventory = StructuralInventoryV1::default();
    let mut pending = vec![(root, 0_usize)];
    while let Some((operation, depth)) = pending.pop() {
        if depth > MAX_PLIRON_PROGRESS_NESTING_DEPTH_V1 {
            return Err(resource_limit(
                "operation nesting depth",
                depth,
                MAX_PLIRON_PROGRESS_NESTING_DEPTH_V1,
            ));
        }
        let operation_ref = operation.try_deref(context).map_err(|error| {
            structural_rejection(format!(
                "an operation cannot be borrowed during structural inventory: {}",
                bounded_display_detail(error.disp(context))
            ))
        })?;
        add_count(
            &mut inventory.operands,
            operation_ref.get_num_operands(),
            MAX_PLIRON_PROGRESS_OPERANDS_V1,
            "operands",
        )?;
        add_count(
            &mut inventory.results,
            operation_ref.get_num_results(),
            MAX_PLIRON_PROGRESS_RESULTS_V1,
            "results",
        )?;
        add_count(
            &mut inventory.attributes,
            operation_ref.attributes.0.len(),
            MAX_PLIRON_PROGRESS_ATTRIBUTES_V1,
            "attributes",
        )?;
        add_count(
            &mut inventory.edges,
            operation_ref.get_num_successors(),
            MAX_PLIRON_PROGRESS_EDGES_V1,
            "CFG edges",
        )?;
        for region in operation_ref.regions() {
            add_count(
                &mut inventory.regions,
                1,
                MAX_PLIRON_PROGRESS_REGIONS_V1,
                "regions",
            )?;
            let is_root_region = region == root_region;
            let region_ref = region.try_deref(context).map_err(|error| {
                structural_rejection(format!(
                    "a region cannot be borrowed during structural inventory: {}",
                    bounded_display_detail(error.disp(context))
                ))
            })?;
            for block in region_ref.iter(context) {
                add_count(
                    &mut inventory.blocks,
                    1,
                    MAX_PLIRON_PROGRESS_BLOCKS_V1,
                    "basic blocks",
                )?;
                let block_ref = block.try_deref(context).map_err(|error| {
                    structural_rejection(format!(
                        "a block cannot be borrowed during structural inventory: {}",
                        bounded_display_detail(error.disp(context))
                    ))
                })?;
                add_count(
                    &mut inventory.block_arguments,
                    block_ref.get_num_arguments(),
                    MAX_PLIRON_PROGRESS_BLOCK_ARGUMENTS_V1,
                    "block arguments",
                )?;
                add_count(
                    &mut inventory.attributes,
                    block_ref.attributes.0.len(),
                    MAX_PLIRON_PROGRESS_ATTRIBUTES_V1,
                    "attributes",
                )?;
                let root_block_index = if is_root_region {
                    let index = inventory.root_blocks.len();
                    inventory.root_blocks.push(block);
                    Some(index)
                } else {
                    None
                };
                for child in block_ref.iter(context) {
                    add_count(
                        &mut inventory.operations,
                        1,
                        MAX_PLIRON_PROGRESS_OPERATIONS_V1,
                        "operations",
                    )?;
                    if let Some(index) = root_block_index {
                        inventory.root_operation_blocks.insert(child, index);
                    }
                    pending.push((child, depth.saturating_add(1)));
                }
            }
        }
    }
    Ok(inventory)
}

fn add_count(
    count: &mut usize,
    amount: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), PlironProgressFindingV1> {
    let actual = count.checked_add(amount).unwrap_or(usize::MAX);
    if actual > limit {
        return Err(resource_limit(resource, actual, limit));
    }
    *count = actual;
    Ok(())
}

fn resource_limit(resource: &'static str, actual: usize, limit: usize) -> PlironProgressFindingV1 {
    PlironProgressFindingV1::ResourceLimitExceeded {
        resource,
        actual,
        limit,
    }
}

fn structural_rejection(reason: String) -> PlironProgressFindingV1 {
    PlironProgressFindingV1::StructuralPrerequisiteRejected {
        reason: bounded_detail(reason),
    }
}

fn bounded_detail(mut detail: String) -> String {
    if detail.len() <= MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1 {
        return detail;
    }
    let mut end = MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1 - 3;
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail.truncate(end);
    detail.push_str("...");
    detail
}

struct BoundedProgressDetailWriterV1 {
    detail: String,
    truncated: bool,
}

impl fmt::Write for BoundedProgressDetailWriterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        let payload_limit = MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1 - 3;
        let remaining = payload_limit.saturating_sub(self.detail.len());
        if value.len() <= remaining {
            self.detail.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.detail.push_str(&value[..end]);
        self.truncated = true;
        Ok(())
    }
}

fn bounded_display_detail(display: impl fmt::Display) -> String {
    let mut writer = BoundedProgressDetailWriterV1 {
        detail: String::with_capacity(MAX_PLIRON_PROGRESS_DIAGNOSTIC_BYTES_V1),
        truncated: false,
    };
    let _ = writer.write_fmt(format_args!("{display}"));
    if writer.truncated {
        writer.detail.push_str("...");
    }
    writer.detail
}

fn panic_detail(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        bounded_detail((*message).to_owned())
    } else if let Some(message) = payload.downcast_ref::<String>() {
        bounded_detail(message.clone())
    } else {
        "an untyped panic escaped a Rust IR access boundary".to_owned()
    }
}
