// Actual LLVM/LLD/MC controls. No GPU execution and no source-safety claim.
void repeatedDirectCallSitesKeepExactBudgetsAndGraph() {
  for (uint32_t Count : {1u, 2u, 3u}) {
    auto Payload = finalize(makeKernelBitcode(true, 600, false, Count));
    auto Budget = generousBudget();
    Budget.DirectCalls = Count;
    auto RequestValue = directRequest(Payload, Budget);
    auto Result = analyzeGfx942PhysicalMachineEffects(RequestValue);
    if (!Result)
      fail(takeError(Result.takeError()));
    auto Alpha = llvm::find_if(Result->Functions, [](const auto &Function) {
      return Function.Symbol == "alpha";
    });
    auto Helper = llvm::find_if(Result->Functions, [](const auto &Function) {
      return Function.Symbol == "alpha_helper";
    });
    require(Alpha != Result->Functions.end() && Helper != Result->Functions.end(),
            "repeated call fixture omitted a reachable function");
    require(Alpha->DirectCallees == std::vector<std::string>{"alpha_helper"},
            "repeated calls did not preserve unique adjacency");
    std::set<uint64_t> Offsets;
    for (const auto &Instruction : Result->Instructions) {
      if (Instruction.FunctionSymbol != "alpha" ||
          Instruction.BranchKind != PhysicalMachineBranchKind::DirectCall)
        continue;
      require(Instruction.BranchTarget == Helper->CodeOffset &&
                  (Instruction.Opcode == "S_SWAPPC_B64_vi" ||
                   Instruction.Opcode == "S_CALL_B64_vi"),
              "repeated call instruction lost its exact target or opcode");
      require(Offsets.insert(Instruction.InstructionOffset).second,
              "repeated call sites share an instruction identity");
    }
    require(Offsets.size() == Count,
            "native trace deduplicated repeated static call sites");

    auto Bundle = encodePhysicalMachineAnalysisBundle(*Result);
    if (!Bundle)
      fail(takeError(Bundle.takeError()));
    require(!Bundle->empty(), "repeated call bundle is empty");
    auto Again = analyzeGfx942PhysicalMachineEffects(RequestValue);
    if (!Again)
      fail(takeError(Again.takeError()));
    auto AgainBundle = encodePhysicalMachineAnalysisBundle(*Again);
    if (!AgainBundle)
      fail(takeError(AgainBundle.takeError()));
    require(*Bundle == *AgainBundle, "repeated call bundle is not deterministic");

    // One static helper write/return remains one site, not Count dynamic rows.
    const auto HelperWrites = llvm::count_if(Result->Effects, [](const auto &E) {
      return E.EntrySymbol == "alpha" && E.FunctionSymbol == "alpha_helper" &&
             E.Kind == PhysicalMachineEffectKind::GlobalWrite;
    });
    const auto HelperReturns = llvm::count_if(Result->Effects, [](const auto &E) {
      return E.EntrySymbol == "alpha" && E.FunctionSymbol == "alpha_helper" &&
             E.Kind == PhysicalMachineEffectKind::Return;
    });
    require(HelperWrites == 1 && HelperReturns == 1,
            "shared helper effects were expanded into dynamic occurrences");

    // Same real payload, strictly smaller count: graph cardinality is still one.
    --Budget.DirectCalls;
    auto TooSmall =
        analyzeGfx942PhysicalMachineEffects(directRequest(Payload, Budget));
    require(!TooSmall, "one-below static call-site budget was accepted");
    const std::string ErrorText = takeError(TooSmall.takeError());
    require(StringRef(ErrorText).contains("effect expansion exceeds request"),
            "one-below budget failed for an unrelated fixture reason");

    if (Count == 2) {
      auto Sites = decodeSymbolSites(Payload, "alpha");
      auto LastCall = Sites.end();
      for (auto It = Sites.begin(); It != Sites.end(); ++It)
        if (It->Name == "S_SWAPPC_B64_vi")
          LastCall = It;
      require(LastCall != Sites.end(), "repeated fixture lacks S_SWAPPC");
      auto LastHigh = Sites.end();
      for (auto It = Sites.begin(); It != LastCall; ++It)
        if (It->Name == "S_ADDC_U32_vi" && It->Size == 8)
          LastHigh = It;
      require(LastHigh != Sites.end(),
              "repeated fixture lacks a call address materialization");
      auto Changed = Payload;
      const uint32_t Literal = support::endian::read32le(
          Changed.data() + LastHigh->FileOffset + 4);
      support::endian::write32le(Changed.data() + LastHigh->FileOffset + 4,
                                Literal ^ 1u);
      requireRejectedWith(std::move(Changed),
                          "call target is not one exact function");
    }
  }
}
