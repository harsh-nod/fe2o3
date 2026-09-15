//! Exact logical types of an authenticated phase definition. Preflight feeds
//! these back through its ordinary inspect_type; no generic phantom traversal.
use super::{
    canonical_recipe::{field, outer_brand},
    definitions::{self, Definition, Types},
    source_calls::spend,
};
use crate::collector::production_importer_v1::ProductionSemanticImportErrorV1;
use rustc_middle::ty::{Instance, Ty, TyCtxt};

type Result<T> = std::result::Result<T, ProductionSemanticImportErrorV1>;
fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}

/// Fixed stack storage, not a logical claim about a Vec's requested capacity.
pub(super) struct Dependencies<'tcx> {
    values: [Option<Ty<'tcx>>; 32],
    len: usize,
}

impl<'tcx> Dependencies<'tcx> {
    pub(super) fn into_values(self) -> [Option<Ty<'tcx>>; 32] {
        self.values
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = Ty<'tcx>> + '_ {
        self.values[..self.len].iter().filter_map(|value| *value)
    }

    fn push(&mut self, ty: Ty<'tcx>, work: &mut usize) -> Result<()> {
        for prior in &self.values[..self.len] {
            spend(work, 1)?;
            if *prior == Some(ty) {
                return Ok(());
            }
        }
        spend(work, 1)?;
        let slot = self
            .values
            .get_mut(self.len)
            .ok_or_else(|| rejected("phase exact dependency roster ceiling"))?;
        *slot = Some(ty);
        self.len += 1;
        Ok(())
    }
}

pub(super) fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    work: &mut usize,
) -> Result<Option<Dependencies<'tcx>>> {
    let Some(role) = definitions::classify(tcx, instance, work)
        .map_err(|_| rejected("phase dependency definition work"))?
    else {
        return Ok(None);
    };
    let definition = Definition::observe(tcx, instance, role, work)
        .map_err(|_| rejected("phase dependency original definition rejected"))?;
    collect(tcx, &definition, work).map(Some)
}

pub(super) fn collect<'tcx>(
    tcx: TyCtxt<'tcx>,
    definition: &Definition<'tcx>,
    work: &mut usize,
) -> Result<Dependencies<'tcx>> {
    spend(
        work,
        std::mem::size_of::<Dependencies<'tcx>>() / std::mem::size_of::<usize>(),
    )?;
    let mut result = Dependencies {
        values: [None; 32],
        len: 0,
    };
    let mut add = |ty| result.push(ty, work);
    // ABI locals are already inspected. Repeating them is harmless and keeps
    // this callback's closed dependency roster usable by replay as well.
    match definition.types {
        Types::OwnerConvert {
            workgroup,
            owner,
            root,
            epoch,
        } => {
            for ty in [
                workgroup,
                owner,
                root,
                epoch,
                outer_brand(tcx, field(tcx, owner, 2)?)?,
            ] {
                add(ty)?;
            }
        }
        Types::Issue {
            owner_reference,
            owner,
            phase,
        } => {
            for ty in [
                owner_reference,
                owner,
                phase.workgroup,
                phase.root,
                phase.brand,
                phase.epoch,
                phase.dynamic_epoch,
                outer_brand(tcx, field(tcx, owner, 2)?)?,
            ] {
                add(ty)?;
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
            for ty in [
                owner_reference,
                owner,
                closure,
                result,
                call_tuple,
                result_pair,
                completion,
                phase.workgroup,
                phase.root,
                phase.brand,
                phase.epoch,
                phase.dynamic_epoch,
                outer_brand(tcx, field(tcx, owner, 2)?)?,
            ] {
                add(ty)?;
            }
        }
        Types::Bind {
            phase_reference,
            storage_reference,
            storage,
            phase,
            lease,
            element,
            ..
        } => {
            for ty in [
                phase_reference,
                storage_reference,
                storage,
                lease,
                element,
                phase.workgroup,
                phase.root,
                phase.brand,
                phase.epoch,
                phase.dynamic_epoch,
                field(tcx, lease, 1)?,
                field(tcx, storage, 0)?,
                field(tcx, storage, 2)?,
                outer_brand(tcx, field(tcx, storage, 1)?)?,
            ] {
                add(ty)?;
            }
        }
        Types::Finish {
            phase,
            advanced,
            completion,
        } => {
            for ty in [
                phase.workgroup,
                advanced,
                completion,
                phase.root,
                phase.brand,
                phase.epoch,
                phase.dynamic_epoch,
                outer_brand(tcx, field(tcx, completion, 1)?)?,
            ] {
                add(ty)?;
            }
            let args =
                crate::collector::production_importer_v1::rust_exact_reviewed_adt_arguments_v1(
                    tcx,
                    advanced,
                    "fe2o3_device::execution::WorkgroupCapability",
                )
                .ok_or_else(|| rejected("phase dependency advanced Workgroup"))?;
            add(args
                .get(2)
                .and_then(|arg| arg.as_type())
                .ok_or_else(|| rejected("phase dependency advanced epoch"))?)?;
        }
    }
    Ok(result)
}
