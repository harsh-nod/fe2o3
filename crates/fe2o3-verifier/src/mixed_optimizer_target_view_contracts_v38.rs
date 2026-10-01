//! Physical tag contracts from one exact retained canonical inventory. This is
//! not source validity: in particular, a source NonZero scalar may have stricter
//! valid bits than its physical integer storage row. The source classifier must
//! establish that independently before a paired variant operation is admitted.
use super::pointer_byte_operations_v30::scalar_bytes;
use super::{Error, Inventory, Resource, Result, Writer};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, FormalIndexWidth, ScalarType,
    StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1 as Layout,
    StorageVariantEncodingV1 as Encoding, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{cell::Cell, fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TargetByteTagClassV38 {
    NotVariant,
    DirectScalar,
    /// Physical scalar decoding only; original source validity is still required.
    ScalarNiche,
    /// Nominal pointer tokens cannot be inspected as arbitrary numerical bytes.
    UnsupportedPointerNiche,
}

/// Borrow once per exact inventory, not once per function. There is no copied
/// row archive or caller-provided contract table. Row IDs remain owner-scoped.
pub(super) struct TargetByteViewContractsV38<'a, 'owner> {
    inventory: &'a Inventory<'owner>,
    width: FormalIndexWidth,
    required: usize,
    slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
}

pub(super) fn headers() -> usize {
    size_of::<TargetByteViewContractsV38<'_, '_>>()
        + 2 * size_of::<Result<TargetByteViewContractsV38<'_, '_>>>()
        + 2 * size_of::<Result<TargetByteTagClassV38>>()
        + 2 * size_of::<Result<()>>()
        + 2 * size_of::<std::result::Result<(), Resource>>()
        + 2 * size_of::<TargetByteTagClassV38>()
        + size_of::<(
            [&Inventory<'_>; 2],
            [&Layout; 4],
            &mut Writer<'_, '_>,
            &[Layout],
            std::slice::Iter<'static, Layout>,
            std::iter::Enumerate<std::slice::Iter<'static, Layout>>,
            std::slice::Iter<'static, fe2o3_kernel_ir::StorageVariantV1>,
            [usize; 12],
            [u128; 2],
            [&str; 2],
            Encoding,
            FormalIndexWidth,
        )>()
}

fn classify(
    rows: &[Layout],
    row: &Layout,
    width: FormalIndexWidth,
    out: &mut Writer<'_, '_>,
) -> Result<TargetByteTagClassV38> {
    out.budget.charge_work(4)?;
    let Kind::Variants { encoding, .. } = &row.kind else {
        return Ok(TargetByteTagClassV38::NotVariant);
    };
    let tag = rows
        .get(encoding.tag().layout.0 as usize)
        .ok_or(Error::Statement("target tag layout owner"))?;
    match (&tag.kind, encoding) {
        (Kind::Pointer(_), Encoding::Niche { .. }) => {
            Ok(TargetByteTagClassV38::UnsupportedPointerNiche)
        }
        (Kind::Scalar(scalar), _) => {
            if tag.size != scalar_bytes(*scalar, width)? as u64
                || !(scalar.is_integer()
                    || (*scalar == ScalarType::Bool && matches!(encoding, Encoding::Niche { .. })))
            {
                return Err(Error::Statement("target tag scalar width or kind"));
            }
            Ok(match encoding {
                Encoding::Direct { .. } => TargetByteTagClassV38::DirectScalar,
                Encoding::Niche { .. } => TargetByteTagClassV38::ScalarNiche,
            })
        }
        _ => Err(Error::Statement("target tag representation is not modeled")),
    }
}

impl<'a, 'owner> TargetByteViewContractsV38<'a, 'owner> {
    pub(super) fn derive(
        inventory: &'a Inventory<'owner>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(3)?;
        scalar_bytes(ScalarType::Index, width)?;
        let rows = &inventory.owner().module().storage_layouts;
        for row in rows {
            out.budget.charge_work(2)?;
            classify(rows, row, width, out)?;
            if let Kind::Variants { variants, .. } = &row.kind {
                for variant in variants {
                    out.budget.charge_work(2)?;
                    if rows.get(variant.layout.0 as usize).is_none() {
                        return Err(Error::Statement("target variant payload layout owner"));
                    }
                }
            }
        }
        Ok(Self {
            inventory,
            width,
            required: out.budget.storage(),
            slot: std::ptr::from_ref(&*out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
        })
    }

    fn retain<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(Error::Resource(error)) = &result {
            self.failure.set(Some(*error));
        }
        result
    }

    pub(super) fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if self.slot != std::ptr::from_ref(&*out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
        {
            self.failure.set(Some(Resource::Accounting));
            return Err(Resource::Accounting.into());
        }
        self.retain(out.budget.charge_work(1).map_err(Error::from))?;
        if !std::ptr::eq(owner, self.inventory.owner()) {
            return Err(Error::Statement("target tag contract owner differs"));
        }
        Ok(())
    }

    pub(super) fn row_class(
        &self,
        id: Id,
        out: &mut Writer<'_, '_>,
    ) -> Result<TargetByteTagClassV38> {
        self.check_owner(self.inventory.owner(), out)?;
        let rows = &self.inventory.owner().module().storage_layouts;
        let row = rows
            .get(id.0 as usize)
            .ok_or(Error::Statement("target tag row ordinal"))?;
        self.retain(classify(rows, row, self.width, out))
    }

    /// Emit one closed owner-qualified physical registry. Unsupported rows are
    /// absent, not approximated; callers must inspect row_class before admission.
    pub(super) fn emit(&self, namespace: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        let result = self.emit_checked(namespace, out);
        self.retain(result)
    }

    fn emit_checked(&self, namespace: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check_owner(self.inventory.owner(), out)?;
        emit!(
            out,
            "open spec fn byte_target_view_contracts_{namespace}_v38(little_endian: bool) -> MemoryViewContractsV38 {{\n MemoryViewContractsV38 {{ owner: {namespace}, rows: Map::empty()"
        );
        let rows = &self.inventory.owner().module().storage_layouts;
        for (id, row) in rows.iter().enumerate() {
            out.budget.charge_work(2)?;
            let class = classify(rows, row, self.width, out)?;
            if matches!(
                class,
                TargetByteTagClassV38::NotVariant | TargetByteTagClassV38::UnsupportedPointerNiche
            ) {
                continue;
            }
            let Kind::Variants { encoding, variants } = &row.kind else {
                return Err(Error::Statement("target tag class changed"));
            };
            let tag = &rows[encoding.tag().layout.0 as usize];
            emit!(
                out,
                "\n .insert({id}, MemoryTagContractV38 {{ object_bytes: {}, tag_offset: {}, tag_width: {}, little_endian, inhabited: seq![",
                row.size,
                encoding.tag().offset,
                tag.size
            );
            for variant in variants {
                out.budget.charge_work(1)?;
                emit!(out, "{},", !variant.uninhabited);
            }
            emit!(out, "], discriminants: seq![");
            for variant in variants {
                out.budget.charge_work(1)?;
                emit!(out, "{}int,", variant.discriminant);
            }
            emit!(out, "], untagged_valid_bits: seq![");
            if matches!(class, TargetByteTagClassV38::ScalarNiche) {
                let max = if matches!(tag.kind, Kind::Scalar(ScalarType::Bool)) {
                    1
                } else if tag.size == 16 {
                    u128::MAX
                } else {
                    (1u128 << (tag.size * 8)) - 1
                };
                emit!(out, "(0,{max}int)");
            }
            emit!(out, "], encoding: ");
            match encoding {
                Encoding::Direct { .. } => {
                    emit!(out, "MemoryTagEncodingV38::Direct {{ tags: seq![");
                    for variant in variants {
                        out.budget.charge_work(1)?;
                        let bits = variant
                            .direct_tag_bits
                            .ok_or(Error::Statement("target direct tag bits"))?;
                        emit!(out, "{bits}int,");
                    }
                    emit!(out, "] }}");
                }
                Encoding::Niche {
                    untagged_variant,
                    first_niche_variant,
                    last_niche_variant,
                    niche_start,
                    ..
                } => {
                    emit!(
                        out,
                        "MemoryTagEncodingV38::Niche {{ untagged: {untagged_variant}, first: {first_niche_variant}, last: {last_niche_variant}, start: {niche_start}int }}"
                    );
                }
            }
            emit!(out, " }})");
        }
        emit!(
            out,
            "\n }}\n}}\nopen spec fn byte_target_view_contracts_match_{namespace}_v38(memory: ByteMemoryV30, little_endian: bool) -> bool {{\n memory.view_contracts == byte_target_view_contracts_{namespace}_v38(little_endian)\n}}\n"
        );
        Ok(())
    }
}
