//! Three original charged invocations, without an aggregate authority conversion.

use super::*;
use fe2o3_runtime::RuntimeGfx942GeneratedCohort3V1;

impl<'scope, 'work> NativeConditionalFillInvocationScopeV1<'scope, 'work> {
    /// Prepares exactly three original closed-fill invocations under this actual
    /// proof/account epoch. Each keeps its own source, argument charge and result
    /// decoder. The inert wrapper authorizes no launch and cannot enter the
    /// singleton runtime path. Use the runtime's distinct whole-cohort scope.
    /// Different output lengths are permitted with separately valid full64 grids;
    /// ordered Batch3 completion is not independent or out-of-order completion.
    ///
    /// This type-checks the actual lending chain without constructing protected
    /// owners or executing it. A real application supplies a bounded timer/wake
    /// source instead of the immediately-ready demonstration below.
    ///
    /// ```no_run
    /// use fe2o3_host::*;
    /// use fe2o3_runtime::*;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// async fn compose<'w, K, A>(
    ///     app: &mut ProvedNativeConditionalFillApplicationV1<'w>, budget: &mut Budget<'w>,
    ///     context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    ///     device: RuntimeDeviceIdV1, stream: RuntimeStreamIdV1, arguments: [A;3],
    ///     geometries: [AqlDispatchGeometryV1;3], limits: GeneratedRuntimeArgumentLimitsV1,
    ///     result_budget: &GeneratedRuntimeResultBudgetV1,
    ///     mut results: [GeneratedRuntimeChargedResultV1<u32>;3],
    /// ) where K: CompilerGeneratedKernelExpectationV1, A: CompilerGeneratedRuntimeArguments<K> {
    ///     let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
    ///     app.with_native_invocation_scope_async_v1(budget,deadline,async |root| {
    ///         context.with_generated_gfx942_cohort3_scope_async_v1(deadline,
    ///             |_| std::future::ready(()), async |scope| {
    ///                 let ticket=scope.try_submit_cohort3_v1(device,stream,|checked| {
    ///                     root.prepare_generated_cohort3::<K,A>(arguments,checked,geometries,
    ///                         100,limits,result_budget)
    ///                 }).unwrap();
    ///                 let observed=scope.cohort3_completion_future_v1(&ticket).unwrap();
    ///                 scope.drive_with_wake_v1(|_| std::future::ready(())).await.unwrap();
    ///                 observed.await.unwrap();
    ///                 for (index,result) in results.iter_mut().enumerate() {
    ///                     let _original=result.take_cohort3_completed_v1(scope,&ticket,index).unwrap();
    ///                 }
    ///             }).await.unwrap();
    ///         Ok(())
    ///     }).await.unwrap();
    /// }
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_cohort3<'a, K, A>(
        &'a self,
        arguments: [A; 3],
        device: &CheckedGfx942XnackMinusDevice,
        geometries: [AqlDispatchGeometryV1; 3],
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        RuntimeGfx942GeneratedCohort3V1<
            impl RuntimeGfx942GeneratedCompletionCarrierV1<CurrentnessError = Error>
            + 'a
            + use<'a, 'scope, 'work, K, A>,
        >,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        let [a, b, c] = arguments;
        let [ag, bg, cg] = geometries;
        let members = [
            self.prepare_generated_invocation::<K, A>(
                a,
                device,
                ag,
                timeout_milliseconds,
                limits,
                result_budget,
            )?,
            self.prepare_generated_invocation::<K, A>(
                b,
                device,
                bg,
                timeout_milliseconds,
                limits,
                result_budget,
            )?,
            self.prepare_generated_invocation::<K, A>(
                c,
                device,
                cg,
                timeout_milliseconds,
                limits,
                result_budget,
            )?,
        ];
        self.revalidate()?;
        Ok(RuntimeGfx942GeneratedCohort3V1::new(members))
    }
}
