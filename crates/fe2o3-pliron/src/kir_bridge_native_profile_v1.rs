//! Native-only admission for shared bridge algorithms; target accounting is frozen.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Census {
    tree: usize,
    slots: usize,
    functions: usize,
    definitions: usize,
    signature_nodes: usize,
    value_type_nodes: usize,
    edges: usize,
}

impl Census {
    fn structural(self) -> Result<usize, KirBridgeErrorV12> {
        [
            self.slots,
            self.functions,
            self.signature_nodes,
            self.value_type_nodes,
            self.edges,
        ]
        .into_iter()
        .try_fold(self.tree, checked_bridge_add_v12)
    }

    // The caller prepays canonical bytes before this allocation-free traversal.
    fn source(module: &Module) -> Result<Self, KirBridgeErrorV12> {
        let mut census = Self {
            tree: BUILTIN_MODULE_ROOT_TREE_WORK_V1,
            slots: 0,
            functions: module.functions.len(),
            definitions: 0,
            signature_nodes: 0,
            value_type_nodes: 0,
            edges: 0,
        };
        for function in &module.functions {
            for ty in function
                .signature
                .parameters
                .iter()
                .chain(&function.signature.results)
            {
                census.signature_nodes =
                    checked_bridge_add_v12(census.signature_nodes, source_type_nodes(ty, 0)?)?;
            }
            let Some(body) = &function.body else { continue };
            census.definitions = checked_bridge_add_v12(census.definitions, 1)?;
            add_tree_work(&mut census.tree, 3)?;
            census.slots = checked_bridge_add_v12(census.slots, body.parameters.len())?;
            for block in &body.blocks {
                add_tree_work(&mut census.tree, 1)?;
                census.slots = checked_bridge_add_v12(census.slots, block.parameters.len())?;
                for value in &block.parameters {
                    census.value_type_nodes = checked_bridge_add_v12(
                        census.value_type_nodes,
                        source_type_nodes(&value.ty, 0)?,
                    )?;
                }
                for operation in &block.operations {
                    add_tree_work(&mut census.tree, 2)?;
                    census.slots = checked_bridge_add_v12(census.slots, operation.results.len())?;
                    for value in &operation.results {
                        census.value_type_nodes = checked_bridge_add_v12(
                            census.value_type_nodes,
                            source_type_nodes(&value.ty, 0)?,
                        )?;
                    }
                    operation.kind.try_visit_operands(|_| {
                        census.slots = checked_bridge_add_v12(census.slots, 1)?;
                        Ok::<_, KirBridgeErrorV12>(())
                    })?;
                }
                add_tree_work(&mut census.tree, 2)?;
                let terminator = block
                    .terminator
                    .as_ref()
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?;
                terminator.try_visit_operands(|_| {
                    census.slots = checked_bridge_add_v12(census.slots, 1)?;
                    Ok::<_, KirBridgeErrorV12>(())
                })?;
                // This includes duplicate targets and cases with no arguments.
                terminator.try_visit_edges_v1(|_, _| {
                    census.edges = checked_bridge_add_v12(census.edges, 1)?;
                    Ok::<_, KirBridgeErrorV12>(())
                })?;
            }
        }
        Ok(census)
    }
}

fn source_type_nodes(ty: &Type, depth: usize) -> Result<usize, KirBridgeErrorV12> {
    if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
        return Err(KirBridgeErrorV1::UnsupportedType.into());
    }
    match ty {
        Type::Pointer(pointer) => {
            checked_bridge_add_v12(1, source_type_nodes(&pointer.pointee, depth + 1)?)
        }
        Type::Slice(slice) => {
            checked_bridge_add_v12(1, source_type_nodes(&slice.element, depth + 1)?)
        }
        // A fixed vector interns a scalar element as well as its outer type.
        Type::Vector(_) => Ok(2),
        Type::Unit | Type::Scalar(_) => Ok(1),
    }
}

fn live_type_nodes(
    context: &Context,
    ty: TypeHandle,
    depth: usize,
) -> Result<usize, KirBridgeErrorV12> {
    if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
        return Err(KirBridgeErrorV1::UnsupportedType.into());
    }
    let raw = ty.deref(context);
    if let Some(pointer) = raw.downcast_ref::<PlironPointerType>() {
        return checked_bridge_add_v12(1, live_type_nodes(context, pointer.pointee(), depth + 1)?);
    }
    if let Some(slice) = raw.downcast_ref::<PlironSliceType>() {
        return checked_bridge_add_v12(1, live_type_nodes(context, slice.element(), depth + 1)?);
    }
    if let Some(vector) = raw.downcast_ref::<PlironFixedVectorTypeV12>() {
        let element = vector.element().deref(context);
        if !ranked_data_type_node_is_supported_v2(&*element)
            || element.is::<PlironPointerType>()
            || element.is::<PlironSliceType>()
            || element.is::<PlironFixedVectorTypeV12>()
            || element.is::<UnitType>()
        {
            return Err(KirBridgeErrorV1::UnsupportedType.into());
        }
        return Ok(2);
    }
    if !ranked_data_type_node_is_supported_v2(&*raw) {
        return Err(KirBridgeErrorV1::UnsupportedType.into());
    }
    Ok(1)
}

// Retain every frozen cross/linear term. Only the byte-by-byte square is
// removed: bridge loops compare/copy bytes per structural record, not per byte.
fn native_envelope(
    bytes: usize,
    census: Census,
) -> Result<KirBridgeEnvelopeV12, KirBridgeErrorV12> {
    let volume = checked_bridge_add_v12(census.structural()?, 1)?;
    let arithmetic = || CanonicalKernelIrVerificationResourceErrorV1::Arithmetic;
    let work = volume
        .checked_mul(volume)
        .and_then(|n| n.checked_mul(4))
        .and_then(|square| {
            bytes
                .checked_mul(volume)
                .and_then(|n| n.checked_mul(8))
                .and_then(|cross| square.checked_add(cross))
        })
        .and_then(|n| {
            bytes
                .checked_add(volume)
                .and_then(|n| n.checked_mul(8))
                .and_then(|linear| n.checked_add(linear))
        })
        .ok_or_else(arithmetic)?;
    // Storage still admits byte payloads, graph slots and the opaque arena.
    // This is the unchanged formula, not a claim about allocator size classes.
    let storage = bytes
        .checked_add(census.tree)
        .and_then(|n| n.checked_add(census.slots))
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(64))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(arithmetic)?;
    Ok(KirBridgeEnvelopeV12 { work, storage })
}

struct SignatureRow {
    function: Ptr<Operation>,
    signature: TypeHandle,
}

pub(crate) struct NativeBridgeWitnessV1 {
    root: OperationHandle,
    source: Census,
    signatures: Vec<SignatureRow>,
}

impl NativeBridgeWitnessV1 {
    // Private constructor only. Reservation stays live with the witness.
    fn capture(
        graph: &KirPlironGraphV12<'_>,
        source: Census,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, KirBridgeErrorV12> {
        graph.validate_custody_v12()?;
        budget.charge_work(checked_bridge_add_v12(source.tree, 1)?)?;
        let storage = source
            .definitions
            .checked_mul(std::mem::size_of::<SignatureRow>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.reserve_storage(storage)?;
        let mut signatures = Vec::new();
        signatures
            .try_reserve_exact(source.definitions)
            .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
        let context = &graph.session.context;
        let root = graph.session.operations[&graph.root.identity];
        let region = root.deref(context).get_region(0);
        let raw_region = region.deref(context);
        let mut blocks = raw_region.iter(context);
        let block = blocks.next().ok_or(KirBridgeErrorV1::MalformedGraph)?;
        if blocks.next().is_some() {
            return Err(KirBridgeErrorV1::MalformedGraph.into());
        }
        for function in block.deref(context).iter(context) {
            if signatures.len() == source.definitions {
                return Err(KirBridgeErrorV1::MalformedGraph.into());
            }
            let function_op = Operation::get_op::<FuncOp>(function, context)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            signatures.push(SignatureRow {
                function,
                signature: function_op.get_type(context),
            });
        }
        if signatures.len() != source.definitions {
            return Err(KirBridgeErrorV1::MalformedGraph.into());
        }
        Ok(Self {
            root: graph.root.clone(),
            source,
            signatures,
        })
    }

    fn live_census(
        &self,
        graph: &KirPlironGraphV12<'_>,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Census, KirBridgeErrorV12> {
        if self.root.owner != graph.root.owner || self.root.identity != graph.root.identity {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        let context = &graph.session.context;
        let root = graph.session.operations[&graph.root.identity];
        if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
            return Err(KirBridgeErrorV1::MalformedGraph.into());
        }
        let region = root.deref(context).get_region(0);
        let raw_region = region.deref(context);
        let mut root_blocks = raw_region.iter(context);
        let root_block = root_blocks.next().ok_or(KirBridgeErrorV1::MalformedGraph)?;
        if root_blocks.next().is_some() {
            return Err(KirBridgeErrorV1::MalformedGraph.into());
        }
        let mut census = Census {
            tree: BUILTIN_MODULE_ROOT_TREE_WORK_V1,
            slots: 0,
            functions: self.source.functions,
            definitions: 0,
            signature_nodes: self.source.signature_nodes,
            value_type_nodes: 0,
            edges: 0,
        };
        for function in root_block.deref(context).iter(context) {
            let expected = self
                .signatures
                .get(census.definitions)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            let function_op = Operation::get_op::<FuncOp>(function, context)
                .ok_or(KirBridgeErrorV1::MalformedGraph)?;
            if expected.function != function
                || expected.signature != function_op.get_type(context)
                || function.deref(context).num_regions() != 1
            {
                return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
            }
            census.definitions = checked_bridge_add_v12(census.definitions, 1)?;
            add_tree_work(&mut census.tree, 3)?;
            for block in function
                .deref(context)
                .get_region(0)
                .deref(context)
                .iter(context)
            {
                add_tree_work(&mut census.tree, 1)?;
                let raw_block = block.deref(context);
                census.slots = checked_bridge_add_v12(census.slots, raw_block.get_num_arguments())?;
                precharge_types(raw_block.get_num_arguments(), budget)?;
                for argument in raw_block.arguments() {
                    census.value_type_nodes = checked_bridge_add_v12(
                        census.value_type_nodes,
                        live_type_nodes(context, argument.get_type(context), 0)?,
                    )?;
                }
                for operation in raw_block.iter(context) {
                    add_tree_work(&mut census.tree, 2)?;
                    let raw = operation.deref(context);
                    if raw.num_regions() != 0 {
                        return Err(KirBridgeErrorV1::MalformedGraph.into());
                    }
                    census.slots = checked_bridge_add_v12(
                        census.slots,
                        checked_bridge_add_v12(raw.get_num_operands(), raw.get_num_results())?,
                    )?;
                    census.edges = checked_bridge_add_v12(census.edges, raw.get_num_successors())?;
                    precharge_types(raw.get_num_results(), budget)?;
                    for result in raw.results() {
                        census.value_type_nodes = checked_bridge_add_v12(
                            census.value_type_nodes,
                            live_type_nodes(context, result.get_type(context), 0)?,
                        )?;
                    }
                }
            }
        }
        if census.definitions != self.signatures.len() {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        Ok(census)
    }
}

fn precharge_types(
    count: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), KirBridgeErrorV12> {
    // 65 admitted recursive levels plus a vector's scalar element at the leaf.
    budget.charge_work(
        count
            .checked_mul(fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 + 2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    Ok(())
}

pub(crate) fn import_native_neutral_v1<'input>(
    input: &'input VerifiedCanonicalKernelIrModuleV12,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(KirPlironGraphV12<'input>, NativeBridgeWitnessV1), KirBridgeErrorV12> {
    let bytes = input.canonical().canonical_bytes().len();
    budget.charge_work(bytes)?;
    let census = Census::source(input.module())?;
    let envelope = native_envelope(bytes, census)?;
    let (graph, storage) = import_admitted_connected_v12(input, census.tree, envelope, budget)?;
    // The private import helper retains its scratch reservation on success;
    // unlike the public import wrapper, this closed entry has no transfer gap.
    debug_assert_eq!(storage.retained_storage(), graph.retained_storage());
    let witness = NativeBridgeWitnessV1::capture(&graph, census, budget)?;
    Ok((graph, witness))
}

pub(super) fn extraction_envelope(
    graph: &KirPlironGraphV12<'_>,
    witness: &NativeBridgeWitnessV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(usize, KirBridgeEnvelopeV12), KirBridgeErrorV12> {
    let census = witness.live_census(graph, budget)?;
    Ok((
        census.tree,
        native_envelope(graph.source.canonical().canonical_bytes().len(), census)?,
    ))
}

#[cfg(test)]
mod tests {
    include!("kir_bridge_native_profile_v1_tests.rs");
}
