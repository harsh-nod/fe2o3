//! Inert cross-process content join. A valid self-signature is not an admitted
//! runtime, compiler image, original source relation or native proof authority.
use super::*;
use fe2o3_compiler_lineage::MixedMiddleEndInputV50;

/// Canonical signed content bound to exact transported V50 fields. Neither this
/// borrowed view nor any getter constructs an executed request or load authority.
#[derive(Clone, Copy)]
pub struct InertTypedSourceReceiptV53<'a> {
    wire: &'a [u8],
    expected: Expected,
}
impl<'a> InertTypedSourceReceiptV53<'a> {
    /// Complete exact signed wire, not an independently trusted certificate.
    pub fn canonical_bytes(&self) -> &'a [u8] {
        self.wire
    }
    /// Claimed original-MIR and original-SSA identities, bound to the input.
    pub fn source_identities(&self) -> [[u8; 32]; 2] {
        self.expected.binding.source
    }
    /// Exact transported original semantic-MIR content identity.
    pub fn source_semantic_identity(&self) -> [u8; 32] {
        self.expected.binding.source[0]
    }
    /// Exact transported original SSA identity, still requiring source authentication.
    pub fn source_ssa_identity(&self) -> [u8; 32] {
        self.expected.binding.source[1]
    }
    /// Exact four transported graph content identities and extents; no IR admission.
    pub fn graph_identities(&self) -> [([u8; 32], u64); 4] {
        self.expected.binding.graphs
    }
    /// Exact recomputed typed statement identity.
    pub fn statement_identity(&self) -> [u8; 32] {
        self.expected.binding.statement
    }
    /// Exact canonical generated-source identity.
    pub fn generated_identity(&self) -> [u8; 32] {
        self.expected.binding.generated
    }
    /// Claimed INDEX/endian/launch context, requiring an independent actual join.
    pub fn claimed_context_identity(&self) -> [u8; 32] {
        self.expected.binding.context
    }
    /// Claimed admitted runtime identity, not authenticated by this content check.
    pub fn claimed_runtime_identity(&self) -> [u8; 32] {
        self.expected.runtime
    }
    /// Claimed toolchain components, requiring measured-runtime authentication.
    pub fn claimed_toolchain_identities(&self) -> [[u8; 32]; 5] {
        self.expected.toolchain
    }
    /// Claimed actual execution identity, not recomputed from unavailable process output.
    pub fn claimed_execution_identity(&self) -> [u8; 32] {
        self.expected.execution
    }
    /// Self-signature key only; this method does not authenticate its provenance.
    pub fn claimed_signer(&self) -> [u8; 32] {
        self.expected.key
    }
    /// Signed generator census, requiring the protected compiler subject join.
    pub fn claimed_census(&self) -> [u64; 6] {
        self.expected.binding.census
    }
    /// Signed bounded round count, not independent fixed-policy replay.
    pub fn claimed_rounds(&self) -> u64 {
        self.expected.binding.rounds
    }
    /// Complete retained prefix witness hash and byte length.
    pub fn prefix_witness_identity(&self) -> ([u8; 32], u64) {
        (
            self.expected.binding.witness,
            self.expected.binding.witness_bytes,
        )
    }
    /// Content and self-signatures never authenticate runtime or compiler execution.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// Independent compiler, native, premise and currentness owners remain mandatory.
    pub const fn grants_load_or_launch_authority(&self) -> bool {
        false
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Cursor<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.offset.checked_add(N).ok_or(Resource::Arithmetic)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| refusal("typed receipt extent"))?
            .try_into()
            .map_err(|_| refusal("typed receipt field"))?;
        self.offset = end;
        Ok(value)
    }
    fn word(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take()?))
    }
}
fn decode(wire: &[u8]) -> Result<Expected> {
    if wire.len() != WIRE || !wire.starts_with(MAGIC) {
        return Err(refusal("distinct typed V50 wire"));
    }
    let mut r = Cursor {
        bytes: wire,
        offset: MAGIC.len(),
    };
    if r.take::<2>()? != 50u16.to_le_bytes() || r.take::<2>()? != [4, 1] {
        return Err(refusal("typed receipt version and proved-shape claim"));
    }
    let policy = u16::from_le_bytes(r.take()?);
    let source = [r.take()?, r.take()?];
    let mut graphs = [([0; 32], 0); 4];
    for row in &mut graphs {
        *row = (r.take()?, r.word()?);
    }
    let statement = r.take()?;
    let generated = r.take()?;
    let witness = r.take()?;
    let context = r.take()?;
    let witness_bytes = r.word()?;
    let rounds = r.word()?;
    let mut census = [0; 6];
    for row in &mut census {
        *row = r.word()?;
    }
    let runtime = r.take()?;
    let mut toolchain = [[0; 32]; 5];
    for row in &mut toolchain {
        *row = r.take()?;
    }
    let execution = r.take()?;
    let key = r.take()?;
    if r.offset != UNSIGNED {
        return Err(refusal("typed receipt unsigned extent"));
    }
    Ok(Expected {
        binding: Binding {
            policy,
            source,
            graphs,
            statement,
            generated,
            witness,
            context,
            witness_bytes,
            rounds,
            census,
        },
        runtime,
        toolchain,
        execution,
        key,
    })
}

fn graph_identity(bytes: &[u8], budget: &mut Budget<'_>) -> Result<([u8; 32], u64)> {
    use fe2o3_kernel_ir::{
        VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1 as DOMAIN,
        VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_POLICY_V1 as POLICY,
    };
    let len = count(bytes.len())?;
    budget.charge_work(
        bytes
            .len()
            .checked_add(DOMAIN.len() + 14)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut hash = Sha256::new();
    hash.update((DOMAIN.len() as u32).to_le_bytes());
    hash.update(DOMAIN);
    hash.update(POLICY.to_le_bytes());
    hash.update(len.to_le_bytes());
    hash.update(bytes);
    Ok((hash.finalize().into(), len))
}

/// Checks fixed V50 format, exact source/four-graph/generated/witness content,
/// statement hashing and strict self-signature. It does NOT admit graph structure,
/// replay Policy11, validate claimed census/context/runtime, or prove execution.
/// The receiving protected backend must bind every claim to the exact measured
/// compiler subject and independent native/premise owners before authority exists.
/// Input backing remains caller-owned; scratch is prepaid and settled on all exits.
pub fn check_inert_typed_source_receipt_v53<'a>(
    input: MixedMiddleEndInputV50<'a>,
    budget: &mut Budget<'_>,
) -> Result<InertTypedSourceReceiptV53<'a>> {
    let floor = budget.storage();
    let header = size_of::<Expected>() * 2
        + size_of::<Binding>()
        + size_of::<Cursor<'_>>()
        + size_of::<InertTypedSourceReceiptV53<'_>>()
        + size_of::<MixedMiddleEndInputV50<'_>>()
        + size_of::<fe2o3_compiler_lineage::MixedMiddleEndLayoutV50>()
        + size_of::<Sha256>() * 2
        + size_of::<VerifyingKey>()
        + size_of::<Signature>()
        + UNSIGNED
        + WIRE
        + size_of::<Result<InertTypedSourceReceiptV53<'_>>>() * 2
        + size_of::<[&[u8]; 5]>()
        + 16 * size_of::<usize>();
    budget.with_prepaid_scope(floor, 0, 0, header, |budget| {
        fe2o3_compiler_lineage::MixedMiddleEndLayoutV50::new(input)
            .map_err(|_| refusal("bounded exact V50 input extents"))?;
        budget.charge_work(WIRE)?;
        let expected = decode(input.execution_receipt)?;
        let b = expected.binding;
        let total = input
            .semantic_mir
            .len()
            .checked_add(input.prefix_witness.len())
            .and_then(|n| n.checked_add(input.generated_source.len().checked_mul(2)?))
            .and_then(|n| n.checked_add(64))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(total)?;
        let semantic: [u8; 32] = Sha256::digest(input.semantic_mir).into();
        let witness: [u8; 32] = Sha256::digest(input.prefix_witness).into();
        let generated = crate::generated_verus_proof_input_v3::borrowed_source_identity_v53(
            input.generated_source,
        )
        .map_err(|_| refusal("typed generated-source framing"))?
        .as_bytes();
        if b.source[0] != semantic
            || input.source_ssa_identity != b.source[1]
            || b.witness != witness
            || b.witness_bytes != count(input.prefix_witness.len())?
            || b.generated != generated
        {
            return Err(refusal("exact typed source/generated/witness content"));
        }
        for (stored, bytes) in
            b.graphs
                .iter()
                .zip([input.original, input.prefix, input.licm, input.forwarded])
        {
            if *stored != graph_identity(bytes, budget)? {
                return Err(refusal("exact typed four-graph content"));
            }
        }
        let mut hash = Sha256::new();
        for bytes in [
            super::super::DOMAIN,
            &b.source[0],
            &b.source[1],
            &b.witness,
            &b.context,
        ] {
            super::super::charge_hash(budget, &mut hash, bytes)?;
        }
        for (id, len) in b.graphs {
            super::super::charge_hash(budget, &mut hash, &id)?;
            super::super::charge_hash(budget, &mut hash, &len.to_le_bytes())?;
        }
        for n in b.census {
            super::super::charge_hash(budget, &mut hash, &n.to_le_bytes())?;
        }
        super::super::charge_hash(budget, &mut hash, &b.generated)?;
        let statement: [u8; 32] = hash.finalize().into();
        if statement != b.statement {
            return Err(refusal("typed statement hash"));
        }
        import_metered(&expected, input.execution_receipt, budget)?;
        Ok(InertTypedSourceReceiptV53 {
            wire: input.execution_receipt,
            expected,
        })
    })
}

#[cfg(test)]
#[path = "mixed_optimizer_typed_source_receipt_view_v53_tests.rs"]
mod tests;
