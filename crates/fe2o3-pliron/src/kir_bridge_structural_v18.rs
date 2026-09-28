//! Structural admission used only by the closed V18 optimizer.
//! Public bridge and historical optimizer profiles remain unchanged.

use super::*;
use dialect_gpu::storage_types_v18::{ExecutionRoleTypeV18, StorageObjectTypeV18};
use std::mem::size_of;

fn checked_add(a: usize, b: usize) -> Result<usize, KirBridgeErrorV18> {
    resources::add(a, b).map_err(Into::into)
}

fn headers() -> Result<usize, ResourceError> {
    [
        2 * size_of::<Census>(),
        2 * size_of::<StructuralBridgeWitnessV18>(),
        2 * size_of::<Vec<SignatureRow>>(),
        2 * size_of::<Result<(usize, resources::Envelope), KirBridgeErrorV18>>(),
        2 * size_of::<Result<StructuralBridgeWitnessV18, KirBridgeErrorV18>>(),
        2 * size_of::<
            Result<
                (
                    KirPlironGraphV18<'_>,
                    StructuralBridgeWitnessV18,
                    KirBridgeStorageV18,
                ),
                KirBridgeErrorV18,
            >,
        >(),
    ]
    .into_iter()
    .try_fold(0, resources::add)
}

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
    fn structural(self) -> Result<usize, KirBridgeErrorV18> {
        [
            self.slots,
            self.functions,
            self.signature_nodes,
            self.value_type_nodes,
            self.edges,
        ]
        .into_iter()
        .try_fold(self.tree, checked_add)
    }

    // The caller prepays canonical bytes before this allocation-free traversal.
    fn source(module: &Module) -> Result<Self, KirBridgeErrorV18> {
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
                    checked_add(census.signature_nodes, source_type_nodes(ty, 0)?)?;
            }
            let Some(body) = &function.body else { continue };
            census.definitions = checked_add(census.definitions, 1)?;
            add_tree_work(&mut census.tree, 3)?;
            census.slots = checked_add(census.slots, body.parameters.len())?;
            for block in &body.blocks {
                add_tree_work(&mut census.tree, 1)?;
                census.slots = checked_add(census.slots, block.parameters.len())?;
                for value in &block.parameters {
                    census.value_type_nodes =
                        checked_add(census.value_type_nodes, source_type_nodes(&value.ty, 0)?)?;
                }
                for operation in &block.operations {
                    add_tree_work(&mut census.tree, 2)?;
                    census.slots = checked_add(census.slots, operation.results.len())?;
                    for value in &operation.results {
                        census.value_type_nodes =
                            checked_add(census.value_type_nodes, source_type_nodes(&value.ty, 0)?)?;
                    }
                    operation.kind.try_visit_operands(|_| {
                        census.slots = checked_add(census.slots, 1)?;
                        Ok::<_, KirBridgeErrorV18>(())
                    })?;
                }
                add_tree_work(&mut census.tree, 2)?;
                let terminator = block
                    .terminator
                    .as_ref()
                    .ok_or(KirBridgeErrorV1::MalformedGraph)?;
                terminator.try_visit_operands(|_| {
                    census.slots = checked_add(census.slots, 1)?;
                    Ok::<_, KirBridgeErrorV18>(())
                })?;
                // This includes duplicate targets and cases with no arguments.
                terminator.try_visit_edges_v1(|_, _| {
                    census.edges = checked_add(census.edges, 1)?;
                    Ok::<_, KirBridgeErrorV18>(())
                })?;
            }
        }
        Ok(census)
    }
}

fn source_type_nodes(ty: &Type, depth: usize) -> Result<usize, KirBridgeErrorV18> {
    if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
        return Err(KirBridgeErrorV1::UnsupportedType.into());
    }
    match ty {
        // These nominal leaves carry fixed role/table identifiers. The existing
        // exact-owner V18 profile performs their independent admission.
        Type::Execution(_) | Type::StorageObject(_) => Ok(1),
        Type::Pointer(pointer) => checked_add(1, source_type_nodes(&pointer.pointee, depth + 1)?),
        Type::Slice(slice) => checked_add(1, source_type_nodes(&slice.element, depth + 1)?),
        // A fixed vector interns a scalar element as well as its outer type.
        Type::Vector(_) => Ok(2),
        Type::Unit | Type::Scalar(_) => Ok(1),
    }
}

fn live_type_nodes(
    context: &Context,
    ty: TypeHandle,
    depth: usize,
) -> Result<usize, KirBridgeErrorV18> {
    if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
        return Err(KirBridgeErrorV1::UnsupportedType.into());
    }
    let raw = ty.deref(context);
    if raw.is::<StorageObjectTypeV18>() || raw.is::<ExecutionRoleTypeV18>() {
        return Ok(1);
    }
    if let Some(pointer) = raw.downcast_ref::<PlironPointerType>() {
        return checked_add(1, live_type_nodes(context, pointer.pointee(), depth + 1)?);
    }
    if let Some(slice) = raw.downcast_ref::<PlironSliceType>() {
        return checked_add(1, live_type_nodes(context, slice.element(), depth + 1)?);
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
fn native_envelope(bytes: usize, census: Census) -> Result<resources::Envelope, KirBridgeErrorV18> {
    let volume = checked_add(census.structural()?, 1)?;
    let arithmetic = || ResourceError::Arithmetic;
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
    Ok(resources::Envelope { work, storage })
}

struct SignatureRow {
    function: Ptr<Operation>,
    signature: TypeHandle,
}

pub(crate) struct StructuralBridgeWitnessV18 {
    root: OperationHandle,
    source: Census,
    signatures: Vec<SignatureRow>,
}

impl StructuralBridgeWitnessV18 {
    fn retained_storage(&self) -> Result<usize, ResourceError> {
        resources::add(
            size_of::<Self>(),
            resources::mul(self.signatures.capacity(), size_of::<SignatureRow>())?,
        )
    }
    // Private constructor only. Reservation stays live with the witness.
    fn capture(
        graph: &KirPlironGraphV18<'_>,
        source: Census,
        budget: &mut Budget<'_>,
    ) -> Result<Self, KirBridgeErrorV18> {
        graph.validate_custody(budget)?;
        budget.charge_work(checked_add(source.tree, 1)?)?;
        let storage = source
            .definitions
            .checked_mul(std::mem::size_of::<SignatureRow>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or(ResourceError::Arithmetic)?;
        budget.reserve_storage(storage)?;
        let mut signatures = Vec::new();
        signatures
            .try_reserve_exact(source.definitions)
            .map_err(|_| ResourceError::Allocation)?;
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
        graph: &KirPlironGraphV18<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Census, KirBridgeErrorV18> {
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
            census.definitions = checked_add(census.definitions, 1)?;
            add_tree_work(&mut census.tree, 3)?;
            for block in function
                .deref(context)
                .get_region(0)
                .deref(context)
                .iter(context)
            {
                add_tree_work(&mut census.tree, 1)?;
                let raw_block = block.deref(context);
                census.slots = checked_add(census.slots, raw_block.get_num_arguments())?;
                precharge_argument_types(raw_block.get_num_arguments(), budget)?;
                for argument in raw_block.arguments() {
                    census.value_type_nodes = checked_add(
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
                    census.slots = checked_add(
                        census.slots,
                        checked_add(raw.get_num_operands(), raw.get_num_results())?,
                    )?;
                    census.edges = checked_add(census.edges, raw.get_num_successors())?;
                    precharge_types(raw.get_num_results(), budget)?;
                    for ty in raw.result_types() {
                        census.value_type_nodes =
                            checked_add(census.value_type_nodes, live_type_nodes(context, ty, 0)?)?;
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

fn precharge_types(count: usize, budget: &mut Budget<'_>) -> Result<(), KirBridgeErrorV18> {
    // 65 admitted recursive levels plus a vector's scalar element at the leaf.
    budget.charge_work(
        count
            .checked_mul(fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 + 2)
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    Ok(())
}

fn precharge_argument_types(
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<(), KirBridgeErrorV18> {
    // Pinned Pliron exposes arguments as Values, not their types. get_type()
    // finds each argument's position by a prefix scan, so pay the triangular
    // total before entering the iterator. Results use the direct type iterator.
    let indices = count
        .checked_add(1)
        .and_then(|next| count.checked_mul(next))
        .map(|twice| twice / 2)
        .ok_or(ResourceError::Arithmetic)?;
    let types = count
        .checked_mul(fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 + 2)
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(resources::add(indices, types)?)?;
    Ok(())
}

pub(super) fn import<'input>(
    input: &'input VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<
    (
        KirPlironGraphV18<'input>,
        StructuralBridgeWitnessV18,
        KirBridgeStorageV18,
    ),
    KirBridgeErrorV18,
> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut scope = resources::Scope::enter(budget)?;
    let result = (|| {
        scope.budget.reserve_storage(headers()?)?;
        let bytes = input.canonical_bytes().len();
        scope.budget.charge_work(bytes)?;
        let census = Census::source(input.module())?;
        let envelope = native_envelope(bytes, census)?;
        let (mut graph, storage) =
            import_inner(input, scope.budget, floor, ledger, Some(envelope))?;
        let witness = StructuralBridgeWitnessV18::capture(&graph, census, scope.budget)?;
        let retained = resources::add(storage.retained_storage(), witness.retained_storage()?)?;
        graph.retained_storage = retained;
        Ok((graph, witness, KirBridgeStorageV18 { retained }))
    })();
    let release = scope.finish();
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            release?;
            Ok(value)
        }
    }
}

pub(super) fn extraction_envelope(
    graph: &KirPlironGraphV18<'_>,
    witness: &StructuralBridgeWitnessV18,
    budget: &mut Budget<'_>,
) -> Result<(usize, resources::Envelope), KirBridgeErrorV18> {
    budget.reserve_storage(headers()?)?;
    let census = witness.live_census(graph, budget)?;
    Ok((
        census.tree,
        native_envelope(graph.profile.owner().canonical_bytes().len(), census)?,
    ))
}

#[cfg(test)]
#[path = "kir_bridge_structural_v18_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "kir_bridge_structural_v18_optimizer_tests.rs"]
mod optimizer_tests;
