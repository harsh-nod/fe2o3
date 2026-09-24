// Entry-state contract is checked against LLVM's emitted descriptor, never fixed up.
struct PhysicalDescriptor {
  uint64_t Offset=0; uint32_t Rsrc1=0,Rsrc2=0,Rsrc3=0,Vgprs=0,Sgprs=0,Boundary=0;
  uint16_t Properties=0;
  std::array<uint8_t,32> Digest{};
};
std::optional<PhysicalDescriptor> physicalDescriptor(
    const PhysicalMachineEffectEvidence &E,ArrayRef<uint8_t> Payload,
    const PhysicalMetadata &M, StringRef Symbol = PhysicalName,
    uint32_t MinimumSgprs = 20, uint32_t MinimumVgprs = 9) {
  if(MinimumSgprs<3 || MinimumSgprs>64 || MinimumVgprs<1 || MinimumVgprs>64 || Payload.empty() || Payload.size()>64*1024 || E.Entries.size()!=1 ||
      E.Entries[0].Symbol!=Symbol || E.PayloadBytes!=Payload.size() ||
      E.PayloadDigest!=SHA256::hash(Payload)) return std::nullopt;
  auto O=object::ObjectFile::createObjectFile(MemoryBufferRef(
      StringRef(reinterpret_cast<const char*>(Payload.data()),Payload.size()),"<physical-descriptor>"));
  if(!O) { consumeError(O.takeError()); return std::nullopt; }
  std::optional<PhysicalDescriptor> Result; size_t Count=0;
  for(const auto &S:(*O)->symbols()) {
    if(++Count>4096) return std::nullopt;
    auto Name=S.getName();
    if(!Name) { consumeError(Name.takeError()); return std::nullopt; }
    if(*Name!=Symbol.str()+".kd") continue;
    if(Result) return std::nullopt;
    auto Sec=S.getSection();
    if(!Sec) { consumeError(Sec.takeError()); return std::nullopt; }
    if(*Sec==(*O)->section_end()) return std::nullopt;
    auto Content=(*Sec)->getContents();
    if(!Content) { consumeError(Content.takeError()); return std::nullopt; }
    auto Address=S.getAddress();
    if(!Address) { consumeError(Address.takeError()); return std::nullopt; }
    if(*Address<(*Sec)->getAddress()) return std::nullopt;
    const auto Relative=*Address-(*Sec)->getAddress();
    if(Relative>Content->size() || Content->size()-Relative<64) return std::nullopt;
    const auto *B=reinterpret_cast<const uint8_t*>(Content->data())+Relative;
    const uintptr_t Base=reinterpret_cast<uintptr_t>(Payload.data()),Begin=reinterpret_cast<uintptr_t>(B);
    if(Begin<Base || Begin-Base>Payload.size() || Payload.size()-(Begin-Base)<64) return std::nullopt;
    PhysicalDescriptor D; D.Offset=Begin-Base;
    D.Digest=SHA256::hash(ArrayRef<uint8_t>(B,64));
    if(D.Digest!=E.Entries[0].DescriptorIdentity) return std::nullopt;
    using namespace llvm::amdhsa;
    if(support::endian::read32le(B+GROUP_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
        support::endian::read32le(B+PRIVATE_SEGMENT_FIXED_SIZE_OFFSET)!=0 ||
        support::endian::read32le(B+KERNARG_SIZE_OFFSET)!=288 ||
        support::endian::read16le(B+KERNARG_PRELOAD_OFFSET)!=0) return std::nullopt;
    for(const auto &[Start,Size]:std::array<std::pair<unsigned,unsigned>,3>{{
          {12,4},{24,20},{60,4}}})
      if(llvm::any_of(ArrayRef<uint8_t>(B+Start,Size),[](uint8_t V){return V!=0;}))
        return std::nullopt;
    D.Rsrc1=support::endian::read32le(B+COMPUTE_PGM_RSRC1_OFFSET);
    D.Rsrc2=support::endian::read32le(B+COMPUTE_PGM_RSRC2_OFFSET);
    D.Rsrc3=support::endian::read32le(B+COMPUTE_PGM_RSRC3_OFFSET);
    D.Properties=support::endian::read16le(B+KERNEL_CODE_PROPERTIES_OFFSET);
    // User SGPR count=2, only workgroup X enabled, X-only workitem ID.
    // No scratch, trap-handler, other system IDs, LDS, exception or preload.
    // Properties enable only kernarg: no dispatch/queue/ID shifting its s0 base.
    if(D.Rsrc2!=132 || D.Properties!=KERNEL_CODE_PROPERTY_ENABLE_SGPR_KERNARG_SEGMENT_PTR)
      return std::nullopt;
    D.Vgprs=((D.Rsrc1&0x3f)+1)*8;
    D.Boundary=((D.Rsrc3&0x3f)+1)*4;
    const uint32_t SgprBlocks=(D.Rsrc1>>6)&15;
    if(SgprBlocks>13) return std::nullopt;
    D.Sgprs=(SgprBlocks+1)*8;
    if(D.Vgprs<MinimumVgprs || D.Boundary<MinimumVgprs || D.Vgprs<D.Boundary || D.Vgprs<M.Vgprs ||
        D.Sgprs<MinimumSgprs || D.Sgprs<M.Sgprs || M.Vgprs<MinimumVgprs || M.Sgprs<MinimumSgprs ||
        M.Agprs!=0 || M.SgprSpills!=0 || M.VgprSpills!=0) return std::nullopt;
    for(const auto &I:E.Instructions) for(const auto &Operand:I.Operands) {
      if(Operand.Kind!=PhysicalMachineOperandKind::Register) continue;
      SmallVector<StringRef,8> Parts; StringRef(Operand.Register).split(Parts,'_');
      if(Parts.empty() || Parts.size()>16) return std::nullopt;
      for(StringRef Part:Parts) {
        if(Part=="EXEC" || Part=="VCC" || Part=="SCC") continue;
        const bool V=Part.consume_front("VGPR"); const bool S=!V && Part.consume_front("SGPR");
        if(!V && !S) return std::nullopt;
        unsigned Index=0;
        if(Part.empty() || Part.getAsInteger(10,Index) ||
            Index>=(V?D.Vgprs:D.Sgprs) || (V && Index>=D.Boundary)) return std::nullopt;
      }
    }
    Result=D;
  }
  return Result;
}
