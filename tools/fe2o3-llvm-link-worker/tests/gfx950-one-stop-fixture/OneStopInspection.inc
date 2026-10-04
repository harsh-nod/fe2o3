// Exact selected ELF/descriptor/metadata join. No runtime address is produced.
struct Observation {
  uint64_t EntryOffset=0, EntryAddress=0, DescriptorOffset=0, DescriptorAddress=0;
  uint64_t NoteOffset=0, NoteBytes=0, TextOffset=0, TextBytes=0;
  uint32_t Rsrc1=0,Rsrc2=0,Rsrc3=0,Vgprs=0,Sgprs=0,Boundary=0;
  FixtureMetadata Metadata; std::vector<TraceRow> Trace;
  std::array<uint8_t,32> DescriptorDigest{};
};
bool within(size_t Total,uint64_t Offset,uint64_t Size) {
  return Offset<=Total && Size<=Total-Offset;
}
Expected<Observation> inspect(ArrayRef<uint8_t> Bytes,const Request &R,Mc &M) {
  if (Bytes.empty() || Bytes.size()>ArtifactCap) return refusal("artifact bound");
  auto Ordinary=inspectLinkedOutputForPublication(Bytes,R);
  if (!Ordinary) return Ordinary.takeError();
  auto O=object::ObjectFile::createObjectFile(MemoryBufferRef(
      StringRef(reinterpret_cast<const char*>(Bytes.data()),Bytes.size()),"<one-stop>"));
  if (!O) return O.takeError();
  auto *Elf=dyn_cast<object::ELFObjectFile<object::ELF64LE>>(O->get());
  if (!Elf) return refusal("ELF64LE required");
  const auto &F=Elf->getELFFile(); const auto &Header=F.getHeader();
  if (Header.e_machine!=ELF::EM_AMDGPU || Header.e_type!=ELF::ET_DYN ||
      Header.e_ident[ELF::EI_OSABI]!=ELF::ELFOSABI_AMDGPU_HSA ||
      Header.e_ident[ELF::EI_ABIVERSION]!=4 ||
      Header.e_flags!=(ELF::EF_AMDGPU_MACH_AMDGCN_GFX950 | ELF::EF_AMDGPU_FEATURE_XNACK_OFF_V4 |
                      ELF::EF_AMDGPU_FEATURE_SRAMECC_ANY_V4)) return refusal("exact gfx950 COV6 header");
  auto Sections=F.sections(); if (!Sections) return Sections.takeError();
  if (Sections->size()>128) return refusal("section cap");
  Observation A; size_t TextCount=0,Notes=0,SymbolCount=0,EntryCount=0,DescriptorCount=0;
  for (const auto &S:*Sections) {
    if ((S.sh_type==ELF::SHT_REL || S.sh_type==ELF::SHT_RELA) && S.sh_size)
      return refusal("relocation forbidden");
    if ((S.sh_flags&ELF::SHF_EXECINSTR)!=0) {
      if (++TextCount!=1 || S.sh_type!=ELF::SHT_PROGBITS ||
          (S.sh_flags&(ELF::SHF_ALLOC|ELF::SHF_WRITE))!=ELF::SHF_ALLOC ||
          S.sh_size!=1152 || !within(Bytes.size(),S.sh_offset,S.sh_size))
        return refusal("one exact executable section with prefetch padding");
      A.TextOffset=S.sh_offset; A.TextBytes=S.sh_size;
    }
    if (S.sh_type==ELF::SHT_NOTE) {
      Error E=Error::success();
      for (const auto N:F.notes(S,E)) {
        if (N.getName()!="AMDGPU" || N.getType()!=ELF::NT_AMDGPU_METADATA) continue;
        auto B=N.getDescAsStringRef(4);
        if (++Notes!=1 || B.empty() || B.size()>16384) { consumeError(std::move(E)); return refusal("metadata cap/cardinality"); }
        const uintptr_t Base=reinterpret_cast<uintptr_t>(Bytes.data());
        const uintptr_t Start=reinterpret_cast<uintptr_t>(B.data());
        if (Start<Base || !within(Bytes.size(),Start-Base,B.size())) {
          consumeError(std::move(E)); return refusal("metadata extent");
        }
        A.NoteOffset=Start-Base; A.NoteBytes=B.size();
      }
      if (E) return E;
    }
  }
  for (const auto &S:(*O)->symbols()) {
    if (++SymbolCount>4096) return refusal("symbol cap");
    auto N=S.getName(); if (!N) return N.takeError();
    if (N->size()>256) return refusal("symbol name cap");
    auto Sec=S.getSection(); if (!Sec) return Sec.takeError();
    auto Type=S.getType(); if (!Type) return Type.takeError();
    if (*Sec==(*O)->section_end()) continue;
    const uint64_t Size=object::ELFSymbolRef(S).getSize();
    auto VA=S.getAddress(); if (!VA) return VA.takeError();
    auto B=(*Sec)->getContents(); if (!B) return B.takeError();
    const auto SectionVA=(*Sec)->getAddress();
    if (*VA<SectionVA || !within(B->size(),*VA-SectionVA,Size)) return refusal("symbol section range");
    if (*N!=Name && *N!=Name.str()+".kd") {
      if (*Type==object::SymbolRef::ST_Function || ((*Sec)->isText() && Size))
        return refusal("foreign executable symbol");
      continue;
    }
    const uintptr_t Base=reinterpret_cast<uintptr_t>(Bytes.data());
    const uintptr_t Begin=reinterpret_cast<uintptr_t>(B->data());
    if (Begin<Base || !within(Bytes.size(),Begin-Base,B->size())) return refusal("symbol file relation");
    const uint64_t FileOffset=Begin-Base+(*VA-SectionVA);
    if (*N==Name) {
      if (++EntryCount!=1 || *Type!=object::SymbolRef::ST_Function || !(*Sec)->isText() ||
          Size!=84 || FileOffset!=A.TextOffset || *VA!=SectionVA)
        return refusal("single exact entry");
      A.EntryOffset=FileOffset; A.EntryAddress=*VA;
    } else {
      if (++DescriptorCount!=1 || Size!=64 || (*Sec)->isText() || *VA%64)
        return refusal("single aligned descriptor");
      A.DescriptorOffset=FileOffset; A.DescriptorAddress=*VA;
    }
  }
  if (TextCount!=1 || Notes!=1 || EntryCount!=1 || DescriptorCount!=1 ||
      !within(Bytes.size(),A.EntryOffset,84) || !within(Bytes.size(),A.DescriptorOffset,64))
    return refusal("entry/descriptor/note cardinality");
  // Pinned AMDGPUTargetELFStreamer: align64 then 16 cache lines of S_NOP.
  for (uint64_t At=A.EntryOffset+84;At<A.TextOffset+A.TextBytes;At+=4)
    if (support::endian::read32le(Bytes.data()+At)!=0xbf800000)
      return refusal("unexpected executable prefetch padding");
  auto Metadata=fixtureMetadata(Bytes,Name);
  if (!Metadata) return refusal("complete explicit/hidden ABI metadata");
  A.Metadata=std::move(*Metadata);
  const uint8_t *B=Bytes.data()+A.DescriptorOffset;
  using namespace llvm::amdhsa;
  if (support::endian::read32le(B+GROUP_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
      support::endian::read32le(B+PRIVATE_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
      support::endian::read32le(B+KERNARG_SIZE_OFFSET)!=264 ||
      support::endian::read16le(B+KERNARG_PRELOAD_OFFSET)!=0 ||
      A.EntryAddress>INT64_MAX || A.DescriptorAddress>INT64_MAX ||
      support::endian::read64le(B+KERNEL_CODE_ENTRY_BYTE_OFFSET_OFFSET)!=A.EntryAddress-A.DescriptorAddress)
    return refusal("descriptor segments/entry/kernarg");
  for (const auto &[Start,Size]:std::array<std::pair<unsigned,unsigned>,3>{{{12,4},{24,20},{60,4}}})
    if (llvm::any_of(ArrayRef<uint8_t>(B+Start,Size),[](uint8_t V){return V!=0;}))
      return refusal("descriptor reserved bytes");
  A.Rsrc1=support::endian::read32le(B+COMPUTE_PGM_RSRC1_OFFSET);
  A.Rsrc2=support::endian::read32le(B+COMPUTE_PGM_RSRC2_OFFSET);
  A.Rsrc3=support::endian::read32le(B+COMPUTE_PGM_RSRC3_OFFSET);
  // Only counts and the explicitly reported, integer-irrelevant FP modes.
  constexpr uint32_t AllowedRsrc1=0x000003ffU | 0x000ff000U | (1U<<21) | (1U<<23);
  if ((A.Rsrc1&~AllowedRsrc1)!=0 || A.Rsrc2!=132 || (A.Rsrc3&~0x3fU)!=0 ||
      support::endian::read16le(B+KERNEL_CODE_PROPERTIES_OFFSET)!=
          KERNEL_CODE_PROPERTY_ENABLE_SGPR_KERNARG_SEGMENT_PTR)
    return refusal("descriptor exact live-ins/mode");
  A.Vgprs=((A.Rsrc1&63)+1)*8; A.Sgprs=(((A.Rsrc1>>6)&15)+1)*8;
  A.Boundary=((A.Rsrc3&63)+1)*4;
  if (A.Vgprs<10 || A.Vgprs>32 || A.Sgprs<14 || A.Sgprs>32 ||
      A.Boundary<10 || A.Boundary>A.Vgprs || A.Metadata.Vgprs<10 ||
      A.Metadata.Vgprs>A.Boundary || A.Metadata.Sgprs<14 || A.Metadata.Sgprs>A.Sgprs ||
      A.Metadata.Agprs || A.Metadata.SgprSpills || A.Metadata.VgprSpills)
    return refusal("zero spills/AGPR and bounded register resources");
  A.DescriptorDigest=SHA256::hash(ArrayRef<uint8_t>(B,64));
  auto Trace=decode(M,Bytes.slice(A.EntryOffset,84),A.EntryAddress);
  if (!Trace) return Trace.takeError();
  if (!exactRows(M,*Trace)) return refusal("full exact entry MC relation");
  A.Trace=std::move(*Trace); return A;
}
json::Array traceJson(const Observation &A) {
  json::Array Rows;
  for (const auto &T:A.Trace) {
    json::Array Registers,Immediates,Uses,Writes;
    for (const auto &V:T.Actual.Registers) Registers.emplace_back(V);
    for (const auto V:T.Actual.Immediates) Immediates.emplace_back(std::to_string(V));
    for (const auto &V:T.Actual.Uses) Uses.emplace_back(V);
    for (const auto &V:T.Actual.Writes) Writes.emplace_back(V);
    Rows.emplace_back(json::Object{{"offset",T.Offset},{"bytes",T.Actual.Bytes},
      {"encoding",hex(T.Encoding)},{"opcode",T.Actual.Opcode},{"operand_order",T.Actual.Order},
      {"registers",std::move(Registers)},{"immediates_u64_decimal",std::move(Immediates)},
      {"defs",T.Actual.Defs},{"flags",T.Actual.Flags},{"implicit_uses",std::move(Uses)},
      {"implicit_defs",std::move(Writes)}});
  }
  return Rows;
}
