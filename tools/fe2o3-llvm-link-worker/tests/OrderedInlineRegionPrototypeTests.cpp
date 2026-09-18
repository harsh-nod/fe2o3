// TEST-ONLY: closed worker transport and encoding experiment.
// This is an unauthenticated worker transport/encoding experiment. Its LLVM
// fixture is not Rust-source-produced, a canonical KIR subject, a compiler
// handoff, a protected finalizer input, or permission to execute an HSACO.
// No production parser, builder, source selector, or admission path is added.
//
// The expected GFX9 VOP2 words below are independently specified, not copied
// from this test's assembler output. Opcode/layout cross-check reference:
// https://github.com/llvm/llvm-project/blob/release/21.x/llvm/test/MC/AMDGPU/gfx9_asm_vop2.s
// (v_xor_b32 opcode 0x15; no-carry v_add_u32 opcode 0x34). That reference is
// an encoding cross-check, NOT the identity of the pinned package under test.
// Actual pinned compilation and MC decoding must agree before this test passes.

#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <limits>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <unistd.h>

#if !defined(FE2O3_LLVM_BUILD_ID) || !defined(FE2O3_WORKER_BUILD_ID)
#error "the existing worker CMake identities must be supplied"
#endif

using namespace fe2o3::worker;
using namespace llvm;

namespace {

constexpr StringLiteral TripleName = "amdgcn-amd-amdhsa";
constexpr StringLiteral KernelName = "ordered_region_fixture";
constexpr size_t FixtureByteLimit = 64 * 1024;
constexpr size_t PayloadByteLimit = 1024 * 1024;
constexpr size_t InstructionLimit = 512;
constexpr size_t ReportByteLimit = 64 * 1024;
// Existing WorkerMachineEffect.cpp trace flag layout: bit4 is Predicable.
// All other current bits (memory/terminator/barrier/trap), and any future bit,
// are forbidden for this closed integer unit. This is a test expectation only.
constexpr uint16_t AllowedUnitFlags = 1 << 4;

[[noreturn]] void fail(StringRef Message) {
  errs() << "ordered-inline-region prototype failed: "
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

// This closed *test guard* does not establish compiler or source authority.
// In particular, LLVM may accept missing clobbers; we never credit LLVM with
// rejecting an unsound contract merely because this guard rejects that case.
enum class TestTarget { Gfx942XnackMinus, Gfx950XnackMinus };
enum class TestOpcode { XorU32, AddU32, MemoryOperation };
struct ClosedTestContract {
  TestTarget Target = TestTarget::Gfx942XnackMinus;
  uint32_t Wave = 64;
  uint32_t Width = 32;
  std::array<TestOpcode, 2> Operations{TestOpcode::XorU32,
                                    TestOpcode::AddU32};
  std::array<uint32_t, 3> Inputs{34, 35, 36};
  uint32_t Scratch = 32;
  uint32_t Output = 33;
  bool ScratchClobber = true;
  bool EarlyClobber = true;
  bool SideEffect = true;
  bool WritesImplicitState = false;
  bool TouchesMemory = false;
};

bool acceptsClosedTestContract(const ClosedTestContract &Value) {
  return Value.Target == TestTarget::Gfx942XnackMinus && Value.Wave == 64 &&
         Value.Width == 32 &&
         Value.Operations == std::array{TestOpcode::XorU32, TestOpcode::AddU32} &&
         Value.Inputs == std::array<uint32_t, 3>{34, 35, 36} &&
         Value.Scratch == 32 && Value.Output == 33 && Value.ScratchClobber &&
         Value.EarlyClobber && Value.SideEffect && !Value.WritesImplicitState &&
         !Value.TouchesMemory;
}

size_t rejectContractControls() {
  require(acceptsClosedTestContract({}), "default test contract was rejected");
  size_t Count = 0;
  auto Check = [&](ClosedTestContract Value) {
    require(!acceptsClosedTestContract(Value),
            "closed test guard accepted a changed contract");
    ++Count;
  };
  ClosedTestContract Changed;
  Changed.Target = TestTarget::Gfx950XnackMinus;
  Check(Changed);
  Changed = {};
  Changed.Wave = 32;
  Check(Changed);
  Changed = {};
  Changed.Width = 64;
  Check(Changed);
  Changed = {};
  std::swap(Changed.Operations[0], Changed.Operations[1]);
  Check(Changed);
  Changed = {};
  Changed.Operations[0] = TestOpcode::MemoryOperation;
  Check(Changed);
  Changed = {};
  Changed.Inputs[2] = Changed.Scratch;
  Check(Changed);
  Changed = {};
  Changed.Output = Changed.Inputs[0];
  Check(Changed);
  Changed = {};
  Changed.ScratchClobber = false;
  Check(Changed);
  Changed = {};
  Changed.EarlyClobber = false;
  Check(Changed);
  Changed = {};
  Changed.SideEffect = false;
  Check(Changed);
  Changed = {};
  Changed.WritesImplicitState = true;
  Check(Changed);
  Changed = {};
  Changed.TouchesMemory = true;
  Check(Changed);
  return Count;
}

std::unique_ptr<TargetMachine> createMachine() {
  static const bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUAsmPrinter();
    LLVMInitializeAMDGPUAsmParser();
    LLVMInitializeAMDGPUDisassembler();
    return true;
  }();
  (void)Initialized;
  Triple TripleValue(TripleName);
  std::string Diagnostic;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, Diagnostic);
  require(TargetValue != nullptr, Diagnostic);
  TargetOptions Options;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, "gfx942", "-xnack,-wavefrontsize32,+wavefrontsize64", Options,
      Reloc::PIC_, CodeModel::Small, CodeGenOptLevel::None));
  require(Machine != nullptr, "cannot create pinned gfx942 machine");
  return Machine;
}

enum class Fixture { UsedResult, UnusedResult, WideEncodingControl, ModuleOnly };

// All assembly strings are closed native-test constants. There is no arbitrary
// assembly argument, serialized region, caller-supplied graph or public builder.
Input makeFixture(Fixture Kind) {
  require(acceptsClosedTestContract({}), "closed test guard was bypassed");
  LLVMContext Context;
  Module ModuleValue("unauthenticated-ordered-region-test", Context);
  auto Machine = createMachine();
  ModuleValue.setTargetTriple(Triple(TripleName));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);

  if (Kind == Fixture::ModuleOnly) {
    // Deliberately no LLVM Function/launch contract. The existing worker must
    // reject this public assembly label as a compiler-module export.
    ModuleValue.setModuleInlineAsm(
        ".text\n.p2align 8\n.globl ordered_region_fixture\n"
        ".type ordered_region_fixture,@function\nordered_region_fixture:\n"
        "s_endpgm\n.size ordered_region_fixture,.-ordered_region_fixture\n");
  } else {
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
    Metadata *Workgroup[] = {
        ConstantAsMetadata::get(ConstantInt::get(I32, 64)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
    Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, Workgroup));
    BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
    IRBuilder<> Builder(Entry);
    Value *FirstInput =
        Builder.CreateAdd(Kernel->getArg(1), ConstantInt::get(I32, 17));
    FunctionType *UnitType = FunctionType::get(I32, {I32, I32, I32}, false);
    const char *Unit =
        "v_xor_b32_e32 v32, $1, $2\n\tv_add_u32_e32 $0, v32, $3";
    if (Kind == Fixture::WideEncodingControl)
      Unit = "v_xor_b32_e64 v32, $1, $2\n\tv_add_u32_e32 $0, v32, $3";
    InlineAsm *Assembly = InlineAsm::get(
        UnitType, Unit, "=&{v33},{v34},{v35},{v36},~{v32}", true);
    CallInst *Value = Builder.CreateCall(
        UnitType, Assembly, {FirstInput, Kernel->getArg(2), Kernel->getArg(3)});
    // These compiler-owned surrounding operations are explicitly outside the
    // unit. No claim is made that sideeffect is a general scheduling barrier.
    if (Kind != Fixture::UnusedResult)
      Builder.CreateStore(Value, Kernel->getArg(0))->setVolatile(true);
    llvm::Value *SecondAddress = Builder.CreateGEP(
        I32, Kernel->getArg(0), ConstantInt::get(I32, 1));
    Builder.CreateStore(Kernel->getArg(3), SecondAddress)->setVolatile(true);
    Builder.CreateRetVoid();
  }

  std::string Verification;
  raw_string_ostream VerificationStream(Verification);
  if (verifyModule(ModuleValue, &VerificationStream))
    fail(Verification);
  std::string Text;
  raw_string_ostream Stream(Text);
  ModuleValue.print(Stream, nullptr);
  Stream.flush();
  require(!Text.empty() && Text.size() <= FixtureByteLimit,
          "native LLVM test fixture exceeds its byte cap");
  Input Result;
  Result.Kind = InputKind::LlvmTextIr;
  Result.Bytes.assign(Text.begin(), Text.end());
  Result.Digest = SHA256::hash(Result.Bytes);
  return Result;
}

Request makeRequest(Fixture Kind, OptimizationLevel Level) {
  Request Value;
  Value.RequestId.fill(0x41);
  Value.Identity.fill(0x42);
  Value.Protocol = ProtocolVersion::V2;
  Value.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  Value.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
  // Exactly like the existing native pipeline tests, these are visibly
  // synthetic request fields. They are NOT measured executable/owner custody.
  Value.WorkerExecutableDigest.fill(0x51);
  Value.WorkerExecutableBytes = 4096;
  Value.CompilerEnvelopeIdentity.fill(0x62);
  Value.Target = "gfx942:xnack-";
  Value.CodeObjectVersion = 6;
  Value.LinkOptions = {Level, false, true};
  Value.CompilerModule = makeFixture(Kind);
  Value.Inputs = {Value.CompilerModule};
  Value.RequiredSymbols = {KernelName.str(), KernelName.str() + ".kd"};
  Value.ExpectedDefinedSymbols = Value.RequiredSymbols;
  Value.ExportSymbols = {KernelName.str()};
  Value.FinalSymbols = Value.RequiredSymbols;
  Value.MaxOutputBytes = PayloadByteLimit;
  return Value;
}

PhysicalMachineEffectEvidence analyzePayload(ArrayRef<uint8_t> Bytes) {
  require(!Bytes.empty() && Bytes.size() <= PayloadByteLimit,
          "payload exceeds prototype cap");
  const auto Identities = physicalMachineEffectIdentities();
  PhysicalMachineEffectRequest RequestValue;
  RequestValue.AnalyzerIdentity = Identities.Analyzer;
  RequestValue.ToolchainIdentity = Identities.Toolchain;
  RequestValue.ExecutionChallenge.fill(0x70);
  RequestValue.RequestIdentity.fill(0x71); // Native-test marker, not wire custody.
  RequestValue.RequestBytes = 4096;
  RequestValue.Payload.assign(Bytes.begin(), Bytes.end());
  RequestValue.PayloadDigest = SHA256::hash(Bytes);
  RequestValue.PayloadBytes = Bytes.size();
  RequestValue.Entries = {{KernelName.str(), {16, 8, 4, 2, 0}}};
  auto Result = unwrap(analyzeGfx942PhysicalMachineEffects(RequestValue));
  require(Result.PayloadDigest == RequestValue.PayloadDigest &&
              Result.PayloadBytes == Bytes.size() &&
              Result.ExecutionChallenge == RequestValue.ExecutionChallenge &&
              Result.RequestIdentity == RequestValue.RequestIdentity &&
              Result.RequestBytes == RequestValue.RequestBytes &&
              Result.AnalyzerIdentity == RequestValue.AnalyzerIdentity &&
              Result.ToolchainIdentity == RequestValue.ToolchainIdentity &&
              Result.Entries.size() == 1 && Result.Functions.size() == 1 &&
              Result.Entries[0].Symbol == KernelName &&
              Result.Functions[0].Symbol == KernelName &&
              !Result.Instructions.empty() &&
              Result.Instructions.size() <= InstructionLimit,
          "decoded fixture identity, entry or instruction bounds changed");
  return Result;
}

struct BuiltFixture {
  Request RequestValue;
  Response ResponseValue;
  PhysicalMachineEffectEvidence Evidence;
  std::vector<std::string> InspectionDiagnostics;
};

void checkDiagnosticBounds(ArrayRef<std::string> Diagnostics) {
  require(Diagnostics.size() <= MaxDiagnostics,
          "inspection diagnostic count exceeds existing worker cap");
  size_t Total = 0;
  for (const auto &Diagnostic : Diagnostics) {
    require(Diagnostic.size() <= MaxDiagnosticBytes &&
                Diagnostic.size() <= MaxTotalDiagnosticBytes - Total,
            "inspection diagnostics exceed existing worker byte caps");
    Total += Diagnostic.size();
  }
}

void checkInspectionDiagnostics(ArrayRef<std::string> Diagnostics) {
  checkDiagnosticBounds(Diagnostics);
  // inspectLinkedOutputForPublication already validated the real ELF symbol
  // closure and compiler launch contract before returning these diagnostics.
  // These are reporting-contract assertions, not an alternative symbol parser
  // or authority derived from diagnostic strings.
  size_t Targets = 0, Exports = 0, Unresolved = 0, Metadata = 0, Kernels = 0;
  for (const auto &Diagnostic : Diagnostics) {
    StringRef Text(Diagnostic);
    Targets += Text.starts_with(
        "post_link.check=target status=ok arch=gfx942 code_object_version=6 "
        "e_flags=");
    Exports += Text ==
        "post_link.check=exports status=ok "
        "symbols=[ordered_region_fixture,ordered_region_fixture.kd]";
    Unresolved += Text == "post_link.check=unresolved status=ok symbols=[]";
    Metadata += Text ==
        "post_link.check=metadata status=ok kernels=1 "
        "target=amdgcn-amd-amdhsa--gfx942%3Axnack-";
    Kernels += Text.starts_with(
                   "post_link.kernel name=ordered_region_fixture "
                   "symbol=ordered_region_fixture.kd ") &&
               Text.ends_with(" wavefront_size=64 max_workgroup_size=64 "
                              "reqd_workgroup_size=[64,1,1]");
  }
  require(Diagnostics.size() == 5 && Targets == 1 && Exports == 1 &&
              Unresolved == 1 && Metadata == 1 && Kernels == 1,
          "bounded post-link diagnostic facts changed");
}

BuiltFixture buildFixture(Fixture Kind, OptimizationLevel Level) {
  Request RequestValue = makeRequest(Kind, Level);
  Response ResponseValue = execute(RequestValue);
  if (!ResponseValue.LinkedOutput) {
    size_t Remaining = MaxTotalDiagnosticBytes;
    for (const std::string &Diagnostic : ResponseValue.Diagnostics) {
      StringRef Part = StringRef(Diagnostic).take_front(Remaining);
      errs() << Part << '\n';
      Remaining -= Part.size();
      if (Remaining == 0)
        break;
    }
    fail("closed native LLVM fixture did not complete the worker pipeline");
  }
  const Output &OutputValue = *ResponseValue.LinkedOutput;
  require(ResponseValue.FailureStage == Stage::Complete &&
              ResponseValue.Protocol == ProtocolVersion::V2 &&
              ResponseValue.RequestId == RequestValue.RequestId &&
              ResponseValue.RequestIdentity == RequestValue.Identity &&
              ResponseValue.CompilerEnvelopeIdentity ==
                  RequestValue.CompilerEnvelopeIdentity &&
              ResponseValue.WorkerBuildIdentity == FE2O3_WORKER_BUILD_ID &&
              !ResponseValue.DeviceLibraryProvider &&
              !OutputValue.Bytes.empty() &&
              OutputValue.Bytes.size() <= PayloadByteLimit &&
              OutputValue.Digest == SHA256::hash(OutputValue.Bytes) &&
              ResponseValue.Derivation.has_value(),
          "worker success lacks exact bounded output/custody fields");
  const DerivationEvidence &Derivation = *ResponseValue.Derivation;
  require(Derivation.Hsaco.Digest == OutputValue.Digest &&
              Derivation.Hsaco.ByteLength == OutputValue.Bytes.size() &&
              Derivation.EvidenceIdentity ==
                  unwrap(calculateDerivationEvidenceIdentity(Derivation)) &&
              Derivation.NativeLinkInputs.size() == 1 &&
              Derivation.NativeLinkInputs[0].Source ==
                  NativeLinkInputSource::GeneratedObject &&
              Derivation.NativeLinkInputs[0].Content ==
                  Derivation.GeneratedObject,
          "normal LLVM/object/LLD derivation does not end at these bytes");
  auto InspectionDiagnostics =
      unwrap(inspectLinkedOutputForPublication(OutputValue.Bytes, RequestValue));
  checkInspectionDiagnostics(InspectionDiagnostics);
  auto Evidence = analyzePayload(OutputValue.Bytes);
  return {std::move(RequestValue), std::move(ResponseValue), std::move(Evidence),
          std::move(InspectionDiagnostics)};
}

struct ExpectedInstruction {
  const char *Opcode;
  std::array<const char *, 3> Registers;
  uint32_t Word;
};

// GFX9 VOP2: opcode[30:25], vdst[24:17], vsrc1[16:9], src0[8:0].
// src0 vector registers are encoded as 256 + VGPR index. These two literal
// words are intentionally not obtained from InlineAsm's returned output.
constexpr std::array<ExpectedInstruction, 2> ExpectedRegion{{
    {"V_XOR_B32_e32_vi", {"VGPR32", "VGPR34", "VGPR35"}, 0x2a404722},
    {"V_ADD_U32_e32_gfx9", {"VGPR33", "VGPR32", "VGPR36"}, 0x68424920},
}};

bool matchesInstruction(const PhysicalMachineInstructionTrace &Actual,
                        const ExpectedInstruction &Expected,
                        ArrayRef<uint8_t> Payload) {
  if (Actual.FunctionSymbol != KernelName || Actual.Opcode != Expected.Opcode ||
      Actual.Encoding.size() != 4 || Actual.ExplicitDefinitionCount != 1 ||
      Actual.Operands.size() != 3 || !Actual.ImplicitDefinitions.empty() ||
      Actual.ImplicitUses != std::vector<std::string>{"EXEC"} ||
      Actual.MemoryAccess != PhysicalMachineMemoryAccess::None ||
      Actual.MemoryWidth != 0 ||
      (Actual.Flags & static_cast<uint16_t>(~AllowedUnitFlags)) != 0 ||
      Actual.BranchKind != PhysicalMachineBranchKind::None ||
      Actual.BranchTarget != 0 ||
      Actual.InstructionOffset > Payload.size() ||
      Payload.size() - Actual.InstructionOffset < 4)
    return false;
  for (size_t Index = 0; Index < Expected.Registers.size(); ++Index) {
    const auto &Operand = Actual.Operands[Index];
    if (Operand.Kind != PhysicalMachineOperandKind::Register ||
        Operand.Register != Expected.Registers[Index] || Operand.TiedTo != -1)
      return false;
  }
  ArrayRef<uint8_t> Raw = Payload.slice(Actual.InstructionOffset, 4);
  return std::equal(Raw.begin(), Raw.end(), Actual.Encoding.begin()) &&
         support::endian::read32le(Raw.data()) == Expected.Word;
}

std::optional<size_t> locateExactRegion(
    const PhysicalMachineEffectEvidence &Evidence, ArrayRef<uint8_t> Payload) {
  if (Payload.empty() || Payload.size() > PayloadByteLimit ||
      Evidence.Instructions.size() < 2 ||
      Evidence.Instructions.size() > InstructionLimit ||
      Evidence.PayloadDigest != SHA256::hash(Payload) ||
      Evidence.PayloadBytes != Payload.size())
    return std::nullopt;
  std::optional<size_t> Found;
  for (size_t Index = 0; Index + 1 < Evidence.Instructions.size(); ++Index) {
    const auto &First = Evidence.Instructions[Index];
    const auto &Second = Evidence.Instructions[Index + 1];
    if (!matchesInstruction(First, ExpectedRegion[0], Payload) ||
        !matchesInstruction(Second, ExpectedRegion[1], Payload) ||
        First.InstructionOffset + 4 != Second.InstructionOffset ||
        First.BlockOrdinal != Second.BlockOrdinal)
      continue;
    if (Found)
      return std::nullopt; // No ambiguous first-match selection.
    Found = Index;
  }
  return Found;
}

bool isFootprintRegister(StringRef Name) {
  return Name == "VGPR32" || Name == "VGPR33" || Name == "VGPR34" ||
         Name == "VGPR35" || Name == "VGPR36";
}

json::Object instructionJson(const PhysicalMachineInstructionTrace &Site) {
  json::Array Operands;
  for (const auto &Operand : Site.Operands)
    if (Operand.Kind == PhysicalMachineOperandKind::Register)
      Operands.emplace_back(Operand.Register);
  json::Array Uses;
  for (const auto &Use : Site.ImplicitUses)
    Uses.emplace_back(Use);
  json::Array Definitions;
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

json::Object describePositive(const BuiltFixture &Built, OptimizationLevel Level,
                              bool UsedResult) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  auto Position = locateExactRegion(Built.Evidence, Payload);
  require(Position.has_value(),
          "independent opcode/order/e32/register/EXEC/byte check failed");
  json::Array Region;
  Region.emplace_back(instructionJson(Built.Evidence.Instructions[*Position]));
  Region.emplace_back(instructionJson(Built.Evidence.Instructions[*Position + 1]));
  json::Array Boundary;
  size_t BoundarySites = 0;
  size_t BoundaryMoves = 0;
  std::array<bool, 3> HasInputDefinition{false, false, false};
  bool HasOutputUse = false;
  for (size_t Index = 0; Index < Built.Evidence.Instructions.size(); ++Index) {
    if (Index == *Position || Index == *Position + 1)
      continue;
    const auto &Site = Built.Evidence.Instructions[Index];
    bool TouchesFootprint = false;
    for (size_t OperandIndex = 0; OperandIndex < Site.Operands.size();
         ++OperandIndex) {
      const auto &Operand = Site.Operands[OperandIndex];
      if (Operand.Kind != PhysicalMachineOperandKind::Register)
        continue;
      TouchesFootprint |= isFootprintRegister(Operand.Register);
      if (Index < *Position && OperandIndex < Site.ExplicitDefinitionCount) {
        for (size_t InputIndex = 0; InputIndex < 3; ++InputIndex)
          HasInputDefinition[InputIndex] |=
              Operand.Register == "VGPR" + std::to_string(34 + InputIndex);
      }
      if (Index > *Position + 1 &&
          OperandIndex >= Site.ExplicitDefinitionCount &&
          Operand.Register == "VGPR33")
        HasOutputUse = true;
    }
    if (TouchesFootprint) {
      ++BoundarySites;
      if (StringRef(Site.Opcode).starts_with("V_MOV_B32_"))
        ++BoundaryMoves;
      if (Boundary.size() < 32)
        Boundary.emplace_back(instructionJson(Site));
    }
  }
  require(std::all_of(HasInputDefinition.begin(), HasInputDefinition.end(),
                      [](bool Value) { return Value; }),
          "fixed-register input materialization was not observed before unit");
  require(!UsedResult || HasOutputUse,
          "fixed-register output has no observed use after unit");
  json::Array InspectionDiagnostics;
  for (const auto &Diagnostic : Built.InspectionDiagnostics)
    InspectionDiagnostics.emplace_back(Diagnostic);
  return json::Object{{"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
          {"result_used", UsedResult},
          {"llvm_text_sha256", hex(Built.RequestValue.CompilerModule.Digest)},
          {"llvm_text_bytes", Built.RequestValue.CompilerModule.Bytes.size()},
          {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)},
          {"hsaco_bytes", Payload.size()},
          {"descriptor_sha256", hex(Built.Evidence.Entries[0].DescriptorIdentity)},
          {"entry_file_offset", Built.Evidence.Entries[0].CodeOffset},
          {"entry_code_bytes", Built.Evidence.Entries[0].CodeSize},
          {"static_instruction_count", Built.Evidence.Instructions.size()},
          {"post_link_inspection_diagnostics", std::move(InspectionDiagnostics)},
          {"region", std::move(Region)},
          {"boundary_register_site_count", BoundarySites},
          {"boundary_v_mov_b32_count", BoundaryMoves},
          {"boundary_sites", std::move(Boundary)},
          {"boundary_sites_truncated", BoundarySites > 32}};
}

size_t rejectDecodedControls(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const size_t Position = *locateExactRegion(Built.Evidence, Payload);
  size_t Count = 0;
  auto Check = [&](PhysicalMachineEffectEvidence Changed) {
    require(!locateExactRegion(Changed, Payload),
            "independent matcher accepted a corrupted decoded observation");
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
  Changed.Instructions[Position].ImplicitDefinitions = {"EXEC"};
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].ImplicitUses.clear();
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions[Position].Flags |= 1; // Contradictory MayLoad bit.
  Check(std::move(Changed));
  Changed = Built.Evidence;
  ++Changed.Instructions[Position + 1].BlockOrdinal;
  Check(std::move(Changed));
  Changed = Built.Evidence;
  Changed.Instructions.push_back(Built.Evidence.Instructions[Position]);
  Changed.Instructions.push_back(Built.Evidence.Instructions[Position + 1]);
  Check(std::move(Changed));
  return Count;
}

size_t rejectReencodedPayloadControls(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  const size_t Position = *locateExactRegion(Built.Evidence, Payload);
  const uint64_t Offset = Built.Evidence.Instructions[Position].InstructionOffset;
  require(Offset <= Payload.size() && Payload.size() - Offset >= 8,
          "mutation unit is not in bounded payload");
  std::vector<uint8_t> Changed = Payload;
  // v34 -> v35 in src0: a well-formed instruction, not a decoder-corruption test.
  Changed[Offset] ^= 1;
  auto Evidence = analyzePayload(Changed);
  require(!locateExactRegion(Evidence, Changed),
          "changed physical source register matched original independent unit");
  Changed = Payload;
  std::swap_ranges(Changed.begin() + Offset, Changed.begin() + Offset + 4,
                   Changed.begin() + Offset + 4);
  Evidence = analyzePayload(Changed);
  require(!locateExactRegion(Evidence, Changed),
          "reversed instruction bytes matched original ordered unit");
  return 2;
}

void rejectActualWideEncoding(const BuiltFixture &Wide) {
  const auto &Payload = Wide.ResponseValue.LinkedOutput->Bytes;
  require(!locateExactRegion(Wide.Evidence, Payload),
          "real e64 emission matched the required e32 unit");
  size_t WidePairs = 0;
  for (size_t Index = 0; Index + 1 < Wide.Evidence.Instructions.size(); ++Index) {
    const auto &First = Wide.Evidence.Instructions[Index];
    const auto &Second = Wide.Evidence.Instructions[Index + 1];
    if (First.Opcode != "V_XOR_B32_e64_vi" || First.Encoding.size() != 8 ||
        First.FunctionSymbol != KernelName ||
        First.InstructionOffset > Payload.size() ||
        Payload.size() - First.InstructionOffset < 8 ||
        First.InstructionOffset + 8 != Second.InstructionOffset ||
        First.BlockOrdinal != Second.BlockOrdinal ||
        !First.ImplicitDefinitions.empty() ||
        First.ImplicitUses != std::vector<std::string>{"EXEC"} ||
        First.MemoryAccess != PhysicalMachineMemoryAccess::None ||
        First.MemoryWidth != 0 ||
        (First.Flags & static_cast<uint16_t>(~AllowedUnitFlags)) != 0 ||
        First.BranchKind != PhysicalMachineBranchKind::None ||
        First.BranchTarget != 0 ||
        !matchesInstruction(Second, ExpectedRegion[1], Payload))
      continue;
    std::vector<std::string> Registers;
    for (const auto &Operand : First.Operands)
      if (Operand.Kind == PhysicalMachineOperandKind::Register)
        Registers.push_back(Operand.Register);
    auto Raw = ArrayRef<uint8_t>(Payload).slice(First.InstructionOffset, 8);
    if (Registers == std::vector<std::string>{"VGPR32", "VGPR34", "VGPR35"} &&
        std::equal(Raw.begin(), Raw.end(), First.Encoding.begin()))
      ++WidePairs;
  }
  require(WidePairs == 1,
          "negative encoding control did not actually emit one e64/e32 pair");
}

void rejectModuleAssemblyOnly() {
  auto RequestValue = makeRequest(Fixture::ModuleOnly, OptimizationLevel::O0);
  auto Result = execute(RequestValue);
  checkDiagnosticBounds(Result.Diagnostics);
  require(!Result.LinkedOutput && !Result.Derivation &&
              Result.FailureStage == Stage::InputValidation,
          "module-only body unexpectedly acquired compiler export authority");
  require(std::any_of(Result.Diagnostics.begin(), Result.Diagnostics.end(),
                      [](const std::string &Diagnostic) {
                        return StringRef(Diagnostic).contains(
                            "compiler-module export is not defined by the "
                            "compiler module");
                      }),
          "module-only rejection did not exercise the existing export gate");
}

} // namespace

int main(int Argc, char **) {
  require(Argc == 1, "this test accepts no paths, assembly or profile options");
  // In-process LLVM/LLD only; no hardware or worker children are launched.
  // CTest should additionally supply TIMEOUT=120. A future receipt runner must
  // enforce its own process-group timeout and stdout/stderr byte caps.
  alarm(90);
  const size_t ContractNegatives = rejectContractControls();
  json::Array PositiveCases;
  std::optional<BuiltFixture> MutationBaseline;
  for (OptimizationLevel Level : {OptimizationLevel::O0, OptimizationLevel::O3}) {
    for (bool Used : {true, false}) {
      auto Built = buildFixture(Used ? Fixture::UsedResult : Fixture::UnusedResult,
                                Level);
      PositiveCases.emplace_back(describePositive(Built, Level, Used));
      if (Level == OptimizationLevel::O3 && Used)
        MutationBaseline = std::move(Built);
    }
  }
  require(MutationBaseline.has_value(), "missing real emitted mutation baseline");
  const size_t DecodedNegatives = rejectDecodedControls(*MutationBaseline);
  const size_t PayloadNegatives = rejectReencodedPayloadControls(*MutationBaseline);
  auto Wide = buildFixture(Fixture::WideEncodingControl, OptimizationLevel::O3);
  rejectActualWideEncoding(Wide);
  rejectModuleAssemblyOnly();

  json::Object Report{
      {"schema", "fe2o3-ordered-inline-unit-worker-prototype-v1"},
      {"authority", "unauthenticated-native-test-fixture"},
      {"source_produced", false},
      {"production_exact_region_admission", false},
      {"protected_finalizer_admission", false},
      {"hardware_executed", false},
      {"target", "gfx942:xnack-"},
      {"wave_width", 64},
      {"workgroup_size", 64},
      {"code_object_version", 6},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID},
      {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"whole_kernel_order_or_byte_stability_claim", false},
      {"physical_register_allocation_or_lifetime_proof", false},
      {"implicit_exec_reads", "required; not clobbered"},
      {"boundary_operations", "compiler-owned; reported outside the unit"},
      {"positive_cases", std::move(PositiveCases)},
      {"closed_contract_negatives", ContractNegatives},
      {"decoded_observation_negatives", DecodedNegatives},
      {"actual_payload_mutation_negatives", PayloadNegatives},
      {"actual_e64_encoding_rejected_by_matcher", true},
      {"module_only_export_rejected_by_worker", true}};
  std::string Text;
  raw_string_ostream Stream(Text);
  Stream << formatv("{0:2}", json::Value(std::move(Report)));
  Stream.flush();
  require(Text.size() <= ReportByteLimit, "bounded prototype report exceeded cap");
  outs() << Text << '\n';
  alarm(0);
  return 0;
}
