//! Prepaid metadata owns signatures; opaque frames borrow the input packet.
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1;

pub(super) struct Packet<'a> {
    pub(super) semantic: &'a [u8],
    pub(super) native: &'a [u8],
    pub(super) order: Vec<u32>,
    pub(super) roots: Vec<Root<'a>>,
}
pub(super) struct Root<'a> {
    semantic_root: u32,
    launch_rank: u8,
    launch: Launch<'a>,
    frames: [&'a [u8]; 4],
    text: &'a str,
    commitments: Vec<Staging>,
    signatures: Vec<Signature>,
    formula: Signature,
}
impl Root<'_> {
    pub(super) fn view(&self) -> NativeConditionalSourceRootV2<'_> {
        NativeConditionalSourceRootV2 {
            semantic_root: self.semantic_root,
            launch_rank: self.launch_rank,
            launch: self.launch,
            induction_bytes: self.frames[0],
            recipe_bytes: self.frames[1],
            source_rows_bytes: self.frames[2],
            ranked_ir: self.text,
            cpu_input_bytes: self.frames[3],
            staging_commitments: &self.commitments,
            effect_receipts: &self.signatures,
            formula_receipt: &self.formula,
        }
    }
}

pub(super) fn decode<'a>(bytes: &'a [u8], scope: &mut Scope<'_, '_>) -> Result<Packet<'a>, E> {
    require(bytes.len() <= MAX_BYTES, "aggregate packet limit")?;
    let mut r = Reader { bytes, scope };
    require(r.take(8)? == MAGIC, "magic")?;
    require(r.array::<2>()? == VERSION.to_le_bytes(), "version")?;
    require(r.array::<2>()? == [0; 2], "flags")?;
    require(r.u32()? as usize == bytes.len(), "total length")?;
    require(r.array::<1>()? == [ROUTE], "route")?;
    r.scope
        .reserve(add(size_of::<Packet<'_>>(), size_of::<Vec<u8>>())?)?;
    let semantic = r.blob()?;
    let native = r.blob()?;
    let count = r.count(MAX_ROOTS, 4)?;
    require(count > 0, "empty order")?;
    let mut order = r.scope.vector(count)?;
    for _ in 0..count {
        order.push(r.u32()?);
    }
    let count = r.count(MAX_ROOTS, MIN_ROOT_BYTES)?;
    check_order(&order, count, r.scope)?;
    let mut roots = r.scope.vector(count)?;
    for _ in 0..count {
        let semantic_root = r.u32()?;
        let launch_rank = r.array::<1>()?[0];
        let name = r.text()?;
        let binding = r.array()?;
        let rank = r.array::<1>()?[0];
        let workgroup = match r.array::<1>()?[0] {
            0 => None,
            1 => Some(r.dimensions()?),
            _ => return Err(E::Invalid("workgroup option")),
        };
        let grid = r.dimensions()?;
        let launch = Launch::new(
            name,
            binding,
            ProductionSourceLaunchInputV1::new(rank, workgroup, grid),
        );
        let induction = r.blob()?;
        let recipe = r.blob()?;
        let source_rows = r.blob()?;
        let text = r.text()?;
        let cpu = r.blob()?;
        let count = r.count(MAX_BYTES / EFFECT_BYTES, EFFECT_BYTES)?;
        require(count > 0, "empty staging")?;
        let mut commitments = r.scope.vector(count)?;
        let mut signatures = r.scope.vector(count)?;
        for _ in 0..count {
            commitments.push(Staging {
                receipt: r.array()?,
                effect: r.array()?,
                signer: r.array()?,
                execution: r.array()?,
                toolchain: [r.array()?, r.array()?, r.array()?, r.array()?, r.array()?],
            });
            signatures.push(r.signature()?);
        }
        require(
            r.u32()? as usize == EVIDENCE_BYTES,
            "formula evidence length",
        )?;
        require(r.array::<2>()? == VERSION.to_le_bytes(), "formula version")?;
        require(r.array::<2>()? == [0; 2], "formula flags")?;
        let formula = r.signature()?;
        roots.push(Root {
            semantic_root,
            launch_rank,
            launch,
            frames: [induction, recipe, source_rows, cpu],
            text,
            commitments,
            signatures,
            formula,
        });
    }
    require(r.bytes.is_empty(), "trailing bytes")?;
    Ok(Packet {
        semantic,
        native,
        order,
        roots,
    })
}

struct Reader<'a, 's, 'b, 'w> {
    bytes: &'a [u8],
    scope: &'s mut Scope<'b, 'w>,
}
impl<'a> Reader<'a, '_, '_, '_> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], E> {
        let (bytes, tail) = self
            .bytes
            .split_at_checked(count)
            .ok_or(E::Invalid("truncated packet"))?;
        self.scope.budget.charge_work(add(count, 1)?)?;
        self.bytes = tail;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], E> {
        self.take(N)?
            .try_into()
            .map_err(|_| Resource::Accounting.into())
    }
    fn u32(&mut self) -> Result<u32, E> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn blob(&mut self) -> Result<&'a [u8], E> {
        let count = self.u32()? as usize;
        self.take(count)
    }
    fn text(&mut self) -> Result<&'a str, E> {
        let bytes = self.blob()?;
        self.scope.budget.charge_work(bytes.len())?;
        std::str::from_utf8(bytes).map_err(|_| E::Invalid("UTF-8"))
    }
    fn count(&mut self, maximum: usize, minimum: usize) -> Result<usize, E> {
        let count = self.u32()? as usize;
        require(
            count <= maximum && count <= self.bytes.len() / minimum,
            "count/remaining bytes",
        )?;
        Ok(count)
    }
    fn dimensions(&mut self) -> Result<[u32; 3], E> {
        Ok([self.u32()?, self.u32()?, self.u32()?])
    }
    fn signature(&mut self) -> Result<Signature, E> {
        Ok(Signature::from_untrusted_parts(
            self.array()?,
            self.array()?,
        ))
    }
}
