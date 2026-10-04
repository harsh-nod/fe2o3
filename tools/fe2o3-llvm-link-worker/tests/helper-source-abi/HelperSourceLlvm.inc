// Structural observation of the unchanged normal-driver LLVM, not source authority.

// The LLVM module is gone before native compilation starts. JSON strings and the
// expected rosters therefore own their bytes independently of that module.
struct HelperLlvmObservation {
  json::Object Fields;
  std::vector<std::string> Declarations;
};
bool helperLlvmNameRosterMatches(const json::Array *Rows,
                                const std::vector<std::string> &Expected,
                                bool Helpers) {
  if (!Rows || Rows->size() != Expected.size() || Rows->size() > 16)
    return false;
  std::vector<std::string> Names;
  Names.reserve(Rows->size());
  for (const auto &Row : *Rows) {
    const auto *Object = Helpers ? Row.getAsObject() : nullptr;
    const auto Name = Helpers
        ? (Object ? Object->getString("symbol") : std::optional<StringRef>{})
        : Row.getAsString();
    if (!Name || Name->empty() || Name->size() > 128)
      return false;
    Names.push_back(Name->str());
  }
  std::sort(Names.begin(), Names.end());
  return Names == Expected;
}
bool helperLlvmNamesRetained(const HelperLlvmObservation &Observation,
                            const HelperInput &In) {
  return helperLlvmNameRosterMatches(
             Observation.Fields.getArray("helpers"), In.Helpers, true) &&
         helperLlvmNameRosterMatches(
             Observation.Fields.getArray("declared_intrinsics"),
             Observation.Declarations, false);
}
void helperLlvmNameControls() {
  const std::vector<std::string> Expected{"first", "second"};
  json::Array Good{json::Object{{"symbol", "first"}},
                   json::Object{{"symbol", "second"}}};
  require(helperLlvmNameRosterMatches(&Good, Expected, true),
          "owned LLVM symbol roster positive control");
  json::Array Changed{json::Object{{"symbol", "first"}},
                      json::Object{{"symbol", "changed"}}};
  json::Array Duplicate{json::Object{{"symbol", "first"}},
                        json::Object{{"symbol", "first"}}};
  json::Array Malformed{json::Object{{"symbol", 17}},
                        json::Object{{"symbol", "second"}}};
  require(!helperLlvmNameRosterMatches(&Changed, Expected, true) &&
              !helperLlvmNameRosterMatches(&Duplicate, Expected, true) &&
              !helperLlvmNameRosterMatches(&Malformed, Expected, true) &&
              !helperLlvmNameRosterMatches(nullptr, Expected, true),
          "owned LLVM symbol roster negative controls");
  json::Array Intrinsics{"first", "second"};
  json::Array BadIntrinsics{"first", "changed"};
  require(helperLlvmNameRosterMatches(&Intrinsics, Expected, false) &&
              !helperLlvmNameRosterMatches(&BadIntrinsics, Expected, false),
          "owned LLVM intrinsic roster controls");
}

HelperLlvmObservation helperLlvmObservation(const HelperInput &In) {
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  const StringRef Text(reinterpret_cast<const char *>(In.Llvm.Bytes.data()),
                       In.Llvm.Bytes.size());
  auto ModuleValue = parseAssemblyString(Text, Diagnostic, Context);
  require(ModuleValue != nullptr, "actual helper LLVM parse");
  require(!verifyModule(*ModuleValue, &errs()), "actual helper LLVM verification");
  require(ModuleValue->getTargetTriple().str() == TripleName &&
              ModuleValue->getModuleInlineAsm().empty() &&
              ModuleValue->global_empty() && ModuleValue->alias_empty() &&
              ModuleValue->ifunc_empty(),
          "unexpected LLVM module transport");
  Function *Kernel = ModuleValue->getFunction(In.Entry);
  require(Kernel && !Kernel->isDeclaration() && Kernel->hasExternalLinkage() &&
              Kernel->getCallingConv() == CallingConv::AMDGPU_KERNEL &&
              !Kernel->isVarArg() && Kernel->getReturnType()->isVoidTy() &&
              Kernel->arg_size() == 4,
          "actual helper kernel four-component ABI");
  auto *Pointer = dyn_cast<PointerType>(Kernel->getArg(0)->getType());
  require(Pointer && Pointer->getAddressSpace() == 1 &&
              Kernel->getArg(1)->getType()->isIntegerTy(64) &&
              Kernel->getArg(2)->getType()->isIntegerTy(32) &&
              Kernel->getArg(3)->getType()->isIntegerTy(32),
          "actual helper ptr/len/a/b component types");
  auto *Workgroup = Kernel->getMetadata("reqd_work_group_size");
  require(Workgroup && Workgroup->getNumOperands() == 3,
          "actual helper required workgroup metadata");
  for (unsigned I = 0; I != 3; ++I) {
    auto *Value = mdconst::dyn_extract<ConstantInt>(Workgroup->getOperand(I));
    require(Value && Value->getZExtValue() == (I == 0 ? 64 : 1),
            "actual helper required workgroup value");
  }
  size_t Functions = 0, Definitions = 0, Blocks = 0, Instructions = 0;
  json::Array HelperRows, DeclarationRows;
  std::vector<std::string> Declarations;
  std::vector<uint64_t> Constants;
  std::vector<std::string> RootCallees;
  for (const auto &F : *ModuleValue) {
    require(++Functions <= 16, "helper LLVM function count bound");
    if (F.isDeclaration()) {
      require(F.isIntrinsic(), "unexpected external LLVM callable");
      Declarations.push_back(F.getName().str());
      DeclarationRows.emplace_back(F.getName().str());
      continue;
    }
    ++Definitions;
    const bool Root = &F == Kernel;
    require(Root || std::binary_search(In.Helpers.begin(), In.Helpers.end(),
                                      F.getName().str()),
            "LLVM definition absent from exact retained source roster");
    require(F.getFnAttribute("target-cpu").getValueAsString() == "gfx942" &&
                F.hasFnAttribute(Attribute::NoUnwind),
            "helper target or unwind contract changed");
    if (!Root)
      require(F.hasInternalLinkage() && F.getCallingConv() == CallingConv::C &&
                  !F.isVarArg() && F.arg_size() == 1 &&
                  F.getArg(0)->getType()->isIntegerTy(32) &&
                  F.getReturnType()->isIntegerTy(32),
              "retained helper scalar LLVM ABI");
    const CallBase *Assembly = nullptr;
    size_t Returns = 0;
    for (const auto &B : F) {
      require(++Blocks <= 64, "helper LLVM block bound");
      for (const auto &I : B) {
        require(++Instructions <= 512, "helper LLVM instruction bound");
        if (const auto *Call = dyn_cast<CallBase>(&I)) {
          if (const auto *Asm = dyn_cast<InlineAsm>(Call->getCalledOperand())) {
            require(!Root && !Assembly && isa<CallInst>(Call) &&
                        Asm->getAsmString() == "v_or_b32 $0, $1, $2" &&
                        Asm->getConstraintString() == "=v,v,v" &&
                        Asm->hasSideEffects() && !Asm->isAlignStack() &&
                        Asm->getDialect() == InlineAsm::AD_ATT &&
                        Call->arg_size() == 2 &&
                        Call->getType()->isIntegerTy(32) &&
                        Call->getArgOperand(0) == F.getArg(0),
                    "exact helper VOR transport changed");
            const auto *Constant = dyn_cast<ConstantInt>(Call->getArgOperand(1));
            require(Constant && Constant->getType()->isIntegerTy(32) &&
                        (Constant->getZExtValue() == 256 ||
                         Constant->getZExtValue() == 512),
                    "helper specialization constant");
            Constants.push_back(Constant->getZExtValue());
            Assembly = Call;
          } else {
            const Function *Callee = Call->getCalledFunction();
            require(Callee && Root, "indirect/nested helper call outside profile");
            if (!Callee->isIntrinsic()) {
              require(isa<CallInst>(Call) &&
                          std::binary_search(In.Helpers.begin(), In.Helpers.end(),
                                             Callee->getName().str()) &&
                          Call->getCallingConv() == CallingConv::C &&
                          Call->arg_size() == 1 &&
                          Call->getArgOperand(0)->getType()->isIntegerTy(32) &&
                          Call->getType()->isIntegerTy(32),
                      "root/helper call ABI mismatch");
              RootCallees.push_back(Callee->getName().str());
            }
          }
        }
        if (isa<ReturnInst>(I))
          ++Returns;
      }
    }
    if (!Root) {
      require(Assembly && Returns == 1, "one exact VOR and one helper return");
      for (const auto &B : F)
        if (const auto *Return = dyn_cast<ReturnInst>(B.getTerminator()))
          require(Return->getReturnValue() == Assembly,
                  "helper return is not the actual VOR result");
      HelperRows.emplace_back(json::Object{
          {"symbol", F.getName().str()},
          {"llvm_noinline", F.hasFnAttribute(Attribute::NoInline)},
          {"specialization_u32",
           cast<ConstantInt>(Assembly->getArgOperand(1))->getZExtValue()},
          {"one_i32_argument_and_i32_result", true},
          {"vor_uses_argument_and_constant_and_is_returned", true}});
    }
  }
  std::sort(Constants.begin(), Constants.end());
  const std::vector<uint64_t> Expected =
      In.Label == "two" ? std::vector<uint64_t>{256, 512}
      : In.Label == "default256" ? std::vector<uint64_t>{256}
                                : std::vector<uint64_t>{512};
  std::sort(RootCallees.begin(), RootCallees.end());
  require(Definitions == In.Helpers.size() + 1 &&
              RootCallees == In.Helpers && Constants == Expected,
          "exact helper definition/call/specialization census");
  std::sort(Declarations.begin(), Declarations.end());
  return {json::Object{
      {"kernel_component_count", 4},
      {"defined_function_count", Definitions},
      {"instruction_count", Instructions},
      {"declared_intrinsics", std::move(DeclarationRows)},
      {"helpers", std::move(HelperRows)},
      {"root_expression_and_guard_semantics_reproved_here", false}},
      std::move(Declarations)};
}
