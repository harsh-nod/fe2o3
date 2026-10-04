# AMD public ISA metadata attribution

The generated CDNA 3 and CDNA 4 metadata is derived from the two public XML
members pinned by `manifest.json` in AMD's public
[2026-08-06 ISA archive](https://gpuopen.com/download/AMD_GPU_MR_ISA_XML_2026_08_06.zip).
Both XML `Document` headers explicitly declare `License=MIT`, `AMD Public Use.`,
schema `1.1.1`, release date `2026-02-20`, and the following copyright.
The archive filename's date and the XML release date are distinct observations.
The generator omits `Description` prose; all other metadata is retained.

Copyright (c) 2026 Advanced Micro Devices, Inc., or its affiliates.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

The standard full MIT text above accompanies the XML's explicit MIT license
designation. The pinned XML bytes, not this attribution, are the metadata
input. This attribution does not confer compiler, proof, artifact, or hardware
authority. fe2o3-authored generator and query code retains ordinary repository
licensing terms.
