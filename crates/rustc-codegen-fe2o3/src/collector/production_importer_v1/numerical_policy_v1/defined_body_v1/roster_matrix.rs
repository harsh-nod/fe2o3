// Included inside defined_body_v1::roster; keeps the Math roster guards unchanged.
impl<'a, 'tcx> Roster<'a, 'tcx> {
    pub fn matrix_current_instance(
        &self,
        instance: Instance<'tcx>,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticImportErrorV1> {
        let identity = canonical_function_identities_v1(self.tcx, instance).function();
        let index = *self
            .terminals
            .get(&identity)
            .ok_or_else(|| rejected("defined Matrix missing Current terminal producer"))?;
        let producer = &self.plan.terminal_producers()[index];
        let callable = SemanticCallableIdV1::from_index((self.functions.len() + index) as u32);
        if producer.instance != instance
            || producer.expansion != ProductionTerminalExpansionV1::MatrixContextCurrent
            || !matches!(
                &self.callables[callable.index() as usize],
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { .. },
                    ..
                }
            )
        {
            return Err(rejected(
                "defined Matrix substituted Current terminal instance or operation",
            ));
        }
        Ok(callable)
    }

    pub fn matrix_getter_edges(
        &self,
        getter: SemanticFunctionIdV1,
        bridge: SemanticFunctionIdV1,
        current: SemanticCallableIdV1,
        instance: Instance<'tcx>,
    ) -> Result<(), ProductionSemanticImportErrorV1> {
        let calls = self
            .plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == getter)
            .collect::<Vec<_>>();
        if !matches!(calls.as_slice(), [call] if call.block == 0 && call.callee == bridge)
            || self
                .plan
                .terminal_expansion_producers()
                .iter()
                .any(|call| call.caller == getter)
            || self
                .plan
                .normalized_intrinsic_producers()
                .iter()
                .any(|call| call.caller == getter || call.caller == bridge)
            || self
                .plan
                .direct_call_producers()
                .iter()
                .any(|call| call.caller == bridge)
        {
            return Err(rejected(
                "defined Matrix getter/bridge retained direct-call recipe",
            ));
        }
        let calls = self
            .plan
            .terminal_expansion_producers()
            .iter()
            .filter(|call| call.caller == bridge)
            .collect::<Vec<_>>();
        if !matches!(calls.as_slice(), [call] if call.block == 0 && call.arguments == 0
            && call.instance == instance && call.expansion == ProductionTerminalExpansionV1::MatrixContextCurrent
            && self.functions.len() + call.terminal as usize == current.index() as usize
            && call.identities.function() == canonical_function_identities_v1(self.tcx, instance).function())
        {
            return Err(rejected(
                "defined Matrix bridge/Current retained terminal recipe",
            ));
        }
        Ok(())
    }
}
