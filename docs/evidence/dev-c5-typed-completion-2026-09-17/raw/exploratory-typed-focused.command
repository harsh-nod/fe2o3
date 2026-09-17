env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked --offline -p fe2o3-runtime -p fe2o3-host --all-features --lib completion -- --test-threads=1 
