use super::*;

#[test]
fn exact_limit_is_not_marked_truncated() {
    let mut output = BoundedText::new(4);
    assert!(output.write_str("abcd").is_ok());
    assert_eq!(output.finish(), "abcd");
}

#[test]
fn zero_limit_stops_nonempty_output() {
    let mut output = BoundedText::new(0);
    assert!(output.write_str("").is_ok());
    assert!(output.write_str("x").is_err());
    assert_eq!(output.finish(), " [truncated]");
}

#[test]
fn truncation_does_not_split_utf8() {
    let mut output = BoundedText::new(2);
    assert!(output.write_str("a\u{e9}").is_err());
    assert_eq!(output.finish(), "a [truncated]");
}

#[test]
fn truncation_is_sticky() {
    let mut output = BoundedText::new(1);
    assert!(output.write_str("ab").is_err());
    assert!(output.write_str("").is_err());
    assert!(output.write_str("later").is_err());
    assert_eq!(output.finish(), "a [truncated]");
}

#[test]
fn fallible_formatting_stops_debug_traversal() {
    struct Count<'a>(&'a std::cell::Cell<usize>);
    impl fmt::Debug for Count<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for _ in 0..1000 {
                self.0.set(self.0.get() + 1);
                f.write_str("x")?;
            }
            Ok(())
        }
    }
    let calls = std::cell::Cell::new(0);
    let mut output = BoundedText::new(8);
    assert!(write!(output, "{:?}", Count(&calls)).is_err());
    assert_eq!(calls.get(), 9);
    assert_eq!(output.finish(), "xxxxxxxx [truncated]");
}

#[test]
fn type_ids_are_unique_and_bounded_by_recorded_projections() {
    let mut projections = [None; MAX_PROJECTIONS];
    for (index, projection) in projections.iter_mut().enumerate() {
        *projection = Some(TypedProjection {
            kind: SemanticProjectionKindV1::Field(0),
            input: index as u32,
            output: index as u32 + 1,
        });
    }
    assert_eq!(
        observed_types(0, 100, &projections),
        [0, 100]
            .into_iter()
            .chain(1..=MAX_PROJECTIONS as u32)
            .collect::<Vec<_>>()
    );
    let first = projections[0];
    projections.fill(first);
    assert_eq!(observed_types(0, 0, &projections), vec![0, 1]);
}
