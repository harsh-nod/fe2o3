# G7 fixture runner

Run `scripts/device-link/run.sh` with one evidence class:

- `cpu` runs the independent CPU oracle against a boundary emulation.
- `compile` checks the typed-admission crate and Rust device FFI macros. It
  does not assemble or link the external LLVM IR.
- `hardware` exits 77 with `UNAVAILABLE` until G5/G6 provide authenticated
  publication and the typed runtime consumes that authority.
- `all` requires CPU and compile checks, then preserves the hardware result. It
  exits 77 while hardware evidence is explicitly unavailable.

The runner does not invoke COMGR, `llvm-link`, `ld.lld`, or another command-line
linker. A later hardware implementation must consume the supervised direct-LLVM
artifact and bind its exact target, contract-set identity, payload digest, and
observed context before loading.
