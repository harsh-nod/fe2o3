//! Closed internal schema selection; no caller-controlled fallback or authority.
use crate::conditional_invocation_codec::Theorem;
use crate::conditional_invocation_rows_v1 as row;
use crate::conditional_invocation_v1::*;
use crate::conditional_invocation_v2::*;
use crate::conditional_invocation_validate_v1::{Records, theorem_matches};
use crate::decode::Reader;
use crate::nominal_v3::Output as Writer;

impl Theorem for ConditionalTheoremV1 {
    const MAGIC: [u8; 8] = CONDITIONAL_INVOCATION_MAGIC_V1;
    const VERSION: u16 = CONDITIONAL_INVOCATION_VERSION_V1;
    const DOMAIN: &'static [u8] = CONDITIONAL_INVOCATION_DOMAIN_V1;
    const FIXED: usize = row::FIXED;

    fn read(r: &mut Reader<'_>) -> FormatResult<Self> {
        row::theorem(r)
    }
    fn write(self, w: &mut Writer<'_>) {
        row::put_theorem(w, self);
    }
    fn matches(self, subjects: ConditionalSubjectsV1) -> bool {
        theorem_matches(CONDITIONAL_MEMORY_THEOREM_DOMAIN_V1, subjects, self, None)
    }
}

impl ConditionalTheoremV2 {
    // Reuse only the eight-field row grammar, never V1 theorem validation.
    fn wire_prefix(self) -> ConditionalTheoremV1 {
        ConditionalTheoremV1 {
            statement_identity: self.statement_identity,
            generated_source_identity: self.generated_source_identity,
            execution_identity: self.execution_identity,
            receipt_identity: self.receipt_identity,
            staging_receipt_identity: self.staging_receipt_identity,
            staging_obligation_identity: self.staging_obligation_identity,
            staging_signer_identity: self.staging_signer_identity,
            staging_execution_identity: self.staging_execution_identity,
        }
    }
}

impl Theorem for ConditionalTheoremV2 {
    const MAGIC: [u8; 8] = CONDITIONAL_INVOCATION_MAGIC_V2;
    const VERSION: u16 = CONDITIONAL_INVOCATION_VERSION_V2;
    const DOMAIN: &'static [u8] = CONDITIONAL_INVOCATION_DOMAIN_V2;
    const FIXED: usize = row::FIXED + 32;

    fn read(r: &mut Reader<'_>) -> FormatResult<Self> {
        let t = row::theorem(r)?;
        Ok(Self {
            statement_identity: t.statement_identity,
            generated_source_identity: t.generated_source_identity,
            execution_identity: t.execution_identity,
            receipt_identity: t.receipt_identity,
            staging_receipt_identity: t.staging_receipt_identity,
            staging_obligation_identity: t.staging_obligation_identity,
            staging_signer_identity: t.staging_signer_identity,
            staging_execution_identity: t.staging_execution_identity,
            cpu_input_commitment: r.fixed()?,
        })
    }
    fn write(self, w: &mut Writer<'_>) {
        row::put_theorem(w, self.wire_prefix());
        w.bytes(&self.cpu_input_commitment);
    }
    fn matches(self, subjects: ConditionalSubjectsV1) -> bool {
        theorem_matches(
            CONDITIONAL_MEMORY_THEOREM_DOMAIN_V2,
            subjects,
            self.wire_prefix(),
            Some(self.cpu_input_commitment),
        )
    }
}

impl Records for ConditionalInvocationContractInputV2<'_> {
    fn argument(&self, i: usize) -> FormatResult<ConditionalArgumentBindingV1> {
        self.arguments
            .get(i)
            .copied()
            .ok_or(invalid("argument reference"))
    }
    fn read(&self, i: usize) -> FormatResult<ConditionalReadOccurrenceV1> {
        self.reads.get(i).copied().ok_or(invalid("read reference"))
    }
    fn premise(&self, i: usize) -> FormatResult<ConditionalRuntimePremiseV1> {
        self.premises
            .get(i)
            .copied()
            .ok_or(invalid("premise reference"))
    }
}
