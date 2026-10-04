//! Failure-only rendering of the existing selected source owner, never a second import.
use super::Outcome;
use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use fe2o3_mir_model::{SemanticU32InductionBoundSnapshotReportV1, SsaBlockIdV1};
use std::fmt::{self, Write};

// The parent retains 32 KiB of stderr. Reserve room for its surrounding diagnostics.
const MAX_BYTES: usize = 24 * 1024;
const END: &str = "END bounded loop source diagnostic\n";
const TRUNCATED: &str = " [truncated]\n";

struct Capped<'a> {
    text: &'a mut String,
    limit: usize,
}
impl Write for Capped<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let remaining = self.limit.saturating_sub(self.text.len());
        let mut end = value.len().min(remaining);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        if end == value.len() {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

fn row(text: &mut String, limit: usize, args: fmt::Arguments<'_>) {
    if limit.saturating_sub(text.len()) < TRUNCATED.len() {
        return;
    }
    let limit = limit.min(text.len() + 1024);
    if (Capped {
        text,
        limit: limit - TRUNCATED.len(),
    })
    .write_fmt(args)
    .is_err()
    {
        text.push_str(TRUNCATED);
    }
}

fn needed(outcome: &Outcome, examined: usize) -> bool {
    examined != 0 && !matches!(outcome, Outcome::Joined { .. })
}

pub(super) fn capture(
    text: &mut String,
    original: &ProductionPreRankedKirOwnerV1,
    root: SemanticFunctionIdV1,
    report: &SemanticU32InductionBoundSnapshotReportV1,
    ordinal: Option<usize>,
    outcome: &Outcome,
) {
    if !text.is_empty() || !needed(outcome, report.checked_additions_examined()) {
        return;
    }
    let source = original.semantic_ssa().source_semantic();
    let body = &source.functions()[report.function().index() as usize];
    let limit = MAX_BYTES - END.len();
    row(
        text,
        limit,
        format_args!(
            "BEGIN bounded loop source diagnostic: first failing checked-add body; root={} body={} certificates={} checked_additions={} ordinal={ordinal:?} outcome={outcome:?}\n",
            root.index(),
            report.function().index(),
            report.certificates().len(),
            report.checked_additions_examined(),
        ),
    );
    row(
        text,
        limit,
        format_args!(
            "actual semantic={:?} body_identity={:?}; locals={} blocks={}; limits: 96 locals, 64 blocks, 512 statements, 1024 bytes/row, 24576 total bytes\n",
            report.semantic_mir_sha256(),
            report.function_identity(),
            body.locals().len(),
            body.blocks().len(),
        ),
    );
    if let Some(certificate) = ordinal.and_then(|i| report.certificates().get(i)) {
        row(
            text,
            limit,
            format_args!("actual certificate={certificate:?}\n"),
        );
        if let Some(function) = original.semantic_ssa().plan_for_function(report.function())
            && let Some(variables) = function
                .plan()
                .transport_variables(SsaBlockIdV1::new(certificate.header().block().index()))
        {
            row(
                text,
                limit,
                format_args!("actual header transport variables={variables:?}\n"),
            );
        }
    }
    let locals_limit = limit.min(text.len() + 6 * 1024);
    let mut locals = 0;
    for (index, local) in body.locals().iter().take(96).enumerate() {
        if locals_limit.saturating_sub(text.len()) < 256 {
            break;
        }
        row(
            text,
            locals_limit,
            format_args!(
                "local {index}: role={:?} ty={} shape={:?}\n",
                local.role(),
                local.ty().index(),
                source.types()[local.ty().index() as usize].shape(),
            ),
        );
        locals += 1;
    }
    row(
        text,
        limit,
        format_args!(
            "locals rendered={locals}/{}; statement kinds below retain actual Copy/Move, places, checked operands and CFG targets\n",
            body.locals().len()
        ),
    );
    let mut statements = 0;
    let mut blocks = 0;
    for (block_index, block) in body.blocks().iter().take(64).enumerate() {
        if limit.saturating_sub(text.len()) < 2048 || statements == 512 {
            break;
        }
        row(
            text,
            limit,
            format_args!(
                "block {block_index}: statements={}\n",
                block.statements().len()
            ),
        );
        for (statement, value) in block.statements().iter().enumerate() {
            if limit.saturating_sub(text.len()) < 1152 || statements == 512 {
                break;
            }
            row(
                text,
                limit,
                format_args!("b{block_index}.s{statement}: {:?}\n", value.kind()),
            );
            statements += 1;
        }
        row(
            text,
            limit,
            format_args!(
                "b{block_index}.terminator: {:?}\n",
                block.terminator().kind()
            ),
        );
        blocks += 1;
    }
    row(
        text,
        limit,
        format_args!(
            "rendered blocks={blocks}/{} statements={statements}; any capped rows/omitted remainder are diagnostic only\n",
            body.blocks().len()
        ),
    );
    text.push_str(END);
    debug_assert!(text.len() <= MAX_BYTES);
}

#[test]
fn capped_rows_stop_debug_rendering_and_preserve_utf8() {
    struct NeverEnds;
    impl fmt::Debug for NeverEnds {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            loop {
                f.write_str("\u{03b1}")?;
            }
        }
    }
    let mut text = String::new();
    row(&mut text, 127, format_args!("{NeverEnds:?}"));
    assert!(text.len() <= 127);
    assert!(text.ends_with(TRUNCATED));
    assert!(std::str::from_utf8(text.as_bytes()).is_ok());
    let before = text.clone();
    row(&mut text, before.len(), format_args!("not appended"));
    assert_eq!(text, before);
}

#[test]
fn diagnostic_is_failure_only_and_total_transcript_is_bounded() {
    assert!(!needed(&Outcome::NoCertificate, 0));
    assert!(needed(&Outcome::NoCertificate, 1));
    assert!(needed(
        &Outcome::Unavailable("UnsupportedFragment".into()),
        1
    ));
    let mut text = String::new();
    for index in 0..1024 {
        row(
            &mut text,
            MAX_BYTES - END.len(),
            format_args!("{index}: {:?}\n", [255_u8; 256]),
        );
    }
    text.push_str(END);
    assert!(text.len() <= MAX_BYTES);
    assert!(text.ends_with(END));
}
