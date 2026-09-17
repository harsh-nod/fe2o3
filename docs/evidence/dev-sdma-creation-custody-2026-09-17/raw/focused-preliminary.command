prlimit --core=0:0 -- env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-kfd --all-features --lib sdma_creation -- --test-threads=4 
