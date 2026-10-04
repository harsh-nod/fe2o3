//! Coordinate guards exercised inside the actual constructor's rustc callback.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::HARD_MAX_VALIDATION_WORK_V1;
use rustc_span::edition::Edition;
use rustc_span::hygiene::Transparency;
use rustc_span::{DesugaringKind, ExpnData, ExpnKind, FileName, LocalExpnId, SyntaxContext};

fn at(file: &SourceFile, offset: u32) -> BytePos {
    BytePos(file.start_pos.0.checked_add(offset).unwrap())
}

fn file_span(file: &SourceFile) -> Span {
    Span::with_root_ctxt(file.start_pos, at(file, file.normalized_source_len.0))
}

fn endpoint_work(files: usize, file: &SourceFile) -> u64 {
    2 * (files as u64 + 1)
        + 3 * (file.multibyte_chars.len() as u64 + 1)
        + 5 * (u64::from(file.normalized_source_len.0) + 1)
}

fn assert_exact_work<'tcx>(
    tcx: TyCtxt<'tcx>,
    span: Span,
    expected_work: u64,
    make_owner: &impl Fn(u64) -> ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> u64 {
    let mut owner = make_owner(HARD_MAX_VALIDATION_WORK_V1);
    let initial = owner.totals.validation_work;
    let observed = reobserve_receiver_source_v1(tcx, span, span, &mut owner).unwrap();
    assert_eq!(owner.totals.validation_work - initial, expected_work);
    assert_eq!(
        observed,
        canonical_source_provenance_v1(tcx, span, 256)
            .unwrap()
            .provenance()
    );
    let exact = owner.totals.validation_work;
    assert_eq!(
        reobserve_receiver_source_v1(tcx, span, span, &mut make_owner(exact)).unwrap(),
        observed,
    );
    assert!(matches!(
        reobserve_receiver_source_v1(tcx, span, span, &mut make_owner(exact - 1)),
        Err(ProductionSemanticBodyErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
    expected_work
}

fn expanded(tcx: TyCtxt<'_>, expansion: Span, callsite: Span) -> Span {
    let data = ExpnData::default(
        ExpnKind::Desugaring(DesugaringKind::QuestionMark),
        callsite,
        Edition::Edition2024,
        None,
        None,
    );
    let expansion_id = tcx.with_stable_hashing_context(|context| LocalExpnId::fresh(data, context));
    expansion.with_ctxt(
        SyntaxContext::root().apply_mark(expansion_id.to_expn_id(), Transparency::Opaque),
    )
}

pub(in crate::production_semantic_body_v1) fn assert_receiver_source_guards_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    make_owner: &impl Fn(u64) -> ProductionSemanticBodyRequestOwnerV1<'tcx>,
) -> Span {
    let source_map = tcx.sess.source_map();
    let files = [
        ("ascii-short", format!("{}x\n", "a".repeat(4))),
        ("ascii-long", format!("{}x\n", "a".repeat(64))),
        ("unicode-short", format!("{}x\n", "\u{00e9}".repeat(4))),
        ("unicode-long", format!("{}x\n", "\u{00e9}".repeat(64))),
        (
            "unicode-earlier-line",
            format!("{}\nx\n", "\u{00e9}".repeat(64)),
        ),
    ]
    .map(|(name, source)| {
        source_map.new_source_file(
            FileName::Custom(format!("receiver-coordinate-{name}")),
            source,
        )
    });
    let template = &files[3];
    let imported = source_map.new_imported_source_file(
        template.name.clone(),
        template.src_hash,
        template.checksum_hash,
        template.stable_id,
        template.normalized_source_len.0,
        template.unnormalized_source_len,
        rustc_span::def_id::CrateNum::ZERO,
        template.lines.clone(),
        template.multibyte_chars.clone(),
        template.normalized_pos.clone(),
        0,
    );
    assert!(imported.src.is_none());
    assert!(imported.external_src.read().get_source().is_none());
    let file_count = source_map.files().len();
    let mut work = Vec::new();
    for file in &files {
        let end = at(file, file.normalized_source_len.0 - 1);
        let start = BytePos(end.0 - 1); // the ASCII x after the prefix
        work.push(assert_exact_work(
            tcx,
            Span::with_root_ctxt(start, end),
            3 + 4 * endpoint_work(file_count, file),
            make_owner,
        ));
    }
    // Four endpoints each reserve five file-byte and three multibyte passes.
    // These exact deltas are independent of file count and owner initialization.
    assert_eq!(work[1] - work[0], 20 * 60);
    assert_eq!(work[3] - work[2], 20 * 120 + 12 * 60);
    assert_eq!(work[2] - work[0], 20 * 4 + 12 * 4);
    assert_eq!(work[4] - work[3], 20);

    let imported_work = 3 + 4 * endpoint_work(file_count, &imported);
    assert_exact_work(tcx, file_span(&imported), imported_work, make_owner);
    let eof = at(&imported, imported.normalized_source_len.0);
    assert_exact_work(
        tcx,
        Span::with_root_ctxt(eof, eof),
        imported_work,
        make_owner,
    );
    assert!(
        imported.external_src.read().get_source().is_none(),
        "no source loading"
    );

    let mut invalid = Vec::new();
    for file in [&files[2], &imported] {
        invalid.push(Span::with_root_ctxt(at(file, 1), at(file, 2)));
        invalid.push(Span::with_root_ctxt(at(file, 0), at(file, 1)));
    }
    let interior = invalid[0];
    // The effective span can be valid while the final root callsite is not.
    invalid.push(expanded(tcx, file_span(&files[0]), interior));
    invalid.push(Span::with_root_ctxt(BytePos(u32::MAX), BytePos(u32::MAX)));
    for span in invalid {
        assert!(matches!(
            reobserve_receiver_source_v1(tcx, span, span, &mut make_owner(HARD_MAX_VALIDATION_WORK_V1)),
            Err(ProductionSemanticBodyErrorV1::Unsupported { construct, .. })
                if construct == "shared receiver source byte endpoint"
        ));
    }
    // Also reject an invalid body-span fallback, without a panicking validator.
    assert!(matches!(
        reobserve_receiver_source_v1(tcx, rustc_span::DUMMY_SP, interior,
            &mut make_owner(HARD_MAX_VALIDATION_WORK_V1)),
        Err(ProductionSemanticBodyErrorV1::Unsupported { construct, .. })
            if construct == "shared receiver source byte endpoint"
    ));
    let crossing = Span::with_root_ctxt(files[0].start_pos, files[1].start_pos);
    for span in [crossing, expanded(tcx, file_span(&files[0]), crossing)] {
        assert!(matches!(
            reobserve_receiver_source_v1(tcx, span, span, &mut make_owner(HARD_MAX_VALIDATION_WORK_V1)),
            Err(ProductionSemanticBodyErrorV1::Unsupported { construct, .. })
                if construct == "shared receiver cross-file source span"
        ));
    }
    let different_files = expanded(tcx, file_span(&files[0]), file_span(&imported));
    assert_exact_work(
        tcx,
        different_files,
        5 + 2 * endpoint_work(file_count, &files[0]) + 2 * endpoint_work(file_count, &imported),
        make_owner,
    );
    assert!(imported.external_src.read().get_source().is_none());
    interior
}
