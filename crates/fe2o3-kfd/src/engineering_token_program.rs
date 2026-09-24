//! Owned CPU descriptions only. No queue packet, address, signal or staged state.

use super::*;
use std::collections::BTreeSet;

pub const MAX_TOKEN_PROGRAM_DISPATCHES_V1: usize = 1024;
pub const MAX_TOKEN_PROGRAM_SLOTS_V1: usize = 256;
pub const MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1: u32 = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenProgramDefinitionV1 {
    pub dispatches: Vec<OrderedBatchDispatchV1>,
    pub slots: Vec<TokenProgramSlotV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TokenProgramSlotV1 {
    /// A whole four-byte explicit by-value field, never implicit ABI storage.
    ScalarU32 {
        dispatch: u16,
        offset: u32,
        minimum: u32,
        maximum: u32,
    },
    /// Extent, access, argument position and geometry remain immutable.
    Pointer {
        dispatch: u16,
        pointer: u16,
        buffers: Vec<u64>,
        maximum_offset: u64,
    },
}

/// Updates are positional: exactly one correctly typed value per registered slot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TokenProgramUpdateV1 {
    ScalarU32 { value: u32 },
    Pointer { buffer: u64, offset: u64 },
}

pub(crate) fn token_program_payload_bytes(definition: u32, kernarg: u32) -> io::Result<usize> {
    if definition == 0 || definition > MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1 {
        return Err(invalid("token program definition limit"));
    }
    definition
        .checked_add(kernarg)
        .filter(|total| *total <= MAX_TRANSFER_BYTES_V1)
        .map(|total| total as usize)
        .ok_or_else(|| invalid("token program transfer limit"))
}

/// Encodes registration data for the separate default-off engineering command.
pub fn encode_token_program_v1(
    definition: &TokenProgramDefinitionV1,
    kernargs: &[u8],
) -> io::Result<(CommandV1, Vec<u8>)> {
    ProgramTemplate::new(definition.clone(), kernargs.to_vec())?;
    let mut payload = serde_json::to_vec(definition).map_err(|_| invalid("token program JSON"))?;
    let definition_bytes =
        u32::try_from(payload.len()).map_err(|_| invalid("definition length"))?;
    let kernarg_bytes = u32::try_from(kernargs.len()).map_err(|_| invalid("kernarg length"))?;
    token_program_payload_bytes(definition_bytes, kernarg_bytes)?;
    payload.extend_from_slice(kernargs);
    Ok((
        CommandV1::RegisterTokenProgram {
            definition_bytes,
            kernarg_bytes,
        },
        payload,
    ))
}

pub(crate) struct ProgramTemplate {
    pub(crate) definition: TokenProgramDefinitionV1,
    payload: Vec<u8>,
    offsets: Vec<usize>,
}

impl ProgramTemplate {
    pub(crate) fn decode(
        definition_bytes: u32,
        kernarg_bytes: u32,
        payload: &[u8],
    ) -> io::Result<Self> {
        if token_program_payload_bytes(definition_bytes, kernarg_bytes)? != payload.len() {
            return Err(invalid("token program exact payload length"));
        }
        let (json, kernargs) = payload.split_at(definition_bytes as usize);
        let definition: TokenProgramDefinitionV1 =
            serde_json::from_slice(json).map_err(|_| invalid("token program JSON"))?;
        let incoming: serde_json::Value =
            serde_json::from_slice(json).map_err(|_| invalid("token program JSON"))?;
        if incoming
            != serde_json::to_value(&definition).map_err(|_| invalid("token program encoding"))?
        {
            return Err(invalid("token program unrecognized definition"));
        }
        Self::new(definition, kernargs.to_vec())
    }

    fn new(definition: TokenProgramDefinitionV1, payload: Vec<u8>) -> io::Result<Self> {
        if !(1..=MAX_TOKEN_PROGRAM_DISPATCHES_V1).contains(&definition.dispatches.len())
            || definition.slots.len() > MAX_TOKEN_PROGRAM_SLOTS_V1
            || payload.len() > MAX_TRANSFER_BYTES_V1 as usize
        {
            return Err(invalid("token program count limits"));
        }
        let mut offsets = Vec::with_capacity(definition.dispatches.len());
        let mut total = 0usize;
        for dispatch in &definition.dispatches {
            if dispatch.payload_bytes > MAX_KERNARG_BYTES_V1
                || dispatch.pointers.len() > MAX_POINTER_FIXUPS_V1
            {
                return Err(invalid("token program dispatch limits"));
            }
            offsets.push(total);
            total = total
                .checked_add(dispatch.payload_bytes as usize)
                .filter(|total| *total <= MAX_TRANSFER_BYTES_V1 as usize)
                .ok_or_else(|| invalid("token program payload overflow"))?;
        }
        if total != payload.len() {
            return Err(invalid("token program kernarg length"));
        }
        let mut occupied = BTreeSet::new();
        for slot in &definition.slots {
            match slot {
                TokenProgramSlotV1::ScalarU32 {
                    dispatch,
                    offset,
                    minimum,
                    maximum,
                } => {
                    let command = definition
                        .dispatches
                        .get(usize::from(*dispatch))
                        .ok_or_else(|| invalid("token program dispatch index"))?;
                    let end = offset
                        .checked_add(4)
                        .filter(|end| *end <= command.payload_bytes)
                        .ok_or_else(|| invalid("token program scalar extent"))?;
                    if minimum > maximum
                        || (*offset..end).any(|byte| !occupied.insert((*dispatch, false, byte)))
                    {
                        return Err(invalid("token program scalar slot"));
                    }
                    // Native registration additionally requires one exact four-byte
                    // by-value metadata field, ruling out partial/overlapping fields.
                }
                TokenProgramSlotV1::Pointer {
                    dispatch,
                    pointer,
                    buffers,
                    ..
                } => {
                    let command = definition
                        .dispatches
                        .get(usize::from(*dispatch))
                        .ok_or_else(|| invalid("token program dispatch index"))?;
                    if usize::from(*pointer) >= command.pointers.len()
                        || buffers.is_empty()
                        || buffers.len() > 8
                        || buffers.contains(&0)
                        || buffers.iter().copied().collect::<BTreeSet<_>>().len() != buffers.len()
                        || !occupied.insert((*dispatch, true, u32::from(*pointer)))
                    {
                        return Err(invalid("token program pointer slot"));
                    }
                }
            }
        }
        Ok(Self {
            definition,
            payload,
            offsets,
        })
    }

    pub(crate) fn initial(&self) -> (Vec<OrderedBatchDispatchV1>, Vec<u8>) {
        (self.definition.dispatches.clone(), self.payload.clone())
    }

    pub(crate) fn materialize(
        &self,
        updates: &[TokenProgramUpdateV1],
    ) -> io::Result<(Vec<OrderedBatchDispatchV1>, Vec<u8>)> {
        if updates.len() != self.definition.slots.len() {
            return Err(invalid("token program exact update count"));
        }
        let (mut commands, mut payload) = self.initial();
        for (slot, update) in self.definition.slots.iter().zip(updates) {
            match (slot, update) {
                (
                    TokenProgramSlotV1::ScalarU32 {
                        dispatch,
                        offset,
                        minimum,
                        maximum,
                    },
                    TokenProgramUpdateV1::ScalarU32 { value },
                ) if (*minimum..=*maximum).contains(value) => {
                    let start = self.offsets[usize::from(*dispatch)] + *offset as usize;
                    payload[start..start + 4].copy_from_slice(&value.to_le_bytes());
                }
                (
                    TokenProgramSlotV1::Pointer {
                        dispatch,
                        pointer,
                        buffers,
                        maximum_offset,
                    },
                    TokenProgramUpdateV1::Pointer { buffer, offset },
                ) if buffers.contains(buffer) && offset <= maximum_offset => {
                    let fixup =
                        &mut commands[usize::from(*dispatch)].pointers[usize::from(*pointer)];
                    fixup.buffer = *buffer;
                    fixup.buffer_offset = *offset;
                }
                _ => return Err(invalid("token program update type or bounds")),
            }
        }
        Ok((commands, payload))
    }
}

#[cfg(test)]
#[path = "engineering_token_program_tests.rs"]
mod tests;
