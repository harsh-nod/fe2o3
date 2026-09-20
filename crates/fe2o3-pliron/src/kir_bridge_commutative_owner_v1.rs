// Included in the existing V12 bridge; the native witness cannot be fabricated.
impl KirPlironGraphV12<'_> {
    pub(crate) fn begin_commutative_capture_v1(
        &self,
        limits: crate::kir_occurrence_capture_v1::Limits,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        crate::kir_occurrence_capture_v1::CommutativeCapture,
        crate::KirOptimizationMapErrorV12,
    > {
        use crate::KirOptimizationMapErrorV12 as E;
        let mut work = 0usize;
        let allowance = limits.work()?;
        let roster =
            self.optimization_roster_metered_v1::<true, _>(limits.nodes, &mut |units| {
                work = work.checked_add(units).ok_or(E::Arithmetic)?;
                if work > allowance {
                    return Err(E::Limit);
                }
                Ok(())
            })?;
        crate::kir_occurrence_capture_v1::CommutativeCapture::new(
            &self.session.context,
            self.session.operations[&self.root.identity],
            self.source.module(),
            &roster,
            limits,
            work,
            budget,
        )
    }
    pub(crate) fn execute_commutative_cse_v1(
        &mut self,
        capture: &crate::kir_occurrence_capture_v1::CommutativeCapture,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<crate::CommutativeBitwiseExecutionV1, crate::commutative_cse_owner_v1::Error> {
        use crate::commutative_cse_owner_v1::Error as E;
        use CanonicalKernelIrVerificationResourceErrorV1 as R;
        self.validate_custody_v12().map_err(E::Bridge)?;
        if self.optimization_started {
            return Err(R::Accounting.into());
        }
        // Same conservative opaque upstream allowances as the native execution
        // base plus DomInfo, but no old capture/map/report/policy attribution.
        // Dynamic raw work and visible tables are charged separately.
        let volume = self
            .source
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(32_769)
            .ok_or(R::Arithmetic)?;
        let work = volume
            .checked_mul(192)
            .and_then(|n| n.checked_add(25_268_224))
            .ok_or(R::Arithmetic)?;
        let persistent = volume
            .checked_mul(8)
            .and_then(|n| n.checked_add(4096))
            .ok_or(R::Arithmetic)?;
        let temporary = volume
            .checked_mul(80)
            .and_then(|n| n.checked_add(4096))
            .ok_or(R::Arithmetic)?;
        let retained = self
            .retained_storage
            .checked_add(persistent)
            .ok_or(R::Arithmetic)?;
        budget.charge_work(work)?;
        budget.reserve_storage(persistent.checked_add(temporary).ok_or(R::Arithmetic)?)?;
        self.retained_storage = retained;
        self.optimization_started = true;
        let (result, work) = {
            let mut ledger = crate::fixed_policy_v3::CseLedger::new(budget);
            let result =
                self.session
                    .execute_commutative_owner_v1(&self.root, capture, &mut ledger);
            let work = ledger.finish();
            (result, work)
        };
        budget.release_storage(temporary)?;
        let dynamic_work = work?;
        let mut report = result?;
        report.dynamic_work = dynamic_work;
        Ok(report)
    }
    pub(crate) fn extract_commutative_cse_v1(
        &mut self,
        witness: &NativeBridgeWitnessV1,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            KirBridgeOptimizedReceiptV1,
            KirBridgeStorageV12,
        ),
        KirBridgeErrorV12,
    > {
        if !self.optimization_started {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        // This private entry retains the exact accepted extraction reservation;
        // the outer closed owner scope controls final transfer/cleanup.
        self.extract_admitted_inner_v1(budget, false, Some(witness))
    }
}
