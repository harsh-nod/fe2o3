 stable_retained_read_count_body {
    ($contents:ident) => { $contents.leases.len() };
}
