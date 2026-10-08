//! The actual protected host owner prepares four separately charged originals.

use super::*;
use crate::generated_runtime_carrier::GeneratedRegistryCarrierV1;
use fe2o3_runtime::{RuntimeGfx942GeneratedRegistry4V1, RuntimeGfx942RegistryCompletionCarrierV1};

impl<'scope, 'work> NativeConditionalFillInvocationScopeV1<'scope, 'work> {
    /// Prepares sixteen original closed-fill sources and sixteen independent
    /// copied-result credits under this actual proof/currentness loan. All
    /// source residuals remain retained through destruction of one common native
    /// registry. No hardware completion order or rolling admission is claimed.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_registry16<'a, K, A>(
        &'a self,
        arguments: [A; 16],
        device: &CheckedGfx942XnackMinusDevice,
        geometries: [AqlDispatchGeometryV1; 16],
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        fe2o3_runtime::RuntimeGfx942GeneratedRegistry16V1<
            impl RuntimeGfx942RegistryCompletionCarrierV1<CurrentnessError = Error>
            + 'a
            + use<'a, 'scope, 'work, K, A>,
        >,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        let mut members = core::array::from_fn::<_, 16, _>(|_| None);
        for ((slot, arguments), geometry) in members.iter_mut().zip(arguments).zip(geometries) {
            *slot = Some(
                GeneratedRegistryCarrierV1::new(self.prepare_carrier::<K, A>(
                    arguments,
                    device,
                    geometry,
                    timeout_milliseconds,
                    limits,
                    result_budget,
                )?)
                .map_err(failure)?,
            );
        }
        self.revalidate()?;
        Ok(fe2o3_runtime::RuntimeGfx942GeneratedRegistry16V1::new(
            members.map(|member| member.unwrap_or_else(|| std::process::abort())),
        ))
    }

    /// Prepares the same four native sources for two bounded result cycles.
    /// Both output frames are prepaid in the original result account before
    /// native entry; returned observers belong only to cycle two. Cycle-one
    /// observers remain those created with `arguments`. Neither can release the
    /// retained sources, common DATA or proof/currentness loan.
    /// This is eight publications maximum, four outstanding, not high depth.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_registry4_repeat2<'a, K, A>(
        &'a self,
        arguments: [A; 4],
        device: &CheckedGfx942XnackMinusDevice,
        geometries: [AqlDispatchGeometryV1; 4],
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<(
        fe2o3_runtime::RuntimeGfx942GeneratedRegistry4Repeat2V1<
            impl fe2o3_runtime::RuntimeGfx942RegistryRepeat2CarrierV1<CurrentnessError = Error>
            + 'a
            + use<'a, 'scope, 'work, K, A>,
        >,
        [crate::GeneratedRuntimeChargedResultV1<u32>; 4],
    )>
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        let [a, b, c, d] = arguments;
        let [ag, bg, cg, dg] = geometries;
        let prepare = |arguments, geometry| {
            GeneratedRegistryCarrierV1::new_repeat2(self.prepare_carrier::<K, A>(
                arguments,
                device,
                geometry,
                timeout_milliseconds,
                limits,
                result_budget,
            )?)
            .map_err(failure)
        };
        let (a, ar) = prepare(a, ag)?;
        let (b, br) = prepare(b, bg)?;
        let (c, cr) = prepare(c, cg)?;
        let (d, dr) = prepare(d, dg)?;
        self.revalidate()?;
        Ok((
            fe2o3_runtime::RuntimeGfx942GeneratedRegistry4Repeat2V1::new([a, b, c, d]),
            [ar, br, cr, dr],
        ))
    }

    /// Prepares four original closed-fill invocations for distinct registry
    /// readback readiness. Each source remains charged and proof/currentness
    /// rooted even after its copied result is decoded. Only final registry
    /// destruction may release common native backing. Construction is inert.
    /// This is not scalar/whole-cohort completion, rolling rearm, out-of-order
    /// execution or thousands-of-native-operations qualification.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_generated_registry4<'a, K, A>(
        &'a self,
        arguments: [A; 4],
        device: &CheckedGfx942XnackMinusDevice,
        geometries: [AqlDispatchGeometryV1; 4],
        timeout_milliseconds: u32,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<
        RuntimeGfx942GeneratedRegistry4V1<
            impl RuntimeGfx942RegistryCompletionCarrierV1<CurrentnessError = Error>
            + 'a
            + use<'a, 'scope, 'work, K, A>,
        >,
    >
    where
        K: CompilerGeneratedKernelExpectationV1,
        A: CompilerGeneratedRuntimeArguments<K>,
    {
        let [a, b, c, d] = arguments;
        let [ag, bg, cg, dg] = geometries;
        let prepare = |arguments, geometry| {
            GeneratedRegistryCarrierV1::new(self.prepare_carrier::<K, A>(
                arguments,
                device,
                geometry,
                timeout_milliseconds,
                limits,
                result_budget,
            )?)
            .map_err(failure)
        };
        let members = [
            prepare(a, ag)?,
            prepare(b, bg)?,
            prepare(c, cg)?,
            prepare(d, dg)?,
        ];
        self.revalidate()?;
        Ok(RuntimeGfx942GeneratedRegistry4V1::new(members))
    }
}
