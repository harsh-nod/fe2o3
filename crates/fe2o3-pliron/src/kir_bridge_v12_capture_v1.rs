// This allocation-free census only reads the already admitted output. Symbol
// byte lengths do not change the number of pointer-keyed roster entries.
fn optimization_roster_endpoint_count_v12(
    output: &Module,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<usize, crate::KirOptimizationMapErrorV12> {
    use crate::KirOptimizationMapErrorV12 as E;
    budget.charge_work(1)?;
    let mut endpoints = 0usize;
    for function in &output.functions {
        budget.charge_work(1)?;
        let Some(body) = &function.body else { continue };
        endpoints = endpoints
            .checked_add(body.parameters.len())
            .ok_or(E::Arithmetic)?;
        for block in &body.blocks {
            budget.charge_work(1)?;
            endpoints = endpoints
                .checked_add(block.parameters.len())
                .and_then(|n| n.checked_add(block.operations.len()))
                .and_then(|n| n.checked_add(usize::from(block.terminator.is_some())))
                .ok_or(E::Arithmetic)?;
            for operation in &block.operations {
                budget.charge_work(1)?;
                endpoints = endpoints
                    .checked_add(operation.results.len())
                    .ok_or(E::Arithmetic)?;
            }
        }
    }
    // index_live_functions performs at most F fixed-size pointer-key lookups.
    // Keep a worst-case collision allowance separate from the logical visitor.
    budget.charge_work(
        output
            .functions
            .len()
            .checked_mul(output.functions.len())
            .ok_or(E::Arithmetic)?,
    )?;
    Ok(endpoints)
}

impl<'input> KirPlironGraphV12<'input> {
    pub(crate) const fn neutral_input_v1(&self) -> &'input VerifiedCanonicalKernelIrModuleV12 {
        self.source
    }
    pub(crate) fn neutral_occurrence_limits_v1(
        &self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<crate::kir_occurrence_capture_v1::Limits, crate::KirOptimizationMapErrorV12> {
        crate::kir_occurrence_capture_v1::Limits::for_graph(
            &self.session.context,
            self.session.operations[&self.root.identity],
            self.source.module(),
            budget,
        )
    }
    pub(crate) fn neutral_live_roster_v1(
        &self,
        limit: usize,
        meter: &mut dyn FnMut(usize) -> Result<(), crate::KirOptimizationMapErrorV12>,
    ) -> Result<crate::kir_optimization_map_v12::LiveRosterV12, crate::KirOptimizationMapErrorV12>
    {
        self.optimization_roster_metered_v1::<true, _>(limit, meter)
    }
    pub(crate) fn begin_neutral_occurrence_capture_v1(
        &self,
        limits: crate::kir_occurrence_capture_v1::Limits,
    ) -> Result<crate::kir_occurrence_capture_v1::Capture, crate::KirOptimizationMapErrorV12> {
        self.begin_neutral_occurrence_capture_for_policy_v1(
            limits,
            crate::fixed_policy_v3::FixedPolicy::Historical2,
        )
    }
    pub(crate) fn begin_neutral_occurrence_capture_for_policy_v1(
        &self,
        limits: crate::kir_occurrence_capture_v1::Limits,
        policy: crate::fixed_policy_v3::FixedPolicy,
    ) -> Result<crate::kir_occurrence_capture_v1::Capture, crate::KirOptimizationMapErrorV12> {
        let mut work = 0usize;
        let allowance = limits.work()?;
        let roster =
            self.optimization_roster_metered_v1::<true, _>(limits.nodes, &mut |units| {
                work = work
                    .checked_add(units)
                    .ok_or(crate::KirOptimizationMapErrorV12::Arithmetic)?;
                if work > allowance {
                    return Err(crate::KirOptimizationMapErrorV12::Limit);
                }
                Ok(())
            })?;
        let create = match policy {
            crate::fixed_policy_v3::FixedPolicy::Historical2 => {
                return crate::kir_occurrence_capture_v1::Capture::new(
                    &self.session.context,
                    self.session.operations[&self.root.identity],
                    self.source.module(),
                    &roster,
                    limits,
                    work,
                );
            }
            crate::fixed_policy_v3::FixedPolicy::Checked3
            | crate::fixed_policy_v3::FixedPolicy::Integer6 => {
                crate::kir_occurrence_capture_v1::Capture::new_for_policy
            }
        };
        create(
            &self.session.context,
            self.session.operations[&self.root.identity],
            self.source.module(),
            &roster,
            limits,
            work,
            policy,
        )
    }
}
