#include "WorkerLldPolicy.h"
#include "WorkerPipeline.h"
#include "WorkerProtocol.h"

#include "llvm/ADT/SmallVector.h"
#include "llvm/BinaryFormat/ELF.h"
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

std::vector<uint8_t> makeOcmlKernelBitcode() {
  LLVMContext Context;
  Module ModuleValue("gfx942-ocml-sin-kernel", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine->createDataLayout());
  ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version", 500);

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
  FunctionCallee Sin = ModuleValue.getOrInsertFunction(
      "__ocml_sin_f32", FunctionType::get(F32, {F32}, false));

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
  Value *Result = Builder.CreateCall(Sin, {InputValue});
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

std::vector<uint8_t> makeExactLdsGemmSlice1TextIr(uint32_t Workgroup = 64,
                                                  uint32_t MaxWorkgroup = 64,
                                                  uint32_t StaticLdsTiles = 2) {
  LLVMContext Context;
  Module ModuleValue("exact-lds-gemm-slice1", Context);
  std::unique_ptr<TargetMachine> Machine = createMachine("gfx942");
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine->createDataLayout());
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

  auto Argument = Kernel->arg_begin();
  Value *A = &*Argument++;
  Value *ALength = &*Argument++;
  Value *B = &*Argument++;
  Value *BLength = &*Argument++;
  Value *C = &*Argument++;
  Value *CLength = &*Argument;
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

Request makeOcmlKernelRequest() {
  return makeV2Request(
      makeInput(InputKind::LlvmBitcode, makeOcmlKernelBitcode()), {},
      {"__ocml_sin_f32"}, {"fe2o3_gfx942_ocml_sin_f32_v1"},
      {"__ocml_sin_f32", "fe2o3_gfx942_ocml_sin_f32_v1",
       "fe2o3_gfx942_ocml_sin_f32_v1.kd"});
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

} // namespace

int main(int ArgumentCount, char **Arguments) {
  require(ArgumentCount == 1 || ArgumentCount == 2 || ArgumentCount == 4 ||
              ArgumentCount == 5,
          "usage: fe2o3-worker-pipeline-tests "
          "[OUTPUT.hsaco [INPUT.bc INPUT.o [OCML_OUTPUT.hsaco]]]");

  fe2o3::worker::detail::enforceReusableLldResult({0, true});
  fe2o3::worker::detail::enforceReusableLldResult({1, true});
  testLldExitPolicy(0);
  testLldExitPolicy(1);
  testSyntheticOcmlPipeline();
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

  auto MakeExactLdsGemmSlice1Request = [](uint32_t Workgroup = 64,
                                          uint32_t MaxWorkgroup = 64,
                                          uint32_t StaticLdsTiles = 2) {
    Request Result = makeV2Request(
        makeInput(InputKind::LlvmTextIr,
                  makeExactLdsGemmSlice1TextIr(Workgroup, MaxWorkgroup,
                                               StaticLdsTiles)),
        {}, {}, {}, {"tiled_gemm_lds_v1", "tiled_gemm_lds_v1.kd"}, 6);
    Result.Target = "gfx942:xnack-";
    Result.LinkOptions = {OptimizationLevel::O2, true, true};
    return Result;
  };
  Request ExactLdsGemmSlice1 = MakeExactLdsGemmSlice1Request();
  Response ExactLdsGemmSlice1Response = runSuccess(
      ExactLdsGemmSlice1, {"tiled_gemm_lds_v1", "tiled_gemm_lds_v1.kd"});
  requireDiagnostic(ExactLdsGemmSlice1Response,
                    "post_link.check=lds_gemm_slice1_profile status=ok ");
  requireDiagnostic(ExactLdsGemmSlice1Response,
                    "post_link.kernel name=tiled_gemm_lds_v1 "
                    "symbol=tiled_gemm_lds_v1.kd kernarg_size=304 "
                    "group_size=1024 private_size=0 kernarg_align=8 "
                    "wavefront_size=64 max_workgroup_size=64 "
                    "reqd_workgroup_size=[64,1,1]");

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
      ExactLdsGemmSlice1Response.LinkedOutput->Bytes;
  makeStaticSymbolTableRelocationSection(ExactLdsGemmRelocation);
  requireInspectionFailure(
      ExactLdsGemmRelocation, ExactLdsGemmSlice1,
      "post_link.check=lds_gemm_slice1_profile status=failed "
      "reason=residual_relocation_section");

  std::vector<uint8_t> ExactLdsGemmDependency =
      ExactLdsGemmSlice1Response.LinkedOutput->Bytes;
  makeDynamicDependency(ExactLdsGemmDependency);
  requireInspectionFailure(
      ExactLdsGemmDependency, ExactLdsGemmSlice1,
      "post_link.check=lds_gemm_slice1_profile status=failed "
      "reason=dynamic_dependency");

  std::vector<uint8_t> ExactLdsGemmUndefined =
      ExactLdsGemmSlice1Response.LinkedOutput->Bytes;
  makeDynamicSymbolUndefined(ExactLdsGemmUndefined, "tiled_gemm_lds_v1");
  requireInspectionFailure(ExactLdsGemmUndefined, ExactLdsGemmSlice1,
                           "post_link.check=unresolved status=failed "
                           "symbols=[tiled_gemm_lds_v1]");

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
