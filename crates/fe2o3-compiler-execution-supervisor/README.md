# fe2o3 compiler-execution supervisor

This package owns the protected process boundary around the static
compiler-execution issuer. Its first checkpoint implements authority-free
program admission: it authenticates the provisioned static launcher and issuer
before any durable root or signing key can enter the supervisor state.

Both source images are read through stable file descriptions, checked against
exact SHA-256 and length measurements, validated as loader-independent x86-64
ELF images, copied into distinct anonymous mode-0555 memfds, sealed with
`WRITE`, `GROW`, `SHRINK`, `EXEC`, and `SEAL`, reopened read-only, and measured
again. The issuer must match the exact executable and runtime measurements in
the sealed caller policy. The launcher measurement belongs to trusted service
provisioning and is never accepted in a per-launch request.

The admitted program is move-only and exposes no descriptor. It has no signing,
publication, load, launch, or GPU authority. Later checkpoints bind it to the
service credential profile, durable root, signing key, client handoff, static
pre-exec manifest, `clone3` pidfd, and authenticated readiness lifecycle.
