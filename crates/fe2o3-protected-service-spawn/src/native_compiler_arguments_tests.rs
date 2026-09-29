use super::*;

fn strings(values: &[&str]) -> Vec<CString> {
    values
        .iter()
        .map(|value| CString::new(*value).unwrap())
        .collect()
}

#[test]
fn exact_strings_and_null_tables_survive_source_drop_and_owner_move() {
    fn send<T: Send>() {}
    send::<CompilerArguments>();
    let arguments = strings(&["original-argv0", "--cfg", "", "--cfg", "two words"]);
    let environment = strings(&["A=", "B=x=y", "C=two words"]);
    let owner = CompilerArguments::copy(&arguments, &environment).unwrap();
    assert_eq!(owner.arguments(), arguments);
    assert_eq!(owner.environment(), environment);
    for (source, retained) in arguments.iter().zip(owner.arguments()) {
        assert_ne!(source.as_ptr(), retained.as_ptr());
    }
    drop(arguments);
    drop(environment);
    let moved = Box::new(owner);
    for (values, table) in [
        (&moved.arguments, &moved.argv),
        (&moved.environment, &moved.envp),
    ] {
        assert_eq!(table.len(), values.len() + 1);
        assert_eq!(table.last(), Some(&0));
        for (value, pointer) in values.iter().zip(table) {
            assert_eq!(*pointer, value.as_ptr().expose_provenance());
        }
    }
    assert_eq!(moved.argv(), moved.argv.as_ptr());
    assert_eq!(moved.envp(), moved.envp.as_ptr());
    assert!(moved.retained_storage().unwrap() <= CompilerArguments::MAX_STORAGE);
}

#[test]
fn empty_environment_has_only_its_terminator_without_inheritance() {
    let owner = CompilerArguments::copy(&strings(&[""]), &[]).unwrap();
    assert_eq!(owner.arguments()[0].as_bytes(), b"");
    assert!(owner.environment().is_empty());
    assert_eq!(owner.envp, [0]);
}

#[test]
fn count_limits_are_closed_before_copying() {
    let arguments = vec![CString::new("").unwrap(); MAX_ARGUMENTS];
    let environment = vec![CString::new("X=").unwrap(); MAX_ENVIRONMENT];
    let owner = CompilerArguments::copy(&arguments, &environment).unwrap();
    assert_eq!(owner.argv.len(), MAX_ARGUMENTS + 1);
    assert_eq!(owner.envp.len(), MAX_ENVIRONMENT + 1);
    assert!(owner.retained_storage().unwrap() <= CompilerArguments::MAX_STORAGE);
    assert!(CompilerArguments::copy(&[], &environment).is_err());
    assert!(
        CompilerArguments::copy(&vec![CString::new("").unwrap(); MAX_ARGUMENTS + 1], &[]).is_err()
    );
    assert!(
        CompilerArguments::copy(
            &arguments,
            &vec![CString::new("").unwrap(); MAX_ENVIRONMENT + 1]
        )
        .is_err()
    );
}

#[test]
fn byte_limit_includes_every_terminator_and_both_tables() {
    let exact = [CString::new(vec![b'a'; MAX_BYTES - 1]).unwrap()];
    let owner = CompilerArguments::copy(&exact, &[]).unwrap();
    assert_eq!(owner.arguments()[0].as_bytes_with_nul().len(), MAX_BYTES);
    assert!(owner.retained_storage().unwrap() <= CompilerArguments::MAX_STORAGE);
    assert!(CompilerArguments::copy(&exact, &strings(&[""])).is_err());
    assert!(CompilerArguments::copy(&[CString::new(vec![b'a'; MAX_BYTES]).unwrap()], &[]).is_err());
}
