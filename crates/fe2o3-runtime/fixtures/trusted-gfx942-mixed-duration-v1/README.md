# Fixed-Work Scheduling Fixtures

Two qualification-only gfx942:xnack- code objects implement the same wrapping
u32 recurrence with literal bounds: 257 iterations (`mixed_short`) and
33,554,433 iterations (`mixed_long`). Each launch has exactly one Wave64
workgroup and one 384-byte HostVisible ReadWrite binding. Its 64 payload words
are surrounded by two 64-byte guards. The pointer addresses the whole binding;
`data.len = 64` counts payload words, whose first word is at byte offset 64.

The feature-gated runtime gate checks a closed artifact/name tuple, policy
signature, geometry, both kernarg templates, ABI/effects, exact input/guards and
retained digest. It grants only ordinary qualification launches. No dynamic
work count, protected generated authority, compiler acceptance, atomic or
collective semantics are admitted. Existing qualification gates are unchanged.

Rust computes the entire expected image by affine repeated squaring. The
independent Python oracle uses modular geometric summation with exact integer
division. Both require all 384 bytes, including guards; CPU tests compare all
three independently generated images and reject every single-byte corruption.

Run `bash build-and-verify.sh` in this directory with the pinned ROCm 7.2.0
compiler, linker and disassembler. It rebuilds both objects byte-identically in
an exact-owned temporary directory, then runs `python3 -I -B oracle.py check`.
The selected tool hashes are identities, not a complete compiler closure.
The object disassembly retains the literal counter and recurrence backedge.
This inspection and reproducible construction are not machine-code refinement.

Two ignored native tests exist in `retained_release_tests/mixed_duration.rs`.
First qualify each artifact's full-buffer correctness and backing refund. Only
then run the owner-thread two-stream canary. It requires a retained long native
receipt before short enqueue and an actual incomplete long completion signal
after short success, plus exact returned submission identities, full outputs,
and owner-thread cleanup. Timing misses fail after draining rather than being
reported as concurrency success. Neither test is a device-timeline producer.

Fixed work does not guarantee duration. Native correctness, out-of-order
completion and physical overlap are distinct gates. These fixtures alone do
not close SCALE-1, SCALE-2, SCALE-3, protected execution or HIP/HSA parity.
