#[allow(unsafe_code)]
fn main() -> std::process::ExitCode {
    // SAFETY: this dedicated executable calls the entrypoint exactly once on its
    // main thread, before creating threads, handlers or owners for FDs 3..=16.
    // No application work, retry or fallback follows return/unwind; the process
    // terminates. The remaining preconditions are EXTERNAL deployment obligations:
    // an independently trusted administrator must establish this host-root process
    // in its actual user/mount/cgroup context, the unique activation inputs, and
    // the approved-helper/authenticated-peer role binding and separation. Keep
    // the creator and original privileged cleanup controller alive through all
    // in-process cleanup, outside every child domain, with exclusive consuming
    // waits and no competing FD, credential, signal, namespace/map, cgroup or
    // approved-backing mutation. Children, executed images and descendants must
    // not obtain cgroup controls, migrate, delegate or create child cgroups.
    // An actual outside service-manager custodian must retain whole-service-domain
    // termination responsibility after every exit, including fail-stop/unwind.
    // This main and its comments cannot prove administrator provenance or admit
    // the effective unit, containment or installed image. KillMode=mixed text,
    // numeric root, activation metadata and fixed approval are not that proof;
    // paired deployment qualification remains required before using this route.
    match unsafe {
        fe2o3_compiler_execution_coordinator::run_inherited_compiler_execution_coordinator_v3()
    } {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
