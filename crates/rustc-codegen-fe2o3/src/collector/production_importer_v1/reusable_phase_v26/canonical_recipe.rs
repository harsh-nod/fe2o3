//! Maps checked original phase bodies without equating raw/canonical IDs.
//! This module constructs an inert recipe, never the live protocol receipt.
use super::{
    completion,
    definitions::{self, BodyRecipe, Definition, Types},
};
use crate::collector::production_importer_v1::{
    ProductionSemanticImportErrorV1, numerical_policy_v1::defined_body_v1::DefinedSourceRosterV1,
    rust_exact_reviewed_adt_arguments_v1, source_body_v1,
};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::ty::{Instance, Ty, TyCtxt, TyKind, TypingEnv};

type Result<T> = std::result::Result<T, ProductionSemanticImportErrorV1>;
fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}
fn spend(work: &mut usize, n: usize) -> Result<()> {
    *work = work
        .checked_sub(n)
        .ok_or_else(|| rejected("phase canonical transport work ceiling"))?;
    Ok(())
}

pub(super) struct Mapper<'r, 'a, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    pub functions: &'a [SemanticFunctionDeclV1],
    pub callables: &'a [SemanticCallableDeclV1],
    pub roster: &'r DefinedSourceRosterV1<'a, 'tcx>,
}

impl<'a, 'tcx> Mapper<'_, 'a, 'tcx> {
    pub(super) fn function(
        &self,
        instance: Instance<'tcx>,
        work: &mut usize,
    ) -> Result<SemanticFunctionIdV1> {
        let mut found = None;
        for (index, producer) in self.plan.function_producers().iter().enumerate() {
            spend(work, 1)?;
            if producer.instance == instance
                && found
                    .replace(SemanticFunctionIdV1::from_index(index as u32))
                    .is_some()
            {
                return Err(rejected("phase duplicate original function instance"));
            }
        }
        found.ok_or_else(|| rejected("phase original function outside retained roster"))
    }

    pub(super) fn callable(
        &self,
        instance: Instance<'tcx>,
        work: &mut usize,
    ) -> Result<SemanticPhaseCallableV1> {
        let mut found = None;
        for (index, producer) in self.plan.function_producers().iter().enumerate() {
            spend(work, 1)?;
            if producer.instance != instance {
                continue;
            }
            let function = self
                .functions
                .get(index)
                .ok_or_else(|| rejected("phase callable function"))?;
            if found
                .replace(SemanticPhaseCallableV1 {
                    callable: SemanticCallableIdV1::from_index(index as u32),
                    identity: function.identity(),
                    abi: function.abi().identity(),
                })
                .is_some()
            {
                return Err(rejected("phase duplicate original callable"));
            }
        }
        for (index, producer) in self.plan.terminal_producers().iter().enumerate() {
            spend(work, 1)?;
            if producer.instance != instance {
                continue;
            }
            let callable = SemanticCallableIdV1::from_index((self.functions.len() + index) as u32);
            let binding = self
                .callables
                .get(callable.index() as usize)
                .and_then(SemanticCallableDeclV1::binding)
                .ok_or_else(|| rejected("phase terminal binding absent"))?;
            if found
                .replace(SemanticPhaseCallableV1 {
                    callable,
                    identity: binding.identity(),
                    abi: binding.abi().identity(),
                })
                .is_some()
            {
                return Err(rejected("phase defined/terminal callable collision"));
            }
        }
        found.ok_or_else(|| rejected("phase callable outside live source roster"))
    }

    fn types<const N: usize>(
        &self,
        types: [Ty<'tcx>; N],
        work: &mut usize,
    ) -> Result<[SemanticTypeIdV1; N]> {
        spend(
            work,
            self.plan
                .type_producers()
                .len()
                .checked_mul(N)
                .ok_or_else(|| rejected("phase type mapping overflow"))?,
        )?;
        self.roster.types(types)
    }

    fn reference(
        &self,
        ty: Ty<'tcx>,
        pointee: Ty<'tcx>,
        kind: SemanticPhaseReferenceKindV1,
        work: &mut usize,
    ) -> Result<SemanticPhaseReferenceV1> {
        let [reference, pointee] = self.types([ty, pointee], work)?;
        Ok(SemanticPhaseReferenceV1 {
            reference,
            pointee,
            kind,
        })
    }

    fn brands(
        &self,
        phase: definitions::PhaseTypes<'tcx>,
        outer: Ty<'tcx>,
        work: &mut usize,
    ) -> Result<SemanticPhaseBrandsV1> {
        let [
            root_brand,
            outer_workgroup_brand,
            phase_brand,
            dynamic_epoch,
        ] = self.types([phase.root, outer, phase.brand, phase.dynamic_epoch], work)?;
        Ok(SemanticPhaseBrandsV1 {
            root_brand,
            outer_workgroup_brand,
            phase_brand,
            dynamic_epoch,
        })
    }

    pub(super) fn recipe(
        &self,
        definition: &Definition<'tcx>,
        replay: &mut source_body_v1::Replay<'a, 'tcx>,
        work: &mut usize,
    ) -> Result<SemanticDefinedReusablePhaseRecipeV1> {
        use SemanticDefinedReusablePhaseRecipeV1 as R;
        use SemanticPhaseReferenceKindV1::{Shared, Unique};
        let function = self.function(definition.instance, work)?;
        let mapping = self.roster.reconstructed_body(function, replay, work)?;
        Ok(match definition.types {
            Types::OwnerConvert {
                workgroup,
                owner,
                root,
                epoch,
            } => {
                let outer = outer_brand(self.tcx, field(self.tcx, owner, 2)?)?;
                let [
                    workgroup,
                    owner,
                    root_brand,
                    outer_workgroup_brand,
                    input_epoch,
                ] = self.types([workgroup, owner, root, outer, epoch], work)?;
                R::OwnerConvert {
                    workgroup,
                    owner,
                    root_brand,
                    outer_workgroup_brand,
                    input_epoch,
                }
            }
            Types::Issue {
                owner_reference,
                owner,
                phase,
            } => {
                let reference = self.reference(owner_reference, owner, Unique, work)?;
                let brands = self.brands(
                    phase,
                    outer_brand(self.tcx, field(self.tcx, owner, 2)?)?,
                    work,
                )?;
                let [owner, phase_workgroup] = self.types([owner, phase.workgroup], work)?;
                R::Issue {
                    owner_reference: reference,
                    owner,
                    phase_workgroup,
                    brands,
                }
            }
            Types::Bind {
                phase_reference,
                storage_reference,
                storage,
                phase,
                lease,
                element,
                elements,
            } => {
                let phase_reference =
                    self.reference(phase_reference, phase.workgroup, Shared, work)?;
                let storage_reference = self.reference(storage_reference, storage, Unique, work)?;
                let brands = self.brands(
                    phase,
                    outer_brand(self.tcx, field(self.tcx, storage, 1)?)?,
                    work,
                )?;
                let [
                    phase_workgroup,
                    reusable_storage,
                    phase_lds,
                    element,
                    uninitialized_marker,
                    storage_marker,
                    thread_marker,
                ] = self.types(
                    [
                        phase.workgroup,
                        storage,
                        lease,
                        element,
                        field(self.tcx, lease, 1)?,
                        field(self.tcx, storage, 0)?,
                        field(self.tcx, storage, 2)?,
                    ],
                    work,
                )?;
                R::Bind {
                    phase_reference,
                    storage_reference,
                    phase_workgroup,
                    reusable_storage,
                    phase_lds,
                    element,
                    uninitialized_marker,
                    storage_marker,
                    thread_marker,
                    brands,
                    elements,
                }
            }
            Types::Finish {
                phase,
                advanced,
                completion,
            } => {
                let BodyRecipe::Finish {
                    barrier,
                    barrier_block,
                    return_block,
                    ..
                } = definition.recipe
                else {
                    return Err(rejected("phase Finish original recipe changed"));
                };
                let brands = self.brands(
                    phase,
                    outer_brand(self.tcx, field(self.tcx, completion, 1)?)?,
                    work,
                )?;
                let epoch = rust_exact_reviewed_adt_arguments_v1(
                    self.tcx,
                    advanced,
                    "fe2o3_device::execution::WorkgroupCapability",
                )
                .and_then(|args| args.get(2))
                .and_then(|a| a.as_type())
                .ok_or_else(|| rejected("phase advanced epoch"))?;
                let [
                    workgroup_before_barrier,
                    workgroup_after_barrier,
                    completion,
                    input_epoch,
                    advanced_epoch,
                ] = self.types(
                    [phase.workgroup, advanced, completion, phase.epoch, epoch],
                    work,
                )?;
                R::Finish {
                    workgroup_before_barrier,
                    workgroup_after_barrier,
                    completion,
                    brands,
                    input_epoch,
                    advanced_epoch,
                    barrier: self.callable(barrier, work)?,
                    barrier_block: mapping.block(barrier_block.as_u32())?,
                    return_block: mapping.block(return_block.as_u32())?,
                }
            }
            Types::WithPhase {
                owner_reference,
                owner,
                closure,
                result,
                phase,
                call_tuple,
                result_pair,
                completion,
            } => {
                let BodyRecipe::WithPhase {
                    issue,
                    invoke,
                    drop,
                    issue_block,
                    invoke_block,
                    drop_block,
                    drop_result,
                    ..
                } = definition.recipe
                else {
                    return Err(rejected("phase wrapper original recipe changed"));
                };
                let original = completion::observe(self.tcx, definition, work)
                    .map_err(|_| rejected("phase live completion relay rejected"))?;
                let closure_function = self.function(original.closure, work)?;
                let closure_mapping =
                    self.roster
                        .reconstructed_body(closure_function, replay, work)?;
                let closure_body = &self.functions[closure_function.index() as usize];
                let pack_block = closure_mapping.block(original.pack_block.as_u32())?;
                let statements = closure_body.blocks()[pack_block.index() as usize].statements();
                // Non-entry blocks receive no RustCall entry projection prefix.
                // Replay compared the retained scalar reads and final tuple pack.
                if statements.len() != original.pack_statement as usize + 1 {
                    return Err(rejected("phase closure pack source statement mapping"));
                }
                let SemanticStatementKindV1::Assign(pack) = statements[original.pack_statement as usize].kind() else {
                    return Err(rejected("phase closure pack assignment"));
                };
                let SemanticRvalueKindV1::Aggregate(aggregate) = pack.value().kind() else {
                    return Err(rejected("phase closure pack aggregate"));
                };
                let operand = aggregate
                    .operands()
                    .first()
                    .ok_or_else(|| rejected("phase closure completion field"))?;
                let completion_field = match original.field {
                    completion::Field::RetainedResult => {
                        SemanticPhaseCompletionFieldV1::RetainedFinishResult
                    }
                    completion::Field::ErasedConstant => {
                        SemanticPhaseCompletionFieldV1::ErasedZstConstant {
                            canonical_operand: operand_hash(operand, work)?,
                        }
                    }
                };
                let mapped_drop = mapping.block(drop_block.as_u32())?;
                let SemanticTerminatorKindV1::Call(drop_call) = self.functions
                    [function.index() as usize]
                    .blocks()[mapped_drop.index() as usize]
                    .terminator()
                    .kind()
                else {
                    return Err(rejected("phase wrapper mapped drop call"));
                };
                let drop_operand = drop_call
                    .arguments()
                    .first()
                    .ok_or_else(|| rejected("phase wrapper drop argument"))?;
                let wrapper_drop = match drop_operand {
                    SemanticOperandV1::Move(_) => SemanticPhaseCompletionDropV1::RetainedPairField,
                    SemanticOperandV1::Constant(_) => {
                        SemanticPhaseCompletionDropV1::ErasedZstConstant {
                            canonical_operand: operand_hash(drop_operand, work)?,
                        }
                    }
                    _ => {
                        return Err(rejected(
                            "phase wrapper drop is not its original move/erasure",
                        ));
                    }
                };
                let (hash, bytes) =
                    canonical_semantic_source_body_sha256_v25(closure_body, (*work / 2) as u64)
                        .map_err(|_| rejected("phase closure fixed V25 source fragment"))?;
                spend(
                    work,
                    bytes
                        .checked_mul(2)
                        .ok_or_else(|| rejected("phase closure fragment work overflow"))?,
                )?;
                let finish_call_block = closure_mapping.block(original.finish_block.as_u32())?;
                let relay = SemanticPhaseCompletionRelayV1 {
                    closure_function,
                    closure_body: hash,
                    finish: self.callable(original.finish, work)?,
                    finish_call_block,
                    finish_normal_target: pack_block,
                    closure_pack_block: pack_block,
                    closure_pack_statement: original.pack_statement,
                    closure_return_block: pack_block,
                    completion_field,
                    wrapper_drop,
                };
                let reference = self.reference(owner_reference, owner, Unique, work)?;
                let brands = self.brands(
                    phase,
                    outer_brand(self.tcx, field(self.tcx, owner, 2)?)?,
                    work,
                )?;
                let drop_type = definitions::normalize(
                    self.tcx,
                    definition.instance,
                    definition.body.local_decls[drop_result].ty,
                )
                .map_err(|_| rejected("phase drop result normalization"))?;
                let [
                    owner,
                    closure,
                    call_tuple,
                    phase_workgroup,
                    completion,
                    result_pair,
                    result,
                    drop_result,
                ] = self.types(
                    [
                        owner,
                        closure,
                        call_tuple,
                        phase.workgroup,
                        completion,
                        result_pair,
                        result,
                        drop_type,
                    ],
                    work,
                )?;
                R::WithPhase {
                    owner_reference: reference,
                    owner,
                    closure,
                    call_tuple,
                    phase_workgroup,
                    completion,
                    result_pair,
                    result,
                    drop_result,
                    brands,
                    issue: self.callable(issue, work)?,
                    invoke: self.callable(invoke, work)?,
                    drop_completion: self.callable(drop, work)?,
                    issue_block: mapping.block(issue_block.as_u32())?,
                    invoke_block: mapping.block(invoke_block.as_u32())?,
                    drop_block: mapped_drop,
                    relay,
                }
            }
        })
    }
}

pub(super) fn field<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, index: usize) -> Result<Ty<'tcx>> {
    let TyKind::Adt(definition, args) = *ty.kind() else {
        return Err(rejected("phase marker field requires original ADT"));
    };
    if !definition.is_struct() {
        return Err(rejected("phase marker field requires original struct"));
    }
    let field = definition
        .non_enum_variant()
        .fields
        .get(rustc_abi::FieldIdx::from_usize(index))
        .ok_or_else(|| rejected("phase original marker field absent"))?;
    tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), field.ty(tcx, args))
        .map_err(|_| rejected("phase original marker field normalization"))
}

pub(super) fn outer_brand<'tcx>(tcx: TyCtxt<'tcx>, marker: Ty<'tcx>) -> Result<Ty<'tcx>> {
    let TyKind::Adt(phantom, args) = *marker.kind() else {
        return Err(rejected("phase outer brand phantom"));
    };
    if Some(phantom.did()) != tcx.lang_items().phantom_data() || args.len() != 1 {
        return Err(rejected("phase outer brand exact phantom"));
    }
    let TyKind::FnPtr(signature, _) = args[0]
        .as_type()
        .ok_or_else(|| rejected("phase outer brand type"))?
        .kind()
    else {
        return Err(rejected("phase outer brand invariant"));
    };
    let args = signature.skip_binder().inputs_and_output;
    let [input, output] = args.as_slice() else {
        return Err(rejected("phase outer brand invariant arity"));
    };
    if input != output
        || rust_exact_reviewed_adt_arguments_v1(
            tcx,
            *input,
            "fe2o3_device::execution::WorkgroupBrand",
        )
        .is_none()
    {
        return Err(rejected("phase outer brand exact nominal identity"));
    }
    Ok(*input)
}

fn operand_hash(operand: &SemanticOperandV1, work: &mut usize) -> Result<[u8; 32]> {
    let before = *work;
    let mut remaining = before as u64;
    let result = canonical_reusable_phase_operand_sha256_v25(operand, &mut remaining)
        .map_err(|_| rejected("phase erased operand fixed V25 commitment"));
    *work = usize::try_from(remaining).map_err(|_| rejected("phase operand work conversion"))?;
    result
}
