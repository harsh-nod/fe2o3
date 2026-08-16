# Host-link ELF fixtures

The `.hex` files preserve complete static x86-64 ELF executables as lowercase
hex so Git records their exact bytes without a binary patch:

- `minimal-static.hex`: 912 bytes, SHA-256
  `7ab5cb021cbe38abd72fffd64f20bcd71a981992df6dee0ddff8a5d4f0af7a5d`.
- `rust-static.hex`: 1000 bytes, SHA-256
  `a1fdb07712fd2acde1108bb3e9496dbe930243152b863cf648c20a6958c7fde6`.

They were generated from the adjacent sources with rustc 1.97.1 and its
bundled LLD 22.1.6. Both are static `ET_EXEC` files with no interpreter or
dynamic section. Both retain LLD's mergeable string `.comment` section with
`SHF_MERGE|SHF_STRINGS` and `sh_entsize=1`; the Rust fixture also retains
real `.eh_frame_hdr` and `.eh_frame` output.

The unit test verifies the size, digest, whole-file parser, and incremental
descriptor admission path for both fixtures. Compatibility matrices for real
inputs and additional tool outputs are opt-in so ordinary tests do not depend
on a particular host installation:

```bash
FE2O3_HOST_LINK_COMPAT_SYSROOT=/path/to/rustc-1.97.1-sysroot \
FE2O3_HOST_LINK_EXPECTED_RLIB_COUNT=27 \
FE2O3_HOST_LINK_COMPAT_ARCHIVES=/path/libgcc.a:/path/libstdc++.a:/path/liblldELF.a \
FE2O3_HOST_LINK_COMPAT_OUTPUTS=/path/minimal-static:/path/rust-static \
cargo test -p fe2o3-host-link-closure --lib selected_ --locked -- --nocapture
```

`FE2O3_HOST_LINK_COMPAT_ARCHIVES` and
`FE2O3_HOST_LINK_COMPAT_OUTPUTS` use the platform path-list separator.
