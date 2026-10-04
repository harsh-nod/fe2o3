// Test-only transport experiment; no production/source authority.
#include "WorkerMachineEffect.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/AMDHSAKernelDescriptor.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/FormatVariadic.h"
#include "llvm/Support/JSON.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdlib>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>
using namespace llvm;
using namespace fe2o3::worker;
namespace {
constexpr StringLiteral TripleName = "amdgcn-amd-amdhsa";
constexpr StringLiteral KernelName = "whole_body_transport_fixture";
constexpr size_t PayloadByteLimit = 1024 * 1024;
constexpr size_t InstructionLimit = 64;
constexpr size_t FixtureByteLimit = 16 * 1024;
constexpr size_t ReportByteLimit = 32 * 1024;
[[noreturn]] void fail(StringRef Message) {
  errs() << "whole-body transport candidate failed: "
         << Message.take_front(4096) << '\n';
  std::exit(1);
}
void require(bool Value, StringRef Message) { if (!Value) fail(Message); }
template <typename T> T unwrap(Expected<T> Value) {
  if (!Value) fail(toString(Value.takeError()));
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
// Existing synthetic request markers, full V2 execute path, derivation checks,
// post-link inspection and native decoder: not an alternate worker route.
#include "OrderedProgramWorkerSupport.inc"

// Fixed GFX9 words, independently specified before executing the candidate.
// VOP1 immediate selectors 129/130/131 encode 1/2/3. S_ENDPGM is authored.
constexpr std::array<uint32_t, 7> Words{{
    0x7e440281, 0x7e460282, 0x7e480283,
    0x2a404722, 0x26404920, 0x2a424123, 0xbf810000}};
constexpr StringLiteral Body =
    "v_mov_b32_e32 v34, 1\n\t"
    "v_mov_b32_e32 v35, 2\n\t"
    "v_mov_b32_e32 v36, 3\n\t"
    "v_xor_b32_e32 v32, v34, v35\n\t"
    "v_and_b32_e32 v32, v32, v36\n\t"
    "v_xor_b32_e32 v33, v35, v32\n\t"
    "s_endpgm";
constexpr StringLiteral Clobbers = "~{v32},~{v33},~{v34},~{v35},~{v36}";

Input fixture(bool Naked, bool MissingLaunch = false, bool WrongCpu = false) {
  LLVMContext Context;
  Module ModuleValue("synthetic-whole-body-transport", Context);
  auto Machine = createMachine();
  ModuleValue.setTargetTriple(Triple(TripleName));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);
  auto *TypeValue = FunctionType::get(Type::getVoidTy(Context), false);
  auto *Kernel = Function::Create(TypeValue, GlobalValue::ExternalLinkage,
                                  KernelName, ModuleValue);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr(Attribute::NoUnwind);
  if (Naked) Kernel->addFnAttr(Attribute::Naked);
  Kernel->addFnAttr("target-cpu", WrongCpu ? "gfx950" : "gfx942");
  Kernel->addFnAttr("target-features",
                    "-xnack,-wavefrontsize32,+wavefrontsize64");
  Kernel->addFnAttr("amdgpu-flat-work-group-size", "64,64");
  // No implicit-argument override or assertion of absent ABI preloads.
  // LLVM owns hidden arguments, descriptor emission and exact exports.
  if (!MissingLaunch) {
    auto *I32 = Type::getInt32Ty(Context);
    Metadata *Workgroup[] = {
        ConstantAsMetadata::get(ConstantInt::get(I32, 64)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
    Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, Workgroup));
  }
  IRBuilder<> Builder(BasicBlock::Create(Context, "entry", Kernel));
  auto *Asm = InlineAsm::get(TypeValue, Body, Clobbers, true);
  Builder.CreateCall(TypeValue, Asm, {});
  Builder.CreateUnreachable();
  std::string Diagnostic;
  raw_string_ostream Errors(Diagnostic);
  require(!verifyModule(ModuleValue, &Errors), "fixture LLVM verification");
  std::string Text;
  raw_string_ostream Out(Text);
  ModuleValue.print(Out, nullptr);
  Out.flush();
  require(!Text.empty() && Text.size() <= FixtureByteLimit, "fixture byte cap");
  Input Value;
  Value.Kind = InputKind::LlvmTextIr;
  Value.Bytes.assign(Text.begin(), Text.end());
  Value.Digest = SHA256::hash(Value.Bytes);
  return Value;
}

void negativeContracts(bool Naked) {
  for (bool Missing : {true, false}) {
    auto RequestValue = makeInputRequest(
        fixture(Naked, Missing, !Missing), OptimizationLevel::O0);
    auto Result = execute(RequestValue);
    require(!Result.LinkedOutput && !Result.Derivation &&
                Result.FailureStage == Stage::InputValidation,
            "missing launch/wrong CPU was not refused before publication");
    checkDiagnosticBounds(Result.Diagnostics);
    const StringRef Needle = Missing
        ? "has no exact required-workgroup metadata"
        : "target CPU does not match request";
    require(llvm::any_of(Result.Diagnostics, [Needle](const std::string &Text) {
              return StringRef(Text).contains(Needle);
            }), "negative refused for an unexpected reason");
  }
}

json::Object descriptor(const BuiltFixture &Built) {
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  StringRef Data(reinterpret_cast<const char *>(Payload.data()), Payload.size());
  auto Object = unwrap(object::ObjectFile::createObjectFile(
      MemoryBufferRef(Data, "<whole-body-final-payload>")));
  std::optional<json::Object> Result;
  size_t Scanned = 0;
  for (const auto &Symbol : Object->symbols()) {
    require(++Scanned <= 4096, "descriptor symbol scan cap");
    if (unwrap(Symbol.getName()) != KernelName.str() + ".kd") continue;
    require(!Result, "duplicate descriptor");
    auto Section = unwrap(Symbol.getSection());
    require(Section != Object->section_end(), "descriptor section missing");
    auto Contents = unwrap(Section->getContents());
    const uint64_t Address = unwrap(Symbol.getAddress());
    require(Address >= Section->getAddress(), "descriptor before section");
    const uint64_t Relative = Address - Section->getAddress();
    require(Relative <= Contents.size() && Contents.size() - Relative >= 64,
            "descriptor extent");
    const auto *Bytes =
        reinterpret_cast<const uint8_t *>(Contents.data()) + Relative;
    const uintptr_t Base = reinterpret_cast<uintptr_t>(Payload.data());
    const uintptr_t Begin = reinterpret_cast<uintptr_t>(Bytes);
    require(Begin >= Base && Begin - Base <= Payload.size() &&
                Payload.size() - (Begin - Base) >= 64, "descriptor storage");
    const auto Digest = SHA256::hash(ArrayRef<uint8_t>(Bytes, 64));
    require(Digest == Built.Evidence.Entries[0].DescriptorIdentity,
            "descriptor differs from native analyzer");
    using namespace llvm::amdhsa;
    const uint32_t Group = support::endian::read32le(
        Bytes + GROUP_SEGMENT_FIXED_SIZE_OFFSET);
    const uint32_t Private = support::endian::read32le(
        Bytes + PRIVATE_SEGMENT_FIXED_SIZE_OFFSET);
    const uint32_t Kernarg = support::endian::read32le(Bytes + KERNARG_SIZE_OFFSET);
    const uint32_t Rsrc1 = support::endian::read32le(Bytes + COMPUTE_PGM_RSRC1_OFFSET);
    const uint32_t Rsrc2 = support::endian::read32le(Bytes + COMPUTE_PGM_RSRC2_OFFSET);
    const uint32_t Rsrc3 = support::endian::read32le(Bytes + COMPUTE_PGM_RSRC3_OFFSET);
    const uint16_t Properties = support::endian::read16le(
        Bytes + KERNEL_CODE_PROPERTIES_OFFSET);
    const uint16_t Preload = support::endian::read16le(Bytes + KERNARG_PRELOAD_OFFSET);
    const uint32_t Capacity = ((Rsrc1 & 63) + 1) * 8;
    const uint32_t Boundary = ((Rsrc3 & 63) + 1) * 4;
    errs() << "observed descriptor: kernarg=" << Kernarg
           << " group=" << Group << " private=" << Private
           << " capacity=" << Capacity << " boundary=" << Boundary << '\n';
    require(Group == 0 && Private == 0 && Capacity >= 37 &&
                Boundary >= 37 && Capacity >= Boundary &&
                !(Properties & KERNEL_CODE_PROPERTY_USES_DYNAMIC_STACK),
            "whole-body scratch/LDS/dynamic-stack/capacity contract");
    Result = json::Object{
        {"sha256", hex(Digest)}, {"file_offset", Begin - Base},
        {"bytes", 64}, {"kernarg_bytes", Kernarg}, {"group_bytes", Group},
        {"private_bytes", Private}, {"compute_pgm_rsrc1", Rsrc1},
        {"compute_pgm_rsrc2", Rsrc2}, {"compute_pgm_rsrc3", Rsrc3},
        {"kernel_code_properties", Properties}, {"kernarg_preload", Preload},
        {"vgpr_capacity", Capacity}, {"architected_vgpr_boundary", Boundary}};
  }
  require(Result.has_value(), "descriptor absent");
  return std::move(*Result);
}

json::Object inspect(const Input &Fixture, OptimizationLevel Level) {
  auto Built = buildRequest(makeInputRequest(Fixture, Level));
  const auto &Evidence = Built.Evidence;
  const auto &Entry = Evidence.Entries[0];
  const auto &Payload = Built.ResponseValue.LinkedOutput->Bytes;
  errs() << "observed entry: bytes=" << Entry.CodeSize
         << " instructions=" << Evidence.Instructions.size()
         << " blocks=" << Evidence.Blocks.size() << '\n';
  if (Entry.CodeOffset <= Payload.size() &&
      Entry.CodeSize <= Payload.size() - Entry.CodeOffset)
    errs() << "observed entry prefix: "
           << hex(ArrayRef<uint8_t>(Payload).slice(
                  Entry.CodeOffset, std::min<uint64_t>(Entry.CodeSize, 256)))
           << '\n';
  require(Entry.CodeSize == Words.size() * 4 &&
              Entry.CodeOffset <= Payload.size() &&
              Entry.CodeSize <= Payload.size() - Entry.CodeOffset &&
              Evidence.Instructions.size() == Words.size() &&
              Evidence.Blocks.size() == 1 &&
              Evidence.Functions[0].DirectCallees.empty(),
          "entry has added instructions/padding/CFG");
  for (size_t Index = 0; Index < Words.size(); ++Index) {
    const auto &Site = Evidence.Instructions[Index];
    const auto Offset = Entry.CodeOffset + Index * 4;
    require(Site.FunctionSymbol == KernelName && Site.InstructionOffset == Offset &&
                Site.Encoding.size() == 4 &&
                support::endian::read32le(Site.Encoding.data()) == Words[Index] &&
                support::endian::read32le(Payload.data() + Offset) == Words[Index],
            "complete body differs from independent literal encoding");
    require(Site.MemoryAccess == PhysicalMachineMemoryAccess::None &&
                Site.BranchKind == (Index + 1 == Words.size()
                    ? PhysicalMachineBranchKind::Return
                    : PhysicalMachineBranchKind::None),
            "unexpected memory/control-flow effect");
  }
  require(Evidence.Effects.size() == 1 &&
              Evidence.Effects[0].Kind == PhysicalMachineEffectKind::Return &&
              Evidence.Effects[0].InstructionOffset == Entry.CodeOffset + 24,
          "expected exactly authored termination");
  json::Array Diagnostics;
  for (const auto &Text : Built.InspectionDiagnostics) Diagnostics.emplace_back(Text);
  const auto &Derivation = *Built.ResponseValue.Derivation;
  return json::Object{
      {"optimization", Level == OptimizationLevel::O0 ? "O0" : "O3"},
      {"llvm_sha256", hex(Built.RequestValue.CompilerModule.Digest)},
      {"llvm_bytes", Built.RequestValue.CompilerModule.Bytes.size()},
      {"hsaco_sha256", hex(Built.ResponseValue.LinkedOutput->Digest)},
      {"hsaco_bytes", Payload.size()}, {"entry_offset", Entry.CodeOffset},
      {"entry_bytes", Entry.CodeSize}, {"instructions", Words.size()},
      {"body_hex", hex(ArrayRef<uint8_t>(Payload).slice(Entry.CodeOffset, Entry.CodeSize))},
      {"whole_entry_exact", true}, {"descriptor", descriptor(Built)},
      {"derivation_identity", hex(Derivation.EvidenceIdentity)},
      {"post_link_checks", std::move(Diagnostics)}};
}
} // namespace

int main(int Argc, char **Argv) {
  require(Argc == 2 && (StringRef(Argv[1]) == "naked" ||
                        StringRef(Argv[1]) == "ordinary"),
          "usage: whole-body-transport-candidate naked|ordinary");
  const bool Naked = StringRef(Argv[1]) == "naked";
  negativeContracts(Naked);
  const Input Fixture = fixture(Naked);
  json::Array Cases;
  Cases.emplace_back(inspect(Fixture, OptimizationLevel::O0));
  Cases.emplace_back(inspect(Fixture, OptimizationLevel::O3));
  json::Object Report{
      {"report_kind", "private-whole-body-transport-experiment"},
      {"authority", "unauthenticated-synthetic-worker-test"},
      {"shell", Naked ? "naked" : "ordinary"},
      {"llvm_build_claim", FE2O3_LLVM_BUILD_ID},
      {"worker_build_claim", FE2O3_WORKER_BUILD_ID},
      {"target", "gfx942:xnack-"}, {"code_object_version", 6},
      {"wave_width", 64}, {"workgroup_size", 64},
      {"explicit_arguments", 0}, {"negative_contract_cases", 2},
      {"llvm_text", std::string(Fixture.Bytes.begin(), Fixture.Bytes.end())},
      {"source_produced", false}, {"production_admission", false},
      {"protected_finalizer_admission", false}, {"artifact_authority", false},
      {"hardware_execution", false}, {"general_whole_body_support", false},
      {"runtime_closure_attestation", "unavailable"},
      {"cases", std::move(Cases)}};
  std::string Text = formatv("{0:2}\n", json::Value(std::move(Report))).str();
  require(Text.size() <= ReportByteLimit, "report cap");
  outs() << Text;
}
