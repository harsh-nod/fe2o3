//! Failure-only ABI observation. No validator replay, admission or authority.
use super::*;
use std::ffi::OsStr;
use std::fmt::{self, Write as _};
use std::io::Write as _;

const MAX_ARGUMENTS: usize = 8;
const MAX_BYTES: usize = 16_384;

pub(super) fn record(
    types: &[SemanticTypeDeclV1],
    location: SemanticMirLocationV1,
    step: &'static str,
    abi: Option<&SemanticFunctionAbiV1>,
    error: SemanticMirErrorV1,
) -> SemanticMirErrorV1 {
    if !matches!(error, SemanticMirErrorV1::InvalidFunctionAbi) { return error }
    let trace = std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY");
    let role = std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY_ROLE");
    if let Some(text) = render(types, location, step, abi, &error,
        trace.as_deref(), role.as_deref())
    {
        let _ = std::io::stderr().lock().write_all(&text.bytes[..text.len]);
    }
    error
}

fn render(
    types: &[SemanticTypeDeclV1],
    location: SemanticMirLocationV1,
    step: &str,
    abi: Option<&SemanticFunctionAbiV1>,
    error: &SemanticMirErrorV1,
    trace: Option<&OsStr>,
    role: Option<&OsStr>,
) -> Option<Text> {
    if trace != Some(OsStr::new("1")) || role != Some(OsStr::new("abi"))
        || !matches!(error, SemanticMirErrorV1::InvalidFunctionAbi)
    { return None }
    let mut out = Text::default();
    let complete = (|| -> fmt::Result {
        writeln!(out, "ABI_FAILURE_BEGIN location={location:?} step={step} diagnostic_only=true")?;
        let Some(abi) = abi else {
            return writeln!(out, "no_localized_abi=true");
        };
        writeln!(out, "identity={:?} canon={:?} extern={:?} variadic={} fixed={} source_count={} argument_count={} output={:?}",
            abi.identity(), abi.canon_abi(), abi.extern_abi(), abi.c_variadic(), abi.fixed_count(),
            abi.source_input_types().len(), abi.arguments().len(), abi.source_output_type())?;
        for (index, ty) in abi.source_input_types().iter().copied().take(MAX_ARGUMENTS).enumerate() {
            writeln!(out, "source_argument={index} type={ty:?} ownership={:?}",
                abi.source_argument_ownership().get(index))?;
            type_record(&mut out, types, ty)?;
        }
        writeln!(out, "source_prefix_truncated={}", abi.source_input_types().len() > MAX_ARGUMENTS)?;
        writeln!(out, "source_output={:?}", abi.source_output_type())?;
        type_record(&mut out, types, abi.source_output_type())?;
        value_record(&mut out, types, "return", abi.return_value())?;
        for (index, argument) in abi.arguments().iter().take(MAX_ARGUMENTS).enumerate() {
            writeln!(out, "physical_argument={index} role={:?}", argument.role())?;
            value_record(&mut out, types, "argument", argument.value())?;
        }
        writeln!(out, "argument_prefix_truncated={}", abi.arguments().len() > MAX_ARGUMENTS)
    })().is_ok();
    // Reserve the end marker independently so a bounded prefix is never called complete.
    out.end(complete);
    Some(out)
}

fn value_record(
    out: &mut Text,
    types: &[SemanticTypeDeclV1],
    label: &str,
    value: &SemanticAbiValueV1,
) -> fmt::Result {
    writeln!(out, "value={label} source={:?} adjusted_type={:?} mode={:?} override={:?}",
        value.source_ty(), value.adjusted_ty(), value.mode(), value.pointee_override())?;
    type_record(out, types, value.source_ty())?;
    if let Some(adjusted) = value.adjusted() {
        writeln!(out, "adjusted_layout_identity={:?} adjusted_layout={:?}", adjusted.layout_identity(), adjusted.layout())?;
        type_record(out, types, value.adjusted_ty())?;
    }
    Ok(())
}

fn type_record(out: &mut Text, types: &[SemanticTypeDeclV1], id: SemanticTypeIdV1) -> fmt::Result {
    let Some(ty) = types.get(id.index() as usize) else {
        return writeln!(out, "type={id:?} missing=true");
    };
    writeln!(out, "type={id:?} identity={:?} size={:?} align={} backend={:?} properties={:?} shape_kind={:?}",
        ty.identity(), ty.layout().size_bytes(), ty.layout().alignment_bytes(),
        ty.layout().backend_repr(), ty.abi_properties(), std::mem::discriminant(ty.shape()))?;
    match ty.shape() {
        SemanticTypeShapeV1::Pointer(pointer) => {
            writeln!(out, "pointer={pointer:?}")?;
            if let Some(pointee) = types.get(pointer.pointee().index() as usize) {
                writeln!(out, "pointee={:?} size={:?} align={} properties={:?} shape_kind={:?}",
                    pointer.pointee(), pointee.layout().size_bytes(), pointee.layout().alignment_bytes(),
                    pointee.abi_properties(), std::mem::discriminant(pointee.shape()))?;
            }
        }
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            let prefix = &fields.fields()[..fields.fields().len().min(MAX_ARGUMENTS)];
            writeln!(out, "fields={prefix:?} field_count={} fields_truncated={}",
                fields.fields().len(), fields.fields().len() > MAX_ARGUMENTS)?;
        }
        _ => {}
    }
    Ok(())
}

struct Text { bytes: [u8; MAX_BYTES], len: usize }
impl Default for Text {
    fn default() -> Self { Self { bytes: [0; MAX_BYTES], len: 0 } }
}
impl fmt::Write for Text {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        if end > MAX_BYTES - 96 { return Err(fmt::Error) }
        self.bytes[self.len..end].copy_from_slice(text.as_bytes()); self.len = end;
        Ok(())
    }
}
impl Text {
    fn end(&mut self, complete: bool) {
        let marker: &[u8] = if complete {
            b"ABI_FAILURE_END payload_truncated=false diagnostic_only=true\n"
        } else {
            b"\nABI_FAILURE_END payload_truncated=true diagnostic_only=true\n"
        };
        self.bytes[self.len..self.len + marker.len()].copy_from_slice(marker);
        self.len += marker.len();
    }
}

#[cfg(test)]
#[path = "abi_failure_observation_v1/tests.rs"]
mod tests;
