//! Current retained bytes and authenticated source-map coordinates must agree.
use super::hir::Anchor;
use super::text::Coordinates;
use super::{Error, Result, SOURCE_CAP};
use fe2o3_source_isa_observation::source_candidate_io_v1::RetainedSource;
use rustc_middle::ty::TyCtxt;
use rustc_span::{FileName, SourceFile, Span};
use std::ops::Range;
use std::sync::Arc;

pub(super) fn bounded_file(tcx: TyCtxt<'_>, span: Span) -> Result<Arc<SourceFile>> {
    if span.is_dummy() || span.lo() > span.hi() {
        return Err(Error::refused("publisher source-map span is invalid"));
    }
    let files = tcx.sess.source_map().files();
    if files.len() > 4096 {
        return Err(Error::refused("publisher source-map roster bound exceeded"));
    }
    let mut selected = None;
    for file in files.iter() {
        if file.start_pos <= span.lo()
            && span
                .hi()
                .0
                .checked_sub(file.start_pos.0)
                .is_some_and(|offset| offset <= file.normalized_source_len.0)
        {
            if file.normalized_source_len.0 as usize > SOURCE_CAP
                || file.unnormalized_source_len as usize > SOURCE_CAP
                || selected.replace(file.clone()).is_some()
            {
                return Err(Error::refused(
                    "publisher source-map file is oversized or ambiguous",
                ));
            }
        }
    }
    selected.ok_or_else(|| Error::refused("publisher source-map file is unavailable"))
}

pub(super) fn join(input: &RetainedSource, path: &str, file: &SourceFile) -> Result<()> {
    let FileName::Real(name) = &file.name else {
        return Err(Error::refused(
            "publisher original is not a real source file",
        ));
    };
    let actual = name
        .local_path()
        .ok_or_else(|| Error::refused("publisher original local path is unavailable"))?;
    let cwd = std::env::current_dir()
        .map_err(|_| Error::refused("publisher current directory is unavailable"))?;
    let actual = if actual.is_absolute() {
        actual.to_owned()
    } else {
        cwd.join(actual)
    };
    if actual != cwd.join(path) {
        return Err(Error::refused(
            "publisher retained source path differs from actual HIR file",
        ));
    }
    let original = std::str::from_utf8(input.original())
        .map_err(|_| Error::refused("publisher original is not UTF-8"))?;
    if original.len() > SOURCE_CAP
        || original.len() != file.unnormalized_source_len as usize
        || !file.src_hash.matches(original)
        || file
            .src
            .as_deref()
            .is_none_or(|normalized| normalized != original)
    {
        return Err(Error::refused(
            "publisher current bytes, source-map hash or normalization differ",
        ));
    }
    Ok(())
}

fn range(file: &SourceFile, span: Span, original: &str) -> Result<Range<usize>> {
    if span.is_dummy() || span.from_expansion() {
        return Err(Error::refused(
            "publisher coordinate is not direct original source",
        ));
    }
    let start = span
        .lo()
        .0
        .checked_sub(file.start_pos.0)
        .ok_or_else(|| Error::refused("publisher source coordinate underflow"))?
        as usize;
    let end = span
        .hi()
        .0
        .checked_sub(file.start_pos.0)
        .ok_or_else(|| Error::refused("publisher source coordinate underflow"))?
        as usize;
    if start > end
        || end > original.len()
        || !original.is_char_boundary(start)
        || !original.is_char_boundary(end)
    {
        return Err(Error::refused("publisher source coordinate range differs"));
    }
    Ok(start..end)
}

pub(super) fn coordinates(anchor: &Anchor, original: &str) -> Result<Coordinates> {
    let body = range(&anchor.file, anchor.body, original)?;
    let selected = range(&anchor.file, anchor.selected, original)?;
    if body.start >= selected.start
        || body.end <= selected.end
        || original.as_bytes().get(body.start) != Some(&b'{')
        || original.as_bytes().get(body.end - 1) != Some(&b'}')
    {
        return Err(Error::refused(
            "publisher selected macro is not inside its HIR body",
        ));
    }
    let mut arguments = [0..0, 0..0, 0..0];
    for (index, ident) in anchor.parameters.iter().enumerate() {
        let span = range(&anchor.file, ident.span, original)?;
        if original.get(span.clone()) != Some(ident.name.as_str()) {
            return Err(Error::refused(
                "publisher actual parameter identifier bytes differ",
            ));
        }
        arguments[index] = span;
    }
    Ok(Coordinates {
        insertion: body.start + 1,
        selected,
        arguments,
    })
}
