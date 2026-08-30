fn main() {
    if let Err(error) =
        fe2o3_compiler_execution_coordinator::run_compiler_execution_reference_provisioner_v1()
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
