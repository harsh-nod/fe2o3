//! Closed occurrence encodings. Ordinals and duplicate edge occurrences are retained.
use crate::loop_unroll_history_wire_v1::{Error, Meter, add};
use fe2o3_kernel_analysis::{
    CanonicalKirLoopLimitsV1 as LoopLimits, CanonicalKirLoopUnrollCopyV1 as CopyRole,
    CanonicalKirLoopUnrollLimitsV1 as Limits, CanonicalKirLoopUnrollOriginV1 as Origin,
    CanonicalKirLoopUnrollSelectionV1 as Selection,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Site,
};
use std::mem::size_of;

pub(super) const WIDTHS: [usize; 6] = [20, 44, 28, 20, 28, 36];
pub(super) const SCRATCH: usize = size_of::<Reader<'_>>()
    + size_of::<Writer<'_>>()
    + size_of::<Origin<Definition>>()
    + size_of::<[(usize, usize); 10]>()
    + 20
    + 104;
pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
    pub family: u8,
}
impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = add(self.pos, n)?;
        let bytes = self.bytes.get(self.pos..end).ok_or(Error::Rows {
            family: self.family,
        })?;
        self.pos = end;
        Ok(bytes)
    }
    pub fn word(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| Error::Rows {
                family: self.family,
            },
        )?))
    }
    fn block(&mut self) -> Result<Block, Error> {
        Ok(Block {
            function: Function(self.word()?),
            block: self.word()?,
        })
    }
    fn site(&mut self) -> Result<Site, Error> {
        Ok(Site {
            block: self.block()?,
            operation: self.word()?,
        })
    }
    fn edge(&mut self) -> Result<Edge, Error> {
        Ok(Edge {
            source: self.block()?,
            successor: self.word()?,
        })
    }
}
pub(super) struct Writer<'a> {
    pub bytes: &'a mut [u8],
    pub pos: usize,
    pub family: u8,
}
impl Writer<'_> {
    pub fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = add(self.pos, bytes.len())?;
        self.bytes
            .get_mut(self.pos..end)
            .ok_or(Error::Rows {
                family: self.family,
            })?
            .copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
    fn word(&mut self, value: u32) -> Result<(), Error> {
        self.put(&value.to_le_bytes())
    }
    fn block(&mut self, value: Block) -> Result<(), Error> {
        self.word(value.function.0)?;
        self.word(value.block)
    }
    fn site(&mut self, value: Site) -> Result<(), Error> {
        self.block(value.block)?;
        self.word(value.operation)
    }
    fn edge(&mut self, value: Edge) -> Result<(), Error> {
        self.block(value.source)?;
        self.word(value.successor)
    }
}
pub(super) trait Coordinate: Copy {
    const WIDTH: usize;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error>;
    fn write(self, c: &mut Writer<'_>) -> Result<(), Error>;
}
impl Coordinate for Block {
    const WIDTH: usize = 8;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        c.block()
    }
    fn write(self, c: &mut Writer<'_>) -> Result<(), Error> {
        c.block(self)
    }
}
impl Coordinate for Site {
    const WIDTH: usize = 12;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        c.site()
    }
    fn write(self, c: &mut Writer<'_>) -> Result<(), Error> {
        c.site(self)
    }
}
impl Coordinate for Edge {
    const WIDTH: usize = 12;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        c.edge()
    }
    fn write(self, c: &mut Writer<'_>) -> Result<(), Error> {
        c.edge(self)
    }
}
impl Coordinate for Argument {
    const WIDTH: usize = 16;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            edge: c.edge()?,
            argument: c.word()?,
        })
    }
    fn write(self, c: &mut Writer<'_>) -> Result<(), Error> {
        c.edge(self.edge)?;
        c.word(self.argument)
    }
}
impl Coordinate for Definition {
    const WIDTH: usize = 20;
    fn read(c: &mut Reader<'_>) -> Result<Self, Error> {
        let tag = c.take(4)?;
        if tag[1..] != [0; 3] {
            return Err(Error::Tag { family: c.family });
        }
        let value = match tag[0] {
            0 => {
                let function = Function(c.word()?);
                let argument = c.word()?;
                if c.take(8)? != [0; 8] {
                    return Err(Error::Tag { family: c.family });
                }
                Self::FunctionArgument { function, argument }
            }
            1 => {
                let block = c.block()?;
                let argument = c.word()?;
                if c.word()? != 0 {
                    return Err(Error::Tag { family: c.family });
                }
                Self::BlockArgument { block, argument }
            }
            2 => Self::Result {
                operation: c.site()?,
                result: c.word()?,
            },
            _ => return Err(Error::Tag { family: c.family }),
        };
        Ok(value)
    }
    fn write(self, c: &mut Writer<'_>) -> Result<(), Error> {
        match self {
            Self::FunctionArgument { function, argument } => {
                c.put(&[0; 4])?;
                c.word(function.0)?;
                c.word(argument)?;
                c.put(&[0; 8])
            }
            Self::BlockArgument { block, argument } => {
                c.put(&[1, 0, 0, 0])?;
                c.block(block)?;
                c.word(argument)?;
                c.word(0)
            }
            Self::Result { operation, result } => {
                c.put(&[2, 0, 0, 0])?;
                c.site(operation)?;
                c.word(result)
            }
        }
    }
}
fn control(copy: CopyRole, present: bool, family: u8) -> Result<[u8; 4], Error> {
    let (tag, index) = match copy {
        CopyRole::Retained => (0, 0),
        CopyRole::Header(n) if n <= 8 => (1, n),
        CopyRole::Body(n) if n < 8 => (2, n),
        CopyRole::OmittedBody => (3, 0),
        _ => return Err(Error::Tag { family }),
    };
    Ok([tag, index, u8::from(present), 0])
}
pub(super) fn row<C: Coordinate>(c: &mut Reader<'_>) -> Result<Origin<C>, Error> {
    let tag = c.take(4)?;
    let copy = match (tag[0], tag[1]) {
        (0, 0) => CopyRole::Retained,
        (1, n) if n <= 8 => CopyRole::Header(n),
        (2, n) if n < 8 => CopyRole::Body(n),
        (3, 0) => CopyRole::OmittedBody,
        _ => return Err(Error::Tag { family: c.family }),
    };
    if tag[2] > 1 || tag[3] != 0 {
        return Err(Error::Tag { family: c.family });
    }
    let input = C::read(c)?;
    let output = if tag[2] == 1 {
        Some(C::read(c)?)
    } else {
        if c.take(C::WIDTH)?.iter().any(|byte| *byte != 0) {
            return Err(Error::Tag { family: c.family });
        }
        None
    };
    Ok(Origin {
        input,
        output,
        copy,
    })
}
pub(super) fn header(bytes: &[u8], family: u8) -> Result<usize, Error> {
    let width = *WIDTHS
        .get(usize::from(family))
        .ok_or(Error::Tag { family })?;
    let h = bytes.get(..8).ok_or(Error::Rows { family })?;
    if h[4..] != [family, 1, 0, 0] {
        return Err(Error::Tag { family });
    }
    let count = u32::from_le_bytes(h[..4].try_into().map_err(|_| Error::Rows { family })?) as usize;
    if add(8, count.checked_mul(width).ok_or(Resource::Arithmetic)?)? != bytes.len() {
        return Err(Error::Rows { family });
    }
    Ok(count)
}
pub(super) fn validate<C: Coordinate>(bytes: &[u8], family: u8) -> Result<(), Error> {
    let count = header(bytes, family)?;
    let mut c = Reader {
        bytes,
        pos: 8,
        family,
    };
    for _ in 0..count {
        let _ = row::<C>(&mut c)?;
    }
    if c.pos != bytes.len() {
        return Err(Error::Rows { family });
    }
    Ok(())
}
pub(super) fn extent<C: Coordinate>(values: &[Origin<C>]) -> Result<usize, Error> {
    u32::try_from(values.len()).map_err(|_| Error::Limit)?;
    add(
        8,
        values
            .len()
            .checked_mul(4 + 2 * C::WIDTH)
            .ok_or(Resource::Arithmetic)?,
    )
}
pub(super) fn encode<C: Coordinate>(
    values: &[Origin<C>],
    bytes: &mut [u8],
    family: u8,
) -> Result<(), Error> {
    let mut c = Writer {
        bytes,
        pos: 0,
        family,
    };
    c.word(u32::try_from(values.len()).map_err(|_| Error::Limit)?)?;
    c.put(&[family, 1, 0, 0])?;
    for value in values {
        c.put(&control(value.copy, value.output.is_some(), family)?)?;
        value.input.write(&mut c)?;
        match value.output {
            Some(output) => output.write(&mut c)?,
            None => c.put(&[0; 20][..C::WIDTH])?,
        }
    }
    if c.pos != c.bytes.len() {
        return Err(Error::Rows { family });
    }
    Ok(())
}
pub(super) fn decode<C: Coordinate>(
    bytes: &[u8],
    family: u8,
    meter: &mut Meter<'_, '_>,
) -> Result<(Vec<Origin<C>>, usize), Error> {
    meter.work(bytes.len())?;
    let count = header(bytes, family)?;
    let (mut values, storage) = meter.table(count)?;
    let mut c = Reader {
        bytes,
        pos: 8,
        family,
    };
    for _ in 0..count {
        meter.push(&mut values, row::<C>(&mut c)?)?;
    }
    Ok((values, storage))
}
pub(super) fn settings(bytes: &[u8]) -> Result<(Limits, Option<Selection>), Error> {
    if bytes.len() != 104 || bytes[56] > 8 || bytes[57..64] != [0; 7] || bytes[90..96] != [0; 6] {
        return Err(Error::Settings);
    }
    let size = |at| -> Result<usize, Error> {
        let value = u64::from_le_bytes(bytes[at..at + 8].try_into().map_err(|_| Error::Settings)?);
        usize::try_from(value).map_err(|_| Resource::Arithmetic.into())
    };
    let limits = Limits {
        loops: LoopLimits {
            functions: size(0)?,
            blocks: size(8)?,
            edges: size(16)?,
            definitions: size(24)?,
            operations: size(32)?,
            loops: size(40)?,
            rows: size(48)?,
        },
        max_iterations: bytes[56],
        max_output_operand_uses: size(64)?,
        max_output_edge_arguments: size(72)?,
        max_origin_rows: size(80)?,
    };
    let selection = match bytes[88] {
        0 if bytes[89..104] == [0; 15] => None,
        1 if bytes[89] <= limits.max_iterations => Some(Selection {
            fact: size(96)?,
            iterations: bytes[89],
        }),
        _ => return Err(Error::Settings),
    };
    Ok((limits, selection))
}
pub(super) fn write_settings(
    limits: Limits,
    selection: Option<Selection>,
    bytes: &mut [u8],
) -> Result<(), Error> {
    if bytes.len() != 104
        || limits.max_iterations > 8
        || selection.is_some_and(|s| s.iterations > limits.max_iterations)
    {
        return Err(Error::Settings);
    }
    bytes.fill(0);
    for (at, value) in [
        (0, limits.loops.functions),
        (8, limits.loops.blocks),
        (16, limits.loops.edges),
        (24, limits.loops.definitions),
        (32, limits.loops.operations),
        (40, limits.loops.loops),
        (48, limits.loops.rows),
        (64, limits.max_output_operand_uses),
        (72, limits.max_output_edge_arguments),
        (80, limits.max_origin_rows),
    ] {
        bytes[at..at + 8].copy_from_slice(
            &u64::try_from(value)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
    }
    bytes[56] = limits.max_iterations;
    if let Some(selection) = selection {
        bytes[88] = 1;
        bytes[89] = selection.iterations;
        bytes[96..104].copy_from_slice(
            &u64::try_from(selection.fact)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
    }
    Ok(())
}
