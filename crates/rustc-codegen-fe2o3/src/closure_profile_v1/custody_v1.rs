//! Borrow custody follows admitted device calls, never a helper's name or role alone.

use super::*;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct FunctionKeyV1 {
    function: [u8; 32],
    monomorphization: [u8; 32],
    mir: [u8; 32],
    abi: [u8; 32],
}

impl FunctionKeyV1 {
    pub(super) fn from_instance<'tcx>(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
    ) -> Result<Self, ClosureProfileErrorV1> {
        let identities =
            crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(tcx, instance);
        let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
        let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
            ClosureProfileErrorV1::new(format!("closure custody FnAbi unavailable: {error:?}"))
        })?;
        Ok(Self {
            function: *identities.function().as_bytes(),
            monomorphization: *identities.monomorphization().as_bytes(),
            mir: crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance),
            abi: crate::rustc_semantic_adapter_v1::rustc_fn_abi_sha256_v1(tcx, abi),
        })
    }

    fn from_call(call: &ClosureTransportCallV1) -> Self {
        Self {
            function: call.target_function_identity,
            monomorphization: call.target_monomorphization_identity,
            mir: call.target_mir_identity,
            abi: call.target_fn_abi_identity,
        }
    }

    pub(super) fn hash_into(&self, hash: &mut Sha256) {
        hash.update(self.function);
        hash.update(self.monomorphization);
        hash.update(self.mir);
        hash.update(self.abi);
    }
}

#[derive(Clone, Debug)]
pub(super) struct TransportedArgumentV1 {
    closure_type: [u8; 32],
    identity: [u8; 32],
}

impl TransportedArgumentV1 {
    pub(super) fn require_type<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        ty: Ty<'tcx>,
    ) -> Result<(), ClosureProfileErrorV1> {
        if crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, ty).as_bytes()
            != &self.closure_type
        {
            return Err(ClosureProfileErrorV1::new(
                "transported closure argument changed its exact environment type",
            ));
        }
        Ok(())
    }

    pub(super) fn identity(&self) -> [u8; 32] {
        self.identity
    }
}

/// Only successful caller admission can mint custody. Every incoming call is
/// checked, including calls to a callee already visited by the collector.
#[derive(Default)]
pub(crate) struct CollectedClosureCustodyV1 {
    arguments: BTreeMap<(String, FunctionKeyV1), BTreeMap<usize, TransportedArgumentV1>>,
}

impl CollectedClosureCustodyV1 {
    pub(crate) fn analyze<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
        internal_helper: bool,
        target: &str,
    ) -> Result<ProductionClosureLoweringV1, ClosureProfileErrorV1> {
        let incoming = if internal_helper {
            self.arguments.get(&(
                target.to_owned(),
                FunctionKeyV1::from_instance(tcx, instance)?,
            ))
        } else {
            None
        };
        analyze_with_custody_v1(
            tcx,
            instance,
            ClosureOriginPolicyV1::Either,
            target,
            incoming,
        )
    }

    pub(crate) fn observe_calls<'tcx>(
        &mut self,
        tcx: TyCtxt<'tcx>,
        caller: Instance<'tcx>,
        plan: Option<&ProductionClosureLoweringV1>,
    ) -> Result<(), ClosureProfileErrorV1> {
        let body = tcx.instance_mir(caller.def);
        if let Some(plan) = plan
            && plan.owner != FunctionKeyV1::from_instance(tcx, caller)?
        {
            return Err(ClosureProfileErrorV1::new(
                "closure custody plan belongs to a different caller, MIR, or ABI",
            ));
        }
        let roots = plan
            .into_iter()
            .flat_map(|plan| &plan.environments)
            .map(|environment| Local::from_usize(environment.local))
            .collect::<BTreeSet<_>>();
        for (block_index, block) in body.basic_blocks.iter_enumerated() {
            let TerminatorKind::Call { func, args, .. } = &block.terminator().kind else {
                continue;
            };
            for (argument_index, argument) in args.iter().enumerate() {
                let argument_ty = normalized_ty(
                    tcx,
                    caller,
                    argument.node.ty(body, tcx),
                    "closure custody call argument",
                )?;
                if !matches!(argument_ty.kind(), TyKind::Closure(..)) {
                    continue;
                }
                let plan = plan.ok_or_else(|| {
                    ClosureProfileErrorV1::new(
                        "closure-bearing call has no admitted caller environment custody",
                    )
                })?;
                let block = block_index.as_usize();
                if let Some(call) = plan
                    .transport_calls
                    .iter()
                    .find(|call| call.block == block && call.argument_index == argument_index)
                {
                    if resolve_operand_closure_custody(
                        tcx,
                        caller,
                        argument,
                        argument_ty,
                        &roots,
                        &plan.aliases,
                    )? != call.closure_custody
                    {
                        return Err(ClosureProfileErrorV1::new(
                            "closure transport changed its exact caller environment root",
                        ));
                    }
                    let target = resolve_direct_call(tcx, caller, func)?;
                    let target_key = FunctionKeyV1::from_instance(tcx, target)?;
                    if target_key != FunctionKeyV1::from_call(call) {
                        return Err(ClosureProfileErrorV1::new(
                            "closure transport changed its exact callee identity, MIR, or ABI",
                        ));
                    }
                    let expected_type = match &call.closure_custody {
                        ClosureCustodyV1::EnvironmentLocal(local) => {
                            plan.environments
                                .iter()
                                .find(|environment| environment.local == *local)
                                .ok_or_else(|| {
                                    ClosureProfileErrorV1::new(
                                        "closure transport omitted its admitted environment",
                                    )
                                })?
                                .closure_type_identity
                        }
                        ClosureCustodyV1::ZeroSizedConstant {
                            closure_type_identity,
                            ..
                        } => *closure_type_identity,
                    };
                    let closure_type =
                        *crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, argument_ty)
                            .as_bytes();
                    if closure_type != expected_type {
                        return Err(ClosureProfileErrorV1::new(
                            "closure transport operand changed its admitted environment type",
                        ));
                    }
                    let mut hash = Sha256::new();
                    hash.update(b"fe2o3.device-closure-argument-custody.v1\0");
                    plan.owner.hash_into(&mut hash);
                    hash.update(plan.identity());
                    hash.update((block as u64).to_le_bytes());
                    hash.update((argument_index as u64).to_le_bytes());
                    hash.update(closure_type);
                    let key = (plan.target.clone(), target_key);
                    if !self.arguments.contains_key(&key)
                        && self.arguments.len() >= fe2o3_rustc_front::MAX_FUNCTIONS_V1
                    {
                        return Err(ClosureProfileErrorV1::new(
                            "closure custody exceeds the collected-function budget",
                        ));
                    }
                    let incoming = self.arguments.entry(key).or_default();
                    let custody = incoming.entry(argument_index + 1).or_insert_with(|| {
                        TransportedArgumentV1 {
                            closure_type,
                            identity: hash.finalize().into(),
                        }
                    });
                    custody.require_type(tcx, argument_ty)?;
                } else if !(argument_index == 0
                    && plan.calls.iter().any(|call| call.block == block)
                    || plan
                        .higher_order_calls
                        .iter()
                        .any(|call| call.block == block))
                {
                    return Err(ClosureProfileErrorV1::new(
                        "closure-bearing call has no exact admitted transport or invocation",
                    ));
                }
            }
        }
        Ok(())
    }
}
