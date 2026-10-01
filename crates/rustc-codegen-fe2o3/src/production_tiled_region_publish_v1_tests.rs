//! Pure controls only: strings, counters and effect models cannot mint source custody.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn sample() -> (String, text::Coordinates) {
    let input = "fn root() {\nlet result = matrix.multiply_accumulate(lhs, rhs, accumulator).into_values();\nuse_result(result[0]);\n}\n";
    let start = input.find("matrix.multiply").unwrap();
    let end = input.find(";\nuse_result").unwrap();
    let find = |needle: &str| {
        let start = start + input[start..end].find(needle).unwrap();
        start..start + needle.len()
    };
    (
        input.into(),
        text::Coordinates {
            insertion: input.find('{').unwrap() + 1,
            selected: start..end,
            operands: [
                find("matrix"),
                find("lhs"),
                find("rhs"),
                find("accumulator"),
            ],
        },
    )
}
#[test]
fn bf16_publisher_literal_helper_orders_are_exact_and_not_encoding_edits() {
    let identity = text::helper(
        "__fe2o3_bf16_tile_generated",
        Bf16TileReturnOrderV1::Identity,
    )
    .unwrap();
    let swapped =
        text::helper("__fe2o3_bf16_tile_generated", Bf16TileReturnOrderV1::Swap01).unwrap();
    assert_eq!(identity.capacity(), HELPER_CAP);
    assert_eq!(swapped.capacity(), HELPER_CAP);
    assert_eq!(identity.matches("multiply_accumulate(").count(), 1);
    assert_eq!(
        swapped,
        identity.replace(
            "[values[0], values[1], values[2], values[3]]",
            "[values[1], values[0], values[2], values[3]]"
        )
    );
    assert!(identity.contains("matrix: &::fe2o3_device::DeviceMatrix"));
    assert!(identity.contains("Bf16MfmaAFragment<'wave>"));
    assert!(identity.contains("Bf16MfmaBFragment<'wave>"));
    assert_eq!(Bf16TileReturnOrderV1::Swap01.permutation(), [1, 0, 2, 3]);
}
#[test]
fn bf16_publisher_splice_changes_only_selected_expression_and_insertion() {
    let (input, coordinates) = sample();
    text::require_direct(&input, &coordinates).unwrap();
    let helper = text::helper(
        "__fe2o3_bf16_tile_generated",
        Bf16TileReturnOrderV1::Identity,
    )
    .unwrap();
    let call = text::call("__fe2o3_bf16_tile_generated", &input, &coordinates).unwrap();
    assert_eq!(
        call,
        "__fe2o3_bf16_tile_generated(&matrix, lhs, rhs, accumulator)"
    );
    let output = text::splice(&input, &coordinates, &helper, &call).unwrap();
    assert_eq!(output.capacity(), CANDIDATE_CAP);
    assert_eq!(
        output,
        format!(
            "{}{}{}{}{}",
            &input[..coordinates.insertion],
            helper,
            &input[coordinates.insertion..coordinates.selected.start],
            call,
            &input[coordinates.selected.end..]
        )
    );
    assert!(output.ends_with("use_result(result[0]);\n}\n"));
}
#[test]
fn bf16_publisher_spelling_refuses_comments_wrappers_extra_calls_and_type_arguments() {
    let (input, mut c) = sample();
    let selected = input[c.selected.clone()].to_owned();
    for changed in [
        selected.replace(".multiply", "/*not admitted*/.multiply"),
        format!("identity({selected})"),
        selected.replace("multiply_accumulate(", "multiply_accumulate::<u32>("),
        selected.replace(".into_values()", ".other_conversion()"),
    ] {
        let text = format!(
            "{}{}{}",
            &input[..c.selected.start],
            changed,
            &input[c.selected.end..]
        );
        c.selected.end = c.selected.start + changed.len();
        let find = |needle: &str| {
            let at = c.selected.start + changed.find(needle).unwrap();
            at..at + needle.len()
        };
        c.operands = [
            find("matrix"),
            find("lhs"),
            find("rhs"),
            find("accumulator"),
        ];
        assert!(text::require_direct(&text, &c).is_err());
        c.selected.end = c.selected.start + selected.len();
    }
}
#[test]
fn bf16_publisher_spelling_accepts_only_ascii_whitespace_variation() {
    let (input, _) = sample();
    let input = input
        .replace(".multiply", "\n   .multiply")
        .replace(", rhs", ",\n rhs");
    let start = input.find("matrix").unwrap();
    let end = input.find(";\nuse_result").unwrap();
    let find = |needle: &str| {
        let at = start + input[start..end].find(needle).unwrap();
        at..at + needle.len()
    };
    let c = text::Coordinates {
        insertion: input.find('{').unwrap() + 1,
        selected: start..end,
        operands: [
            find("matrix"),
            find("lhs"),
            find("rhs"),
            find("accumulator"),
        ],
    };
    text::require_direct(&input, &c).unwrap();
    assert!(text::identifier("r#type").is_err());
    assert!(text::identifier("λ").is_err());
}
#[test]
fn bf16_publisher_helper_and_operand_names_are_bounded() {
    for invalid in [
        "",
        "_",
        "x",
        "__fe2o3_bf16_tile_",
        "__fe2o3_bf16_tile_a();",
        "__fe2o3_bf16_tile_é",
    ] {
        assert!(text::helper_name(invalid).is_err());
    }
    text::helper_name(&format!("__fe2o3_bf16_tile_{}", "x".repeat(32))).unwrap();
    assert!(text::helper_name(&format!("__fe2o3_bf16_tile_{}", "x".repeat(33))).is_err());
    text::identifier(&"x".repeat(IDENT_CAP)).unwrap();
    assert!(text::identifier(&"x".repeat(IDENT_CAP + 1)).is_err());
}
#[test]
fn bf16_publisher_splice_bounds_and_utf8_refuse_before_growth() {
    let (input, mut c) = sample();
    let helper = text::helper("__fe2o3_bf16_tile_x", Bf16TileReturnOrderV1::Identity).unwrap();
    let call = text::call("__fe2o3_bf16_tile_x", &input, &c).unwrap();
    assert!(text::splice(&"x".repeat(SOURCE_CAP + 1), &c, &helper, &call).is_err());
    assert!(text::splice(&input, &c, &"x".repeat(HELPER_CAP + 1), &call).is_err());
    assert!(text::splice(&input, &c, &helper, &"x".repeat(CALL_CAP + 1)).is_err());
    c.insertion = c.selected.end;
    assert!(text::splice(&input, &c, &helper, &call).is_err());
    c.insertion = 1;
    c.selected = 1..2;
    assert!(text::splice("éxx", &c, &helper, &call).is_err());
}
#[test]
fn bf16_publisher_exact_prepaid_boundary_keeps_original_floor_and_work() {
    let mut work = Work::new(PUBLISH_WORK);
    let mut budget = Budget::new(&mut work, PUBLISH_SCRATCH + 19);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    prepaid(&mut budget, ledger, 19, |actual| {
        assert!(actual.work_ledger_identity_v1() == ledger);
        assert_eq!(actual.storage(), PUBLISH_SCRATCH + 19);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), PUBLISH_WORK);
    assert_eq!(budget.peak_storage(), PUBLISH_SCRATCH + 19);
}
#[test]
fn bf16_publisher_one_short_work_and_storage_never_run_callback() {
    for short_work in [true, false] {
        let mut work = Work::new(PUBLISH_WORK - usize::from(short_work));
        let mut budget = Budget::new(&mut work, PUBLISH_SCRATCH + 19 - usize::from(!short_work));
        budget.reserve_storage(19).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(prepaid::<()>(&mut budget, ledger, 19, |_| panic!("unpaid callback")).is_err());
        assert_eq!(budget.storage(), 19);
        assert!(if short_work {
            budget.failed_work().is_some()
        } else {
            budget.failed_storage().is_some()
        });
        assert!(
            same_ledger(&budget, ledger, 19).is_err(),
            "sticky denial is not reset"
        );
    }
}
#[test]
fn bf16_publisher_wrong_ledger_and_undercut_floor_refuse_before_work() {
    let mut first = Work::new(PUBLISH_WORK);
    let mut second = Work::new(PUBLISH_WORK);
    let a = Budget::new(&mut first, PUBLISH_SCRATCH);
    let mut b = Budget::new(&mut second, PUBLISH_SCRATCH);
    assert!(
        prepaid::<()>(&mut b, a.work_ledger_identity_v1(), 0, |_| panic!(
            "wrong ledger"
        ))
        .is_err()
    );
    let id = b.work_ledger_identity_v1();
    assert!(prepaid::<()>(&mut b, id, 1, |_| panic!("undercut floor")).is_err());
    assert_eq!(b.work(), 0);
}
#[test]
fn bf16_publisher_error_and_unwind_keep_progress_and_consumed_work() {
    for panic in [false, true] {
        let mut work = Work::new(PUBLISH_WORK);
        let mut budget = Budget::new(&mut work, PUBLISH_SCRATCH);
        let ledger = budget.work_ledger_identity_v1();
        let mut progress = Bf16SourcePublicationProgressV1::new();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepaid::<()>(&mut budget, ledger, 0, |_| {
                progress.effect = Bf16SourcePublicationEffectV1::MayHaveCreatedCandidate;
                if panic {
                    panic!("synthetic callback, not an actual link");
                }
                Err(Error::refused("synthetic post-link refusal"))
            })
        }));
        assert_eq!(
            progress.effect,
            Bf16SourcePublicationEffectV1::MayHaveCreatedCandidate
        );
        assert!(progress.require_fresh().is_err());
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), PUBLISH_WORK);
        if panic {
            assert!(caught.is_err());
        } else {
            let error = caught.unwrap().unwrap_err().with_effect(progress.effect);
            assert_eq!(
                error.effect,
                Bf16SourcePublicationEffectV1::MayHaveCreatedCandidate
            );
        }
    }
}
fn wire() -> String {
    let d = "12".repeat(32);
    format!(
        r#"{{"schema":"fe2o3-bf16-tile-source-promotion-request-v1","semantic_sha256":"{d}","canonical_sha256":"{d}","mir_sha256":"{d}","original_sha256":"{d}","original_path":"original/src/lib.rs","candidate_path":"candidate/src/lib.rs","helper_name":"__fe2o3_bf16_tile_generated","return_order":"identity"}}"#
    )
}
#[test]
fn bf16_publisher_request_is_closed_borrowed_and_no_physical_controls() {
    let wire = wire();
    let request = Bf16TileSourcePublishRequestV1::parse(wire.as_bytes()).unwrap();
    assert_eq!(request.return_order, Bf16TileReturnOrderV1::Identity);
    assert!(request.helper_name.as_ptr() >= wire.as_ptr());
    for modified in [
        wire.replace("\"identity\"", "\"swap02\""),
        wire.replace("\"identity\"", "\"identity\",\"vgprs\":[0,1]"),
        wire.replace("\"identity\"", "\"identity\",\"return_order\":\"swap01\""),
        wire.replace("candidate/src/lib.rs", "../candidate.rs"),
        wire.replace("candidate/src/lib.rs", "original/src/lib.rs"),
        wire.replace(&"12".repeat(32), &"00".repeat(32)),
        wire.replace(&"12".repeat(32), &"AF".repeat(32)),
        wire.replace("__fe2o3_bf16_tile_generated", r"__fe2o3_bf16_tile_\u0078"),
    ] {
        assert!(Bf16TileSourcePublishRequestV1::parse(modified.as_bytes()).is_err());
    }
    let swap = wire.replace("\"identity\"", "\"swap01\"");
    assert_eq!(
        Bf16TileSourcePublishRequestV1::parse(swap.as_bytes())
            .unwrap()
            .return_order
            .permutation(),
        [1, 0, 2, 3]
    );
    assert!(Bf16TileSourcePublishRequestV1::parse(&vec![b' '; 8193]).is_err());
}
#[test]
fn bf16_publisher_logical_resource_terms_are_explicit_and_finite() {
    assert_eq!(PUBLISH_WORK, BYTE_WORK + HIR_WORK + LOOKUP_WORK);
    assert_eq!(
        PUBLISH_SCRATCH,
        BUFFER_STORAGE + HEADER_STORAGE + HIR_DEPTH * 512
    );
    assert!(PUBLISH_WORK < 4 * 1024 * 1024);
    assert!(PUBLISH_SCRATCH < 256 * 1024);
    assert_eq!(CANDIDATE_CAP, 70_144);
}
