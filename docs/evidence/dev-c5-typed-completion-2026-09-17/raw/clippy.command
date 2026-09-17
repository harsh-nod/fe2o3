env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --locked --offline -p fe2o3-host -p fe2o3-runtime --all-features --all-targets -- -D warnings 
