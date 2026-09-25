//! Closed historical physical rows. No constructor/Deserialize grants stop,
//! process, native handle or source authority. Oracle bytes never fill samples.
use crate::rocgdb_mi_parser_v3::MiResultsV3;
use crate::syntax::{Refusal, fields, positive, text};
use serde::{Serialize, Serializer, ser::SerializeStruct};

pub(crate) const KEYS: &[&str] = &[
    "s", "p", "pid", "i", "a", "t", "start", "g", "w", "wg", "d", "q", "ag", "ar", "pc", "c", "cp",
    "co", "entry", "desc", "kernarg", "out", "regid", "dwarf", "rbytes", "mbytes", "status", "reg",
    "mem",
];
fn u64_string<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(v)
}
macro_rules! context {
    ($($name:ident),+ $(,)?) => {
        /// Observed scalar selectors only; not native ownership or requery handles.
        #[derive(Debug, Serialize)]
        pub struct SnapshotContextV2 { $(#[serde(serialize_with="u64_string")] $name: u64,)+ }
    };
}
context!(
    sequence,
    pid,
    inferior,
    process,
    host,
    start,
    gpu,
    wave,
    workgroup,
    dispatch,
    queue,
    agent,
    architecture,
    pc,
    completion_base,
    checkpoint,
    code_object,
    entry,
    descriptor,
    kernarg,
    output_base,
    register_id
);
/// Exact absence reported by the same producer after its confirmation/retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnavailableV2 {
    RegisterUnavailable,
    MemoryUnavailable,
    ShortMemory,
}
/// Fixed retained observation. An unavailable result never serializes zero arrays.
#[derive(Debug)]
pub struct PhysicalSnapshotV2 {
    context: SnapshotContextV2,
    unavailable: Option<UnavailableV2>,
    register: [u8; 4],
    memory: [u8; 272],
}
impl PhysicalSnapshotV2 {
    pub(crate) fn captured(&self) -> bool {
        self.unavailable.is_none()
    }
}
const _: () = assert!(std::mem::size_of::<PhysicalSnapshotV2>() <= 512);
struct Hex<'a>(&'a [u8]);
impl Serialize for Hex<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if !matches!(self.0.len(), 4 | 272) {
            return Err(serde::ser::Error::custom("fixed byte extent"));
        }
        let mut hex = [0_u8; 544];
        let digits = b"0123456789abcdef";
        for (n, &b) in self.0.iter().enumerate() {
            hex[2 * n] = digits[(b >> 4) as usize];
            hex[2 * n + 1] = digits[(b & 15) as usize];
        }
        let text =
            std::str::from_utf8(&hex[..self.0.len() * 2]).map_err(serde::ser::Error::custom)?;
        s.serialize_str(text)
    }
}
impl Serialize for PhysicalSnapshotV2 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out =
            s.serialize_struct("PhysicalSnapshotV2", if self.captured() { 5 } else { 3 })?;
        out.serialize_field(
            "status",
            if self.captured() {
                "available"
            } else {
                "unavailable"
            },
        )?;
        out.serialize_field("context", &self.context)?;
        if let Some(reason) = self.unavailable {
            out.serialize_field("reason", &reason)?;
        } else {
            out.serialize_field("register_hex", &Hex(&self.register))?;
            out.serialize_field("memory_hex", &Hex(&self.memory))?;
            out.serialize_field("fixture_oracle_matched", &true)?;
        }
        out.end()
    }
}
fn decode<const N: usize>(bytes: &[u8]) -> Result<[u8; N], Refusal> {
    if bytes.len() != N * 2 {
        return Err(Refusal::Shape);
    }
    let nibble = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(Refusal::Shape),
    };
    let mut value = [0; N];
    for (out, pair) in value.iter_mut().zip(bytes.chunks_exact(2)) {
        *out = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(value)
}
fn oracle(register: &[u8; 4], memory: &[u8; 272]) -> Result<(), Refusal> {
    if u32::from_le_bytes(*register) != 0x1357_9bdf
        || memory[..8] != 0x0123_4567_89ab_cdef_u64.to_le_bytes()
        || memory[264..] != 0xfedc_ba98_7654_3210_u64.to_le_bytes()
    {
        return Err(Refusal::Artifact);
    }
    for lane in 0..64_u32 {
        let at = 8 + lane as usize * 4;
        if memory[at..at + 4] != (0x1357_9bdf_u32 ^ lane).to_le_bytes() {
            return Err(Refusal::Artifact);
        }
    }
    Ok(())
}
pub(crate) fn parse(r: &MiResultsV3, gpu: u64, mi_pc: u64) -> Result<PhysicalSnapshotV2, Refusal> {
    fields(r, KEYS, KEYS)?;
    if positive(text(r, "p")?)? != 8
        || positive(text(r, "dwarf")?)? != 44
        || positive(text(r, "rbytes")?)? != 4
        || positive(text(r, "mbytes")?)? != 272
    {
        return Err(Refusal::Shape);
    }
    let get = |k| positive(text(r, k)?);
    let context = SnapshotContextV2 {
        sequence: get("s")?,
        pid: get("pid")?,
        inferior: get("i")?,
        process: get("a")?,
        host: get("t")?,
        start: get("start")?,
        gpu: get("g")?,
        wave: get("w")?,
        workgroup: get("wg")?,
        dispatch: get("d")?,
        queue: get("q")?,
        agent: get("ag")?,
        architecture: get("ar")?,
        pc: get("pc")?,
        completion_base: get("c")?,
        checkpoint: get("cp")?,
        code_object: get("co")?,
        entry: get("entry")?,
        descriptor: get("desc")?,
        kernarg: get("kernarg")?,
        output_base: get("out")?,
        register_id: get("regid")?,
    };
    if context.sequence > 16
        || context.gpu != gpu
        || context.gpu == context.host
        || context.pc != mi_pc
        || context.entry.checked_add(80) != Some(context.pc)
        || context
            .entry
            .checked_sub(6144)
            .and_then(|v| v.checked_add(1984))
            != Some(context.descriptor)
        || !context.output_base.is_multiple_of(4096)
        || context.output_base.checked_add(272).is_none()
        || context.completion_base.checked_add(8).is_none()
    {
        return Err(Refusal::Stop);
    }
    let unavailable = match text(r, "status")? {
        b"available" => None,
        b"register-unavailable" => Some(UnavailableV2::RegisterUnavailable),
        b"memory-unavailable" => Some(UnavailableV2::MemoryUnavailable),
        b"short-memory" => Some(UnavailableV2::ShortMemory),
        _ => return Err(Refusal::Shape),
    };
    let (register, memory) = if unavailable.is_none() {
        let register = decode(text(r, "reg")?)?;
        let memory = decode(text(r, "mem")?)?;
        oracle(&register, &memory)?;
        (register, memory)
    } else {
        if !text(r, "reg")?.is_empty() || !text(r, "mem")?.is_empty() {
            return Err(Refusal::Shape);
        }
        ([0; 4], [0; 272]) // Private vacant storage, never presented as observed bytes.
    };
    Ok(PhysicalSnapshotV2 {
        context,
        unavailable,
        register,
        memory,
    })
}
