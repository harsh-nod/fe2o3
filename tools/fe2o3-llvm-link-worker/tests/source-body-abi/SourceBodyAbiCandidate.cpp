// Source-fed diagnostic, not production authority. Reuse unchanged native test
// request/decoder/literal GFX9 encoding helpers, never its synthetic main.
#define main fe2o3_unused_ordered_program_test_main
#include "OrderedProgramPrototypeTests.cpp"
#undef main
#include "llvm/IR/IntrinsicsAMDGPU.h"
#include "llvm/Support/AMDHSAKernelDescriptor.h"
namespace {
constexpr StringLiteral BodyConstraints =
    "{s16},{v4},{s18},{s19},{v34},{v35},{v36},{v0},{v1},"
    "~{v2},~{v3},~{v32},~{v33},~{s20},~{s21},~{vcc},~{scc},~{memory}";
constexpr StringLiteral BodyAssembly =
    "v_xor_b32_e32 v32, v34, v35\n\t"
    "v_and_b32_e32 v32, v32, v36\n\t"
    "v_xor_b32_e32 v33, v35, v32\n\t"
    "v_lshlrev_b64 v[2:3], 2, v[0:1]\n\t"
    "v_add_co_u32_e32 v2, vcc, s16, v2\n\t"
    "v_addc_co_u32_e32 v3, vcc, v4, v3, vcc\n\t"
    "v_cmp_gt_u64_e32 vcc, s[18:19], v[0:1]\n\t"
    "s_and_saveexec_b64 s[20:21], vcc\n\t"
    "global_store_dword v[2:3], v33, off\n\t"
    "s_waitcnt vmcnt(0)\n\t"
    "s_mov_b64 exec, s[20:21]\n\t"
    "s_endpgm";
const Value *unaryValue(const Value *V, unsigned Op) {
  const auto *I = dyn_cast_or_null<Instruction>(V);
  return I && I->getOpcode() == Op && I->getNumOperands() == 1
      ? I->getOperand(0) : nullptr;
}
const Value *shift32(const Value *V) {
  const auto *I = dyn_cast_or_null<BinaryOperator>(V);
  const auto *C = I ? dyn_cast<ConstantInt>(I->getOperand(1)) : nullptr;
  return I && I->getOpcode() == Instruction::LShr && C && C->equalsInt(32)
      ? I->getOperand(0) : nullptr;
}
bool halves(const Value *Lo, const Value *Hi, const Value *InputValue) {
  return InputValue && unaryValue(Lo, Instruction::Trunc) == InputValue &&
      shift32(unaryValue(Hi, Instruction::Trunc)) == InputValue;
}
bool intrinsicValue(const Value *V, Intrinsic::ID Id) {
  const auto *Call = dyn_cast_or_null<CallInst>(V);
  return Call && Call->arg_empty() && Call->getCalledFunction() &&
      Call->getCalledFunction()->isDeclaration() &&
      Call->getCalledFunction()->getIntrinsicID() == Id &&
      Call->getType()->isIntegerTy(32);
}
bool ordinaryIntrinsicAttributes(const Function &F) {
  if (F.hasMetadata() ||
      F.arg_size() != 0 || !F.hasExternalLinkage() || !F.hasDefaultVisibility() ||
      F.getCallingConv() != CallingConv::C ||
      F.getDLLStorageClass() != GlobalValue::DefaultStorageClass || F.isDSOLocal() ||
      F.getUnnamedAddr() != GlobalValue::UnnamedAddr::None || F.hasSection() ||
      F.hasComdat() || F.hasGC() || F.hasPersonalityFn() || F.hasPrefixData() ||
      F.hasPrologueData() ||
      !F.doesNotAccessMemory() || !F.hasFnAttribute(Attribute::NoUnwind) ||
      !F.hasFnAttribute(Attribute::Speculatable) || !F.hasFnAttribute(Attribute::WillReturn))
    return false;
  // LLVM22 materializes the intrinsic's standard return attributes, including
  // noundef and workitem-id range(0,1024), while parsing the input. Compare the
  // complete typed attribute list with that exact intrinsic definition, not
  // with an empty return list or a permissive per-attribute allowlist.
  return F.getAttributes() ==
      Intrinsic::getAttributes(F.getContext(), F.getIntrinsicID(), F.getFunctionType());
}
// Complete typed dataflow/census, not source authority. The external runner
// must join the fresh actual source export and borrowed V17 lowering.
std::optional<std::string> bodySymbol(const Input &InputValue) {
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  std::string Text(InputValue.Bytes.begin(), InputValue.Bytes.end());
  auto M = parseAssemblyString(Text, Diagnostic, Context);
  if (!M || M->getTargetTriple().str() != TripleName ||
      !M->getModuleInlineAsm().empty() || !M->alias_empty() || !M->ifunc_empty() ||
      !M->global_empty() || !M->named_metadata_empty() || M->size() != 3 ||
      M->getDataLayoutStr() != createMachine()->createDataLayout().getStringRepresentation())
    return std::nullopt;
  std::string Verification;
  raw_string_ostream Errors(Verification);
  if (verifyModule(*M, &Errors)) return std::nullopt;
  const Function *Kernel = nullptr;
  for (const Function &F : *M) {
    if (F.isDeclaration()) {
      if (F.getIntrinsicID() != Intrinsic::amdgcn_workgroup_id_x &&
          F.getIntrinsicID() != Intrinsic::amdgcn_workitem_id_x) return std::nullopt;
      if (!ordinaryIntrinsicAttributes(F)) return std::nullopt;
    } else {
      if (Kernel) return std::nullopt;
      Kernel = &F;
    }
  }
  if (!Kernel || Kernel->getCallingConv() != CallingConv::AMDGPU_KERNEL ||
      Kernel->size() != 1 || Kernel->arg_size() != 5 || Kernel->isVarArg() ||
      !Kernel->getReturnType()->isVoidTy() || Kernel->hasFnAttribute(Attribute::Naked) ||
      Kernel->getEntryBlock().size() != 18 ||
      !isa<UnreachableInst>(Kernel->getEntryBlock().getTerminator()) ||
      Kernel->getName().empty() || Kernel->getName().size() > 128 ||
      !Kernel->hasExternalLinkage() || !Kernel->hasDefaultVisibility() ||
      Kernel->getDLLStorageClass() != GlobalValue::DefaultStorageClass ||
      Kernel->isDSOLocal() || Kernel->getUnnamedAddr() != GlobalValue::UnnamedAddr::None ||
      Kernel->hasSection() || Kernel->hasComdat() || Kernel->hasGC() ||
      Kernel->hasPersonalityFn() || Kernel->hasPrefixData() || Kernel->hasPrologueData() ||
      Kernel->getAttributes().getFnAttrs().getNumAttributes() != 4 ||
      Kernel->getAttributes().getRetAttrs().hasAttributes() ||
      !Kernel->hasFnAttribute(Attribute::NoUnwind)) return std::nullopt;
  for (unsigned I = 0; I < Kernel->arg_size(); ++I)
    if (Kernel->getAttributes().getParamAttrs(I).hasAttributes()) return std::nullopt;
  SmallVector<std::pair<unsigned,MDNode*>,4> Metadata;
  Kernel->getAllMetadata(Metadata);
  const MDNode *Launch = Kernel->getMetadata("reqd_work_group_size");
  if (Metadata.size() != 1 || !Launch || Launch->getNumOperands() != 3 ||
      Metadata[0].first != Context.getMDKindID("reqd_work_group_size"))
    return std::nullopt;
  for (unsigned I = 0; I < 3; ++I) {
    const auto *MDC = dyn_cast_or_null<ConstantAsMetadata>(Launch->getOperand(I).get());
    const auto *C = MDC ? dyn_cast<ConstantInt>(MDC->getValue()) : nullptr;
    if (!C || !C->getType()->isIntegerTy(32) || !C->equalsInt(I == 0 ? 64 : 1))
      return std::nullopt;
  }
  const auto *Pointer = dyn_cast<PointerType>(Kernel->getArg(0)->getType());
  if (!Pointer || Pointer->getAddressSpace() != 1 ||
      !Kernel->getArg(1)->getType()->isIntegerTy(64) ||
      !Kernel->getArg(2)->getType()->isIntegerTy(32) ||
      !Kernel->getArg(3)->getType()->isIntegerTy(32) ||
      !Kernel->getArg(4)->getType()->isIntegerTy(32) ||
      Kernel->getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
      Kernel->getFnAttribute("target-features").getValueAsString() !=
          "-xnack,-wavefrontsize32,+wavefrontsize64" ||
      Kernel->getFnAttribute("amdgpu-flat-work-group-size").getValueAsString() != "64,64" ||
      Kernel->hasFnAttribute("amdgpu-implicitarg-num-bytes")) return std::nullopt;
  const CallInst *Unit = nullptr;
  size_t Calls = 0;
  for (const Instruction &I : Kernel->getEntryBlock()) {
    if (I.hasPoisonGeneratingAnnotations() || I.hasMetadata()) return std::nullopt;
    if (const auto *Call = dyn_cast<CallInst>(&I)) {
      if (!Call->getAttributes().isEmpty() || Call->getNumOperandBundles() != 0 || Call->isTailCall() ||
          Call->getCallingConv() != CallingConv::C) return std::nullopt;
      ++Calls;
      if (Call->isInlineAsm()) {
        if (Unit) return std::nullopt;
        Unit = Call;
      } else if (!intrinsicValue(Call, Intrinsic::amdgcn_workgroup_id_x) &&
                 !intrinsicValue(Call, Intrinsic::amdgcn_workitem_id_x)) return std::nullopt;
    } else if (I.mayHaveSideEffects() && !isa<UnreachableInst>(I)) return std::nullopt;
  }
  if (!Unit || Calls != 3 || Unit->arg_size() != 9 || !Unit->getType()->isVoidTy() ||
      Unit->getNextNode() != Kernel->getEntryBlock().getTerminator()) return std::nullopt;
  const auto *Asm = dyn_cast<InlineAsm>(Unit->getCalledOperand());
  if (!Asm || !Asm->hasSideEffects() || Asm->isAlignStack() || Asm->canThrow() ||
      Asm->getDialect() != InlineAsm::AD_ATT || Asm->getAsmString() != BodyAssembly ||
      Asm->getConstraintString() != BodyConstraints ||
      !llvm::all_of(Unit->args(), [](const Use &U) { return U->getType()->isIntegerTy(32); }))
    return std::nullopt;
  const auto *Ptr = dyn_cast_or_null<PtrToIntInst>(unaryValue(Unit->getArgOperand(0), Instruction::Trunc));
  if (!Ptr || Ptr->getPointerOperand() != Kernel->getArg(0) ||
      !Ptr->getType()->isIntegerTy(64) ||
      !halves(Unit->getArgOperand(0), Unit->getArgOperand(1), Ptr) ||
      !halves(Unit->getArgOperand(2), Unit->getArgOperand(3), Kernel->getArg(1)) ||
      Unit->getArgOperand(4) != Kernel->getArg(2) ||
      Unit->getArgOperand(5) != Kernel->getArg(3) ||
      Unit->getArgOperand(6) != Kernel->getArg(4)) return std::nullopt;
  const auto *Index = dyn_cast_or_null<BinaryOperator>(unaryValue(Unit->getArgOperand(7), Instruction::Trunc));
  if (!Index || Index->getOpcode() != Instruction::Add || !Index->getType()->isIntegerTy(64) ||
      Index->hasNoUnsignedWrap() || Index->hasNoSignedWrap() ||
      !halves(Unit->getArgOperand(7), Unit->getArgOperand(8), Index)) return std::nullopt;
  const auto *Base = dyn_cast<BinaryOperator>(Index->getOperand(0));
  const auto *Stride = Base ? dyn_cast<ConstantInt>(Base->getOperand(1)) : nullptr;
  if (!Base || Base->getOpcode() != Instruction::Mul || !Stride || !Stride->equalsInt(64) ||
      Base->hasNoUnsignedWrap() || Base->hasNoSignedWrap() ||
      !intrinsicValue(unaryValue(Base->getOperand(0), Instruction::ZExt), Intrinsic::amdgcn_workgroup_id_x) ||
      !intrinsicValue(unaryValue(Index->getOperand(1), Instruction::ZExt), Intrinsic::amdgcn_workitem_id_x))
    return std::nullopt;
  return Kernel->getName().str();
}
size_t bodyGuardNegatives(const Input &InputValue) {
  const std::array<std::pair<StringRef,StringRef>, 17> Changes{{
    {"~{memory}", "~{v5}"}, {"~{s21}", "~{s22}"},
    {"global_store_dword v[2:3], v33, off", "global_store_dword v[2:3], v34, off"},
    {"v_addc_co_u32_e32 v3, vcc, v4, v3, vcc", "v_add_co_u32_e32 v3, vcc, v4, v3"},
    {"v_cmp_gt_u64_e32", "v_cmp_gt_i64_e32"},
    {"s_mov_b64 exec, s[20:21]", "s_mov_b64 exec, s[22:23]"},
    {"lshr i64 %length, 32", "lshr i64 %length, 31"},
    {"mul i64 %group, 64", "mul i64 %group, 32"},
    {"i32 %a, i32 %b, i32 %c, i32 %idx_lo", "i32 %b, i32 %a, i32 %c, i32 %idx_lo"},
    {"v_xor_b32_e32 v32, v34, v35", "v_xor_b32_e32 v7, v34, v35"},
    {"!0 = !{i32 64, i32 1, i32 1}", "!0 = !{i32 32, i32 1, i32 1}"},
    {"lshr i64 %ptr, 32", "lshr exact i64 %ptr, 32"},
    {"(ptr addrspace(1) %data, i64 %length", "(ptr addrspace(1) %data, i32 %length"},
    {"define amdgpu_kernel", "define internal amdgpu_kernel"},
    {"declare i32 @llvm.amdgcn.workgroup.id.x()", "declare fastcc i32 @llvm.amdgcn.workgroup.id.x()"},
    {"i32 %idx_hi)\n  unreachable", "i32 %idx_hi) [ \"deopt\"(i32 0) ]\n  unreachable"},
    {"target datalayout = \"e-p:64:64", "target datalayout = \"e-p:32:32"}
  }};
  size_t Count = 0;
  for (const auto &[From,To] : Changes) {
    std::string Text(InputValue.Bytes.begin(), InputValue.Bytes.end());
    size_t At = Text.find(From.str());
    require(At != std::string::npos && Text.find(From.str(), At + From.size()) == std::string::npos,
            "negative mutation anchor absent/ambiguous");
    Text.replace(At, From.size(), To.str());
    Input Changed = InputValue;
    Changed.Bytes.assign(Text.begin(), Text.end());
    Changed.Digest = SHA256::hash(Changed.Bytes);
    require(!bodySymbol(Changed), "changed ABI/dataflow/effect/clobber input accepted");
    ++Count;
  }
  return Count;
}
std::vector<std::string> registers(const PhysicalMachineInstructionTrace &I) {
  std::vector<std::string> Result;
  for (const auto &O : I.Operands)
    if (O.Kind == PhysicalMachineOperandKind::Register) Result.push_back(O.Register);
  return Result;
}
void validateTail(const PhysicalMachineEffectEvidence &E, size_t P,
                  ArrayRef<uint8_t> Payload, StringRef Symbol) {
  require(P + 12 == E.Instructions.size(), "tail is not complete remainder of entry");
  const auto &Entry = E.Entries[0];
  uint64_t Cursor = Entry.CodeOffset;
  for (size_t I = 0; I < E.Instructions.size(); ++I) {
    const auto &Site = E.Instructions[I];
    require(Site.FunctionSymbol == Symbol && Site.InstructionOffset == Cursor &&
        !Site.Encoding.empty() && Cursor <= Payload.size() &&
        Site.Encoding.size() <= Payload.size() - Cursor &&
        std::equal(Site.Encoding.begin(), Site.Encoding.end(), Payload.begin() + Cursor),
        "complete native entry bytes/trace discontinuity");
    Cursor += Site.Encoding.size();
    if (I < P) require(Site.MemoryAccess != PhysicalMachineMemoryAccess::Write &&
        Site.MemoryAccess != PhysicalMachineMemoryAccess::ReadWrite &&
        Site.MemoryAccess != PhysicalMachineMemoryAccess::WorkgroupWrite &&
        Site.MemoryAccess != PhysicalMachineMemoryAccess::WorkgroupReadWrite &&
        Site.BranchKind == PhysicalMachineBranchKind::None,
        "compiler prologue has a write/control transfer");
  }
  require(Cursor == Entry.CodeOffset + Entry.CodeSize, "uninspected entry suffix");
  const std::array<StringRef,9> Opcodes{
      "V_LSHLREV_B64_vi", "V_ADD_CO_U32_e32_gfx9", "V_ADDC_CO_U32_e32_gfx9",
      "V_CMP_GT_U64_e32_vi", "S_AND_SAVEEXEC_B64_vi", "GLOBAL_STORE_DWORD_vi",
      "S_WAITCNT_vi", "S_MOV_B64_vi", "S_ENDPGM_vi"};
  const std::array<std::vector<std::string>,9> Registers{{
      {"VGPR2_VGPR3","VGPR0_VGPR1"}, {"VGPR2","SGPR16","VGPR2"},
      {"VGPR3","VGPR4","VGPR3"},
      {"SGPR18_SGPR19","VGPR0_VGPR1"}, {"SGPR20_SGPR21","VCC"},
      {"VGPR2_VGPR3","VGPR33"}, {}, {"EXEC","SGPR20_SGPR21"}, {}}};
  // In e32 forms VCC is an implicit definition/use, not an MC operand.
  // The unchanged worker sorts the descriptor's implicit register names.
  const std::array<uint16_t,9> Definitions{{1,1,1,0,1,0,0,1,0}};
  const std::array<std::vector<std::string>,9> ImplicitUses{{
      {"EXEC"}, {"EXEC"}, {"EXEC","VCC"}, {"EXEC"}, {"EXEC"}, {"EXEC"},
      {}, {}, {}}};
  const std::array<std::vector<std::string>,9> ImplicitDefinitions{{
      {}, {"VCC"}, {"VCC"}, {"VCC"}, {"EXEC","SCC"}, {}, {}, {}, {}}};
  for (size_t I = 0; I < Opcodes.size(); ++I) {
    const auto &Site = E.Instructions[P + 3 + I];
    require(Site.Opcode == Opcodes[I] && registers(Site) == Registers[I],
        "native tail opcode or explicit register contract changed");
    require(Site.ExplicitDefinitionCount == Definitions[I] &&
        Site.ImplicitUses == ImplicitUses[I] &&
        Site.ImplicitDefinitions == ImplicitDefinitions[I],
        "native tail explicit/implicit register effects changed");
    require(Site.BranchKind == (I == 8 ? PhysicalMachineBranchKind::Return :
        PhysicalMachineBranchKind::None), "unexpected tail control transfer");
    require(Site.MemoryAccess == (I == 5 ? PhysicalMachineMemoryAccess::Write :
        PhysicalMachineMemoryAccess::None), "unexpected tail memory effect");
    if (I == 5) require(Site.MemoryWidth == 4 && Site.Encoding.size() == 8 &&
        support::endian::read64le(Site.Encoding.data()) == 0x007f2102dc708000ULL,
        "tail global store width/address/data/off/modifier encoding");
    if (I == 0) require(llvm::any_of(Site.Operands, [](const auto &O) {
        return O.Kind == PhysicalMachineOperandKind::SignedImmediate && O.Value == 2;
      }), "address scale is not four bytes");
    if (I == 6) require(Site.Encoding.size() == 4 &&
        support::endian::read32le(Site.Encoding.data()) == 0xbf8c0f70,
        "wait must retain vmcnt(0), maximal expcnt/lgkmcnt");
    if (I == 8) require(Site.Encoding.size() == 4 &&
        support::endian::read32le(Site.Encoding.data()) == 0xbf810000,
        "authored s_endpgm changed");
  }
}
uint32_t descriptorKernarg(const BuiltFixture &Built, StringRef Symbol) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  auto Object = unwrap(object::ObjectFile::createObjectFile(
      MemoryBufferRef(StringRef(reinterpret_cast<const char*>(Payload.data()), Payload.size()), "<source-body>")));
  std::optional<uint32_t> Size;
  size_t Count = 0;
  for (const auto &S : Object->symbols()) {
    require(++Count <= 4096, "descriptor scan bound");
    if (unwrap(S.getName()) != Symbol.str() + ".kd") continue;
    require(!Size, "duplicate descriptor");
    auto Section = unwrap(S.getSection());
    require(Section != Object->section_end(), "descriptor section");
    auto Data = unwrap(Section->getContents());
    uint64_t Address = unwrap(S.getAddress());
    require(Address >= Section->getAddress(), "descriptor location");
    uint64_t Offset = Address - Section->getAddress();
    require(Offset <= Data.size() && Data.size() - Offset >= 64, "descriptor extent");
    auto Bytes = ArrayRef<uint8_t>(reinterpret_cast<const uint8_t*>(Data.data()) + Offset, 64);
    require(SHA256::hash(Bytes) == Built.Evidence.Entries[0].DescriptorIdentity, "descriptor native identity");
    using namespace llvm::amdhsa;
    require(support::endian::read32le(Bytes.data() + GROUP_SEGMENT_FIXED_SIZE_OFFSET) == 0 &&
        support::endian::read32le(Bytes.data() + PRIVATE_SEGMENT_FIXED_SIZE_OFFSET) == 0 &&
        !(support::endian::read16le(Bytes.data() + KERNEL_CODE_PROPERTIES_OFFSET) &
          KERNEL_CODE_PROPERTY_USES_DYNAMIC_STACK), "unexpected LDS/private/dynamic stack");
    Size = support::endian::read32le(Bytes.data() + KERNARG_SIZE_OFFSET);
  }
  require(Size.has_value() && *Size == 288,
      "expected explicit ptr/len/three-u32 layout aligned to32 plus unchanged256 hidden kernargs");
  return *Size;
}
json::Object bodyCase(const Input &InputValue, StringRef Symbol, OptimizationLevel Level) {
  auto Built = buildRequest(makeInputRequest(InputValue, Level, Symbol), Symbol);
  const auto &E = Built.Evidence;
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const auto Position = locateProgram(Profile::Three, E, Payload, Symbol);
  require(Position.has_value(), "independent three-op e32 contract absent/ambiguous");
  validateTail(E, *Position, Payload, Symbol);
  json::Array Instructions, PostLink;
  for (const auto &Diagnostic : Built.InspectionDiagnostics)
    PostLink.emplace_back(Diagnostic);
  for (const auto &I : E.Instructions) {
    auto Row = instructionJson(I);
    json::Array FullOperands;
    for (const auto &O : I.Operands) FullOperands.emplace_back(json::Object{
      {"kind", static_cast<unsigned>(O.Kind)}, {"register", O.Register},
      {"value_u64_decimal", std::to_string(O.Value)}, {"tied_to", O.TiedTo}});
    Row["operands"] = std::move(FullOperands);
    Row["explicit_definitions"] = I.ExplicitDefinitionCount;
    Row["memory_access"] = static_cast<unsigned>(I.MemoryAccess);
    Row["memory_width"] = I.MemoryWidth;
    Instructions.emplace_back(std::move(Row));
  }
  return json::Object{
    {"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
    {"llvm_sha256", hex(InputValue.Digest)},
    {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)},
    {"hsaco_bytes", Payload.size()}, {"entry_offset", E.Entries[0].CodeOffset},
    {"entry_bytes", E.Entries[0].CodeSize},
    {"compiler_owned_prologue_instructions", *Position},
    {"authored_tail_first_offset", E.Instructions[*Position].InstructionOffset},
    {"authored_tail_instructions", 12},
    {"kernarg_bytes", descriptorKernarg(Built, Symbol)},
    {"hidden_kernarg_bytes", 256}, {"descriptor_resources", descriptorResources(Built, Symbol)},
    {"complete_entry_trace", std::move(Instructions)},
    {"post_link_metadata_checks", std::move(PostLink)},
    {"native_functional_execution", false}};
}
}
int main(int Argc, char **Argv) {
  alarm(90);
  require(Argc == 2, "usage: source-body-abi-candidate ABS_SOURCE_DERIVED_LLVM");
  const Input InputValue = readSourceLlvm(Argv[1]);
  const auto Symbol = bodySymbol(InputValue);
  require(Symbol.has_value(), "typed complete source-derived LLVM profile refused");
  const size_t Negatives = bodyGuardNegatives(InputValue);
  json::Array Cases;
  Cases.emplace_back(bodyCase(InputValue, *Symbol, OptimizationLevel::O0));
  Cases.emplace_back(bodyCase(InputValue, *Symbol, OptimizationLevel::O3));
  const Input After = readSourceLlvm(Argv[1]);
  require(After.Digest == InputValue.Digest && After.Bytes == InputValue.Bytes,
      "input changed across native work");
  emitReport(json::Object{
    {"schema","private-source-fed-body-abi-v1"}, {"authority","observation_only"},
    {"source_join","external fresh-source runner required"},
    {"synthetic_worker_identity_fields",true}, {"target","gfx942:xnack-"},
    {"wave_width",64}, {"workgroup_size",64}, {"code_object_version",6},
    {"abi_and_index_prologue","compiler_owned"}, {"naked_body",false},
    {"complete_physical_register_kernel_api",false},
    {"input_guard_negatives",Negatives}, {"cases",std::move(Cases)},
    {"hardware_executed",false}, {"native_functional_proof",false},
    {"protected_finalizer_admission",false}});
  alarm(0);
  return 0;
}
