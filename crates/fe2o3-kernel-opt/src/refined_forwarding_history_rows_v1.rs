//! Closed inert row syntax. No sorting, deduplication or fabricated edge facts.
use crate::CanonicalRefinedForwardingHistoryLimitsV1 as Limits;
use crate::refined_forwarding_history_wire_v1::{
    Error, MAX_REFINED_FORWARDING_HISTORY_ROWS_V1, Meter,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardingLimits,
    CanonicalKirCrossBlockForwardingOriginV1 as Forwarding,
    CanonicalKirInductionRefinementOriginV1 as Refinement, CanonicalKirLicmHoistV1 as Hoist,
    CanonicalKirLicmOriginV1 as Licm, CanonicalKirLoadForwardingRowV1 as Load,
    CanonicalKirLoopLimitsV1 as LoopLimits, CanonicalKirLoopPreheaderV1 as Preheader,
    CanonicalKirMemorySsaLimitsV1 as MemoryLimits,
    CanonicalKirPrivateCellOriginKindV1 as PromotionKind,
    CanonicalKirPrivateCellOriginV1 as Promotion,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirOperationCoordinateV1 as Site, ControlFlowLimits as ControlLimits, ValueId,
};

pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Resource::Arithmetic)?;
        let out = self.bytes.get(self.pos..end).ok_or(Error::Rows)?;
        self.pos = end;
        Ok(out)
    }
    pub fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    pub fn word(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| Error::Rows)?,
        ))
    }
    pub fn wide(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| Error::Rows)?,
        ))
    }
    pub fn size(&mut self) -> Result<usize, Error> {
        usize::try_from(self.wide()?).map_err(|_| Resource::Arithmetic.into())
    }
    pub fn block(&mut self) -> Result<Block, Error> {
        Ok(Block {
            function: Function(self.word()?),
            block: self.word()?,
        })
    }
    pub fn site(&mut self) -> Result<Site, Error> {
        Ok(Site {
            block: self.block()?,
            operation: self.word()?,
        })
    }
    pub fn done(&self) -> Result<(), Error> {
        if self.pos == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Rows)
        }
    }
}

pub(super) struct Writer<'a> {
    pub bytes: &'a mut [u8],
    pub pos: usize,
}
impl Writer<'_> {
    pub fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .pos
            .checked_add(bytes.len())
            .ok_or(Resource::Arithmetic)?;
        self.bytes
            .get_mut(self.pos..end)
            .ok_or(Error::Rows)?
            .copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
    pub fn byte(&mut self, value: u8) -> Result<(), Error> {
        self.put(&[value])
    }
    pub fn word(&mut self, value: u32) -> Result<(), Error> {
        self.put(&value.to_le_bytes())
    }
    pub fn wide(&mut self, value: u64) -> Result<(), Error> {
        self.put(&value.to_le_bytes())
    }
    pub fn block(&mut self, value: Block) -> Result<(), Error> {
        self.word(value.function.0)?;
        self.word(value.block)
    }
    pub fn site(&mut self, value: Site) -> Result<(), Error> {
        self.block(value.block)?;
        self.word(value.operation)
    }
}

pub(super) trait Row: Sized {
    const MIN: usize;
    fn width(&self) -> usize;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error>;
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error>;
}
impl Row for Load {
    const MIN: usize = 24;
    fn width(&self) -> usize {
        24
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            first: c.site()?,
            load: c.site()?,
        })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        w.site(self.first)?;
        w.site(self.load)
    }
}
impl Row for Site {
    const MIN: usize = 12;
    fn width(&self) -> usize {
        12
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        c.site()
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        w.site(*self)
    }
}
impl Row for Promotion {
    const MIN: usize = 25;
    fn width(&self) -> usize {
        match self.kind {
            PromotionKind::Retained => 25,
            _ => 53,
        }
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        let input = c.site()?;
        let output = c.site()?;
        let kind = match c.byte()? {
            0 => PromotionKind::Retained,
            1 => PromotionKind::LoadCopy {
                allocation: c.site()?,
                previous_store: c.site()?,
                stored_value: ValueId(c.word()?),
            },
            _ => return Err(Error::Tag),
        };
        Ok(Self {
            input,
            output,
            kind,
        })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        w.site(self.input)?;
        w.site(self.output)?;
        match self.kind {
            PromotionKind::Retained => w.byte(0),
            PromotionKind::LoadCopy {
                allocation,
                previous_store,
                stored_value,
            } => {
                w.byte(1)?;
                w.site(allocation)?;
                w.site(previous_store)?;
                w.word(stored_value.0)
            }
        }
    }
}
impl Row for Preheader {
    const MIN: usize = 16;
    fn width(&self) -> usize {
        16
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            header: c.block()?,
            preheader: c.block()?,
        })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        w.block(self.header)?;
        w.block(self.preheader)
    }
}
impl Row for Licm {
    const MIN: usize = 25;
    fn width(&self) -> usize {
        if self.hoist.is_some() { 37 } else { 25 }
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        let input = c.site()?;
        let output = c.site()?;
        let hoist = match c.byte()? {
            0 => None,
            1 => Some(Hoist {
                header: c.block()?,
                sequence: c.word()?,
            }),
            _ => return Err(Error::Tag),
        };
        Ok(Self {
            input,
            output,
            hoist,
        })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        w.site(self.input)?;
        w.site(self.output)?;
        match self.hoist {
            None => w.byte(0),
            Some(h) => {
                w.byte(1)?;
                w.block(h.header)?;
                w.word(h.sequence)
            }
        }
    }
}
impl Row for Refinement {
    const MIN: usize = 25;
    fn width(&self) -> usize {
        match self {
            Self::Unchanged { .. } => 25,
            _ => 45,
        }
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        match c.byte()? {
            0 => Ok(Self::Unchanged {
                input: c.site()?,
                output: c.site()?,
            }),
            1 => Ok(Self::CheckedAddSplit {
                input: c.site()?,
                sum_output: c.site()?,
                false_output: c.site()?,
                induction_row_ordinal: c.size()?,
            }),
            _ => Err(Error::Tag),
        }
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        match *self {
            Self::Unchanged { input, output } => {
                w.byte(0)?;
                w.site(input)?;
                w.site(output)
            }
            Self::CheckedAddSplit {
                input,
                sum_output,
                false_output,
                induction_row_ordinal,
            } => {
                w.byte(1)?;
                w.site(input)?;
                w.site(sum_output)?;
                w.site(false_output)?;
                w.wide(u64::try_from(induction_row_ordinal).map_err(|_| Resource::Arithmetic)?)
            }
        }
    }
}
impl Row for Forwarding {
    const MIN: usize = 25;
    fn width(&self) -> usize {
        if self.store.is_some() { 37 } else { 25 }
    }
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        let input = c.site()?;
        let output = c.site()?;
        let store = match c.byte()? {
            0 => None,
            1 => Some(c.site()?),
            _ => return Err(Error::Tag),
        };
        Ok(Self {
            input,
            output,
            store,
        })
    }
    fn write(&self, w: &mut Writer<'_>) -> Result<(), Error> {
        w.site(self.input)?;
        w.site(self.output)?;
        match self.store {
            None => w.byte(0),
            Some(s) => {
                w.byte(1)?;
                w.site(s)
            }
        }
    }
}

pub(super) fn count<T: Row>(bytes: &[u8]) -> Result<usize, Error> {
    let mut c = Reader { bytes, pos: 0 };
    let n = c.word()? as usize;
    if n > MAX_REFINED_FORWARDING_HISTORY_ROWS_V1
        || n.checked_mul(T::MIN)
            .and_then(|n| n.checked_add(4))
            .ok_or(Resource::Arithmetic)?
            > bytes.len()
    {
        return Err(Error::Rows);
    }
    Ok(n)
}
pub(super) fn scan<T: Row>(bytes: &[u8]) -> Result<usize, Error> {
    let n = count::<T>(bytes)?;
    let mut c = Reader { bytes, pos: 4 };
    for _ in 0..n {
        T::read(&mut c)?;
    }
    c.done()?;
    Ok(n)
}
pub(super) fn decode<T: Row>(
    bytes: &[u8],
    meter: &mut Meter<'_, '_>,
) -> Result<(Vec<T>, usize), Error> {
    meter.work(bytes.len())?;
    let n = count::<T>(bytes)?;
    let (mut rows, storage) = meter.table::<T>(n)?;
    let mut c = Reader { bytes, pos: 4 };
    for _ in 0..n {
        meter.push(&mut rows, T::read(&mut c)?)?;
    }
    c.done()?;
    Ok((rows, storage))
}
pub(super) fn extent<T: Row>(rows: &[T]) -> Result<usize, Error> {
    if rows.len() > MAX_REFINED_FORWARDING_HISTORY_ROWS_V1 {
        return Err(Error::Rows);
    }
    rows.iter().try_fold(4usize, |n, row| {
        n.checked_add(row.width())
            .ok_or_else(|| Resource::Arithmetic.into())
    })
}
pub(super) fn encode<T: Row>(rows: &[T], bytes: &mut [u8]) -> Result<(), Error> {
    let mut w = Writer { bytes, pos: 0 };
    w.word(u32::try_from(rows.len()).map_err(|_| Resource::Arithmetic)?)?;
    for row in rows {
        row.write(&mut w)?;
    }
    if w.pos != w.bytes.len() {
        return Err(Error::Rows);
    }
    Ok(())
}

pub(super) fn read_limits(bytes: &[u8]) -> Result<Limits, Error> {
    if bytes.len() != 136 {
        return Err(Error::Limits);
    }
    let mut c = Reader { bytes, pos: 0 };
    Ok(Limits {
        refinement: LoopLimits {
            functions: c.size()?,
            blocks: c.size()?,
            edges: c.size()?,
            definitions: c.size()?,
            operations: c.size()?,
            loops: c.size()?,
            rows: c.size()?,
        },
        forwarding: ForwardingLimits {
            memory: MemoryLimits {
                functions: c.size()?,
                blocks: c.size()?,
                operations: c.size()?,
                effects: c.size()?,
                edges: c.size()?,
            },
            control_flow: ControlLimits {
                blocks: c.size()?,
                edges: c.size()?,
                edge_arguments: c.size()?,
                phi_inputs: c.size()?,
                analysis_work: c.wide()?,
            },
        },
    })
}
pub(super) fn write_limits(limits: Limits, bytes: &mut [u8]) -> Result<(), Error> {
    if bytes.len() != 136 {
        return Err(Error::Limits);
    }
    let r = limits.refinement;
    let m = limits.forwarding.memory;
    let c = limits.forwarding.control_flow;
    let mut w = Writer { bytes, pos: 0 };
    for n in [
        r.functions,
        r.blocks,
        r.edges,
        r.definitions,
        r.operations,
        r.loops,
        r.rows,
        m.functions,
        m.blocks,
        m.operations,
        m.effects,
        m.edges,
        c.blocks,
        c.edges,
        c.edge_arguments,
        c.phi_inputs,
    ] {
        w.wide(u64::try_from(n).map_err(|_| Resource::Arithmetic)?)?;
    }
    w.wide(c.analysis_work)
}
