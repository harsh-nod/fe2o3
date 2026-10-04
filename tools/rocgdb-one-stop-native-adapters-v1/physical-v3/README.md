# Disabled host-entry maintenance — physical-v3 source overlay

This separate GPL source overlay inherits the exact disabled physical-v2 package. It changes four standalone native hooks to retain a narrowly scoped same-owner host-entry maintenance epoch. It does not change the V2 wire format, activate a debugger, supply a runtime profile or launch a kernel. Selection, capture and publication remain literally false. The separate Rust physical-v2 consumer remains unbound.

The existing commit effect is accepted only after the initial entry commit has armed retained actual process/top target references, while the same inferior/host, target stack, owner, execution flags, unloaded runtime and closed thread roster remain valid before and after the effect. No new resume, selector, command, ACK, row, stop, retry or deadline reset is granted. Epoch retirement is permanent. Normal reference release is checked to be nonfinal while the stack still owns references; after drift/in-flight failure, bounded references may instead remain until debugger destruction and can delay target close.

SOURCE-CONTRACT.md specifies the one combined 63-row source contract. Its measured final size is 2,183,132 bytes, exceeding the old v2 limit by 20,444 bytes. The new explicit source-only cap is 2,144 KiB (2,195,456 bytes), with 12,324 bytes spare. Per-file 512 KiB and metadata 64 KiB remain unchanged. All native caps—including 65,536-byte logical storage, 131,072 work units, existing read/API/packet limits and deadlines—remain unchanged.

Read-only source verification requires an explicit canonical absolute source root and stage:
`node verify-source.mjs <source-root> <physical-publication-disabled-v2|physical-host-entry-maintenance-disabled-v3>`.
The two stages use the SAME 63-path cumulative reader. The parent’s original three-stage verifier remains unchanged. A passed selected-source check is not a full-checkout, compiled ABI, loaded debugger, ownership or hardware qualification.

The package includes one exact four-file patch, four changed standalone hooks, source contracts and CPU controls. No upstream checkout, acquisition, patch application, installation, debugger execution or native retry automation is included. Unchanged parent sources and licenses remain exact dependencies.

Metadata controls are `tests/source-files-tests.mjs`. Placement controls require `FE2O3_ROCGDB_TEST_SOURCE` to name the exact final-false selected source tree. The strict C++17 `tests/maintenance.cc` harness uses exact published helper bodies with hostile mock GDB object graphs, the unchanged parent owner-core header, and the exact external `gdbsupport/gdb_ref_ptr.h`. Add the parent `src`, this `tests`, and the selected source `gdbsupport` directories to the compiler include path. No test invokes GDB, a GPU, or a target process.

The private source-identical helper fixture previously passed 100 groups / 481 checks; that is not a qualification of this relocated package, a real GDB ABI, the exact prior native failure cause, or successful capture. Relocated controls, false projection and package integration require fresh root qualification. See SOURCE-CONTRACT.md for extraction, ordinary-source inversion, resource and target-close limitations. Publishing this overlay does not close an outstanding native milestone or change accepted-exit counts.
