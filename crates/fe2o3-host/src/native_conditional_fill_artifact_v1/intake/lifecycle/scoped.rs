//! Lexical native authority over actual publication, currentness and proof owners.

use super::*;
use crate::generated_runtime_arguments::prepare_charged_with_plan;
use crate::generated_runtime_carrier::{GeneratedRuntimeAuthorityV1, GeneratedRuntimeCarrierV1};
use crate::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedRuntimeArguments,
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1,
};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, NativeConditionalFill64PremisesV1};
use fe2o3_runtime::{
    Gfx942RuntimeInvocationBindingV1 as Invocation, RuntimeGfx942GeneratedCompletionCarrierV1,
    WorkerV3Gfx942ExecutionAuthorityV1,
};
use std::cell::{RefCell, RefMut};
mod arena1024;
mod cohort3;
mod epoch;
mod registry4;
pub(super) use epoch::EpochAnchor;
use epoch::{CarrierRetention, Epoch};

struct Current<'scope, 'work> {
    proof: &'scope mut Proof<'work>,
    budget: &'scope mut Budget<'work>,
}

/// Borrowed same-thread native application root, available only inside the
/// original proved owner's lexical callback. No bare evidence or decoded receipt
/// constructs it. Generated carriers borrow it until exact native quiescence.
///
/// ```compile_fail
/// use fe2o3_host::NativeConditionalFillInvocationScopeV1 as Scope;
/// fn escape<'a, 'w>(scope: &Scope<'a, 'w>) -> &'static Scope<'static, 'w> { scope }
/// ```
pub struct NativeConditionalFillInvocationScopeV1<'scope, 'work> {
    epoch: Epoch<'scope, 'work>,
    files: &'scope Files<'work>,
    current: RefCell<Current<'scope, 'work>>,
    retained: usize,
    deadline: Instant,
}

impl<'work> ProvedNativeConditionalFillApplicationV1<'work> {
    /// Keeps the exact publication/proof owners and original account borrowed.
    /// Compose generated preparations inside RuntimeContext's lexical generated
    /// scope; its tickets and carriers cannot escape this callback. No operation
    /// is submitted merely by entering this host scope.
    ///
    /// Only the fixed temporary header charge is released, after successful
    /// closing revalidation. A failed attempt never resets work or storage floors.
    pub fn with_native_invocation_scope<R>(
        &mut self,
        budget: &mut Budget<'work>,
        deadline: Instant,
        callback: impl for<'scope> FnOnce(
            &NativeConditionalFillInvocationScopeV1<'scope, 'work>,
        ) -> Result<R>,
    ) -> Result<R> {
        let mut root = self.open_invocation_scope(budget, deadline)?;
        let result = callback(&root)?;
        root.revalidate()?;
        root.epoch.close()?;
        drop(root);
        budget.release_storage(size_of::<NativeConditionalFillInvocationScopeV1<'_, 'work>>())?;
        Ok(result)
    }

    /// Lends the actual proof/publication owner across awaits on the caller's
    /// executor, without nested `block_on`, owner threads, or lifetime erasure.
    /// Use the runtime's async generated scope inside this callback and settle
    /// every original carrier before returning. Its original account must stay
    /// inside the same consuming async account callback or active sync callback.
    ///
    /// The hidden host/account epochs detect forgotten nested futures or owners.
    /// Dropping this future with retained carriers fails stop before proof/data
    /// teardown. Dropping an empty open scope poisons that proved owner; it never
    /// resets the original resource ledger. Only a successful close releases the
    /// exact temporary frame reservation. This remains a same-thread API.
    ///
    /// Admission and proof/currentness revalidation remain synchronous bounded
    /// operations. Async composition does not claim nonblocking control I/O or
    /// genuine native hardware qualification.
    ///
    /// ```no_run
    /// use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Proved;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// async fn compose<'w>(app: &mut Proved<'w>, budget: &mut Budget<'w>) {
    ///     app.with_native_invocation_scope_async_v1(
    ///         budget, std::time::Instant::now() + std::time::Duration::from_secs(10),
    ///         async |_root| { std::future::ready(()).await; Ok(()) },
    ///     ).await.unwrap();
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Proved;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// async fn escape<'w>(app: &mut Proved<'w>, budget: &mut Budget<'w>) {
    ///     let root = app.with_native_invocation_scope_async_v1(
    ///         budget, std::time::Instant::now(), async |root| Ok(root)).await;
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Proved;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn requires_send<T: Send>(_: T) {}
    /// fn transfer<'w>(app: &mut Proved<'w>, budget: &mut Budget<'w>) {
    ///     requires_send(app.with_native_invocation_scope_async_v1(
    ///         budget, std::time::Instant::now(), async |_| Ok(())));
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Proved;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn reuse<'w>(app: &mut Proved<'w>, budget: &mut Budget<'w>) {
    ///     let future = app.with_native_invocation_scope_async_v1(
    ///         budget, std::time::Instant::now(), async |_| Ok(()));
    ///     budget.charge_work(1).unwrap();
    ///     drop(future);
    /// }
    /// ```
    pub async fn with_native_invocation_scope_async_v1<R>(
        &mut self,
        budget: &mut Budget<'work>,
        deadline: Instant,
        callback: impl for<'scope> AsyncFnOnce(
            &NativeConditionalFillInvocationScopeV1<'scope, 'work>,
        ) -> Result<R>,
    ) -> Result<R> {
        let mut root = self.open_invocation_scope(budget, deadline)?;
        let result = callback(&root).await?;
        root.revalidate()?;
        root.epoch.close()?;
        drop(root);
        budget.release_storage(size_of::<NativeConditionalFillInvocationScopeV1<'_, 'work>>())?;
        Ok(result)
    }

    fn open_invocation_scope<'scope>(
        &'scope mut self,
        budget: &'scope mut Budget<'work>,
        deadline: Instant,
    ) -> Result<NativeConditionalFillInvocationScopeV1<'scope, 'work>> {
        require(
            self.files.ledger == budget.work_ledger_identity_v1()
                && self.files.account == budget.storage_account_identity_v1()
                && budget.storage() >= self.retained,
            "native invocation original account differs",
        )?;
        self.revalidate(deadline, budget)?;
        // Loan acquisition prepays its inline header separately; count it only
        // once in the complete scope frame and retain both charges until close.
        let temporary = size_of::<NativeConditionalFillInvocationScopeV1<'_, 'work>>()
            .checked_sub(Epoch::LOAN_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(64)?;
        budget.reserve_storage(temporary)?;
        let epoch = self.invocation_epoch.begin(&self.files.failed, budget)?;
        Ok(NativeConditionalFillInvocationScopeV1 {
            epoch,
            files: &self.files,
            current: RefCell::new(Current {
                proof: &mut self.proof,
                budget,
            }),
            retained: self
                .retained
                .checked_add(temporary)
                .and_then(|n| n.checked_add(Epoch::LOAN_STORAGE))
                .ok_or(Resource::Arithmetic)?,
            deadline,
        })
    }
}

impl<'scope, 'work> NativeConditionalFillInvocationScopeV1<'scope, 'work> {
    fn current(&self) -> Result<RefMut<'_, Current<'scope, 'work>>> {
        borrow_original(&self.current, &self.files.failed)
    }

    fn revalidate(&self) -> Result<()> {
        let result = (|| {
            let mut current = self.current()?;
            let Current { proof, budget } = &mut *current;
            require(
                budget.work_ledger_identity_v1() == self.files.ledger
                    && budget.storage_account_identity_v1() == self.files.account
                    && budget.storage() >= self.retained,
                "native invocation account changed",
            )?;
            self.files.revalidate(budget)?;
            proof.probe(self.deadline, budget).map_err(failure)?;
            check_content(self.files, proof, budget)?;
            self.files.revalidate(budget)?;
            proof.probe(self.deadline, budget).map_err(failure)
        })();
        if result.is_err() {
            self.files.failed.set(true);
        }
        result
    }

    /// Creates an opaque carrier only from actual native source, protected proof,
    /// currentness/publication custody, generated Rust ABI and exact charged
    /// invocation storage. The checked device is borrowed from Context preparation.
    /// No native effect occurs here; runtime repeats all native admission gates.
    ///
    /// ```compile_fail
    /// use fe2o3_host::*;
    /// fn escape<'w, K, A>(
    ///     app: &mut ProvedNativeConditionalFillApplicationV1<'w>,
    ///     budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'w>,
    ///     args: A, device: &fe2o3_kfd::CheckedGfx942XnackMinusDevice,
    ///     limits: GeneratedRuntimeArgumentLimitsV1, results: &GeneratedRuntimeResultBudgetV1,
    /// ) where K: CompilerGeneratedKernelExpectationV1, A: CompilerGeneratedRuntimeArguments<K> {
    ///     let _carrier = app.with_native_invocation_scope(budget, std::time::Instant::now(), |root| {
    ///         root.prepare_generated_invocation::<K, A>(args, device,
    ///             fe2o3_aql::AqlDispatchGeometryV1::new([64,1,1],[64,1,1]).unwrap(),
    ///             100, limits, results)
    ///     });
    /// }
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_invocation<'a, K, A>(
        &'a self,
        arguments: A,
        device: &CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        impl RuntimeGfx942GeneratedCompletionCarrierV1<CurrentnessError = Error>
        + 'a
        + use<'a, 'scope, 'work, K, A>,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        self.prepare_carrier::<K, A>(
            arguments,
            device,
            geometry,
            timeout_milliseconds,
            limits,
            result_budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_carrier<'a, K, A>(
        &'a self,
        arguments: A,
        device: &CheckedGfx942XnackMinusDevice,
        geometry: AqlDispatchGeometryV1,
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<GeneratedRuntimeCarrierV1<Authority<'a, 'scope, 'work>>>
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        self.revalidate()?;
        let prepared: Result<_> = (|| {
            let mut current = self.current()?;
            let budget = &mut *current.budget;
            let (
                storage,
                footprint,
                object,
                dispatch_contract,
                invocation,
                plan_storage,
                abi_storage,
            ) = {
                let owner = self.files.finalized.source().recovered_handoff();
                let (abi, abi_storage) =
                    check_native_conditional_fill_abi_v1(owner, budget).map_err(failure)?;
                budget.reserve_storage(abi_storage)?;
                require(
                    K::KERNEL_BINDING_ID_V1 == *abi.kernel_id()
                        && K::EXPORT_NAME == abi.entry_name(),
                    "native generated marker differs from actual V5 source",
                )?;
                let generated = A::generated_argument_layout().map_err(failure)?;
                let (plan, plan_storage) = abi
                    .prepare_argument_packing(&generated, budget)
                    .map_err(failure)?;
                budget.reserve_storage(plan_storage)?;
                let contract = abi.native_contract_identity(budget).map_err(failure)?;
                let packed = prepare_charged_with_plan(
                    arguments,
                    &plan,
                    limits,
                    result_budget,
                    A::account_runtime_arguments,
                    |arguments, budget| arguments.bind_runtime_arguments(&plan, budget),
                )
                .map_err(failure)?;
                let count = packed_count(&packed.packed_view_v1(), *abi.kernel_id())?;
                let premises =
                    NativeConditionalFill64PremisesV1::new(contract, count, abi.max_grid_x())
                        .map_err(|_| failure("native packed output or source grid bound"))?;
                premises
                    .validate_shape_v1(
                        geometry.grid(),
                        geometry.workgroup(),
                        count.checked_mul(4).ok_or(Resource::Arithmetic)?,
                    )
                    .map_err(|_| failure("native full64 invocation geometry"))?;
                let invocation = Invocation::NativeConditionalFill64V1 {
                    contract_identity: *premises.contract_identity(),
                    premise_identity: *premises.identity(),
                };
                let parts = packed.into_runtime_inputs(geometry, 0, timeout_milliseconds);
                let hsaco = self.files.publication.exact_artifact_bytes();
                budget.charge_work(hsaco.len().checked_add(4096).ok_or(Resource::Arithmetic)?)?;
                let object: [u8; 32] = Sha256::digest(hsaco).into();
                let storage = parts
                    .storage
                    .prepare(hsaco, abi.entry_name())
                    .map_err(failure)?;
                let kernel = fe2o3_amdhsa_loader::validate(
                    hsaco,
                    fe2o3_amdhsa_loader::AdmittedProfile::Gfx942XnackOffCov6,
                )
                .map_err(|error| failure(format_args!("native loader plan: {error:?}")))?
                .bind_kernel(abi.entry_name())
                .map_err(|error| {
                    failure(format_args!("native selected kernel closure: {error:?}"))
                })?;
                let prepared = storage.prepared();
                require(
                    prepared.kernel_name() == abi.entry_name()
                        && prepared.identity() == kernel.identity_inputs()
                        && prepared.identity().object_sha256() == object
                        && prepared.finalized_hsaco_length() == hsaco.len() as u64
                        && Some(prepared.descriptor_offset())
                            == kernel
                                .selected_binding()
                                .descriptor_address()
                                .checked_sub(kernel.envelope().plan().image_start()),
                    "native actual finalized entry differs",
                )?;
                let storage = storage
                    .project_native_conditional_fill64(hsaco, premises)
                    .map_err(failure)?;
                require(
                    storage.prepared().invocation_binding() == invocation,
                    "native projection family differs",
                )?;
                let dispatch_contract = storage.prepared().dispatch_contract_sha256();
                drop(plan);
                (
                    storage,
                    parts.footprint,
                    object,
                    dispatch_contract,
                    invocation,
                    plan_storage,
                    abi_storage,
                )
            };
            budget.release_storage(
                plan_storage
                    .checked_add(abi_storage)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            Ok((storage, footprint, object, dispatch_contract, invocation))
        })();
        // Close the original custody even when argument callbacks or packing fail.
        self.revalidate()?;
        let (storage, footprint, object, dispatch_contract, invocation) = prepared?;
        let retention = self.epoch.retain_carrier()?;
        Ok(GeneratedRuntimeCarrierV1 {
            storage,
            footprint,
            result_budget: result_budget.clone(),
            authority: Authority {
                _retention: retention,
                root: self,
                object,
                length: self.files.publication.exact_artifact_bytes().len() as u64,
                kernel: K::EXPORT_NAME,
                dispatch_contract,
                invocation,
                device_unique_id: device.observation().unique_id(),
            },
        })
    }
}

fn packed_count(
    packed: &crate::generated_kfd_arguments::GeneratedPackedArgumentsViewV1<'_>,
    kernel_id: [u8; 32],
) -> Result<u64> {
    require(
        *packed.kernel_id.as_bytes() == kernel_id
            && packed.alignment == 8
            && packed.pointer_width == fe2o3_artifacts::PointerWidth::Bits64,
        "native packed kernel or ABI alignment",
    )?;
    let [buffer] = packed.buffers else {
        return Err(failure("native packed output cardinality"));
    };
    let [fixup] = packed.pointer_fixups else {
        return Err(failure("native packed pointer cardinality"));
    };
    require(
        packed.explicit_kernarg.len() == 16
            && packed.explicit_kernarg[..8] == [0; 8]
            && buffer.access() == fe2o3_runtime::Gfx942RuntimeBufferAccessV1::WriteOnly
            && fixup.kernarg_offset() == 0
            && fixup.buffer_index() == 0
            && fixup.buffer_byte_offset() == 0
            && fixup.required_alignment() == 4,
        "native packed exact exclusive whole-output ABI",
    )?;
    let count = u64::from_le_bytes(packed.explicit_kernarg[8..16].try_into().unwrap());
    require(
        count != 0 && count.checked_mul(4) == Some(buffer.bytes().len() as u64),
        "native packed output count or extent",
    )?;
    Ok(count)
}

fn borrow_original<'a, T>(current: &'a RefCell<T>, failed: &Cell<bool>) -> Result<RefMut<'a, T>> {
    if failed.get() {
        return Err(failure("native invocation custody is terminal"));
    }
    current.try_borrow_mut().map_err(|_| {
        failed.set(true);
        failure("reentrant native invocation custody")
    })
}

struct Authority<'root, 'scope, 'work> {
    _retention: CarrierRetention<'root>,
    root: &'root NativeConditionalFillInvocationScopeV1<'scope, 'work>,
    object: [u8; 32],
    length: u64,
    kernel: &'static str,
    dispatch_contract: [u8; 32],
    invocation: Invocation,
    device_unique_id: u64,
}
impl GeneratedRuntimeAuthorityV1 for Authority<'_, '_, '_> {
    fn artifact_bytes(&self) -> &[u8] {
        self.root.files.publication.exact_artifact_bytes()
    }
}

// SAFETY: only the private checked factory above constructs this carrier. It
// borrows the actual original native V5 publication/source, live protected proof
// and registered currentness under the original account, never a detached hash.
// Actual generated packing discharges the closed descriptor's exact ABI and
// spatial premises; mandatory native projection checks the machine and full64
// resource profile. The opaque carrier keeps original charged decoder storage.
// Lexical runtime custody prevents any borrowed owner from escaping or dropping
// before native/readback/DATA settlement, and fails stop on uncertain effects.
#[allow(unsafe_code)]
unsafe impl WorkerV3Gfx942ExecutionAuthorityV1 for Authority<'_, '_, '_> {
    type CurrentnessError = Error;
    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.object
    }
    fn finalized_hsaco_length(&self) -> u64 {
        self.length
    }
    fn kernel_name(&self) -> &str {
        self.kernel
    }
    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.dispatch_contract
    }
    fn invocation_binding(&self) -> Invocation {
        self.invocation
    }
    fn device_unique_id(&self) -> u64 {
        self.device_unique_id
    }
    fn revalidate_currentness(&self) -> Result<()> {
        self.root.revalidate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_kfd_arguments::{
        GeneratedKfdPackingObservationV1, GeneratedPackedArgumentsViewV1,
    };
    use fe2o3_artifacts::PointerWidth;
    use fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1 as Fixup;
    use fe2o3_runtime::{
        Gfx942RuntimeBufferAccessV1 as Access, Gfx942RuntimeDispatchBufferV1 as Buffer,
    };

    #[test]
    fn scoped_native_packing_checks_actual_whole_output_not_supplied_extent() {
        let mut explicit = [0; 16];
        explicit[8..].copy_from_slice(&65_u64.to_le_bytes());
        let observation = GeneratedKfdPackingObservationV1::empty_for_test();
        let buffers = [Buffer::new(vec![0; 260], Access::WriteOnly).unwrap()];
        let fixups = [Fixup::new(0, 0, 0, 4)];
        let mut view = GeneratedPackedArgumentsViewV1 {
            kernel_id: crate::KernelId::from_bytes([7; 32]),
            alignment: 8,
            pointer_width: PointerWidth::Bits64,
            argument_fields: &[],
            explicit_kernarg: &explicit,
            buffers: &buffers,
            pointer_fixups: &fixups,
            observation: &observation,
        };
        assert_eq!(packed_count(&view, [7; 32]).unwrap(), 65);
        assert!(packed_count(&view, [8; 32]).is_err());
        view.pointer_width = PointerWidth::Bits32;
        assert!(packed_count(&view, [7; 32]).is_err());
        view.pointer_width = PointerWidth::Bits64;
        view.alignment = 16;
        assert!(packed_count(&view, [7; 32]).is_err());
        view.alignment = 8;
        let invalids = [[0; 16], [1; 16]];
        for invalid in &invalids {
            view.explicit_kernarg = invalid;
            assert!(packed_count(&view, [7; 32]).is_err());
        }
        view.explicit_kernarg = &explicit;
        let short = [Buffer::new(vec![0; 256], Access::WriteOnly).unwrap()];
        view.buffers = &short;
        assert!(packed_count(&view, [7; 32]).is_err());
        let read_write = [Buffer::new(vec![0; 260], Access::ReadWrite).unwrap()];
        view.buffers = &read_write;
        assert!(packed_count(&view, [7; 32]).is_err());
        view.buffers = &buffers;
        let offset = [Fixup::new(0, 0, 4, 4)];
        view.pointer_fixups = &offset;
        assert!(packed_count(&view, [7; 32]).is_err());
        let aliases = [Fixup::new(0, 0, 0, 4), Fixup::new(0, 0, 0, 4)];
        view.pointer_fixups = &aliases;
        assert!(packed_count(&view, [7; 32]).is_err());
    }

    #[test]
    fn scoped_native_currentness_reentry_retains_original_owner_and_poison_is_sticky() {
        let original = Box::new(31);
        let address: *const i32 = &*original;
        let current = RefCell::new(original);
        let failed = Cell::new(false);
        let held = borrow_original(&current, &failed).unwrap();
        assert!(std::ptr::eq::<i32>(&**held, address));
        assert!(borrow_original(&current, &failed).is_err());
        assert!(failed.get());
        drop(held);
        assert!(borrow_original(&current, &failed).is_err());
        assert!(std::ptr::eq::<i32>(&**current.borrow(), address));
    }
}
