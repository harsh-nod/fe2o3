// TEST-ONLY closed one/three/sixteen-step native observation.
// CLI profile names select three literal programs, never arbitrary serialized
// program authority. Input LLVM is unauthenticated; the external runner joins
// source/export/owner/emitter observations. Neither those joins nor these
// synthetic worker requests authorize protected publication or GPU execution.
//
// Independent encoding cross-check, not the pinned gfx942 toolchain identity:
// https://github.com/llvm/llvm-project/blob/2078da43e25a4623cab2d0d60decddf709aaea28/llvm/test/MC/AMDGPU/gfx9_asm_vop1.s
// https://github.com/llvm/llvm-project/blob/2078da43e25a4623cab2d0d60decddf709aaea28/llvm/test/MC/AMDGPU/gfx9_asm_vop2.s
// Those public gfx900 examples cross-check the GFX9 field formulas below.
// Actual pinned gfx942 assembly, decoding and final emission must still agree.

#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/SourceMgr.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"

#include <algorithm>
#include <array>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <fcntl.h>
#include <memory>
#include <optional>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <utility>
#include <vector>

#if !defined(FE2O3_LLVM_BUILD_ID) || !defined(FE2O3_WORKER_BUILD_ID)
#error "the existing worker CMake identities must be supplied"
#endif

using namespace fe2o3::worker;
using namespace llvm;

namespace {
constexpr StringLiteral TripleName = "amdgcn-amd-amdhsa";
constexpr StringLiteral KernelName = "ordered_program_fixture";
constexpr StringLiteral Constraints = "=&{v33},{v34},{v35},{v36},~{v32}";
constexpr size_t FixtureByteLimit = 64 * 1024;
constexpr size_t PayloadByteLimit = 1024 * 1024;
constexpr size_t InstructionLimit = 512;
constexpr size_t ReportByteLimit = 64 * 1024;
constexpr uint16_t AllowedUnitFlags =
    1 << 4; // Existing Predicable trace bit only.

[[noreturn]] void fail(StringRef Message) {
  errs() << "ordered-program native observation failed: "
         << Message.take_front(4096) << '\n';
  std::exit(1);
}
void require(bool Condition, StringRef Message) {
  if (!Condition)
    fail(Message);
}
template <typename T> T unwrap(Expected<T> Value) {
  if (!Value)
    fail(toString(Value.takeError()));
  return std::move(*Value);
}
std::string hex(ArrayRef<uint8_t> Bytes) {
  constexpr char Digits[] = "0123456789abcdef";
  std::string Result;
  Result.reserve(Bytes.size() * 2);
  for (uint8_t Byte : Bytes) {
    Result.push_back(Digits[Byte >> 4]);
    Result.push_back(Digits[Byte & 15]);
  }
  return Result;
}

enum class Profile { One, Three, Sixteen };
enum class Opcode { Mov, Add, Sub, And, Or, Xor };
struct Step {
  Opcode Op;
  uint8_t Destination;
  uint8_t Source0;
  uint8_t Source1; // Zero and unused for mov.
};

// Independently transcribed from the literal source contract, not the Rust
// descriptor decoder, LLVM emitter, caller JSON or final machine output.
constexpr std::array<Step, 1> One{{{Opcode::Mov, 33, 34, 0}}};
constexpr std::array<Step, 3> Three{{{Opcode::Xor, 32, 34, 35},
                                     {Opcode::And, 32, 32, 36},
                                     {Opcode::Xor, 33, 35, 32}}};
constexpr std::array<Step, 16> Sixteen{{
    {Opcode::Mov, 32, 34, 0},
    {Opcode::Xor, 33, 34, 35},
    {Opcode::And, 32, 33, 36},
    {Opcode::Or, 33, 32, 35},
    {Opcode::Add, 32, 33, 36},
    {Opcode::Sub, 33, 32, 34},
    {Opcode::Mov, 32, 33, 0},
    {Opcode::Xor, 32, 32, 35},
    {Opcode::Or, 33, 32, 34},
    {Opcode::And, 33, 33, 36},
    {Opcode::Add, 33, 33, 34},
    {Opcode::Sub, 32, 33, 35},
    {Opcode::Mov, 33, 32, 0},
    {Opcode::Xor, 33, 33, 36},
    {Opcode::Mov, 32, 35, 0}, // Dead scratch write must remain.
    {Opcode::Mov, 33, 33, 0}, // Output self-move must remain.
}};

ArrayRef<Step> steps(Profile Value) {
  switch (Value) {
  case Profile::One:
    return One;
  case Profile::Three:
    return Three;
  case Profile::Sixteen:
    return Sixteen;
  }
  fail("unknown closed program profile");
}
StringRef profileName(Profile Value) {
  switch (Value) {
  case Profile::One:
    return "one";
  case Profile::Three:
    return "three";
  case Profile::Sixteen:
    return "sixteen";
  }
  fail("unknown closed program profile");
}
Profile parseProfile(StringRef Name) {
  if (Name == "one")
    return Profile::One;
  if (Name == "three")
    return Profile::Three;
  if (Name == "sixteen")
    return Profile::Sixteen;
  fail("profile must be one, three or sixteen");
}
StringRef mnemonic(Opcode Op) {
  switch (Op) {
  case Opcode::Mov:
    return "v_mov_b32_e32";
  case Opcode::Add:
    return "v_add_u32_e32";
  case Opcode::Sub:
    return "v_sub_u32_e32";
  case Opcode::And:
    return "v_and_b32_e32";
  case Opcode::Or:
    return "v_or_b32_e32";
  case Opcode::Xor:
    return "v_xor_b32_e32";
  }
  fail("unknown closed opcode");
}
StringRef machineOpcode(Opcode Op) {
  switch (Op) {
  case Opcode::Mov:
    return "V_MOV_B32_e32_vi";
  case Opcode::Add:
    return "V_ADD_U32_e32_gfx9";
  case Opcode::Sub:
    return "V_SUB_U32_e32_gfx9";
  case Opcode::And:
    return "V_AND_B32_e32_vi";
  case Opcode::Or:
    return "V_OR_B32_e32_vi";
  case Opcode::Xor:
    return "V_XOR_B32_e32_vi";
  }
  fail("unknown closed opcode");
}
constexpr uint32_t expectedWord(Step Value) {
  // VOP1 mov: base7e000200 | vdst[24:17] | vector src0(256+r).
  // VOP2: base high byte | vdst[24:17] | vsrc1[16:9] | src0[8:0].
  uint32_t Base = 0;
  switch (Value.Op) {
  case Opcode::Mov:
    Base = 0x7e000200;
    break;
  case Opcode::Add:
    Base = 0x68000000;
    break;
  case Opcode::Sub:
    Base = 0x6a000000;
    break;
  case Opcode::And:
    Base = 0x26000000;
    break;
  case Opcode::Or:
    Base = 0x28000000;
    break;
  case Opcode::Xor:
    Base = 0x2a000000;
    break;
  }
  return Base | (uint32_t(Value.Destination) << 17) |
         (Value.Op == Opcode::Mov ? 0 : uint32_t(Value.Source1) << 9) |
         (256 + uint32_t(Value.Source0));
}
// The six public gfx900 examples, not observed output from this candidate.
static_assert(expectedWord({Opcode::Mov, 5, 1, 0}) == 0x7e0a0301);
static_assert(expectedWord({Opcode::Add, 5, 1, 2}) == 0x680a0501);
static_assert(expectedWord({Opcode::Sub, 5, 1, 2}) == 0x6a0a0501);
static_assert(expectedWord({Opcode::And, 5, 1, 2}) == 0x260a0501);
static_assert(expectedWord({Opcode::Or, 5, 1, 2}) == 0x280a0501);
static_assert(expectedWord({Opcode::Xor, 5, 1, 2}) == 0x2a0a0501);

std::string assemblyRegister(uint8_t Value) {
  switch (Value) {
  case 32:
    return "v32";
  case 33:
    return "$0";
  case 34:
    return "$1";
  case 35:
    return "$2";
  case 36:
    return "$3";
  default:
    fail("register is outside the closed fixture roles");
  }
}
std::string assembly(Profile Kind, bool WideControl = false) {
  require(!WideControl || Kind == Profile::Three,
          "wide control is fixed three-step only");
  std::string Text;
  size_t Index = 0;
  for (Step Value : steps(Kind)) {
    if (Index)
      Text += "\n\t";
    Text +=
        WideControl && Index == 0 ? "v_xor_b32_e64" : mnemonic(Value.Op).str();
    Text += " " + assemblyRegister(Value.Destination) + ", " +
            assemblyRegister(Value.Source0);
    if (Value.Op != Opcode::Mov)
      Text += ", " + assemblyRegister(Value.Source1);
    ++Index;
  }
  return Text;
}

#include "OrderedProgramWorkerSupport.inc"

Input makeFixture(Profile Kind, bool Used, bool WideControl = false) {
  LLVMContext Context;
  Module ModuleValue("synthetic-not-source-ordered-program-test", Context);
  auto Machine = createMachine();
  ModuleValue.setTargetTriple(Triple(TripleName));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);
  Type *I32 = Type::getInt32Ty(Context);
  Type *GlobalPointer = PointerType::get(Context, 1);
  FunctionType *KernelType = FunctionType::get(
      Type::getVoidTy(Context), {GlobalPointer, I32, I32, I32}, false);
  Function *Kernel = Function::Create(KernelType, GlobalValue::ExternalLinkage,
                                      KernelName, ModuleValue);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr("target-cpu", "gfx942");
  Kernel->addFnAttr("target-features",
                    "-xnack,-wavefrontsize32,+wavefrontsize64");
  Kernel->addFnAttr("amdgpu-flat-work-group-size", "64,64");
  Metadata *Workgroup[] = {ConstantAsMetadata::get(ConstantInt::get(I32, 64)),
                           ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
                           ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
  Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, Workgroup));
  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
  IRBuilder<> Builder(Entry);
  FunctionType *UnitType = FunctionType::get(I32, {I32, I32, I32}, false);
  InlineAsm *Unit =
      InlineAsm::get(UnitType, assembly(Kind, WideControl), Constraints, true);
  CallInst *Result = Builder.CreateCall(
      UnitType, Unit,
      {Kernel->getArg(1), Kernel->getArg(2), Kernel->getArg(3)});
  Builder
      .CreateStore(Used ? static_cast<Value *>(Result) : Kernel->getArg(1),
                   Kernel->getArg(0))
      ->setVolatile(true);
  Builder.CreateRetVoid();
  std::string Diagnostic;
  raw_string_ostream Errors(Diagnostic);
  require(!verifyModule(ModuleValue, &Errors),
          "synthetic LLVM fixture does not verify");
  std::string Text;
  raw_string_ostream Output(Text);
  ModuleValue.print(Output, nullptr);
  Output.flush();
  require(!Text.empty() && Text.size() <= FixtureByteLimit,
          "synthetic LLVM fixture byte cap");
  Input Value;
  Value.Kind = InputKind::LlvmTextIr;
  Value.Bytes.assign(Text.begin(), Text.end());
  Value.Digest = SHA256::hash(Value.Bytes);
  return Value;
}

bool matchesInstruction(const PhysicalMachineInstructionTrace &Actual,
                        Step Expected, ArrayRef<uint8_t> Payload,
                        StringRef Symbol) {
  const size_t Arity = Expected.Op == Opcode::Mov ? 2 : 3;
  if (Actual.FunctionSymbol != Symbol ||
      Actual.Opcode != machineOpcode(Expected.Op) ||
      Actual.Encoding.size() != 4 || Actual.ExplicitDefinitionCount != 1 ||
      Actual.Operands.size() != Arity || !Actual.ImplicitDefinitions.empty() ||
      Actual.ImplicitUses != std::vector<std::string>{"EXEC"} ||
      Actual.MemoryAccess != PhysicalMachineMemoryAccess::None ||
      Actual.MemoryWidth != 0 ||
      (Actual.Flags & static_cast<uint16_t>(~AllowedUnitFlags)) != 0 ||
      Actual.BranchKind != PhysicalMachineBranchKind::None ||
      Actual.BranchTarget != 0 || Actual.InstructionOffset > Payload.size() ||
      Payload.size() - Actual.InstructionOffset < 4)
    return false;
  const std::array<uint8_t, 3> Registers{Expected.Destination, Expected.Source0,
                                         Expected.Source1};
  for (size_t Index = 0; Index < Arity; ++Index) {
    const auto &Operand = Actual.Operands[Index];
    if (Operand.Kind != PhysicalMachineOperandKind::Register ||
        Operand.TiedTo != -1 ||
        Operand.Register != "VGPR" + std::to_string(Registers[Index]))
      return false;
  }
  const auto Raw = Payload.slice(Actual.InstructionOffset, 4);
  return std::equal(Raw.begin(), Raw.end(), Actual.Encoding.begin()) &&
         support::endian::read32le(Raw.data()) == expectedWord(Expected);
}

std::optional<size_t>
locateProgram(Profile Kind, const PhysicalMachineEffectEvidence &Evidence,
              ArrayRef<uint8_t> Payload, StringRef Symbol) {
  const auto Expected = steps(Kind);
  if (Payload.empty() || Payload.size() > PayloadByteLimit ||
      Evidence.Instructions.size() < Expected.size() ||
      Evidence.Instructions.size() > InstructionLimit ||
      Evidence.PayloadDigest != SHA256::hash(Payload) ||
      Evidence.PayloadBytes != Payload.size() || Evidence.Entries.size() != 1 ||
      Evidence.Entries[0].Symbol != Symbol)
    return std::nullopt;
  const auto &Entry = Evidence.Entries[0];
  if (Entry.CodeOffset > Payload.size() ||
      Entry.CodeSize > Payload.size() - Entry.CodeOffset)
    return std::nullopt;
  std::optional<size_t> Found;
  for (size_t Index = 0;
       Index <= Evidence.Instructions.size() - Expected.size(); ++Index) {
    bool Match = true;
    const auto &First = Evidence.Instructions[Index];
    for (size_t Offset = 0; Offset < Expected.size(); ++Offset) {
      const auto &Site = Evidence.Instructions[Index + Offset];
      Match &=
          matchesInstruction(Site, Expected[Offset], Payload, Symbol) &&
          Site.BlockOrdinal == First.BlockOrdinal &&
          Site.InstructionOffset == First.InstructionOffset + 4 * Offset &&
          Site.InstructionOffset >= Entry.CodeOffset &&
          Site.InstructionOffset - Entry.CodeOffset <= Entry.CodeSize &&
          Entry.CodeSize - (Site.InstructionOffset - Entry.CodeOffset) >= 4;
    }
    if (!Match)
      continue;
    if (Found)
      return std::nullopt; // Never arbitrary first-match selection.
    Found = Index;
  }
  return Found;
}

bool footprint(StringRef Register) {
  return Register == "VGPR32" || Register == "VGPR33" || Register == "VGPR34" ||
         Register == "VGPR35" || Register == "VGPR36";
}
json::Object instructionJson(const PhysicalMachineInstructionTrace &Site) {
  json::Array Operands, Uses, Definitions;
  for (const auto &Operand : Site.Operands)
    if (Operand.Kind == PhysicalMachineOperandKind::Register)
      Operands.emplace_back(Operand.Register);
  for (const auto &Use : Site.ImplicitUses)
    Uses.emplace_back(Use);
  for (const auto &Definition : Site.ImplicitDefinitions)
    Definitions.emplace_back(Definition);
  return json::Object{{"file_offset", Site.InstructionOffset},
                      {"opcode", Site.Opcode},
                      {"bytes_hex", hex(Site.Encoding)},
                      {"mc_flags", Site.Flags},
                      {"register_operands", std::move(Operands)},
                      {"implicit_reads", std::move(Uses)},
                      {"implicit_writes", std::move(Definitions)}};
}
json::Array expectedJson(Profile Kind) {
  json::Array Result;
  for (Step Value : steps(Kind)) {
    json::Array Registers;
    Registers.emplace_back("VGPR" + std::to_string(Value.Destination));
    Registers.emplace_back("VGPR" + std::to_string(Value.Source0));
    if (Value.Op != Opcode::Mov)
      Registers.emplace_back("VGPR" + std::to_string(Value.Source1));
    std::array<uint8_t, 4> Bytes{};
    support::endian::write32le(Bytes.data(), expectedWord(Value));
    Result.emplace_back(
        json::Object{{"mnemonic", mnemonic(Value.Op)},
                     {"opcode", machineOpcode(Value.Op)},
                     {"bytes_hex", hex(Bytes)},
                     {"register_operands", std::move(Registers)}});
  }
  return Result;
}
json::Object describePositive(Profile Kind, const BuiltFixture &Built,
                              OptimizationLevel Level, bool Used,
                              StringRef Symbol) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const auto Position = locateProgram(Kind, Built.Evidence, Payload, Symbol);
  require(Position.has_value(),
          "complete ordered e32 program is absent or ambiguous");
  const size_t Count = steps(Kind).size();
  json::Array Program, Boundary;
  for (size_t Offset = 0; Offset < Count; ++Offset)
    Program.emplace_back(
        instructionJson(Built.Evidence.Instructions[*Position + Offset]));
  std::array<bool, 3> InputDefinitions{false, false, false};
  bool OutputUse = false;
  size_t BoundarySites = 0;
  for (size_t Index = 0; Index < Built.Evidence.Instructions.size(); ++Index) {
    if (Index >= *Position && Index < *Position + Count)
      continue;
    const auto &Site = Built.Evidence.Instructions[Index];
    bool Touches = false;
    for (size_t OperandIndex = 0; OperandIndex < Site.Operands.size();
         ++OperandIndex) {
      const auto &Operand = Site.Operands[OperandIndex];
      if (Operand.Kind != PhysicalMachineOperandKind::Register)
        continue;
      Touches |= footprint(Operand.Register);
      if (Index < *Position && OperandIndex < Site.ExplicitDefinitionCount)
        for (size_t Input = 0; Input < 3; ++Input)
          InputDefinitions[Input] |=
              Operand.Register == "VGPR" + std::to_string(34 + Input);
      if (Index >= *Position + Count &&
          OperandIndex >= Site.ExplicitDefinitionCount)
        OutputUse |= Operand.Register == "VGPR33";
    }
    if (Touches) {
      ++BoundarySites;
      if (Boundary.size() < 32)
        Boundary.emplace_back(instructionJson(Site));
    }
  }
  require(std::all_of(InputDefinitions.begin(), InputDefinitions.end(),
                      [](bool Value) { return Value; }),
          "fixed input materialization not observed before program");
  require(!Used || OutputUse,
          "used-result fixture has no observed output use after program");
  json::Array Diagnostics;
  for (const auto &Value : Built.InspectionDiagnostics)
    Diagnostics.emplace_back(Value);
  return json::Object{
      {"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
      {"profile", profileName(Kind)},
      {"program_count", Count},
      {"result_used", Used},
      {"llvm_text_sha256", hex(Built.RequestValue.CompilerModule.Digest)},
      {"llvm_text_bytes", Built.RequestValue.CompilerModule.Bytes.size()},
      {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)},
      {"hsaco_bytes", Payload.size()},
      {"descriptor_sha256", hex(Built.Evidence.Entries[0].DescriptorIdentity)},
      {"entry_file_offset", Built.Evidence.Entries[0].CodeOffset},
      {"entry_code_bytes", Built.Evidence.Entries[0].CodeSize},
      {"static_instruction_count", Built.Evidence.Instructions.size()},
      {"post_link_inspection_diagnostics", std::move(Diagnostics)},
      {"program", std::move(Program)},
      {"boundary_register_site_count", BoundarySites},
      {"boundary_sites", std::move(Boundary)},
      {"boundary_sites_truncated", BoundarySites > 32},
      {"boundary_observation_is_value_or_lifetime_proof", false}};
}

size_t rejectDecodedControls(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const size_t Position =
      *locateProgram(Profile::Three, Built.Evidence, Payload, KernelName);
  size_t Count = 0;
  auto Check = [&](PhysicalMachineEffectEvidence Changed) {
    require(!locateProgram(Profile::Three, Changed, Payload, KernelName),
            "matcher accepted a corrupted decoded observation");
    ++Count;
  };
  auto Changed = Built.Evidence;
  Changed.Instructions[Position].Opcode = "V_XOR_B32_e64_vi";
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].Encoding.push_back(0);
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].Operands[1].Register = "VGPR35";
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].Operands[1].TiedTo = 0;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].ExplicitDefinitionCount = 2;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].ImplicitDefinitions = {"EXEC"};
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].ImplicitUses.clear();
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].Flags |= 1;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].MemoryAccess =
      PhysicalMachineMemoryAccess::Read;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].BranchKind =
      PhysicalMachineBranchKind::ConditionalDirect;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  ++Changed.Instructions[Position + 1].BlockOrdinal;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  ++Changed.Instructions[Position + 1].InstructionOffset;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions.erase(Changed.Instructions.begin() + Position + 1);
  Check(std::move(Changed));
  Changed = Built.Evidence;
  std::swap(Changed.Instructions[Position], Changed.Instructions[Position + 1]);
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position + 2] = Changed.Instructions[Position];
  Check(std::move(Changed));
  Changed = Built.Evidence;
  for (size_t Index = 0; Index < Three.size(); ++Index)
    Changed.Instructions.push_back(
        Built.Evidence.Instructions[Position + Index]);
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.PayloadDigest[0] ^= 1;
  Check(std::move(Changed));
  require(!locateProgram(Profile::Sixteen, Built.Evidence, Payload, KernelName),
          "wrong closed count/profile matched three-step fixture");
  ++Count;
  return Count;
}

size_t rejectPayloadControls(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const size_t Position =
      *locateProgram(Profile::Three, Built.Evidence, Payload, KernelName);
  const uint64_t Offset =
      Built.Evidence.Instructions[Position].InstructionOffset;
  require(Offset <= Payload.size() && Payload.size() - Offset >= 12,
          "mutation range outside payload");
  auto Check = [&](const std::vector<uint8_t> &Changed) {
    auto Evidence = analyzePayload(Changed);
    require(!locateProgram(Profile::Three, Evidence, Changed, KernelName),
            "redecoded payload mutation matched original program");
  };
  auto Changed = Payload;
  Changed[Offset] ^= 1;
  Check(Changed); // Valid src0 v34 -> v35.
  Changed = Payload;
  std::swap_ranges(Changed.begin() + Offset, Changed.begin() + Offset + 4,
                   Changed.begin() + Offset + 4);
  Check(Changed);
  Changed = Payload;
  std::copy_n(Changed.begin() + Offset, 4, Changed.begin() + Offset + 8);
  Check(Changed);
  return 3;
}

void rejectActualWide(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  require(!locateProgram(Profile::Three, Built.Evidence, Payload, KernelName),
          "real e64 substitution matched closed e32 program");
  size_t Found = 0;
  for (size_t Index = 0; Index + 2 < Built.Evidence.Instructions.size();
       ++Index) {
    const auto &First = Built.Evidence.Instructions[Index];
    if (First.FunctionSymbol != KernelName ||
        First.Opcode != "V_XOR_B32_e64_vi" || First.Encoding.size() != 8 ||
        First.InstructionOffset > Payload.size() ||
        Payload.size() - First.InstructionOffset < 8 ||
        !First.ImplicitDefinitions.empty() ||
        First.ImplicitUses != std::vector<std::string>{"EXEC"} ||
        First.MemoryAccess != PhysicalMachineMemoryAccess::None ||
        First.MemoryWidth != 0 ||
        First.BranchKind != PhysicalMachineBranchKind::None ||
        First.BranchTarget != 0 ||
        (First.Flags & static_cast<uint16_t>(~AllowedUnitFlags)) != 0)
      continue;
    std::vector<std::string> Registers;
    for (const auto &Operand : First.Operands)
      if (Operand.Kind == PhysicalMachineOperandKind::Register)
        Registers.push_back(Operand.Register);
    const auto Raw =
        ArrayRef<uint8_t>(Payload).slice(First.InstructionOffset, 8);
    if (Registers != std::vector<std::string>{"VGPR32", "VGPR34", "VGPR35"} ||
        !std::equal(Raw.begin(), Raw.end(), First.Encoding.begin()))
      continue;
    bool Rest = true;
    for (size_t Offset = 1; Offset < Three.size(); ++Offset) {
      const auto &Site = Built.Evidence.Instructions[Index + Offset];
      Rest &= matchesInstruction(Site, Three[Offset], Payload, KernelName) &&
              Site.BlockOrdinal == First.BlockOrdinal &&
              Site.InstructionOffset ==
                  First.InstructionOffset + 8 + 4 * (Offset - 1);
    }
    Found += Rest;
  }
  require(Found == 1,
          "negative control did not actually emit one e64/e32/e32 program");
}

void emitReport(json::Object Report);
#include "OrderedProgramSourceObservation.inc"

void emitReport(json::Object Report) {
  std::string Text;
  raw_string_ostream Stream(Text);
  Stream << formatv("{0:2}", json::Value(std::move(Report)));
  Stream.flush();
  require(Text.size() + 1 <= ReportByteLimit, "program observation report cap");
  outs() << Text << '\n';
}

int syntheticQualification() {
  auto InlineAssemblyControls = inlineAssemblyGuardControls();
  json::Array Cases;
  std::optional<BuiltFixture> MutationBaseline;
  for (Profile Kind : {Profile::One, Profile::Three, Profile::Sixteen}) {
    for (OptimizationLevel Level :
         {OptimizationLevel::O0, OptimizationLevel::O3}) {
      for (bool Used : {true, false}) {
        const Input Fixture = makeFixture(Kind, Used);
        require(sourceKernelSymbol(Fixture, Kind, Used) == KernelName,
                "synthetic fixture did not meet the same input LLVM guard");
        auto Built = buildRequest(makeInputRequest(Fixture, Level));
        auto Observed = describePositive(Kind, Built, Level, Used, KernelName);
        // Retain actual instruction/resource observations for the independent
        // outer matcher. Only boundary/diagnostic detail is omitted here to
        // keep twelve cases within the unchanged 64 KiB report cap.
        Cases.emplace_back(json::Object{
            {"profile", profileName(Kind)},
            {"program_count", steps(Kind).size()},
            {"result_used", Used},
            {"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
            {"llvm_text_sha256", hex(Fixture.Digest)},
            {"llvm_text_bytes", Fixture.Bytes.size()},
            {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)},
            {"hsaco_bytes", Built.ResponseValue.LinkedOutput->Bytes.size()},
            {"descriptor_sha256", std::move(Observed["descriptor_sha256"])},
            {"entry_file_offset", std::move(Observed["entry_file_offset"])},
            {"entry_code_bytes", std::move(Observed["entry_code_bytes"])},
            {"static_instruction_count",
             std::move(Observed["static_instruction_count"])},
            {"program", std::move(Observed["program"])},
            {"descriptor_resources", descriptorResources(Built, KernelName)},
            {"complete_exact_sequence_observed", true},
            {"descriptor_capacity_checked", true}});
        if (Kind == Profile::Three && Level == OptimizationLevel::O3 && Used)
          MutationBaseline = std::move(Built);
      }
    }
  }
  require(MutationBaseline.has_value(), "missing emitted mutation baseline");
  const size_t Decoded = rejectDecodedControls(*MutationBaseline);
  const size_t Payload = rejectPayloadControls(*MutationBaseline);
  rejectActualWide(buildRequest(makeInputRequest(
      makeFixture(Profile::Three, true, true), OptimizationLevel::O3)));
  emitReport(json::Object{
      {"schema", "fe2o3-ordered-program-worker-prototype-v1"},
      {"authority", "unauthenticated-native-test-fixture"},
      {"source_produced", false},
      {"synthetic_worker_request_identity_fields", true},
      {"production_exact_program_admission", false},
      {"protected_finalizer_admission", false},
      {"hardware_executed", false},
      {"physical_register_allocation_or_lifetime_proof", false},
      {"whole_kernel_order_or_byte_stability_claim", false},
      {"runtime_closure_attestation", "unavailable"},
      {"target", "gfx942:xnack-"},
      {"wave_width", 64},
      {"workgroup_size", 64},
      {"code_object_version", 6},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID},
      {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"positive_cases", std::move(Cases)},
      {"inline_asm_guard_controls", std::move(InlineAssemblyControls)},
      {"decoded_observation_negatives", Decoded},
      {"actual_payload_mutation_negatives", Payload},
      {"actual_e64_rejected_by_matcher", true},
      {"encoding_reference_scope", "public gfx900 cross-check; actual pinned "
                                   "gfx942 qualification is this run"}});
  return 0;
}
} // namespace

int main(int Argc, char **Argv) {
  // In-process LLVM/LLD only. External runner must also cap process-group time
  // and stdout/stderr, and enforce disk/cache/RAM guards before this
  // executable.
  alarm(90);
  if (Argc == 1) {
    const int Result = syntheticQualification();
    alarm(0);
    return Result;
  }
  require(Argc == 4,
          "expected no args or one|three|sixteen used|unused ABS_LLVM");
  const Profile Kind = parseProfile(Argv[1]);
  const StringRef Mode(Argv[2]);
  require(Mode == "used" || Mode == "unused",
          "result mode must be used or unused");
  const int Result = observeSourceLlvm(Kind, Mode == "used", Argv[3]);
  alarm(0);
  return Result;
}
