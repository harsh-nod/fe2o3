fn main() {
    if fe2o3_external_anchor_service::run_inherited_external_anchor_service_v1().is_err() {
        std::process::exit(1);
    }
}
