// Test-only arbitrary LLVM-byte observation. Source ancestry is joined by the
// external retained-callback runner, not inferred from this file or its label.
// This leaf does not add a production worker request or finalizer selector.

Input readSourceLlvm(StringRef Path) {
  require(Path.starts_with("/") && Path.size() <= 4096 &&
              llvm::none_of(Path, [](char Byte) {
                return static_cast<unsigned char>(Byte) < 32 || Byte == 127;
              }), "LLVM input requires a bounded absolute path");
  int File = open(Path.str().c_str(), O_RDONLY | O_NOFOLLOW | O_NONBLOCK);
  require(File >= 0, "cannot open non-symlink LLVM input");
  struct stat Before {}, After {};
  require(fstat(File, &Before) == 0 && S_ISREG(Before.st_mode) &&
              Before.st_size > 0 &&
              static_cast<uint64_t>(Before.st_size) <= FixtureByteLimit,
          "LLVM input is not a bounded regular file");
  Input Result;
  Result.Kind = InputKind::LlvmTextIr;
  Result.Bytes.resize(static_cast<size_t>(Before.st_size));
  size_t Offset = 0;
  while (Offset < Result.Bytes.size()) {
    ssize_t Count = read(File, Result.Bytes.data() + Offset,
                         Result.Bytes.size() - Offset);
    if (Count < 0 && errno == EINTR)
      continue;
    require(Count > 0, "LLVM input shrank or became unreadable");
    Offset += static_cast<size_t>(Count);
  }
  require(fstat(File, &After) == 0 && Before.st_dev == After.st_dev &&
              Before.st_ino == After.st_ino && Before.st_size == After.st_size &&
              Before.st_mtim.tv_sec == After.st_mtim.tv_sec &&
              Before.st_mtim.tv_nsec == After.st_mtim.tv_nsec &&
              Before.st_ctim.tv_sec == After.st_ctim.tv_sec &&
              Before.st_ctim.tv_nsec == After.st_ctim.tv_nsec,
          "LLVM input changed while reading");
  require(close(File) == 0, "LLVM input close failed");
  Result.Digest = SHA256::hash(Result.Bytes);
  return Result;
}

std::string sourceKernelSymbol(const Input &InputValue) {
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  // LLVM's textual parser needs a sentinel beyond the logical buffer end.
  // The bounded copy changes no logical input bytes or transport digest.
  const std::string Text(InputValue.Bytes.begin(), InputValue.Bytes.end());
  auto Parsed = parseAssemblyString(Text, Diagnostic, Context);
  require(Parsed != nullptr, "LLVM observation input is not valid textual IR");
  require(Parsed->getTargetTriple().str() == TripleName &&
              Parsed->getModuleInlineAsm().empty() && Parsed->alias_empty() &&
              Parsed->ifunc_empty(),
          "LLVM observation requires the exact target and no module asm/aliases/ifuncs");
  std::string Symbol;
  for (const Function &FunctionValue : *Parsed) {
    if (FunctionValue.isDeclaration())
      continue;
    require(Symbol.empty() &&
                FunctionValue.getCallingConv() == CallingConv::AMDGPU_KERNEL,
            "LLVM observation needs exactly one defined AMD kernel and no helpers");
    Symbol = FunctionValue.getName().str();
  }
  require(!Symbol.empty() && Symbol.size() <= 128 &&
              ((Symbol[0] >= 'A' && Symbol[0] <= 'Z') ||
               (Symbol[0] >= 'a' && Symbol[0] <= 'z') || Symbol[0] == '_') &&
              llvm::all_of(Symbol, [](char Byte) {
                return (Byte >= 'A' && Byte <= 'Z') ||
                       (Byte >= 'a' && Byte <= 'z') ||
                       (Byte >= '0' && Byte <= '9') || Byte == '_';
              }), "LLVM kernel symbol is outside the closed observation profile");
  // Refuse structural profile mismatches before asking LLVM to verify the
  // module. In particular, an absent layout must not get a default profile.
  require(!Parsed->getDataLayoutStr().empty(),
          "LLVM observation requires an explicit data layout");
  std::string Verification;
  raw_string_ostream Stream(Verification);
  require(!verifyModule(*Parsed, &Stream), "LLVM observation input fails verification");
  return Symbol;
}

json::Object descriptorResources(const BuiltFixture &Built, StringRef Symbol) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  StringRef Data(reinterpret_cast<const char *>(Payload.data()), Payload.size());
  auto Object = unwrap(object::ObjectFile::createObjectFile(
      MemoryBufferRef(Data, "<test-only-final-descriptor>")));
  std::optional<json::Object> Result;
  size_t Count = 0;
  for (const auto &Candidate : Object->symbols()) {
    require(++Count <= 4096, "descriptor symbol scan exceeds test bound");
    if (unwrap(Candidate.getName()) != Symbol.str() + ".kd")
      continue;
    require(!Result, "duplicate final descriptor symbol");
    auto Section = unwrap(Candidate.getSection());
    require(Section != Object->section_end(), "descriptor has no section");
    auto Contents = unwrap(Section->getContents());
    uint64_t Address = unwrap(Candidate.getAddress());
    require(Address >= Section->getAddress(), "descriptor address precedes section");
    uint64_t Relative = Address - Section->getAddress();
    require(Relative <= Contents.size() && Contents.size() - Relative >= 64,
            "descriptor is outside its exact section bytes");
    const uint8_t *Bytes = reinterpret_cast<const uint8_t *>(Contents.data()) + Relative;
    const uintptr_t Base = reinterpret_cast<uintptr_t>(Payload.data());
    const uintptr_t Begin = reinterpret_cast<uintptr_t>(Bytes);
    require(Begin >= Base && Begin - Base <= Payload.size() &&
                Payload.size() - (Begin - Base) >= 64,
            "descriptor does not name final payload storage");
    const auto Digest = SHA256::hash(ArrayRef<uint8_t>(Bytes, 64));
    require(Digest == Built.Evidence.Entries[0].DescriptorIdentity,
            "independent descriptor bytes differ from final decoder");
    const uint32_t Rsrc3 = support::endian::read32le(Bytes + 44);
    const uint32_t Rsrc1 = support::endian::read32le(Bytes + 48);
    // Gfx942 is a gfx90a-family allocation profile: RSRC1 uses EIGHT,
    // not generic gfx9 wave64's four-register granule. RSRC3's architected
    // VGPR / accumulator boundary uses four. Neither field is a live-use count.
    const uint32_t Capacity = ((Rsrc1 & 0x3f) + 1) * 8;
    const uint32_t ArchitectedBoundary = ((Rsrc3 & 0x3f) + 1) * 4;
    require(Capacity >= 37 && ArchitectedBoundary >= 37 &&
                Capacity >= ArchitectedBoundary,
            "final descriptor does not cover fixed v0..v36 footprint");
    Result = json::Object{{"descriptor_file_offset", Begin - Base},
        {"descriptor_bytes", 64}, {"descriptor_sha256", hex(Digest)},
        {"compute_pgm_rsrc1", Rsrc1}, {"compute_pgm_rsrc3", Rsrc3},
        {"vgpr_capacity", Capacity},
        {"architected_vgpr_boundary", ArchitectedBoundary},
        {"required_footprint_high_water", 37},
        {"interpretation", "encoded-capacity-not-metadata-usage-or-lifetime"}};
  }
  require(Result.has_value(), "final descriptor symbol is absent");
  return std::move(*Result);
}

int observeSourceLlvm(StringRef Mode, StringRef Path) {
  require(Mode == "--observe-llvm-used" || Mode == "--observe-llvm-unused",
          "unsupported LLVM observation mode");
  const bool Used = Mode == "--observe-llvm-used";
  const Input InputValue = readSourceLlvm(Path);
  const std::string Symbol = sourceKernelSymbol(InputValue);
  json::Array Cases;
  for (OptimizationLevel Level : {OptimizationLevel::O0, OptimizationLevel::O3}) {
    auto Built = buildRequest(makeInputRequest(InputValue, Level, Symbol), Symbol);
    auto Case = describePositive(Built, Level, Used, Symbol);
    Case["descriptor_resources"] = descriptorResources(Built, Symbol);
    Cases.emplace_back(std::move(Case));
  }
  // No LLVM rewriting or pretty-print replacement was performed. Also reject
  // changed bytes at the same input path during the two compilations.
  const Input After = readSourceLlvm(Path);
  require(After.Bytes == InputValue.Bytes && After.Digest == InputValue.Digest,
          "LLVM input changed during compilation");
  json::Object Report{{"schema", "fe2o3-ordered-region-llvm-machine-observation-v1"},
      {"authority", "unauthenticated-test-transport"},
      {"source_ancestry", "not-established-by-llvm-file"},
      {"production_exact_region_admission", false},
      {"protected_finalizer_admission", false}, {"hardware_executed", false},
      {"runtime_closure_attestation", "unavailable"},
      {"physical_register_allocation_or_lifetime_proof", false},
      {"whole_kernel_order_or_byte_stability_claim", false},
      {"target", "gfx942:xnack-"}, {"wave_width", 64},
      {"workgroup_size", 64}, {"code_object_version", 6},
      {"kernel_symbol", Symbol}, {"result_used", Used},
      {"llvm_text_sha256", hex(InputValue.Digest)},
      {"llvm_text_bytes", InputValue.Bytes.size()},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID},
      {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"cases", std::move(Cases)}};
  std::string Text;
  raw_string_ostream Stream(Text);
  Stream << formatv("{0:2}", json::Value(std::move(Report)));
  Stream.flush();
  require(Text.size() + 1 <= ReportByteLimit, "LLVM observation report exceeds cap");
  outs() << Text << '\n';
  alarm(0);
  return 0;
}
