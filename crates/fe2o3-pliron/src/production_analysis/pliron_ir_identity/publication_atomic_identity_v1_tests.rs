use pliron::parsable::parse_from_str;

fn publication_identity_text_v1() -> &'static str {
    r#"builtin.func @publication_identity: builtin.function <() -> ()>
{
  ^entry():
    extent = kernel.index_constant () [] [kernel_index_value: kernel.index_value 128]: <() -> (kernel.index)>;
    cell = kernel.index_constant () [] [kernel_index_value: kernel.index_value 0]: <() -> (kernel.index)>;
    payload = kernel.ranked_view (extent) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;
    flags = kernel.ranked_view (extent) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;
    kernel.access (flags, cell) [] [kernel_access_kind: kernel.access_kind AtomicWrite, kernel_atomic_ordering: kernel.atomic_ordering Release, kernel_atomic_scope: kernel.atomic_scope System, kernel_publication_atomic: kernel.publication_atomic ReleaseRequestU32]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> ()>;
    kernel.access (flags, cell) [] [kernel_access_kind: kernel.access_kind AtomicWrite, kernel_atomic_ordering: kernel.atomic_ordering Release, kernel_atomic_scope: kernel.atomic_scope System, kernel_publication_atomic: kernel.publication_atomic ReleaseReadyU32]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> ()>;
    acquired = kernel.access (flags, cell) [] [kernel_access_kind: kernel.access_kind AtomicRead, kernel_atomic_ordering: kernel.atomic_ordering Acquire, kernel_atomic_scope: kernel.atomic_scope System, kernel_publication_atomic: kernel.publication_atomic AcquireU32]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> (kernel.index)>;
    checked, success = kernel.publication_read_guard (cell, extent, acquired) [] []: <(kernel.index, kernel.index, kernel.index) -> (kernel.index, kernel.checked_access_capability)>;
    kernel.access (payload, checked, success) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,true,[0]>, kernel.index, kernel.checked_access_capability) -> ()>;
    kernel.return () [] []: <() -> ()>
}
"#
}

fn publication_identity_parse_v1(text: &str) -> (Context, FuncOp) {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    let operation = parse_from_str(Operation::top_level_parser(), &mut context, text).unwrap();
    (context, FuncOp::from_operation(operation))
}

fn publication_identity_capture_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<BuiltIdentityV1, ()> {
    LivePlironStructuralIdentityProviderV1::new(context, function)
        .capture_with_resource_limits_v1(
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .map(|capture| capture.snapshot)
        .map_err(|_| ())
}

#[test]
fn publication_identity_encodes_three_exact_markers_and_native_guard() {
    use dialect_kernel::PublicationAtomicAccessAttr;
    let (context, function) = publication_identity_parse_v1(publication_identity_text_v1());
    verify_operation(function.get_operation(), &context).unwrap();
    let captured = publication_identity_capture_v1(&context, &function).unwrap();
    for (marker, expected) in [
        (
            PublicationAtomicAccessAttr::ReleaseRequestU32,
            "u32:release:system:store=1:results=0",
        ),
        (
            PublicationAtomicAccessAttr::ReleaseReadyU32,
            "u32:release:system:store=2:results=0",
        ),
        (
            PublicationAtomicAccessAttr::AcquireU32,
            "u32:acquire:system:result=index-zext-u32",
        ),
    ] {
        let attribute: AttrObj = Box::new(marker);
        assert_eq!(
            render_attribute(&context, &attribute, PlironPreserveLocationV1::Function).unwrap(),
            ("kernel.publication_atomic".to_owned(), expected.to_owned())
        );
        assert!(
            captured
                .identity
                .canonical
                .windows(expected.len())
                .any(|bytes| bytes == expected.as_bytes())
        );
    }
    assert!(
        !captured
            .identity
            .grants_operational_semantics_or_refinement_authority()
    );
    assert_real_capture_exact_and_one_under_v1(&context, &function);
}

#[test]
fn publication_identity_preserves_marker_values_and_alpha_renaming() {
    let text = publication_identity_text_v1();
    let (context, function) = publication_identity_parse_v1(text);
    let original = publication_identity_capture_v1(&context, &function).unwrap();
    let (renamed_context, renamed) =
        publication_identity_parse_v1(&text.replace("acquired", "observed_word"));
    let renamed = publication_identity_capture_v1(&renamed_context, &renamed).unwrap();
    assert!(original.identity.exactly_matches(&renamed.identity));
    let (mutated_context, mutated) =
        publication_identity_parse_v1(&text.replace("ReleaseRequestU32", "ReleaseReadyU32"));
    let mutated = publication_identity_capture_v1(&mutated_context, &mutated).unwrap();
    assert!(!original.identity.exactly_matches(&mutated.identity));
}

#[test]
fn publication_identity_rejects_native_order_scope_kind_result_and_guard_mutants() {
    let text = publication_identity_text_v1();
    for (from, to) in [
        (
            "kernel.atomic_ordering Release",
            "kernel.atomic_ordering Relaxed",
        ),
        (
            "kernel.atomic_ordering Acquire",
            "kernel.atomic_ordering Release",
        ),
        (
            "kernel.atomic_scope System",
            "kernel.atomic_scope Workgroup",
        ),
        ("kernel.access_kind AtomicWrite", "kernel.access_kind Write"),
        ("ReleaseReadyU32", "AcquireU32"),
        ("(cell, extent, acquired)", "(cell, extent, cell)"),
        (
            "acquired = kernel.access",
            "acquired, surplus = kernel.access",
        ),
    ] {
        let mutant = if from == "acquired = kernel.access" {
            text.replace(from, to).replace(
                "kernel.publication_atomic AcquireU32]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> (kernel.index)>",
                "kernel.publication_atomic AcquireU32]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> (kernel.index, kernel.index)>",
            )
        } else {
            text.replace(from, to)
        };
        let (context, function) = publication_identity_parse_v1(&mutant);
        assert!(
            verify_operation(function.get_operation(), &context).is_err(),
            "{from}"
        );
        assert!(
            publication_identity_capture_v1(&context, &function).is_err(),
            "{from}"
        );
    }
}

#[test]
fn publication_identity_rejects_wrong_attribute_key_type_and_unknown_variant() {
    let text = publication_identity_text_v1();
    for mutant in [
        text.replace("kernel_publication_atomic:", "unrelated_marker:"),
        text.replace(
            "kernel.publication_atomic ReleaseRequestU32",
            "builtin.string \"ReleaseRequestU32\"",
        ),
    ] {
        let (context, function) = publication_identity_parse_v1(&mutant);
        assert!(publication_identity_capture_v1(&context, &function).is_err());
    }
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    assert!(
        parse_from_str(
            Operation::top_level_parser(),
            &mut context,
            &text.replace("ReleaseRequestU32", "ReleaseUnknownU32"),
        )
        .is_err()
    );
}
