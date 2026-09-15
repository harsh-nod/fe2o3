//! V14-only phase records. Historical execution payload codecs stay unchanged.
//! These bounded bytes are inert: the live frontend must replay source custody.
use super::*;
use crate::reusable_phase_v1::*;
use ReusablePhaseOperationV1 as Op;
use PhaseOperationSourceV1 as Source;
use ReusablePhaseTokenRoleV1 as TokenRole;

const CALL_BYTES: usize = 184;
const TERMINAL_CALL_BYTES: usize = 180;
// Revision 1 was a frozen ART draft representing every Seal barrier as a child.
// It is not reinterpreted as the new closed terminal/defined representation.
const PHASE_CONTRACT_REVISION: u8 = 2;

fn writer(length: usize, maximum: usize) -> Option<ContractWriter> {
    if length > maximum { return None; }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).ok()?;
    if bytes.capacity() > maximum { return None; }
    Some(ContractWriter { bytes })
}
fn finish(w: ContractWriter, expected: usize) -> Option<Vec<u8>> {
    (w.bytes.len() == expected).then_some(w.bytes)
}
fn provenance_size(p: &ExecutionCapabilityProvenanceV1) -> Option<usize> {
    (p.is_complete() && !p.root.as_str().is_empty() && p.root.as_str().len() <= 256)
        .then_some(194 + p.root.as_str().len())
}
fn defined_size(d: PhaseDefinedCallV1) -> Option<usize> {
    d.is_complete().then_some(349 + d.signature.arguments().count() * 32)
}
fn position_size(p: PhaseSourcePositionV1) -> usize {
    match p { PhaseSourcePositionV1::Statement(_) => 5, PhaseSourcePositionV1::Terminator => 1 }
}
fn source_size(s: &Source) -> Option<usize> {
    Some(1 + match s {
        Source::Defined(d) => defined_size(*d)?,
        Source::ClosureReturn { closure, .. } => 32 + defined_size(*closure)? + 12 + 32,
        Source::WrapperDrop { drop_call, .. } => 32 + 1 + match drop_call {
            PhaseDropOccurrenceV1::Source(_) => TERMINAL_CALL_BYTES,
            PhaseDropOccurrenceV1::CallEntry(_) => CALL_BYTES,
        } + 32 + 32,
        Source::LeaseEnd { source_event: e, .. } => 32 + CALL_BYTES + 44 + position_size(e.original_position) + position_size(e.expanded_position) + 32,
        Source::WrapperEnd { .. } => 32 + 4 + 32,
    })
}
fn operation_size(op: &Op) -> Option<usize> {
    Some(1 + match op {
        Op::OwnerConvert { .. } => 64,
        Op::Begin { wrapper, invoke, .. } => 96 + 96 + defined_size(*wrapper)? + defined_size(*invoke)? + 32,
        Op::Bind { .. } => 192 + 6 + 8,
        Op::Seal { .. } => 96 + TERMINAL_CALL_BYTES,
        Op::RelayClosure { .. } | Op::RelayDrop { .. } | Op::CloseStorage { .. } => 32,
        Op::End { storage_count } if *storage_count < MAX_EXECUTION_CAPABILITY_RESULTS_V1 as u8 => 1,
        Op::End { .. } => return None,
    })
}
fn put_id(w: &mut ContractWriter, id: ExecutionTypeIdentityV1) -> Option<()> {
    id.is_complete().then_some(())?; w.identity(id); Some(())
}
fn id(r: &mut ContractReader<'_>) -> Option<ExecutionTypeIdentityV1> {
    let id = r.identity()?; id.is_complete().then_some(id)
}
fn put_digest(w: &mut ContractWriter, d: [u8; 32]) -> Option<()> {
    (d != [0; 32]).then_some(())?; w.digest(d); Some(())
}
fn digest(r: &mut ContractReader<'_>) -> Option<[u8; 32]> {
    let d = r.digest()?; (d != [0; 32]).then_some(d)
}
fn put_call(w: &mut ContractWriter, c: PhaseCallOccurrenceV1) -> Option<()> {
    c.is_complete().then_some(())?;
    w.digest(c.source.function); w.digest(c.source.operation); w.u32(c.source.block);
    c.source.occurrence?.encode(w);
    w.u32(c.callee_instance); w.u32(c.original_normal_target); w.u32(c.expanded_normal_target);
    Some(())
}
fn call(r: &mut ContractReader<'_>) -> Option<PhaseCallOccurrenceV1> {
    let c = PhaseCallOccurrenceV1 { source: ExecutionCapabilitySourceV1 {
        function: digest(r)?, operation: digest(r)?, block: r.u32()?,
        occurrence: Some(ExecutionCapabilitySourceOccurrenceV1::decode(r)?),
    }, callee_instance: r.u32()?, original_normal_target: r.u32()?, expanded_normal_target: r.u32()? };
    c.is_complete().then_some(c)
}
fn put_terminal_call(w: &mut ContractWriter, c: PhaseTerminalCallOccurrenceV1) -> Option<()> {
    c.is_complete().then_some(())?;
    w.digest(c.source.function); w.digest(c.source.operation); w.u32(c.source.block);
    c.source.occurrence?.encode(w);
    w.u32(c.original_normal_target); w.u32(c.expanded_normal_target); Some(())
}
fn terminal_call(r: &mut ContractReader<'_>) -> Option<PhaseTerminalCallOccurrenceV1> {
    let c = PhaseTerminalCallOccurrenceV1 { source: ExecutionCapabilitySourceV1 {
        function: digest(r)?, operation: digest(r)?, block: r.u32()?,
        occurrence: Some(ExecutionCapabilitySourceOccurrenceV1::decode(r)?),
    }, original_normal_target: r.u32()?, expanded_normal_target: r.u32()? };
    c.is_complete().then_some(c)
}
fn put_drop_call(w: &mut ContractWriter, c: PhaseDropOccurrenceV1) -> Option<()> {
    match c {
        PhaseDropOccurrenceV1::Source(s) => { w.u8(0); put_terminal_call(w, s) }
        PhaseDropOccurrenceV1::CallEntry(c) => { w.u8(1); put_call(w, c) }
    }
}
fn drop_call(r: &mut ContractReader<'_>) -> Option<PhaseDropOccurrenceV1> {
    match r.u8()? {
        0 => Some(PhaseDropOccurrenceV1::Source(terminal_call(r)?)),
        1 => Some(PhaseDropOccurrenceV1::CallEntry(call(r)?)),
        _ => None,
    }
}
fn put_defined(w: &mut ContractWriter, d: PhaseDefinedCallV1) -> Option<()> {
    d.is_complete().then_some(())?; put_call(w, d.call)?;
    w.u8(d.signature.arguments().count().try_into().ok()?);
    for argument in d.signature.arguments() { put_id(w, argument)?; }
    put_id(w, d.signature.output())?;
    w.digest(d.defined_abi); w.digest(d.defined_body); w.u32(d.incoming_count);
    w.digest(d.incoming_digest); w.digest(d.source_binding); Some(())
}
fn defined(r: &mut ContractReader<'_>) -> Option<PhaseDefinedCallV1> {
    let call = call(r)?; let count = usize::from(r.u8()?);
    if count > MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1 { return None; }
    let mut args = [ExecutionTypeIdentityV1::new([0; 32]); MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1];
    for slot in &mut args[..count] { *slot = id(r)?; }
    let d = PhaseDefinedCallV1 { call, signature: ExecutionCapabilitySignatureV1::new(&args[..count], id(r)?)?,
        defined_abi: digest(r)?, defined_body: digest(r)?, incoming_count: r.u32()?, incoming_digest: digest(r)?, source_binding: digest(r)? };
    d.is_complete().then_some(d)
}
fn put_position(w: &mut ContractWriter, p: PhaseSourcePositionV1) {
    match p { PhaseSourcePositionV1::Statement(i) => { w.u8(0); w.u32(i); }, PhaseSourcePositionV1::Terminator => w.u8(1) }
}
fn position(r: &mut ContractReader<'_>) -> Option<PhaseSourcePositionV1> {
    match r.u8()? { 0 => Some(PhaseSourcePositionV1::Statement(r.u32()?)), 1 => Some(PhaseSourcePositionV1::Terminator), _ => None }
}
fn put_source(w: &mut ContractWriter, s: &Source) -> Option<()> {
    match s {
        Source::Defined(d) => { w.u8(0); put_defined(w, *d)?; },
        Source::ClosureReturn { phase, closure, pack_block, pack_statement, return_block, source_protocol } => {
            w.u8(1); put_digest(w, phase.bytes())?; put_defined(w, *closure)?;
            w.u32(*pack_block); w.u32(*pack_statement); w.u32(*return_block); put_digest(w, *source_protocol)?;
        }
        Source::WrapperDrop { phase, drop_call, drop_abi, source_protocol } => {
            w.u8(2); put_digest(w, phase.bytes())?; put_drop_call(w, *drop_call)?; put_digest(w, *drop_abi)?; put_digest(w, *source_protocol)?;
        }
        Source::LeaseEnd { phase, bind, source_event: e, source_protocol } => {
            w.u8(3); put_digest(w, phase.bytes())?; put_call(w, *bind)?;
            put_digest(w, e.function)?; w.u32(e.instance); w.u32(e.original_block); put_position(w, e.original_position);
            w.u32(e.expanded_block); put_position(w, e.expanded_position); put_digest(w, *source_protocol)?;
        }
        Source::WrapperEnd { phase, wrapper_normal_target, source_protocol } => {
            w.u8(4); put_digest(w, phase.bytes())?; w.u32(*wrapper_normal_target); put_digest(w, *source_protocol)?;
        }
    }
    Some(())
}
fn source(r: &mut ContractReader<'_>) -> Option<Source> {
    Some(match r.u8()? {
        0 => Source::Defined(defined(r)?),
        1 => Source::ClosureReturn { phase: PhaseKeyV1::from_untrusted_bytes(digest(r)?), closure: defined(r)?, pack_block: r.u32()?, pack_statement: r.u32()?, return_block: r.u32()?, source_protocol: digest(r)? },
        2 => Source::WrapperDrop { phase: PhaseKeyV1::from_untrusted_bytes(digest(r)?), drop_call: drop_call(r)?, drop_abi: digest(r)?, source_protocol: digest(r)? },
        3 => Source::LeaseEnd { phase: PhaseKeyV1::from_untrusted_bytes(digest(r)?), bind: call(r)?, source_event: PhaseLifetimeEndV1 {
            function: digest(r)?, instance: r.u32()?, original_block: r.u32()?, original_position: position(r)?, expanded_block: r.u32()?, expanded_position: position(r)?,
        }, source_protocol: digest(r)? },
        4 => Source::WrapperEnd { phase: PhaseKeyV1::from_untrusted_bytes(digest(r)?), wrapper_normal_target: r.u32()?, source_protocol: digest(r)? },
        _ => return None,
    })
}
fn put_operation(w: &mut ContractWriter, op: &Op) -> Option<()> {
    match op {
        Op::OwnerConvert { workgroup, owner } => { w.u8(0); put_id(w, *workgroup)?; put_id(w, *owner)?; }
        Op::Begin { owner_reference, owner, phase_workgroup, outer_brand, phase_brand, dynamic_epoch, wrapper, invoke, source_protocol } => {
            w.u8(1); for t in [owner_reference, owner, phase_workgroup] { put_id(w, *t)?; }
            for d in [outer_brand, phase_brand, dynamic_epoch] { put_digest(w, *d)?; }
            put_defined(w, *wrapper)?; put_defined(w, *invoke)?; put_digest(w, *source_protocol)?;
        }
        Op::Bind { phase_reference, storage_reference, phase_workgroup, reusable_storage, phase_lds, element, layout, elements } => {
            w.u8(2); for t in [phase_reference, storage_reference, phase_workgroup, reusable_storage, phase_lds, element] { put_id(w, *t)?; }
            layout.checked_footprint(*elements).filter(|n| *n != 0)?; put_layout(w, *layout); w.u64(*elements);
        }
        Op::Seal { workgroup_before_barrier, workgroup_after_barrier, completion, barrier_call } => {
            w.u8(3); for t in [workgroup_before_barrier, workgroup_after_barrier, completion] { put_id(w, *t)?; } put_terminal_call(w, *barrier_call)?;
        }
        Op::RelayClosure { completion } => { w.u8(4); put_id(w, *completion)?; }
        Op::RelayDrop { completion } => { w.u8(5); put_id(w, *completion)?; }
        Op::CloseStorage { last_lease } => { w.u8(6); put_id(w, *last_lease)?; }
        Op::End { storage_count } => { w.u8(7); w.u8(*storage_count); }
    }
    Some(())
}
fn operation(r: &mut ContractReader<'_>) -> Option<Op> {
    Some(match r.u8()? {
        0 => Op::OwnerConvert { workgroup: id(r)?, owner: id(r)? },
        1 => Op::Begin { owner_reference: id(r)?, owner: id(r)?, phase_workgroup: id(r)?, outer_brand: digest(r)?, phase_brand: digest(r)?, dynamic_epoch: digest(r)?, wrapper: defined(r)?, invoke: defined(r)?, source_protocol: digest(r)? },
        2 => Op::Bind { phase_reference: id(r)?, storage_reference: id(r)?, phase_workgroup: id(r)?, reusable_storage: id(r)?, phase_lds: id(r)?, element: id(r)?, layout: get_layout(r)?, elements: r.u64()? },
        3 => Op::Seal { workgroup_before_barrier: id(r)?, workgroup_after_barrier: id(r)?, completion: id(r)?, barrier_call: terminal_call(r)? },
        4 => Op::RelayClosure { completion: id(r)? },
        5 => Op::RelayDrop { completion: id(r)? },
        6 => Op::CloseStorage { last_lease: id(r)? },
        7 => Op::End { storage_count: r.u8()? },
        _ => return None,
    })
}

pub(crate) fn encode_contract(p: &ReusablePhaseOpV1) -> Option<Vec<u8>> {
    let (operands, results) = p.operation.arity();
    if operands != p.operands.len() || operands > MAX_EXECUTION_CAPABILITY_OPERANDS_V1
        || results > MAX_EXECUTION_CAPABILITY_RESULTS_V1 || p.obligations.bits() != p.operation.required_obligations() { return None; }
    let size = 1 + operation_size(&p.operation)? + provenance_size(&p.provenance)? + source_size(&p.source)? + 4;
    let mut w = writer(size, MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1)?;
    w.u8(PHASE_CONTRACT_REVISION); put_operation(&mut w, &p.operation)?; encode_execution_provenance(&mut w, &p.provenance)?;
    put_source(&mut w, &p.source)?; w.u32(p.obligations.bits()); finish(w, size)
}
pub(crate) fn decode_contract(bytes: &[u8], operands: Vec<ValueId>) -> Option<ReusablePhaseOpV1> {
    if bytes.len() > MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1 || operands.len() > MAX_EXECUTION_CAPABILITY_OPERANDS_V1 { return None; }
    let mut r = ContractReader::new(bytes);
    if r.u8()? != PHASE_CONTRACT_REVISION { return None; }
    let p = ReusablePhaseOpV1 { operands, operation: operation(&mut r)?, provenance: decode_execution_provenance(&mut r)?,
        source: source(&mut r)?, obligations: ExecutionSafetyObligationsV1::from_bits(r.u32()?) };
    if !r.finished() || encode_contract(&p)?.as_slice() != bytes { return None; }
    Some(p)
}

fn put_restore(w: &mut ContractWriter, s: PhaseStorageRestoreV1) -> Option<()> {
    put_id(w, s.reusable_source)?; put_id(w, s.element)?; put_layout(w, s.layout); w.u64(s.elements); put_digest(w, s.allocation_anchor_epoch)
}
fn restore(r: &mut ContractReader<'_>) -> Option<PhaseStorageRestoreV1> {
    Some(PhaseStorageRestoreV1 { reusable_source: id(r)?, element: id(r)?, layout: get_layout(r)?, elements: r.u64()?, allocation_anchor_epoch: digest(r)? })
}
pub(crate) fn encode_token(t: &ReusablePhaseTokenTypeV1) -> Option<Vec<u8>> {
    if !t.is_complete() { return None; }
    let role = match t.role { TokenRole::OwnerLoan(_) => 2, TokenRole::StorageLoan(_) | TokenRole::ClosedStorage(_) => 111, TokenRole::CompletionReady { .. } => 65 };
    let size = 1 + provenance_size(&t.provenance)? + 192 + role;
    let mut w = writer(size, MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1)?;
    w.u8(1); encode_execution_provenance(&mut w, &t.provenance)?;
    w.digest(t.phase.bytes()); w.identity(t.owner_source); w.digest(t.owner_anchor_epoch);
    w.digest(t.outer_brand); w.digest(t.phase_brand); w.digest(t.initial_epoch);
    match t.role {
        TokenRole::OwnerLoan(state) => { w.u8(0); w.u8(match state { PhaseLoanStateV1::Active => 0, PhaseLoanStateV1::Sealed => 1, PhaseLoanStateV1::Returned => 2 }); }
        TokenRole::StorageLoan(s) => { w.u8(1); put_restore(&mut w, s)?; }
        TokenRole::ClosedStorage(s) => { w.u8(2); put_restore(&mut w, s)?; }
        TokenRole::CompletionReady { completion_source, barrier_epoch } => { w.u8(3); w.identity(completion_source); w.digest(barrier_epoch); }
    }
    finish(w, size)
}
pub(crate) fn decode_token(bytes: &[u8]) -> Option<ReusablePhaseTokenTypeV1> {
    if bytes.len() > MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1 { return None; }
    let mut r = ContractReader::new(bytes);
    if r.u8()? != 1 { return None; }
    let provenance = decode_execution_provenance(&mut r)?;
    let phase = PhaseKeyV1::from_untrusted_bytes(digest(&mut r)?);
    let owner_source = id(&mut r)?; let owner_anchor_epoch = digest(&mut r)?;
    let outer_brand = digest(&mut r)?; let phase_brand = digest(&mut r)?; let initial_epoch = digest(&mut r)?;
    let role = match r.u8()? {
        0 => TokenRole::OwnerLoan(match r.u8()? { 0 => PhaseLoanStateV1::Active, 1 => PhaseLoanStateV1::Sealed, 2 => PhaseLoanStateV1::Returned, _ => return None }),
        1 => TokenRole::StorageLoan(restore(&mut r)?), 2 => TokenRole::ClosedStorage(restore(&mut r)?),
        3 => TokenRole::CompletionReady { completion_source: id(&mut r)?, barrier_epoch: digest(&mut r)? },
        _ => return None,
    };
    let t = ReusablePhaseTokenTypeV1 { provenance, phase, owner_source, owner_anchor_epoch, outer_brand, phase_brand, initial_epoch, role };
    (r.finished() && t.is_complete()).then_some(t)
}

pub(crate) fn encode_capability(c: &ExecutionCapabilityTypeV1, version: u16) -> Option<Vec<u8>> {
    let role = match c.role {
        ExecutionCapabilityRoleV1::ReusableWorkgroup => 18,
        ExecutionCapabilityRoleV1::ReusablePhaseCompletion => 19,
        _ => return encode_execution_capability_type_v1(c),
    };
    if version != 14 || !c.is_complete() { return None; }
    let size = 1 + 32 + provenance_size(&c.provenance)? + 66 + 1;
    let mut w = writer(size, MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1)?;
    w.u8(1); w.identity(c.source_type); encode_execution_provenance(&mut w, &c.provenance)?;
    w.optional_digest(c.workgroup_brand); w.optional_digest(c.epoch); w.u8(role); finish(w, size)
}
pub(crate) fn decode_capability(bytes: &[u8], version: u16) -> Option<ExecutionCapabilityTypeV1> {
    if let Some(old) = decode_execution_capability_type_v1(bytes) { return Some(old); }
    if version != 14 || bytes.len() > MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1 { return None; }
    let mut r = ContractReader::new(bytes);
    if r.u8()? != 1 { return None; }
    let source_type = id(&mut r)?; let provenance = decode_execution_provenance(&mut r)?;
    let workgroup_brand = r.optional_digest()?; let epoch = r.optional_digest()?;
    let role = match r.u8()? { 18 => ExecutionCapabilityRoleV1::ReusableWorkgroup, 19 => ExecutionCapabilityRoleV1::ReusablePhaseCompletion, _ => return None };
    let c = ExecutionCapabilityTypeV1 { source_type, provenance, workgroup_brand, epoch, role };
    (r.finished() && c.is_complete()).then_some(c)
}
