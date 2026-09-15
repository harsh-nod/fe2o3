use super::*;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticLocalRoleV1;

pub(in crate::collector::production_importer_v1) struct Roster<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    types: &'a [SemanticTypeDeclV1],
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    defined: BTreeMap<SemanticFunctionIdentityV1, SemanticFunctionIdV1>,
    terminals: BTreeMap<SemanticFunctionIdentityV1, usize>,
}

impl<'a, 'tcx> Roster<'a, 'tcx> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
        types: &'a [SemanticTypeDeclV1],
        functions: &'a [SemanticFunctionDeclV1],
        callables: &'a [SemanticCallableDeclV1],
    ) -> Result<Self, ProductionSemanticImportErrorV1> {
        if functions.len() != plan.function_producers().len()
            || functions.len() != plan.function_abi_producers().len()
            || functions.len() != plan.body_producers().len()
            || types.len() != plan.type_producers().len()
            || callables.len() != functions.len() + plan.terminal_producers().len()
        {
            return Err(rejected("defined Math requires complete canonical rosters"));
        }
        // Reuse the exact converters: matching digest labels alone would not
        // detect a replaced layout or ABI payload under an unchanged identity.
        let expected_types = construct_production_semantic_types_v1(tcx, plan.type_producers())
            .map_err(|error| ProductionSemanticImportErrorV1::TypeConstruction(Box::new(error)))?
            .into_records();
        if expected_types != types {
            return Err(rejected("defined Math substituted semantic type roster"));
        }
        let expected_abis = construct_production_semantic_fn_abis_v1(
            tcx,
            plan.function_abi_producers(),
            plan.type_producers(),
        )
        .map_err(|error| ProductionSemanticImportErrorV1::FunctionAbiConstruction(Box::new(error)))?
        .into_records();
        let terminal_abis = plan
            .terminal_producers()
            .iter()
            .map(|terminal| terminal.abi.clone())
            .collect::<Vec<_>>();
        let terminal_abis =
            construct_production_semantic_fn_abis_v1(tcx, &terminal_abis, plan.type_producers())
                .map_err(|error| {
                    ProductionSemanticImportErrorV1::FunctionAbiConstruction(Box::new(error))
                })?
                .into_records();
        let mut defined = BTreeMap::new();
        for (index, ((function, producer), abi)) in functions
            .iter()
            .zip(plan.function_producers())
            .zip(&expected_abis)
            .enumerate()
        {
            let id = SemanticFunctionIdV1::from_index(index as u32);
            let observed = canonical_function_identities_v1(tcx, producer.instance);
            if function.identity() != observed.function()
                || producer.identities.function() != observed.function()
                || function.item_definition_identity() != observed.item_definition()
                || function.monomorphization_identity() != observed.monomorphization()
                || function.generic_type_arguments_identity() != observed.generic_type_arguments()
                || function.const_generic_arguments_identity() != observed.const_generic_arguments()
                || function.role() != semantic_function_role_v1(producer.role)
                || function.abi() != abi
                || plan.function_abi_producers()[index].function != id
                || callables[index] != SemanticCallableDeclV1::defined(id)
                || defined.insert(function.identity(), id).is_some()
            {
                return Err(rejected(
                    "defined Math function roster identity or ABI transport",
                ));
            }
        }
        let mut terminals = BTreeMap::new();
        for (index, (producer, abi)) in plan
            .terminal_producers()
            .iter()
            .zip(&terminal_abis)
            .enumerate()
        {
            let Some(binding) = callables[functions.len() + index].binding() else {
                return Err(rejected("defined Math missing terminal binding"));
            };
            let observed = canonical_function_identities_v1(tcx, producer.instance);
            let expected_binding = SemanticNonBodyCallableBindingV1::new(
                observed.function(),
                observed.item_definition(),
                observed.monomorphization(),
                observed.generic_type_arguments(),
                observed.const_generic_arguments(),
                producer.source.provenance,
                abi.clone(),
            );
            if binding.identity() != observed.function()
                || producer.identities.function() != observed.function()
                || binding != &expected_binding
                || defined.contains_key(&binding.identity())
                || terminals.insert(binding.identity(), index).is_some()
            {
                return Err(rejected(
                    "defined Math terminal roster identity or ABI transport",
                ));
            }
        }
        Ok(Self {
            tcx,
            plan,
            types,
            functions,
            callables,
            defined,
            terminals,
        })
    }

    pub fn defined(
        &self,
        function: SemanticFunctionIdV1,
    ) -> Result<Instance<'tcx>, ProductionSemanticImportErrorV1> {
        let producer = self
            .plan
            .function_producers()
            .get(function.index() as usize)
            .ok_or_else(|| rejected("defined Math absent function producer"))?;
        if !matches!(
            producer.instance.def,
            rustc_middle::ty::InstanceKind::Item(_)
        ) || !self.tcx.is_mir_available(producer.instance.def_id())
            || !self.plan.function_mir(function).is_some_and(|retained| {
                std::ptr::eq(retained, self.tcx.instance_mir(producer.instance.def))
            })
        {
            return Err(rejected(
                "defined Math requires the original retained rustc body",
            ));
        }
        let raw = self
            .plan
            .function_mir(function)
            .ok_or_else(|| rejected("defined Math absent retained body"))?;
        let retained = &self.plan.body_producers()[function.index() as usize];
        let semantic = &self.functions[function.index() as usize];
        if retained.function != function
            || semantic.source() != retained.source.provenance
            || semantic.entry() != retained.entry
            || semantic.locals().len() != retained.locals.len()
            || semantic.blocks().len() != retained.blocks.len()
            || semantic
                .locals()
                .iter()
                .zip(&retained.locals)
                .any(|(local, binding)| {
                    let role = match binding.rustc_local {
                        0 => SemanticLocalRoleV1::Return,
                        index if index as usize <= raw.arg_count => {
                            SemanticLocalRoleV1::Argument(index - 1)
                        }
                        _ => SemanticLocalRoleV1::Temporary,
                    };
                    local.identity() != binding.identity
                        || local.ty() != binding.ty
                        || local.source() != binding.source.provenance
                        || local.role() != role
                })
            || semantic
                .blocks()
                .iter()
                .zip(&retained.blocks)
                .any(|(block, binding)| {
                    block.identity() != binding.identity
                        || block.source() != binding.source.provenance
                        || block.statements().len() != binding.statements.len()
                        || block
                            .statements()
                            .iter()
                            .zip(&binding.statements)
                            .any(|(statement, source)| statement.source() != source.provenance)
                        || block.terminator().source() != binding.terminator.provenance
                })
        {
            return Err(rejected(
                "defined Math canonical local/block/source mapping transport",
            ));
        }
        Ok(producer.instance)
    }

    pub fn reconstructed_body<'p>(
        &self,
        function: SemanticFunctionIdV1,
        replay: &mut source_body_v1::Replay<'p, 'tcx>,
        work: &mut usize,
    ) -> Result<source_body_v1::Correspondence<'p, 'a>, ProductionSemanticImportErrorV1> {
        // new() has independently checked this exact source ABI and type roster.
        let functions = self.functions;
        let semantic = functions.get(function.index() as usize)
            .ok_or_else(|| rejected("defined source replay missing function"))?;
        replay.check(self.plan, function, semantic, work)
    }

    pub fn defined_instance(
        &self,
        instance: Instance<'tcx>,
    ) -> Result<SemanticFunctionIdV1, ProductionSemanticImportErrorV1> {
        let identity = canonical_function_identities_v1(self.tcx, instance).function();
        let function = *self
            .defined
            .get(&identity)
            .ok_or_else(|| rejected("defined Math missing original bridge"))?;
        if self.defined(function)? != instance {
            return Err(rejected("defined Math substituted bridge instance"));
        }
        Ok(function)
    }

    pub fn current_instance(
        &self,
        instance: Instance<'tcx>,
    ) -> Result<SemanticCallableIdV1, ProductionSemanticImportErrorV1> {
        let identity = canonical_function_identities_v1(self.tcx, instance).function();
        let index = *self
            .terminals
            .get(&identity)
            .ok_or_else(|| rejected("defined Math missing Current terminal producer"))?;
        let producer = &self.plan.terminal_producers()[index];
        let callable = SemanticCallableIdV1::from_index((self.functions.len() + index) as u32);
        if producer.instance != instance
            || producer.expansion != ProductionTerminalExpansionV1::MathContextCurrent
            || !matches!(
                &self.callables[callable.index() as usize],
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::MathContextCurrent { .. },
                    ..
                }
            )
        {
            return Err(rejected(
                "defined Math substituted Current terminal instance or operation",
            ));
        }
        Ok(callable)
    }

    pub fn types<const N: usize>(
        &self,
        rust_types: [Ty<'tcx>; N],
    ) -> Result<[SemanticTypeIdV1; N], ProductionSemanticImportErrorV1> {
        let mut ids = [SemanticTypeIdV1::from_index(0); N];
        for (id, ty) in ids.iter_mut().zip(rust_types) {
            *id = semantic_type_for_rust_v1(self.tcx, self.types, ty)?;
            if self.plan.type_producers()[id.index() as usize].ty != ty {
                return Err(rejected(
                    "defined Math Rust/semantic type identity transport",
                ));
            }
        }
        Ok(ids)
    }

    pub fn root(
        &self,
        root: &AuthenticatedProductionKernelContextRootV1,
        contexts: &AuthenticatedProductionKernelContextsV1,
    ) -> Result<(), ProductionSemanticImportErrorV1> {
        let function = self
            .functions
            .get(root.selected_root.index() as usize)
            .ok_or_else(|| rejected("defined Math missing authenticated root"))?;
        if !self.plan.roots().contains(&root.selected_root)
            || !contexts.expected_roots.contains(&root.selected_root)
            || function.role() != SemanticFunctionRoleV1::KernelRoot
            || function.identity().as_bytes() != &root.root_function_identity
            || function.kernel_entry().is_none_or(|entry| {
                entry.kernel_binding_identity().as_bytes() != &root.kernel_binding
            })
        {
            return Err(rejected(
                "defined Math canonical authenticated root identity transport",
            ));
        }
        Ok(())
    }

    pub fn getter_edges(
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
                "defined Math getter/bridge retained direct-call recipe",
            ));
        }
        let calls = self
            .plan
            .terminal_expansion_producers()
            .iter()
            .filter(|call| call.caller == bridge)
            .collect::<Vec<_>>();
        if !matches!(calls.as_slice(), [call] if call.block == 0 && call.arguments == 0
            && call.instance == instance && call.expansion == ProductionTerminalExpansionV1::MathContextCurrent
            && self.functions.len() + call.terminal as usize == current.index() as usize
            && call.identities.function() == canonical_function_identities_v1(self.tcx, instance).function())
        {
            return Err(rejected(
                "defined Math bridge/Current retained terminal recipe",
            ));
        }
        Ok(())
    }
}

include!("roster_matrix.rs");
