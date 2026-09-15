//! Closed original definition predicate. No source/provider authentication here.
use rustc_hir::{Mutability, Safety, def_id::DefId};
use rustc_middle::{
    mir::{self, Body, TerminatorKind},
    ty::{
        self, EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeFoldable,
        TypeVisitableExt, TypingEnv,
    },
};
use rustc_target::callconv::PassMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Types<'tcx> {
    pub input: Ty<'tcx>,
    pub output: Ty<'tcx>,
    pub element: Ty<'tcx>,
    pub brand: Ty<'tcx>,
    pub epoch: Ty<'tcx>,
    pub elements: u64,
    pub storage: Ty<'tcx>,
    pub state: Ty<'tcx>,
    pub workgroup: Ty<'tcx>,
    pub epoch_marker: Ty<'tcx>,
    pub thread: Ty<'tcx>,
    pub element_size: u64,
    pub element_align: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Nominal {
    pub input: DefId,
    pub output: DefId,
    pub uninitialized: DefId,
    pub workgroup_brand: DefId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Instance,
    Signature,
    Nominal,
    Fields,
    Layout,
    Abi,
    Body,
}

pub(super) fn normalize<'tcx, T: TypeFoldable<TyCtxt<'tcx>>>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    value: T,
) -> Option<T> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(value),
        )
        .ok()
}

fn fields<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, count: usize) -> Option<Vec<Ty<'tcx>>> {
    let TyKind::Adt(def, args) = *ty.kind() else {
        return None;
    };
    if !def.is_struct() || def.non_enum_variant().fields.len() != count {
        return None;
    }
    def.non_enum_variant()
        .fields
        .iter()
        .map(|f| {
            tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), f.ty(tcx, args))
                .ok()
        })
        .collect()
}

fn phantom<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(def, args) = *ty.kind() else {
        return None;
    };
    (Some(def.did()) == tcx.lang_items().phantom_data() && args.len() == 1)
        .then(|| args[0].as_type())
        .flatten()
}

fn invariant<'tcx>(tcx: TyCtxt<'tcx>, marker: Ty<'tcx>, value: Ty<'tcx>) -> bool {
    phantom(tcx, marker).is_some_and(|ty| {
        matches!(ty.kind(), TyKind::FnPtr(sig, header)
        if sig.bound_vars().is_empty() && header.safety == Safety::Safe
            && header.abi == rustc_abi::ExternAbi::Rust && !header.c_variadic
            && sig.skip_binder().inputs_and_output.as_slice() == [value, value])
    })
}

fn zst<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    tcx.layout_of(TypingEnv::fully_monomorphized().as_query_input(ty))
        .is_ok_and(|layout| {
            layout.size.bytes() == 0
                && layout.align.abi.bytes() == 1
                && !layout.uninhabited
                && layout.backend_repr == (rustc_abi::BackendRepr::Memory { sized: true })
        })
}

pub(super) fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    nominal: Nominal,
) -> Result<Types<'tcx>, Error> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || instance.args.len() != 5
        || instance.args.len() != tcx.generics_of(instance.def_id()).count()
        || !tcx.is_mir_available(instance.def_id())
    {
        return Err(Error::Instance);
    }
    let sig = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let sig = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), sig)
        .map_err(|_| Error::Signature)?;
    if sig.safety != Safety::Safe
        || sig.abi != rustc_abi::ExternAbi::Rust
        || sig.c_variadic
        || sig.has_non_region_param()
        || sig.has_infer()
        || sig.has_aliases()
        || sig.has_escaping_bound_vars()
    {
        return Err(Error::Signature);
    }
    let [input] = sig.inputs() else {
        return Err(Error::Signature);
    };
    let output = sig.output();
    let TyKind::Adt(input_def, input_args) = *input.kind() else {
        return Err(Error::Nominal);
    };
    let TyKind::Adt(output_def, output_args) = *output.kind() else {
        return Err(Error::Nominal);
    };
    if input_def.did() != nominal.input
        || output_def.did() != nominal.output
        || input_def == output_def
        || input_args.len() != 6
        || output_args.len() != 4
        || input_args[0].as_region().is_none()
        || output_args[0] != input_args[0]
        || output_args[1] != input_args[1]
        || output_args[2] != input_args[2]
        || output_args[3] != input_args[4]
        || instance.args.as_slice()
            != [
                input_args[0],
                input_args[1],
                input_args[2],
                input_args[4],
                input_args[5],
            ]
    {
        return Err(Error::Nominal);
    }
    let element = input_args[1].as_type().ok_or(Error::Nominal)?;
    let elements = input_args[2]
        .as_const()
        .and_then(|c| c.try_to_target_usize(tcx))
        .ok_or(Error::Nominal)?;
    let state = input_args[3].as_type().ok_or(Error::Nominal)?;
    let brand = input_args[4].as_type().ok_or(Error::Nominal)?;
    let epoch = input_args[5].as_type().ok_or(Error::Nominal)?;
    if elements == 0
        || !matches!(state.kind(), TyKind::Adt(d, args) if d.did() == nominal.uninitialized && args.is_empty())
    {
        return Err(Error::Nominal);
    }
    let input_fields = fields(tcx, *input, 5).ok_or(Error::Fields)?;
    let output_fields = fields(tcx, output, 3).ok_or(Error::Fields)?;
    if output_fields != [input_fields[0], input_fields[2], input_fields[4]]
        || !invariant(tcx, input_fields[1], state)
        || !invariant(tcx, input_fields[3], epoch)
    {
        return Err(Error::Fields);
    }
    let storage = phantom(tcx, input_fields[0]).ok_or(Error::Fields)?;
    let TyKind::Ref(_, array, Mutability::Mut) = *storage.kind() else {
        return Err(Error::Fields);
    };
    if !matches!(array.kind(), TyKind::Array(t, n) if *t == element && n.try_to_target_usize(tcx) == Some(elements))
    {
        return Err(Error::Fields);
    }
    let marker = phantom(tcx, input_fields[2]).ok_or(Error::Fields)?;
    let TyKind::FnPtr(signature, header) = *marker.kind() else {
        return Err(Error::Fields);
    };
    if !signature.bound_vars().is_empty()
        || signature.skip_binder().inputs_and_output.len() != 2
        || header.safety != Safety::Safe
        || header.abi != rustc_abi::ExternAbi::Rust
        || header.c_variadic
    {
        return Err(Error::Fields);
    }
    let wg = signature.skip_binder().inputs_and_output[0];
    if !invariant(tcx, input_fields[2], wg)
        || !matches!(wg.kind(), TyKind::Adt(d, args) if d.did() == nominal.workgroup_brand
            && args.len() == 2 && args[0] == input_args[0] && args[1].as_type() == Some(brand))
        || !phantom(tcx, input_fields[4]).is_some_and(|ty|
            matches!(ty.kind(), TyKind::RawPtr(unit, Mutability::Mut) if *unit == tcx.types.unit))
    { return Err(Error::Fields) }
    let layout = tcx
        .layout_of(TypingEnv::fully_monomorphized().as_query_input(element))
        .map_err(|_| Error::Layout)?;
    if layout.uninhabited
        || layout.size.bytes() == 0
        || layout.size.bytes().checked_mul(elements).is_none()
        || !zst(tcx, *input)
        || !zst(tcx, output)
        || input_fields.iter().any(|t| !zst(tcx, *t))
    {
        return Err(Error::Layout);
    }
    let abi = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
        )
        .map_err(|_| Error::Abi)?;
    if abi.can_unwind
        || abi.args.len() != 1
        || !matches!(abi.args[0].mode, PassMode::Ignore)
        || !matches!(abi.ret.mode, PassMode::Ignore)
    {
        return Err(Error::Abi);
    }
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != mir::MirPhase::Runtime(mir::RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || !body.user_type_annotations.is_empty()
        || body.arg_count != 1
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1
        || normalize(tcx, instance, body.return_ty()) != Some(output)
        || normalize(
            tcx,
            instance,
            body.local_decls[mir::Local::from_usize(1)].ty,
        ) != Some(*input)
        || body.basic_blocks[mir::START_BLOCK].is_cleanup
        || !body.basic_blocks[mir::START_BLOCK].statements.is_empty()
        || !body.basic_blocks[mir::START_BLOCK]
            .terminator
            .as_ref()
            .is_some_and(|t| matches!(t.kind, TerminatorKind::Return))
    {
        return Err(Error::Body);
    }
    Ok(Types {
        input: *input,
        output,
        element,
        brand,
        epoch,
        elements,
        storage: input_fields[0],
        state: input_fields[1],
        workgroup: input_fields[2],
        epoch_marker: input_fields[3],
        thread: input_fields[4],
        element_size: layout.size.bytes(),
        element_align: layout.align.abi.bytes(),
    })
}
