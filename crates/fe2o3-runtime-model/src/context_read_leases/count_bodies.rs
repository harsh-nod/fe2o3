macro_rules! stable_retained_read_count_body {
    ($contents:ident) => { $contents.leases.len() - $contents.free_reads.len() };
}
