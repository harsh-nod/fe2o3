use super::*;
use std::collections::HashSet;

impl KirPlironGraphV1 {
    pub const fn declared_version(&self) -> Option<CanonicalKernelIrVersionV1> {
        match self.canonical_version {
            KirBridgeCanonicalVersionV1::V13 => Some(CanonicalKernelIrVersionV1::V13),
            KirBridgeCanonicalVersionV1::V14 => Some(CanonicalKernelIrVersionV1::V14),
            KirBridgeCanonicalVersionV1::V9
            | KirBridgeCanonicalVersionV1::V10
            | KirBridgeCanonicalVersionV1::V11
            | KirBridgeCanonicalVersionV1::V12 => None,
        }
    }
}

fn bridge_version(version: CanonicalKernelIrVersionV1) -> KirBridgeCanonicalVersionV1 {
    match version {
        CanonicalKernelIrVersionV1::V13 => KirBridgeCanonicalVersionV1::V13,
        CanonicalKernelIrVersionV1::V14 => KirBridgeCanonicalVersionV1::V14,
    }
}

impl PlironSession {
    /// Imports into the existing live bridge. The declared version is never inferred or downgraded.
    pub fn import_canonical_kir_declared_o0(
        &mut self,
        input: &VerifiedCanonicalKernelIrV1,
    ) -> Result<KirPlironGraphV1, KirBridgeErrorV1> {
        input
            .revalidate()
            .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        let module = match input.version() {
            CanonicalKernelIrVersionV1::V13 => {
                fe2o3_kernel_ir::decode_module_v13(input.canonical_bytes())
            }
            CanonicalKernelIrVersionV1::V14 => {
                fe2o3_kernel_ir::decode_module_v14(input.canonical_bytes())
            }
        }
        .map_err(|_| KirBridgeErrorV1::CanonicalInputRejected)?;
        import_module(
            self,
            input.canonical_bytes(),
            bridge_version(input.version()),
            module,
        )
    }

    /// Exact O0 replay, including phase attributes and the original live occurrence roster.
    pub fn extract_canonical_kir_declared_o0(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV1, KirBridgeRoundTripReportV1), KirBridgeErrorV1> {
        let version = graph
            .declared_version()
            .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
        let output =
            VerifiedCanonicalKernelIrV1::from_module(extract_module(self, graph)?, version)
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), bridge_version(version))?;
        if output_digest != graph.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip);
        }
        let report = KirBridgeRoundTripReportV1 {
            input: graph.input,
            output: output_digest,
            correspondence: graph.correspondence.clone(),
        };
        Ok((output, report))
    }

    /// Structural extraction only. It does not approve a rewrite's memory or numerical semantics.
    pub fn extract_optimized_canonical_kir_declared_v1(
        &mut self,
        graph: &KirPlironGraphV1,
    ) -> Result<(VerifiedCanonicalKernelIrV1, KirBridgeOptimizedReceiptV1), KirBridgeErrorV1> {
        let version = graph
            .declared_version()
            .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
        let (module, correspondence) = extract_optimized_module(self, graph)?;
        let output = VerifiedCanonicalKernelIrV1::from_module(module, version)
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
        let output_digest = digest(output.canonical_bytes(), bridge_version(version))?;
        let report = KirBridgeOptimizedReceiptV1 {
            input: graph.input,
            output: output_digest,
            correspondence,
        };
        Ok((output, report))
    }
}

pub(super) fn reserve_phase_origins(
    count: usize,
) -> Result<HashMap<Ptr<Operation>, KirBridgeCoordinateV1>, KirBridgeErrorV1> {
    if count > HARD_MAX_OPERATION_TREE_ITEMS {
        return Err(OperationHandleError::OperationTreeLimitExceeded.into());
    }
    let mut origins = HashMap::new();
    origins
        .try_reserve(count)
        .map_err(|_| KirBridgeErrorV1::SizeOverflow)?;
    if origins.capacity() > HARD_MAX_OPERATION_TREE_ITEMS {
        return Err(OperationHandleError::OperationTreeLimitExceeded.into());
    }
    Ok(origins)
}

/// Uses existing original module/coordinates and live pointers, not a second authority graph.
/// A phase carrier can have rewired SSA operands, but its original payload and occurrence cannot change.
pub(super) fn validate_phase_custody(
    context: &Context,
    root: Ptr<Operation>,
    graph: &KirPlironGraphV1,
) -> Result<(), KirBridgeErrorV1> {
    if graph.canonical_version != KirBridgeCanonicalVersionV1::V14 {
        return Ok(());
    }
    if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let mut work = 0;
    add_tree_work(&mut work, graph.origins.phase_operations.capacity())?;
    let mut seen = HashSet::new();
    seen.try_reserve(graph.origins.phase_operations.len())
        .map_err(|_| KirBridgeErrorV1::SizeOverflow)?;
    add_tree_work(&mut work, seen.capacity())?;
    let region = root.deref(context).get_region(0);
    let region_ref = region.deref(context);
    let mut root_blocks = region_ref.iter(context);
    let root_block = root_blocks.next().ok_or(KirBridgeErrorV1::MalformedGraph)?;
    if root_blocks.next().is_some() {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    for function in root_block.deref(context).iter(context) {
        add_tree_work(&mut work, 1)?;
        if !Operation::is_op::<FuncOp>(function, context) {
            continue;
        }
        let function_index = graph
            .origins
            .functions
            .get(&function)
            .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
        if function.deref(context).num_regions() != 1 {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        let body = function.deref(context).get_region(0);
        for block in body.deref(context).iter(context) {
            add_tree_work(&mut work, 1)?;
            for live in block.deref(context).iter(context) {
                add_tree_work(&mut work, 1)?;
                let Some(phase) = Operation::get_op::<PlironReusablePhaseOp>(live, context) else {
                    continue;
                };
                let coordinate = graph
                    .origins
                    .phase_operations
                    .get(&live)
                    .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
                let KirBridgeCoordinateV1::Operation {
                    function,
                    block,
                    operation,
                } = *coordinate
                else {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                };
                if function as usize != *function_index || !seen.insert(live) {
                    return Err(KirBridgeErrorV1::GraphIdentityMismatch);
                }
                let original = graph
                    .metadata
                    .functions
                    .get(function as usize)
                    .and_then(|f| f.body.as_ref())
                    .and_then(|b| b.blocks.get(block as usize))
                    .and_then(|b| b.operations.get(operation as usize))
                    .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
                phase
                    .verify(context)
                    .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
                if phase.canonical_graph_epoch(context) != Some(graph.input.digest)
                    || phase.canonical_coordinate(context)
                        != Some(source_coordinate_attr(*coordinate)?)
                    || phase.canonical_contract(context).as_ref() != Some(original)
                {
                    return Err(KirBridgeErrorV1::GraphIdentityMismatch);
                }
            }
        }
    }
    if seen.len() != graph.origins.phase_operations.len() {
        return Err(KirBridgeErrorV1::GraphIdentityMismatch);
    }
    Ok(())
}
