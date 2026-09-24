# Authored two-wave LDS exchange (V22)

This bounded profile authors the complete gfx942:xnack- entry body in Rust:
kernarg loads, input load, explicit waits, LDS write, workgroup barrier, peer
read, output mask/store, EXEC restore and termination. The compiler does not
supply an executable setup or tail.

There are two separate public commands. The diagnostic command below exports
pre-ranked KIR/LLVM observations. The [checked command](physical-lds-exchange-checked-v22.md)
uses the normal source-owned ranked/formal and ABI continuation, then exports
inert LLVM and handoff bytes. Neither command runs a native worker or GPU,
exports source custody, or authorizes a launch.

## Source shape

The [complete fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/physical_lds_exchange_v22.rs)
contains two positive physical-register layouts and seventeen negative controls.
The root has exactly these logical arguments and a finite authored launch:

```rust
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_one(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        // This excerpt omits the explicit kernarg and global-address instructions.
        // Compile the linked complete fixture, not this excerpt.
        // ... global load into v8 and s_waitcnt_vmcnt0 ...
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        // ... explicit output address, mask, store, wait, restore, s_endpgm0 ...
    }
}
```

One workgroup contains 128 invocations, two Wave64 waves. Invocation x writes
input[x] to LDS[x], then reads LDS[x xor 64] after all participants publish their
completed writes. The output store consumes that actual ready peer result.
The profile is one authored block with 32 native instructions, not a general
barrier/loop/memory language.

The typed frame is offset 0, 512 bytes, alignment 4, publication epoch 1. It is
not an extra source parameter, an invented kernarg slot or an author-provided
LLVM attribute. The normal emitter derives its static LDS reservation from the
verified frame. The two slices still have four native components: pointer and
length pairs at kernarg offsets 0, 8, 16 and 24.

Physical register names are translated into actual SSA uses/definitions.
Pointer low/high/carry provenance is checked together; simulated pointer bits
are symbolic, not fabricated GPU addresses. Pending global and LDS values are
not usable until their matching explicit waits. LGKM completion is not workgroup
publication; the barrier is required separately and is LDS-only, not a global
memory happens-before claim.

All 128 input reads and LDS participants precede output masking. Empty output
therefore does not remove the full readable/initialized input requirement or
permit half a workgroup to skip the barrier.

## Public diagnostic command

Use the repository's pinned nightly 2026-04-03 and existing offline dependencies.
Set RUSTC to its absolute compiler path. No toolchain is installed by the script.

```sh
cargo build --locked --offline -p rustc-codegen-fe2o3 --lib --bin fe2o3-rustc-extract
node scripts/physical-lds-exchange-source-v22.mjs one /absolute/new-lds-diagnostic
node scripts/physical-lds-exchange-source-v22.mjs registers /absolute/new-lds-register-diagnostic
```

For a nondefault build directory, set FE2O3_PHYSICAL_LDS_EXCHANGE_BIN_DIR_V22
to its debug binary directory. Output directories must be new.

The wrapper invokes ordinary Cargo with
FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_LDS_EXCHANGE_DIRECTORY_V22. The real driver
authenticates live source/MIR39 and materializes exact KIR22. It writes
diagnostic/canonical-v22.bin, canonical.ll and native-observation-input-v22.txt.
The wrapper records source/binary/process pins and checks the LLVM/sidecar
relation. These are diagnostic file comparisons, not reconstruction of source
or compiler authority. This endpoint itself does not run ranked/formal checks.

## Exact source controls

Both public commands accept the same nineteen case names:

- Positives: one, registers.
- Launch: wrong-launch, dynamic-grid.
- Typed frame: frame-base, frame-size, frame-alignment, frame-epoch.
- Removed rows: missing-write-wait, missing-barrier, missing-read-wait, missing-vm.
- Data/address lineage: wrong-peer, wrong-lds-address, wrong-store, wrong-carry.
- Source custody/census: foreign-input, foreign-marker, mixed-marker.

Use a separate fresh directory per command. A named rejection must be an actual
nonzero compiler exit with the precise expected diagnostic and no extraction
output. A signal, panic-path substitute, unrelated failure or synthetic response
does not satisfy the control.

The removed-row examples currently fail the earlier block/operation-count
boundary. They do not pretend to isolate the later readiness check; separate
same-count canonical/formal mutations test those relations. The foreign-input
fixture uses a fixed foreign constant slice and fails actual root-argument
transport, not a reachable slice-bounds panic.

## CPU/debug and qualification boundary

The typed CPU simulator models global input, global output and one workgroup LDS
allocation, explicit pending/wait state and all 128 barrier arrivals. The
specialized capture library records observational forward/reverse state, not
resumable hardware execution. Generic debug capture remains refused with zero
records. This page does not promise a public V22 debugger CLI or viewer.

See [dated qualification](physical-lds-exchange-qualification-20260924.md) for
separately retained actual-source runs. New public wrapper qualification and
unchanged-source native O0/O3 checks are separate gates; they must not be
inferred from component tests.

Each wrapper limits an individual child to 300 seconds and its whole workflow
to 900 seconds, with post-close/decode/hash/publication deadline checks. It
limits combined child streams to 16 MiB and observes at most 1 GiB of fresh
output. These are bounded engineering observations, not whole-process-family
cleanup, full toolchain attestation, memory reservation or sandbox guarantees.
A receipt file alone is not acceptance: require the completed command exit 0.
No GPU execution, physical register sampling, host admission or milestone
completion is claimed.
