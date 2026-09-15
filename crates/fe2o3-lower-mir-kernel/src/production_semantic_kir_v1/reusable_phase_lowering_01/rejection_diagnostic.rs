//! Bounded error-only observation. Never retries the emitter or grants a type.
use fe2o3_kernel_ir::{
    ExecutionCapabilityProvenanceV1, MAX_EXECUTION_CAPABILITY_OPERANDS_V1, PhaseOperationSourceV1,
    ReusablePhaseOperationV1, Type, ValueId,
};
use std::fmt::{self, Write as _};
use std::io;

const BYTE_LIMIT: usize = 16_384;
const ROOT_LIMIT: usize = 256;

struct Buffer {
    bytes: [u8; BYTE_LIMIT],
    len: usize,
    truncated: bool,
}

impl Buffer {
    fn new() -> Self {
        Self {
            bytes: [0; BYTE_LIMIT],
            len: 0,
            truncated: false,
        }
    }

    fn text(&self) -> &str {
        // write_str only retains complete UTF-8 prefixes.
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }

    fn publish(&self, out: &mut impl io::Write) {
        let _ = writeln!(
            out,
            "{} diagnostic_truncated={}",
            self.text(),
            self.truncated
        );
    }
}

impl fmt::Write for Buffer {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        let mut kept = text.len().min(BYTE_LIMIT - self.len);
        while !text.is_char_boundary(kept) {
            kept -= 1;
        }
        self.bytes[self.len..self.len + kept].copy_from_slice(&text.as_bytes()[..kept]);
        self.len += kept;
        if kept != text.len() {
            self.truncated = true;
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

fn root(out: &mut Buffer, value: &str) -> fmt::Result {
    let mut kept = value.len().min(ROOT_LIMIT);
    while !value.is_char_boundary(kept) {
        kept -= 1;
    }
    write!(
        out,
        "{:?} root_truncated={}",
        &value[..kept],
        kept != value.len()
    )
}

fn provenance(out: &mut Buffer, value: &ExecutionCapabilityProvenanceV1) -> fmt::Result {
    write!(out, " provenance_root=")?;
    root(out, value.root.as_str())?;
    write!(
        out,
        " kernel_binding={:?} frontend_unit={:?} kernel_marker={:?} target_brand={:?} launch_brand={:?} issuance={:?}",
        value.kernel_binding,
        value.frontend_unit,
        value.kernel_marker,
        value.target_brand,
        value.launch_brand,
        value.issuance
    )
}

fn input(out: &mut Buffer, index: usize, id: ValueId, ty: &Type, first: ValueId) -> fmt::Result {
    write!(
        out,
        "\ninput={index} producer={} before_first_result={}",
        id.0,
        id.0 < first.0
    )?;
    match ty {
        Type::ExecutionCapability(cap) => {
            write!(
                out,
                " kind=ExecutionCapability source_type={:?} role={:?} workgroup_brand={:?} epoch={:?}",
                cap.source_type, cap.role, cap.workgroup_brand, cap.epoch
            )?;
            provenance(out, &cap.provenance)
        }
        Type::ReusablePhaseToken(token) => {
            write!(
                out,
                " kind=ReusablePhaseToken role={:?} phase={:?} owner_source={:?} owner_anchor_epoch={:?} outer_brand={:?} phase_brand={:?} initial_epoch={:?}",
                token.role,
                token.phase,
                token.owner_source,
                token.owner_anchor_epoch,
                token.outer_brand,
                token.phase_brand,
                token.initial_epoch
            )?;
            provenance(out, &token.provenance)
        }
        // Do not traverse an unexpected recursive pointer/slice type.
        _ => write!(
            out,
            " kind=Other discriminant={:?}",
            std::mem::discriminant(ty)
        ),
    }
}

fn render(
    row: &impl fmt::Debug,
    operation: &ReusablePhaseOperationV1,
    source: &PhaseOperationSourceV1,
    source_provenance: &impl fmt::Debug,
    context_root: &str,
    inputs: &[(ValueId, &Type)],
    first_result: ValueId,
) -> Buffer {
    let mut out = Buffer::new();
    // Production row/provenance and the phase descriptors contain only bounded
    // scalar/digest records. Inputs are capped and strings are prefix-bounded.
    let result = (|| -> fmt::Result {
        write!(
            out,
            "phase-emission-rejection diagnostic_only=true row={row:?} first_result={} input_count={} expected_root=",
            first_result.0,
            inputs.len()
        )?;
        root(&mut out, context_root)?;
        write!(out, " expected_source_provenance={source_provenance:?}")?;
        for (index, (id, ty)) in inputs
            .iter()
            .take(MAX_EXECUTION_CAPABILITY_OPERANDS_V1)
            .enumerate()
        {
            input(&mut out, index, *id, ty, first_result)?;
        }
        write!(
            out,
            "\ninputs_truncated={} operation={operation:?}\nsource={source:?}",
            inputs.len() > MAX_EXECUTION_CAPABILITY_OPERANDS_V1
        )
    })();
    out.truncated |= result.is_err();
    out
}

pub(super) fn report(
    row: &impl fmt::Debug,
    operation: &ReusablePhaseOperationV1,
    source: &PhaseOperationSourceV1,
    source_provenance: &impl fmt::Debug,
    context_root: &str,
    inputs: &[(ValueId, &Type)],
    first_result: ValueId,
) {
    render(
        row,
        operation,
        source,
        source_provenance,
        context_root,
        inputs,
        first_result,
    )
    .publish(&mut io::stderr().lock());
}

#[cfg(test)]
#[path = "rejection_diagnostic_tests.rs"]
mod tests;
