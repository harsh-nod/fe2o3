# Third-party software and notices

Except for the third-party-derived material identified below, fe2o3-authored
source is available under either Apache-2.0 or MIT, at the user's option. See
`LICENSE-APACHE` and `LICENSE-MIT`. A file-specific notice takes
precedence for the material it identifies.

## Contributor Covenant 2.1

`CODE_OF_CONDUCT.md` is adapted from Contributor Covenant 2.1.

- Upstream: <https://github.com/EthicalSource/contributor_covenant/tree/2.1>
- Audited tag: `2.1`
- Commit: `8a3be1350b07f38b53bbc7073f765a48c4c53ce1`
- Tree: `e710a202f5b468353eb7782b212d438d7f2831dd`
- Upstream `CODE_OF_CONDUCT.md` SHA-256:
  `606ffdda2de576d2c12434301ff5b03bd13f44373b868fba8530bc22af0205a6`
- Upstream `LICENSE.md` SHA-256:
  `030cdb4fdfb7a9dfdbb87112fee428c835ae9f0757347ea8b5619434b8c2d331`
- License: [Creative Commons Attribution 4.0 International][CC-BY-4.0]

The fe2o3 `CODE_OF_CONDUCT.md` adaptation is distributed under CC BY
4.0. That file-specific license does not change the license of other fe2o3
files.

The audited project identifies the work as Contributor Covenant and states that
it is managed by the Organization for Ethical Source. The fe2o3 adaptation
changes the private enforcement contact and reporting instructions, Markdown
formatting, line wrapping, and minor punctuation. The attribution and change
notice also appear in `CODE_OF_CONDUCT.md`.

## AMD KFD and DRM UAPI-derived definitions

The following crates contain reviewed Rust transcriptions or adaptations of
public UAPI records and constants:

- `crates/fe2o3-kfd-uapi`
- `crates/fe2o3-drm-uapi`

Their crate READMEs and schema manifests are the authoritative, machine-checked
provenance records. Their fe2o3-authored request encoders implement public
Linux `_IOC` ABI facts and are distinguished below from those transcribed
records and constants. The primary source inputs are:

| Input | Exact reviewed source | SHA-256 | Notice |
| --- | --- | --- | --- |
| KFD UAPI 1.18 | `amdgpu-dkms` `1:6.16.13.30300400-2341068.24.04`, `include/uapi/linux/kfd_ioctl.h` | `b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d` | AMD MIT-style notice below |
| Generic Linux ioctl ABI reference | `linux-libc-dev` `6.8.0-137.137`, `/usr/include/asm-generic/ioctl.h` | `76396e5537d75285c3ca20e3b6a79b101eebfdc14d39c104ff7eab778672160e` | `GPL-2.0 WITH Linux-syscall-note`; no header source is copied into the Rust encoder |
| Core DRM UAPI | `linux-headers-6.8.0-124` `6.8.0-124.124`, `include/uapi/drm/drm.h` | `3ab6ac01bf91067aed96b70d7fa7847a86e7f726d74278151f085143688659cc` | Core DRM MIT-style notice below |
| AMDGPU DRM UAPI | `amdgpu-dkms` `1:6.16.13.30300400-2341068.24.04`, `include/uapi/drm/amdgpu_drm.h` | `9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc` | AMDGPU MIT-style notice below |

The DRM crate also checks independent exported and libdrm copies:

- `linux-libc-dev` `6.8.0-137.137`, exported `drm.h`,
  SHA-256
  `6b80aff056e2ac2e126e5144a3ce2c750292edb4d080d4689ac487dc17e4dae8`;
- `libdrm-dev` `2.4.125-1ubuntu0.1~24.04.2`, `drm.h`,
  SHA-256
  `e97d535df3d33844a7c66578cb5adb501c57d17fb5ba55395309d1f275432060`;
  and
- the same libdrm package's `amdgpu_drm.h`, SHA-256
  `2881120496c69fc2154e590d0bc6e615a48adc43df1a658dd8cd8f78ec648557`.

Additional AMDGPU DKMS implementation files are named and hashed in the crate
schema manifests as semantic review evidence. Those files are not copied
wholesale into the fe2o3 source archive; each remains governed by its upstream
per-file notice.

### AMD KFD and AMDGPU header notices

The reviewed KFD header states:

> Copyright 2014 Advanced Micro Devices, Inc.

The reviewed AMDGPU DRM header states:

> Copyright 2000 Precision Insight, Inc., Cedar Park, Texas.
>
> Copyright 2000 VA Linux Systems, Inc., Fremont, California.
>
> Copyright 2002 Tungsten Graphics, Inc., Cedar Park, Texas.
>
> Copyright 2014 Advanced Micro Devices, Inc.

It also identifies Kevin E. Martin, Gareth Hughes, and Keith Whitwell as
authors. Both headers carry this permission notice:

> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> COPYRIGHT HOLDER(S) OR AUTHOR(S) BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

### Core DRM header notice

The reviewed core DRM header identifies Rickard E. (Rik) Faith as author and
Richard Henderson in its acknowledgments, and states:

> Copyright 1999 Precision Insight, Inc., Cedar Park, Texas.
>
> Copyright 2000 VA Linux Systems, Inc., Sunnyvale, California.
>
> All rights reserved.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice (including the next
> paragraph) shall be included in all copies or substantial portions of the
> Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL VA
> LINUX SYSTEMS AND/OR ITS SUPPLIERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

### Linux generic ioctl notice

The reviewed `asm-generic/ioctl.h` declares
`GPL-2.0 WITH Linux-syscall-note`. The complete
[GPL-2.0 text][Linux-GPL-2.0] and [Linux syscall exception][Linux-syscall-note]
are maintained in the Linux source tree. The exception states:

> NOTE! This copyright does *not* cover user programs that use kernel services
> by normal system calls - this is merely considered normal use of the kernel,
> and does *not* fall under the heading of "derived work".

The linked exception is controlling and should be read in full.

The KFD Rust encoder first appeared as a new file in fe2o3 commit
`40258e76b93829431c1c3542c792047e549dc705`; its documentation from
that introduction describes encoding requests "without libc or generated
bindings." The DRM encoder followed in commit
`c218f2ef02d0f087b8b3eeeb73f46f22c7873d78`. The repository history
contains Rust expressions implementing the reviewed bit widths, shifts, and
directions, not an imported or generated copy of `asm-generic/ioctl.h`.
Accordingly, this inventory treats the encoder as fe2o3-authored code
validated against public ABI facts. The Linux header and its license remain
listed as the exact review reference; this provenance statement does not
replace the upstream license notice.

## ROCr semantic comparison sources

Some KFD schemas compare behavior and numeric contracts with ROCr sources at
`ROCm/rocm-systems` commit
`97f5574fe2fdc7bef44fb01545347912ee9f1779`. Exact source paths and
SHA-256 values appear in the KFD crate README, manifests, and oracle scripts.
The audited `projects/rocr-runtime/LICENSE.txt` has SHA-256
`ffa5a77ce21419e276bd9068faec94333128e49e1c95426d9c1d35435e8fe835`
and contains this notice:

> The University of Illinois/NCSA Open Source License (NCSA)
>
> Copyright (c) 2014-2025, Advanced Micro Devices, Inc. All rights reserved.
>
> Developed by:
>
> AMD Research and AMD HSA Software Development
>
> Advanced Micro Devices, Inc.
>
> www.amd.com
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> with the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> - Redistributions of source code must retain the above copyright notice,
>   this list of conditions and the following disclaimers.
> - Redistributions in binary form must reproduce the above copyright notice,
>   this list of conditions and the following disclaimers in the documentation
>   and/or other materials provided with the distribution.
> - Neither the names of Advanced Micro Devices, Inc,
>   nor the names of its contributors may be used to endorse or promote
>   products derived from this Software without specific prior written
>   permission.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> CONTRIBUTORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## Release inventory boundary

The developer-preview release is a source-only Git archive. It does not vendor
Cargo dependencies, Rust toolchains, ROCm tools, Linux kernel components, or
other third-party binaries. Those components are obtained separately and
remain subject to their own licenses and notices. `Cargo.lock` records the
resolved Rust dependency versions but is not a license inventory.

The SPDX document attached to a developer-preview release inventories files in
the fe2o3 Git tree. It is intentionally a **source SBOM**, not a dependency or
binary SBOM. Because the tree contains material under file-specific third-party
terms, its package-level `licenseDeclared` is `NOASSERTION`. Do not use
that SBOM to represent the licenses or complete software bill of materials of a
downstream binary distribution.

Anyone redistributing a binary, vendored source tree, container, toolchain, or
appliance built with fe2o3 is responsible for identifying every included
third-party component and preserving its license and attribution terms. Before
fe2o3 publishes binaries, vendors dependencies, or enables crates.io packages,
the release process must add a reviewable dependency SBOM and generated notice
set for the exact release closure.

[CC-BY-4.0]: https://creativecommons.org/licenses/by/4.0/legalcode
[Linux-GPL-2.0]: https://github.com/torvalds/linux/blob/v6.8/LICENSES/preferred/GPL-2.0
[Linux-syscall-note]: https://github.com/torvalds/linux/blob/v6.8/LICENSES/exceptions/Linux-syscall-note
