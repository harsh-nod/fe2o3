//! Bounded untrusted edit requests; live source custody is never deserialized.
use crate::production_ordered_composition_source_v1::OrderedCompositionTypedEditV1;
use fe2o3_kernel_ir::{
    Gfx942OrderedProgramRegistersV1 as Registers, Gfx942ProgramBinaryOpcodeV1 as Opcode,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role, Gfx942U32ProgramV1 as Program,
};
use serde::Deserialize;

pub(crate) const REQUEST_CAP: usize = 8192;
const SCHEMA: &str = "fe2o3-ordered-composition-source-promotion-request-v1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    schema: String,
    semantic_sha256: String,
    canonical_sha256: String,
    definition_ordinal: u8,
    original_path: String,
    original_sha256: String,
    candidate_path: String,
    helper_name: String,
    edit: Option<WireEdit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEdit {
    /// Register order is input0, input1, input2, scratch, output.
    registers: [u8; 5],
    instructions: Vec<WireInstruction>,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireRole {
    Input0,
    Input1,
    Input2,
    Scratch,
    Output,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireDestination {
    Scratch,
    Output,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireOpcode {
    Add,
    Subtract,
    And,
    Or,
    Xor,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireInstruction {
    Move {
        destination: WireDestination,
        source: WireRole,
    },
    Binary {
        opcode: WireOpcode,
        destination: WireDestination,
        left: WireRole,
        right: WireRole,
    },
}
impl From<WireRole> for Role {
    fn from(v: WireRole) -> Self {
        match v {
            WireRole::Input0 => Self::Input0,
            WireRole::Input1 => Self::Input1,
            WireRole::Input2 => Self::Input2,
            WireRole::Scratch => Self::Scratch,
            WireRole::Output => Self::Output,
        }
    }
}
impl From<WireDestination> for Destination {
    fn from(v: WireDestination) -> Self {
        match v {
            WireDestination::Scratch => Self::Scratch,
            WireDestination::Output => Self::Output,
        }
    }
}
impl From<WireOpcode> for Opcode {
    fn from(v: WireOpcode) -> Self {
        match v {
            WireOpcode::Add => Self::Add,
            WireOpcode::Subtract => Self::Subtract,
            WireOpcode::And => Self::And,
            WireOpcode::Or => Self::Or,
            WireOpcode::Xor => Self::Xor,
        }
    }
}
impl From<WireInstruction> for Instruction {
    fn from(v: WireInstruction) -> Self {
        match v {
            WireInstruction::Move {
                destination,
                source,
            } => Self::Move {
                destination: destination.into(),
                source: source.into(),
            },
            WireInstruction::Binary {
                opcode,
                destination,
                left,
                right,
            } => Self::Binary {
                opcode: opcode.into(),
                destination: destination.into(),
                left: left.into(),
                right: right.into(),
            },
        }
    }
}

/// Inert selection/edit data only. Its private fields cannot stand in for a
/// compiler Instance, source span, same-owner inspection, or source authority.
pub(crate) struct SourcePromotionRequestV1 {
    pub(super) semantic: [u8; 32],
    pub(super) canonical: [u8; 32],
    pub(super) definition: u8,
    pub(super) original: String,
    pub(super) original_sha256: [u8; 32],
    pub(super) candidate: String,
    pub(super) helper: String,
    pub(super) edit: Option<OrderedCompositionTypedEditV1>,
    pub(super) request_sha256: [u8; 32],
}
fn digest(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("promotion identity must be exactly 64 lowercase hex digits".into());
    }
    let mut out = [0; 32];
    for (slot, pair) in out.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let nibble = |b: u8| {
            if b.is_ascii_digit() {
                b - b'0'
            } else {
                b - b'a' + 10
            }
        };
        *slot = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    if out == [0; 32] {
        return Err("zero promotion identity is not a selection".into());
    }
    Ok(out)
}
impl SourcePromotionRequestV1 {
    pub(crate) fn request_digest(&self) -> &[u8; 32] {
        &self.request_sha256
    }
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, String> {
        use sha2::{Digest, Sha256};
        if bytes.is_empty() || bytes.len() > REQUEST_CAP {
            return Err("promotion request exceeds the 8192-byte bound".into());
        }
        let wire: WireRequest = serde_json::from_slice(bytes)
            .map_err(|e| format!("closed promotion request JSON: {e}"))?;
        if wire.schema != SCHEMA || wire.definition_ordinal >= 8 {
            return Err("unsupported promotion schema or definition ordinal".into());
        }
        for path in [&wire.original_path, &wire.candidate_path] {
            fe2o3_source_isa_observation::source_edit_v1::validate_source_edit_path_v1(path)
                .map_err(|_| "promotion requires bounded relative Rust source paths")?;
        }
        if wire.original_path == wire.candidate_path {
            return Err("promotion requires a different create-new candidate path".into());
        }
        crate::production_ordered_composition_source_v1::validate_ordered_composition_helper_name_v1(&wire.helper_name)?;
        let edit = wire
            .edit
            .map(|edit| {
                if !(1..=16).contains(&edit.instructions.len()) {
                    return Err("promotion edit requires 1..=16 instructions".to_owned());
                }
                let [a, b, c, scratch, output] = edit.registers;
                let registers = Registers::new(scratch, output, [a, b, c])
                    .map_err(|e| format!("promotion registers: {e}"))?;
                let mut steps = [Instruction::Move {
                    destination: Destination::Output,
                    source: Role::Input0,
                }; 16];
                let count = edit.instructions.len();
                for (slot, step) in steps.iter_mut().zip(edit.instructions) {
                    *slot = step.into();
                }
                let program = Program::from_instructions(&steps[..count])
                    .map_err(|e| format!("promotion instructions: {e}"))?;
                Ok(OrderedCompositionTypedEditV1 { program, registers })
            })
            .transpose()?;
        Ok(Self {
            semantic: digest(&wire.semantic_sha256)?,
            canonical: digest(&wire.canonical_sha256)?,
            definition: wire.definition_ordinal,
            original: wire.original_path,
            original_sha256: digest(&wire.original_sha256)?,
            candidate: wire.candidate_path,
            helper: wire.helper_name,
            edit,
            request_sha256: Sha256::digest(bytes).into(),
        })
    }

    /// Reads one regular, non-symlink request with a fixed stack buffer. The
    /// request is untrusted data; these file checks do not authenticate source.
    pub(crate) fn read(path: &std::path::Path) -> Result<Self, String> {
        use std::io::Read;
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| format!("promotion request open: {e}"))?;
        let before = file.metadata().map_err(|e| e.to_string())?;
        if !before.is_file() || before.len() == 0 || before.len() > REQUEST_CAP as u64 {
            return Err("promotion request must be a bounded regular file".into());
        }
        let mut bytes = [0u8; REQUEST_CAP + 1];
        let mut length = 0;
        while length < bytes.len() {
            let n = file.read(&mut bytes[length..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            length += n;
        }
        let after = file.metadata().map_err(|e| e.to_string())?;
        let named = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        let stamp = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if !named.is_file()
            || named.file_type().is_symlink()
            || stamp(&before) != stamp(&after)
            || stamp(&before) != stamp(&named)
            || length as u64 != before.len()
        {
            return Err("promotion request changed during bounded read".into());
        }
        Self::parse(&bytes[..length])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn request() -> Value {
        json!({"schema":SCHEMA,"semantic_sha256":"1".repeat(64),"canonical_sha256":"2".repeat(64),
            "definition_ordinal":0,"original_path":"src/kernel.rs","original_sha256":"3".repeat(64),
            "candidate_path":"src/candidate.rs","helper_name":"__fe2o3_region_0123456789abcdef","edit":null})
    }
    fn parse(v: &Value) -> Result<SourcePromotionRequestV1, String> {
        SourcePromotionRequestV1::parse(&serde_json::to_vec(v).unwrap())
    }
    #[test]
    fn selection_is_closed_and_never_an_owner() {
        let r = parse(&request()).unwrap();
        assert_eq!(r.definition, 0);
        assert!(r.edit.is_none());
        for (field, value) in [
            ("schema", json!("old")),
            ("definition_ordinal", json!(8)),
            ("canonical_sha256", json!("0".repeat(64))),
            ("semantic_sha256", json!("A".repeat(64))),
            ("original_path", json!("../kernel.rs")),
            ("candidate_path", json!("src/kernel.rs")),
            ("helper_name", json!("fn injected()")),
            ("unknown", json!(true)),
        ] {
            let mut v = request();
            v[field] = value;
            assert!(parse(&v).is_err(), "{field}");
        }
        assert!(SourcePromotionRequestV1::parse(&vec![b' '; REQUEST_CAP + 1]).is_err());
        assert!(SourcePromotionRequestV1::parse(br#"{"schema":"a","schema":"b"}"#).is_err());
    }
    #[test]
    fn edits_validate_registers_initialization_and_instruction_bounds() {
        let mut v = request();
        v["edit"] = json!({"registers":[0,1,2,3,4],"instructions":[
            {"kind":"binary","opcode":"xor","destination":"scratch","left":"input0","right":"input1"},
            {"kind":"binary","opcode":"and","destination":"output","left":"scratch","right":"input2"}
        ]});
        assert_eq!(parse(&v).unwrap().edit.unwrap().program.count(), 2);
        let good = v.clone();
        v["edit"]["registers"] = json!([0, 1, 2, 3, 3]);
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["registers"] = json!([0, 1, 2, 3, 64]);
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["instructions"][0]["left"] = json!("output");
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["instructions"][0]["destination"] = json!("input0");
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["instructions"][0]["opcode"] = json!("load");
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["instructions"][0]["address"] = json!(123);
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["instructions"] = json!([]);
        assert!(parse(&v).is_err());
        v = good.clone();
        v["edit"]["instructions"] = json!(vec![good["edit"]["instructions"][0].clone(); 17]);
        assert!(parse(&v).is_err());
    }
}
