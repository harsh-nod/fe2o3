//! Diagnostic byte coordinates; observing them does not authenticate source bytes.

use rustc_span::{SourceFile, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub(super) struct SourceCoordinatesV1 {
    pub(super) normalized_start: u32,
    pub(super) normalized_end: u32,
    pub(super) original_start: u32,
    pub(super) original_end: u32,
}

pub(super) fn source_coordinates_v1(
    file: &SourceFile,
    span: Span,
) -> Result<SourceCoordinatesV1, &'static str> {
    if span.is_dummy() {
        return Err("dummy source span");
    }
    let lo = span.lo();
    let hi = span.hi();
    if lo > hi {
        return Err("reversed source span");
    }
    let normalized_start =
        lo.0.checked_sub(file.start_pos.0)
            .ok_or("source span starts before file")?;
    let normalized_end =
        hi.0.checked_sub(file.start_pos.0)
            .ok_or("source span ends before file")?;
    if normalized_start > normalized_end || normalized_end > file.normalized_source_len.0 {
        return Err("source span outside normalized file range");
    }

    // Imported files retain UTF-8 metadata even when source text is unavailable.
    for relative in [normalized_start, normalized_end] {
        let preceding = file
            .multibyte_chars
            .partition_point(|character| character.pos.0 < relative);
        if let Some(character) = preceding
            .checked_sub(1)
            .and_then(|index| file.multibyte_chars.get(index))
        {
            let end = character
                .pos
                .0
                .checked_add(u32::from(character.bytes))
                .ok_or("source multibyte character range overflow")?;
            if relative < end {
                return Err("source span endpoint is not a UTF-8 boundary");
            }
        }
    }

    let check_text = |source: &str| {
        if source.len() != file.normalized_source_len.0 as usize {
            return Err("source text length differs from normalized file length");
        }
        if !source.is_char_boundary(normalized_start as usize)
            || !source.is_char_boundary(normalized_end as usize)
        {
            return Err("source span endpoint is not a UTF-8 boundary");
        }
        Ok(())
    };
    if let Some(source) = file.src.as_deref() {
        check_text(source)?;
    } else if let Some(source) = file.external_src.read().get_source() {
        check_text(source)?;
    }

    // Rustc retains normalization metadata even when imported text is unavailable.
    let original_start = file.original_relative_byte_pos(lo).0;
    let original_end = file.original_relative_byte_pos(hi).0;
    if original_start > original_end || original_end > file.unnormalized_source_len {
        return Err("source span outside original file range");
    }
    Ok(SourceCoordinatesV1 {
        normalized_start,
        normalized_end,
        original_start,
        original_end,
    })
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rustc_data_structures::sync::FreezeLock;
    use rustc_span::{
        BytePos, DUMMY_SP, ExternalSource, ExternalSourceKind, FileName, RelativeBytePos,
        SourceFileHashAlgorithm,
    };

    use super::*;

    fn source_file(source: &str, start: u32) -> SourceFile {
        let mut file = SourceFile::new(
            FileName::Custom("source-census-coordinates.rs".to_owned()),
            source.to_owned(),
            SourceFileHashAlgorithm::Sha256,
            None,
        )
        .unwrap();
        file.start_pos = BytePos(start);
        file
    }

    fn file_span(file: &SourceFile, range: Range<u32>) -> Span {
        Span::with_root_ctxt(
            BytePos(file.start_pos.0.checked_add(range.start).unwrap()),
            BytePos(file.start_pos.0.checked_add(range.end).unwrap()),
        )
    }

    fn assert_coordinates(file: &SourceFile, normalized: Range<u32>, original: Range<u32>) {
        assert_eq!(
            source_coordinates_v1(file, file_span(file, normalized.clone())),
            Ok(SourceCoordinatesV1 {
                normalized_start: normalized.start,
                normalized_end: normalized.end,
                original_start: original.start,
                original_end: original.end,
            }),
        );
    }

    #[test]
    fn distinguishes_lf_crlf_and_bom_coordinates() {
        for (source, original_b, original_start) in [
            ("a\nb\n", 2..3, 0),
            ("a\r\nb\r\n", 3..4, 0),
            ("\u{feff}a\nb\n", 5..6, 3),
            ("\u{feff}a\r\nb\r\n", 6..7, 3),
        ] {
            let file = source_file(source, 100);
            let original_len = source.len() as u32;
            assert_eq!(file.src.as_deref().unwrap(), "a\nb\n");
            assert_coordinates(&file, 2..3, original_b);
            assert_coordinates(&file, 0..4, original_start..original_len);
            assert_coordinates(&file, 0..0, original_start..original_start);
            assert_coordinates(&file, 4..4, original_len..original_len);
        }
    }

    #[test]
    fn newline_ranges_include_original_crlf_and_preserve_lone_cr() {
        let file = source_file("\u{feff}a\r\nb\r\n", 100);
        assert_coordinates(&file, 1..2, 4..6);
        assert_coordinates(&file, 3..4, 7..9);

        let file = source_file("a\rb", 100);
        assert_coordinates(&file, 1..2, 1..2);
        assert_coordinates(&file, 0..3, 0..3);
    }

    #[test]
    fn utf8_coordinates_are_bytes_and_reject_split_codepoints() {
        let file = source_file("\u{feff}\u{00e9}\r\n\u{1f600}x", 100);
        assert_coordinates(&file, 0..2, 3..5);
        assert_coordinates(&file, 3..7, 7..11);
        assert_coordinates(&file, 7..8, 11..12);
        assert_coordinates(&file, 8..8, 12..12);
        for range in [1..2, 0..1, 4..7, 3..4, 5..5, 6..8] {
            assert_eq!(
                source_coordinates_v1(&file, file_span(&file, range)),
                Err("source span endpoint is not a UTF-8 boundary"),
            );
        }
    }

    #[test]
    fn coordinates_are_independent_of_source_map_start() {
        for start in [0, 1, 4096, u32::MAX - 4] {
            let file = source_file("a\r\nb", start);
            assert_coordinates(&file, 2..3, 3..4);
            assert_coordinates(&file, 3..3, 4..4);
        }
    }

    #[test]
    fn empty_and_bom_only_files_allow_non_dummy_eof_spans() {
        for (source, original_end) in [("", 0), ("\u{feff}", 3)] {
            let file = source_file(source, 100);
            assert_coordinates(&file, 0..0, original_end..original_end);
        }
    }

    #[test]
    fn rejects_dummy_and_out_of_file_spans() {
        let file = source_file("abc", 100);
        assert_eq!(
            source_coordinates_v1(&file, DUMMY_SP),
            Err("dummy source span")
        );
        for (lo, hi) in [(99, 100), (99, 101), (100, 104), (104, 104)] {
            let span = Span::with_root_ctxt(BytePos(lo), BytePos(hi));
            assert!(source_coordinates_v1(&file, span).is_err());
        }
    }

    #[test]
    fn safe_span_construction_orders_reversed_endpoints() {
        let file = source_file("abc", 100);
        // Rustc swaps reversed endpoints, so safe construction cannot reach that guard.
        let span = Span::with_root_ctxt(BytePos(102), BytePos(101));
        assert_eq!((span.lo().0, span.hi().0), (101, 102));
        assert_eq!(
            source_coordinates_v1(&file, span),
            source_coordinates_v1(&file, file_span(&file, 1..2)),
        );
    }

    #[test]
    fn imported_metadata_allows_observation_without_loading_text() {
        let file = imported_file("\u{feff}\u{00e9}\r\n\u{1f600}x");
        assert_coordinates(&file, 0..0, 3..3);
        assert_coordinates(&file, 0..2, 3..5);
        assert_coordinates(&file, 2..3, 5..7);
        assert_coordinates(&file, 3..7, 7..11);
        assert_coordinates(&file, 7..8, 11..12);
        assert_coordinates(&file, 8..8, 12..12);
        for range in [1..2, 0..1, 4..7, 3..4, 5..5, 6..8] {
            assert_eq!(
                source_coordinates_v1(&file, file_span(&file, range)),
                Err("source span endpoint is not a UTF-8 boundary"),
            );
        }
        assert!(source_coordinates_v1(&file, file_span(&file, 8..9)).is_err());
        assert!(file.src.is_none());
        assert!(matches!(
            &*file.external_src.read(),
            ExternalSource::Foreign {
                kind: ExternalSourceKind::AbsentOk,
                ..
            }
        ));
    }

    #[test]
    fn available_external_text_checks_utf8_boundaries() {
        let source = "\u{feff}\u{00e9}\r\nx";
        let file = imported_file(source);
        assert!(file.add_external_src(|| Some(source.to_owned())));
        assert_coordinates(&file, 3..4, 7..8);
        for range in [1..2, 0..1, 1..1] {
            assert_eq!(
                source_coordinates_v1(&file, file_span(&file, range)),
                Err("source span endpoint is not a UTF-8 boundary"),
            );
        }
    }

    fn imported_file(source: &str) -> SourceFile {
        let mut file = source_file(source, 100);
        file.src = None;
        file.external_src = FreezeLock::new(ExternalSource::Foreign {
            kind: ExternalSourceKind::AbsentOk,
            metadata_index: 0,
        });
        file
    }

    #[test]
    fn rejects_inconsistent_text_length_and_original_range() {
        let mut file = source_file("abc", 100);
        file.normalized_source_len = RelativeBytePos(4);
        assert_eq!(
            source_coordinates_v1(&file, file_span(&file, 0..4)),
            Err("source text length differs from normalized file length"),
        );

        let mut file = source_file("\u{feff}a\r\nb", 100);
        file.unnormalized_source_len = 6;
        assert_eq!(
            source_coordinates_v1(&file, file_span(&file, 2..3)),
            Err("source span outside original file range"),
        );
    }
}
