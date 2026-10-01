//! Fixed renderer and HIR-derived byte coordinates. No Rust parser or offsets from requests.
use super::*;
use rustc_middle::ty::TyCtxt;
use rustc_span::{FileName, SourceFile, Span};
use std::{ops::Range, sync::Arc};

pub(super) struct Coordinates {
    pub insertion: usize,
    pub selected: Range<usize>,
    pub operands: [Range<usize>; 4],
}
pub(super) fn identifier(name: &str) -> Result<()> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || bytes.len() > IDENT_CAP
        || name == "_"
        || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_')
        || !bytes
            .iter()
            .all(|v| v.is_ascii_alphanumeric() || *v == b'_')
    {
        return Err(Error::refused(
            "BF16 source identifier outside closed ASCII profile",
        ));
    }
    Ok(())
}
pub(super) fn helper_name(name: &str) -> Result<()> {
    identifier(name)?;
    let suffix = name
        .strip_prefix("__fe2o3_bf16_tile_")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::refused("BF16 helper prefix differs"))?;
    if suffix.len() > 32 {
        return Err(Error::refused("BF16 helper suffix exceeds bound"));
    }
    Ok(())
}
fn buffer(capacity: usize) -> Result<String> {
    let mut value = String::new();
    value
        .try_reserve_exact(capacity)
        .map_err(|_| Error::refused("BF16 text allocation refused"))?;
    if value.capacity() != capacity {
        return Err(Error::refused("BF16 text allocation capacity differs"));
    }
    Ok(value)
}
fn append(output: &mut String, value: &str, cap: usize) -> Result<()> {
    if output.capacity() != cap
        || output
            .len()
            .checked_add(value.len())
            .is_none_or(|n| n > cap)
    {
        return Err(Error::refused("BF16 renderer byte/capacity bound exceeded"));
    }
    output.push_str(value);
    Ok(())
}
pub(super) fn helper(name: &str, order: Bf16TileReturnOrderV1) -> Result<String> {
    helper_name(name)?;
    let mut out = buffer(HELPER_CAP)?;
    for part in [
        "\n    #[inline(never)]\n    fn ",
        name,
        "<'wave>(\n",
        "        matrix: &::fe2o3_device::DeviceMatrix,\n",
        "        lhs: ::fe2o3_device::Bf16MfmaAFragment<'wave>,\n",
        "        rhs: ::fe2o3_device::Bf16MfmaBFragment<'wave>,\n",
        "        accumulator: ::fe2o3_device::F32AccumulatorFragment<'wave>,\n",
        "    ) -> [f32; 4] {\n",
        "        let values = matrix.multiply_accumulate(lhs, rhs, accumulator).into_values();\n",
        match order {
            Bf16TileReturnOrderV1::Identity => {
                "        [values[0], values[1], values[2], values[3]]\n"
            }
            Bf16TileReturnOrderV1::Swap01 => {
                "        [values[1], values[0], values[2], values[3]]\n"
            }
        },
        "    }\n",
    ] {
        append(&mut out, part, HELPER_CAP)?;
    }
    Ok(out)
}
fn spelling<'a>(original: &'a str, range: &Range<usize>) -> Result<&'a str> {
    let text = original
        .get(range.clone())
        .ok_or_else(|| Error::refused("BF16 operand byte range"))?;
    identifier(text)?;
    Ok(text)
}
pub(super) fn call(name: &str, original: &str, coordinates: &Coordinates) -> Result<String> {
    helper_name(name)?;
    let mut out = buffer(CALL_CAP)?;
    append(&mut out, name, CALL_CAP)?;
    append(&mut out, "(&", CALL_CAP)?;
    for (i, range) in coordinates.operands.iter().enumerate() {
        if i != 0 {
            append(&mut out, ", ", CALL_CAP)?;
        }
        append(&mut out, spelling(original, range)?, CALL_CAP)?;
    }
    append(&mut out, ")", CALL_CAP)?;
    Ok(out)
}
pub(super) fn require_direct(original: &str, coordinates: &Coordinates) -> Result<()> {
    let [matrix, lhs, rhs, accumulator] = &coordinates.operands;
    let mut expected = buffer(CALL_CAP)?;
    for part in [
        spelling(original, matrix)?,
        ".multiply_accumulate(",
        spelling(original, lhs)?,
        ",",
        spelling(original, rhs)?,
        ",",
        spelling(original, accumulator)?,
        ").into_values()",
    ] {
        append(&mut expected, part, CALL_CAP)?;
    }
    let selected = original
        .get(coordinates.selected.clone())
        .ok_or_else(|| Error::refused("BF16 selected source byte range"))?;
    if !selected
        .bytes()
        .filter(|v| !v.is_ascii_whitespace())
        .eq(expected.bytes())
    {
        return Err(Error::refused(
            "BF16 source is not the closed direct conversion spelling",
        ));
    }
    Ok(())
}
pub(super) fn splice(original: &str, c: &Coordinates, helper: &str, call: &str) -> Result<String> {
    if original.is_empty()
        || original.len() > SOURCE_CAP
        || helper.len() > HELPER_CAP
        || call.len() > CALL_CAP
        || c.insertion > c.selected.start
        || c.selected.start >= c.selected.end
    {
        return Err(Error::refused("BF16 splice input bounds differ"));
    }
    let mut out = buffer(CANDIDATE_CAP)?;
    for part in [
        original.get(..c.insertion),
        Some(helper),
        original.get(c.insertion..c.selected.start),
        Some(call),
        original.get(c.selected.end..),
    ] {
        append(
            &mut out,
            part.ok_or_else(|| Error::refused("BF16 splice UTF-8 coordinate"))?,
            CANDIDATE_CAP,
        )?;
    }
    Ok(out)
}
pub(super) fn source_file(tcx: TyCtxt<'_>, body: Span) -> Result<Arc<SourceFile>> {
    let files = tcx.sess.source_map().files();
    if files.len() > NAMES {
        return Err(Error::refused("BF16 source map roster bound"));
    }
    let mut selected = None;
    for file in files.iter() {
        if file.start_pos <= body.lo()
            && body
                .hi()
                .0
                .checked_sub(file.start_pos.0)
                .is_some_and(|n| n <= file.normalized_source_len.0)
        {
            if file.normalized_source_len.0 as usize > SOURCE_CAP
                || file.unnormalized_source_len as usize > SOURCE_CAP
                || selected.replace(file.clone()).is_some()
            {
                return Err(Error::refused("BF16 source map file bound or ambiguity"));
            }
        }
    }
    selected.ok_or_else(|| Error::refused("BF16 actual source file unavailable"))
}
pub(super) fn join_file(
    retained: &RetainedSource,
    path: &str,
    file: &SourceFile,
    actual: &str,
) -> Result<()> {
    let FileName::Real(name) = &file.name else {
        return Err(Error::refused("BF16 source file is not real"));
    };
    let observed_path = name
        .local_path()
        .ok_or_else(|| Error::refused("BF16 local path unavailable"))?;
    let expected = std::path::Path::new(path);
    let cwd = std::env::current_dir()
        .map_err(|_| Error::refused("BF16 current directory unavailable"))?;
    if cwd.capacity() > 4096 {
        return Err(Error::refused("BF16 current directory capacity bound"));
    }
    let same = if observed_path.is_absolute() {
        observed_path
            .components()
            .eq(cwd.components().chain(expected.components()))
    } else {
        observed_path == expected
    };
    if !same
        || retained.original() != actual.as_bytes()
        || actual.len() > SOURCE_CAP
        || actual.len() != file.unnormalized_source_len as usize
        || !file.src_hash.matches(actual)
        || file.src.as_deref().is_none_or(|text| text != actual)
    {
        return Err(Error::refused(
            "BF16 retained path/source map/normalization join differs",
        ));
    }
    Ok(())
}
fn range(file: &SourceFile, span: Span, source: &str) -> Result<Range<usize>> {
    if span.is_dummy() || span.from_expansion() {
        return Err(Error::refused("BF16 source coordinate is indirect"));
    }
    let lo = span
        .lo()
        .0
        .checked_sub(file.start_pos.0)
        .ok_or_else(|| Error::refused("BF16 source coordinate underflow"))? as usize;
    let hi = span
        .hi()
        .0
        .checked_sub(file.start_pos.0)
        .ok_or_else(|| Error::refused("BF16 source coordinate underflow"))? as usize;
    if lo >= hi || hi > source.len() || !source.is_char_boundary(lo) || !source.is_char_boundary(hi)
    {
        return Err(Error::refused("BF16 source coordinate bounds"));
    }
    Ok(lo..hi)
}
pub(super) fn coordinates(anchor: &anchor::Anchor, source: &str) -> Result<Coordinates> {
    let body = range(&anchor.file, anchor.body, source)?;
    let selected = range(&anchor.file, anchor.selected, source)?;
    if body.start >= selected.start
        || selected.end >= body.end
        || source.as_bytes().get(body.start) != Some(&b'{')
        || source.as_bytes().get(body.end - 1) != Some(&b'}')
    {
        return Err(Error::refused(
            "BF16 source selection is not inside its block",
        ));
    }
    let mut operands = [0..0, 0..0, 0..0, 0..0];
    for (slot, ident) in operands.iter_mut().zip(anchor.operands) {
        *slot = range(&anchor.file, ident.span, source)?;
        if slot.start < selected.start
            || slot.end > selected.end
            || source.get(slot.clone()) != Some(ident.name.as_str())
        {
            return Err(Error::refused("BF16 actual local identifier bytes differ"));
        }
    }
    Ok(Coordinates {
        insertion: body.start + 1,
        selected,
        operands,
    })
}
