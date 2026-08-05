// This macro is the single executable vecadd algorithm body. It is expanded by
// both ordinary rustc in `src/lib.rs` and Verus in `verus/vecadd.rs`.
macro_rules! vecadd_thread_body {
    (
        $domain:ident,
        $thread:ident,
        $a:ident,
        $b:ident,
        $output:ident,
        $write_index:ty,
        $mismatch:path,
        $overflow:path
    ) => {{
        if $thread.domain().len() != $domain.len()
            || $a.len() != $domain.len()
            || $b.len() != $domain.len()
            || $output.len() != $domain.len()
        {
            return Err($mismatch);
        }

        let write = match <$write_index>::new($thread, $output.len()) {
            Some(write) => write,
            None => return Err($mismatch),
        };
        let index = write.index().value();
        if $a[index] > u32::MAX - $b[index] {
            return Err($overflow);
        }
        $output[index] = $a[index] + $b[index];
        Ok(())
    }};
}
