# Joined Native Compiler And Application Manager

Status: implemented candidate undergoing CPU qualification. No genuine protected
compiler-to-application end-to-end pass, effective systemd-unit qualification,
GPU launch, or milestone closure is established by this document or the observer
tests.

## One Original Request

The explicit `--native-application-proof-custodian` Cargo route uses one installed
`fe2o3-native-application-manager`, started **before compilation** by an approved
external whole-cgroup custodian. It is not paired with a concurrently running
standalone `fe2o3-compiler-execution.service` or its V1 counterpart. Those services
compete for the same fixed compiler listener and lifecycle resources. The manager
does not stop other services or bypass their locks; conflicting startup refuses.

The closed sequence is:

1. Admit the actual fixed native compiler, proof, and running manager installations
   on one original request account. Open the fixed compiler listener
   `/run/fe2o3/compiler-execution-supervisor.sock` and accept one protected request.
2. Retain the original compiler completion and receiver while consuming actual
   compiler, issuer, trace, and anchor retirement. Check every original cleanup
   cell is empty and its retained storage is zero. Keep that same controller,
   deployment guard, prepaid final shutdown, and spent cleanup account alive.
3. For a nonzero or signaled compiler, shut down that original pool permanently
   and deliver the exact original terminal result. Do not start or require an
   application supervisor, application socket, or application Ready.
4. For a successful compiler, use the same controller to prepare the actual
   application anchor and supervisor. Only the final nonroot supervisor listens
   on `/run/fe2o3/native-application-supervisor.sock`. Its authenticated Ready and
   same-installation revalidation precede sending the original compiler completion.
5. Cargo then uses the existing native registration, original descriptors,
   currentness, proof-controller custody, and exact ACK/EOF path. The manager
   retains original owners through application retirement and final pool shutdown.

This is not cleanup-service reset or re-admission. A dropped pending completion
does not count as delivery. Failure or unwind with an open creator scope remains
fail-stop; the external custodian must terminate its entire service domain.
The inherited standalone coordinator API retains its separate terminal contract.

## Qualification Order

Install only independently approved fixed V3 compiler records/images and native
proof records using their existing measured provisioning paths. The semantic
policy must be independently pinned; exported candidate bytes or readiness do not
approve themselves. The worker and exact functional-refinement runtime must also
be admitted. Do not print signing seeds, relax source policies, or install a PR
runner persistently on a shared host.

Bootstrap source-policy inputs with the separate build-only protected compiler
route and explicit `--export-native-policy-inputs` selection before installing
the native proof profile. Its actual-owner export remains an authority-free
candidate. Independently review and pin both source packet and roster before
`export-native-policy-candidate`, then independently approve/pin the resulting
policy for `inspect-native-fixed-resources` and `install-native`. The bootstrap
standalone compiler and its original Cargo invocation must fully retire before
starting the joined manager. Do not retain its anchor as a shortcut. Source/roster
stability across bootstrap and application builds must be checked by actual
native admission, not assumed from matching filenames.

For the installed-systemd GPU harness, an administrator starts the manual-only
manager unit once under its reviewed effective cgroup/profile contract, with
standalone compiler services stopped and no stale application socket. Then run
`scripts/qualify_native_conditional_application.py` with its explicit pinned Cargo
binary, exact producer spelling, selected device/roster, and owned output paths.
The observer makes one bounded `systemctl show` request, requires a running native
manager and inactive standalone compiler units, observes the compiler socket and
absence of the application path, then invokes Cargo exactly once. It never probes
a one-shot listener by connecting, polls for readiness, starts/stops services, or
removes socket paths. `Type=exec`, unit state, socket metadata, and these observations
are not admission or quiescence authority; actual protected protocols remain decisive.
If prerequisites are not yet observable, fix the owned deployment before a new
attempt; do not make the observer retry into authority.

The application socket is intentionally **not** a pre-compilation prerequisite.
It appears only after the original successful compiler has retired. The result
oracle still refuses legacy families, incomplete output, failed Cargo, or missing
native cleanup/proof identities. `rootServiceCleanupQualified` stays false: an
application result alone does not prove root service or whole-cgroup cleanup.

For disposable private-namespace CPU qualification, use the same sequence with
an independently approved external cgroup custodian, not host systemd as an
assumed prerequisite. `scripts/qualify_native_conditional_cpu_chain.py` exposes
only the inert order and proof-only result checks; it does not provision, launch
the manager, or establish successful execution. A gfx950 host can execute the
closed gfx942 compiler/proof chain without GPU admission, but this still requires
genuine source provenance, approved policy/tool installations, original root
service custody, and a separately observed terminal cleanup outcome.

`scripts/qualify-native-static-cpu.sh` additionally executes the exact newly built
manager ELF as the original nonroot caller and requires its specific root-role
refusal, not an arbitrary failure. It retains the executable FD, checks unchanged
path/content identities and bounds/reaps its child. It also runs selected musl
compiler-phase, cleanup-checkpoint, actual CPU-child cancellation and creator
fail-stop controls, plus the private installed-record tests. These cover the
packaged refusal boundary and component mechanics, not a successful protected
compiler-to-manager transition. Both `joinedProtectedPhaseQualified` and
`rootServiceCleanupQualified` remain false until the genuine positive obligations
are exercised and retained independently.
