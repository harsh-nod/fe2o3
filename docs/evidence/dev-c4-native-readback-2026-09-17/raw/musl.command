env CARGO_INCREMENTAL=0 cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib --target x86_64-unknown-linux-musl -- --test-threads=1 
