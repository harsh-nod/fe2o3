#include "WorkerLldPolicy.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/Bitcode/BitcodeWriter.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/Linker/Linker.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Support/Endian.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/Path.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"

#include <algorithm>
#include <array>
#include <cerrno>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <tuple>
#include <utility>
#include <vector>

#include <sys/wait.h>
#include <unistd.h>

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif

using namespace fe2o3::worker;
using namespace llvm;
using namespace llvm::object;

namespace {

constexpr StringLiteral AmdGpuTriple = "amdgcn-amd-amdhsa";
constexpr StringLiteral ExactLdsGemmSlice1ProducerDataLayout =
    "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-"
    "p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-"
    "v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-"
    "v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";

enum class LayoutMode { Exact, Absent, Incompatible };

struct FixtureOptions {
  StringRef Cpu = "gfx942";
  uint8_t CodeObjectVersion = 5;
  LayoutMode Layout = LayoutMode::Exact;
  StringRef FunctionCpu;
  StringRef FunctionFeatures;
  bool WeakDefinition = false;
  bool WeakImport = false;
  bool NoInlineDefinition = false;
  uint32_t Addend = 1;
};

FixtureOptions withLayout(LayoutMode Layout) {
  FixtureOptions Result;
  Result.Layout = Layout;
  return Result;
}

FixtureOptions withCpu(StringRef Cpu) {
  FixtureOptions Result;
  Result.Cpu = Cpu;
  return Result;
}

FixtureOptions withCodeObjectVersion(uint8_t Version) {
  FixtureOptions Result;
  Result.CodeObjectVersion = Version;
  return Result;
}

FixtureOptions withNoInlineCodeObjectVersion(uint8_t Version) {
  FixtureOptions Result = withCodeObjectVersion(Version);
  Result.NoInlineDefinition = true;
  return Result;
}

FixtureOptions withFunctionContract(StringRef Cpu, StringRef Features) {
  FixtureOptions Result;
  Result.FunctionCpu = Cpu;
  Result.FunctionFeatures = Features;
  return Result;
}

FixtureOptions withFunctionFeatures(StringRef Features) {
  return withFunctionContract({}, Features);
}

FixtureOptions withFunctionCpu(StringRef Cpu) {
  return withFunctionContract(Cpu, {});
}

FixtureOptions withWeakImport() {
  FixtureOptions Result;
  Result.WeakImport = true;
  return Result;
}

FixtureOptions withAddend(uint32_t Addend) {
  FixtureOptions Result;
  Result.Addend = Addend;
  return Result;
}

FixtureOptions withWeakDefinition(uint32_t Addend) {
  FixtureOptions Result = withAddend(Addend);
  Result.WeakDefinition = true;
  return Result;
}

[[noreturn]] void fail(StringRef Message) {
  errs() << "pipeline test failed: " << Message << '\n';
  std::abort();
}

void require(bool Condition, StringRef Message) {
  if (!Condition)
    fail(Message);
}

std::unique_ptr<TargetMachine> createMachine(StringRef Cpu) {
  static bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUAsmPrinter();
    LLVMInitializeAMDGPUAsmParser();
    return true;
  }();
  (void)Initialized;

  Triple TripleValue(AmdGpuTriple);
  std::string LookupError;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, LookupError);
  require(TargetValue != nullptr, LookupError);
  TargetOptions OptionsValue;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, Cpu, "", OptionsValue, Reloc::PIC_, CodeModel::Small,
      CodeGenOptLevel::None));
  require(Machine != nullptr, "could not create fixture target machine");
  return Machine;
}

std::unique_ptr<Module> makeModule(LLVMContext &Context, StringRef ModuleName,
                                   StringRef Definition,
                                   std::optional<StringRef> Callee,
                                   const FixtureOptions &Options) {
  auto Result = std::make_unique<Module>(ModuleName, Context);
  std::unique_ptr<TargetMachine> Machine = createMachine(Options.Cpu);
  Result->setTargetTriple(Triple(AmdGpuTriple));
  if (Options.Layout == LayoutMode::Exact)
    Result->setDataLayout(Machine->createDataLayout());
  else if (Options.Layout == LayoutMode::Incompatible)
    Result->setDataLayout("e-p:32:32");
  Result->addModuleFlag(Module::Error, "amdhsa_code_object_version",
                        Options.CodeObjectVersion * 100);

  Type *I32 = Type::getInt32Ty(Context);
  FunctionType *Signature = FunctionType::get(I32, {I32}, false);
  Function *Defined = Function::Create(Signature, GlobalValue::ExternalLinkage,
                                       Definition, *Result);
  if (Options.WeakDefinition)
    Defined->setLinkage(GlobalValue::WeakAnyLinkage);
  if (Options.NoInlineDefinition)
    Defined->addFnAttr(Attribute::NoInline);
  if (!Options.FunctionCpu.empty())
    Defined->addFnAttr("target-cpu", Options.FunctionCpu);
  if (!Options.FunctionFeatures.empty())
    Defined->addFnAttr("target-features", Options.FunctionFeatures);
  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Defined);
  IRBuilder<> Builder(Entry);
  Value *Argument = Defined->getArg(0);
  Value *ReturnValue = nullptr;
  if (Callee) {
    FunctionCallee Imported = Result->getOrInsertFunction(*Callee, Signature);
    if (Options.WeakImport)
      cast<Function>(Imported.getCallee())
          ->setLinkage(GlobalValue::ExternalWeakLinkage);
    ReturnValue = Builder.CreateCall(Imported, {Argument});
  } else {
    ReturnValue =
        Builder.CreateAdd(Argument, ConstantInt::get(I32, Options.Addend));
  }
  Builder.CreateRet(ReturnValue);
  return Result;
}

std::vector<uint8_t> makeBitcode(StringRef ModuleName, StringRef Definition,
                                 std::optional<StringRef> Callee,
                                 const FixtureOptions &Options = {}) {
  LLVMContext Context;
  std::unique_ptr<Module> ModuleValue =
      makeModule(Context, ModuleName, Definition, Callee, Options);
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(*ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t>
makeFloatConsumerBitcode(StringRef Entry, ArrayRef<StringRef> Imports,
                         ArrayRef<StringRef> UnusedDeclarations,
                         uint8_t CodeObjectVersion = 5) {
  LLVMContext Context;
  Module ModuleValue("float-consumer", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version",
                            CodeObjectVersion * 100);
  Type *F32 = Type::getFloatTy(Context);
  FunctionType *Signature = FunctionType::get(F32, {F32}, false);
  Function *Defined = Function::Create(Signature, GlobalValue::ExternalLinkage,
                                       Entry, ModuleValue);
  BasicBlock *Block = BasicBlock::Create(Context, "entry", Defined);
  IRBuilder<> Builder(Block);
  Value *Result = Defined->getArg(0);
  for (StringRef Import : Imports) {
    FunctionCallee Imported =
        ModuleValue.getOrInsertFunction(Import, Signature);
    Result = Builder.CreateCall(Imported, {Result});
  }
  for (StringRef Declaration : UnusedDeclarations)
    ModuleValue.getOrInsertFunction(Declaration, Signature);
  Builder.CreateRet(Result);
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeFloatConsumerBitcode(StringRef Entry, StringRef Import,
                                              uint8_t CodeObjectVersion = 5) {
  return makeFloatConsumerBitcode(Entry, ArrayRef<StringRef>(Import), {},
                                  CodeObjectVersion);
}

std::vector<uint8_t>
makeOcmlKernelBitcode(StringRef Import = "__ocml_sin_f32",
                      uint8_t CodeObjectVersion = 5,
                      bool TwoCalls = false) {
  LLVMContext Context;
  Module ModuleValue("gfx942-ocml-sin-kernel", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version",
                            CodeObjectVersion * 100);

  Type *F32 = Type::getFloatTy(Context);
  PointerType *GlobalF32 = PointerType::get(Context, 1);
  Type *I32 = Type::getInt32Ty(Context);
  Type *I64 = Type::getInt64Ty(Context);
  FunctionType *KernelSignature = FunctionType::get(
      Type::getVoidTy(Context), {GlobalF32, GlobalF32, I64}, false);
  Function *Kernel =
      Function::Create(KernelSignature, GlobalValue::ExternalLinkage,
                       "fe2o3_gfx942_ocml_sin_f32_v1", ModuleValue);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr("target-cpu", "gfx942");
  Kernel->addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
  Kernel->addFnAttr("amdgpu-flat-work-group-size", "256,256");
  Metadata *Workgroup[] = {ConstantAsMetadata::get(ConstantInt::get(I32, 256)),
                           ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
                           ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
  Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, Workgroup));

  auto Argument = Kernel->arg_begin();
  Value *Input = &*Argument++;
  Input->setName("input");
  Value *Output = &*Argument++;
  Output->setName("output");
  Value *Length = &*Argument;
  Length->setName("length");

  FunctionCallee WorkitemId = ModuleValue.getOrInsertFunction(
      "llvm.amdgcn.workitem.id.x", FunctionType::get(I32, false));
  FunctionCallee Ocml = ModuleValue.getOrInsertFunction(
      Import, FunctionType::get(F32, {F32}, false));

  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
  BasicBlock *Active = BasicBlock::Create(Context, "active", Kernel);
  BasicBlock *Exit = BasicBlock::Create(Context, "exit", Kernel);
  IRBuilder<> Builder(Entry);
  Value *Lane = Builder.CreateCall(WorkitemId);
  Value *Index = Builder.CreateZExt(Lane, I64);
  Builder.CreateCondBr(Builder.CreateICmpULT(Index, Length), Active, Exit);

  Builder.SetInsertPoint(Active);
  Value *InputElement = Builder.CreateInBoundsGEP(F32, Input, Index);
  Value *OutputElement = Builder.CreateInBoundsGEP(F32, Output, Index);
  Value *InputValue = Builder.CreateLoad(F32, InputElement);
  Value *Result = Builder.CreateCall(Ocml, {InputValue});
  if (TwoCalls) {
    Value *Second = Builder.CreateCall(Ocml, {Result});
    Result = Builder.CreateFAdd(Result, Second);
  }
  Builder.CreateStore(Result, OutputElement);
  Builder.CreateBr(Exit);

  Builder.SetInsertPoint(Exit);
  Builder.CreateRetVoid();

  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeCrossDefinedCompilerBitcode() {
  LLVMContext Context;
  std::unique_ptr<Module> Entry =
      makeModule(Context, "cross-entry", "cross_entry", "cross_helper", {});
  std::unique_ptr<Module> Helper =
      makeModule(Context, "cross-helper", "cross_helper", std::nullopt, {});
  require(!Linker::linkModules(*Entry, std::move(Helper)),
          "could not form cross-defined compiler fixture");
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(*Entry, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeSymbolKindCompilerIr() {
  constexpr StringLiteral Ir = R"IR(
target triple = "amdgcn-amd-amdhsa"

@ordinary_global = external global i32
@address_slot = internal global ptr @ordinary_global
@llvm.used = appending global [1 x ptr] [ptr @used_only], section "llvm.metadata"
@llvm.compiler.used = appending global [1 x ptr] [ptr @weak_used], section "llvm.metadata"
@aliased = alias void (), ptr @aliased_impl
@selected = ifunc void (), ptr @resolve

declare void @unused_function()
declare void @used_only()
declare extern_weak void @weak_used()
declare extern_weak void @weak_call()
declare i32 @llvm.amdgcn.workitem.id.x()

define void @aliased_impl() {
entry:
  ret void
}

define ptr @resolve() {
entry:
  ret ptr @aliased_impl
}

define i32 @symbol_kinds_entry() {
entry:
  %global = load i32, ptr @ordinary_global
  %id = call i32 @llvm.amdgcn.workitem.id.x()
  call void @weak_call()
  %result = add i32 %global, %id
  ret i32 %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
)IR";
  return std::vector<uint8_t>(Ir.bytes_begin(), Ir.bytes_end());
}

std::vector<uint8_t> makeInlineAsmCompilerIr(StringRef Symbol,
                                             bool ModuleAssembly,
                                             bool LocalBinding = false) {
  std::string Ir;
  raw_string_ostream Stream(Ir);
  Stream << "target triple = \"amdgcn-amd-amdhsa\"\n";
  if (ModuleAssembly && LocalBinding)
    Stream << "module asm \".local " << Symbol << "\"\n";
  if (ModuleAssembly)
    Stream << "module asm \".long " << Symbol << "\"\n";
  Stream << R"IR(
declare float @__ocml_sin_f32(float)

define float @ocml_entry(float %value) {
entry:
)IR";
  if (!ModuleAssembly) {
    Stream << "  call void asm sideeffect \"";
    if (LocalBinding)
      Stream << ".local " << Symbol << "\\0A";
    Stream << ".long " << Symbol << "\", \"\"()\n";
  }
  Stream << R"IR(  %result = call float @__ocml_sin_f32(float %value)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
)IR";
  Stream.flush();
  return std::vector<uint8_t>(Ir.begin(), Ir.end());
}

std::vector<uint8_t> makeIntrinsicCompilerIr(bool Malformed) {
  StringRef ReturnType = Malformed ? "i64" : "i32";
  std::string Ir;
  raw_string_ostream Stream(Ir);
  Stream << "target triple = \"amdgcn-amd-amdhsa\"\n\n"
         << "declare " << ReturnType << " @llvm.amdgcn.workitem.id.x()\n\n"
         << "define " << ReturnType << " @intrinsic_entry() {\n"
         << "entry:\n"
         << "  %id = call " << ReturnType << " @llvm.amdgcn.workitem.id.x()\n"
         << "  ret " << ReturnType << " %id\n"
         << "}\n\n"
         << "!llvm.module.flags = !{!0}\n"
         << "!0 = !{i32 1, !\"amdhsa_code_object_version\", i32 500}\n";
  Stream.flush();
  return std::vector<uint8_t>(Ir.begin(), Ir.end());
}

struct SyntheticOcmlOptions {
  LayoutMode Layout = LayoutMode::Exact;
  bool WrongTriple = false;
  bool WrongAbi = false;
  bool UnresolvedDependency = false;
  uint8_t CodeObjectVersion = 5;
};

std::vector<uint8_t>
makeSyntheticOcmlBitcode(const SyntheticOcmlOptions &Options = {}) {
  LLVMContext Context;
  Module ModuleValue("synthetic-ocml", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(
      Triple(Options.WrongTriple ? "nvptx64-nvidia-cuda" : AmdGpuTriple));
  if (Options.Layout == LayoutMode::Exact)
    ModuleValue.setDataLayout(Machine->createDataLayout());
  else if (Options.Layout == LayoutMode::Incompatible)
    ModuleValue.setDataLayout("e-p:32:32");
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version",
                            Options.CodeObjectVersion * 100);

  Type *F32 = Type::getFloatTy(Context);
  FunctionType *F32Signature = FunctionType::get(F32, {F32}, false);
  Function *Helper =
      Function::Create(F32Signature, GlobalValue::LinkOnceODRLinkage,
                       "__fe2o3_required_ocml_helper", ModuleValue);
  Helper->setVisibility(GlobalValue::HiddenVisibility);
  Helper->addFnAttr(Attribute::NoInline);
  BasicBlock *HelperBlock = BasicBlock::Create(Context, "entry", Helper);
  IRBuilder<> HelperBuilder(HelperBlock);
  HelperBuilder.CreateRet(
      HelperBuilder.CreateFAdd(Helper->getArg(0), ConstantFP::get(F32, 1.0)));

  FunctionType *RootSignature =
      Options.WrongAbi ? FunctionType::get(Type::getInt32Ty(Context),
                                           {Type::getInt32Ty(Context)}, false)
                       : F32Signature;
  Function *Root =
      Function::Create(RootSignature, GlobalValue::LinkOnceODRLinkage,
                       "__ocml_sin_f32", ModuleValue);
  Root->setVisibility(GlobalValue::HiddenVisibility);
  BasicBlock *RootBlock = BasicBlock::Create(Context, "entry", Root);
  IRBuilder<> RootBuilder(RootBlock);
  if (Options.WrongAbi) {
    RootBuilder.CreateRet(RootBuilder.CreateAdd(
        Root->getArg(0), ConstantInt::get(Type::getInt32Ty(Context), 1)));
  } else if (Options.UnresolvedDependency) {
    FunctionCallee Missing = ModuleValue.getOrInsertFunction(
        "__ocml_missing_dependency", F32Signature);
    RootBuilder.CreateRet(RootBuilder.CreateCall(Missing, {Root->getArg(0)}));
  } else {
    RootBuilder.CreateRet(RootBuilder.CreateCall(Helper, {Root->getArg(0)}));
  }

  Function *ExpRoot =
      Function::Create(F32Signature, GlobalValue::LinkOnceODRLinkage,
                       "__ocml_exp_f32", ModuleValue);
  ExpRoot->setVisibility(GlobalValue::HiddenVisibility);
  BasicBlock *ExpBlock = BasicBlock::Create(Context, "entry", ExpRoot);
  IRBuilder<> ExpBuilder(ExpBlock);
  ExpBuilder.CreateRet(ExpBuilder.CreateCall(Helper, {ExpRoot->getArg(0)}));

  Function *UndeclaredRoot =
      Function::Create(F32Signature, GlobalValue::LinkOnceODRLinkage,
                       "__ocml_sqrt_f32", ModuleValue);
  UndeclaredRoot->setVisibility(GlobalValue::HiddenVisibility);
  BasicBlock *UndeclaredBlock =
      BasicBlock::Create(Context, "entry", UndeclaredRoot);
  IRBuilder<> UndeclaredBuilder(UndeclaredBlock);
  UndeclaredBuilder.CreateRet(
      UndeclaredBuilder.CreateCall(Helper, {UndeclaredRoot->getArg(0)}));

  Function *Decoy =
      Function::Create(F32Signature, GlobalValue::LinkOnceODRLinkage,
                       "__ocml_dead_decoy", ModuleValue);
  Decoy->setVisibility(GlobalValue::HiddenVisibility);
  BasicBlock *DecoyBlock = BasicBlock::Create(Context, "entry", Decoy);
  IRBuilder<>(DecoyBlock).CreateRet(Decoy->getArg(0));

  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeEmptyProviderBitcode(StringRef Name) {
  LLVMContext Context;
  Module ModuleValue(Name, Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 500);
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeTextIr(StringRef ModuleName, StringRef Definition,
                                std::optional<StringRef> Callee,
                                const FixtureOptions &Options = {}) {
  LLVMContext Context;
  std::unique_ptr<Module> ModuleValue =
      makeModule(Context, ModuleName, Definition, Callee, Options);
  std::string Buffer;
  raw_string_ostream Stream(Buffer);
  ModuleValue->print(Stream, nullptr);
  Stream.flush();
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t>
makeKernelBitcode(StringRef Name,
                  std::optional<std::array<uint32_t, 3>> RequiredWorkgroup =
                      std::array<uint32_t, 3>{256, 1, 1},
                  uint32_t MaxWorkgroup = 256) {
  LLVMContext Context;
  auto ModuleValue = std::make_unique<Module>("publication-kernel", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue->setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue->setDataLayout(Machine->createDataLayout());
  ModuleValue->addModuleFlag(Module::Error, "amdhsa_code_object_version", 500);

  FunctionType *Signature = FunctionType::get(Type::getVoidTy(Context), false);
  Function *Kernel = Function::Create(Signature, GlobalValue::ExternalLinkage,
                                      Name, *ModuleValue);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr("target-cpu", "gfx942");
  Kernel->addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
  std::string FlatWorkgroup =
      (Twine(MaxWorkgroup) + "," + Twine(MaxWorkgroup)).str();
  Kernel->addFnAttr("amdgpu-flat-work-group-size", FlatWorkgroup);
  if (RequiredWorkgroup) {
    Metadata *Workgroup[] = {
        ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context),
                                                 (*RequiredWorkgroup)[0])),
        ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context),
                                                 (*RequiredWorkgroup)[1])),
        ConstantAsMetadata::get(ConstantInt::get(Type::getInt32Ty(Context),
                                                 (*RequiredWorkgroup)[2]))};
    Kernel->setMetadata("reqd_work_group_size",
                        MDNode::get(Context, Workgroup));
  }
  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
  IRBuilder<>(Entry).CreateRetVoid();

  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(*ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeExactLdsGemmSlice1TextIr(
    uint32_t Workgroup = 64, uint32_t MaxWorkgroup = 64,
    uint32_t StaticLdsTiles = 2,
    StringRef DataLayout = ExactLdsGemmSlice1ProducerDataLayout) {
  LLVMContext Context;
  Module ModuleValue("exact-lds-gemm-slice1", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(DataLayout);
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);

  Type *I16 = Type::getInt16Ty(Context);
  Type *I32 = Type::getInt32Ty(Context);
  Type *I64 = Type::getInt64Ty(Context);
  Type *F32 = Type::getFloatTy(Context);
  Type *GlobalPointer = PointerType::get(Context, 1);
  FunctionType *Signature = FunctionType::get(
      Type::getVoidTy(Context),
      {GlobalPointer, I64, GlobalPointer, I64, GlobalPointer, I64}, false);
  Function *Kernel = Function::Create(Signature, GlobalValue::ExternalLinkage,
                                      "tiled_gemm_lds_v1", ModuleValue);
  Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
  Kernel->addFnAttr("target-cpu", "gfx942");
  Kernel->addFnAttr("target-features",
                    "-wavefrontsize32,+wavefrontsize64,-xnack");
  Kernel->addFnAttr("amdgpu-flat-work-group-size",
                    (Twine(MaxWorkgroup) + "," + Twine(MaxWorkgroup)).str());
  Metadata *Required[] = {
      ConstantAsMetadata::get(ConstantInt::get(I32, Workgroup)),
      ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
      ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
  Kernel->setMetadata("reqd_work_group_size", MDNode::get(Context, Required));

  ArrayType *TileType = ArrayType::get(I16, 256);
  auto MakeTile = [&](StringRef Name) {
    auto *Tile = new GlobalVariable(ModuleValue, TileType, false,
                                    GlobalValue::InternalLinkage,
                                    UndefValue::get(TileType), Name, nullptr,
                                    GlobalVariable::NotThreadLocal, 3);
    Tile->setAlignment(Align(16));
    return Tile;
  };
  GlobalVariable *TileA = MakeTile("tiled_gemm_lds_v1.a.tile");
  GlobalVariable *TileB =
      StaticLdsTiles == 2 ? MakeTile("tiled_gemm_lds_v1.b.tile") : TileA;

  auto ArgumentIt = Kernel->arg_begin();
  Argument *A = &*ArgumentIt++;
  Argument *ALength = &*ArgumentIt++;
  Argument *B = &*ArgumentIt++;
  Argument *BLength = &*ArgumentIt++;
  Argument *C = &*ArgumentIt++;
  Argument *CLength = &*ArgumentIt;
  A->setName("arg0.data");
  ALength->setName("arg0.len");
  B->setName("arg1.data");
  BLength->setName("arg1.len");
  C->setName("arg2.data");
  CLength->setName("arg2.len");
  for (Argument *Pointer : {A, B, C}) {
    Pointer->addAttr(Attribute::NoAlias);
    Pointer->addAttr(
        Attribute::getWithCaptureInfo(Context, CaptureInfo::none()));
  }
  A->addAttr(Attribute::ReadOnly);
  B->addAttr(Attribute::ReadOnly);
  A->addAttr(Attribute::getWithAlignment(Context, Align(2)));
  B->addAttr(Attribute::getWithAlignment(Context, Align(2)));
  C->addAttr(Attribute::getWithAlignment(Context, Align(4)));

  Metadata *AccessQualifiers[] = {
      MDString::get(Context, "read_only"),  MDString::get(Context, "none"),
      MDString::get(Context, "read_only"),  MDString::get(Context, "none"),
      MDString::get(Context, "read_write"), MDString::get(Context, "none")};
  Metadata *TypeNames[] = {
      MDString::get(Context, "ushort*"), MDString::get(Context, "ulong"),
      MDString::get(Context, "ushort*"), MDString::get(Context, "ulong"),
      MDString::get(Context, "float*"),  MDString::get(Context, "ulong")};
  Metadata *TypeQualifiers[] = {
      MDString::get(Context, "const"),    MDString::get(Context, ""),
      MDString::get(Context, "const"),    MDString::get(Context, ""),
      MDString::get(Context, "restrict"), MDString::get(Context, "")};
  Kernel->setMetadata("kernel_arg_access_qual",
                      MDNode::get(Context, AccessQualifiers));
  MDNode *Types = MDNode::get(Context, TypeNames);
  Kernel->setMetadata("kernel_arg_type", Types);
  Kernel->setMetadata("kernel_arg_base_type", Types);
  Kernel->setMetadata("kernel_arg_type_qual",
                      MDNode::get(Context, TypeQualifiers));

  BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
  IRBuilder<> Builder(Entry);
  Value *Zero32 = ConstantInt::get(I32, 0);
  Value *Zero64 = ConstantInt::get(I64, 0);
  Value *AValue =
      Builder.CreateLoad(I16, Builder.CreateInBoundsGEP(I16, A, Zero64));
  Value *BValue =
      Builder.CreateLoad(I16, Builder.CreateInBoundsGEP(I16, B, Zero64));
  Value *TileAElement =
      Builder.CreateInBoundsGEP(TileType, TileA, {Zero32, Zero32});
  Value *TileBElement =
      Builder.CreateInBoundsGEP(TileType, TileB, {Zero32, Zero32});
  Builder.CreateStore(AValue, TileAElement)->setVolatile(true);
  Builder.CreateStore(BValue, TileBElement)->setVolatile(true);
  auto *LoadedA = Builder.CreateLoad(I16, TileAElement);
  LoadedA->setVolatile(true);
  auto *LoadedB = Builder.CreateLoad(I16, TileBElement);
  LoadedB->setVolatile(true);
  Value *Sum = Builder.CreateAdd(LoadedA, LoadedB);
  Value *Lengths =
      Builder.CreateAdd(Builder.CreateAdd(ALength, BLength), CLength);
  Value *LengthBit = Builder.CreateTrunc(Lengths, I16);
  Value *Observed = Builder.CreateAdd(Sum, LengthBit);
  Builder.CreateStore(Builder.CreateUIToFP(Observed, F32),
                      Builder.CreateInBoundsGEP(F32, C, Zero64));
  Builder.CreateRetVoid();

  std::string Text;
  raw_string_ostream Stream(Text);
  ModuleValue.print(Stream, nullptr);
  Stream.flush();
  return std::vector<uint8_t>(Text.begin(), Text.end());
}

std::vector<uint8_t> makeCov6TwoKernelBitcode() {
  LLVMContext Context;
  Module ModuleValue("cov6-two-kernel", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 600);

  Type *I32 = Type::getInt32Ty(Context);
  FunctionType *HelperSignature = FunctionType::get(I32, {I32}, false);
  FunctionCallee SharedHelper =
      ModuleValue.getOrInsertFunction("cov6_shared_helper", HelperSignature);
  Type *GlobalPointer = PointerType::get(Context, 1);
  FunctionType *KernelSignature =
      FunctionType::get(Type::getVoidTy(Context), {GlobalPointer, I32}, false);

  auto AddKernel = [&](StringRef Name, uint32_t Addend) {
    Function *Kernel = Function::Create(
        KernelSignature, GlobalValue::ExternalLinkage, Name, ModuleValue);
    Kernel->setCallingConv(CallingConv::AMDGPU_KERNEL);
    Kernel->addFnAttr("target-cpu", "gfx942");
    Kernel->addFnAttr("target-features", "-wavefrontsize32,+wavefrontsize64");
    Kernel->addFnAttr("amdgpu-flat-work-group-size", "256,256");
    Metadata *Workgroup[] = {
        ConstantAsMetadata::get(ConstantInt::get(I32, 256)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1)),
        ConstantAsMetadata::get(ConstantInt::get(I32, 1))};
    Kernel->setMetadata("reqd_work_group_size",
                        MDNode::get(Context, Workgroup));

    BasicBlock *Entry = BasicBlock::Create(Context, "entry", Kernel);
    IRBuilder<> Builder(Entry);
    Value *Input =
        Builder.CreateAdd(Kernel->getArg(1), ConstantInt::get(I32, Addend));
    Value *Output = Builder.CreateCall(SharedHelper, {Input});
    Builder.CreateStore(Output, Kernel->getArg(0));
    Builder.CreateRetVoid();
  };
  AddKernel("cov6_alpha", 1);
  AddKernel("cov6_bravo", 2);

  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::vector<uint8_t> makeObject(StringRef ModuleName, StringRef Definition,
                                std::optional<StringRef> Callee,
                                const FixtureOptions &Options = {}) {
  LLVMContext Context;
  std::unique_ptr<Module> ModuleValue =
      makeModule(Context, ModuleName, Definition, Callee, Options);
  std::unique_ptr<TargetMachine> Machine = createMachine(Options.Cpu);
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  legacy::PassManager Passes;
  require(!Machine->addPassesToEmitFile(Passes, Stream, nullptr,
                                        CodeGenFileType::ObjectFile, false),
          "fixture target machine cannot emit objects");
  Passes.run(*ModuleValue);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

std::string loadIntegratedWave64CollectivesV1Body() {
  SmallString<256> SourcePath(__FILE__);
  sys::path::remove_filename(SourcePath);
  sys::path::append(SourcePath, "..", "..", "..");
  sys::path::append(SourcePath, "crates", "fe2o3-hsaco-finalize", "src");
  sys::path::append(SourcePath, "wave64_collectives_v1_worker.rs");
  auto Source = MemoryBuffer::getFile(SourcePath);
  if (!Source)
    fail((Twine("cannot read integrated Wave64 source: ") +
          Source.getError().message())
             .str());
  StringRef Rust = (*Source)->getBuffer();
  constexpr StringLiteral Prefix = "const CANONICAL_LLVM_BODY: &str = r#\"";
  size_t Begin = Rust.find(Prefix);
  require(Begin != StringRef::npos,
          "integrated Wave64 source has no canonical LLVM body");
  Begin += Prefix.size();
  size_t End = Rust.find("\"#;", Begin);
  require(End != StringRef::npos,
          "integrated Wave64 source has an unterminated LLVM body");
  return Rust.slice(Begin, End).str();
}

void appendWave64CompilerSection(std::vector<uint8_t> &Module, StringRef Name,
                                 ArrayRef<uint8_t> Bytes) {
  std::string Header = (Twine("\nmodule asm \".section ") + Name +
                        ",\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n")
                           .str();
  llvm::append_range(Module, arrayRefFromStringRef(Header));
  for (size_t Offset = 0; Offset != Bytes.size(); Offset += 16) {
    std::string Line = "module asm \".byte ";
    raw_string_ostream Stream(Line);
    size_t End = std::min(Bytes.size(), Offset + 16);
    for (size_t Index = Offset; Index != End; ++Index) {
      if (Index != Offset)
        Stream << ", ";
      Stream << "0x" << format_hex_no_prefix(Bytes[Index], 2);
    }
    Stream << "\"\n";
    Stream.flush();
    llvm::append_range(Module, arrayRefFromStringRef(Line));
  }
}

constexpr std::array<uint8_t, 32> ExactWave64MirSha256 = {
    0x9b, 0xfb, 0x30, 0x50, 0x89, 0x75, 0x1c, 0xe7, 0x59, 0x32, 0x27,
    0x06, 0x97, 0x68, 0xd5, 0xe7, 0xa9, 0x83, 0x60, 0xb7, 0xde, 0xd8,
    0x90, 0xf8, 0xb2, 0x13, 0xa0, 0xd9, 0x5d, 0x15, 0x8e, 0x7a};
constexpr std::array<uint8_t, 32> ExactWave64KirSha256 = {
    0x7d, 0x88, 0x09, 0x25, 0xf5, 0xb3, 0xee, 0x4f, 0xcb, 0xb5, 0xd6,
    0xbe, 0x34, 0xa4, 0xbe, 0x63, 0xfd, 0xe6, 0x99, 0x14, 0x69, 0x3a,
    0x66, 0xb6, 0x35, 0xb7, 0x19, 0xae, 0x41, 0xf7, 0xba, 0x96};
constexpr std::array<uint8_t, 32> ExactWave64ProfileSha256 = {
    0xcd, 0x6f, 0x6c, 0x45, 0xf3, 0x78, 0x3b, 0xf9, 0x44, 0xf6, 0xa4,
    0xe0, 0xc4, 0x01, 0xf2, 0xaa, 0xda, 0x7c, 0x7d, 0x5d, 0x40, 0xbe,
    0xa7, 0xac, 0xee, 0x6a, 0xe7, 0xcf, 0xf4, 0x0f, 0x77, 0x68};

std::vector<uint8_t> makeExactWave64CollectivesV1TextIr(
    ArrayRef<uint8_t> Descriptor = ArrayRef<uint8_t>(),
    ArrayRef<uint8_t> Authority = ArrayRef<uint8_t>(),
    ArrayRef<uint8_t> Mir = ExactWave64MirSha256,
    ArrayRef<uint8_t> Kir = ExactWave64KirSha256,
    ArrayRef<uint8_t> Profile = ExactWave64ProfileSha256) {
  std::array<uint8_t, 64> DefaultDescriptor{};
  DefaultDescriptor.fill(0xd3);
  std::array<uint8_t, 32> DefaultAuthority{};
  DefaultAuthority.fill(0xa5);
  if (Descriptor.empty())
    Descriptor = DefaultDescriptor;
  if (Authority.empty())
    Authority = DefaultAuthority;
  std::string Body = loadIntegratedWave64CollectivesV1Body();
  std::vector<uint8_t> Result(Body.begin(), Body.end());
  appendWave64CompilerSection(Result, ".fe2o3.kd.v1", Descriptor);
  appendWave64CompilerSection(Result, ".fe2o3.wave64-auth.v1", Authority);
  appendWave64CompilerSection(Result, ".fe2o3.wave64-mir.v1", Mir);
  appendWave64CompilerSection(Result, ".fe2o3.wave64-kir.v1", Kir);
  appendWave64CompilerSection(Result, ".fe2o3.wave64-descriptor.v1", Profile);
  return Result;
}

std::string loadIntegratedWorkgroupSyncBody(StringRef Filename) {
  SmallString<256> SourcePath(__FILE__);
  sys::path::remove_filename(SourcePath);
  sys::path::append(SourcePath, "..", "..", "..");
  sys::path::append(SourcePath, "crates", "fe2o3-hsaco-finalize", "src");
  sys::path::append(SourcePath, Filename);
  auto Source = MemoryBuffer::getFile(SourcePath);
  if (!Source)
    fail((Twine("cannot read integrated workgroup-sync source: ") +
          Source.getError().message())
             .str());
  StringRef Rust = (*Source)->getBuffer();
  constexpr StringLiteral Prefix = "const LLVM_BODY_TAIL: &str = r#\"";
  size_t Begin = Rust.find(Prefix);
  require(Begin != StringRef::npos,
          "integrated workgroup-sync source has no LLVM body");
  Begin += Prefix.size();
  size_t End = Rust.find("\"#;", Begin);
  require(End != StringRef::npos,
          "integrated workgroup-sync source has an unterminated LLVM body");
  auto DataLayout = exactWorkgroupSyncDataLayoutForTesting();
  if (!DataLayout)
    fail(toString(DataLayout.takeError()));
  return (Twine("target triple = \"amdgcn-amd-amdhsa\"\n"
                "target datalayout = \"") +
          *DataLayout + "\"\n\n" + Rust.slice(Begin, End))
      .str();
}

std::string loadExactRowSoftmaxV1Body() {
  SmallString<256> FixturePath(__FILE__);
  sys::path::remove_filename(FixturePath);
  sys::path::append(FixturePath, "fixtures", "row-softmax-v1-llvm22-body.ll");
  auto Fixture = MemoryBuffer::getFile(FixturePath);
  if (!Fixture)
    fail((Twine("cannot read exact row-softmax fixture: ") +
          Fixture.getError().message())
             .str());
  return (*Fixture)->getBuffer().str();
}

std::vector<uint8_t> makeExactRowSoftmaxV1TextIr() {
  std::string Body = loadExactRowSoftmaxV1Body();
  std::array<uint8_t, 64> Descriptor{};
  Descriptor.fill(0x52);
  constexpr StringLiteral Transcript =
      "row-softmax-cpp-profile-authority-test-v1";
  auto Result = makeExactRowSoftmaxV1CompilerInputForTesting(
      Body, Descriptor, arrayRefFromStringRef(Transcript));
  if (!Result)
    fail(toString(Result.takeError()));
  return std::move(*Result);
}

std::vector<uint8_t>
makeExactWorkgroupSyncTextIr(ExactWorkgroupSyncProfileForTesting Profile) {
  StringRef Filename =
      Profile == ExactWorkgroupSyncProfileForTesting::LdsReduction
          ? "workgroup_lds_reduction_v1_profile.rs"
          : "workgroup_scoped_atomic_v1_profile.rs";
  std::string Body = loadIntegratedWorkgroupSyncBody(Filename);
  std::array<uint8_t, 64> Descriptor{};
  Descriptor.fill(Profile == ExactWorkgroupSyncProfileForTesting::LdsReduction
                      ? 0xd1
                      : 0xa7);
  auto Result =
      makeExactWorkgroupSyncCompilerInputForTesting(Body, Descriptor, Profile);
  if (!Result)
    fail(toString(Result.takeError()));
  return std::move(*Result);
}

std::string replaceExactText(StringRef Source, StringRef Expected,
                             StringRef Replacement) {
  size_t Position = Source.find(Expected);
  require(Position != StringRef::npos,
          (Twine("workgroup-sync fixture omitted ") + Expected).str());
  std::string Result = Source.str();
  Result.replace(Position, Expected.size(), Replacement.str());
  return Result;
}

void mutateExactCompilerSectionIdentity(std::vector<uint8_t> &Bytes,
                                        StringRef Section) {
  StringRef Text(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  size_t Position = Text.find(Section);
  require(Position != StringRef::npos,
          "workgroup-sync fixture omitted an identity section");
  Position = Text.find("0x", Position);
  require(Position != StringRef::npos && Position + 2 < Bytes.size(),
          "workgroup-sync identity section omitted its bytes");
  Bytes[Position + 2] = Bytes[Position + 2] == '0' ? '1' : '0';
}

Input makeInput(InputKind Kind, std::vector<uint8_t> Bytes) {
  std::array<uint8_t, 32> Digest = SHA256::hash(Bytes);
  return {Kind, Digest, std::move(Bytes)};
}

Request makeRequest(std::vector<Input> Inputs,
                    std::vector<std::string> ExpectedSymbols,
                    StringRef Target = "gfx942",
                    uint8_t CodeObjectVersion = 5) {
  llvm::sort(Inputs, [](const Input &Left, const Input &Right) {
    return std::tuple(Left.Digest, Left.Bytes.size(), Left.Kind) <
           std::tuple(Right.Digest, Right.Bytes.size(), Right.Kind);
  });
  llvm::sort(ExpectedSymbols);
  Request Result;
  Result.RequestId.fill(0x31);
  Result.Identity.fill(0x72);
  Result.LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
  Result.Target = Target.str();
  Result.CodeObjectVersion = CodeObjectVersion;
  Result.LinkOptions = {OptimizationLevel::O3, true, true};
  Result.Inputs = std::move(Inputs);
  Result.RequiredSymbols = ExpectedSymbols;
  Result.ExpectedDefinedSymbols = std::move(ExpectedSymbols);
  Result.MaxOutputBytes = 4 * 1024 * 1024;
  return Result;
}

Request makeV2Request(Input CompilerModule,
                      std::vector<Input> ExternalProviders,
                      std::vector<std::string> Imports,
                      std::vector<std::string> Exports,
                      std::vector<std::string> FinalSymbols,
                      uint8_t CodeObjectVersion = 5) {
  std::vector<Input> Inputs = ExternalProviders;
  Inputs.push_back(CompilerModule);
  Request Result =
      makeRequest(std::move(Inputs), FinalSymbols, "gfx942", CodeObjectVersion);
  Result.Protocol = ProtocolVersion::V2;
  Result.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
  Result.WorkerExecutableDigest.fill(0x51);
  Result.WorkerExecutableBytes = 4096;
  Result.CompilerEnvelopeIdentity.fill(0x62);
  Result.CompilerModule = std::move(CompilerModule);
  Result.ExternalProviders = std::move(ExternalProviders);
  llvm::sort(Imports);
  llvm::sort(Exports);
  llvm::sort(FinalSymbols);
  Result.ImportSymbols = std::move(Imports);
  Result.ExportSymbols = std::move(Exports);
  Result.FinalSymbols = std::move(FinalSymbols);
  return Result;
}

std::set<std::string> inspectHsaco(ArrayRef<uint8_t> Bytes,
                                   uint8_t CodeObjectVersion) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "fixture.hsaco"));
  if (!ObjectOrError)
    fail(toString(ObjectOrError.takeError()));
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  require(Elf != nullptr, "output is not ELF");
  require(Elf->getEMachine() == ELF::EM_AMDGPU, "output is not AMDGPU ELF");
  require(Elf->getEType() == ELF::ET_DYN, "output is not a shared ELF");
  require(Bytes.size() >= ELF::EI_NIDENT, "output has a truncated ELF header");
  require(Bytes[ELF::EI_CLASS] == ELF::ELFCLASS64 &&
              Bytes[ELF::EI_DATA] == ELF::ELFDATA2LSB &&
              Bytes[ELF::EI_VERSION] == ELF::EV_CURRENT,
          "output has the wrong ELF envelope");
  require(Bytes[ELF::EI_OSABI] == ELF::ELFOSABI_AMDGPU_HSA,
          "output does not use the AMDHSA OS ABI");
  uint8_t ExpectedAbiVersion = 0;
  switch (CodeObjectVersion) {
  case 4:
    ExpectedAbiVersion = ELF::ELFABIVERSION_AMDGPU_HSA_V4;
    break;
  case 5:
    ExpectedAbiVersion = ELF::ELFABIVERSION_AMDGPU_HSA_V5;
    break;
  case 6:
    ExpectedAbiVersion = ELF::ELFABIVERSION_AMDGPU_HSA_V6;
    break;
  default:
    fail("test requested an unsupported code-object version");
  }
  require(Elf->getEIdentABIVersion() == ExpectedAbiVersion,
          "output does not use the requested code-object version");

  std::set<std::string> Symbols;
  for (SymbolRef Symbol : Elf->getDynamicSymbolIterators()) {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      fail(toString(FlagsOrError.takeError()));
    require((*FlagsOrError & SymbolRef::SF_Undefined) == 0,
            "output has an unresolved dynamic symbol");
    if ((*FlagsOrError & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) == 0 ||
        (*FlagsOrError &
         (SymbolRef::SF_FormatSpecific | SymbolRef::SF_Hidden)) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      fail(toString(NameOrError.takeError()));
    if (!NameOrError->empty())
      Symbols.insert(NameOrError->str());
  }
  return Symbols;
}

Response runSuccess(const Request &RequestValue,
                    const std::set<std::string> &ExpectedSymbols) {
  Response Result = execute(RequestValue);
  if (!Result.LinkedOutput) {
    errs() << "failed request exports:";
    for (const std::string &Symbol : ExpectedSymbols)
      errs() << ' ' << Symbol;
    errs() << '\n';
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("expected worker success");
  }
  require(Result.FailureStage == Stage::Complete,
          "success reported the wrong stage");
  require(Result.LinkedOutput->Digest ==
              SHA256::hash(Result.LinkedOutput->Bytes),
          "success output digest is incorrect");
  require(inspectHsaco(Result.LinkedOutput->Bytes,
                       RequestValue.CodeObjectVersion) == ExpectedSymbols,
          "HSACO exports do not match the request");
  return Result;
}

Response runSuccessWithPolicy(const Request &RequestValue,
                              const Gfx942DeviceLibraryPolicy &Policy,
                              const std::set<std::string> &ExpectedSymbols) {
  Response Result =
      executeWithUnauthenticatedGfx942DeviceLibraryPolicyForTesting(
          RequestValue, Policy);
  if (!Result.LinkedOutput) {
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("expected synthetic OCML worker success");
  }
  require(Result.FailureStage == Stage::Complete,
          "synthetic OCML success reported the wrong stage");
  require(Result.LinkedOutput->Digest ==
              SHA256::hash(Result.LinkedOutput->Bytes),
          "synthetic OCML output digest is incorrect");
  require(inspectHsaco(Result.LinkedOutput->Bytes,
                       RequestValue.CodeObjectVersion) == ExpectedSymbols,
          "synthetic OCML HSACO exports do not match the request");
  require(Result.WorkerBuildIdentity ==
              "fe2o3-unauthenticated-test-device-library-policy",
          "synthetic provider result claimed the measured worker identity");
  require(Result.DeviceLibraryProvider.has_value(),
          "synthetic OCML success omitted structured provider evidence");
  const DeviceLibraryProviderEvidence &Evidence = *Result.DeviceLibraryProvider;
  require(Evidence.ProviderIdentity == "gfx942-ocml-v1" &&
              Evidence.Target == RequestValue.Target &&
              Evidence.CodeObjectVersion == RequestValue.CodeObjectVersion,
          "synthetic OCML provider evidence changed its target profile");
  require(Evidence.ImportSymbols == RequestValue.ImportSymbols,
          "synthetic OCML provider evidence changed its import closure");
  require(Evidence.Files.size() == Policy.Files.size(),
          "synthetic OCML provider evidence changed its file count");
  for (size_t I = 0; I < Policy.Files.size(); ++I)
    require(Evidence.Files[I].Basename == Policy.Files[I].Basename &&
                Evidence.Files[I].Digest == Policy.Files[I].Digest,
            "synthetic OCML provider evidence changed an ordered file pin");
  auto ManifestIdentity = calculateProviderManifestIdentity(Evidence);
  require(static_cast<bool>(ManifestIdentity) &&
              *ManifestIdentity == Evidence.ManifestIdentity,
          "synthetic OCML provider evidence has the wrong identity");
  return Result;
}

Response requireFailure(const Request &RequestValue, Stage ExpectedStage) {
  Response Result = execute(RequestValue);
  require(!Result.LinkedOutput, "rejected request returned output bytes");
  if (Result.FailureStage != ExpectedStage) {
    errs() << "unexpected failure stage: expected "
           << static_cast<unsigned>(ExpectedStage) << ", got "
           << static_cast<unsigned>(Result.FailureStage) << '\n';
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("request failed at an unexpected stage");
  }
  require(!Result.Diagnostics.empty(), "failure omitted diagnostics");
  size_t Total = 0;
  for (const std::string &Diagnostic : Result.Diagnostics) {
    require(Diagnostic.size() <= MaxDiagnosticBytes,
            "diagnostic exceeded its byte bound");
    Total += Diagnostic.size();
  }
  require(Total <= MaxTotalDiagnosticBytes,
          "diagnostics exceeded their total byte bound");
  return Result;
}

Response requireFailureWithPolicy(const Request &RequestValue,
                                  const Gfx942DeviceLibraryPolicy &Policy,
                                  Stage ExpectedStage) {
  Response Result =
      executeWithUnauthenticatedGfx942DeviceLibraryPolicyForTesting(
          RequestValue, Policy);
  require(!Result.LinkedOutput,
          "rejected synthetic OCML request returned output bytes");
  if (Result.FailureStage != ExpectedStage) {
    for (const std::string &Diagnostic : Result.Diagnostics)
      errs() << Diagnostic << '\n';
    fail("synthetic OCML request failed at an unexpected stage");
  }
  require(!Result.Diagnostics.empty(),
          "synthetic OCML failure omitted diagnostics");
  return Result;
}

void requireDiagnostic(const Response &ResponseValue, StringRef Text) {
  if (llvm::any_of(ResponseValue.Diagnostics,
                   [Text](const std::string &Diagnostic) {
                     return StringRef(Diagnostic).contains(Text);
                   }))
    return;
  errs() << "missing diagnostic: " << Text << '\n';
  for (const std::string &Diagnostic : ResponseValue.Diagnostics)
    errs() << "actual diagnostic: " << Diagnostic << '\n';
  fail("response omitted an expected diagnostic");
}

void requireInspectionFailure(ArrayRef<uint8_t> Bytes,
                              const Request &RequestValue,
                              StringRef ExpectedDiagnostic) {
  auto Inspection = inspectLinkedOutputForPublication(Bytes, RequestValue);
  require(!Inspection, "adversarial output passed publication inspection");
  std::string Diagnostic = toString(Inspection.takeError());
  if (!StringRef(Diagnostic).contains(ExpectedDiagnostic)) {
    errs() << "unexpected publication diagnostic: " << Diagnostic << '\n';
    fail("publication inspection failed for the wrong reason");
  }
}

uint64_t read64(ArrayRef<uint8_t> Bytes, size_t Offset) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 8,
          "fixture ELF read is out of bounds");
  return support::endian::read64le(Bytes.data() + Offset);
}

uint32_t read32(ArrayRef<uint8_t> Bytes, size_t Offset) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 4,
          "fixture ELF read is out of bounds");
  return support::endian::read32le(Bytes.data() + Offset);
}

uint16_t read16(ArrayRef<uint8_t> Bytes, size_t Offset) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 2,
          "fixture ELF read is out of bounds");
  return support::endian::read16le(Bytes.data() + Offset);
}

void write32(MutableArrayRef<uint8_t> Bytes, size_t Offset, uint32_t Value) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 4,
          "fixture ELF write is out of bounds");
  support::endian::write32le(Bytes.data() + Offset, Value);
}

void write64(MutableArrayRef<uint8_t> Bytes, size_t Offset, uint64_t Value) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 8,
          "fixture ELF write is out of bounds");
  support::endian::write64le(Bytes.data() + Offset, Value);
}

void write16(MutableArrayRef<uint8_t> Bytes, size_t Offset, uint16_t Value) {
  require(Offset <= Bytes.size() && Bytes.size() - Offset >= 2,
          "fixture ELF write is out of bounds");
  support::endian::write16le(Bytes.data() + Offset, Value);
}

void makeDynamicSymbolUndefined(
    std::vector<uint8_t> &Bytes, StringRef SymbolName,
    std::optional<StringRef> Replacement = std::nullopt) {
  constexpr size_t Elf64SectionTypeOffset = 4;
  constexpr size_t Elf64SectionOffsetOffset = 24;
  constexpr size_t Elf64SectionSizeOffset = 32;
  constexpr size_t Elf64SectionLinkOffset = 40;
  constexpr size_t Elf64SectionEntrySizeOffset = 56;
  constexpr size_t Elf64SymbolNameOffset = 0;
  constexpr size_t Elf64SymbolSectionIndexOffset = 6;
  uint64_t SectionTable = read64(Bytes, 40);
  uint16_t SectionEntrySize = read16(Bytes, 58);
  uint16_t SectionCount = read16(Bytes, 60);
  require(SectionEntrySize >= 64, "fixture has a short section header");

  for (uint16_t I = 0; I < SectionCount; ++I) {
    size_t Section = SectionTable + static_cast<uint64_t>(I) * SectionEntrySize;
    if (read32(Bytes, Section + Elf64SectionTypeOffset) != ELF::SHT_DYNSYM)
      continue;
    uint64_t Symbols = read64(Bytes, Section + Elf64SectionOffsetOffset);
    uint64_t SymbolBytes = read64(Bytes, Section + Elf64SectionSizeOffset);
    uint64_t SymbolSize = read64(Bytes, Section + Elf64SectionEntrySizeOffset);
    uint32_t StringsIndex = read32(Bytes, Section + Elf64SectionLinkOffset);
    require(StringsIndex < SectionCount && SymbolSize >= 24,
            "fixture has an invalid dynamic symbol section");
    size_t StringsSection =
        SectionTable + static_cast<uint64_t>(StringsIndex) * SectionEntrySize;
    uint64_t Strings = read64(Bytes, StringsSection + Elf64SectionOffsetOffset);
    uint64_t StringBytes =
        read64(Bytes, StringsSection + Elf64SectionSizeOffset);
    for (uint64_t Offset = 0; Offset < SymbolBytes; Offset += SymbolSize) {
      uint32_t NameOffset =
          read32(Bytes, Symbols + Offset + Elf64SymbolNameOffset);
      require(NameOffset < StringBytes,
              "fixture dynamic symbol has an invalid name");
      const char *Name =
          reinterpret_cast<const char *>(Bytes.data() + Strings + NameOffset);
      size_t Remaining = StringBytes - NameOffset;
      size_t Length = strnlen(Name, Remaining);
      require(Length < Remaining, "fixture dynamic symbol is unterminated");
      if (StringRef(Name, Length) != SymbolName)
        continue;
      if (Replacement) {
        require(Replacement->size() <= Length,
                "replacement dynamic symbol is too long");
        std::fill(Bytes.begin() + Strings + NameOffset,
                  Bytes.begin() + Strings + NameOffset + Length, 0);
        llvm::copy(*Replacement, Bytes.begin() + Strings + NameOffset);
      }
      write16(Bytes, Symbols + Offset + Elf64SymbolSectionIndexOffset,
              ELF::SHN_UNDEF);
      return;
    }
  }
  fail("fixture did not contain the requested dynamic symbol");
}

void overwriteStaticSymbolPrefix(std::vector<uint8_t> &Bytes,
                                 StringRef SymbolName,
                                 ArrayRef<uint8_t> Prefix) {
  constexpr size_t Elf64SectionTypeOffset = 4;
  constexpr size_t Elf64SectionAddressOffset = 16;
  constexpr size_t Elf64SectionOffsetOffset = 24;
  constexpr size_t Elf64SectionSizeOffset = 32;
  constexpr size_t Elf64SectionLinkOffset = 40;
  constexpr size_t Elf64SectionEntrySizeOffset = 56;
  constexpr size_t Elf64SymbolNameOffset = 0;
  constexpr size_t Elf64SymbolSectionIndexOffset = 6;
  constexpr size_t Elf64SymbolValueOffset = 8;
  constexpr size_t Elf64SymbolSizeOffset = 16;
  uint64_t SectionTable = read64(Bytes, 40);
  uint16_t SectionEntrySize = read16(Bytes, 58);
  uint16_t SectionCount = read16(Bytes, 60);
  require(SectionEntrySize >= 64, "fixture has a short section header");

  for (uint16_t I = 0; I < SectionCount; ++I) {
    size_t Section = SectionTable + static_cast<uint64_t>(I) * SectionEntrySize;
    if (read32(Bytes, Section + Elf64SectionTypeOffset) != ELF::SHT_SYMTAB)
      continue;
    uint64_t Symbols = read64(Bytes, Section + Elf64SectionOffsetOffset);
    uint64_t SymbolBytes = read64(Bytes, Section + Elf64SectionSizeOffset);
    uint64_t SymbolSize = read64(Bytes, Section + Elf64SectionEntrySizeOffset);
    uint32_t StringsIndex = read32(Bytes, Section + Elf64SectionLinkOffset);
    require(StringsIndex < SectionCount && SymbolSize >= 24,
            "fixture has an invalid static symbol section");
    size_t StringsSection =
        SectionTable + static_cast<uint64_t>(StringsIndex) * SectionEntrySize;
    uint64_t Strings = read64(Bytes, StringsSection + Elf64SectionOffsetOffset);
    uint64_t StringBytes =
        read64(Bytes, StringsSection + Elf64SectionSizeOffset);
    for (uint64_t Offset = 0; Offset < SymbolBytes; Offset += SymbolSize) {
      uint32_t NameOffset =
          read32(Bytes, Symbols + Offset + Elf64SymbolNameOffset);
      require(NameOffset < StringBytes,
              "fixture static symbol has an invalid name");
      const char *Name =
          reinterpret_cast<const char *>(Bytes.data() + Strings + NameOffset);
      size_t Remaining = StringBytes - NameOffset;
      size_t Length = strnlen(Name, Remaining);
      require(Length < Remaining, "fixture static symbol is unterminated");
      if (StringRef(Name, Length) != SymbolName)
        continue;
      uint16_t CodeSectionIndex =
          read16(Bytes, Symbols + Offset + Elf64SymbolSectionIndexOffset);
      require(CodeSectionIndex < SectionCount,
              "fixture code symbol has an invalid section");
      uint64_t Value = read64(Bytes, Symbols + Offset + Elf64SymbolValueOffset);
      uint64_t Size = read64(Bytes, Symbols + Offset + Elf64SymbolSizeOffset);
      require(Prefix.size() <= Size, "fixture code symbol is too short");
      size_t CodeSection =
          SectionTable +
          static_cast<uint64_t>(CodeSectionIndex) * SectionEntrySize;
      uint64_t SectionAddress =
          read64(Bytes, CodeSection + Elf64SectionAddressOffset);
      uint64_t SectionOffset =
          read64(Bytes, CodeSection + Elf64SectionOffsetOffset);
      require(Value >= SectionAddress,
              "fixture code symbol precedes its section");
      uint64_t FileOffset = SectionOffset + Value - SectionAddress;
      require(FileOffset <= Bytes.size() &&
                  Prefix.size() <= Bytes.size() - FileOffset,
              "fixture code symbol is outside file bytes");
      llvm::copy(Prefix, Bytes.begin() + FileOffset);
      return;
    }
  }
  fail("fixture did not contain the requested static symbol");
}

void makeStaticSymbolTableRelocationSection(std::vector<uint8_t> &Bytes) {
  constexpr size_t Elf64SectionTypeOffset = 4;
  uint64_t SectionTable = read64(Bytes, 40);
  uint16_t SectionEntrySize = read16(Bytes, 58);
  uint16_t SectionCount = read16(Bytes, 60);
  require(SectionEntrySize >= 64, "fixture has a short section header");
  for (uint16_t I = 0; I < SectionCount; ++I) {
    size_t Section = SectionTable + static_cast<uint64_t>(I) * SectionEntrySize;
    if (read32(Bytes, Section + Elf64SectionTypeOffset) != ELF::SHT_SYMTAB)
      continue;
    write32(Bytes, Section + Elf64SectionTypeOffset, ELF::SHT_RELA);
    return;
  }
  fail("fixture did not contain a static symbol table");
}

void makeDynamicDependency(std::vector<uint8_t> &Bytes) {
  constexpr size_t Elf64SectionTypeOffset = 4;
  constexpr size_t Elf64SectionOffsetOffset = 24;
  constexpr size_t Elf64SectionSizeOffset = 32;
  constexpr size_t Elf64SectionEntrySizeOffset = 56;
  uint64_t SectionTable = read64(Bytes, 40);
  uint16_t SectionEntrySize = read16(Bytes, 58);
  uint16_t SectionCount = read16(Bytes, 60);
  require(SectionEntrySize >= 64, "fixture has a short section header");
  for (uint16_t I = 0; I < SectionCount; ++I) {
    size_t Section = SectionTable + static_cast<uint64_t>(I) * SectionEntrySize;
    if (read32(Bytes, Section + Elf64SectionTypeOffset) != ELF::SHT_DYNAMIC)
      continue;
    uint64_t Entries = read64(Bytes, Section + Elf64SectionOffsetOffset);
    uint64_t EntryBytes = read64(Bytes, Section + Elf64SectionSizeOffset);
    uint64_t EntrySize = read64(Bytes, Section + Elf64SectionEntrySizeOffset);
    require(EntrySize >= sizeof(ELF64LE::Dyn) && EntryBytes >= EntrySize,
            "fixture has an invalid dynamic section");
    for (uint64_t Offset = 0; Offset < EntryBytes; Offset += EntrySize) {
      if (read64(Bytes, Entries + Offset) != ELF::DT_NULL)
        continue;
      write64(Bytes, Entries + Offset, ELF::DT_NEEDED);
      return;
    }
  }
  fail("fixture did not contain a dynamic terminator");
}

void mutateNamedSectionByte(std::vector<uint8_t> &Bytes,
                            StringRef SectionName) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<mutation>"));
  if (!ObjectOrError)
    fail(toString(ObjectOrError.takeError()));
  auto *Object = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  require(Object != nullptr, "mutation fixture is not ELF64LE");
  const ELFFile<ELF64LE> &File = Object->getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    fail(toString(Sections.takeError()));
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      fail(toString(Name.takeError()));
    if (*Name != SectionName)
      continue;
    require(Section.sh_size != 0 && Section.sh_offset < Bytes.size() &&
                Section.sh_size <= Bytes.size() - Section.sh_offset,
            "mutation section range is invalid");
    Bytes[Section.sh_offset] ^= 1;
    return;
  }
  fail("mutation fixture omitted the requested section");
}

void mutateNamedSectionFlags(std::vector<uint8_t> &Bytes, StringRef SectionName,
                             uint64_t Flags) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<mutation>"));
  if (!ObjectOrError)
    fail(toString(ObjectOrError.takeError()));
  auto *Object = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  require(Object != nullptr, "mutation fixture is not ELF64LE");
  const ELFFile<ELF64LE> &File = Object->getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    fail(toString(Sections.takeError()));
  uint64_t SectionTable = read64(Bytes, 40);
  uint16_t SectionEntrySize = read16(Bytes, 58);
  for (size_t Index = 0; Index != Sections->size(); ++Index) {
    auto Name = File.getSectionName((*Sections)[Index]);
    if (!Name)
      fail(toString(Name.takeError()));
    if (*Name != SectionName)
      continue;
    write64(Bytes, SectionTable + Index * SectionEntrySize + 8, Flags);
    return;
  }
  fail("mutation fixture omitted the requested section");
}

void corruptMetadataKey(std::vector<uint8_t> &Bytes, StringRef Key) {
  auto Position = std::search(Bytes.begin(), Bytes.end(), Key.bytes_begin(),
                              Key.bytes_end());
  require(Position != Bytes.end(), "fixture has no requested metadata key");
  *Position ^= 0x20;
}

void replaceMetadataText(std::vector<uint8_t> &Bytes, StringRef Expected,
                         StringRef Replacement) {
  require(Expected.size() == Replacement.size(),
          "metadata replacement changes the encoded length");
  auto Position = std::search(Bytes.begin(), Bytes.end(),
                              Expected.bytes_begin(), Expected.bytes_end());
  require(Position != Bytes.end(), "fixture has no requested metadata text");
  llvm::copy(Replacement, Position);
}

void replaceMetadataFieldText(std::vector<uint8_t> &Bytes, StringRef Key,
                              StringRef Expected, StringRef Replacement) {
  require(Expected.size() == Replacement.size(),
          "metadata field replacement changes the encoded length");
  auto Search = Bytes.begin();
  while (Search != Bytes.end()) {
    auto KeyPosition =
        std::search(Search, Bytes.end(), Key.bytes_begin(), Key.bytes_end());
    require(KeyPosition != Bytes.end(),
            "fixture has no requested metadata field value");
    auto ValueBegin = KeyPosition + Key.size();
    auto ValueEnd = std::min(Bytes.end(), ValueBegin + 256);
    auto Value = std::search(ValueBegin, ValueEnd, Expected.bytes_begin(),
                             Expected.bytes_end());
    if (Value != ValueEnd) {
      llvm::copy(Replacement, Value);
      return;
    }
    Search = ValueBegin;
  }
  fail("fixture has no requested metadata field value");
}

void replaceMetadataByte(std::vector<uint8_t> &Bytes, StringRef Key,
                         uint8_t Expected, uint8_t Replacement) {
  auto Position = std::search(Bytes.begin(), Bytes.end(), Key.bytes_begin(),
                              Key.bytes_end());
  require(Position != Bytes.end(), "fixture has no requested metadata key");
  auto Value = Position + Key.size();
  auto End = std::min(Bytes.end(), Value + 8);
  Value = std::find(Value, End, Expected);
  require(Value != End, "fixture metadata value has an unexpected encoding");
  *Value = Replacement;
}

struct ArgumentMetadataOverride {
  std::optional<size_t> Index;
  StringRef Field;
  std::optional<StringRef> StringValue;
  std::optional<uint64_t> UnsignedValue;
  std::optional<bool> BooleanValue;
};

ArgumentMetadataOverride stringOverride(size_t Index, StringRef Field,
                                        StringRef Value) {
  return {Index, Field, Value, std::nullopt, std::nullopt};
}

ArgumentMetadataOverride unsignedOverride(size_t Index, StringRef Field,
                                          uint64_t Value) {
  return {Index, Field, std::nullopt, Value, std::nullopt};
}

ArgumentMetadataOverride booleanOverride(size_t Index, StringRef Field,
                                         bool Value) {
  return {Index, Field, std::nullopt, std::nullopt, Value};
}

struct ExactMetadataFixtureOptions {
  StringRef OmittedKernelField;
  std::optional<size_t> OmittedArgument;
  StringRef OmittedArgumentField;
  ArgumentMetadataOverride Override;
  uint16_t OptionalHiddenMask = 0;
  std::optional<std::pair<size_t, size_t>> SwappedArguments;
  std::array<uint64_t, 3> RequiredWorkgroup = {64, 1, 1};
  StringRef UnknownRootKey;
  StringRef UnknownKernelKey;
  std::optional<size_t> UnknownArgument;
  StringRef UnknownArgumentKey;
};

std::string makeExactLdsGemmSlice1MetadataBlobWithOptions(
    const ExactMetadataFixtureOptions &Options) {
  msgpack::Document Document;
  auto StringNode = [&](StringRef Value) {
    return Document.getNode(Value, /*Copy=*/true);
  };
  auto Root = Document.getRoot().getMap(/*Convert=*/true);
  auto Version = Document.getArrayNode();
  Version.push_back(Document.getNode(uint64_t(1)));
  Version.push_back(Document.getNode(uint64_t(2)));
  Root["amdhsa.version"] = Version;
  Root["amdhsa.target"] = StringNode("amdgcn-amd-amdhsa--gfx942:xnack-");
  if (!Options.UnknownRootKey.empty())
    Root[Options.UnknownRootKey] = Document.getNode(uint64_t(1));

  auto Kernels = Document.getArrayNode();
  auto Kernel = Document.getMapNode();
  auto KernelUnsigned = [&](StringRef Name, uint64_t Value) {
    if (Options.OmittedKernelField != Name)
      Kernel[Name] = Document.getNode(Value);
  };
  auto KernelString = [&](StringRef Name, StringRef Value) {
    if (Options.OmittedKernelField != Name)
      Kernel[Name] = StringNode(Value);
  };
  KernelString(".name", "tiled_gemm_lds_v1");
  KernelString(".symbol", "tiled_gemm_lds_v1.kd");
  KernelUnsigned(".kernarg_segment_size", 304);
  KernelUnsigned(".kernarg_segment_align", 8);
  KernelUnsigned(".group_segment_fixed_size", 1024);
  KernelUnsigned(".private_segment_fixed_size", 0);
  KernelUnsigned(".wavefront_size", 64);
  KernelUnsigned(".sgpr_count", 16);
  KernelUnsigned(".vgpr_count", 16);
  KernelUnsigned(".max_flat_workgroup_size", 64);
  KernelUnsigned(".sgpr_spill_count", 0);
  KernelUnsigned(".vgpr_spill_count", 0);
  if (Options.OmittedKernelField != ".uses_dynamic_stack")
    Kernel[".uses_dynamic_stack"] = Document.getNode(false);
  if (Options.OmittedKernelField != ".reqd_workgroup_size") {
    auto Workgroup = Document.getArrayNode();
    for (uint64_t Dimension : Options.RequiredWorkgroup)
      Workgroup.push_back(Document.getNode(Dimension));
    Kernel[".reqd_workgroup_size"] = Workgroup;
  }
  if (!Options.UnknownKernelKey.empty())
    Kernel[Options.UnknownKernelKey] = Document.getNode(uint64_t(1));

  if (Options.OmittedKernelField != ".args") {
    auto Arguments = Document.getArrayNode();
    std::vector<msgpack::DocNode> ArgumentNodes;
    auto AddString = [&](msgpack::MapDocNode &Map, size_t Index, StringRef Name,
                         StringRef Value) {
      if (Options.OmittedArgument != Index ||
          Options.OmittedArgumentField != Name)
        Map[Name] = StringNode(Value);
    };
    auto AddUnsigned = [&](msgpack::MapDocNode &Map, size_t Index,
                           StringRef Name, uint64_t Value) {
      if (Options.OmittedArgument != Index ||
          Options.OmittedArgumentField != Name)
        Map[Name] = Document.getNode(Value);
    };
    auto AddBoolean = [&](msgpack::MapDocNode &Map, size_t Index,
                          StringRef Name, bool Value) {
      if (Options.OmittedArgument != Index ||
          Options.OmittedArgumentField != Name)
        Map[Name] = Document.getNode(Value);
    };
    auto ApplyOverride = [&](msgpack::MapDocNode &Map, size_t Index) {
      if (Options.Override.Index == Index) {
        if (Options.Override.StringValue)
          Map[Options.Override.Field] =
              StringNode(*Options.Override.StringValue);
        else if (Options.Override.UnsignedValue)
          Map[Options.Override.Field] =
              Document.getNode(*Options.Override.UnsignedValue);
        else if (Options.Override.BooleanValue)
          Map[Options.Override.Field] =
              Document.getNode(*Options.Override.BooleanValue);
        else
          fail("argument metadata override has no value");
      }
      if (Options.UnknownArgument == Index &&
          !Options.UnknownArgumentKey.empty())
        Map[Options.UnknownArgumentKey] = Document.getNode(uint64_t(1));
    };
    for (size_t Role = 0; Role != 3; ++Role) {
      const size_t PointerIndex = Role * 2;
      auto Pointer = Document.getMapNode();
      AddString(Pointer, PointerIndex, ".name",
                (Twine("arg") + Twine(Role) + ".data").str());
      AddString(Pointer, PointerIndex, ".type_name",
                Role < 2 ? "ushort*" : "float*");
      AddUnsigned(Pointer, PointerIndex, ".offset", Role * 16);
      AddUnsigned(Pointer, PointerIndex, ".size", 8);
      AddString(Pointer, PointerIndex, ".value_kind", "global_buffer");
      AddString(Pointer, PointerIndex, ".address_space", "global");
      AddString(Pointer, PointerIndex, ".access",
                Role < 2 ? "read_only" : "read_write");
      if (Role < 2) {
        AddString(Pointer, PointerIndex, ".actual_access", "read_only");
        AddBoolean(Pointer, PointerIndex, ".is_const", true);
      } else {
        AddBoolean(Pointer, PointerIndex, ".is_restrict", true);
      }
      ApplyOverride(Pointer, PointerIndex);
      ArgumentNodes.push_back(Pointer);

      const size_t LengthIndex = PointerIndex + 1;
      auto Length = Document.getMapNode();
      AddString(Length, LengthIndex, ".name",
                (Twine("arg") + Twine(Role) + ".len").str());
      AddString(Length, LengthIndex, ".type_name", "ulong");
      AddUnsigned(Length, LengthIndex, ".offset", Role * 16 + 8);
      AddUnsigned(Length, LengthIndex, ".size", 8);
      AddString(Length, LengthIndex, ".value_kind", "by_value");
      ApplyOverride(Length, LengthIndex);
      ArgumentNodes.push_back(Length);
    }

    struct HiddenArgument {
      uint64_t Offset;
      uint64_t Size;
      StringLiteral Kind;
    };
    static constexpr std::array<HiddenArgument, 13> Hidden = {{
        {48, 4, "hidden_block_count_x"},
        {52, 4, "hidden_block_count_y"},
        {56, 4, "hidden_block_count_z"},
        {60, 2, "hidden_group_size_x"},
        {62, 2, "hidden_group_size_y"},
        {64, 2, "hidden_group_size_z"},
        {66, 2, "hidden_remainder_x"},
        {68, 2, "hidden_remainder_y"},
        {70, 2, "hidden_remainder_z"},
        {88, 8, "hidden_global_offset_x"},
        {96, 8, "hidden_global_offset_y"},
        {104, 8, "hidden_global_offset_z"},
        {112, 2, "hidden_grid_dims"},
    }};
    for (size_t HiddenIndex = 0; HiddenIndex != Hidden.size(); ++HiddenIndex) {
      const size_t Index = 6 + HiddenIndex;
      if (Options.OmittedArgument == Index &&
          Options.OmittedArgumentField.empty())
        continue;
      auto Argument = Document.getMapNode();
      AddUnsigned(Argument, Index, ".offset", Hidden[HiddenIndex].Offset);
      AddUnsigned(Argument, Index, ".size", Hidden[HiddenIndex].Size);
      AddString(Argument, Index, ".value_kind", Hidden[HiddenIndex].Kind);
      ApplyOverride(Argument, Index);
      ArgumentNodes.push_back(Argument);
    }

    static constexpr std::array<HiddenArgument, 10> OptionalHidden = {{
        {120, 8, "hidden_printf_buffer"},
        {128, 8, "hidden_hostcall_buffer"},
        {136, 8, "hidden_multigrid_sync_arg"},
        {144, 8, "hidden_heap_v1"},
        {152, 8, "hidden_default_queue"},
        {160, 8, "hidden_completion_action"},
        {168, 4, "hidden_dynamic_lds_size"},
        {240, 4, "hidden_private_base"},
        {244, 4, "hidden_shared_base"},
        {248, 8, "hidden_queue_ptr"},
    }};
    for (size_t OptionalIndex = 0; OptionalIndex != OptionalHidden.size();
         ++OptionalIndex) {
      if ((Options.OptionalHiddenMask & (uint16_t(1) << OptionalIndex)) == 0)
        continue;
      const size_t Index = 6 + Hidden.size() + OptionalIndex;
      if (Options.OmittedArgument == Index &&
          Options.OmittedArgumentField.empty())
        continue;
      auto Argument = Document.getMapNode();
      AddUnsigned(Argument, Index, ".offset",
                  OptionalHidden[OptionalIndex].Offset);
      AddUnsigned(Argument, Index, ".size", OptionalHidden[OptionalIndex].Size);
      AddString(Argument, Index, ".value_kind",
                OptionalHidden[OptionalIndex].Kind);
      ApplyOverride(Argument, Index);
      ArgumentNodes.push_back(Argument);
    }
    if (Options.SwappedArguments) {
      const auto [Left, Right] = *Options.SwappedArguments;
      require(Left < ArgumentNodes.size() && Right < ArgumentNodes.size(),
              "metadata fixture swap index is out of range");
      std::swap(ArgumentNodes[Left], ArgumentNodes[Right]);
    }
    for (msgpack::DocNode Argument : ArgumentNodes)
      Arguments.push_back(Argument);
    Kernel[".args"] = Arguments;
  }
  Kernels.push_back(Kernel);
  Root["amdhsa.kernels"] = Kernels;

  std::string Blob;
  Document.writeToBlob(Blob);
  return Blob;
}

std::string makeExactLdsGemmSlice1MetadataBlob(
    StringRef OmittedKernelField = {},
    std::optional<size_t> OmittedArgument = std::nullopt,
    StringRef OmittedArgumentField = {},
    const ArgumentMetadataOverride &Override = {}) {
  ExactMetadataFixtureOptions Options;
  Options.OmittedKernelField = OmittedKernelField;
  Options.OmittedArgument = OmittedArgument;
  Options.OmittedArgumentField = OmittedArgumentField;
  Options.Override = Override;
  return makeExactLdsGemmSlice1MetadataBlobWithOptions(Options);
}

struct RowMetadataFixtureOptions {
  StringRef OmittedKernelField;
  StringRef KernelUnsignedOverrideField;
  uint64_t KernelUnsignedOverrideValue = 0;
  std::optional<bool> WorkgroupProcessorMode;
  std::optional<size_t> OmittedArgument;
  StringRef OmittedArgumentField;
  ArgumentMetadataOverride Override;
  std::optional<std::pair<size_t, size_t>> SwappedArguments;
  bool IncludeCompatibleExplicitOptionals = false;
};

std::string makeExactRowSoftmaxV1MetadataBlob(
    const RowMetadataFixtureOptions &Options = {}) {
  msgpack::Document Document;
  auto StringNode = [&](StringRef Value) {
    return Document.getNode(Value, /*Copy=*/true);
  };
  auto Root = Document.getRoot().getMap(/*Convert=*/true);
  auto Version = Document.getArrayNode();
  Version.push_back(Document.getNode(uint64_t(1)));
  Version.push_back(Document.getNode(uint64_t(2)));
  Root["amdhsa.version"] = Version;
  Root["amdhsa.target"] = StringNode("amdgcn-amd-amdhsa--gfx942:xnack-");

  auto Kernels = Document.getArrayNode();
  auto Kernel = Document.getMapNode();
  auto KernelUnsigned = [&](StringRef Name, uint64_t Value) {
    if (Options.OmittedKernelField == Name)
      return;
    if (Options.KernelUnsignedOverrideField == Name)
      Value = Options.KernelUnsignedOverrideValue;
    Kernel[Name] = Document.getNode(Value);
  };
  auto KernelString = [&](StringRef Name, StringRef Value) {
    if (Options.OmittedKernelField != Name)
      Kernel[Name] = StringNode(Value);
  };
  KernelString(".name", "row_softmax_v1");
  KernelString(".symbol", "row_softmax_v1.kd");
  KernelUnsigned(".kernarg_segment_size", 288);
  KernelUnsigned(".kernarg_segment_align", 8);
  KernelUnsigned(".group_segment_fixed_size", 0);
  KernelUnsigned(".private_segment_fixed_size", 0);
  KernelUnsigned(".wavefront_size", 64);
  KernelUnsigned(".sgpr_count", 42);
  KernelUnsigned(".vgpr_count", 88);
  KernelUnsigned(".agpr_count", 44);
  KernelUnsigned(".max_flat_workgroup_size", 64);
  KernelUnsigned(".sgpr_spill_count", 44);
  KernelUnsigned(".vgpr_spill_count", 28);
  if (Options.OmittedKernelField != ".uses_dynamic_stack")
    Kernel[".uses_dynamic_stack"] = Document.getNode(false);
  if (Options.WorkgroupProcessorMode)
    Kernel[".workgroup_processor_mode"] =
        Document.getNode(*Options.WorkgroupProcessorMode);
  if (Options.OmittedKernelField != ".reqd_workgroup_size") {
    auto Workgroup = Document.getArrayNode();
    for (uint64_t Dimension : std::array<uint64_t, 3>{64, 1, 1})
      Workgroup.push_back(Document.getNode(Dimension));
    Kernel[".reqd_workgroup_size"] = Workgroup;
  }
  KernelString(".language", "OpenCL C");
  if (Options.OmittedKernelField != ".language_version") {
    auto LanguageVersion = Document.getArrayNode();
    LanguageVersion.push_back(Document.getNode(uint64_t(2)));
    LanguageVersion.push_back(Document.getNode(uint64_t(0)));
    Kernel[".language_version"] = LanguageVersion;
  }

  if (Options.OmittedKernelField != ".args") {
    auto Arguments = Document.getArrayNode();
    std::vector<msgpack::DocNode> ArgumentNodes;
    auto AddString = [&](msgpack::MapDocNode &Map, size_t Index, StringRef Name,
                         StringRef Value) {
      if (Options.OmittedArgument != Index ||
          Options.OmittedArgumentField != Name)
        Map[Name] = StringNode(Value);
    };
    auto AddUnsigned = [&](msgpack::MapDocNode &Map, size_t Index,
                           StringRef Name, uint64_t Value) {
      if (Options.OmittedArgument != Index ||
          Options.OmittedArgumentField != Name)
        Map[Name] = Document.getNode(Value);
    };
    auto ApplyOverride = [&](msgpack::MapDocNode &Map, size_t Index) {
      if (Options.Override.Index != Index)
        return;
      if (Options.Override.StringValue)
        Map[Options.Override.Field] = StringNode(*Options.Override.StringValue);
      else if (Options.Override.UnsignedValue)
        Map[Options.Override.Field] =
            Document.getNode(*Options.Override.UnsignedValue);
      else if (Options.Override.BooleanValue)
        Map[Options.Override.Field] =
            Document.getNode(*Options.Override.BooleanValue);
      else
        fail("row argument metadata override has no value");
    };
    for (size_t Slice = 0; Slice != 2; ++Slice) {
      size_t PointerIndex = Slice * 2;
      auto Pointer = Document.getMapNode();
      AddString(Pointer, PointerIndex, ".name",
                (Twine("arg") + Twine(Slice) + ".data").str());
      AddUnsigned(Pointer, PointerIndex, ".offset", Slice * 16);
      AddUnsigned(Pointer, PointerIndex, ".size", 8);
      AddString(Pointer, PointerIndex, ".value_kind", "global_buffer");
      AddString(Pointer, PointerIndex, ".address_space", "global");
      if (Options.IncludeCompatibleExplicitOptionals) {
        AddUnsigned(Pointer, PointerIndex, ".align", 8);
        AddString(Pointer, PointerIndex, ".value_type", "f32");
      }
      ApplyOverride(Pointer, PointerIndex);
      ArgumentNodes.push_back(Pointer);

      size_t LengthIndex = PointerIndex + 1;
      auto Length = Document.getMapNode();
      AddString(Length, LengthIndex, ".name",
                (Twine("arg") + Twine(Slice) + ".len").str());
      AddUnsigned(Length, LengthIndex, ".offset", Slice * 16 + 8);
      AddUnsigned(Length, LengthIndex, ".size", 8);
      AddString(Length, LengthIndex, ".value_kind", "by_value");
      if (Options.IncludeCompatibleExplicitOptionals) {
        AddUnsigned(Length, LengthIndex, ".align", 8);
        AddString(Length, LengthIndex, ".value_type", "u64");
      }
      ApplyOverride(Length, LengthIndex);
      ArgumentNodes.push_back(Length);
    }

    struct HiddenArgument {
      uint64_t Offset;
      uint64_t Size;
      StringLiteral Kind;
    };
    static constexpr std::array<HiddenArgument, 19> Hidden = {{
        {32, 4, "hidden_block_count_x"},
        {36, 4, "hidden_block_count_y"},
        {40, 4, "hidden_block_count_z"},
        {44, 2, "hidden_group_size_x"},
        {46, 2, "hidden_group_size_y"},
        {48, 2, "hidden_group_size_z"},
        {50, 2, "hidden_remainder_x"},
        {52, 2, "hidden_remainder_y"},
        {54, 2, "hidden_remainder_z"},
        {72, 8, "hidden_global_offset_x"},
        {80, 8, "hidden_global_offset_y"},
        {88, 8, "hidden_global_offset_z"},
        {96, 2, "hidden_grid_dims"},
        {112, 8, "hidden_hostcall_buffer"},
        {120, 8, "hidden_multigrid_sync_arg"},
        {128, 8, "hidden_heap_v1"},
        {136, 8, "hidden_default_queue"},
        {144, 8, "hidden_completion_action"},
        {232, 8, "hidden_queue_ptr"},
    }};
    for (size_t HiddenIndex = 0; HiddenIndex != Hidden.size(); ++HiddenIndex) {
      size_t Index = 4 + HiddenIndex;
      if (Options.OmittedArgument == Index &&
          Options.OmittedArgumentField.empty())
        continue;
      auto Argument = Document.getMapNode();
      AddUnsigned(Argument, Index, ".offset", Hidden[HiddenIndex].Offset);
      AddUnsigned(Argument, Index, ".size", Hidden[HiddenIndex].Size);
      AddString(Argument, Index, ".value_kind", Hidden[HiddenIndex].Kind);
      ApplyOverride(Argument, Index);
      ArgumentNodes.push_back(Argument);
    }
    if (Options.OmittedArgument && Options.OmittedArgumentField.empty() &&
        *Options.OmittedArgument < 4)
      ArgumentNodes.erase(ArgumentNodes.begin() + *Options.OmittedArgument);
    if (Options.SwappedArguments) {
      const auto [Left, Right] = *Options.SwappedArguments;
      require(Left < ArgumentNodes.size() && Right < ArgumentNodes.size(),
              "row metadata fixture swap index is out of range");
      std::swap(ArgumentNodes[Left], ArgumentNodes[Right]);
    }
    for (msgpack::DocNode Argument : ArgumentNodes)
      Arguments.push_back(Argument);
    Kernel[".args"] = Arguments;
  }
  Kernels.push_back(Kernel);
  Root["amdhsa.kernels"] = Kernels;

  std::string Blob;
  Document.writeToBlob(Blob);
  return Blob;
}

void requireExactRowMetadataFailure(StringRef Blob, StringRef Diagnostic = {}) {
  Error Failure = validateExactRowSoftmaxV1MetadataForTesting(Blob);
  require(static_cast<bool>(Failure),
          "hostile exact row metadata was accepted");
  std::string Message = toString(std::move(Failure));
  require(Diagnostic.empty() || StringRef(Message).contains(Diagnostic),
          (Twine("exact row metadata diagnostic did not contain ") +
           Diagnostic + ": " + Message)
              .str());
}

void requireExactRowMetadataSuccess(StringRef Blob, StringRef Label) {
  if (Error Failure = validateExactRowSoftmaxV1MetadataForTesting(Blob))
    fail((Twine("compatible exact row metadata was rejected (") + Label +
          "): " + toString(std::move(Failure)))
             .str());
}

void requireExactMetadataFailure(StringRef Blob, StringRef Diagnostic) {
  Error Failure = validateExactLdsGemmSlice1MetadataForTesting(Blob);
  require(static_cast<bool>(Failure), "hostile exact metadata was accepted");
  std::string Message = toString(std::move(Failure));
  require(StringRef(Message).contains(Diagnostic),
          (Twine("exact metadata diagnostic did not contain ") + Diagnostic +
           ": " + Message)
              .str());
}

void requireExactMetadataFailure(StringRef Blob) {
  Error Failure = validateExactLdsGemmSlice1MetadataForTesting(Blob);
  require(static_cast<bool>(Failure), "hostile exact metadata was accepted");
  consumeError(std::move(Failure));
}

void requireExactMetadataSuccess(StringRef Blob, StringRef Label) {
  if (Error Failure = validateExactLdsGemmSlice1MetadataForTesting(Blob))
    fail((Twine("compatible exact metadata was rejected (") + Label +
          "): " + toString(std::move(Failure)))
             .str());
}

void requireGenericMetadataSuccess(StringRef Blob, StringRef Label) {
  if (Error Failure = validateGenericMetadataForTesting(Blob))
    fail((Twine("compatible generic metadata was rejected (") + Label +
          "): " + toString(std::move(Failure)))
             .str());
}

void writeOutput(StringRef Path, ArrayRef<uint8_t> Bytes) {
  std::error_code ErrorCode;
  raw_fd_ostream Stream(Path, ErrorCode, sys::fs::OF_None);
  require(!ErrorCode, "could not open requested HSACO fixture output");
  Stream.write(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  Stream.close();
  require(!Stream.has_error(), "could not write requested HSACO fixture");
}

bool hasObjectSymbol(ArrayRef<uint8_t> Bytes, StringRef Expected) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "ocml-output"));
  if (!ObjectOrError)
    fail(toString(ObjectOrError.takeError()));
  for (SymbolRef Symbol : (*ObjectOrError)->symbols()) {
    auto Name = Symbol.getName();
    if (!Name)
      fail(toString(Name.takeError()));
    if (*Name == Expected)
      return true;
  }
  return false;
}

struct SyntheticDeviceLibraryDirectory {
  SmallString<128> Path;

  SyntheticDeviceLibraryDirectory() {
    if (std::error_code Error =
            sys::fs::createUniqueDirectory("fe2o3-ocml-pipeline", Path))
      fail(Error.message());
  }

  ~SyntheticDeviceLibraryDirectory() {
    if (std::error_code Error = sys::fs::remove_directories(Path))
      errs() << "could not remove OCML test directory: " << Error.message()
             << '\n';
  }
};

Gfx942DeviceLibraryPolicy
makeSyntheticPolicy(SyntheticDeviceLibraryDirectory &Directory,
                    std::vector<uint8_t> Ocml) {
  static constexpr std::array<StringLiteral, 4> Names = {
      "ocml.bc",
      "oclc_isa_version_942.bc",
      "oclc_unsafe_math_off.bc",
      "oclc_finite_only_off.bc",
  };
  std::array<std::vector<uint8_t>, 4> Bytes = {
      std::move(Ocml), makeEmptyProviderBitcode(Names[1]),
      makeEmptyProviderBitcode(Names[2]), makeEmptyProviderBitcode(Names[3])};
  Gfx942DeviceLibraryPolicy Result;
  Result.Directory = Directory.Path.str().str();
  for (size_t I = 0; I < Names.size(); ++I) {
    SmallString<160> File(Result.Directory);
    sys::path::append(File, Names[I]);
    writeOutput(File, Bytes[I]);
    Result.Files.push_back(
        {Names[I].str(), SHA256::hash(Bytes[I]), MaxDeviceLibraryFileBytes});
  }
  return Result;
}

Request makeOcmlRequest(StringRef Import = "__ocml_sin_f32",
                        uint8_t CodeObjectVersion = 5) {
  return makeV2Request(makeInput(InputKind::LlvmBitcode,
                                 makeFloatConsumerBitcode("ocml_entry", Import,
                                                          CodeObjectVersion)),
                       {}, {Import.str()}, {"ocml_entry"},
                       {Import.str(), "ocml_entry"}, CodeObjectVersion);
}

Request makeOcmlKernelRequest(StringRef Import = "__ocml_sin_f32",
                              uint8_t CodeObjectVersion = 5,
                              bool TwoCalls = false) {
  return makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeOcmlKernelBitcode(Import, CodeObjectVersion, TwoCalls)),
      {}, {Import.str()}, {"fe2o3_gfx942_ocml_sin_f32_v1"},
      {Import.str(), "fe2o3_gfx942_ocml_sin_f32_v1",
       "fe2o3_gfx942_ocml_sin_f32_v1.kd"},
      CodeObjectVersion);
}
void testSyntheticOcmlPipeline() {
  SyntheticDeviceLibraryDirectory ValidDirectory;
  Gfx942DeviceLibraryPolicy ValidPolicy =
      makeSyntheticPolicy(ValidDirectory, makeSyntheticOcmlBitcode());
  Request Valid = makeOcmlRequest();
  Response Linked = runSuccessWithPolicy(Valid, ValidPolicy,
                                         {"__ocml_sin_f32", "ocml_entry"});
  requireDiagnostic(Linked,
                    "device_library.check=identity status=ok "
                    "provider=gfx942-ocml-v1 roots=[__ocml_sin_f32] files=4");
  require(hasObjectSymbol(Linked.LinkedOutput->Bytes,
                          "__fe2o3_required_ocml_helper"),
          "required OCML helper was removed from the closure");
  require(!hasObjectSymbol(Linked.LinkedOutput->Bytes, "__ocml_dead_decoy"),
          "dead OCML provider definition escaped global DCE");

  Request Cov6Exp = makeOcmlRequest("__ocml_exp_f32", 6);
  Response Cov6Linked = runSuccessWithPolicy(Cov6Exp, ValidPolicy,
                                             {"__ocml_exp_f32", "ocml_entry"});
  requireDiagnostic(Cov6Linked,
                    "device_library.check=identity status=ok "
                    "provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4");

  Response KernelLinked =
      runSuccessWithPolicy(makeOcmlKernelRequest(), ValidPolicy,
                           {"__ocml_sin_f32", "fe2o3_gfx942_ocml_sin_f32_v1",
                            "fe2o3_gfx942_ocml_sin_f32_v1.kd"});
  requireDiagnostic(KernelLinked,
                    "device_library.check=identity status=ok "
                    "provider=gfx942-ocml-v1 roots=[__ocml_sin_f32] files=4");

  const std::array<StringRef, 2> SinAndSqrt = {"__ocml_sin_f32",
                                               "__ocml_sqrt_f32"};
  Request OmittedOcml = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeFloatConsumerBitcode("ocml_entry", SinAndSqrt, {})),
      {}, {"__ocml_sin_f32"}, {"ocml_entry"}, {"__ocml_sin_f32", "ocml_entry"});
  Response OmittedOcmlFailure = requireFailureWithPolicy(
      OmittedOcml, ValidPolicy, Stage::InputValidation);
  requireDiagnostic(OmittedOcmlFailure,
                    "compiler-module import manifest mismatch: "
                    "omitted=[__ocml_sqrt_f32] extra=[]");

  Gfx942DeviceLibraryPolicy MissingPolicy = ValidPolicy;
  SmallString<160> MissingDirectory(ValidDirectory.Path);
  sys::path::append(MissingDirectory, "provider-must-not-be-opened");
  MissingPolicy.Directory = MissingDirectory.str().str();

  Request HiddenModuleAssembly = makeV2Request(
      makeInput(InputKind::LlvmTextIr,
                makeInlineAsmCompilerIr("hidden_runtime_import", true)),
      {}, {"__ocml_sin_f32"}, {"ocml_entry"}, {"__ocml_sin_f32", "ocml_entry"});
  Response HiddenModuleFailure = requireFailureWithPolicy(
      HiddenModuleAssembly, MissingPolicy, Stage::InputValidation);
  requireDiagnostic(HiddenModuleFailure,
                    "compiler-module object import manifest mismatch: "
                    "omitted=[hidden_runtime_import] extra=[]");

  Request HiddenFunctionAssembly = makeV2Request(
      makeInput(InputKind::LlvmTextIr,
                makeInlineAsmCompilerIr("__ocml_sqrt_f32", false)),
      {}, {"__ocml_sin_f32"}, {"ocml_entry"}, {"__ocml_sin_f32", "ocml_entry"});
  Response HiddenFunctionFailure = requireFailureWithPolicy(
      HiddenFunctionAssembly, MissingPolicy, Stage::InputValidation);
  requireDiagnostic(HiddenFunctionFailure,
                    "compiler-module object import manifest mismatch: "
                    "omitted=[__ocml_sqrt_f32] extra=[]");

  for (bool ModuleAssembly : {false, true}) {
    Request LocalAssembly =
        makeV2Request(makeInput(InputKind::LlvmTextIr,
                                makeInlineAsmCompilerIr("local_hidden_import",
                                                        ModuleAssembly, true)),
                      {}, {"__ocml_sin_f32"}, {"ocml_entry"},
                      {"__ocml_sin_f32", "ocml_entry"});
    Response LocalFailure = requireFailureWithPolicy(
        LocalAssembly, MissingPolicy, Stage::InputValidation);
    requireDiagnostic(LocalFailure,
                      "compiler-module object import manifest mismatch: "
                      "omitted=[local_hidden_import] extra=[]");
  }

  Request ValidIntrinsic = makeV2Request(
      makeInput(InputKind::LlvmTextIr, makeIntrinsicCompilerIr(false)), {}, {},
      {"intrinsic_entry"}, {"intrinsic_entry"});
  runSuccess(ValidIntrinsic, {"intrinsic_entry"});

  Request MalformedIntrinsic = makeV2Request(
      makeInput(InputKind::LlvmTextIr, makeIntrinsicCompilerIr(true)), {}, {},
      {"intrinsic_entry"}, {"intrinsic_entry"});
  MalformedIntrinsic.LinkOptions.VerifyEach = false;
  Response MalformedIntrinsicFailure =
      requireFailure(MalformedIntrinsic, Stage::InputValidation);
  requireDiagnostic(MalformedIntrinsicFailure,
                    "compiler-module verification failed:");

  const std::array<StringRef, 1> Sin = {"__ocml_sin_f32"};
  const std::array<StringRef, 1> UnusedCos = {"__ocml_cos_f32"};
  Request ExtraUnusedOcml = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeFloatConsumerBitcode("ocml_entry", Sin, UnusedCos)),
      {}, {"__ocml_cos_f32", "__ocml_sin_f32"}, {"ocml_entry"},
      {"__ocml_cos_f32", "__ocml_sin_f32", "ocml_entry"});
  Response ExtraUnusedFailure = requireFailureWithPolicy(
      ExtraUnusedOcml, ValidPolicy, Stage::InputValidation);
  requireDiagnostic(ExtraUnusedFailure,
                    "compiler-module import manifest mismatch: omitted=[] "
                    "extra=[__ocml_cos_f32]");

  Request OmittedOrdinary = makeV2Request(
      makeInput(
          InputKind::LlvmBitcode,
          makeBitcode("ordinary-entry", "ordinary_entry", "ordinary_helper")),
      {makeInput(
          InputKind::LlvmBitcode,
          makeBitcode("ordinary-provider", "ordinary_helper", std::nullopt))},
      {}, {"ordinary_entry"}, {"ordinary_entry", "ordinary_helper"});
  Response OmittedOrdinaryFailure =
      requireFailure(OmittedOrdinary, Stage::InputValidation);
  requireDiagnostic(
      OmittedOrdinaryFailure,
      "compiler-module import manifest mismatch: omitted=[ordinary_helper] "
      "extra=[]");

  Request OmittedWeak = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeBitcode("weak-entry", "weak_entry", "weak_helper",
                            withWeakImport())),
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("weak-provider", "weak_helper", std::nullopt))},
      {}, {"weak_entry"}, {"weak_entry", "weak_helper"});
  Response OmittedWeakFailure =
      requireFailure(OmittedWeak, Stage::InputValidation);
  requireDiagnostic(
      OmittedWeakFailure,
      "compiler-module import manifest mismatch: omitted=[weak_helper] "
      "extra=[]");

  Request CrossDefined = makeV2Request(
      makeInput(InputKind::LlvmBitcode, makeCrossDefinedCompilerBitcode()), {},
      {}, {"cross_entry", "cross_helper"}, {"cross_entry", "cross_helper"});
  runSuccess(CrossDefined, {"cross_entry", "cross_helper"});

  Request SymbolKinds = makeV2Request(
      makeInput(InputKind::LlvmTextIr, makeSymbolKindCompilerIr()), {}, {},
      {"symbol_kinds_entry"}, {"symbol_kinds_entry"});
  Response SymbolKindsFailure =
      requireFailure(SymbolKinds, Stage::InputValidation);
  requireDiagnostic(
      SymbolKindsFailure,
      "compiler-module import manifest mismatch: "
      "omitted=[ordinary_global,used_only,weak_call,weak_used] extra=[]");

  Request Unknown = makeOcmlRequest("__ocml_sqrt_f32");
  requireFailureWithPolicy(Unknown, ValidPolicy, Stage::InputValidation);
  Request WrongTarget = Valid;
  WrongTarget.Target = "gfx950";
  requireFailureWithPolicy(WrongTarget, ValidPolicy, Stage::InputValidation);
  Request WrongCodeObject = Valid;
  WrongCodeObject.CodeObjectVersion = 4;
  requireFailureWithPolicy(WrongCodeObject, ValidPolicy,
                           Stage::InputValidation);
  Request MismatchedCodeObject = Cov6Exp;
  MismatchedCodeObject.CodeObjectVersion = 5;
  requireFailureWithPolicy(MismatchedCodeObject, ValidPolicy,
                           Stage::InputValidation);

  SyntheticDeviceLibraryDirectory Cov6ProviderDirectory;
  SyntheticOcmlOptions Cov6ProviderOptions;
  Cov6ProviderOptions.CodeObjectVersion = 6;
  Gfx942DeviceLibraryPolicy Cov6ProviderPolicy = makeSyntheticPolicy(
      Cov6ProviderDirectory, makeSyntheticOcmlBitcode(Cov6ProviderOptions));
  runSuccessWithPolicy(Cov6Exp, Cov6ProviderPolicy,
                       {"__ocml_exp_f32", "ocml_entry"});
  requireFailureWithPolicy(Valid, Cov6ProviderPolicy, Stage::BitcodeLink);

  SyntheticDeviceLibraryDirectory Cov4ProviderDirectory;
  SyntheticOcmlOptions Cov4ProviderOptions;
  Cov4ProviderOptions.CodeObjectVersion = 4;
  Gfx942DeviceLibraryPolicy Cov4ProviderPolicy = makeSyntheticPolicy(
      Cov4ProviderDirectory, makeSyntheticOcmlBitcode(Cov4ProviderOptions));
  requireFailureWithPolicy(Cov6Exp, Cov4ProviderPolicy, Stage::BitcodeLink);

  Gfx942DeviceLibraryPolicy WrongDigest = ValidPolicy;
  WrongDigest.Files[0].Digest[0] ^= 0xff;
  requireFailureWithPolicy(Valid, WrongDigest, Stage::Toolchain);

  SyntheticDeviceLibraryDirectory WrongAbiDirectory;
  SyntheticOcmlOptions WrongAbiOptions;
  WrongAbiOptions.WrongAbi = true;
  Gfx942DeviceLibraryPolicy WrongAbiPolicy = makeSyntheticPolicy(
      WrongAbiDirectory, makeSyntheticOcmlBitcode(WrongAbiOptions));
  requireFailureWithPolicy(Valid, WrongAbiPolicy, Stage::BitcodeLink);

  Request WrongCompilerAbi = makeV2Request(
      makeInput(
          InputKind::LlvmBitcode,
          makeBitcode("wrong-ocml-import-abi", "ocml_entry", "__ocml_sin_f32")),
      {}, {"__ocml_sin_f32"}, {"ocml_entry"}, {"__ocml_sin_f32", "ocml_entry"});
  requireFailureWithPolicy(WrongCompilerAbi, ValidPolicy, Stage::BitcodeLink);

  SyntheticDeviceLibraryDirectory WrongTripleDirectory;
  SyntheticOcmlOptions WrongTripleOptions;
  WrongTripleOptions.WrongTriple = true;
  Gfx942DeviceLibraryPolicy WrongTriplePolicy = makeSyntheticPolicy(
      WrongTripleDirectory, makeSyntheticOcmlBitcode(WrongTripleOptions));
  requireFailureWithPolicy(Valid, WrongTriplePolicy, Stage::BitcodeLink);

  SyntheticDeviceLibraryDirectory WrongLayoutDirectory;
  SyntheticOcmlOptions WrongLayoutOptions;
  WrongLayoutOptions.Layout = LayoutMode::Incompatible;
  Gfx942DeviceLibraryPolicy WrongLayoutPolicy = makeSyntheticPolicy(
      WrongLayoutDirectory, makeSyntheticOcmlBitcode(WrongLayoutOptions));
  requireFailureWithPolicy(Valid, WrongLayoutPolicy, Stage::BitcodeLink);

  SyntheticDeviceLibraryDirectory UnresolvedDirectory;
  SyntheticOcmlOptions UnresolvedOptions;
  UnresolvedOptions.UnresolvedDependency = true;
  Gfx942DeviceLibraryPolicy UnresolvedPolicy = makeSyntheticPolicy(
      UnresolvedDirectory, makeSyntheticOcmlBitcode(UnresolvedOptions));
  requireFailureWithPolicy(Valid, UnresolvedPolicy, Stage::InputValidation);

  Request Duplicate = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeFloatConsumerBitcode("ocml_entry", "__ocml_sin_f32")),
      {makeInput(InputKind::LlvmBitcode, makeSyntheticOcmlBitcode())},
      {"__ocml_sin_f32"}, {"ocml_entry"}, {"__ocml_sin_f32", "ocml_entry"});
  requireFailureWithPolicy(Duplicate, ValidPolicy, Stage::InputValidation);
}

std::optional<std::vector<uint8_t>> testMeasuredOcmlPipeline() {
  auto Policy = measuredGfx942DeviceLibraryPolicy();
  if (!Policy) {
    consumeError(Policy.takeError());
    return std::nullopt;
  }
  Request Valid = makeOcmlRequest();

  const std::array<StringRef, 2> SinAndSqrt = {"__ocml_sin_f32",
                                               "__ocml_sqrt_f32"};
  Request OmittedOcml = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeFloatConsumerBitcode("ocml_entry", SinAndSqrt, {})),
      {}, {"__ocml_sin_f32"}, {"ocml_entry"}, {"__ocml_sin_f32", "ocml_entry"});
  Response OmittedOcmlFailure =
      requireFailure(OmittedOcml, Stage::InputValidation);
  requireDiagnostic(OmittedOcmlFailure,
                    "compiler-module import manifest mismatch: "
                    "omitted=[__ocml_sqrt_f32] extra=[]");

  Response Linked = runSuccess(Valid, {"__ocml_sin_f32", "ocml_entry"});
  requireDiagnostic(Linked,
                    "device_library.check=identity status=ok "
                    "provider=gfx942-ocml-v1 roots=[__ocml_sin_f32] files=4");
  require(!hasObjectSymbol(Linked.LinkedOutput->Bytes, "__ocml_acos_f64"),
          "unrequested measured OCML definition escaped closure reduction");

  Response Cov6Exp = runSuccess(makeOcmlRequest("__ocml_exp_f32", 6),
                                {"__ocml_exp_f32", "ocml_entry"});
  requireDiagnostic(Cov6Exp,
                    "device_library.check=identity status=ok "
                    "provider=gfx942-ocml-v1 roots=[__ocml_exp_f32] files=4");
  require(Cov6Exp.DeviceLibraryProvider.has_value() &&
              Cov6Exp.DeviceLibraryProvider->CodeObjectVersion == 6 &&
              Cov6Exp.DeviceLibraryProvider->ImportSymbols ==
                  std::vector<std::string>({"__ocml_exp_f32"}),
          "measured COV6 OCML link omitted structured provider evidence");
  Request Cov6Kernel = makeOcmlKernelRequest("__ocml_exp_f32", 6, true);
  Cov6Kernel.Target = "gfx942:xnack-";
  Cov6Kernel.LinkOptions.Optimization = OptimizationLevel::O0;
  runSuccess(Cov6Kernel,
             {"__ocml_exp_f32", "fe2o3_gfx942_ocml_sin_f32_v1",
              "fe2o3_gfx942_ocml_sin_f32_v1.kd"});

  const std::array<StringRef, 7> AllSupported = {
      "__ocml_cos_f32",   "__ocml_exp2_f32", "__ocml_exp_f32",
      "__ocml_log10_f32", "__ocml_log2_f32", "__ocml_log_f32",
      "__ocml_sin_f32"};
  std::vector<std::string> AllImports;
  std::set<std::string> AllOutputSymbols = {"ocml_all_entry"};
  for (StringRef Import : AllSupported) {
    AllImports.push_back(Import.str());
    AllOutputSymbols.insert(Import.str());
  }
  std::vector<std::string> AllFinalSymbols(AllImports);
  AllFinalSymbols.push_back("ocml_all_entry");
  Request AllSeven = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeFloatConsumerBitcode("ocml_all_entry", AllSupported, {})),
      {}, AllImports, {"ocml_all_entry"}, AllFinalSymbols);
  runSuccess(AllSeven, AllOutputSymbols);
  Response KernelLinked =
      runSuccess(makeOcmlKernelRequest(),
                 {"__ocml_sin_f32", "fe2o3_gfx942_ocml_sin_f32_v1",
                  "fe2o3_gfx942_ocml_sin_f32_v1.kd"});
  requireDiagnostic(KernelLinked,
                    "device_library.check=identity status=ok "
                    "provider=gfx942-ocml-v1 roots=[__ocml_sin_f32] files=4");
  return KernelLinked.LinkedOutput->Bytes;
}

void testExactRowSoftmaxV1Profile() {
  const std::vector<std::string> Symbols = {"__ocml_exp_f32", "row_softmax_v1",
                                            "row_softmax_v1.kd"};
  auto MakeRequest = [&](std::vector<uint8_t> CompilerBytes,
                         std::vector<std::string> RequestSymbols = {}) {
    if (RequestSymbols.empty())
      RequestSymbols = Symbols;
    Request Result = makeV2Request(
        makeInput(InputKind::LlvmTextIr, std::move(CompilerBytes)), {},
        {"__ocml_exp_f32"}, {}, RequestSymbols, 6);
    Result.Target = "gfx942:xnack-";
    Result.LinkOptions = {OptimizationLevel::O0, true, true};
    return Result;
  };
  auto RequireCompilerFailure = [](ArrayRef<uint8_t> Bytes,
                                   StringRef Diagnostic) {
    Error Failure = validateExactRowSoftmaxV1CompilerInputForTesting(Bytes);
    require(static_cast<bool>(Failure),
            "hostile exact row-softmax compiler input was accepted");
    std::string Message = toString(std::move(Failure));
    if (!StringRef(Message).contains(Diagnostic)) {
      errs() << "unexpected row-softmax compiler diagnostic: " << Message
             << '\n';
      fail("row-softmax compiler input failed for the wrong reason");
    }
  };
  auto MutateCompilerText = [](ArrayRef<uint8_t> Bytes, StringRef Expected,
                               StringRef Replacement) {
    StringRef Text(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
    std::string Mutated = replaceExactText(Text, Expected, Replacement);
    return std::vector<uint8_t>(Mutated.begin(), Mutated.end());
  };

  requireExactRowMetadataSuccess(makeExactRowSoftmaxV1MetadataBlob(),
                                 "measured LLVM 22.1.8 metadata");
  RowMetadataFixtureOptions CompatibleExplicitOptionals;
  CompatibleExplicitOptionals.IncludeCompatibleExplicitOptionals = true;
  requireExactRowMetadataSuccess(
      makeExactRowSoftmaxV1MetadataBlob(CompatibleExplicitOptionals),
      "finalizer-compatible explicit alignments and value types");

  for (StringRef Field : {
           StringRef(".args"),
           StringRef(".reqd_workgroup_size"),
           StringRef(".kernarg_segment_size"),
           StringRef(".kernarg_segment_align"),
           StringRef(".group_segment_fixed_size"),
           StringRef(".private_segment_fixed_size"),
           StringRef(".wavefront_size"),
           StringRef(".sgpr_count"),
           StringRef(".vgpr_count"),
           StringRef(".agpr_count"),
           StringRef(".max_flat_workgroup_size"),
           StringRef(".sgpr_spill_count"),
           StringRef(".vgpr_spill_count"),
           StringRef(".uses_dynamic_stack"),
           StringRef(".language"),
           StringRef(".language_version"),
       }) {
    RowMetadataFixtureOptions Options;
    Options.OmittedKernelField = Field;
    requireExactRowMetadataFailure(makeExactRowSoftmaxV1MetadataBlob(Options));
  }
  for (const auto &[Field, Value] :
       std::array<std::pair<StringRef, uint64_t>, 11>{
           {{".kernarg_segment_size", 289},
            {".kernarg_segment_align", 16},
            {".group_segment_fixed_size", 1},
            {".private_segment_fixed_size", 1},
            {".wavefront_size", 32},
            {".sgpr_count", 43},
            {".vgpr_count", 89},
            {".agpr_count", 45},
            {".max_flat_workgroup_size", 65},
            {".sgpr_spill_count", 45},
            {".vgpr_spill_count", 29}}}) {
    RowMetadataFixtureOptions Options;
    Options.KernelUnsignedOverrideField = Field;
    Options.KernelUnsignedOverrideValue = Value;
    requireExactRowMetadataFailure(makeExactRowSoftmaxV1MetadataBlob(Options));
  }
  for (bool Mode : {false, true}) {
    RowMetadataFixtureOptions Options;
    Options.WorkgroupProcessorMode = Mode;
    requireExactRowMetadataFailure(makeExactRowSoftmaxV1MetadataBlob(Options));
  }

  for (size_t Index = 0; Index != 23; ++Index) {
    RowMetadataFixtureOptions Options;
    Options.OmittedArgument = Index;
    requireExactRowMetadataFailure(makeExactRowSoftmaxV1MetadataBlob(Options));
  }
  for (size_t Index = 0; Index != 4; ++Index) {
    for (StringRef Field : {StringRef(".name"), StringRef(".offset"),
                            StringRef(".size"), StringRef(".value_kind")}) {
      RowMetadataFixtureOptions Options;
      Options.OmittedArgument = Index;
      Options.OmittedArgumentField = Field;
      requireExactRowMetadataFailure(
          makeExactRowSoftmaxV1MetadataBlob(Options));
    }
  }
  for (size_t Index : {size_t(0), size_t(2)}) {
    RowMetadataFixtureOptions MissingAddress;
    MissingAddress.OmittedArgument = Index;
    MissingAddress.OmittedArgumentField = ".address_space";
    requireExactRowMetadataFailure(
        makeExactRowSoftmaxV1MetadataBlob(MissingAddress));
    for (ArgumentMetadataOverride Override : {
             stringOverride(Index, ".name", "wrong.data"),
             stringOverride(Index, ".value_kind", "by_value"),
             stringOverride(Index, ".address_space", "local"),
             unsignedOverride(Index, ".align", 16),
             stringOverride(Index, ".value_type", "f64"),
         }) {
      RowMetadataFixtureOptions Options;
      Options.Override = Override;
      requireExactRowMetadataFailure(
          makeExactRowSoftmaxV1MetadataBlob(Options));
    }
  }
  for (size_t Index : {size_t(1), size_t(3)}) {
    for (ArgumentMetadataOverride Override : {
             stringOverride(Index, ".name", "wrong.len"),
             stringOverride(Index, ".value_kind", "global_buffer"),
             unsignedOverride(Index, ".align", 16),
             stringOverride(Index, ".value_type", "i64"),
         }) {
      RowMetadataFixtureOptions Options;
      Options.Override = Override;
      requireExactRowMetadataFailure(
          makeExactRowSoftmaxV1MetadataBlob(Options));
    }
  }
  for (size_t Index = 4; Index != 23; ++Index) {
    for (ArgumentMetadataOverride Override : {
             unsignedOverride(Index, ".offset", 280 + Index),
             unsignedOverride(Index, ".size", 1),
             stringOverride(Index, ".value_kind", "hidden_none"),
         }) {
      RowMetadataFixtureOptions Options;
      Options.Override = Override;
      requireExactRowMetadataFailure(
          makeExactRowSoftmaxV1MetadataBlob(Options));
    }
  }
  RowMetadataFixtureOptions SwappedArguments;
  SwappedArguments.SwappedArguments = std::pair<size_t, size_t>{0, 1};
  requireExactRowMetadataFailure(
      makeExactRowSoftmaxV1MetadataBlob(SwappedArguments));

  std::vector<uint8_t> Compiler = makeExactRowSoftmaxV1TextIr();
  if (Error Failure =
          validateExactRowSoftmaxV1CompilerInputForTesting(Compiler))
    fail(toString(std::move(Failure)));

  std::vector<uint8_t> WrongBody = Compiler;
  WrongBody.front() ^= 1;
  RequireCompilerFailure(WrongBody, "body identity");

  StringRef CompilerText(reinterpret_cast<const char *>(Compiler.data()),
                         Compiler.size());
  size_t FirstSection = CompilerText.find("\nmodule asm \".section ");
  require(FirstSection != StringRef::npos,
          "exact row-softmax fixture has no compiler sections");
  std::vector<uint8_t> MissingMarkers(Compiler.begin(),
                                      Compiler.begin() + FirstSection);
  RequireCompilerFailure(MissingMarkers, "missing bound sections");

  std::vector<uint8_t> MissingAuthority =
      MutateCompilerText(Compiler, ".fe2o3.row-softmax-auth.v1",
                         ".fe2o3.row-softmax-auth.missing");
  RequireCompilerFailure(MissingAuthority, "section order differs");

  std::string DuplicateText = CompilerText.str();
  size_t Transcript =
      DuplicateText.find("module asm \".section "
                         ".fe2o3.row-softmax-authority-transcript.v1");
  require(Transcript != std::string::npos,
          "exact row-softmax fixture has no authority transcript marker");
  DuplicateText.insert(
      Transcript,
      "module asm \".section "
      ".fe2o3.row-softmax-authority-transcript.v1,\\22\\22,@progbits\"\n"
      "module asm \".balign 8\"\n"
      "module asm \".byte 0x01\"\n\n");
  RequireCompilerFailure(
      std::vector<uint8_t>(DuplicateText.begin(), DuplicateText.end()),
      "section order differs");

  std::string ReorderedText = CompilerText.str();
  constexpr StringLiteral TranscriptName =
      ".fe2o3.row-softmax-authority-transcript.v1";
  constexpr StringLiteral AuthorityName = ".fe2o3.row-softmax-auth.v1";
  constexpr StringLiteral TemporaryName =
      ".fe2o3.row-softmax-temporary-authority-name.v1";
  ReorderedText =
      replaceExactText(ReorderedText, TranscriptName, TemporaryName);
  ReorderedText =
      replaceExactText(ReorderedText, AuthorityName, TranscriptName);
  ReorderedText = replaceExactText(ReorderedText, TemporaryName, AuthorityName);
  RequireCompilerFailure(
      std::vector<uint8_t>(ReorderedText.begin(), ReorderedText.end()),
      "section order differs");

  std::vector<uint8_t> ConflictingMarker =
      MutateCompilerText(Compiler, ".fe2o3.row-exp.v1", ".fe2o3.row-exp.v2");
  RequireCompilerFailure(ConflictingMarker, "section order differs");

  std::vector<uint8_t> WrongTranscriptDigest = Compiler;
  mutateExactCompilerSectionIdentity(WrongTranscriptDigest,
                                     ".fe2o3.row-softmax-auth.v1");
  RequireCompilerFailure(WrongTranscriptDigest,
                         "transcript digest is inconsistent");
  std::vector<uint8_t> WrongExp = Compiler;
  mutateExactCompilerSectionIdentity(WrongExp, ".fe2o3.row-exp.v1");
  RequireCompilerFailure(WrongExp, "exponential boundary identity");

  std::vector<uint8_t> UppercaseHex =
      MutateCompilerText(Compiler, "0xc0", "0xC0");
  RequireCompilerFailure(UppercaseHex, "byte atom is malformed");
  std::vector<uint8_t> TightSeparator =
      MutateCompilerText(Compiler, "0x52, 0x52", "0x52,0x52");
  RequireCompilerFailure(TightSeparator, "byte separator is noncanonical");
  std::vector<uint8_t> BlankBetweenSections = MutateCompilerText(
      Compiler,
      "module asm \".section .fe2o3.row-softmax-authority-transcript.v1",
      "\nmodule asm \".section .fe2o3.row-softmax-authority-transcript.v1");
  RequireCompilerFailure(BlankBetweenSections, "section order differs");
  std::string ShortChunkText = CompilerText.str();
  size_t FirstByteLine =
      ShortChunkText.find("module asm \".byte ", FirstSection);
  size_t FirstByteLineEnd = ShortChunkText.find('\n', FirstByteLine);
  require(FirstByteLine != std::string::npos &&
              FirstByteLineEnd != std::string::npos,
          "exact row-softmax fixture has no first byte line");
  size_t LastSeparator = ShortChunkText.rfind(", ", FirstByteLineEnd);
  require(LastSeparator != std::string::npos && LastSeparator > FirstByteLine,
          "exact row-softmax fixture first byte line has no separator");
  ShortChunkText.replace(LastSeparator, 2, "\"\nmodule asm \".byte ");
  RequireCompilerFailure(
      std::vector<uint8_t>(ShortChunkText.begin(), ShortChunkText.end()),
      "byte chunking is noncanonical");
  std::vector<uint8_t> MissingFinalNewline = Compiler;
  require(MissingFinalNewline.back() == '\n',
          "exact row-softmax fixture is not newline terminated");
  MissingFinalNewline.pop_back();
  RequireCompilerFailure(MissingFinalNewline, "trailing assembly");
  std::vector<uint8_t> ExtraFinalBlank = Compiler;
  ExtraFinalBlank.push_back('\n');
  RequireCompilerFailure(ExtraFinalBlank, "trailing assembly");

  Request Exact = MakeRequest(Compiler);
  Request WrongTarget = Exact;
  WrongTarget.Target = "gfx942:xnack+";
  requireFailure(WrongTarget, Stage::InputValidation);
  Request WrongCov = Exact;
  WrongCov.CodeObjectVersion = 5;
  requireFailure(WrongCov, Stage::InputValidation);
  Request WrongOptions = Exact;
  WrongOptions.LinkOptions.Optimization = OptimizationLevel::O2;
  requireFailure(WrongOptions, Stage::InputValidation);
  Request WrongImport = Exact;
  WrongImport.ImportSymbols = {"__ocml_log_f32"};
  requireFailure(WrongImport, Stage::InputValidation);
  Request WrongExport = Exact;
  WrongExport.ExportSymbols = {"row_softmax_v1"};
  requireFailure(WrongExport, Stage::InputValidation);
  Request WrongFinal = Exact;
  WrongFinal.FinalSymbols.pop_back();
  requireFailure(WrongFinal, Stage::InputValidation);

  Request CrossProfile = MakeRequest(makeExactWave64CollectivesV1TextIr());
  Response CrossProfileFailure =
      requireFailure(CrossProfile, Stage::InputValidation);
  requireDiagnostic(CrossProfileFailure,
                    "row-softmax V1 compiler module body identity");

  std::vector<uint8_t> Workgroup256 =
      MutateCompilerText(Compiler, "!0 = !{i32 64, i32 1, i32 1}",
                         "!0 = !{i32 256, i32 1, i32 1}");
  requireFailure(MakeRequest(std::move(Workgroup256)), Stage::InputValidation);
  std::vector<uint8_t> WrongWaveInput =
      MutateCompilerText(Compiler, "-wavefrontsize32,+wavefrontsize64",
                         "+wavefrontsize32,-wavefrontsize64");
  requireFailure(MakeRequest(std::move(WrongWaveInput)),
                 Stage::InputValidation);

  std::vector<uint8_t> SpoofedCompiler = MutateCompilerText(
      Compiler, "@row_softmax_v1(", "@row_softmax_v1_spoof(");
  Request Spoofed = MakeRequest(
      std::move(SpoofedCompiler),
      {"__ocml_exp_f32", "row_softmax_v1_spoof", "row_softmax_v1_spoof.kd"});
  Response SpoofedFailure = requireFailure(Spoofed, Stage::InputValidation);
  requireDiagnostic(SpoofedFailure,
                    "row-softmax V1 symbols or compiler markers");

  auto Policy = measuredGfx942DeviceLibraryPolicy();
  if (!Policy) {
    consumeError(Policy.takeError());
    return;
  }
  Response First =
      runSuccess(Exact, std::set<std::string>(Symbols.begin(), Symbols.end()));
  require(First.DeviceLibraryProvider.has_value() &&
              First.DeviceLibraryProvider->ImportSymbols ==
                  std::vector<std::string>{"__ocml_exp_f32"},
          "exact row-softmax output omitted closed OCML provider evidence");
  if (const char *Retained = std::getenv("FE2O3_TEST_RETAIN_ROW_SOFTMAX_HSACO"))
    writeOutput(Retained, First.LinkedOutput->Bytes);

  std::vector<uint8_t> WrongDescriptor = First.LinkedOutput->Bytes;
  mutateNamedSectionByte(WrongDescriptor, ".fe2o3.kd.v1");
  requireInspectionFailure(WrongDescriptor, Exact,
                           "reason=descriptor_section_identity");
  std::vector<uint8_t> ForbiddenDescriptorFlags = First.LinkedOutput->Bytes;
  mutateNamedSectionFlags(ForbiddenDescriptorFlags, ".fe2o3.kd.v1",
                          ELF::SHF_ALLOC);
  requireInspectionFailure(ForbiddenDescriptorFlags, Exact,
                           "reason=descriptor_section_envelope");
  std::vector<uint8_t> WrongWorkgroup = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWorkgroup, ".reqd_workgroup_size", 64, 32);
  requireInspectionFailure(WrongWorkgroup, Exact,
                           "kernel_contract_reqd_workgroup_size");
  std::vector<uint8_t> WrongWave = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave, ".wavefront_size", 64, 32);
  requireInspectionFailure(WrongWave, Exact, "kernel_contract_wavefront_size");
  std::vector<uint8_t> WrongKernarg = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongKernarg, ".kernarg_segment_size", 0x20, 0x21);
  requireInspectionFailure(WrongKernarg, Exact,
                           "kernel_contract_kernarg_segment_size");
  std::vector<uint8_t> WrongKernargAlign = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongKernargAlign, ".kernarg_segment_align", 8, 16);
  requireInspectionFailure(WrongKernargAlign, Exact,
                           "kernel_contract_kernarg_segment_align");
  std::vector<uint8_t> WrongMaxWorkgroup = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongMaxWorkgroup, ".max_flat_workgroup_size", 64, 32);
  requireInspectionFailure(
      WrongMaxWorkgroup, Exact,
      "required%20workgroup%20size%20exceeds%20its%20maximum");
  std::vector<uint8_t> WrongGroup = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongGroup, ".group_segment_fixed_size", 0, 1);
  requireInspectionFailure(WrongGroup, Exact,
                           "kernel_contract_group_segment_fixed_size");
  std::vector<uint8_t> WrongPrivate = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongPrivate, ".private_segment_fixed_size", 0, 1);
  requireInspectionFailure(WrongPrivate, Exact,
                           "kernel_contract_private_segment_fixed_size");
  std::vector<uint8_t> WrongSgpr = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongSgpr, ".sgpr_spill_count", 44, 45);
  requireInspectionFailure(WrongSgpr, Exact,
                           "kernel_contract_sgpr_spill_count");
  std::vector<uint8_t> WrongVgpr = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongVgpr, ".vgpr_spill_count", 28, 29);
  requireInspectionFailure(WrongVgpr, Exact,
                           "kernel_contract_vgpr_spill_count");
  std::vector<uint8_t> WrongDynamicStack = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongDynamicStack, ".uses_dynamic_stack", 0xc2, 0xc3);
  requireInspectionFailure(WrongDynamicStack, Exact,
                           "kernel_contract_uses_dynamic_stack");

  std::vector<uint8_t> Relocation = First.LinkedOutput->Bytes;
  makeStaticSymbolTableRelocationSection(Relocation);
  Error RelocationFailure =
      validateExactRowSoftmaxV1ElfClosureForTesting(Relocation);
  require(static_cast<bool>(RelocationFailure) &&
              StringRef(toString(std::move(RelocationFailure)))
                  .contains("residual_relocation_section"),
          "exact row-softmax relocation was accepted or misdiagnosed");
  std::vector<uint8_t> Dependency = First.LinkedOutput->Bytes;
  makeDynamicDependency(Dependency);
  Error DependencyFailure =
      validateExactRowSoftmaxV1ElfClosureForTesting(Dependency);
  require(static_cast<bool>(DependencyFailure) &&
              StringRef(toString(std::move(DependencyFailure)))
                  .contains("dynamic_dependency"),
          "exact row-softmax dependency was accepted or misdiagnosed");

  Request Legacy = makeOcmlKernelRequest("__ocml_exp_f32", 6, true);
  Legacy.Target = "gfx942:xnack-";
  Legacy.LinkOptions.Optimization = OptimizationLevel::O0;
  Response LegacyResponse =
      runSuccess(Legacy, {"__ocml_exp_f32", "fe2o3_gfx942_ocml_sin_f32_v1",
                          "fe2o3_gfx942_ocml_sin_f32_v1.kd"});
  require(!llvm::any_of(
              LegacyResponse.Diagnostics,
              [](const std::string &Diagnostic) {
                return StringRef(Diagnostic).contains("row_softmax_v1_profile");
              }),
          "legacy G1 request was substituted into the row-softmax profile");
}

void testExactWorkgroupSyncProfiles() {
  using Profile = ExactWorkgroupSyncProfileForTesting;
  const std::array Profiles = {Profile::LdsReduction, Profile::ScopedAtomic};
  auto CanonicalLayoutOrError = exactWorkgroupSyncDataLayoutForTesting();
  if (!CanonicalLayoutOrError)
    fail(toString(CanonicalLayoutOrError.takeError()));
  const std::string CanonicalLayout = std::move(*CanonicalLayoutOrError);

  auto BodyFor = [](Profile ProfileValue) {
    return loadIntegratedWorkgroupSyncBody(
        ProfileValue == Profile::LdsReduction
            ? "workgroup_lds_reduction_v1_profile.rs"
            : "workgroup_scoped_atomic_v1_profile.rs");
  };
  auto SymbolsFor = [](Profile ProfileValue) {
    return ProfileValue == Profile::LdsReduction
               ? std::vector<std::string>{"lds_publish_read_reduce_i32_v1",
                                          "lds_publish_read_reduce_i32_v1.kd"}
               : std::vector<std::string>{"scoped_atomic_add_u32_v1",
                                          "scoped_atomic_add_u32_v1.kd"};
  };
  auto MakeRequest = [&](Profile ProfileValue) {
    std::vector<std::string> Symbols = SymbolsFor(ProfileValue);
    Request Result =
        makeV2Request(makeInput(InputKind::LlvmTextIr,
                                makeExactWorkgroupSyncTextIr(ProfileValue)),
                      {}, {}, {}, Symbols, 6);
    Result.Target = "gfx942:xnack-";
    Result.LinkOptions = {OptimizationLevel::O2, true, true};
    return Result;
  };
  auto RequireCompilerFailure = [](ArrayRef<uint8_t> Bytes,
                                   Profile ProfileValue, StringRef Diagnostic) {
    Error Failure =
        validateExactWorkgroupSyncCompilerInputForTesting(Bytes, ProfileValue);
    require(static_cast<bool>(Failure),
            "hostile exact workgroup-sync compiler input was accepted");
    std::string Message = toString(std::move(Failure));
    require(StringRef(Message).contains(Diagnostic),
            "workgroup-sync compiler input failed for the wrong reason");
  };
  auto RequireModuleFailure = [](StringRef Text, Profile ProfileValue,
                                 StringRef Diagnostic) {
    Error Failure =
        validateExactWorkgroupSyncModuleForTesting(Text, ProfileValue);
    require(static_cast<bool>(Failure),
            "hostile exact workgroup-sync LLVM module was accepted");
    std::string Message = toString(std::move(Failure));
    if (!StringRef(Message).contains(Diagnostic)) {
      errs() << "unexpected workgroup-sync module diagnostic: " << Message
             << '\n';
      fail("workgroup-sync LLVM module failed for the wrong reason");
    }
  };

  for (Profile ProfileValue : Profiles) {
    std::string Body = BodyFor(ProfileValue);
    if (Error Failure =
            validateExactWorkgroupSyncModuleForTesting(Body, ProfileValue))
      fail(toString(std::move(Failure)));
    const std::string LayoutLine =
        (Twine("target datalayout = \"") + CanonicalLayout + "\"").str();
    RequireModuleFailure(
        replaceExactText(Body, (Twine(LayoutLine) + "\n").str(), ""),
        ProfileValue, "module envelope");
    RequireModuleFailure(
        replaceExactText(Body, CanonicalLayout,
                         "e-m:e-p:64:64-i64:64-f80:128-n8:16:32:64-S128"),
        ProfileValue, "module envelope");
    RequireModuleFailure(
        replaceExactText(Body, "p7:160:256:256:32", "p7:160:256:256:64"),
        ProfileValue, "module envelope");
    StringRef ReorderedTail(CanonicalLayout);
    require(ReorderedTail.consume_front("e-m:e-"),
            "canonical workgroup-sync data layout prefix drifted");
    std::string ReorderedLayout = (Twine("e-") + ReorderedTail + "-m:e").str();
    RequireModuleFailure(
        replaceExactText(Body, CanonicalLayout, ReorderedLayout), ProfileValue,
        "module envelope");
    RequireModuleFailure(replaceExactText(Body, "-G1-", "-G2-"), ProfileValue,
                         "module envelope");
    std::vector<uint8_t> Compiler = makeExactWorkgroupSyncTextIr(ProfileValue);
    if (Error Failure = validateExactWorkgroupSyncCompilerInputForTesting(
            Compiler, ProfileValue))
      fail(toString(std::move(Failure)));

    std::vector<uint8_t> WrongBody = Compiler;
    WrongBody.front() ^= 1;
    RequireCompilerFailure(WrongBody, ProfileValue, "body identity");

    StringRef Prefix = ProfileValue == Profile::LdsReduction
                           ? ".fe2o3.wg-lds"
                           : ".fe2o3.wg-atomic";
    for (StringRef Suffix :
         {".source.v1", ".namespace.v1", ".authority.v1", ".mir.v1",
          ".fnabi.v1", ".semantics.v1", ".terminals.v3", ".abi.v1",
          ".effects.v1", ".resources.v1", ".kir.v1"}) {
      std::vector<uint8_t> WrongIdentity = Compiler;
      mutateExactCompilerSectionIdentity(WrongIdentity,
                                         (Twine(Prefix) + Suffix).str());
      RequireCompilerFailure(WrongIdentity, ProfileValue,
                             "source/KIR/profile identity");
    }
    std::vector<uint8_t> WrongLayoutIdentity = Compiler;
    mutateExactCompilerSectionIdentity(WrongLayoutIdentity,
                                       (Twine(Prefix) + ".layout.v1").str());
    RequireCompilerFailure(WrongLayoutIdentity, ProfileValue,
                           "target-machine data-layout identity");

    Request Exact = MakeRequest(ProfileValue);
    std::vector<std::string> SymbolList = SymbolsFor(ProfileValue);
    std::set<std::string> Symbols(SymbolList.begin(), SymbolList.end());
    Response First = runSuccess(Exact, Symbols);
    Response Replay = runSuccess(Exact, Symbols);
    require(First.LinkedOutput->Bytes == Replay.LinkedOutput->Bytes,
            "exact workgroup-sync LLVM/LLD output is not reproducible");
    require(First.LinkedOutput->Digest == Replay.LinkedOutput->Digest,
            "exact workgroup-sync output identity is not reproducible");
    requireDiagnostic(First,
                      ProfileValue == Profile::LdsReduction
                          ? "workgroup_lds_reduction_v1_profile status=ok"
                          : "scoped_atomic_v1_profile status=ok");

    Request WrongTarget = Exact;
    WrongTarget.Target = "gfx942:xnack+";
    requireFailure(WrongTarget, Stage::InputValidation);
    Request WrongCov = Exact;
    WrongCov.CodeObjectVersion = 5;
    requireFailure(WrongCov, Stage::InputValidation);
    Request WrongOptions = Exact;
    WrongOptions.LinkOptions.Optimization = OptimizationLevel::O1;
    requireFailure(WrongOptions, Stage::InputValidation);
    Request WrongImports = Exact;
    WrongImports.ImportSymbols = {"host_dependency"};
    requireFailure(WrongImports, Stage::InputValidation);
    Request WrongProvider = Exact;
    WrongProvider.ExternalProviders.push_back(Exact.CompilerModule);
    WrongProvider.Inputs.push_back(Exact.CompilerModule);
    requireFailure(WrongProvider, Stage::InputValidation);
    Request WrongExports = Exact;
    WrongExports.ExportSymbols = {SymbolList.front()};
    requireFailure(WrongExports, Stage::InputValidation);
    Request WrongFinalSymbols = Exact;
    WrongFinalSymbols.FinalSymbols.pop_back();
    requireFailure(WrongFinalSymbols, Stage::InputValidation);

    std::vector<uint8_t> WrongDescriptor = First.LinkedOutput->Bytes;
    mutateNamedSectionByte(WrongDescriptor, ".fe2o3.kd.v1");
    requireInspectionFailure(WrongDescriptor, Exact,
                             "reason=descriptor_section_identity");

    std::vector<uint8_t> WrongCall = First.LinkedOutput->Bytes;
    static constexpr std::array<uint8_t, 4> SwapPcCall = {0x02, 0x1e, 0x80,
                                                          0xbe};
    overwriteStaticSymbolPrefix(WrongCall, SymbolList.front(), SwapPcCall);
    requireInspectionFailure(WrongCall, Exact, "reason=machine_call");

    std::vector<uint8_t> WrongOpcode = First.LinkedOutput->Bytes;
    static constexpr std::array<uint8_t, 4> SNopZero = {0x00, 0xbf, 0x80, 0xbf};
    overwriteStaticSymbolPrefix(WrongOpcode, SymbolList.front(), SNopZero);
    requireInspectionFailure(WrongOpcode, Exact, "reason=machine_");

    std::vector<uint8_t> WrongWorkgroup = First.LinkedOutput->Bytes;
    replaceMetadataByte(WrongWorkgroup, ".reqd_workgroup_size", 64, 32);
    requireInspectionFailure(WrongWorkgroup, Exact,
                             "kernel_contract_reqd_workgroup_size");
    std::vector<uint8_t> WrongKernarg = First.LinkedOutput->Bytes;
    replaceMetadataByte(WrongKernarg, ".kernarg_segment_size", 0x28, 0x29);
    requireInspectionFailure(WrongKernarg, Exact,
                             "kernel_contract_kernarg_segment_size");
    std::vector<uint8_t> WrongWave = First.LinkedOutput->Bytes;
    replaceMetadataByte(WrongWave, ".wavefront_size", 64, 32);
    requireInspectionFailure(WrongWave, Exact,
                             "kernel_contract_wavefront_size");
    std::vector<uint8_t> WrongGroup = First.LinkedOutput->Bytes;
    replaceMetadataByte(WrongGroup, ".group_segment_fixed_size", 0, 1);
    requireInspectionFailure(WrongGroup, Exact,
                             "kernel_contract_group_segment_fixed_size");

    std::vector<uint8_t> WrongUndefined = First.LinkedOutput->Bytes;
    makeDynamicSymbolUndefined(WrongUndefined, SymbolList.front());
    requireInspectionFailure(WrongUndefined, Exact,
                             "post_link.check=unresolved status=failed");

    std::vector<uint8_t> Relocation = First.LinkedOutput->Bytes;
    makeStaticSymbolTableRelocationSection(Relocation);
    Error RelocationFailure = validateExactWorkgroupSyncElfClosureForTesting(
        Relocation, ProfileValue);
    require(static_cast<bool>(RelocationFailure),
            "exact workgroup-sync profile accepted a relocation");
    require(StringRef(toString(std::move(RelocationFailure)))
                .contains("residual_relocation_section"),
            "workgroup-sync relocation diagnostic is missing");

    std::vector<uint8_t> Dependency = First.LinkedOutput->Bytes;
    makeDynamicDependency(Dependency);
    Error DependencyFailure = validateExactWorkgroupSyncElfClosureForTesting(
        Dependency, ProfileValue);
    require(static_cast<bool>(DependencyFailure),
            "exact workgroup-sync profile accepted a dependency");
    require(StringRef(toString(std::move(DependencyFailure)))
                .contains("dynamic_dependency"),
            "workgroup-sync dependency diagnostic is missing");

    if (ProfileValue == Profile::LdsReduction) {
      std::vector<uint8_t> WrongDynamicLds = First.LinkedOutput->Bytes;
      replaceMetadataText(WrongDynamicLds, "hidden_dynamic_lds_size",
                          "hidden_dynamic_lds_sizx");
      requireInspectionFailure(WrongDynamicLds, Exact,
                               "post_link.check=metadata status=failed");
    }
  }

  std::string LdsBody = BodyFor(Profile::LdsReduction);
  RequireModuleFailure(
      replaceExactText(LdsBody, "fence syncscope(\"workgroup\") release",
                       "fence syncscope(\"workgroup\") seq_cst"),
      Profile::LdsReduction, "fence ordering");
  RequireModuleFailure(
      replaceExactText(LdsBody, "  call void @llvm.amdgcn.s.barrier()\n", ""),
      Profile::LdsReduction, "allocation/epoch/barrier");
  std::string WrongExtent =
      replaceExactText(LdsBody, "[0 x i32]", "[64 x i32]");
  WrongExtent = replaceExactText(WrongExtent, "[0 x i32]", "[64 x i32]");
  WrongExtent = replaceExactText(WrongExtent, "[0 x i32]", "[64 x i32]");
  RequireModuleFailure(WrongExtent, Profile::LdsReduction,
                       "allocation/epoch/barrier");
  RequireModuleFailure(
      replaceExactText(
          LdsBody,
          "@__fe2o3_lds_reduction_v1_scratch = external addrspace(3) global "
          "[0 x i32], align 4",
          "@__fe2o3_lds_reduction_v1_scratch = external addrspace(3) global "
          "[0 x i32], align 8"),
      Profile::LdsReduction, "allocation/epoch/barrier");

  std::string AtomicBody = BodyFor(Profile::ScopedAtomic);
  RequireModuleFailure(
      replaceExactText(AtomicBody, "atomicrmw add", "atomicrmw xor"),
      Profile::ScopedAtomic, "operation/order/scope/address space");
  RequireModuleFailure(replaceExactText(AtomicBody, "%value monotonic, align 4",
                                        "%value acquire, align 4"),
                       Profile::ScopedAtomic,
                       "operation/order/scope/address space");
  RequireModuleFailure(
      replaceExactText(AtomicBody, "%value monotonic, align 4",
                       "%value syncscope(\"agent\") monotonic, align 4"),
      Profile::ScopedAtomic, "operation/order/scope/address space");
  std::string WrongAtomicSpace = replaceExactText(
      AtomicBody, "to ptr addrspace(1)", "to ptr addrspace(3)");
  WrongAtomicSpace =
      replaceExactText(WrongAtomicSpace, "atomicrmw add ptr addrspace(1)",
                       "atomicrmw add ptr addrspace(3)");
  RequireModuleFailure(WrongAtomicSpace, Profile::ScopedAtomic,
                       "pointer conversion");
  RequireModuleFailure(replaceExactText(AtomicBody, "%value monotonic, align 4",
                                        "%value monotonic, align 8"),
                       Profile::ScopedAtomic,
                       "operation/order/scope/address space");
}

void testExactMoeTop2V1Profile() {
  const char *FixturePath = std::getenv("FE2O3_TEST_MOE_TOP2_LLVM");
  if (!FixturePath)
    return;
  auto BufferOrError = MemoryBuffer::getFile(FixturePath, false, false);
  if (!BufferOrError)
    fail(BufferOrError.getError().message());
  StringRef Fixture = (*BufferOrError)->getBuffer();
  std::vector<uint8_t> Compiler(Fixture.bytes_begin(), Fixture.bytes_end());
  if (Error Failure = validateExactMoeTop2V1CompilerInputForTesting(Compiler))
    fail(toString(std::move(Failure)));
  if (Error Failure = validateExactMoeTop2V1ModuleForTesting(Fixture))
    fail(toString(std::move(Failure)));

  auto RequireCompilerFailure = [](ArrayRef<uint8_t> Bytes,
                                   StringRef Diagnostic) {
    Error Failure = validateExactMoeTop2V1CompilerInputForTesting(Bytes);
    require(static_cast<bool>(Failure),
            "hostile exact MoE compiler input was accepted");
    std::string Message = toString(std::move(Failure));
    require(StringRef(Message).contains(Diagnostic),
            "exact MoE compiler input failed for the wrong reason");
  };
  auto RequireModuleFailure = [](StringRef Text, StringRef Diagnostic) {
    Error Failure = validateExactMoeTop2V1ModuleForTesting(Text);
    require(static_cast<bool>(Failure),
            "hostile exact MoE LLVM module was accepted");
    std::string Message = toString(std::move(Failure));
    require(StringRef(Message).contains(Diagnostic),
            "exact MoE LLVM module failed for the wrong reason");
  };
  auto CanonicalLayoutOrError = exactWorkgroupSyncDataLayoutForTesting();
  if (!CanonicalLayoutOrError)
    fail(toString(CanonicalLayoutOrError.takeError()));
  const std::string CanonicalLayout = std::move(*CanonicalLayoutOrError);
  const std::string LayoutLine =
      (Twine("target datalayout = \"") + CanonicalLayout + "\"\n").str();
  RequireModuleFailure(replaceExactText(Fixture, LayoutLine, ""),
                       "module envelope");
  RequireModuleFailure(
      replaceExactText(Fixture, CanonicalLayout,
                       "e-m:e-p:64:64-i64:64-f80:128-n8:16:32:64-S128"),
      "module envelope");
  RequireModuleFailure(
      replaceExactText(Fixture, "p7:160:256:256:32", "p7:160:256:256:64"),
      "module envelope");
  StringRef ReorderedTail(CanonicalLayout);
  require(ReorderedTail.consume_front("e-m:e-"),
          "canonical MoE data layout prefix drifted");
  RequireModuleFailure(
      replaceExactText(Fixture, CanonicalLayout,
                       (Twine("e-") + ReorderedTail + "-m:e").str()),
      "module envelope");
  RequireModuleFailure(replaceExactText(Fixture, "-G1-", "-G2-"),
                       "module envelope");

  std::vector<uint8_t> WrongBody = Compiler;
  WrongBody.front() ^= 1;
  RequireCompilerFailure(WrongBody, "body identity");
  for (StringRef Section :
       {".fe2o3.moe.source.v1", ".fe2o3.moe.namespace.v1",
        ".fe2o3.moe.crate.v1", ".fe2o3.moe.authority.v1", ".fe2o3.moe.mir.v1",
        ".fe2o3.moe.fnabi.v1", ".fe2o3.moe.compiler.v1",
        ".fe2o3.moe.terminals.v3", ".fe2o3.moe.abi.v1", ".fe2o3.moe.effects.v1",
        ".fe2o3.moe.profile.v1", ".fe2o3.moe.routing.v1", ".fe2o3.moe.kir.v1",
        ".fe2o3.moe.descriptor.v1", ".fe2o3.moe.provider.v1"}) {
    std::vector<uint8_t> WrongIdentity = Compiler;
    mutateExactCompilerSectionIdentity(WrongIdentity, Section);
    RequireCompilerFailure(WrongIdentity,
                           "source/KIR/compiler/profile identity");
  }
  std::vector<uint8_t> WrongLayoutIdentity = Compiler;
  mutateExactCompilerSectionIdentity(WrongLayoutIdentity,
                                     ".fe2o3.moe.layout.v1");
  RequireCompilerFailure(WrongLayoutIdentity,
                         "target-machine data-layout identity");

  const std::vector<std::string> SymbolList = {
      "moe_top2_route_f32_t8_e4_k2_c4_v1",
      "moe_top2_route_f32_t8_e4_k2_c4_v1.kd"};
  const std::set<std::string> Symbols(SymbolList.begin(), SymbolList.end());
  Request Exact = makeV2Request(makeInput(InputKind::LlvmTextIr, Compiler), {},
                                {}, {}, SymbolList, 6);
  Exact.Target = "gfx942:xnack-";
  Exact.LinkOptions = {OptimizationLevel::O2, true, true};
  Response First = runSuccess(Exact, Symbols);
  Response Replay = runSuccess(Exact, Symbols);
  require(First.LinkedOutput->Bytes == Replay.LinkedOutput->Bytes &&
              First.LinkedOutput->Digest == Replay.LinkedOutput->Digest,
          "exact MoE direct LLVM/LLD output is not reproducible");
  requireDiagnostic(First, "moe_top2_t8_e4_k2_c4_v1_profile status=ok");
  if (const char *OutputPath = std::getenv("FE2O3_TEST_MOE_TOP2_HSACO"))
    writeOutput(OutputPath, First.LinkedOutput->Bytes);

  Request WrongTarget = Exact;
  WrongTarget.Target = "gfx942:xnack+";
  requireFailure(WrongTarget, Stage::InputValidation);
  Request WrongCov = Exact;
  WrongCov.CodeObjectVersion = 5;
  requireFailure(WrongCov, Stage::InputValidation);
  Request WrongOptions = Exact;
  WrongOptions.LinkOptions.Optimization = OptimizationLevel::O1;
  requireFailure(WrongOptions, Stage::InputValidation);
  Request WrongImports = Exact;
  WrongImports.ImportSymbols = {"host_dependency"};
  requireFailure(WrongImports, Stage::InputValidation);
  Request WrongProvider = Exact;
  WrongProvider.ExternalProviders.push_back(Exact.CompilerModule);
  WrongProvider.Inputs.push_back(Exact.CompilerModule);
  requireFailure(WrongProvider, Stage::InputValidation);
  Request WrongExports = Exact;
  WrongExports.ExportSymbols = {SymbolList.front()};
  requireFailure(WrongExports, Stage::InputValidation);
  Request WrongFinalSymbols = Exact;
  WrongFinalSymbols.FinalSymbols.pop_back();
  requireFailure(WrongFinalSymbols, Stage::InputValidation);
  Request WrongWorker = Exact;
  WrongWorker.WorkerBuildIdentity.push_back('x');
  requireFailure(WrongWorker, Stage::Toolchain);
  Request WrongLlvm = Exact;
  WrongLlvm.LlvmBuildIdentity.push_back('x');
  requireFailure(WrongLlvm, Stage::Toolchain);

  std::vector<uint8_t> WrongDescriptor = First.LinkedOutput->Bytes;
  mutateNamedSectionByte(WrongDescriptor, ".fe2o3.kd.v1");
  requireInspectionFailure(WrongDescriptor, Exact,
                           "reason=descriptor_section_identity");
  std::vector<uint8_t> WrongCall = First.LinkedOutput->Bytes;
  static constexpr std::array<uint8_t, 4> SwapPcCall = {0x02, 0x1e, 0x80, 0xbe};
  overwriteStaticSymbolPrefix(WrongCall, SymbolList.front(), SwapPcCall);
  requireInspectionFailure(WrongCall, Exact, "reason=machine_call");
  std::vector<uint8_t> WrongOpcode = First.LinkedOutput->Bytes;
  static constexpr std::array<uint8_t, 4> SNopZero = {0x00, 0xbf, 0x80, 0xbf};
  overwriteStaticSymbolPrefix(WrongOpcode, SymbolList.front(), SNopZero);
  requireInspectionFailure(WrongOpcode, Exact, "reason=machine_identity");

  std::vector<uint8_t> WrongWorkgroup = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWorkgroup, ".reqd_workgroup_size", 64, 32);
  requireInspectionFailure(WrongWorkgroup, Exact,
                           "kernel_contract_reqd_workgroup_size");
  std::vector<uint8_t> WrongKernarg = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongKernarg, ".kernarg_segment_size", 0x80, 0x81);
  requireInspectionFailure(WrongKernarg, Exact,
                           "kernel_contract_kernarg_segment_size");
  std::vector<uint8_t> WrongWave = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave, ".wavefront_size", 64, 32);
  requireInspectionFailure(WrongWave, Exact, "kernel_contract_wavefront_size");
  std::vector<uint8_t> WrongGroup = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongGroup, ".group_segment_fixed_size", 0, 1);
  requireInspectionFailure(WrongGroup, Exact,
                           "kernel_contract_group_segment_fixed_size");
  std::vector<uint8_t> WrongPrivate = First.LinkedOutput->Bytes;
  replaceMetadataByte(WrongPrivate, ".private_segment_fixed_size", 0, 1);
  requireInspectionFailure(WrongPrivate, Exact,
                           "kernel_contract_private_segment_fixed_size");

  std::vector<uint8_t> WrongUndefined = First.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(WrongUndefined, SymbolList.front());
  requireInspectionFailure(WrongUndefined, Exact,
                           "post_link.check=unresolved status=failed");
  std::vector<uint8_t> Relocation = First.LinkedOutput->Bytes;
  makeStaticSymbolTableRelocationSection(Relocation);
  Error RelocationFailure =
      validateExactMoeTop2V1ElfClosureForTesting(Relocation);
  require(static_cast<bool>(RelocationFailure) &&
              StringRef(toString(std::move(RelocationFailure)))
                  .contains("residual_relocation_section"),
          "exact MoE relocation was accepted or misdiagnosed");
  std::vector<uint8_t> Dependency = First.LinkedOutput->Bytes;
  makeDynamicDependency(Dependency);
  Error DependencyFailure =
      validateExactMoeTop2V1ElfClosureForTesting(Dependency);
  require(static_cast<bool>(DependencyFailure) &&
              StringRef(toString(std::move(DependencyFailure)))
                  .contains("dynamic_dependency"),
          "exact MoE dependency was accepted or misdiagnosed");
}

void testLldExitPolicy(int ExitCode) {
  pid_t Child = fork();
  require(Child >= 0, "could not fork LLD contract test");
  if (Child == 0) {
    fe2o3::worker::detail::enforceReusableLldResult({ExitCode, false});
    _exit(99);
  }
  int Status = 0;
  pid_t WaitResult;
  do {
    WaitResult = waitpid(Child, &Status, 0);
  } while (WaitResult < 0 && errno == EINTR);
  require(WaitResult == Child, "could not wait for LLD contract child");
  require(WIFEXITED(Status) && WEXITSTATUS(Status) == ExitCode,
          "non-reusable LLD result did not preserve its exit code");
}

void testExactFlashAttentionLlvmBuildIdentity() {
  constexpr StringLiteral UpstreamIdentity =
      "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
  Error Accepted =
      validateExactFlashAttentionV1LlvmBuildIdentityForTesting(UpstreamIdentity);
  require(!Accepted, "published Flash LLVM identity was rejected");

  Error Drift =
      validateExactFlashAttentionV1LlvmBuildIdentityForTesting("7.2.4");
  require(static_cast<bool>(Drift),
          "unpublished Flash LLVM identity was accepted");
  require(
      toString(std::move(Drift)) ==
          "exact FlashAttention V1 published machine identity requires LLVM "
          "build identity '" + UpstreamIdentity.str() +
          "', worker measured '7.2.4'",
      "Flash LLVM identity drift was misdiagnosed");
}

} // namespace

int main(int ArgumentCount, char **Arguments) {
  if (ArgumentCount == 2 &&
      StringRef(Arguments[1]) == "--exact-flash-llvm-identity-only") {
    testExactFlashAttentionLlvmBuildIdentity();
    return 0;
  }
  if (ArgumentCount == 2 &&
      StringRef(Arguments[1]) == "--exact-row-softmax-only") {
    testExactRowSoftmaxV1Profile();
    return 0;
  }
  require(ArgumentCount == 1 || ArgumentCount == 2 || ArgumentCount == 4 ||
              ArgumentCount == 5,
          "usage: fe2o3-worker-pipeline-tests "
          "[OUTPUT.hsaco [INPUT.bc INPUT.o [OCML_OUTPUT.hsaco]]]");

  testExactRowSoftmaxV1Profile();
  fe2o3::worker::detail::enforceReusableLldResult({0, true});
  fe2o3::worker::detail::enforceReusableLldResult({1, true});
  testLldExitPolicy(0);
  testLldExitPolicy(1);
  testExactFlashAttentionLlvmBuildIdentity();
  testSyntheticOcmlPipeline();
  testExactWorkgroupSyncProfiles();
  testExactMoeTop2V1Profile();
  std::optional<std::vector<uint8_t>> MeasuredOcmlOutput =
      testMeasuredOcmlPipeline();

  Request BitcodePair = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("bc-entry", "bc_entry", "bc_helper")),
       makeInput(InputKind::LlvmBitcode,
                 makeBitcode("bc-helper", "bc_helper", std::nullopt))},
      {"bc_entry", "bc_helper"});
  runSuccess(BitcodePair, {"bc_entry", "bc_helper"});

  Request TextPair = makeRequest(
      {makeInput(InputKind::LlvmTextIr,
                 makeTextIr("text-entry", "text_entry", "text_helper")),
       makeInput(InputKind::LlvmBitcode,
                 makeBitcode("text-helper", "text_helper", std::nullopt))},
      {"text_entry", "text_helper"});
  runSuccess(TextPair, {"text_entry", "text_helper"});

  Request InvalidText = makeRequest(
      {makeInput(InputKind::LlvmTextIr,
                 std::vector<uint8_t>{'n', 'o', 't', ' ', 'i', 'r'})},
      {});
  requireFailure(InvalidText, Stage::BitcodeLink);

  Request AbsentLayout = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("absent-layout", "absent_layout", std::nullopt,
                             withLayout(LayoutMode::Absent)))},
      {"absent_layout"});
  runSuccess(AbsentLayout, {"absent_layout"});

  Request IncompatibleLayout = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("bad-layout", "bad_layout", std::nullopt,
                             withLayout(LayoutMode::Incompatible)))},
      {"bad_layout"});
  requireFailure(IncompatibleLayout, Stage::BitcodeLink);

  Request CompatibleFeatures = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode(
                     "compatible-features", "compatible_features", std::nullopt,
                     withFunctionContract(
                         "gfx942", "-wavefrontsize32,+wavefrontsize64")))},
      {"compatible_features"});
  runSuccess(CompatibleFeatures, {"compatible_features"});

  Request WrongWavefront = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("wrong-wave", "wrong_wave", std::nullopt,
                             withFunctionFeatures(
                                 "+wavefrontsize32,-wavefrontsize64")))},
      {"wrong_wave"});
  requireFailure(WrongWavefront, Stage::BitcodeLink);

  Request WrongInstructionSet = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("wrong-isa", "wrong_isa", std::nullopt,
                             withFunctionFeatures("+gfx950-insts")))},
      {"wrong_isa"});
  requireFailure(WrongInstructionSet, Stage::BitcodeLink);

  Request WrongFunctionCpu =
      makeRequest({makeInput(InputKind::LlvmBitcode,
                             makeBitcode("wrong-cpu", "wrong_cpu", std::nullopt,
                                         withFunctionCpu("gfx950")))},
                  {"wrong_cpu"});
  requireFailure(WrongFunctionCpu, Stage::BitcodeLink);

  std::vector<uint8_t> MixedBitcode =
      makeBitcode("mixed-entry", "mixed_entry", "object_helper");
  std::vector<uint8_t> MixedObject =
      makeObject("mixed-helper", "object_helper", std::nullopt);
  Request Mixed =
      makeRequest({makeInput(InputKind::LlvmBitcode, MixedBitcode),
                   makeInput(InputKind::AmdGpuRelocatable, MixedObject)},
                  {"mixed_entry", "object_helper"});
  Response MixedFirst = runSuccess(Mixed, {"mixed_entry", "object_helper"});
  Response MixedSecond = runSuccess(Mixed, {"mixed_entry", "object_helper"});
  require(MixedFirst.LinkedOutput->Bytes == MixedSecond.LinkedOutput->Bytes,
          "identical requests produced different HSACO bytes");

  Request MixedV2 = makeV2Request(
      makeInput(InputKind::LlvmBitcode, MixedBitcode),
      {makeInput(InputKind::AmdGpuRelocatable, MixedObject)}, {"object_helper"},
      {"mixed_entry"}, {"mixed_entry", "object_helper"});
  Response MixedV2Response =
      runSuccess(MixedV2, {"mixed_entry", "object_helper"});
  require(MixedV2Response.Protocol == ProtocolVersion::V2,
          "V2 pipeline response lost its protocol version");
  require(MixedV2Response.CompilerEnvelopeIdentity ==
              MixedV2.CompilerEnvelopeIdentity,
          "V2 pipeline response lost its compiler envelope identity");

  Request TextV2 = makeV2Request(
      makeInput(InputKind::LlvmTextIr,
                makeTextIr("text-v2", "text_v2_entry", std::nullopt)),
      {}, {}, {"text_v2_entry"}, {"text_v2_entry"});
  Response TextV2Response = runSuccess(TextV2, {"text_v2_entry"});
  requireDiagnostic(TextV2Response,
                    "post_link.check=metadata status=ok kernels=0");

  Request PublicationKernel =
      makeV2Request(makeInput(InputKind::LlvmBitcode,
                              makeKernelBitcode("publication_kernel")),
                    {}, {}, {"publication_kernel"},
                    {"publication_kernel", "publication_kernel.kd"});
  PublicationKernel.Target = "gfx942:xnack-";
  Response PublicationResponse = runSuccess(
      PublicationKernel, {"publication_kernel", "publication_kernel.kd"});
  requireDiagnostic(PublicationResponse,
                    "post_link.check=target status=ok arch=gfx942");
  requireDiagnostic(PublicationResponse,
                    "post_link.check=exports status=ok "
                    "symbols=[publication_kernel,publication_kernel.kd]");
  requireDiagnostic(PublicationResponse,
                    "post_link.check=unresolved status=ok symbols=[]");
  requireDiagnostic(PublicationResponse,
                    "post_link.check=metadata status=ok kernels=1");
  requireDiagnostic(PublicationResponse,
                    "post_link.kernel name=publication_kernel "
                    "symbol=publication_kernel.kd");
  requireDiagnostic(PublicationResponse, "wavefront_size=64");
  requireDiagnostic(PublicationResponse, "max_workgroup_size=256");
  requireDiagnostic(PublicationResponse, "reqd_workgroup_size=[256,1,1]");

  auto MakeExactLdsGemmSlice1Request =
      [](uint32_t Workgroup = 64, uint32_t MaxWorkgroup = 64,
         uint32_t StaticLdsTiles = 2,
         StringRef DataLayout = ExactLdsGemmSlice1ProducerDataLayout) {
        Request Result = makeV2Request(
            makeInput(InputKind::LlvmTextIr,
                      makeExactLdsGemmSlice1TextIr(Workgroup, MaxWorkgroup,
                                                   StaticLdsTiles, DataLayout)),
            {}, {}, {}, {"tiled_gemm_lds_v1", "tiled_gemm_lds_v1.kd"}, 6);
        Result.Target = "gfx942:xnack-";
        Result.LinkOptions = {OptimizationLevel::O2, true, true};
        return Result;
      };

  std::string ExactMetadata = makeExactLdsGemmSlice1MetadataBlob();
  requireExactMetadataSuccess(ExactMetadata, "upstream LLVM 22 shape");
  auto WithOverride = [](ArgumentMetadataOverride Override) {
    return makeExactLdsGemmSlice1MetadataBlob({}, std::nullopt, {}, Override);
  };
  for (const auto &[Field, Diagnostic] :
       std::array<std::pair<StringRef, StringRef>, 5>{
           {{".sgpr_spill_count", "kernel_contract_sgpr_spill_count_missing"},
            {".vgpr_spill_count", "kernel_contract_vgpr_spill_count_missing"},
            {".uses_dynamic_stack",
             "kernel_contract_uses_dynamic_stack_missing"},
            {".args", "kernel_contract_args_missing"},
            {".reqd_workgroup_size", "kernel_contract_reqd_workgroup_size"}}})
    requireExactMetadataFailure(makeExactLdsGemmSlice1MetadataBlob(Field),
                                Diagnostic);

  for (size_t Dimension = 0; Dimension != 3; ++Dimension) {
    ExactMetadataFixtureOptions Options;
    Options.RequiredWorkgroup[Dimension] += 1;
    requireExactMetadataFailure(
        makeExactLdsGemmSlice1MetadataBlobWithOptions(Options));
  }

  for (StringRef Scope :
       {StringRef("root"), StringRef("kernel"), StringRef("argument")}) {
    ExactMetadataFixtureOptions Options;
    if (Scope == "root")
      Options.UnknownRootKey = "amdhsa.fe2o3_unknown";
    else if (Scope == "kernel")
      Options.UnknownKernelKey = ".fe2o3_unknown";
    else {
      Options.UnknownArgument = 0;
      Options.UnknownArgumentKey = ".fe2o3_unknown";
    }
    const std::string Blob =
        makeExactLdsGemmSlice1MetadataBlobWithOptions(Options);
    requireGenericMetadataSuccess(Blob,
                                  (Twine("unknown ") + Scope + " key").str());
    requireExactMetadataFailure(
        Blob, (Twine("kernel_contract_unknown_") + Scope + "_key").str());
  }

  for (const ArgumentMetadataOverride &Override :
       {unsignedOverride(1, ".offset", 0), unsignedOverride(1, ".align", 16),
        unsignedOverride(0, ".pointee_align", 3),
        unsignedOverride(18, ".offset", 304)}) {
    ExactMetadataFixtureOptions Options;
    Options.Override = Override;
    const std::string Blob =
        makeExactLdsGemmSlice1MetadataBlobWithOptions(Options);
    requireGenericMetadataSuccess(Blob, "noncanonical argument layout");
    requireExactMetadataFailure(Blob);
  }

  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 13> RequiredHidden = {{
      {48, 4, "hidden_block_count_x"},
      {52, 4, "hidden_block_count_y"},
      {56, 4, "hidden_block_count_z"},
      {60, 2, "hidden_group_size_x"},
      {62, 2, "hidden_group_size_y"},
      {64, 2, "hidden_group_size_z"},
      {66, 2, "hidden_remainder_x"},
      {68, 2, "hidden_remainder_y"},
      {70, 2, "hidden_remainder_z"},
      {88, 8, "hidden_global_offset_x"},
      {96, 8, "hidden_global_offset_y"},
      {104, 8, "hidden_global_offset_z"},
      {112, 2, "hidden_grid_dims"},
  }};
  static constexpr std::array<HiddenArgumentShape, 10> OptionalHidden = {{
      {120, 8, "hidden_printf_buffer"},
      {128, 8, "hidden_hostcall_buffer"},
      {136, 8, "hidden_multigrid_sync_arg"},
      {144, 8, "hidden_heap_v1"},
      {152, 8, "hidden_default_queue"},
      {160, 8, "hidden_completion_action"},
      {168, 4, "hidden_dynamic_lds_size"},
      {240, 4, "hidden_private_base"},
      {244, 4, "hidden_shared_base"},
      {248, 8, "hidden_queue_ptr"},
  }};

  for (size_t HiddenIndex = 0; HiddenIndex != RequiredHidden.size();
       ++HiddenIndex) {
    const size_t ArgumentIndex = 6 + HiddenIndex;
    const HiddenArgumentShape &Expected = RequiredHidden[HiddenIndex];
    for (const ArgumentMetadataOverride &Override :
         {unsignedOverride(ArgumentIndex, ".offset", Expected.Offset + 1),
          unsignedOverride(ArgumentIndex, ".size", Expected.Size + 1),
          stringOverride(ArgumentIndex, ".value_kind", "by_value")}) {
      ExactMetadataFixtureOptions Options;
      Options.Override = Override;
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlobWithOptions(Options));
    }
    requireExactMetadataFailure(
        makeExactLdsGemmSlice1MetadataBlob({}, ArgumentIndex));

    ExactMetadataFixtureOptions OrderOptions;
    const size_t OtherHiddenIndex = HiddenIndex + 1 == RequiredHidden.size()
                                        ? HiddenIndex - 1
                                        : HiddenIndex + 1;
    OrderOptions.SwappedArguments =
        std::pair<size_t, size_t>{ArgumentIndex, 6 + OtherHiddenIndex};
    requireExactMetadataFailure(
        makeExactLdsGemmSlice1MetadataBlobWithOptions(OrderOptions));
  }

  constexpr uint16_t AllOptionalHidden =
      (uint16_t(1) << OptionalHidden.size()) - 1;
  for (uint16_t Mask = 0; Mask <= AllOptionalHidden; ++Mask) {
    ExactMetadataFixtureOptions Options;
    Options.OptionalHiddenMask = Mask;
    requireExactMetadataSuccess(
        makeExactLdsGemmSlice1MetadataBlobWithOptions(Options),
        (Twine("optional hidden mask ") + Twine(Mask)).str());
  }
  for (size_t HiddenIndex = 0; HiddenIndex != OptionalHidden.size();
       ++HiddenIndex) {
    const size_t ConceptualIndex = 6 + RequiredHidden.size() + HiddenIndex;
    const HiddenArgumentShape &Expected = OptionalHidden[HiddenIndex];
    for (const ArgumentMetadataOverride &Override :
         {unsignedOverride(ConceptualIndex, ".offset", Expected.Offset + 1),
          unsignedOverride(ConceptualIndex, ".size", Expected.Size + 1),
          stringOverride(ConceptualIndex, ".value_kind", "by_value")}) {
      ExactMetadataFixtureOptions Options;
      Options.OptionalHiddenMask = uint16_t(1) << HiddenIndex;
      Options.Override = Override;
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlobWithOptions(Options));
    }

    ExactMetadataFixtureOptions OrderOptions;
    OrderOptions.OptionalHiddenMask = AllOptionalHidden;
    const size_t ActualIndex = 6 + RequiredHidden.size() + HiddenIndex;
    const size_t OtherHiddenIndex = HiddenIndex + 1 == OptionalHidden.size()
                                        ? HiddenIndex - 1
                                        : HiddenIndex + 1;
    OrderOptions.SwappedArguments = std::pair<size_t, size_t>{
        ActualIndex, 6 + RequiredHidden.size() + OtherHiddenIndex};
    requireExactMetadataFailure(
        makeExactLdsGemmSlice1MetadataBlobWithOptions(OrderOptions));
  }

  for (size_t Role = 0; Role != 3; ++Role) {
    const size_t Pointer = Role * 2;
    const size_t Length = Pointer + 1;
    for (StringRef Field :
         {StringRef(".name"), StringRef(".type_name"),
          StringRef(".address_space"), StringRef(".access")}) {
      std::string Diagnostic = (Twine("kernel_contract_arg") + Twine(Role) +
                                "_data_missing_" + Field.drop_front())
                                   .str();
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlob({}, Pointer, Field), Diagnostic);
    }
    for (StringRef Field :
         {StringRef(".offset"), StringRef(".size"), StringRef(".value_kind")})
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlob({}, Pointer, Field),
          "invalid AMDGPU metadata schema");
    if (Role < 2) {
      for (StringRef Field :
           {StringRef(".actual_access"), StringRef(".is_const")}) {
        std::string Diagnostic = (Twine("kernel_contract_arg") + Twine(Role) +
                                  "_data_missing_" + Field.drop_front())
                                     .str();
        requireExactMetadataFailure(
            makeExactLdsGemmSlice1MetadataBlob({}, Pointer, Field), Diagnostic);
      }
    } else {
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlob({}, Pointer, ".is_restrict"),
          "kernel_contract_arg2_data_missing_is_restrict");
    }

    for (StringRef Field : {StringRef(".name"), StringRef(".type_name")}) {
      std::string Diagnostic = (Twine("kernel_contract_arg") + Twine(Role) +
                                "_len_missing_" + Field.drop_front())
                                   .str();
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlob({}, Length, Field), Diagnostic);
    }
    for (StringRef Field :
         {StringRef(".offset"), StringRef(".size"), StringRef(".value_kind")})
      requireExactMetadataFailure(
          makeExactLdsGemmSlice1MetadataBlob({}, Length, Field),
          "invalid AMDGPU metadata schema");

    requireExactMetadataSuccess(
        WithOverride(unsignedOverride(Pointer, ".align", 8)),
        "canonical optional pointer alignment");
    requireExactMetadataSuccess(
        WithOverride(
            stringOverride(Pointer, ".value_type", Role < 2 ? "u16" : "f32")),
        "canonical optional pointer value type");
    requireExactMetadataSuccess(
        WithOverride(
            unsignedOverride(Pointer, ".pointee_align", Role < 2 ? 2 : 4)),
        "canonical optional pointee alignment");
    requireExactMetadataSuccess(
        WithOverride(unsignedOverride(Length, ".align", 8)),
        "canonical optional length alignment");
    requireExactMetadataSuccess(
        WithOverride(stringOverride(Length, ".value_type", "u64")),
        "canonical optional length value type");
    requireExactMetadataSuccess(
        WithOverride(booleanOverride(Length, ".is_const", false)),
        "canonical absent-or-false length const qualifier");
    requireExactMetadataSuccess(
        WithOverride(booleanOverride(Length, ".is_restrict", false)),
        "canonical absent-or-false length restrict qualifier");

    requireExactMetadataFailure(
        WithOverride(stringOverride(Pointer, ".type_name", "uint*")),
        (Twine("kernel_contract_arg") + Twine(Role) + "_data_type_name").str());
    requireExactMetadataFailure(
        WithOverride(unsignedOverride(Pointer, ".align", 16)),
        (Twine("kernel_contract_arg") + Twine(Role) + "_data_align").str());
    requireExactMetadataFailure(
        WithOverride(
            stringOverride(Pointer, ".value_type", Role < 2 ? "f32" : "u16")),
        (Twine("kernel_contract_arg") + Twine(Role) + "_data_value_type")
            .str());
    requireExactMetadataFailure(
        WithOverride(stringOverride(Pointer, ".address_space", "local")),
        (Twine("kernel_contract_arg") + Twine(Role) + "_data_address_space")
            .str());
    requireExactMetadataFailure(
        WithOverride(stringOverride(Pointer, ".access", "write_only")),
        (Twine("kernel_contract_arg") + Twine(Role) + "_data_access").str());
    requireExactMetadataFailure(
        WithOverride(
            unsignedOverride(Pointer, ".pointee_align", Role < 2 ? 4 : 2)),
        (Twine("kernel_contract_arg") + Twine(Role) + "_data_pointee_align")
            .str());
    requireExactMetadataFailure(
        WithOverride(stringOverride(Length, ".type_name", "uint")),
        (Twine("kernel_contract_arg") + Twine(Role) + "_len_type_name").str());
    requireExactMetadataFailure(
        WithOverride(unsignedOverride(Length, ".align", 4)),
        (Twine("kernel_contract_arg") + Twine(Role) + "_len_align").str());
    requireExactMetadataFailure(
        WithOverride(stringOverride(Length, ".value_type", "i64")),
        (Twine("kernel_contract_arg") + Twine(Role) + "_len_value_type").str());
    requireExactMetadataFailure(
        WithOverride(booleanOverride(Length, ".is_const", true)),
        (Twine("kernel_contract_arg") + Twine(Role) + "_len_pointer_qualifier")
            .str());

    if (Role < 2) {
      requireExactMetadataSuccess(
          WithOverride(booleanOverride(Pointer, ".is_restrict", false)),
          "canonical absent-or-false input restrict qualifier");
      requireExactMetadataFailure(
          WithOverride(stringOverride(Pointer, ".actual_access", "read_write")),
          (Twine("kernel_contract_arg") + Twine(Role) + "_data_actual_access")
              .str());
      requireExactMetadataFailure(
          WithOverride(booleanOverride(Pointer, ".is_const", false)),
          (Twine("kernel_contract_arg") + Twine(Role) + "_data_is_const")
              .str());
      requireExactMetadataFailure(
          WithOverride(booleanOverride(Pointer, ".is_restrict", true)),
          (Twine("kernel_contract_arg") + Twine(Role) + "_data_is_restrict")
              .str());
    } else {
      for (StringRef ActualAccess :
           {StringRef("read_only"), StringRef("write_only"),
            StringRef("read_write")})
        requireExactMetadataSuccess(
            WithOverride(
                stringOverride(Pointer, ".actual_access", ActualAccess)),
            "C actual access subset");
      requireExactMetadataSuccess(
          WithOverride(booleanOverride(Pointer, ".is_const", false)),
          "canonical absent-or-false output const qualifier");
      requireExactMetadataFailure(
          WithOverride(booleanOverride(Pointer, ".is_const", true)),
          "kernel_contract_arg2_data_is_const");
      requireExactMetadataFailure(
          WithOverride(booleanOverride(Pointer, ".is_restrict", false)),
          "kernel_contract_arg2_data_is_restrict");
    }
  }
  requireExactMetadataFailure(makeExactLdsGemmSlice1MetadataBlob({}, 6),
                              "kernel_contract_args_cardinality");

  Request ExactLdsGemmSlice1 = MakeExactLdsGemmSlice1Request();
  Response ExactLdsGemmSlice1Response = runSuccess(
      ExactLdsGemmSlice1, {"tiled_gemm_lds_v1", "tiled_gemm_lds_v1.kd"});
  requireDiagnostic(ExactLdsGemmSlice1Response,
                    "post_link.check=lds_gemm_slice1_profile status=ok "
                    "workgroup=[64,1,1] kernarg_size=304");

  Response WrongExactLayout =
      requireFailure(MakeExactLdsGemmSlice1Request(64, 64, 2, "e-p:32:32"),
                     Stage::InputValidation);
  requireDiagnostic(WrongExactLayout,
                    "LLVM module data layout does not match target machine");

  Response WrongExactWorkgroup = requireFailure(
      MakeExactLdsGemmSlice1Request(128, 128), Stage::OutputInspection);
  requireDiagnostic(WrongExactWorkgroup,
                    "post_link.check=lds_gemm_slice1_profile status=failed "
                    "reason=kernel_contract_reqd_workgroup_size");

  Response WrongExactLds = requireFailure(
      MakeExactLdsGemmSlice1Request(64, 64, 1), Stage::OutputInspection);
  requireDiagnostic(WrongExactLds,
                    "post_link.check=lds_gemm_slice1_profile status=failed "
                    "reason=kernel_contract_group_segment_fixed_size");

  Request WrongExactOptions = ExactLdsGemmSlice1;
  WrongExactOptions.LinkOptions.Optimization = OptimizationLevel::O0;
  Response WrongExactOptionsResponse =
      requireFailure(WrongExactOptions, Stage::OutputInspection);
  requireDiagnostic(WrongExactOptionsResponse,
                    "exact%20LDS%20GEMM%20Slice1%20symbols%20require%20the%20"
                    "closed%20Worker%20V2%20profile");

  std::vector<uint8_t> ExactLdsGemmRelocation =
      PublicationResponse.LinkedOutput->Bytes;
  makeStaticSymbolTableRelocationSection(ExactLdsGemmRelocation);
  Error RelocationFailure =
      validateExactLdsGemmSlice1ElfClosureForTesting(ExactLdsGemmRelocation);
  require(static_cast<bool>(RelocationFailure),
          "exact profile accepted a residual relocation section");
  require(StringRef(toString(std::move(RelocationFailure)))
              .contains("reason=residual_relocation_section"),
          "exact profile relocation diagnostic is missing");

  std::vector<uint8_t> ExactLdsGemmDependency =
      PublicationResponse.LinkedOutput->Bytes;
  makeDynamicDependency(ExactLdsGemmDependency);
  Error DependencyFailure =
      validateExactLdsGemmSlice1ElfClosureForTesting(ExactLdsGemmDependency);
  require(static_cast<bool>(DependencyFailure),
          "exact profile accepted a dynamic dependency");
  require(StringRef(toString(std::move(DependencyFailure)))
              .contains("reason=dynamic_dependency"),
          "exact profile dependency diagnostic is missing");

  auto MakeExactWave64CollectivesV1Request = [] {
    Request Result = makeV2Request(
        makeInput(InputKind::LlvmTextIr, makeExactWave64CollectivesV1TextIr()),
        {}, {}, {}, {"wave64_collectives_v1", "wave64_collectives_v1.kd"}, 6);
    Result.Target = "gfx942:xnack-";
    Result.LinkOptions = {OptimizationLevel::O2, true, true};
    return Result;
  };
  auto RequireWave64InputFailure = [](ArrayRef<uint8_t> Bytes,
                                      StringRef Expected) {
    Error Failure =
        validateExactWave64CollectivesV1CompilerInputForTesting(Bytes);
    require(static_cast<bool>(Failure),
            "hostile exact Wave64 compiler input was accepted");
    std::string Diagnostic = toString(std::move(Failure));
    require(StringRef(Diagnostic).contains(Expected),
            "hostile exact Wave64 input failed for the wrong reason");
  };

  std::vector<uint8_t> ExactWave64Compiler =
      makeExactWave64CollectivesV1TextIr();
  if (Error Failure = validateExactWave64CollectivesV1CompilerInputForTesting(
          ExactWave64Compiler))
    fail(toString(std::move(Failure)));
  const size_t ExactWave64BodyBytes =
      loadIntegratedWave64CollectivesV1Body().size();
  for (size_t Index = 0; Index != ExactWave64BodyBytes; ++Index) {
    ExactWave64Compiler[Index] ^= 1;
    RequireWave64InputFailure(ExactWave64Compiler, "body identity");
    ExactWave64Compiler[Index] ^= 1;
  }
  for (size_t Index = 0; Index != ExactWave64MirSha256.size(); ++Index) {
    std::array<uint8_t, 32> WrongMir = ExactWave64MirSha256;
    WrongMir[Index] ^= 1;
    RequireWave64InputFailure(
        makeExactWave64CollectivesV1TextIr({}, {}, WrongMir),
        "compiler/KIR profile identity");
    std::array<uint8_t, 32> WrongKir = ExactWave64KirSha256;
    WrongKir[Index] ^= 1;
    RequireWave64InputFailure(makeExactWave64CollectivesV1TextIr(
                                  {}, {}, ExactWave64MirSha256, WrongKir),
                              "compiler/KIR profile identity");
    std::array<uint8_t, 32> WrongProfile = ExactWave64ProfileSha256;
    WrongProfile[Index] ^= 1;
    RequireWave64InputFailure(
        makeExactWave64CollectivesV1TextIr({}, {}, ExactWave64MirSha256,
                                           ExactWave64KirSha256, WrongProfile),
        "compiler/KIR profile identity");
  }
  std::array<uint8_t, 32> ZeroAuthority{};
  RequireWave64InputFailure(
      makeExactWave64CollectivesV1TextIr({}, ZeroAuthority),
      "authority identity");

  // The worker binds descriptor transport byte-for-byte. The Rust pinned
  // handoff expectation/finalizer is intentionally the sole semantic parser.
  std::array<uint8_t, 64> AlternateDescriptor{};
  AlternateDescriptor.fill(0x4d);
  if (Error Failure = validateExactWave64CollectivesV1CompilerInputForTesting(
          makeExactWave64CollectivesV1TextIr(AlternateDescriptor)))
    fail(toString(std::move(Failure)));

  Request ExactWave64CollectivesV1 = MakeExactWave64CollectivesV1Request();
  Response ExactWave64Response =
      runSuccess(ExactWave64CollectivesV1,
                 {"wave64_collectives_v1", "wave64_collectives_v1.kd"});
  requireDiagnostic(ExactWave64Response,
                    "post_link.check=wave64_collectives_v1_profile status=ok ");
  requireDiagnostic(ExactWave64Response, "explicit_kernarg_size=72");
  requireDiagnostic(ExactWave64Response, "kernarg_size=328");
  requireDiagnostic(ExactWave64Response, "calls=0");
  requireDiagnostic(ExactWave64Response, "descriptor_binding=byte_exact");
  Response ExactWave64Replay =
      runSuccess(ExactWave64CollectivesV1,
                 {"wave64_collectives_v1", "wave64_collectives_v1.kd"});
  require(ExactWave64Response.LinkedOutput->Bytes ==
              ExactWave64Replay.LinkedOutput->Bytes,
          "exact Wave64 direct LLVM/LLD pipeline is not reproducible");

  Request WrongWave64Options = ExactWave64CollectivesV1;
  WrongWave64Options.LinkOptions.Optimization = OptimizationLevel::O1;
  Response WrongWave64OptionsResponse =
      requireFailure(WrongWave64Options, Stage::InputValidation);
  requireDiagnostic(WrongWave64OptionsResponse,
                    "exact Wave64 collectives symbols require the closed "
                    "Worker V2 profile");

  std::vector<uint8_t> WrongWave64Descriptor =
      ExactWave64Response.LinkedOutput->Bytes;
  mutateNamedSectionByte(WrongWave64Descriptor, ".fe2o3.kd.v1");
  requireInspectionFailure(
      WrongWave64Descriptor, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=descriptor_section_identity");

  std::vector<uint8_t> WrongWave64Call =
      ExactWave64Response.LinkedOutput->Bytes;
  static constexpr std::array<uint8_t, 4> SwapPcCall = {0x02, 0x1e, 0x80, 0xbe};
  overwriteStaticSymbolPrefix(WrongWave64Call, "wave64_collectives_v1",
                              SwapPcCall);
  requireInspectionFailure(
      WrongWave64Call, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=machine_call");

  std::vector<uint8_t> WrongWave64Wavefront =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64Wavefront, ".wavefront_size", 64, 32);
  requireInspectionFailure(
      WrongWave64Wavefront, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_wavefront_size");

  std::vector<uint8_t> WrongWave64Workgroup =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64Workgroup, ".reqd_workgroup_size", 64, 32);
  requireInspectionFailure(
      WrongWave64Workgroup, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_reqd_workgroup_size");

  std::vector<uint8_t> WrongWave64MaxWorkgroup =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64MaxWorkgroup, ".max_flat_workgroup_size", 64,
                      32);
  requireInspectionFailure(WrongWave64MaxWorkgroup, ExactWave64CollectivesV1,
                           "post_link.check=metadata status=failed "
                           "reason=AMDGPU%20metadata%20required%20workgroup%"
                           "20size%20exceeds%20its%20"
                           "maximum");

  std::vector<uint8_t> WrongWave64KernargSize =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64KernargSize, ".kernarg_segment_size", 0x48,
                      0x49);
  requireInspectionFailure(
      WrongWave64KernargSize, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_kernarg_segment_size");

  std::vector<uint8_t> WrongWave64KernargAlign =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64KernargAlign, ".kernarg_segment_align", 8, 16);
  requireInspectionFailure(
      WrongWave64KernargAlign, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_kernarg_segment_align");

  std::vector<uint8_t> WrongWave64GroupResource =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64GroupResource, ".group_segment_fixed_size", 0,
                      1);
  requireInspectionFailure(
      WrongWave64GroupResource, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_group_segment_fixed_size");

  std::vector<uint8_t> WrongWave64PrivateResource =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64PrivateResource, ".private_segment_fixed_size",
                      0, 1);
  requireInspectionFailure(
      WrongWave64PrivateResource, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_private_segment_fixed_size");

  std::vector<uint8_t> WrongWave64Spill =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64Spill, ".sgpr_spill_count", 0, 1);
  requireInspectionFailure(
      WrongWave64Spill, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_sgpr_spill_count");

  std::vector<uint8_t> WrongWave64VgprSpill =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64VgprSpill, ".vgpr_spill_count", 0, 1);
  requireInspectionFailure(
      WrongWave64VgprSpill, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_vgpr_spill_count");

  std::vector<uint8_t> WrongWave64DynamicStack =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64DynamicStack, ".uses_dynamic_stack", 0xc2,
                      0xc3);
  requireInspectionFailure(
      WrongWave64DynamicStack, ExactWave64CollectivesV1,
      "post_link.check=wave64_collectives_v1_profile status=failed "
      "reason=kernel_contract_uses_dynamic_stack");

  std::vector<uint8_t> WrongWave64Argument =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataByte(WrongWave64Argument, ".offset", 0, 1);
  requireInspectionFailure(
      WrongWave64Argument, ExactWave64CollectivesV1,
      "post_link.check=metadata status=failed "
      "reason=AMDGPU%20metadata%20arguments%20overlap%20or%20are%20unordered");

  std::vector<uint8_t> WrongWave64MetadataTarget =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataText(WrongWave64MetadataTarget,
                      "amdgcn-amd-amdhsa--gfx942:xnack-",
                      "amdgcn-amd-amdhsa--gfx942:xnack+");
  requireInspectionFailure(WrongWave64MetadataTarget, ExactWave64CollectivesV1,
                           "post_link.check=metadata_target status=failed");

  std::vector<uint8_t> WrongWave64MetadataSymbol =
      ExactWave64Response.LinkedOutput->Bytes;
  replaceMetadataFieldText(WrongWave64MetadataSymbol, ".symbol",
                           "wave64_collectives_v1.kd",
                           "wave64_collectives_v1.xx");
  requireInspectionFailure(
      WrongWave64MetadataSymbol, ExactWave64CollectivesV1,
      "post_link.check=metadata status=failed "
      "reason=AMDGPU%20metadata%20kernel%20descriptor%20does%20not%20match%20"
      "its%20entry%20name");

  std::vector<uint8_t> WrongWave64Flags =
      ExactWave64Response.LinkedOutput->Bytes;
  constexpr size_t Wave64Elf64FlagsOffset = 48;
  uint32_t Wave64Flags = read32(WrongWave64Flags, Wave64Elf64FlagsOffset);
  write32(WrongWave64Flags, Wave64Elf64FlagsOffset,
          Wave64Flags & ~ELF::EF_AMDGPU_MACH);
  requireInspectionFailure(WrongWave64Flags, ExactWave64CollectivesV1,
                           "post_link.check=target status=failed");

  std::vector<uint8_t> WrongWave64Undefined =
      ExactWave64Response.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(WrongWave64Undefined, "wave64_collectives_v1");
  requireInspectionFailure(WrongWave64Undefined, ExactWave64CollectivesV1,
                           "post_link.check=unresolved status=failed "
                           "symbols=[wave64_collectives_v1]");

  std::vector<uint8_t> Wave64Relocation =
      ExactWave64Response.LinkedOutput->Bytes;
  makeStaticSymbolTableRelocationSection(Wave64Relocation);
  Error Wave64RelocationFailure =
      validateExactWave64CollectivesV1ElfClosureForTesting(Wave64Relocation);
  require(static_cast<bool>(Wave64RelocationFailure),
          "Wave64 exact profile accepted a residual relocation");
  require(StringRef(toString(std::move(Wave64RelocationFailure)))
              .contains("reason=residual_relocation_section"),
          "Wave64 exact profile relocation diagnostic is missing");

  std::vector<uint8_t> Wave64Dependency =
      ExactWave64Response.LinkedOutput->Bytes;
  makeDynamicDependency(Wave64Dependency);
  Error Wave64DependencyFailure =
      validateExactWave64CollectivesV1ElfClosureForTesting(Wave64Dependency);
  require(static_cast<bool>(Wave64DependencyFailure),
          "Wave64 exact profile accepted a dynamic dependency");
  require(StringRef(toString(std::move(Wave64DependencyFailure)))
              .contains("reason=dynamic_dependency"),
          "Wave64 exact profile dependency diagnostic is missing");

  std::vector<uint8_t> ExactLdsGemmUndefined =
      PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(ExactLdsGemmUndefined, "publication_kernel");
  requireInspectionFailure(ExactLdsGemmUndefined, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[publication_kernel]");

  Input Cov6Compiler =
      makeInput(InputKind::LlvmBitcode, makeCov6TwoKernelBitcode());
  Input Cov6Helper =
      makeInput(InputKind::LlvmBitcode,
                makeBitcode("cov6-helper", "cov6_shared_helper", std::nullopt,
                            withNoInlineCodeObjectVersion(6)));
  const std::vector<std::string> Cov6FinalSymbols = {
      "cov6_alpha", "cov6_alpha.kd", "cov6_bravo", "cov6_bravo.kd",
      "cov6_shared_helper"};
  auto MakeCov6Request = [&](bool HelperFirst) {
    std::vector<Input> Inputs =
        HelperFirst ? std::vector<Input>{Cov6Helper, Cov6Compiler}
                    : std::vector<Input>{Cov6Compiler, Cov6Helper};
    Request Result =
        makeRequest(std::move(Inputs), Cov6FinalSymbols, "gfx942:xnack-", 6);
    Result.Protocol = ProtocolVersion::V2;
    Result.WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
    Result.WorkerExecutableDigest.fill(0x51);
    Result.WorkerExecutableBytes = 4096;
    Result.CompilerEnvelopeIdentity.fill(0x62);
    Result.CompilerModule = Cov6Compiler;
    Result.ExternalProviders = {Cov6Helper};
    Result.ImportSymbols = {"cov6_shared_helper"};
    Result.ExportSymbols = {"cov6_alpha", "cov6_bravo"};
    Result.FinalSymbols = Cov6FinalSymbols;
    return Result;
  };

  const std::set<std::string> Cov6ExpectedSymbols(Cov6FinalSymbols.begin(),
                                                  Cov6FinalSymbols.end());
  Request Cov6Kernels = MakeCov6Request(false);
  Response Cov6First = runSuccess(Cov6Kernels, Cov6ExpectedSymbols);
  Response Cov6Reordered =
      runSuccess(MakeCov6Request(true), Cov6ExpectedSymbols);
  require(
      Cov6First.LinkedOutput->Bytes == Cov6Reordered.LinkedOutput->Bytes,
      "equivalent producer input orderings changed canonical COV6 HSACO bytes");
  requireDiagnostic(Cov6First, "post_link.check=target status=ok arch=gfx942 "
                               "code_object_version=6");
  requireDiagnostic(Cov6First, "post_link.check=metadata status=ok kernels=2");
  requireDiagnostic(Cov6First, "post_link.kernel name=cov6_alpha "
                               "symbol=cov6_alpha.kd");
  requireDiagnostic(Cov6First, "post_link.kernel name=cov6_alpha "
                               "symbol=cov6_alpha.kd kernarg_size=272");
  requireDiagnostic(Cov6First, "post_link.kernel name=cov6_bravo "
                               "symbol=cov6_bravo.kd");
  requireDiagnostic(Cov6First, "post_link.kernel name=cov6_bravo "
                               "symbol=cov6_bravo.kd kernarg_size=272");
  require(hasObjectSymbol(Cov6First.LinkedOutput->Bytes, "cov6_shared_helper"),
          "COV6 output removed the helper shared by both kernels");

  std::vector<uint8_t> MismatchedCov6Descriptor = Cov6First.LinkedOutput->Bytes;
  replaceMetadataFieldText(MismatchedCov6Descriptor, ".symbol", "cov6_alpha.kd",
                           "cov6_omega.kd");
  requireInspectionFailure(MismatchedCov6Descriptor, Cov6Kernels,
                           "post_link.check=metadata status=failed "
                           "reason=AMDGPU%20metadata%20kernel%20descriptor%20"
                           "does%20not%20match%20its%20entry%20name");

  struct TargetFlagCase {
    const char *Target;
    uint32_t Flags;
  };
  static constexpr TargetFlagCase TargetFlagCases[] = {
      {"gfx942", 0x54c},
      {"gfx942:xnack-", 0x64c},
      {"gfx942:xnack+", 0x74c},
      {"gfx942:sramecc-:xnack-", 0xa4c},
      {"gfx942:sramecc+:xnack-", 0xe4c},
  };
  for (const TargetFlagCase &Case : TargetFlagCases) {
    Request TargetRequest = PublicationKernel;
    TargetRequest.Target = Case.Target;
    Response TargetResponse = runSuccess(
        TargetRequest, {"publication_kernel", "publication_kernel.kd"});
    require(read32(TargetResponse.LinkedOutput->Bytes, 48) == Case.Flags,
            "gfx942 target emitted unexpected ELF flags");
  }

  auto PublicationInspection = inspectLinkedOutputForPublication(
      PublicationResponse.LinkedOutput->Bytes, PublicationKernel);
  if (!PublicationInspection)
    fail(toString(PublicationInspection.takeError()));

  Request DescriptorOmitted = PublicationKernel;
  DescriptorOmitted.ExpectedDefinedSymbols = {"publication_kernel"};
  requireInspectionFailure(PublicationResponse.LinkedOutput->Bytes,
                           DescriptorOmitted,
                           "post_link.check=exports status=failed");

  std::vector<uint8_t> WrongOutputTarget =
      PublicationResponse.LinkedOutput->Bytes;
  constexpr size_t Elf64FlagsOffset = 48;
  uint32_t Flags = read32(WrongOutputTarget, Elf64FlagsOffset);
  write32(WrongOutputTarget, Elf64FlagsOffset, Flags & ~ELF::EF_AMDGPU_MACH);
  requireInspectionFailure(WrongOutputTarget, PublicationKernel,
                           "post_link.check=target status=failed");

  std::vector<uint8_t> WrongXnack = PublicationResponse.LinkedOutput->Bytes;
  write32(WrongXnack, Elf64FlagsOffset,
          (Flags & ~ELF::EF_AMDGPU_FEATURE_XNACK_V4) |
              ELF::EF_AMDGPU_FEATURE_XNACK_ANY_V4);
  requireInspectionFailure(
      WrongXnack, PublicationKernel,
      "post_link.check=target status=failed "
      "reason=e_flags%20expected%3D0x64c%20actual%3D0x54c");

  std::vector<uint8_t> WrongSramEcc = PublicationResponse.LinkedOutput->Bytes;
  write32(WrongSramEcc, Elf64FlagsOffset,
          (Flags & ~ELF::EF_AMDGPU_FEATURE_SRAMECC_V4) |
              ELF::EF_AMDGPU_FEATURE_SRAMECC_OFF_V4);
  requireInspectionFailure(
      WrongSramEcc, PublicationKernel,
      "post_link.check=target status=failed "
      "reason=e_flags%20expected%3D0x64c%20actual%3D0xa4c");

  std::vector<uint8_t> WrongMetadataTarget =
      PublicationResponse.LinkedOutput->Bytes;
  replaceMetadataText(WrongMetadataTarget, "amdgcn-amd-amdhsa--gfx942:xnack-",
                      "amdgcn-amd-amdhsa--gfx942:xnack+");
  requireInspectionFailure(WrongMetadataTarget, PublicationKernel,
                           "post_link.check=metadata_target status=failed");

  std::vector<uint8_t> UndefinedOutput =
      PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(UndefinedOutput, "publication_kernel");
  requireInspectionFailure(UndefinedOutput, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[publication_kernel]");

  std::vector<uint8_t> RuntimeUndefinedOutput =
      PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(RuntimeUndefinedOutput, "publication_kernel",
                             "__ockl_bad");
  requireInspectionFailure(RuntimeUndefinedOutput, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[__ockl_bad]");

  RuntimeUndefinedOutput = PublicationResponse.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(RuntimeUndefinedOutput, "publication_kernel",
                             "__ocml_bad");
  requireInspectionFailure(RuntimeUndefinedOutput, PublicationKernel,
                           "post_link.check=unresolved status=failed "
                           "symbols=[__ocml_bad]");

  std::vector<uint8_t> InvalidMetadata =
      PublicationResponse.LinkedOutput->Bytes;
  corruptMetadataKey(InvalidMetadata, ".wavefront_size");
  requireInspectionFailure(InvalidMetadata, PublicationKernel,
                           "post_link.check=metadata status=failed "
                           "reason=linked%20output%20has%20invalid%20AMDGPU%20"
                           "metadata%20schema");

  Request WrongRequiredWorkgroup = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeKernelBitcode("wrong_required_workgroup",
                                  std::array<uint32_t, 3>{128, 1, 1})),
      {}, {}, {"wrong_required_workgroup"},
      {"wrong_required_workgroup", "wrong_required_workgroup.kd"});
  Response WrongRequiredFailure =
      requireFailure(WrongRequiredWorkgroup, Stage::OutputInspection);
  requireDiagnostic(WrongRequiredFailure,
                    "post_link.check=g1_profile status=failed "
                    "kernel=wrong_required_workgroup "
                    "field=reqd_workgroup_size expected=[256,1,1]");

  Request MissingRequiredWorkgroup = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeKernelBitcode("missing_required_workgroup", std::nullopt)),
      {}, {}, {"missing_required_workgroup"},
      {"missing_required_workgroup", "missing_required_workgroup.kd"});
  Response MissingRequiredFailure =
      requireFailure(MissingRequiredWorkgroup, Stage::OutputInspection);
  requireDiagnostic(MissingRequiredFailure,
                    "post_link.check=g1_profile status=failed "
                    "kernel=missing_required_workgroup "
                    "field=reqd_workgroup_size expected=[256,1,1]");

  Request WrongMaxWorkgroup = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeKernelBitcode("wrong_max_workgroup",
                                  std::array<uint32_t, 3>{256, 1, 1}, 512)),
      {}, {}, {"wrong_max_workgroup"},
      {"wrong_max_workgroup", "wrong_max_workgroup.kd"});
  Response WrongMaxFailure =
      requireFailure(WrongMaxWorkgroup, Stage::OutputInspection);
  requireDiagnostic(WrongMaxFailure,
                    "post_link.check=g1_profile status=failed "
                    "kernel=wrong_max_workgroup "
                    "field=max_flat_workgroup_size expected=256 actual=512");

  std::vector<uint8_t> WrongOutputWavefront =
      PublicationResponse.LinkedOutput->Bytes;
  replaceMetadataByte(WrongOutputWavefront, ".wavefront_size", 64, 32);
  requireInspectionFailure(WrongOutputWavefront, PublicationKernel,
                           "post_link.check=g1_profile status=failed "
                           "kernel=publication_kernel field=wavefront_size "
                           "expected=64 actual=32");

  Request WrongV2Worker = MixedV2;
  WrongV2Worker.WorkerBuildIdentity = "wrong-worker";
  requireFailure(WrongV2Worker, Stage::Toolchain);

  Request SameCardinalitySubstitution = MixedV2;
  SameCardinalitySubstitution.ImportSymbols = {"substituted_helper"};
  requireFailure(SameCardinalitySubstitution, Stage::InputValidation);

  Request SwappedRoles = MixedV2;
  std::swap(SwappedRoles.CompilerModule,
            SwappedRoles.ExternalProviders.front());
  requireFailure(SwappedRoles, Stage::InputValidation);

  Request ImportedSymbolDefinedByModule = MixedV2;
  ImportedSymbolDefinedByModule.ImportSymbols = {"mixed_entry"};
  ImportedSymbolDefinedByModule.ExportSymbols.clear();
  requireFailure(ImportedSymbolDefinedByModule, Stage::InputValidation);

  Request ExportDefinedOnlyByProvider = MixedV2;
  ExportDefinedOnlyByProvider.ImportSymbols.clear();
  ExportDefinedOnlyByProvider.ExportSymbols = {"object_helper"};
  requireFailure(ExportDefinedOnlyByProvider, Stage::InputValidation);

  Request V2Duplicate = makeV2Request(
      makeInput(InputKind::LlvmBitcode,
                makeBitcode("v2-duplicate-module", "v2_duplicate", std::nullopt,
                            withAddend(1))),
      {makeInput(InputKind::AmdGpuRelocatable,
                 makeObject("v2-duplicate-provider", "v2_duplicate",
                            std::nullopt, withAddend(2)))},
      {}, {"v2_duplicate"}, {"v2_duplicate"});
  requireFailure(V2Duplicate, Stage::InputValidation);
  if (ArgumentCount >= 2)
    writeOutput(Arguments[1], MixedFirst.LinkedOutput->Bytes);
  if (ArgumentCount == 4) {
    writeOutput(Arguments[2], MixedBitcode);
    writeOutput(Arguments[3], MixedObject);
  }
  if (ArgumentCount == 5) {
    require(MeasuredOcmlOutput.has_value(),
            "measured gfx942 OCML providers are unavailable");
    writeOutput(Arguments[2], MixedBitcode);
    writeOutput(Arguments[3], MixedObject);
    writeOutput(Arguments[4], *MeasuredOcmlOutput);
  }

  Request ObjectPair = makeRequest(
      {makeInput(InputKind::AmdGpuRelocatable,
                 makeObject("object-entry", "object_entry", "object_leaf")),
       makeInput(InputKind::AmdGpuRelocatable,
                 makeObject("object-leaf", "object_leaf", std::nullopt))},
      {"object_entry", "object_leaf"});
  runSuccess(ObjectPair, {"object_entry", "object_leaf"});

  Request ObjectAsBitcode = makeRequest(
      {makeInput(InputKind::LlvmBitcode, MixedObject)}, {"object_helper"});
  requireFailure(ObjectAsBitcode, Stage::InputValidation);

  Request BitcodeAsObject = makeRequest(
      {makeInput(InputKind::AmdGpuRelocatable, MixedBitcode)}, {"mixed_entry"});
  requireFailure(BitcodeAsObject, Stage::InputValidation);

  Request WrongTarget =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("wrong-target", "wrong_target",
                                        std::nullopt, withCpu("gfx1151")))},
                  {"wrong_target"});
  requireFailure(WrongTarget, Stage::InputValidation);

  Request WrongCodeObject =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("wrong-cov", "wrong_cov", std::nullopt,
                                        withCodeObjectVersion(4)))},
                  {"wrong_cov"});
  requireFailure(WrongCodeObject, Stage::InputValidation);

  Request WrongBitcodeCodeObject = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("wrong-bitcode-cov", "wrong_bitcode_cov",
                             std::nullopt, withCodeObjectVersion(4)))},
      {"wrong_bitcode_cov"});
  requireFailure(WrongBitcodeCodeObject, Stage::BitcodeLink);

  Request Unresolved = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("unresolved", "unresolved_entry", "missing"))},
      {"unresolved_entry"});
  requireFailure(Unresolved, Stage::InputValidation);

  Request UnresolvedWeak = makeRequest(
      {makeInput(InputKind::LlvmBitcode,
                 makeBitcode("unresolved-weak", "unresolved_weak_entry",
                             "missing_weak", withWeakImport()))},
      {"unresolved_weak_entry"});
  requireFailure(UnresolvedWeak, Stage::InputValidation);

  Request Duplicate =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-a", "duplicate",
                                        std::nullopt, withAddend(1))),
                   makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-b", "duplicate",
                                        std::nullopt, withAddend(2)))},
                  {"duplicate"});
  requireFailure(Duplicate, Stage::InputValidation);

  Request DuplicateWeak =
      makeRequest({makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-weak-a", "duplicate_weak",
                                        std::nullopt, withWeakDefinition(1))),
                   makeInput(InputKind::AmdGpuRelocatable,
                             makeObject("duplicate-weak-b", "duplicate_weak",
                                        std::nullopt, withWeakDefinition(2)))},
                  {"duplicate_weak"});
  requireFailure(DuplicateWeak, Stage::InputValidation);

  Request OutputTooSmall = Mixed;
  OutputTooSmall.MaxOutputBytes = 1;
  requireFailure(OutputTooSmall, Stage::NativeLink);

  Request MissingExport = Mixed;
  MissingExport.ExpectedDefinedSymbols.push_back("phantom_export");
  llvm::sort(MissingExport.ExpectedDefinedSymbols);
  requireFailure(MissingExport, Stage::InputValidation);
  return 0;
}
