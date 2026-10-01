//! Fixed-width, inert CPU observations. No source or admission authority.
//! A complete footer is NOT outer source postflight or normal-route success.
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub(super) const HEADER_BYTES: usize = 128;
pub(super) const POSITIVE_BYTES: usize = 5776;
pub(super) const NEGATIVE_BYTES: usize = 80;
pub(super) const FOOTER_BYTES: usize = 64;
pub(super) const COMPLETE_BYTES: usize =
    HEADER_BYTES + 18 * POSITIVE_BYTES + 16 * NEGATIVE_BYTES + FOOTER_BYTES;
pub(super) const FILE_CAP: usize = 131072;
const MAGIC: [u8; 16] = *b"F2BF16CPU-V1\0\0\0\0";
const END: [u8; 16] = *b"F2BF16CPU-END\0\0\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Header {
    // 0 = direct original (both arrays from the SAME MFMA checkpoint);
    // 1 = fresh helper (MFMA checkpoint plus actual after-Call checkpoint).
    pub role: u32,
    // Requested publication order; the original's effective CPU order is 0.
    pub order: u32,
    pub session: u32,
    pub source_sha256: [u8; 32],
    pub canonical_sha256: [u8; 32],
    pub root: u32,
    pub helper: u32,    // u32::MAX for the direct original, not an invented helper.
    pub call: [u32; 2], // [u32::MAX; 2] for the direct original.
    pub matrix: [u32; 2],
    pub store: [u32; 2],
}
impl Header {
    fn validate(self) -> Result<(), Error> {
        check(
            self.session < 4 && self.role == self.session % 2 && self.order == self.session / 2,
            "session/role/order",
        )?;
        if self.role == 0 {
            check(
                self.helper == u32::MAX && self.call == [u32::MAX; 2],
                "direct original must not claim a Call",
            )?;
        } else {
            check(
                self.helper != self.root && self.helper != u32::MAX && self.call != [u32::MAX; 2],
                "fresh helper identity",
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Backing<const N: usize, const I: usize> {
    pub input_id: u64,
    pub allocation: u64,
    pub byte_length: u32,
    pub init_bit_length: u32,
    pub view_offset: u64,
    pub view_length: u64,
    pub bytes: [u8; N],
    pub initialized: [u8; I],
}
impl<const N: usize, const I: usize> Backing<N, I> {
    fn validate(&self, id: u64, allocation: u64, offset: u64, length: u64) -> Result<(), Error> {
        check(
            N.checked_add(7).ok_or(Error::Arithmetic)? / 8 == I,
            "packed init shape",
        )?;
        check(
            usize::try_from(self.byte_length).ok() == Some(N)
                && usize::try_from(self.init_bit_length).ok() == Some(N),
            "exact backing/init lengths",
        )?;
        check(
            self.input_id == id
                && self.allocation == allocation
                && self.view_offset == offset
                && self.view_length == length,
            "backing identity/view",
        )?;
        // This finite positive profile requires every byte initialized.
        for bit in 0..N {
            check(
                self.initialized[bit / 8] & (1 << (bit % 8)) != 0,
                "positive uninitialized backing",
            )?;
        }
        for bit in N..I * 8 {
            check(
                self.initialized[bit / 8] & (1 << (bit % 8)) == 0,
                "unused init bits",
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Positive {
    pub pattern: u32,
    pub output_length: u32,
    pub records: u64,
    pub steps: u64,
    pub matrix_mask: u64,
    pub caller_mask: u64,
    pub store_mask: u64,
    pub allocations: [u64; 3],
    pub matrix: [u32; 256],
    pub caller: [u32; 256],
    pub a: Backing<512, 64>,
    pub b: Backing<512, 64>,
    pub output: Backing<272, 34>,
    pub write_count: u32,
    // Indexed by ACTUAL lane. Non-written lanes are eight zero words.
    // [lane, size, offset-lo, offset-hi, f32-bits, allocation-lo, allocation-hi, reserved].
    pub writes: [[u32; 8]; 64],
}
impl Positive {
    pub fn validate(&self, header: Header, ordinal: u32) -> Result<(), Error> {
        check(
            ordinal < 18
                && self.pattern == ordinal / 3
                && self.output_length == [64, 13, 0][ordinal as usize % 3],
            "positive request order",
        )?;
        check(
            self.records > 0 && self.steps > 0,
            "positive execution counters",
        )?;
        let length = self.output_length;
        let mask = match length {
            64 => u64::MAX,
            13 => (1 << 13) - 1,
            0 => 0,
            _ => return Err(Error::Shape("output length")),
        };
        check(
            self.matrix_mask == u64::MAX
                && self.caller_mask == u64::MAX
                && self.store_mask == mask
                && self.write_count == length,
            "positive completion/write counts",
        )?;
        check(
            self.allocations[0] != self.allocations[1]
                && self.allocations[0] != self.allocations[2]
                && self.allocations[1] != self.allocations[2],
            "distinct backings",
        )?;
        self.a.validate(11, self.allocations[0], 0, 256)?;
        self.b.validate(12, self.allocations[1], 0, 256)?;
        self.output
            .validate(13, self.allocations[2], 8, u64::from(length))?;
        let permutation = if header.role == 1 && header.order == 1 {
            [1, 0, 2, 3]
        } else {
            [0, 1, 2, 3]
        };
        for lane in 0..64 {
            for component in 0..4 {
                let at = (4 * (lane / 16) + component) * 16 + lane % 16;
                let from = (4 * (lane / 16) + permutation[component]) * 16 + lane % 16;
                check(
                    self.caller[at] == self.matrix[from],
                    "actual Return/caller order",
                )?;
            }
            let write = self.writes[lane];
            if lane < length as usize {
                let offset = u64::from(write[2]) | (u64::from(write[3]) << 32);
                let allocation = u64::from(write[5]) | (u64::from(write[6]) << 32);
                let bits = self.caller[(4 * (lane / 16)) * 16 + lane % 16];
                check(
                    write[0] == lane as u32
                        && write[1] == 4
                        && offset == 8 + 4 * lane as u64
                        && write[4] == bits
                        && allocation == self.allocations[2]
                        && write[7] == 0,
                    "actual Store row",
                )?;
                check(
                    self.output.bytes[8 + 4 * lane..12 + 4 * lane] == bits.to_le_bytes(),
                    "Store/backing join",
                )?;
            } else {
                check(write == [0; 8], "unused Store slot")?;
            }
        }
        for slot in 0..68 {
            if !(2..2 + length as usize).contains(&slot) {
                check(
                    self.output.bytes[4 * slot..4 * slot + 4] == 0x7f123456u32.to_le_bytes(),
                    "full backing canary",
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Negative {
    pub control: u32,
    pub classification: u32,
    pub matrix_mask: u64,
    pub caller_mask: u64,
    pub global_writes: u64,
    pub floor_restored: u32,
}
impl Negative {
    pub fn validate(self, ordinal: u32) -> Result<(), Error> {
        check(
            (18..34).contains(&ordinal) && self.control == ordinal - 18,
            "negative request order",
        )?;
        // Classification codes are declared by the adapter, never authority.
        // 1 uninitialized; 2 domain A; 3 domain B; 4 step limit;
        // 5 incomplete observation; 6 event failure; 7 profile launch.
        const EXPECTED: [u32; 16] = [1, 1, 2, 3, 2, 3, 2, 3, 2, 4, 5, 5, 6, 7, 7, 7];
        check(
            self.classification == EXPECTED[self.control as usize]
                && self.matrix_mask == 0
                && self.caller_mask == 0
                && self.global_writes == 0
                && self.floor_restored == 1,
            "exact negative result",
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub(super) enum Error {
    Shape(&'static str),
    Arithmetic,
    Io(std::io::Error),
    Failed,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(s) => write!(f, "CPU sidecar shape: {s}"),
            Self::Arithmetic => f.write_str("CPU sidecar arithmetic"),
            Self::Io(e) => write!(f, "CPU sidecar I/O: {e}"),
            Self::Failed => f.write_str("CPU sidecar already failed"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
fn check(value: bool, detail: &'static str) -> Result<(), Error> {
    if value {
        Ok(())
    } else {
        Err(Error::Shape(detail))
    }
}
struct Put<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}
impl<'a> Put<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn raw(&mut self, value: &[u8]) -> Result<(), Error> {
        let end = self
            .offset
            .checked_add(value.len())
            .ok_or(Error::Arithmetic)?;
        self.bytes
            .get_mut(self.offset..end)
            .ok_or(Error::Shape("encode bound"))?
            .copy_from_slice(value);
        self.offset = end;
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.raw(&value.to_le_bytes())
    }
    fn finish(self) -> Result<(), Error> {
        check(self.offset == self.bytes.len(), "encode exact size")
    }
}
struct Get<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Get<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn raw<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let end = self.offset.checked_add(N).ok_or(Error::Arithmetic)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Error::Shape("decode bound"))?
            .try_into()
            .map_err(|_| Error::Shape("decode width"))?;
        self.offset = end;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.raw()?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.raw()?))
    }
    fn zeros<const N: usize>(&mut self) -> Result<(), Error> {
        check(self.raw::<N>()? == [0; N], "reserved bytes")
    }
    fn finish(self) -> Result<(), Error> {
        check(self.offset == self.bytes.len(), "decode exact size")
    }
}
fn encode_header(h: Header) -> Result<[u8; HEADER_BYTES], Error> {
    h.validate()?;
    let mut bytes = [0; HEADER_BYTES];
    let mut p = Put::new(&mut bytes);
    p.raw(&MAGIC)?;
    p.u32(1)?;
    p.u32(h.role)?;
    p.u32(h.order)?;
    p.u32(h.session)?;
    p.raw(&h.source_sha256)?;
    p.raw(&h.canonical_sha256)?;
    p.u32(h.root)?;
    p.u32(h.helper)?;
    for pair in [h.call, h.matrix, h.store] {
        for word in pair {
            p.u32(word)?;
        }
    }
    p.finish()?;
    Ok(bytes)
}
fn decode_header(bytes: &[u8]) -> Result<Header, Error> {
    let mut g = Get::new(bytes);
    check(
        g.raw::<16>()? == MAGIC && g.u32()? == 1,
        "header magic/version",
    )?;
    let h = Header {
        role: g.u32()?,
        order: g.u32()?,
        session: g.u32()?,
        source_sha256: g.raw()?,
        canonical_sha256: g.raw()?,
        root: g.u32()?,
        helper: g.u32()?,
        call: [g.u32()?, g.u32()?],
        matrix: [g.u32()?, g.u32()?],
        store: [g.u32()?, g.u32()?],
    };
    g.finish()?;
    h.validate()?;
    Ok(h)
}
fn put_backing<const N: usize, const I: usize>(
    p: &mut Put<'_>,
    b: &Backing<N, I>,
) -> Result<(), Error> {
    p.u64(b.input_id)?;
    p.u64(b.allocation)?;
    p.u32(b.byte_length)?;
    p.u32(b.init_bit_length)?;
    p.u64(b.view_offset)?;
    p.u64(b.view_length)?;
    p.raw(&b.bytes)?;
    p.raw(&b.initialized)
}
fn get_backing<const N: usize, const I: usize>(g: &mut Get<'_>) -> Result<Backing<N, I>, Error> {
    Ok(Backing {
        input_id: g.u64()?,
        allocation: g.u64()?,
        byte_length: g.u32()?,
        init_bit_length: g.u32()?,
        view_offset: g.u64()?,
        view_length: g.u64()?,
        bytes: g.raw()?,
        initialized: g.raw()?,
    })
}
fn frame(p: &mut Put<'_>, kind: u32, payload: usize, ordinal: u32) -> Result<(), Error> {
    p.u32(kind)?;
    p.u32(u32::try_from(payload).map_err(|_| Error::Arithmetic)?)?;
    p.u32(ordinal)?;
    p.u32(0)
}
fn read_frame(g: &mut Get<'_>, kind: u32, payload: usize, ordinal: u32) -> Result<(), Error> {
    check(
        g.u32()? == kind && g.u32()? as usize == payload && g.u32()? == ordinal,
        "frame identity/order",
    )?;
    g.zeros::<4>()
}
fn encode_positive(h: Header, ordinal: u32, r: &Positive) -> Result<[u8; POSITIVE_BYTES], Error> {
    r.validate(h, ordinal)?;
    let mut bytes = [0; POSITIVE_BYTES];
    let mut p = Put::new(&mut bytes);
    frame(&mut p, 1, POSITIVE_BYTES - 16, ordinal)?;
    p.u32(r.pattern)?;
    p.u32(r.output_length)?;
    p.u64(r.records)?;
    p.u64(r.steps)?;
    for n in [r.matrix_mask, r.caller_mask, r.store_mask] {
        p.u64(n)?;
    }
    for n in r.allocations {
        p.u64(n)?;
    }
    for words in [&r.matrix, &r.caller] {
        for n in words {
            p.u32(*n)?;
        }
    }
    put_backing(&mut p, &r.a)?;
    put_backing(&mut p, &r.b)?;
    put_backing(&mut p, &r.output)?;
    p.u32(r.write_count)?;
    for write in r.writes {
        for word in write {
            p.u32(word)?;
        }
    }
    p.raw(&[0; 10])?;
    p.finish()?;
    Ok(bytes)
}
fn decode_positive(h: Header, ordinal: u32, bytes: &[u8]) -> Result<Positive, Error> {
    let mut g = Get::new(bytes);
    read_frame(&mut g, 1, POSITIVE_BYTES - 16, ordinal)?;
    let pattern = g.u32()?;
    let output_length = g.u32()?;
    let records = g.u64()?;
    let steps = g.u64()?;
    let matrix_mask = g.u64()?;
    let caller_mask = g.u64()?;
    let store_mask = g.u64()?;
    let allocations = [g.u64()?, g.u64()?, g.u64()?];
    let mut matrix = [0; 256];
    let mut caller = [0; 256];
    for words in [&mut matrix, &mut caller] {
        for n in words {
            *n = g.u32()?;
        }
    }
    let a = get_backing(&mut g)?;
    let b = get_backing(&mut g)?;
    let output = get_backing(&mut g)?;
    let write_count = g.u32()?;
    let mut writes = [[0; 8]; 64];
    for write in &mut writes {
        for word in write {
            *word = g.u32()?;
        }
    }
    g.zeros::<10>()?;
    g.finish()?;
    let r = Positive {
        pattern,
        output_length,
        records,
        steps,
        matrix_mask,
        caller_mask,
        store_mask,
        allocations,
        matrix,
        caller,
        a,
        b,
        output,
        write_count,
        writes,
    };
    r.validate(h, ordinal)?;
    Ok(r)
}
fn encode_negative(ordinal: u32, r: Negative) -> Result<[u8; NEGATIVE_BYTES], Error> {
    r.validate(ordinal)?;
    let mut bytes = [0; NEGATIVE_BYTES];
    let mut p = Put::new(&mut bytes);
    frame(&mut p, 2, NEGATIVE_BYTES - 16, ordinal)?;
    p.u32(r.control)?;
    p.u32(r.classification)?;
    p.u64(r.matrix_mask)?;
    p.u64(r.caller_mask)?;
    p.u64(r.global_writes)?;
    p.u32(r.floor_restored)?;
    p.raw(&[0; 28])?;
    p.finish()?;
    Ok(bytes)
}
fn decode_negative(ordinal: u32, bytes: &[u8]) -> Result<Negative, Error> {
    let mut g = Get::new(bytes);
    read_frame(&mut g, 2, NEGATIVE_BYTES - 16, ordinal)?;
    let r = Negative {
        control: g.u32()?,
        classification: g.u32()?,
        matrix_mask: g.u64()?,
        caller_mask: g.u64()?,
        global_writes: g.u64()?,
        floor_restored: g.u32()?,
    };
    g.zeros::<28>()?;
    g.finish()?;
    r.validate(ordinal)?;
    Ok(r)
}
fn footer(prefix_sha256: [u8; 32]) -> Result<[u8; FOOTER_BYTES], Error> {
    let mut bytes = [0; FOOTER_BYTES];
    let mut p = Put::new(&mut bytes);
    p.raw(&END)?;
    for n in [34, 18, 16, COMPLETE_BYTES as u32] {
        p.u32(n)?;
    }
    p.raw(&prefix_sha256)?;
    p.finish()?;
    Ok(bytes)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Summary {
    pub bytes: usize,
    pub sha256: [u8; 32],
    pub rows: u32,
}
pub(super) struct Writer {
    file: File,
    header: Header,
    hash: Sha256,
    committed_bytes: usize,
    next: u32,
    failed: bool,
}
impl Writer {
    // Root/adapter owns and validates the explicit parent directory; create_new
    // refuses an existing final basename, including a symlink. An I/O failure
    // may leave a partial new file, which is never removed or retried here.
    pub fn create_new(path: &Path, header: Header) -> Result<Self, Error> {
        let bytes = encode_header(header)?;
        check(COMPLETE_BYTES <= FILE_CAP, "fixed complete cap")?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        let mut w = Self {
            file,
            header,
            hash: Sha256::new(),
            committed_bytes: 0,
            next: 0,
            failed: false,
        };
        w.append(&bytes)?;
        Ok(w)
    }
    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Failed);
        }
        // Every append refusal is sticky; no failed attempt can be retried.
        self.failed = true;
        let end = self
            .committed_bytes
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        check(
            end <= COMPLETE_BYTES && end <= FILE_CAP,
            "prepaid sidecar cap",
        )?;
        self.file.write_all(bytes)?;
        self.file.sync_data()?;
        self.hash.update(bytes);
        self.committed_bytes = end;
        self.failed = false;
        Ok(())
    }
    pub fn positive(&mut self, row: &Positive) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Failed);
        }
        let bytes = match encode_positive(self.header, self.next, row) {
            Ok(b) => b,
            Err(e) => {
                self.failed = true;
                return Err(e);
            }
        };
        self.append(&bytes)?;
        self.next += 1;
        Ok(())
    }
    pub fn negative(&mut self, row: Negative) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Failed);
        }
        let bytes = match encode_negative(self.next, row) {
            Ok(b) => b,
            Err(e) => {
                self.failed = true;
                return Err(e);
            }
        };
        self.append(&bytes)?;
        self.next += 1;
        Ok(())
    }
    pub fn committed_prefix(&self) -> (usize, u32, bool) {
        // This is a synced prefix, NOT a census of a possible partial last write.
        (self.committed_bytes, self.next, self.failed)
    }
    pub fn finish(mut self) -> Result<Summary, Error> {
        check(
            !self.failed
                && self.next == 34
                && self.committed_bytes == COMPLETE_BYTES - FOOTER_BYTES,
            "complete CPU prefix",
        )?;
        let bytes = footer(self.hash.clone().finalize().into())?;
        self.append(&bytes)?;
        self.file.sync_all()?;
        Ok(Summary {
            bytes: self.committed_bytes,
            sha256: self.hash.finalize().into(),
            rows: self.next,
        })
    }
}

// Strict inert parser. It does not establish that its input came from a compiler
// callback, nor does a valid footer establish outer postflight or normal refusal.
pub(super) fn validate_complete(bytes: &[u8], expected: Header) -> Result<Summary, Error> {
    check(
        bytes.len() == COMPLETE_BYTES && bytes.len() <= FILE_CAP,
        "exact complete length",
    )?;
    let header = decode_header(&bytes[..HEADER_BYTES])?;
    check(header == expected, "selected header equality")?;
    let mut offset = HEADER_BYTES;
    for ordinal in 0..18 {
        let end = offset
            .checked_add(POSITIVE_BYTES)
            .ok_or(Error::Arithmetic)?;
        decode_positive(header, ordinal, &bytes[offset..end])?;
        offset = end;
    }
    for ordinal in 18..34 {
        let end = offset
            .checked_add(NEGATIVE_BYTES)
            .ok_or(Error::Arithmetic)?;
        decode_negative(ordinal, &bytes[offset..end])?;
        offset = end;
    }
    let hash: [u8; 32] = Sha256::digest(&bytes[..offset]).into();
    check(
        bytes[offset..] == footer(hash)?,
        "footer counts/prefix digest",
    )?;
    Ok(Summary {
        bytes: bytes.len(),
        sha256: Sha256::digest(bytes).into(),
        rows: 34,
    })
}

#[path = "gfx942_bf16_publication_sidecar_controls_v1_tests.rs"]
mod controls;
