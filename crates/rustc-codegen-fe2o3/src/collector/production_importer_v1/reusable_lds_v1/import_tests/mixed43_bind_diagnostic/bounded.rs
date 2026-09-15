use std::fmt;

pub(super) const MAX_SITES: usize = 64;
const MAX_OUTPUT: usize = 128 * 1024;

pub(super) struct Output(String);
impl Output {
    pub(super) fn new() -> Self {
        Self(String::with_capacity(MAX_OUTPUT))
    }
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Write for Output {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > MAX_OUTPUT - self.0.len() {
            return Err(fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}

pub(super) fn exact_index<T: Copy + Eq>(
    identities: impl IntoIterator<Item = T>,
    expected: T,
    maximum: usize,
) -> Result<usize, &'static str> {
    let mut selected = None;
    for (index, identity) in identities.into_iter().enumerate() {
        if index >= maximum {
            return Err("definition roster ceiling");
        }
        if identity == expected && selected.replace(index).is_some() {
            return Err("duplicate exact definition");
        }
    }
    selected.ok_or("reported identity absent; no nominal or shape fallback")
}

pub(super) fn insert_site<T: Ord>(sites: &mut Vec<T>, site: T) -> Result<(), &'static str> {
    match sites.binary_search(&site) {
        Ok(_) => Err("duplicate incoming source site"),
        Err(index) if sites.len() < MAX_SITES => {
            sites.insert(index, site);
            Ok(())
        }
        Err(_) => Err("incoming source-site ceiling"),
    }
}

#[test]
fn definition_selection_has_no_shape_or_duplicate_fallback() {
    assert_eq!(exact_index([1, 2, 3], 2, 3), Ok(1));
    assert_eq!(
        exact_index([1, 2, 3], 4, 3),
        Err("reported identity absent; no nominal or shape fallback")
    );
    assert_eq!(
        exact_index([2, 1, 2], 2, 3),
        Err("duplicate exact definition")
    );
    assert_eq!(
        exact_index([1, 2, 3], 2, 2),
        Err("definition roster ceiling")
    );
}

#[test]
fn incoming_sites_are_complete_sorted_unique_and_bounded() {
    let mut sites = Vec::with_capacity(MAX_SITES);
    for value in (0..MAX_SITES).rev() {
        insert_site(&mut sites, value).unwrap();
    }
    assert_eq!(sites, (0..MAX_SITES).collect::<Vec<_>>());
    assert_eq!(
        insert_site(&mut sites, 1),
        Err("duplicate incoming source site")
    );
    assert_eq!(
        insert_site(&mut sites, MAX_SITES),
        Err("incoming source-site ceiling")
    );
    assert_eq!(sites.len(), MAX_SITES);
}

#[test]
fn output_limit_is_bytes_and_never_silently_truncates() {
    use fmt::Write;
    let mut output = Output::new();
    output.write_str(&"a".repeat(MAX_OUTPUT)).unwrap();
    assert!(output.write_str("x").is_err());
    assert_eq!(output.as_str().len(), MAX_OUTPUT);
    assert!(output.write_str("").is_ok());
}
