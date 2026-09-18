// Connected V12 bridge. Included privately so legacy endpoints retain their
// original type/rejection profile and the existing graph algorithms stay shared.

use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV12, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, VerifiedCanonicalKernelIrModuleV12,
};

pub const KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V12/V1\0";

#[path = "kir_bridge_native_profile_v1.rs"]
mod native_profile_v1;
pub(crate) use native_profile_v1::{NativeBridgeWitnessV1, import_native_neutral_v1};

#[derive(Clone, Copy)]
enum KirBridgeTypeProfileV12 {
    Legacy,
    V12,
}

impl KirBridgeTypeProfileV12 {
    fn preflight_type(self, ty: &Type) -> Result<(), KirBridgeErrorV1> {
        match self {
            Self::Legacy => preflight_type(ty),
            Self::V12 => match ty {
                Type::Execution(_) => Err(KirBridgeErrorV1::UnsupportedType),
                Type::Vector(vector) => vector
                    .validate()
                    .map_err(|_| KirBridgeErrorV1::UnsupportedType),
                Type::Pointer(pointer) => self.preflight_type(&pointer.pointee),
                Type::Slice(slice) => self.preflight_type(&slice.element),
                Type::Unit | Type::Scalar(_) => Ok(()),
            },
        }
    }

    fn preflight_operation(
        self,
        operation: &KirOperation,
        coordinate: KirBridgeCoordinateV1,
    ) -> Result<(), KirBridgeErrorV1> {
        if matches!(self, Self::V12) {
            match &operation.kind {
                OperationKind::VerificationContract(_) | OperationKind::VectorLayoutConvert(_) => {
                    return Ok(());
                }
                OperationKind::VectorLoad(load) => {
                    return load
                        .access
                        .vector
                        .validate()
                        .map_err(|_| KirBridgeErrorV1::UnsupportedType);
                }
                OperationKind::VectorStore(store) => {
                    return store
                        .access
                        .vector
                        .validate()
                        .map_err(|_| KirBridgeErrorV1::UnsupportedType);
                }
                _ => {}
            }
        }
        preflight_operation(operation, coordinate)
    }

    fn to_pliron(self, context: &Context, ty: &Type) -> Result<TypeHandle, KirBridgeErrorV1> {
        if matches!(self, Self::Legacy) {
            return type_to_pliron(context, ty);
        }
        Ok(match ty {
            Type::Execution(_) => return Err(KirBridgeErrorV1::UnsupportedType),
            Type::Vector(vector) => {
                vector
                    .validate()
                    .map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
                PlironFixedVectorTypeV12::try_get(
                    context,
                    type_to_pliron(context, &Type::Scalar(vector.element))?,
                    vector.lanes,
                    match vector.layout {
                        fe2o3_kernel_ir::VectorLayoutV12::Contiguous => {
                            dialect_gpu::vector_v12::VectorLayoutAttrV12::CONTIGUOUS
                        }
                        fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor } => {
                            dialect_gpu::vector_v12::VectorLayoutAttrV12::interleaved(factor)
                        }
                    },
                )
                .ok_or(KirBridgeErrorV1::UnsupportedType)?
                .into()
            }
            Type::Pointer(pointer) => PlironPointerType::get(
                context,
                self.to_pliron(context, &pointer.pointee)?,
                address_space_to_pliron(pointer.address_space)?,
                access_mode_to_pliron(pointer.access),
            )
            .into(),
            Type::Slice(slice) => PlironSliceType::get(
                context,
                self.to_pliron(context, &slice.element)?,
                address_space_to_pliron(slice.address_space)?,
                access_mode_to_pliron(slice.access),
            )
            .into(),
            Type::Unit | Type::Scalar(_) => type_to_pliron(context, ty)?,
        })
    }

    fn decode_type(self, context: &Context, ty: TypeHandle) -> Result<Type, KirBridgeErrorV1> {
        if matches!(self, Self::Legacy) {
            return type_from_pliron(context, ty);
        }
        self.decode_type_depth(context, ty, 0)
    }

    fn decode_type_depth(
        self,
        context: &Context,
        ty: TypeHandle,
        depth: usize,
    ) -> Result<Type, KirBridgeErrorV1> {
        if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
            return Err(KirBridgeErrorV1::UnsupportedType);
        }
        let raw = ty.deref(context);
        if let Some(vector) = raw.downcast_ref::<PlironFixedVectorTypeV12>() {
            let Type::Scalar(element) = type_from_pliron(context, vector.element())? else {
                return Err(KirBridgeErrorV1::UnsupportedType);
            };
            let descriptor = fe2o3_kernel_ir::FixedVectorTypeV12::new(
                element,
                vector.lanes(),
                match vector.layout().interleave_factor() {
                    None => fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
                    Some(factor) => fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor },
                },
            );
            descriptor
                .validate()
                .map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
            return Ok(Type::Vector(descriptor));
        }
        if let Some(pointer) = raw.downcast_ref::<PlironPointerType>() {
            return Ok(Type::pointer(
                self.decode_type_depth(context, pointer.pointee(), depth + 1)?,
                address_space_from_pliron(pointer.address_space()),
                access_mode_from_pliron(pointer.access()),
            ));
        }
        if let Some(slice) = raw.downcast_ref::<PlironSliceType>() {
            return Ok(Type::slice(
                self.decode_type_depth(context, slice.element(), depth + 1)?,
                address_space_from_pliron(slice.address_space()),
                access_mode_from_pliron(slice.access()),
            ));
        }
        type_from_pliron(context, ty)
    }
}

/// Transfer reservation for a V12 bridge owner or extracted output and report.
///
/// Bridge construction/materialization uses a closed conservative logical
/// envelope, not exact upstream allocator bytes or primitive execution steps.
/// Canonical output admission keeps its own full verification accounting.
/// Reserve this payload before subsequent allocation and release it only after
/// the corresponding owners have dropped. Session/interner retention survives
/// individual operation erasure and must be held until the session is dropped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirBridgeStorageV12 {
    retained: usize,
}

impl KirBridgeStorageV12 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

#[derive(Debug)]
pub enum KirBridgeErrorV12 {
    Bridge(KirBridgeErrorV1),
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    Canonical(CanonicalKernelIrReplayAdmissionErrorV12),
    SessionSetup,
}

impl fmt::Display for KirBridgeErrorV12 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
            Self::SessionSetup => formatter.write_str("V12 bridge session registration failed"),
        }
    }
}
impl Error for KirBridgeErrorV12 {}
impl From<KirBridgeErrorV1> for KirBridgeErrorV12 {
    fn from(error: KirBridgeErrorV1) -> Self {
        Self::Bridge(error)
    }
}
impl From<OperationHandleError> for KirBridgeErrorV12 {
    fn from(error: OperationHandleError) -> Self {
        Self::Bridge(error.into())
    }
}
impl From<CanonicalKernelIrVerificationResourceErrorV1> for KirBridgeErrorV12 {
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

/// Move-only V12 graph and its private session, borrowing immutable input custody.
/// Only closed bridge/optimizer methods may mutate the session. Metadata is
/// borrowed from the exact input; operation bodies are extracted from live SSA.
///
/// ```compile_fail
/// use fe2o3_pliron::KirPlironGraphV12;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<KirPlironGraphV12<'static>>();
/// ```
pub struct KirPlironGraphV12<'input> {
    pub(crate) session: PlironSession,
    pub(crate) root: OperationHandle,
    pub(crate) retained_storage: usize,
    pub(crate) optimization_started: bool,
    optimization_capture: Option<crate::kir_optimization_map_v12::CaptureV12>,
    source: &'input VerifiedCanonicalKernelIrModuleV12,
    input: KirBridgeDigestV1,
    correspondence: Vec<KirBridgeCorrespondenceV1>,
    origins: KirBridgeOriginsV1,
}

impl fmt::Debug for KirPlironGraphV12<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KirPlironGraphV12")
            .field("input", &self.input)
            .field("retained_storage", &self.retained_storage)
            .finish_non_exhaustive()
    }
}

impl<'input> KirPlironGraphV12<'input> {
    pub fn import(
        input: &'input VerifiedCanonicalKernelIrModuleV12,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, KirBridgeStorageV12), KirBridgeErrorV12> {
        let floor = budget.storage();
        let result = import_connected_v12(input, budget);
        restore_bridge_floor_v12(budget, floor)?;
        result
    }

    pub const fn root(&self) -> &OperationHandle {
        &self.root
    }
    pub const fn input(&self) -> KirBridgeDigestV1 {
        self.input
    }
    pub fn correspondence(&self) -> &[KirBridgeCorrespondenceV1] {
        &self.correspondence
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
    }

    // No graph walk or allocation. The source cannot change while borrowed.
    pub(crate) fn validate_custody_v12(&self) -> Result<(), KirBridgeErrorV12> {
        self.session.validate_identity()?;
        if self.root.owner != self.session.identity
            || !self.session.operations.contains_key(&self.root.identity)
            || !self
                .session
                .owned_tree_work
                .contains_key(&self.root.identity)
        {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        Ok(())
    }

    pub fn extract_canonical_kir_module_v12_o0(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeRoundTripReportV1,
            KirBridgeStorageV12,
        ),
        KirBridgeErrorV12,
    > {
        let floor = budget.storage();
        let result = self
            .extract_inner_v12(budget, true)
            .map(|(owner, report, storage)| {
                (
                    owner,
                    KirBridgeRoundTripReportV1 {
                        input: report.input,
                        output: report.output,
                        correspondence: report.correspondence,
                    },
                    storage,
                )
            });
        restore_bridge_floor_v12(budget, floor)?;
        result
    }

    pub fn extract_optimized_canonical_kir_module_v12(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            KirBridgeStorageV12,
        ),
        KirBridgeErrorV12,
    > {
        let floor = budget.storage();
        let result = self.extract_inner_v12(budget, false);
        restore_bridge_floor_v12(budget, floor)?;
        result
    }

    fn extract_inner_v12(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        exact: bool,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            KirBridgeStorageV12,
        ),
        KirBridgeErrorV12,
    > {
        self.extract_admitted_inner_v1(budget, exact, None)
    }

    fn extract_admitted_inner_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        exact: bool,
        native: Option<&NativeBridgeWitnessV1>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            KirBridgeStorageV12,
        ),
        KirBridgeErrorV12,
    > {
        self.validate_custody_v12()?;
        let charged_tree = self.session.owned_tree_work[&self.root.identity];
        let source_bytes = self.source.canonical().canonical_bytes().len();
        budget.charge_work(checked_bridge_add_v12(source_bytes, charged_tree)?)?;
        let root = self.session.operations[&self.root.identity];
        let (tree, envelope) = match native {
            Some(witness) => native_profile_v1::extraction_envelope(self, witness, budget)?,
            None => {
                let (tree, operands_and_values) =
                    census_live_graph_v12(&self.session.context, root)?;
                if tree != charged_tree {
                    return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
                }
                (
                    tree,
                    bridge_envelope_v12(
                        source_bytes,
                        checked_bridge_add_v12(tree, operands_and_values)?,
                    )?,
                )
            }
        };
        if tree != charged_tree {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        budget.charge_work(envelope.work)?;
        budget.reserve_storage(envelope.storage)?;
        let extracted = catch_unwind(AssertUnwindSafe(|| {
            if exact {
                validate_exact_graph_origins_v12(
                    &self.session.context,
                    root,
                    self.source.module(),
                    &self.origins,
                )?;
            }
            let (module, correspondence) = extract_optimized_module_graph(
                &self.session.context,
                root,
                self.source.module(),
                &self.origins,
                KirBridgeTypeProfileV12::V12,
            )?;
            if exact && correspondence != self.correspondence {
                return Err(KirBridgeErrorV1::NonExactRoundTrip);
            }
            Ok((module, correspondence))
        }));
        let (module, correspondence) = match extracted {
            Ok(result) => result?,
            Err(_) => {
                self.session.poisoned = true;
                return Err(KirBridgeErrorV1::UpstreamPanicked.into());
            }
        };
        let (owner, canonical_storage) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &module, budget,
            )
            .map_err(KirBridgeErrorV12::Canonical)?;
        budget.reserve_storage(canonical_storage.retained_storage())?;
        let output = digest_v12_with_budget(owner.canonical().canonical_bytes(), budget)?;
        if exact && output != self.input {
            return Err(KirBridgeErrorV1::NonExactRoundTrip.into());
        }
        let report_storage = correspondence
            .capacity()
            .checked_mul(std::mem::size_of::<KirBridgeCorrespondenceV1>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<KirBridgeOptimizedReceiptV1>()))
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.reserve_storage(report_storage)?;
        let retained =
            checked_bridge_add_v12(canonical_storage.retained_storage(), report_storage)?;
        drop(module);
        budget.release_storage(envelope.storage)?;
        Ok((
            owner,
            KirBridgeOptimizedReceiptV1 {
                input: self.input,
                output,
                correspondence,
            },
            KirBridgeStorageV12 { retained },
        ))
    }
}

struct KirBridgeEnvelopeV12 {
    work: usize,
    storage: usize,
}

// Versioned admission units for opaque upstream construction/materialization:
// source payload and graph slots form one volume; quadratic scheduling admits
// nested scans, and 64 payload units per volume plus 4096 session units admit
// coexisting graph/origin tables and temporary collections. These deliberately
// conservative logical units are not claims about Rust allocation sizes or
// exact Pliron primitive work. Growth from closed optimizer passes is separate.
fn bridge_envelope_v12(
    bytes: usize,
    graph_slots: usize,
) -> Result<KirBridgeEnvelopeV12, KirBridgeErrorV12> {
    let volume = checked_bridge_add_v12(checked_bridge_add_v12(bytes, graph_slots)?, 1)?;
    let work = volume
        .checked_mul(volume)
        .and_then(|value| value.checked_mul(4))
        .and_then(|value| {
            volume
                .checked_mul(8)
                .and_then(|linear| value.checked_add(linear))
        })
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let storage = volume
        .checked_mul(64)
        .and_then(|value| value.checked_add(4096))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    Ok(KirBridgeEnvelopeV12 { work, storage })
}

fn checked_bridge_add_v12(a: usize, b: usize) -> Result<usize, KirBridgeErrorV12> {
    a.checked_add(b)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic.into())
}

fn restore_bridge_floor_v12(
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    floor: usize,
) -> Result<(), KirBridgeErrorV12> {
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
    budget.release_storage(release)?;
    Ok(())
}

fn digest_v12_with_budget(
    bytes: &[u8],
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<KirBridgeDigestV1, KirBridgeErrorV12> {
    budget.charge_work(checked_bridge_add_v12(
        bytes.len(),
        12 + KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1.len(),
    )?)?;
    Ok(digest(bytes, KirBridgeCanonicalVersionV1::V12)?)
}

fn source_tree_work_v12(module: &Module) -> Result<usize, KirBridgeErrorV1> {
    let mut tree = BUILTIN_MODULE_ROOT_TREE_WORK_V1;
    for function in &module.functions {
        if let Some(body) = &function.body {
            add_tree_work(&mut tree, 3)?;
            for block in &body.blocks {
                add_tree_work(&mut tree, 1)?;
                add_tree_work(
                    &mut tree,
                    block
                        .operations
                        .len()
                        .checked_mul(2)
                        .and_then(|work| work.checked_add(2))
                        .ok_or(KirBridgeErrorV1::SizeOverflow)?,
                )?;
            }
        }
    }
    Ok(tree)
}

fn import_connected_v12<'input>(
    input: &'input VerifiedCanonicalKernelIrModuleV12,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(KirPlironGraphV12<'input>, KirBridgeStorageV12), KirBridgeErrorV12> {
    let bytes = input.canonical().canonical_bytes();
    budget.charge_work(bytes.len())?;
    let tree = source_tree_work_v12(input.module())?;
    let envelope = bridge_envelope_v12(bytes.len(), tree)?;
    import_admitted_connected_v12(input, tree, envelope, budget)
}

fn import_admitted_connected_v12<'input>(
    input: &'input VerifiedCanonicalKernelIrModuleV12,
    tree: usize,
    envelope: KirBridgeEnvelopeV12,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(KirPlironGraphV12<'input>, KirBridgeStorageV12), KirBridgeErrorV12> {
    let bytes = input.canonical().canonical_bytes();
    budget.charge_work(envelope.work)?;
    budget.reserve_storage(envelope.storage)?;
    let input_digest = digest_v12_with_budget(bytes, budget)?;
    // All budget-deniable import work precedes session allocation. A failure or
    // unwind inside this closure drops the entire arena before the outer rollback.
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (actual_tree, correspondence) =
            preflight_with_profile_v12(input.module(), KirBridgeTypeProfileV12::V12)?;
        if actual_tree != tree {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        let registration =
            dialect_gpu::dialect_registration().map_err(|_| KirBridgeErrorV12::SessionSetup)?;
        let limits =
            crate::ShellLimits::new(32, 64, 512).map_err(|_| KirBridgeErrorV12::SessionSetup)?;
        let mut session = PlironSession::new(limits, [registration])
            .map_err(|_| KirBridgeErrorV12::SessionSetup)?;
        session.require_internal_tree_capacity(tree)?;
        let root = session.create_module("kir_bridge_v12")?;
        let transaction = session.begin_checked_operation_graph_mutation_v1(&root)?;
        let pointer = session.operations[&root.identity];
        let origins = build_module_graph(
            &mut session.context,
            pointer,
            input.module(),
            KirBridgeTypeProfileV12::V12,
        )?;
        session.finish_internal_root_construction(&root, transaction)?;
        let graph = KirPlironGraphV12 {
            session,
            root,
            retained_storage: envelope.storage,
            optimization_started: false,
            optimization_capture: None,
            source: input,
            input: input_digest,
            correspondence,
            origins,
        };
        #[cfg(test)]
        if connected_v12_tests::FAIL_IMPORT_AFTER_BUILD.with(|fail| fail.replace(false)) {
            panic!("injected V12 import failure after session mutation");
        }
        Ok((
            graph,
            KirBridgeStorageV12 {
                retained: envelope.storage,
            },
        ))
    }));
    match result {
        Ok(result) => result,
        Err(_) => Err(KirBridgeErrorV1::UpstreamPanicked.into()),
    }
}

fn census_live_graph_v12(
    context: &Context,
    root: Ptr<Operation>,
) -> Result<(usize, usize), KirBridgeErrorV1> {
    if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let region = root.deref(context).get_region(0);
    let region_owner = region.deref(context);
    let mut root_blocks = region_owner.iter(context);
    let root_block = root_blocks.next().ok_or(KirBridgeErrorV1::MalformedGraph)?;
    if root_blocks.next().is_some() {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let mut tree = BUILTIN_MODULE_ROOT_TREE_WORK_V1;
    let mut slots = 0_usize;
    for function in root_block.deref(context).iter(context) {
        if !Operation::is_op::<FuncOp>(function, context)
            || function.deref(context).num_regions() != 1
        {
            return Err(KirBridgeErrorV1::MalformedGraph);
        }
        add_tree_work(&mut tree, 3)?;
        for block in function
            .deref(context)
            .get_region(0)
            .deref(context)
            .iter(context)
        {
            add_tree_work(&mut tree, 1)?;
            slots = slots
                .checked_add(block.deref(context).get_num_arguments())
                .ok_or(KirBridgeErrorV1::SizeOverflow)?;
            for operation in block.deref(context).iter(context) {
                add_tree_work(&mut tree, 2)?;
                let raw = operation.deref(context);
                if raw.num_regions() != 0 {
                    return Err(KirBridgeErrorV1::MalformedGraph);
                }
                slots = slots
                    .checked_add(raw.get_num_operands())
                    .and_then(|value| value.checked_add(raw.get_num_results()))
                    .ok_or(KirBridgeErrorV1::SizeOverflow)?;
            }
        }
    }
    Ok((tree, slots))
}

// Definition signatures and bodies are rebuilt from live SSA. Only declarations
// need their full signature copied from the immutable source metadata.
fn module_metadata_v12(source: &Module) -> Module {
    Module {
        id: source.id.clone(),
        kernels: source.kernels.clone(),
        required_capabilities: source.required_capabilities.clone(),
        functions: source
            .functions
            .iter()
            .map(|function| fe2o3_kernel_ir::Function {
                id: function.id.clone(),
                role: function.role,
                required_capabilities: function.required_capabilities.clone(),
                signature: if function.body.is_some() {
                    fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new())
                } else {
                    function.signature.clone()
                },
                body: None,
            })
            .collect(),
    }
}

// Exact extraction admits no lost/reassigned source value origin or changed
// block/operation roster. The subsequent live extraction and canonical digest
// check establish exact types, operation semantics, operands and terminators.
fn validate_exact_graph_origins_v12(
    context: &Context,
    root: Ptr<Operation>,
    source: &Module,
    origins: &KirBridgeOriginsV1,
) -> Result<(), KirBridgeErrorV1> {
    let reject = || KirBridgeErrorV1::NonExactRoundTrip;
    if !Operation::is_op::<ModuleOp>(root, context) || root.deref(context).num_regions() != 1 {
        return Err(reject());
    }
    let region = root.deref(context).get_region(0);
    let region_owner = region.deref(context);
    let mut root_blocks = region_owner.iter(context);
    let root_block = root_blocks.next().ok_or_else(reject)?;
    if root_blocks.next().is_some() {
        return Err(reject());
    }
    let root_block_owner = root_block.deref(context);
    let mut live_functions = root_block_owner.iter(context);
    for (function_index, function) in source.functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        let live = live_functions.next().ok_or_else(reject)?;
        if origins.functions.get(&live) != Some(&function_index)
            || !Operation::is_op::<FuncOp>(live, context)
            || live.deref(context).num_regions() != 1
        {
            return Err(reject());
        }
        let region = live.deref(context).get_region(0);
        let region_owner = region.deref(context);
        let mut live_blocks = region_owner.iter(context);
        for (block_index, source_block) in body.blocks.iter().enumerate() {
            let live_block = live_blocks.next().ok_or_else(reject)?;
            if origins.blocks.get(&live_block) != Some(&(function_index, source_block.id)) {
                return Err(reject());
            }
            let raw_block = live_block.deref(context);
            let parameters = if block_index == 0 {
                body.parameters.as_slice()
            } else {
                &[]
            };
            if raw_block.get_num_arguments() != parameters.len() + source_block.parameters.len() {
                return Err(reject());
            }
            for (index, expected) in parameters
                .iter()
                .copied()
                .chain(source_block.parameters.iter().map(|value| value.id))
                .enumerate()
            {
                if origins.values.get(&raw_block.get_argument(index)) != Some(&expected) {
                    return Err(reject());
                }
            }
            let mut live_operations = raw_block.iter(context);
            for operation in &source_block.operations {
                let live_operation = live_operations.next().ok_or_else(reject)?;
                let raw_operation = live_operation.deref(context);
                if raw_operation.get_num_results() != operation.results.len() {
                    return Err(reject());
                }
                for (index, expected) in operation.results.iter().enumerate() {
                    if origins.values.get(&raw_operation.get_result(index)) != Some(&expected.id) {
                        return Err(reject());
                    }
                }
                if Operation::is_op::<PreservedOperationOp>(live_operation, context)
                    && origins.preserved_operations.get(&live_operation) != Some(&operation.kind)
                {
                    return Err(reject());
                }
            }
            let terminator = live_operations.next().ok_or_else(reject)?;
            if live_operations.next().is_some() {
                return Err(reject());
            }
            if Operation::is_op::<PreservedTerminatorOp>(terminator, context)
                && origins.preserved_terminators.get(&terminator)
                    != source_block.terminator.as_ref()
            {
                return Err(reject());
            }
        }
        if live_blocks.next().is_some() {
            return Err(reject());
        }
    }
    if live_functions.next().is_some() {
        return Err(reject());
    }
    Ok(())
}

#[cfg(test)]
#[path = "kir_bridge_v12_tests.rs"]
mod connected_v12_tests;

#[derive(Debug)]
pub enum KirMappedExtractionErrorV12 {
    Bridge(KirBridgeErrorV12),
    Mapping(crate::KirOptimizationMapErrorV12),
}
impl fmt::Display for KirMappedExtractionErrorV12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(e) => e.fmt(f),
            Self::Mapping(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for KirMappedExtractionErrorV12 {}

impl KirPlironGraphV12<'_> {
    pub(crate) fn begin_optimization_capture_v12(
        &mut self,
    ) -> Result<crate::kir_optimization_map_v12::CaptureV12, crate::KirOptimizationMapErrorV12>
    {
        use crate::kir_optimization_map_v12::CaptureLimitsV12;
        let limits = CaptureLimitsV12::for_bytes(self.source.canonical().canonical_bytes().len())?;
        self.begin_optimization_capture_with_limits_v1(limits)
    }

    pub(crate) fn begin_optimization_capture_with_limits_v1(
        &mut self,
        limits: crate::kir_optimization_map_v12::CaptureLimitsV12,
    ) -> Result<crate::kir_optimization_map_v12::CaptureV12, crate::KirOptimizationMapErrorV12>
    {
        self.begin_optimization_capture_for_policy_v1(
            limits,
            crate::fixed_policy_v3::FixedPolicy::Historical2,
        )
    }

    pub(crate) fn begin_optimization_capture_for_policy_v1(
        &mut self,
        limits: crate::kir_optimization_map_v12::CaptureLimitsV12,
        policy: crate::fixed_policy_v3::FixedPolicy,
    ) -> Result<crate::kir_optimization_map_v12::CaptureV12, crate::KirOptimizationMapErrorV12>
    {
        use crate::kir_optimization_map_v12::CaptureV12;
        let captured = catch_unwind(AssertUnwindSafe(|| {
            let roster = self.optimization_roster_v12(limits.node_limit())?;
            match policy {
                crate::fixed_policy_v3::FixedPolicy::Historical2 => {
                    CaptureV12::new(limits, &roster)
                }
                crate::fixed_policy_v3::FixedPolicy::Checked3
                | crate::fixed_policy_v3::FixedPolicy::Integer6 => {
                    CaptureV12::new_for_policy(limits, &roster, policy)
                }
            }
        }));
        let capture = match captured {
            Ok(result) => result?,
            Err(_) => {
                self.session.poisoned = true;
                return Err(crate::KirOptimizationMapErrorV12::UnsupportedMutation);
            }
        };
        self.optimization_capture = Some(capture.clone());
        Ok(capture)
    }

    /// Extracts exact output custody and the checked source-to-output relation
    /// from this same candidate. A successful result transfers the map payload
    /// with the output/bridge receipt; no caller-selected source graph is used.
    pub fn extract_optimized_canonical_kir_module_with_map_v12(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            crate::KirOptimizationMapV12,
            KirBridgeStorageV12,
        ),
        KirMappedExtractionErrorV12,
    > {
        self.extract_admitted_canonical_with_map_v1(budget, None)
    }

    pub(crate) fn extract_admitted_canonical_with_map_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        native: Option<&NativeBridgeWitnessV1>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            crate::KirOptimizationMapV12,
            KirBridgeStorageV12,
        ),
        KirMappedExtractionErrorV12,
    > {
        self.extract_with_map_finalizer_v1(
            budget,
            native,
            crate::fixed_policy_v3::FixedPolicy::Historical2,
            crate::kir_optimization_map_v12::CaptureV12::finish,
        )
    }

    pub(crate) fn extract_admitted_canonical_with_policy3_map_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        native: &NativeBridgeWitnessV1,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            crate::KirOptimizationMapPolicy3V12,
            KirBridgeStorageV12,
        ),
        KirMappedExtractionErrorV12,
    > {
        self.extract_with_map_finalizer_v1(
            budget,
            Some(native),
            crate::fixed_policy_v3::FixedPolicy::Checked3,
            crate::kir_optimization_map_v12::CaptureV12::finish_policy3,
        )
    }

    pub(crate) fn extract_admitted_canonical_with_integer_continuation_map_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        native: &NativeBridgeWitnessV1,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            crate::KirOptimizationMapIntegerContinuationV12,
            KirBridgeStorageV12,
        ),
        KirMappedExtractionErrorV12,
    > {
        self.extract_with_map_finalizer_v1(
            budget,
            Some(native),
            crate::fixed_policy_v3::FixedPolicy::Integer6,
            crate::kir_optimization_map_v12::CaptureV12::finish_integer_continuation,
        )
    }

    fn extract_with_map_finalizer_v1<M>(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        native: Option<&NativeBridgeWitnessV1>,
        policy: crate::fixed_policy_v3::FixedPolicy,
        finish: impl FnOnce(
            &crate::kir_optimization_map_v12::CaptureV12,
            &VerifiedCanonicalKernelIrModuleV12,
            &VerifiedCanonicalKernelIrModuleV12,
            &crate::kir_optimization_map_v12::LiveRosterV12,
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<(M, usize), crate::KirOptimizationMapErrorV12>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            M,
            KirBridgeStorageV12,
        ),
        KirMappedExtractionErrorV12,
    > {
        use crate::KirOptimizationMapErrorV12;
        use crate::kir_optimization_map_v12::CaptureLimitsV12;
        let floor = budget.storage();
        let result = (|| {
            let capture = self
                .optimization_capture
                .as_ref()
                .ok_or(KirMappedExtractionErrorV12::Mapping(
                    KirOptimizationMapErrorV12::Passes,
                ))?
                .clone();
            capture
                .require_policy(policy)
                .map_err(KirMappedExtractionErrorV12::Mapping)?;
            let inner_floor = budget.storage();
            let materialized = self.extract_admitted_inner_v1(budget, false, native);
            restore_bridge_floor_v12(budget, inner_floor)
                .map_err(KirMappedExtractionErrorV12::Bridge)?;
            let (owner, bridge, extracted) =
                materialized.map_err(KirMappedExtractionErrorV12::Bridge)?;
            let map_error = |e| KirMappedExtractionErrorV12::Mapping(e);
            budget
                .reserve_storage(extracted.retained_storage())
                .map_err(|e| map_error(e.into()))?;
            let limits = CaptureLimitsV12::for_policy_bytes(
                self.source.canonical().canonical_bytes().len(),
                policy,
            )
            .map_err(map_error)?;
            if owner.module().functions.len() != self.source.module().functions.len() {
                return Err(map_error(KirOptimizationMapErrorV12::Coverage));
            }
            let roster_limit = optimization_roster_endpoint_count_v12(owner.module(), budget)
                .map_err(map_error)?;
            if roster_limit > limits.node_limit() {
                return Err(map_error(KirOptimizationMapErrorV12::Limit));
            }
            // The live roster coexists with capture, canonical output, and the
            // map/checker scratch. Preserve the existing storage envelope, but
            // meter the actual roster rather than its byte-derived node cap.
            let scratch = limits.storage().map_err(map_error)?;
            budget
                .reserve_storage(scratch)
                .map_err(|e| map_error(e.into()))?;
            let observed = catch_unwind(AssertUnwindSafe(|| {
                self.optimization_roster_metered_v1::<true, _>(roster_limit, &mut |units| {
                    budget.charge_work(units).map_err(Into::into)
                })
            }));
            let roster = match observed {
                Ok(result) => result.map_err(map_error)?,
                Err(_) => {
                    self.session.poisoned = true;
                    return Err(map_error(KirOptimizationMapErrorV12::UnsupportedMutation));
                }
            };
            let (map, map_storage) =
                finish(&capture, self.source, &owner, &roster, budget).map_err(map_error)?;
            budget
                .reserve_storage(map_storage)
                .map_err(|e| map_error(e.into()))?;
            let retained = extracted
                .retained_storage()
                .checked_add(map_storage)
                .ok_or_else(|| map_error(KirOptimizationMapErrorV12::Arithmetic))?;
            drop(roster);
            budget
                .release_storage(scratch)
                .map_err(|e| map_error(e.into()))?;
            Ok((owner, bridge, map, KirBridgeStorageV12 { retained }))
        })();
        // All closure-local owners have dropped on error before floor restore.
        // Success transfers only the explicit retained receipt.
        restore_bridge_floor_v12(budget, floor).map_err(KirMappedExtractionErrorV12::Bridge)?;
        result
    }

    fn optimization_roster_v12(
        &self,
        limit: usize,
    ) -> Result<crate::kir_optimization_map_v12::LiveRosterV12, crate::KirOptimizationMapErrorV12>
    {
        self.optimization_roster_metered_v1::<false, _>(limit, &mut |_| Ok(()))
    }

    // Legacy calls instantiate the no-op meter above. Neutral execution meters
    // its additional physical roster traversal without changing target reports.
    fn optimization_roster_metered_v1<const METER: bool, F>(
        &self,
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
                self.source
                    .module()
                    .functions
                    .len()
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(4))
                    .ok_or(E::Arithmetic)?,
            )?;
        }
        let context = &self.session.context;
        let root = self.session.operations[&self.root.identity];
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
        let live_functions = index_live_functions(
            raw_root_block.iter(context),
            self.source.module(),
            &self.origins,
        )
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
        for (function_index, source) in self.source.module().functions.iter().enumerate() {
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
}

include!("kir_bridge_v12_capture_v1.rs");
