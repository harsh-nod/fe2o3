#include "WorkerPipeline.h"
#include "WorkerDeviceLibraryPolicy.h"
#include "WorkerLldPolicy.h"

#include "lld/Common/Driver.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/ADT/SmallString.h"
#include "llvm/ADT/SmallVector.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/BinaryFormat/AMDGPUMetadataVerifier.h"
#include "llvm/BinaryFormat/ELF.h"
#include "llvm/BinaryFormat/Magic.h"
#include "llvm/BinaryFormat/MsgPackDocument.h"
#include "llvm/Bitcode/BitcodeReader.h"
#include "llvm/Bitcode/BitcodeWriter.h"
#include "llvm/IR/AutoUpgrade.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DebugInfo.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/InstIterator.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Operator.h"
#include "llvm/IR/Verifier.h"
#include "llvm/Linker/Linker.h"
#include "llvm/MC/MCAsmInfo.h"
#include "llvm/MC/MCContext.h"
#include "llvm/MC/MCDisassembler/MCDisassembler.h"
#include "llvm/MC/MCInstrDesc.h"
#include "llvm/MC/MCInstrInfo.h"
#include "llvm/MC/MCRegisterInfo.h"
#include "llvm/MC/MCSubtargetInfo.h"
#include "llvm/MC/MCTargetOptions.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Object/ELFObjectFile.h"
#include "llvm/Object/ObjectFile.h"
#include "llvm/Passes/OptimizationLevel.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/MemoryBuffer.h"
#include "llvm/Support/Path.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"
#include "llvm/Transforms/IPO/GlobalDCE.h"
#include "llvm/Transforms/IPO/Internalize.h"
#include "llvm/Transforms/Utils/ModuleUtils.h"

#include <algorithm>
#include <cstdint>
#include <iomanip>
#include <limits>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <sstream>
#include <system_error>
#include <tuple>

using namespace llvm;
using namespace llvm::object;

LLD_HAS_DRIVER(elf)

#ifndef FE2O3_LLVM_BUILD_ID
#error "FE2O3_LLVM_BUILD_ID must be supplied by CMake"
#endif
#ifndef FE2O3_WORKER_BUILD_ID
#error "FE2O3_WORKER_BUILD_ID must be supplied by CMake"
#endif

namespace fe2o3::worker {
namespace {

constexpr StringLiteral LlvmBuildIdentity = FE2O3_LLVM_BUILD_ID;
constexpr StringLiteral WorkerBuildIdentity = FE2O3_WORKER_BUILD_ID;
constexpr StringLiteral UnauthenticatedTestWorkerIdentity =
    "fe2o3-unauthenticated-test-device-library-policy";
constexpr StringLiteral AmdGpuTriple = "amdgcn-amd-amdhsa";

Response failure(const Request &RequestValue, Stage FailureStage,
                 std::vector<std::string> Diagnostics) {
  Response Result{RequestValue.RequestId,
                  RequestValue.Identity,
                  WorkerBuildIdentity.str(),
                  FailureStage,
                  canonicalDiagnostics(Diagnostics),
                  std::nullopt};
  Result.Protocol = RequestValue.Protocol;
  Result.CompilerEnvelopeIdentity = RequestValue.CompilerEnvelopeIdentity;
  return Result;
}

Response failure(const Request &RequestValue, Stage FailureStage,
                 Error ErrorValue) {
  return failure(RequestValue, FailureStage,
                 {errorToDiagnostic(std::move(ErrorValue))});
}

Error pipelineError(const Twine &Message) {
  return createStringError(inconvertibleErrorCode(), Message);
}

class BoundedRawStream final : public raw_ostream {
public:
  explicit BoundedRawStream(size_t Limit) : Limit(Limit) { SetUnbuffered(); }

  StringRef str() const { return Buffer; }
  bool truncated() const { return Truncated; }

private:
  void write_impl(const char *Pointer, size_t Size) override {
    Position = Size > std::numeric_limits<uint64_t>::max() - Position
                   ? std::numeric_limits<uint64_t>::max()
                   : Position + Size;
    size_t Remaining = Limit - Buffer.size();
    size_t Accepted = std::min(Remaining, Size);
    Buffer.append(Pointer, Accepted);
    Truncated |= Accepted != Size;
  }

  uint64_t current_pos() const override { return Position; }

  std::string Buffer;
  size_t Limit;
  uint64_t Position = 0;
  bool Truncated = false;
};

struct TargetParts {
  std::string Cpu;
  std::optional<bool> SramEcc;
  std::optional<bool> Xnack;
};

enum class PostLinkProfile {
  LegacyGfx942G1,
  ExactRowSoftmaxV1,
  ExactLdsGemmSlice1,
  ExactWave64CollectivesV1,
  ExactFlashAttentionV1,
  ExactWorkgroupLdsReductionV1,
  ExactScopedAtomicV1,
  ExactMoeTop2V1
};

enum class MetadataValidationPolicy {
  Generic,
  ExactRowSoftmaxV1,
  ExactLdsGemmSlice1,
  ExactWave64CollectivesV1,
  ExactFlashAttentionV1,
  ExactWorkgroupLdsReductionV1,
  ExactScopedAtomicV1,
  ExactMoeTop2V1
};

constexpr StringLiteral ExactLdsGemmSlice1Entry = "tiled_gemm_lds_v1";
constexpr StringLiteral ExactLdsGemmSlice1Descriptor = "tiled_gemm_lds_v1.kd";
constexpr StringLiteral ExactRowSoftmaxV1Entry = "row_softmax_v1";
constexpr StringLiteral ExactRowSoftmaxV1Descriptor = "row_softmax_v1.kd";
constexpr StringLiteral ExactRowSoftmaxV1OcmlExp = "__ocml_exp_f32";
constexpr StringLiteral ExactRowSoftmaxV1Check = "row_softmax_v1_profile";
constexpr StringLiteral ExactRowSoftmaxV1PublishedLlvmBuildIdentity =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
constexpr StringLiteral ExactWave64CollectivesV1Entry = "wave64_collectives_v1";
constexpr StringLiteral ExactWave64CollectivesV1Descriptor =
    "wave64_collectives_v1.kd";
constexpr StringLiteral ExactFlashAttentionV1Entry =
    "flash_attention_causal_f32_b1_h1_n8_d16_v1";
constexpr StringLiteral ExactFlashAttentionV1Descriptor =
    "flash_attention_causal_f32_b1_h1_n8_d16_v1.kd";
constexpr StringLiteral ExactFlashAttentionV1OcmlExp = "__ocml_exp_f32";
constexpr StringLiteral ExactFlashAttentionV1PublishedLlvmBuildIdentity =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
constexpr StringLiteral ExactWorkgroupLdsReductionV1Entry =
    "lds_publish_read_reduce_i32_v1";
constexpr StringLiteral ExactWorkgroupLdsReductionV1Descriptor =
    "lds_publish_read_reduce_i32_v1.kd";
constexpr StringLiteral ExactWorkgroupLdsReductionV1Scratch =
    "__fe2o3_lds_reduction_v1_scratch";
constexpr StringLiteral ExactScopedAtomicV1Entry = "scoped_atomic_add_u32_v1";
constexpr StringLiteral ExactScopedAtomicV1Descriptor =
    "scoped_atomic_add_u32_v1.kd";
constexpr StringLiteral ExactMoeTop2V1Entry =
    "moe_top2_route_f32_t8_e4_k2_c4_v1";
constexpr StringLiteral ExactMoeTop2V1Descriptor =
    "moe_top2_route_f32_t8_e4_k2_c4_v1.kd";
constexpr StringLiteral ExactMoeTop2V1Check = "moe_top2_t8_e4_k2_c4_v1_profile";
constexpr StringLiteral ExactLdsGemmSlice1ProducerDataLayout =
    "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-"
    "p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-"
    "v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-"
    "v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";
constexpr StringLiteral ExactRowSoftmaxV1ProducerDataLayout =
    "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-"
    "p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-"
    "i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-"
    "v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";

constexpr StringLiteral ExactRowDescriptorSection = ".fe2o3.kd.v1";
constexpr StringLiteral ExactRowTranscriptSection =
    ".fe2o3.row-softmax-authority-transcript.v1";
// The section spelling is a legacy producer ABI. Its bytes are only the
// SHA-256 digest used to check transcript consistency inside this worker.
constexpr StringLiteral ExactRowTranscriptDigestSection =
    ".fe2o3.row-softmax-auth.v1";
constexpr StringLiteral ExactRowExpBoundarySection = ".fe2o3.row-exp.v1";
constexpr std::array<uint8_t, 32> ExactRowBodySha256 = {
    0xd4, 0x8d, 0x33, 0x20, 0xc2, 0x86, 0xc6, 0xda, 0x22, 0x53, 0xa1,
    0x04, 0x38, 0x60, 0x89, 0xe3, 0x89, 0x64, 0x8f, 0x42, 0x60, 0xf2,
    0xe7, 0xef, 0xda, 0x21, 0x26, 0x9f, 0xef, 0x95, 0x1c, 0x2c};
constexpr std::array<uint8_t, 32> ExactRowExpBoundaryIdentity = {
    0xc0, 0x55, 0xb0, 0xa1, 0x34, 0x51, 0x90, 0x5b, 0xaa, 0x0d, 0xd4,
    0x8e, 0x8f, 0x8b, 0xd3, 0x5c, 0x92, 0x8f, 0xe1, 0x79, 0xc6, 0xc3,
    0xfa, 0xa1, 0xfb, 0x9d, 0xc2, 0x75, 0x6e, 0xd2, 0x75, 0x28};

constexpr StringLiteral ExactWave64DescriptorSection = ".fe2o3.kd.v1";
constexpr StringLiteral ExactWave64AuthoritySection = ".fe2o3.wave64-auth.v1";
constexpr StringLiteral ExactWave64MirSection = ".fe2o3.wave64-mir.v1";
constexpr StringLiteral ExactWave64KirSection = ".fe2o3.wave64-kir.v1";
constexpr StringLiteral ExactWave64ProfileSection =
    ".fe2o3.wave64-descriptor.v1";
constexpr StringLiteral ExactFlashDescriptorSection = ".fe2o3.kd.v1";
constexpr StringLiteral ExactFlashTranscriptSection =
    ".fe2o3.flash-attention-authority-transcript.v1";
constexpr StringLiteral ExactFlashAuthoritySection =
    ".fe2o3.flash-attention-auth.v1";
constexpr StringLiteral ExactFlashOcmlBoundarySection =
    ".fe2o3.flash-attention-ocml-exp.v1";
constexpr std::array<uint8_t, 32> ExactFlashBodySha256 = {
    0x44, 0xee, 0xbf, 0x70, 0x89, 0xda, 0xb9, 0x54, 0xbf, 0x25, 0xc8,
    0x6e, 0xfa, 0x1d, 0x92, 0xda, 0xd4, 0xa2, 0xb8, 0x1b, 0x47, 0xeb,
    0x7b, 0x08, 0xf1, 0xe4, 0x49, 0xd1, 0xd0, 0x9a, 0xee, 0xad};
constexpr std::array<uint8_t, 32> ExactFlashAuthoritySha256 = {
    0x4c, 0xde, 0x34, 0xc0, 0x5c, 0xaa, 0xa8, 0x8b, 0x74, 0xc8, 0x42,
    0x65, 0x1b, 0x08, 0x11, 0x46, 0x87, 0x1c, 0xeb, 0xdb, 0x46, 0x24,
    0xf1, 0xa6, 0xb6, 0xea, 0x9a, 0x36, 0x4f, 0xa3, 0xd0, 0xd0};
constexpr std::array<uint8_t, 32> ExactFlashOcmlBoundarySha256 = {
    0xdb, 0x91, 0x96, 0x57, 0x5c, 0xcc, 0xcc, 0xd8, 0x03, 0x53, 0xf5,
    0xed, 0x04, 0xbc, 0x42, 0x5b, 0x64, 0x34, 0x4a, 0x42, 0x07, 0x09,
    0x79, 0x3e, 0xe8, 0x37, 0x79, 0xad, 0xd2, 0x1e, 0x47, 0x60};
constexpr std::array<uint8_t, 32> ExactFlashMachineSha256 = {
    0xd2, 0xaa, 0x57, 0xc0, 0xf4, 0x68, 0xf5, 0x74, 0xf4, 0x4a, 0x9f,
    0xea, 0x06, 0xbb, 0xb8, 0xe9, 0x8a, 0xa9, 0xb6, 0x0b, 0xb2, 0xd9,
    0x30, 0x3c, 0xc4, 0xd8, 0xb6, 0xca, 0xf0, 0xcf, 0xca, 0x54};
constexpr std::array<uint8_t, 32> ExactWave64BodySha256 = {
    0xe3, 0x90, 0x1d, 0x41, 0xc7, 0x20, 0xcf, 0x9d, 0xdd, 0x7e, 0xd0,
    0x1f, 0xbc, 0x48, 0x77, 0x93, 0x1d, 0x42, 0x08, 0x08, 0x11, 0x44,
    0xe0, 0xea, 0x1f, 0x0a, 0x40, 0x32, 0xd0, 0x83, 0xb0, 0x13};
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

enum class ExactWorkgroupSyncKind { LdsReduction, ScopedAtomic };

struct ExactWorkgroupSyncProfile {
  ExactWorkgroupSyncKind Kind;
  StringLiteral Entry;
  StringLiteral Descriptor;
  StringLiteral SectionPrefix;
  std::array<uint8_t, 32> BodySha256;
  std::array<std::array<uint8_t, 32>, 11> SectionIdentities;
  std::array<uint8_t, 32> MachineSha256;
  StringLiteral Check;
};

constexpr ExactWorkgroupSyncProfile ExactWorkgroupLdsReductionV1 = {
    ExactWorkgroupSyncKind::LdsReduction,
    ExactWorkgroupLdsReductionV1Entry,
    ExactWorkgroupLdsReductionV1Descriptor,
    ".fe2o3.wg-lds",
    {0xf8, 0xfd, 0x67, 0x3b, 0x19, 0x6c, 0x6b, 0xd2, 0x3e, 0xc3, 0x15,
     0xba, 0xc0, 0x19, 0x5e, 0x80, 0x54, 0xaf, 0xb4, 0xac, 0xcc, 0xc1,
     0xca, 0xea, 0x5a, 0x77, 0xb9, 0x30, 0x71, 0x91, 0x60, 0x21},
    {{{0x3e, 0x7e, 0xc0, 0x81, 0xc7, 0x95, 0x82, 0x88, 0xf9, 0xd9, 0x97,
       0xd4, 0x0e, 0x6f, 0x41, 0xa7, 0xfa, 0xab, 0xc5, 0x6a, 0x3a, 0xdd,
       0x73, 0x40, 0x99, 0xcd, 0x17, 0x77, 0x44, 0x3b, 0x29, 0x83},
      {0x6b, 0xc8, 0xf4, 0x49, 0xf4, 0x58, 0xcf, 0x8f, 0x31, 0xb4, 0x62,
       0x5b, 0x38, 0xb7, 0x20, 0x4d, 0xd3, 0x4f, 0x20, 0xbe, 0xea, 0xbb,
       0x80, 0xb5, 0x54, 0x54, 0xa5, 0x66, 0x6b, 0xe7, 0x49, 0xb5},
      {0xbb, 0x05, 0xd2, 0xed, 0xce, 0x90, 0x93, 0xf5, 0x3e, 0x68, 0xb6,
       0x37, 0xe8, 0xa4, 0x6f, 0x70, 0x9a, 0x34, 0x1c, 0x29, 0x5a, 0x14,
       0xfd, 0xee, 0xd7, 0x44, 0xfd, 0xa4, 0x7c, 0x3f, 0xdf, 0x3a},
      {0x20, 0xd5, 0x49, 0x5b, 0x23, 0x66, 0x24, 0xc5, 0x1a, 0x67, 0x87,
       0xd9, 0x95, 0x56, 0x94, 0x56, 0xb1, 0xa6, 0xbb, 0xfc, 0x7c, 0x70,
       0xe5, 0x43, 0x69, 0x4d, 0xeb, 0xc6, 0x2b, 0xeb, 0x46, 0xb1},
      {0xb3, 0x84, 0x04, 0x57, 0xdb, 0x66, 0x5f, 0x11, 0x4c, 0xae, 0xff,
       0x92, 0xa4, 0xc7, 0xdd, 0xbe, 0x63, 0x88, 0xac, 0x14, 0xbe, 0xc4,
       0x8c, 0x29, 0x77, 0xc9, 0xa6, 0x21, 0x16, 0x81, 0x40, 0xc6},
      {0x1c, 0x9f, 0xfd, 0xb9, 0x49, 0xc2, 0x18, 0xc2, 0xca, 0xd9, 0x87,
       0x55, 0x89, 0x57, 0xa2, 0x71, 0x9a, 0xf4, 0x92, 0x34, 0x91, 0x98,
       0xbe, 0x95, 0xa9, 0x43, 0xf8, 0x46, 0x91, 0x05, 0xfd, 0xf2},
      {0x50, 0x97, 0xff, 0x92, 0xf4, 0x88, 0x1d, 0x71, 0x17, 0x18, 0x29,
       0x30, 0x84, 0x8d, 0x55, 0xab, 0x78, 0x1e, 0xe6, 0x82, 0x24, 0xe1,
       0xac, 0x78, 0x9e, 0xbf, 0x85, 0xf8, 0xbd, 0x41, 0x98, 0xcf},
      {0x28, 0x01, 0x83, 0x42, 0x9d, 0x4e, 0xcd, 0xc3, 0x28, 0x2f, 0xa0,
       0xa7, 0x72, 0x74, 0xe2, 0x22, 0x26, 0xde, 0xc7, 0xda, 0xce, 0xb9,
       0x1e, 0xb4, 0x21, 0xa3, 0x3c, 0x8d, 0xc6, 0x8b, 0x0f, 0x2b},
      {0x20, 0x8a, 0x56, 0x97, 0xf3, 0x78, 0x9f, 0x56, 0x81, 0xfe, 0xf0,
       0xdd, 0x59, 0x25, 0xfb, 0x46, 0xea, 0x72, 0x14, 0x72, 0x9f, 0x9f,
       0xdd, 0xfa, 0xbc, 0x35, 0x7f, 0xfb, 0xdf, 0xff, 0x80, 0xd0},
      {0xba, 0x37, 0xb8, 0x71, 0x05, 0xee, 0x1e, 0xb2, 0x6d, 0x8a, 0x36,
       0x52, 0x3b, 0x15, 0xa6, 0xe4, 0xfe, 0x78, 0x19, 0x98, 0xe5, 0x42,
       0x5e, 0xb2, 0x36, 0xdd, 0x46, 0xbc, 0xf8, 0x05, 0xce, 0x7f},
      {0x82, 0xde, 0x81, 0x98, 0xaf, 0xd7, 0xbc, 0xd5, 0x08, 0x2d, 0x08,
       0x95, 0xb8, 0xf6, 0x2a, 0xba, 0xef, 0x1d, 0x31, 0xfb, 0x7b, 0x0c,
       0x0d, 0xe7, 0xd6, 0xb6, 0xb5, 0x20, 0x1a, 0x75, 0x94, 0xa7}}},
    {0xcf, 0x07, 0x6a, 0xdc, 0xb1, 0x44, 0xf1, 0x44, 0x79, 0x89, 0x8c,
     0xc6, 0x20, 0xd4, 0x9d, 0x1a, 0x5e, 0x0c, 0x1c, 0xcd, 0xb0, 0x9d,
     0x17, 0xa8, 0x99, 0xca, 0x97, 0xfc, 0x21, 0xbe, 0xfc, 0x45},
    "workgroup_lds_reduction_v1_profile"};

constexpr ExactWorkgroupSyncProfile ExactScopedAtomicV1 = {
    ExactWorkgroupSyncKind::ScopedAtomic,
    ExactScopedAtomicV1Entry,
    ExactScopedAtomicV1Descriptor,
    ".fe2o3.wg-atomic",
    {0xdf, 0xf0, 0x69, 0x33, 0x72, 0x35, 0xe4, 0xf5, 0x0f, 0x8d, 0x09,
     0x81, 0x2e, 0x85, 0x4f, 0x3f, 0x42, 0x75, 0xda, 0x38, 0xc2, 0xab,
     0x04, 0xf7, 0xac, 0xcb, 0x44, 0xbc, 0x18, 0x00, 0xeb, 0x5c},
    {{{0xc0, 0xf0, 0x0a, 0x14, 0xc5, 0x94, 0x1f, 0x34, 0x74, 0x1f, 0xc1,
       0x0c, 0xa7, 0x79, 0x8c, 0xe9, 0xcf, 0x47, 0x28, 0x82, 0x94, 0xb0,
       0xbc, 0xc4, 0x3c, 0xdd, 0xb7, 0xd2, 0x2b, 0xbf, 0xe9, 0x7e},
      {0x40, 0x93, 0x57, 0xef, 0x99, 0xd9, 0xec, 0x78, 0xc9, 0x60, 0xcc,
       0xa0, 0xe2, 0x1a, 0x4e, 0x15, 0x3c, 0x60, 0xaf, 0x52, 0x2c, 0x1c,
       0x4d, 0x72, 0x6a, 0x9f, 0x23, 0xb5, 0xc7, 0x27, 0x1b, 0x91},
      {0xcb, 0xe0, 0x12, 0xbd, 0xe7, 0x63, 0x22, 0x7f, 0xcf, 0x4e, 0x22,
       0x35, 0x75, 0xf8, 0x93, 0x5f, 0xd8, 0x6b, 0xfa, 0xf2, 0xc9, 0x81,
       0x72, 0xb0, 0x9d, 0xe1, 0x1f, 0xf0, 0x79, 0xb1, 0x34, 0x86},
      {0x52, 0x1d, 0xec, 0x6e, 0x8e, 0x00, 0xb3, 0x8a, 0x4c, 0x47, 0x9c,
       0xf3, 0xb9, 0x3d, 0x51, 0x54, 0x43, 0x18, 0xcd, 0x2b, 0xac, 0xe9,
       0xb0, 0x8c, 0x56, 0xe2, 0xd6, 0xaf, 0x57, 0xab, 0x37, 0xf5},
      {0xfa, 0xd7, 0x32, 0x25, 0x2d, 0xa6, 0x44, 0xac, 0xb7, 0xa3, 0x8f,
       0x09, 0x13, 0xe0, 0x62, 0x46, 0x12, 0x09, 0x3a, 0x7d, 0x98, 0x29,
       0x42, 0x49, 0x7c, 0x3d, 0xe4, 0xda, 0x4f, 0x4b, 0xc8, 0x2f},
      {0xbc, 0xf7, 0xe8, 0x74, 0xdb, 0x23, 0x61, 0x57, 0xdd, 0x6a, 0x8d,
       0x8d, 0x76, 0xc6, 0x9b, 0x69, 0x04, 0x17, 0x3e, 0xfe, 0xb5, 0x4f,
       0x89, 0x05, 0xb9, 0xae, 0x1d, 0x48, 0x10, 0xca, 0x7b, 0x76},
      {0x20, 0xa0, 0x07, 0x6e, 0x0e, 0xe9, 0xeb, 0x4e, 0x8d, 0xd9, 0x0e,
       0x60, 0x1b, 0x36, 0x8f, 0xf3, 0x95, 0x78, 0x5d, 0xfe, 0xf1, 0xfd,
       0x5c, 0x80, 0x6d, 0x13, 0x18, 0x74, 0x16, 0x75, 0xe8, 0x14},
      {0x3c, 0x40, 0x36, 0xa3, 0x27, 0xf8, 0x60, 0xf7, 0x90, 0x47, 0x93,
       0x87, 0x98, 0xf2, 0x98, 0x33, 0xe7, 0x43, 0x94, 0xdf, 0x76, 0xff,
       0xce, 0x7b, 0x40, 0x48, 0xb0, 0x0b, 0x6b, 0x59, 0xb6, 0x21},
      {0xfd, 0xe8, 0x3d, 0x5b, 0x84, 0x2a, 0x55, 0xde, 0x02, 0x38, 0x37,
       0xac, 0xcd, 0x46, 0xe1, 0x56, 0xcb, 0x9c, 0x77, 0xfe, 0xd9, 0x7b,
       0xc0, 0xdf, 0x55, 0x93, 0x61, 0x02, 0x84, 0x3c, 0xee, 0xa3},
      {0x99, 0x84, 0xf9, 0x51, 0x79, 0xcd, 0xc2, 0x5b, 0x14, 0x55, 0x0e,
       0x06, 0xb0, 0x4c, 0x2f, 0x91, 0x8b, 0xcd, 0xa9, 0xcb, 0x9b, 0x4e,
       0xbb, 0xef, 0x26, 0xb7, 0xcc, 0xa7, 0x02, 0x21, 0x05, 0x0f},
      {0x44, 0x38, 0x0e, 0xb9, 0x88, 0xca, 0x81, 0x46, 0x4d, 0x38, 0x2d,
       0xa6, 0xd2, 0xec, 0x08, 0x4b, 0xdf, 0x64, 0xb4, 0xac, 0x76, 0xc3,
       0xc7, 0x25, 0xb6, 0x92, 0xca, 0x53, 0xdc, 0x1c, 0x42, 0x00}}},
    {0xda, 0xb7, 0x5d, 0x4a, 0xf4, 0x69, 0x8f, 0x71, 0x36, 0x91, 0xa3,
     0x5f, 0x43, 0xf5, 0x0c, 0x5b, 0xeb, 0xd2, 0x0f, 0x79, 0x6b, 0x08,
     0xb1, 0xb6, 0xb1, 0xb9, 0xc3, 0xde, 0x10, 0x11, 0x47, 0x51},
    "scoped_atomic_v1_profile"};

constexpr StringLiteral ExactMoeTop2V1BodySha256 =
    "b703e4b9bf89f77887b6c1578475b0a556851e7235342efd5247acf999ca3b39";
constexpr StringLiteral ExactMoeTop2V1MachineSha256 =
    "4728028b85cc3ff407190de6a70b9c844437e9f92fc587e0614940be898346cf";
constexpr std::array<StringLiteral, 16> ExactMoeTop2V1Sections = {
    ".fe2o3.moe.source.v1",   ".fe2o3.moe.namespace.v1",
    ".fe2o3.moe.crate.v1",    ".fe2o3.moe.authority.v1",
    ".fe2o3.moe.mir.v1",      ".fe2o3.moe.fnabi.v1",
    ".fe2o3.moe.compiler.v1", ".fe2o3.moe.terminals.v3",
    ".fe2o3.moe.abi.v1",      ".fe2o3.moe.effects.v1",
    ".fe2o3.moe.profile.v1",  ".fe2o3.moe.routing.v1",
    ".fe2o3.moe.kir.v1",      ".fe2o3.moe.descriptor.v1",
    ".fe2o3.moe.provider.v1", ".fe2o3.moe.layout.v1"};
constexpr std::array<StringLiteral, 15> ExactMoeTop2V1Identities = {
    "b77016caa0c3708e420e583712e65e4e6428db7b4feafd8d0a1d4bdc475ef6ff",
    "4180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1",
    "fce826d20b8f2e4eca29180a2d9fc34949b51a07841dd7f79258625fc6a9f296",
    "0ecec41db62eae781429526170aa60a73437f4cd8261b7e4d34ffe62309ad6e9",
    "934c2205973e24216d537c5f89bc65d8e15dd68376dce477d1768e2936b4fc13",
    "f796180c590cd84125921f2aaeb85ab13ef1b5c0502c1b1316bf9a2114fd30f6",
    "4950c225e0cdbdce4e1230166984949970290dedc19e8dc4cd31f865f1625a4a",
    "3dbbe3ec9d58a7c285a14159294051498378f291525d8445113b17aab9b0e08b",
    "4c225cf47613b98e7baca366167bfa4c27ae43ec47433b49d1df5a1d960fb4aa",
    "496368f70c211b001417fb904622971d008ca24442beaef3e4c6c175b4f5f6ba",
    "100bc49f34627485a959b7201a238bbf8421df800d7f1028bbfff6bd8c51edd1",
    "a94a13c1ad0ac1498e1c6cc63416dc1cda2f7c14c5e4c1c422e354820fc09315",
    "3dfa5db91762403106e7d3a1581700b1d03282f5dd15727761e5cc42c63731b2",
    "7852334c9d38cd4544c535377650554344e8e59de2dc822f4f2492dfea998743",
    "9a0e923eef32bce3ef2de4663fc4d395cfd2179c55dd586180d9c25faa377536"};

bool bytesMatchLowerHex(ArrayRef<uint8_t> Bytes, StringRef Hex) {
  if (Hex.size() != Bytes.size() * 2)
    return false;
  for (size_t Index = 0; Index != Bytes.size(); ++Index) {
    uint8_t Parsed = 0;
    if (Hex.slice(Index * 2, Index * 2 + 2).getAsInteger(16, Parsed) ||
        Parsed != Bytes[Index])
      return false;
  }
  return true;
}

bool isExactLdsGemmSlice1SymbolSet(ArrayRef<std::string> Symbols) {
  return std::set<std::string>(Symbols.begin(), Symbols.end()) ==
         std::set<std::string>{ExactLdsGemmSlice1Entry.str(),
                               ExactLdsGemmSlice1Descriptor.str()};
}

bool isExactRowSoftmaxV1SymbolSet(ArrayRef<std::string> Symbols) {
  return std::set<std::string>(Symbols.begin(), Symbols.end()) ==
         std::set<std::string>{ExactRowSoftmaxV1Entry.str(),
                               ExactRowSoftmaxV1Descriptor.str(),
                               ExactRowSoftmaxV1OcmlExp.str()};
}

bool isExactWave64CollectivesV1SymbolSet(ArrayRef<std::string> Symbols) {
  return std::set<std::string>(Symbols.begin(), Symbols.end()) ==
         std::set<std::string>{ExactWave64CollectivesV1Entry.str(),
                               ExactWave64CollectivesV1Descriptor.str()};
}

bool isExactFlashAttentionV1SymbolSet(ArrayRef<std::string> Symbols) {
  return std::set<std::string>(Symbols.begin(), Symbols.end()) ==
         std::set<std::string>{ExactFlashAttentionV1Entry.str(),
                               ExactFlashAttentionV1Descriptor.str(),
                               ExactFlashAttentionV1OcmlExp.str()};
}

bool isExactWorkgroupSyncSymbolSet(ArrayRef<std::string> Symbols,
                                   const ExactWorkgroupSyncProfile &Profile) {
  return std::set<std::string>(Symbols.begin(), Symbols.end()) ==
         std::set<std::string>{Profile.Entry.str(), Profile.Descriptor.str()};
}

bool isExactMoeTop2V1SymbolSet(ArrayRef<std::string> Symbols) {
  return std::set<std::string>(Symbols.begin(), Symbols.end()) ==
         std::set<std::string>{ExactMoeTop2V1Entry.str(),
                               ExactMoeTop2V1Descriptor.str()};
}

bool isExactLdsGemmSlice1RequestCandidate(const Request &RequestValue) {
  bool CompilerInputMatches =
      RequestValue.Inputs.size() == 1 &&
      RequestValue.CompilerModule.Kind == InputKind::LlvmTextIr &&
      RequestValue.Inputs.front().Kind == RequestValue.CompilerModule.Kind &&
      RequestValue.Inputs.front().Digest ==
          RequestValue.CompilerModule.Digest &&
      RequestValue.Inputs.front().Bytes.size() ==
          RequestValue.CompilerModule.Bytes.size();
  return RequestValue.Protocol == ProtocolVersion::V2 &&
         RequestValue.Target == "gfx942:xnack-" &&
         RequestValue.CodeObjectVersion == 6 && CompilerInputMatches &&
         RequestValue.ExternalProviders.empty() &&
         RequestValue.ImportSymbols.empty() &&
         RequestValue.ExportSymbols.empty() &&
         isExactLdsGemmSlice1SymbolSet(RequestValue.RequiredSymbols) &&
         isExactLdsGemmSlice1SymbolSet(RequestValue.ExpectedDefinedSymbols) &&
         isExactLdsGemmSlice1SymbolSet(RequestValue.FinalSymbols);
}

bool isClosedExactLdsGemmSlice1Request(const Request &RequestValue) {
  return isExactLdsGemmSlice1RequestCandidate(RequestValue) &&
         RequestValue.LinkOptions.Optimization == OptimizationLevel::O2 &&
         RequestValue.LinkOptions.StripDebug &&
         RequestValue.LinkOptions.VerifyEach;
}

bool isExactRowSoftmaxV1RequestCandidate(const Request &RequestValue) {
  bool CompilerInputMatches =
      RequestValue.Inputs.size() == 1 &&
      RequestValue.CompilerModule.Kind == InputKind::LlvmTextIr &&
      RequestValue.Inputs.front().Kind == RequestValue.CompilerModule.Kind &&
      RequestValue.Inputs.front().Digest ==
          RequestValue.CompilerModule.Digest &&
      RequestValue.Inputs.front().Bytes == RequestValue.CompilerModule.Bytes;
  return RequestValue.Protocol == ProtocolVersion::V2 &&
         RequestValue.Target == "gfx942:xnack-" &&
         RequestValue.CodeObjectVersion == 6 && CompilerInputMatches &&
         RequestValue.ExternalProviders.empty() &&
         RequestValue.ImportSymbols ==
             std::vector<std::string>{ExactRowSoftmaxV1OcmlExp.str()} &&
         RequestValue.ExportSymbols.empty() &&
         isExactRowSoftmaxV1SymbolSet(RequestValue.RequiredSymbols) &&
         isExactRowSoftmaxV1SymbolSet(RequestValue.ExpectedDefinedSymbols) &&
         isExactRowSoftmaxV1SymbolSet(RequestValue.FinalSymbols);
}

bool isClosedExactRowSoftmaxV1Request(const Request &RequestValue) {
  return isExactRowSoftmaxV1RequestCandidate(RequestValue) &&
         RequestValue.LinkOptions.Optimization == OptimizationLevel::O0 &&
         RequestValue.LinkOptions.StripDebug &&
         RequestValue.LinkOptions.VerifyEach;
}

bool isExactWave64CollectivesV1RequestCandidate(const Request &RequestValue) {
  bool CompilerInputMatches =
      RequestValue.Inputs.size() == 1 &&
      RequestValue.CompilerModule.Kind == InputKind::LlvmTextIr &&
      RequestValue.Inputs.front().Kind == RequestValue.CompilerModule.Kind &&
      RequestValue.Inputs.front().Digest ==
          RequestValue.CompilerModule.Digest &&
      RequestValue.Inputs.front().Bytes == RequestValue.CompilerModule.Bytes;
  return RequestValue.Protocol == ProtocolVersion::V2 &&
         RequestValue.Target == "gfx942:xnack-" &&
         RequestValue.CodeObjectVersion == 6 && CompilerInputMatches &&
         RequestValue.ExternalProviders.empty() &&
         RequestValue.ImportSymbols.empty() &&
         RequestValue.ExportSymbols.empty() &&
         isExactWave64CollectivesV1SymbolSet(RequestValue.RequiredSymbols) &&
         isExactWave64CollectivesV1SymbolSet(
             RequestValue.ExpectedDefinedSymbols) &&
         isExactWave64CollectivesV1SymbolSet(RequestValue.FinalSymbols);
}

bool isClosedExactWave64CollectivesV1Request(const Request &RequestValue) {
  return isExactWave64CollectivesV1RequestCandidate(RequestValue) &&
         RequestValue.LinkOptions.Optimization == OptimizationLevel::O2 &&
         RequestValue.LinkOptions.StripDebug &&
         RequestValue.LinkOptions.VerifyEach;
}

bool isExactFlashAttentionV1RequestCandidate(const Request &RequestValue) {
  bool CompilerInputMatches =
      RequestValue.Inputs.size() == 1 &&
      RequestValue.CompilerModule.Kind == InputKind::LlvmTextIr &&
      RequestValue.Inputs.front().Kind == RequestValue.CompilerModule.Kind &&
      RequestValue.Inputs.front().Digest ==
          RequestValue.CompilerModule.Digest &&
      RequestValue.Inputs.front().Bytes == RequestValue.CompilerModule.Bytes;
  return RequestValue.Protocol == ProtocolVersion::V2 &&
         RequestValue.Target == "gfx942:xnack-" &&
         RequestValue.CodeObjectVersion == 6 && CompilerInputMatches &&
         RequestValue.ExternalProviders.empty() &&
         RequestValue.ImportSymbols ==
             std::vector<std::string>{ExactFlashAttentionV1OcmlExp.str()} &&
         RequestValue.ExportSymbols.empty() &&
         isExactFlashAttentionV1SymbolSet(RequestValue.RequiredSymbols) &&
         isExactFlashAttentionV1SymbolSet(
             RequestValue.ExpectedDefinedSymbols) &&
         isExactFlashAttentionV1SymbolSet(RequestValue.FinalSymbols);
}

bool isClosedExactFlashAttentionV1Request(const Request &RequestValue) {
  return isExactFlashAttentionV1RequestCandidate(RequestValue) &&
         RequestValue.LinkOptions.Optimization == OptimizationLevel::O2 &&
         RequestValue.LinkOptions.StripDebug &&
         RequestValue.LinkOptions.VerifyEach;
}

bool isExactWorkgroupSyncRequestCandidate(
    const Request &RequestValue, const ExactWorkgroupSyncProfile &Profile) {
  bool CompilerInputMatches =
      RequestValue.Inputs.size() == 1 &&
      RequestValue.CompilerModule.Kind == InputKind::LlvmTextIr &&
      RequestValue.Inputs.front().Kind == RequestValue.CompilerModule.Kind &&
      RequestValue.Inputs.front().Digest ==
          RequestValue.CompilerModule.Digest &&
      RequestValue.Inputs.front().Bytes == RequestValue.CompilerModule.Bytes;
  return RequestValue.Protocol == ProtocolVersion::V2 &&
         RequestValue.Target == "gfx942:xnack-" &&
         RequestValue.CodeObjectVersion == 6 && CompilerInputMatches &&
         RequestValue.ExternalProviders.empty() &&
         RequestValue.ImportSymbols.empty() &&
         RequestValue.ExportSymbols.empty() &&
         isExactWorkgroupSyncSymbolSet(RequestValue.RequiredSymbols, Profile) &&
         isExactWorkgroupSyncSymbolSet(RequestValue.ExpectedDefinedSymbols,
                                       Profile) &&
         isExactWorkgroupSyncSymbolSet(RequestValue.FinalSymbols, Profile);
}

bool isClosedExactWorkgroupSyncRequest(
    const Request &RequestValue, const ExactWorkgroupSyncProfile &Profile) {
  return isExactWorkgroupSyncRequestCandidate(RequestValue, Profile) &&
         RequestValue.LinkOptions.Optimization == OptimizationLevel::O2 &&
         RequestValue.LinkOptions.StripDebug &&
         RequestValue.LinkOptions.VerifyEach;
}

bool isExactMoeTop2V1RequestCandidate(const Request &RequestValue) {
  bool CompilerInputMatches =
      RequestValue.Inputs.size() == 1 &&
      RequestValue.CompilerModule.Kind == InputKind::LlvmTextIr &&
      RequestValue.Inputs.front().Kind == RequestValue.CompilerModule.Kind &&
      RequestValue.Inputs.front().Digest ==
          RequestValue.CompilerModule.Digest &&
      RequestValue.Inputs.front().Bytes == RequestValue.CompilerModule.Bytes;
  return RequestValue.Protocol == ProtocolVersion::V2 &&
         RequestValue.Target == "gfx942:xnack-" &&
         RequestValue.CodeObjectVersion == 6 && CompilerInputMatches &&
         RequestValue.ExternalProviders.empty() &&
         RequestValue.ImportSymbols.empty() &&
         RequestValue.ExportSymbols.empty() &&
         isExactMoeTop2V1SymbolSet(RequestValue.RequiredSymbols) &&
         isExactMoeTop2V1SymbolSet(RequestValue.ExpectedDefinedSymbols) &&
         isExactMoeTop2V1SymbolSet(RequestValue.FinalSymbols);
}

bool isClosedExactMoeTop2V1Request(const Request &RequestValue) {
  return isExactMoeTop2V1RequestCandidate(RequestValue) &&
         RequestValue.LinkOptions.Optimization == OptimizationLevel::O2 &&
         RequestValue.LinkOptions.StripDebug &&
         RequestValue.LinkOptions.VerifyEach;
}

const ExactWorkgroupSyncProfile *
exactWorkgroupSyncProfile(const Request &RequestValue) {
  for (const ExactWorkgroupSyncProfile *Profile :
       {&ExactWorkgroupLdsReductionV1, &ExactScopedAtomicV1})
    if (isExactWorkgroupSyncRequestCandidate(RequestValue, *Profile))
      return Profile;
  return nullptr;
}

bool namesExactWave64CollectivesV1(ArrayRef<std::string> Symbols) {
  return llvm::any_of(Symbols, [](StringRef Symbol) {
    return Symbol == ExactWave64CollectivesV1Entry ||
           Symbol == ExactWave64CollectivesV1Descriptor;
  });
}

bool mentionsExactWave64CollectivesV1(const Request &RequestValue) {
  return namesExactWave64CollectivesV1(RequestValue.RequiredSymbols) ||
         namesExactWave64CollectivesV1(RequestValue.ExpectedDefinedSymbols) ||
         namesExactWave64CollectivesV1(RequestValue.ImportSymbols) ||
         namesExactWave64CollectivesV1(RequestValue.ExportSymbols) ||
         namesExactWave64CollectivesV1(RequestValue.FinalSymbols);
}

bool mentionsExactFlashAttentionV1(const Request &RequestValue) {
  auto NamesFlash = [](ArrayRef<std::string> Symbols) {
    return llvm::any_of(Symbols, [](StringRef Symbol) {
      return Symbol == ExactFlashAttentionV1Entry ||
             Symbol == ExactFlashAttentionV1Descriptor;
    });
  };
  return NamesFlash(RequestValue.RequiredSymbols) ||
         NamesFlash(RequestValue.ExpectedDefinedSymbols) ||
         NamesFlash(RequestValue.ImportSymbols) ||
         NamesFlash(RequestValue.ExportSymbols) ||
         NamesFlash(RequestValue.FinalSymbols);
}

bool containsExactRowSoftmaxV1CompilerMarker(const Request &RequestValue) {
  if (RequestValue.CompilerModule.Kind != InputKind::LlvmTextIr)
    return false;
  StringRef Text(
      reinterpret_cast<const char *>(RequestValue.CompilerModule.Bytes.data()),
      RequestValue.CompilerModule.Bytes.size());
  return Text.contains("module asm \".section .fe2o3.row-softmax-") ||
         Text.contains("module asm \".section .fe2o3.row-exp.");
}

bool mentionsExactRowSoftmaxV1(const Request &RequestValue) {
  auto NamesRow = [](ArrayRef<std::string> Symbols) {
    return llvm::any_of(Symbols, [](StringRef Symbol) {
      return Symbol == ExactRowSoftmaxV1Entry ||
             Symbol == ExactRowSoftmaxV1Descriptor;
    });
  };
  return NamesRow(RequestValue.RequiredSymbols) ||
         NamesRow(RequestValue.ExpectedDefinedSymbols) ||
         NamesRow(RequestValue.ImportSymbols) ||
         NamesRow(RequestValue.ExportSymbols) ||
         NamesRow(RequestValue.FinalSymbols) ||
         containsExactRowSoftmaxV1CompilerMarker(RequestValue);
}

bool mentionsExactWorkgroupSync(const Request &RequestValue) {
  auto Mentions = [&](ArrayRef<std::string> Symbols) {
    return llvm::any_of(Symbols, [](StringRef Symbol) {
      return Symbol == ExactWorkgroupLdsReductionV1Entry ||
             Symbol == ExactWorkgroupLdsReductionV1Descriptor ||
             Symbol == ExactScopedAtomicV1Entry ||
             Symbol == ExactScopedAtomicV1Descriptor;
    });
  };
  return Mentions(RequestValue.RequiredSymbols) ||
         Mentions(RequestValue.ExpectedDefinedSymbols) ||
         Mentions(RequestValue.ImportSymbols) ||
         Mentions(RequestValue.ExportSymbols) ||
         Mentions(RequestValue.FinalSymbols);
}

bool mentionsExactMoeTop2V1(const Request &RequestValue) {
  auto Mentions = [](ArrayRef<std::string> Symbols) {
    return llvm::any_of(Symbols, [](StringRef Symbol) {
      return Symbol == ExactMoeTop2V1Entry ||
             Symbol == ExactMoeTop2V1Descriptor;
    });
  };
  return Mentions(RequestValue.RequiredSymbols) ||
         Mentions(RequestValue.ExpectedDefinedSymbols) ||
         Mentions(RequestValue.ImportSymbols) ||
         Mentions(RequestValue.ExportSymbols) ||
         Mentions(RequestValue.FinalSymbols);
}

Expected<PostLinkProfile>
selectPostLinkProfile(const Request &RequestValue,
                      const std::set<std::string> &ExpectedSymbols) {
  const std::set<std::string> ExactRowSymbols = {
      ExactRowSoftmaxV1Entry.str(), ExactRowSoftmaxV1Descriptor.str(),
      ExactRowSoftmaxV1OcmlExp.str()};
  if (ExpectedSymbols == ExactRowSymbols) {
    if (!isClosedExactRowSoftmaxV1Request(RequestValue))
      return pipelineError("exact row-softmax V1 symbols require the closed "
                           "Worker V2 profile");
    return PostLinkProfile::ExactRowSoftmaxV1;
  }
  const std::set<std::string> ExactLdsSymbols = {
      ExactLdsGemmSlice1Entry.str(), ExactLdsGemmSlice1Descriptor.str()};
  if (ExpectedSymbols == ExactLdsSymbols) {
    if (!isClosedExactLdsGemmSlice1Request(RequestValue))
      return pipelineError(
          "exact LDS GEMM Slice1 symbols require the closed Worker V2 profile");
    return PostLinkProfile::ExactLdsGemmSlice1;
  }
  const std::set<std::string> ExactWave64Symbols = {
      ExactWave64CollectivesV1Entry.str(),
      ExactWave64CollectivesV1Descriptor.str()};
  if (ExpectedSymbols == ExactWave64Symbols) {
    if (!isClosedExactWave64CollectivesV1Request(RequestValue))
      return pipelineError("exact Wave64 collectives symbols require the "
                           "closed Worker V2 profile");
    return PostLinkProfile::ExactWave64CollectivesV1;
  }
  const std::set<std::string> ExactFlashSymbols = {
      ExactFlashAttentionV1Entry.str(), ExactFlashAttentionV1Descriptor.str(),
      ExactFlashAttentionV1OcmlExp.str()};
  if (ExpectedSymbols == ExactFlashSymbols) {
    if (!isClosedExactFlashAttentionV1Request(RequestValue))
      return pipelineError("exact FlashAttention V1 symbols require the "
                           "closed Worker V2 profile");
    return PostLinkProfile::ExactFlashAttentionV1;
  }
  for (const auto &[Profile, Kind] :
       {std::pair{&ExactWorkgroupLdsReductionV1,
                  PostLinkProfile::ExactWorkgroupLdsReductionV1},
        std::pair{&ExactScopedAtomicV1,
                  PostLinkProfile::ExactScopedAtomicV1}}) {
    const std::set<std::string> Symbols = {Profile->Entry.str(),
                                           Profile->Descriptor.str()};
    if (ExpectedSymbols != Symbols)
      continue;
    if (!isClosedExactWorkgroupSyncRequest(RequestValue, *Profile))
      return pipelineError(Twine("exact ") + Profile->Check +
                           " symbols require the closed Worker V2 profile");
    return Kind;
  }
  const std::set<std::string> ExactMoeSymbols = {
      ExactMoeTop2V1Entry.str(), ExactMoeTop2V1Descriptor.str()};
  if (ExpectedSymbols == ExactMoeSymbols) {
    if (!isClosedExactMoeTop2V1Request(RequestValue))
      return pipelineError(
          "exact MoE top-2 symbols require the closed Worker V2 profile");
    return PostLinkProfile::ExactMoeTop2V1;
  }
  return PostLinkProfile::LegacyGfx942G1;
}

const ExactWorkgroupSyncProfile *
postLinkWorkgroupSyncProfile(PostLinkProfile Profile) {
  if (Profile == PostLinkProfile::ExactWorkgroupLdsReductionV1)
    return &ExactWorkgroupLdsReductionV1;
  if (Profile == PostLinkProfile::ExactScopedAtomicV1)
    return &ExactScopedAtomicV1;
  return nullptr;
}

TargetParts parseTarget(StringRef Target) {
  SmallVector<StringRef, 3> Components;
  Target.split(Components, ':', -1, false);
  TargetParts Result;
  Result.Cpu = Components.front().str();
  for (StringRef Component : drop_begin(Components)) {
    bool Enabled = Component.back() == '+';
    if (Component.drop_back() == "sramecc")
      Result.SramEcc = Enabled;
    else if (Component.drop_back() == "xnack")
      Result.Xnack = Enabled;
  }
  return Result;
}

std::string targetMachineFeatures(const TargetParts &Parts) {
  SmallString<32> Features;
  auto Append = [&](StringRef Name, std::optional<bool> Enabled) {
    if (!Enabled)
      return;
    if (!Features.empty())
      Features.push_back(',');
    Features.push_back(*Enabled ? '+' : '-');
    Features.append(Name);
  };
  Append("sramecc", Parts.SramEcc);
  Append("xnack", Parts.Xnack);
  return Features.str().str();
}

Expected<std::array<std::vector<uint8_t>, 5>>
parseExactWave64CompilerSections(StringRef Text) {
  constexpr StringLiteral Marker = "\nmodule asm \".section ";
  size_t BodyEnd = Text.find(Marker);
  if (BodyEnd == StringRef::npos)
    return pipelineError(
        "exact Wave64 compiler module is missing identity sections");
  if (SHA256::hash(arrayRefFromStringRef(Text.take_front(BodyEnd))) !=
      ExactWave64BodySha256)
    return pipelineError(
        "exact Wave64 compiler module body identity does not match");

  SmallVector<StringRef, 128> Lines;
  Text.drop_front(BodyEnd + 1).split(Lines, '\n', -1, true);
  static constexpr std::array Sections = {
      ExactWave64DescriptorSection, ExactWave64AuthoritySection,
      ExactWave64MirSection, ExactWave64KirSection, ExactWave64ProfileSection};
  std::array<std::vector<uint8_t>, Sections.size()> Result;
  size_t LineIndex = 0;
  for (size_t SectionIndex = 0; SectionIndex != Sections.size();
       ++SectionIndex) {
    if (SectionIndex != 0 && LineIndex != Lines.size() &&
        Lines[LineIndex].empty())
      ++LineIndex;
    std::string ExpectedHeader =
        (Twine("module asm \".section ") + Sections[SectionIndex] +
         ",\\22\\22,@progbits\"")
            .str();
    if (LineIndex == Lines.size() || Lines[LineIndex] != ExpectedHeader)
      return pipelineError(
          Twine(
              "exact Wave64 compiler module section order does not match at ") +
          Twine(SectionIndex) + " observed=" +
          (LineIndex == Lines.size() ? StringRef("<end>") : Lines[LineIndex]));
    ++LineIndex;
    if (LineIndex == Lines.size() ||
        Lines[LineIndex] != "module asm \".balign 8\"")
      return pipelineError(
          "exact Wave64 compiler module section alignment does not match");
    ++LineIndex;

    constexpr StringLiteral BytePrefix = "module asm \".byte ";
    while (LineIndex != Lines.size() &&
           Lines[LineIndex].starts_with(BytePrefix)) {
      StringRef Line = Lines[LineIndex++];
      if (!Line.ends_with("\""))
        return pipelineError(
            "exact Wave64 compiler module byte record is malformed");
      SmallVector<StringRef, 16> ByteAtoms;
      Line.drop_front(BytePrefix.size())
          .drop_back()
          .split(ByteAtoms, ',', -1, false);
      if (ByteAtoms.empty() || ByteAtoms.size() > 16)
        return pipelineError(
            "exact Wave64 compiler module byte record is noncanonical");
      for (StringRef Atom : ByteAtoms) {
        Atom = Atom.trim();
        if (!Atom.consume_front("0x") || Atom.size() != 2)
          return pipelineError(
              "exact Wave64 compiler module byte atom is malformed");
        uint8_t Byte = 0;
        if (Atom.getAsInteger(16, Byte))
          return pipelineError(
              "exact Wave64 compiler module byte atom is malformed");
        Result[SectionIndex].push_back(Byte);
      }
    }
    if (Result[SectionIndex].empty())
      return pipelineError("exact Wave64 compiler module section is empty");
  }
  if (LineIndex != Lines.size() &&
      !(LineIndex + 1 == Lines.size() && Lines[LineIndex].empty()))
    return pipelineError("exact Wave64 compiler module has trailing assembly");
  return Result;
}

Error validateExactWave64CompilerInput(StringRef Text) {
  auto Sections = parseExactWave64CompilerSections(Text);
  if (!Sections)
    return Sections.takeError();
  if ((*Sections)[0].size() > 64 * 1024)
    return pipelineError(
        "exact Wave64 compiler descriptor section is too large");
  if ((*Sections)[1].size() != 32 ||
      llvm::all_of((*Sections)[1], [](uint8_t Byte) { return Byte == 0; }))
    return pipelineError(
        "exact Wave64 compiler authority identity does not match");
  const std::array ExpectedIdentities = {
      ExactWave64MirSha256, ExactWave64KirSha256, ExactWave64ProfileSha256};
  for (size_t Index = 0; Index != ExpectedIdentities.size(); ++Index)
    if (ArrayRef((*Sections)[Index + 2]) != ArrayRef(ExpectedIdentities[Index]))
      return pipelineError(
          "exact Wave64 compiler/KIR profile identity does not match");
  return Error::success();
}

Expected<std::array<std::vector<uint8_t>, 4>>
parseExactRowSoftmaxV1CompilerSections(StringRef Text) {
  constexpr StringLiteral Marker = "\nmodule asm \".section ";
  size_t BodyEnd = Text.find(Marker);
  if (BodyEnd == StringRef::npos)
    return pipelineError(
        "exact row-softmax V1 compiler module is missing bound sections");
  if (SHA256::hash(arrayRefFromStringRef(Text.take_front(BodyEnd))) !=
      ExactRowBodySha256)
    return pipelineError(
        "exact row-softmax V1 compiler module body identity does not match");

  static constexpr std::array Sections = {
      ExactRowDescriptorSection, ExactRowTranscriptSection,
      ExactRowTranscriptDigestSection, ExactRowExpBoundarySection};
  SmallVector<StringRef, 256> Lines;
  Text.drop_front(BodyEnd + 1).split(Lines, '\n', -1, true);
  std::array<std::vector<uint8_t>, Sections.size()> Result;
  size_t LineIndex = 0;
  for (size_t SectionIndex = 0; SectionIndex != Sections.size();
       ++SectionIndex) {
    std::string ExpectedHeader =
        (Twine("module asm \".section ") + Sections[SectionIndex] +
         ",\\22\\22,@progbits\"")
            .str();
    if (LineIndex == Lines.size() || Lines[LineIndex] != ExpectedHeader)
      return pipelineError(
          Twine("exact row-softmax V1 compiler section order differs at ") +
          Twine(SectionIndex));
    ++LineIndex;
    if (LineIndex == Lines.size() ||
        Lines[LineIndex] != "module asm \".balign 8\"")
      return pipelineError(
          "exact row-softmax V1 compiler section alignment does not match");
    ++LineIndex;

    constexpr StringLiteral BytePrefix = "module asm \".byte ";
    SmallVector<size_t, 16> RecordWidths;
    while (LineIndex != Lines.size() &&
           Lines[LineIndex].starts_with(BytePrefix)) {
      StringRef Line = Lines[LineIndex++];
      if (!Line.ends_with("\""))
        return pipelineError(
            "exact row-softmax V1 compiler byte record is malformed");
      StringRef Payload = Line.drop_front(BytePrefix.size()).drop_back();
      size_t RecordWidth = 0;
      while (!Payload.empty()) {
        if (Payload.size() < 4 || Payload[0] != '0' || Payload[1] != 'x' ||
            !llvm::isHexDigit(Payload[2]) || !llvm::isHexDigit(Payload[3]) ||
            llvm::isUpper(Payload[2]) || llvm::isUpper(Payload[3]))
          return pipelineError(
              "exact row-softmax V1 compiler byte atom is malformed");
        StringRef Atom = Payload.take_front(4).drop_front(2);
        uint8_t Byte = 0;
        if (Atom.getAsInteger(16, Byte))
          return pipelineError(
              "exact row-softmax V1 compiler byte atom is malformed");
        Result[SectionIndex].push_back(Byte);
        ++RecordWidth;
        Payload = Payload.drop_front(4);
        if (Payload.empty())
          break;
        if (!Payload.consume_front(", "))
          return pipelineError(
              "exact row-softmax V1 compiler byte separator is noncanonical");
        if (Payload.empty())
          return pipelineError(
              "exact row-softmax V1 compiler byte separator is noncanonical");
        if (RecordWidth == 16)
          return pipelineError(
              "exact row-softmax V1 compiler byte record is noncanonical");
      }
      if (RecordWidth == 0 || RecordWidth > 16)
        return pipelineError(
            "exact row-softmax V1 compiler byte record is noncanonical");
      RecordWidths.push_back(RecordWidth);
    }
    if (Result[SectionIndex].empty())
      return pipelineError("exact row-softmax V1 compiler section is empty");
    for (size_t Index = 0; Index + 1 < RecordWidths.size(); ++Index)
      if (RecordWidths[Index] != 16)
        return pipelineError(
            "exact row-softmax V1 compiler byte chunking is noncanonical");
  }
  if (LineIndex + 1 != Lines.size() || !Lines[LineIndex].empty())
    return pipelineError(
        "exact row-softmax V1 compiler module has trailing assembly");
  return Result;
}

Error validateExactRowSoftmaxV1CompilerInput(StringRef Text,
                                             const DataLayout &Layout) {
  if (Layout.getStringRepresentation() != ExactRowSoftmaxV1ProducerDataLayout)
    return pipelineError(
        "exact row-softmax V1 worker target-machine layout does not match");
  auto Sections = parseExactRowSoftmaxV1CompilerSections(Text);
  if (!Sections)
    return Sections.takeError();
  if ((*Sections)[0].size() > 64 * 1024 || (*Sections)[1].size() > 4096)
    return pipelineError(
        "exact row-softmax V1 descriptor or transcript is oversized");
  std::array<uint8_t, 32> TranscriptIdentity = SHA256::hash((*Sections)[1]);
  if ((*Sections)[2].size() != 32 ||
      ArrayRef(TranscriptIdentity) != ArrayRef((*Sections)[2]))
    return pipelineError(
        "exact row-softmax V1 transcript digest is inconsistent");
  if (ArrayRef((*Sections)[3]) != ArrayRef(ExactRowExpBoundaryIdentity))
    return pipelineError(
        "exact row-softmax V1 exponential boundary identity does not match");
  return Error::success();
}

Error validateExactRowSoftmaxV1LlvmBuildIdentity(StringRef Identity) {
  if (Identity == ExactRowSoftmaxV1PublishedLlvmBuildIdentity)
    return Error::success();
  return pipelineError(
      Twine("exact row-softmax V1 requires LLVM build identity '") +
      ExactRowSoftmaxV1PublishedLlvmBuildIdentity + "', worker measured '" +
      Identity + "'");
}

Expected<std::array<std::vector<uint8_t>, 4>>
parseExactFlashAttentionCompilerSections(StringRef Text) {
  constexpr StringLiteral Marker = "\nmodule asm \".section ";
  size_t BodyEnd = Text.find(Marker);
  if (BodyEnd == StringRef::npos)
    return pipelineError(
        "exact FlashAttention compiler module is missing bound sections");
  if (SHA256::hash(arrayRefFromStringRef(Text.take_front(BodyEnd))) !=
      ExactFlashBodySha256)
    return pipelineError(
        "exact FlashAttention compiler module body identity does not match");

  static constexpr std::array Sections = {
      ExactFlashDescriptorSection, ExactFlashTranscriptSection,
      ExactFlashAuthoritySection, ExactFlashOcmlBoundarySection};
  SmallVector<StringRef, 256> Lines;
  Text.drop_front(BodyEnd + 1).split(Lines, '\n', -1, true);
  std::array<std::vector<uint8_t>, Sections.size()> Result;
  size_t LineIndex = 0;
  for (size_t SectionIndex = 0; SectionIndex != Sections.size();
       ++SectionIndex) {
    if (SectionIndex != 0 && LineIndex != Lines.size() &&
        Lines[LineIndex].empty())
      ++LineIndex;
    std::string ExpectedHeader =
        (Twine("module asm \".section ") + Sections[SectionIndex] +
         ",\\22\\22,@progbits\"")
            .str();
    if (LineIndex == Lines.size() || Lines[LineIndex] != ExpectedHeader)
      return pipelineError(
          Twine("exact FlashAttention compiler section order differs at ") +
          Twine(SectionIndex));
    ++LineIndex;
    if (LineIndex == Lines.size() ||
        Lines[LineIndex] != "module asm \".balign 8\"")
      return pipelineError(
          "exact FlashAttention compiler section alignment does not match");
    ++LineIndex;

    constexpr StringLiteral BytePrefix = "module asm \".byte ";
    while (LineIndex != Lines.size() &&
           Lines[LineIndex].starts_with(BytePrefix)) {
      StringRef Line = Lines[LineIndex++];
      if (!Line.ends_with("\""))
        return pipelineError(
            "exact FlashAttention compiler byte record is malformed");
      SmallVector<StringRef, 16> Atoms;
      Line.drop_front(BytePrefix.size())
          .drop_back()
          .split(Atoms, ',', -1, false);
      if (Atoms.empty() || Atoms.size() > 16)
        return pipelineError(
            "exact FlashAttention compiler byte record is noncanonical");
      for (StringRef Atom : Atoms) {
        Atom = Atom.trim();
        if (!Atom.consume_front("0x") || Atom.size() != 2)
          return pipelineError(
              "exact FlashAttention compiler byte atom is malformed");
        uint8_t Byte = 0;
        if (Atom.getAsInteger(16, Byte))
          return pipelineError(
              "exact FlashAttention compiler byte atom is malformed");
        Result[SectionIndex].push_back(Byte);
      }
    }
    if (Result[SectionIndex].empty())
      return pipelineError("exact FlashAttention compiler section is empty");
  }
  if (LineIndex != Lines.size() &&
      !(LineIndex + 1 == Lines.size() && Lines[LineIndex].empty()))
    return pipelineError(
        "exact FlashAttention compiler module has trailing assembly");
  return Result;
}

Error validateExactFlashAttentionCompilerInput(StringRef Text) {
  auto Sections = parseExactFlashAttentionCompilerSections(Text);
  if (!Sections)
    return Sections.takeError();
  if ((*Sections)[0].size() > 64 * 1024 || (*Sections)[1].size() > 4096)
    return pipelineError(
        "exact FlashAttention descriptor or authority transcript is oversized");
  if (ArrayRef((*Sections)[2]) != ArrayRef(ExactFlashAuthoritySha256) ||
      SHA256::hash((*Sections)[1]) != ExactFlashAuthoritySha256)
    return pipelineError(
        "exact FlashAttention authenticated authority does not match");
  if (ArrayRef((*Sections)[3]) != ArrayRef(ExactFlashOcmlBoundarySha256))
    return pipelineError(
        "exact FlashAttention OCML boundary identity does not match");
  return Error::success();
}

Error validateExactFlashAttentionLlvmBuildIdentity(StringRef Identity) {
  if (Identity == ExactFlashAttentionV1PublishedLlvmBuildIdentity)
    return Error::success();
  return pipelineError(
      Twine("exact FlashAttention V1 published machine identity requires LLVM "
            "build identity '") +
      ExactFlashAttentionV1PublishedLlvmBuildIdentity + "', worker measured '" +
      Identity + "'");
}

Expected<std::array<std::vector<uint8_t>, 13>>
parseExactWorkgroupSyncCompilerSections(
    StringRef Text, const ExactWorkgroupSyncProfile &Profile) {
  constexpr StringLiteral Marker = "\nmodule asm \".section ";
  size_t BodyEnd = Text.find(Marker);
  if (BodyEnd == StringRef::npos)
    return pipelineError("exact workgroup-sync compiler module is missing "
                         "identity sections");
  if (SHA256::hash(arrayRefFromStringRef(Text.take_front(BodyEnd))) !=
      Profile.BodySha256)
    return pipelineError(
        "exact workgroup-sync compiler module body identity does not match");

  const std::array<std::string, 13> Sections = {
      ExactWave64DescriptorSection.str(),
      (Twine(Profile.SectionPrefix) + ".source.v1").str(),
      (Twine(Profile.SectionPrefix) + ".namespace.v1").str(),
      (Twine(Profile.SectionPrefix) + ".authority.v1").str(),
      (Twine(Profile.SectionPrefix) + ".mir.v1").str(),
      (Twine(Profile.SectionPrefix) + ".fnabi.v1").str(),
      (Twine(Profile.SectionPrefix) + ".semantics.v1").str(),
      (Twine(Profile.SectionPrefix) + ".terminals.v3").str(),
      (Twine(Profile.SectionPrefix) + ".abi.v1").str(),
      (Twine(Profile.SectionPrefix) + ".effects.v1").str(),
      (Twine(Profile.SectionPrefix) + ".resources.v1").str(),
      (Twine(Profile.SectionPrefix) + ".kir.v1").str(),
      (Twine(Profile.SectionPrefix) + ".layout.v1").str()};
  SmallVector<StringRef, 160> Lines;
  Text.drop_front(BodyEnd + 1).split(Lines, '\n', -1, true);
  std::array<std::vector<uint8_t>, Sections.size()> Result;
  size_t LineIndex = 0;
  for (size_t SectionIndex = 0; SectionIndex != Sections.size();
       ++SectionIndex) {
    if (SectionIndex != 0 && LineIndex != Lines.size() &&
        Lines[LineIndex].empty())
      ++LineIndex;
    std::string ExpectedHeader =
        (Twine("module asm \".section ") + Sections[SectionIndex] +
         ",\\22\\22,@progbits\"")
            .str();
    if (LineIndex == Lines.size() || Lines[LineIndex] != ExpectedHeader)
      return pipelineError(
          Twine("exact workgroup-sync compiler module section order does "
                "not match at ") +
          Twine(SectionIndex));
    ++LineIndex;
    if (LineIndex == Lines.size() ||
        Lines[LineIndex] != "module asm \".balign 8\"")
      return pipelineError(
          "exact workgroup-sync compiler module section alignment does not "
          "match");
    ++LineIndex;

    constexpr StringLiteral BytePrefix = "module asm \".byte ";
    while (LineIndex != Lines.size() &&
           Lines[LineIndex].starts_with(BytePrefix)) {
      StringRef Line = Lines[LineIndex++];
      if (!Line.ends_with("\""))
        return pipelineError(
            "exact workgroup-sync compiler module byte record is malformed");
      SmallVector<StringRef, 16> ByteAtoms;
      Line.drop_front(BytePrefix.size())
          .drop_back()
          .split(ByteAtoms, ',', -1, false);
      if (ByteAtoms.empty() || ByteAtoms.size() > 16)
        return pipelineError(
            "exact workgroup-sync compiler module byte record is "
            "noncanonical");
      for (StringRef Atom : ByteAtoms) {
        Atom = Atom.trim();
        if (!Atom.consume_front("0x") || Atom.size() != 2)
          return pipelineError(
              "exact workgroup-sync compiler module byte atom is malformed");
        uint8_t Byte = 0;
        if (Atom.getAsInteger(16, Byte))
          return pipelineError(
              "exact workgroup-sync compiler module byte atom is malformed");
        Result[SectionIndex].push_back(Byte);
      }
    }
    if (Result[SectionIndex].empty())
      return pipelineError(
          "exact workgroup-sync compiler module section is empty");
  }
  if (LineIndex != Lines.size() &&
      !(LineIndex + 1 == Lines.size() && Lines[LineIndex].empty()))
    return pipelineError(
        "exact workgroup-sync compiler module has trailing assembly");
  return Result;
}

Error validateExactWorkgroupSyncCompilerInput(
    StringRef Text, const ExactWorkgroupSyncProfile &Profile,
    const DataLayout &ExpectedLayout) {
  auto Sections = parseExactWorkgroupSyncCompilerSections(Text, Profile);
  if (!Sections)
    return Sections.takeError();
  if ((*Sections)[0].size() > 64 * 1024)
    return pipelineError(
        "exact workgroup-sync compiler descriptor section is too large");
  for (size_t Index = 0; Index != Profile.SectionIdentities.size(); ++Index)
    if (ArrayRef((*Sections)[Index + 1]) !=
        ArrayRef(Profile.SectionIdentities[Index]))
      return pipelineError(
          "exact workgroup-sync source/KIR/profile identity does not match");
  std::array<uint8_t, 32> ExpectedLayoutIdentity = SHA256::hash(
      arrayRefFromStringRef(ExpectedLayout.getStringRepresentation()));
  if (ArrayRef((*Sections).back()) != ArrayRef(ExpectedLayoutIdentity))
    return pipelineError(
        "exact workgroup-sync target-machine data-layout identity does not "
        "match");
  return Error::success();
}

Expected<std::array<std::vector<uint8_t>, 17>>
parseExactMoeTop2V1CompilerSections(StringRef Text) {
  constexpr StringLiteral Marker = "\nmodule asm \".section ";
  size_t BodyEnd = Text.find(Marker);
  if (BodyEnd == StringRef::npos)
    return pipelineError(
        "exact MoE top-2 compiler module is missing identity sections");
  if (!bytesMatchLowerHex(
          SHA256::hash(arrayRefFromStringRef(Text.take_front(BodyEnd))),
          ExactMoeTop2V1BodySha256))
    return pipelineError(
        "exact MoE top-2 compiler module body identity does not match");

  std::array<std::string, 17> Sections;
  Sections[0] = ExactWave64DescriptorSection.str();
  for (size_t Index = 0; Index != ExactMoeTop2V1Sections.size(); ++Index)
    Sections[Index + 1] = ExactMoeTop2V1Sections[Index].str();
  SmallVector<StringRef, 224> Lines;
  Text.drop_front(BodyEnd + 1).split(Lines, '\n', -1, true);
  std::array<std::vector<uint8_t>, Sections.size()> Result;
  size_t LineIndex = 0;
  for (size_t SectionIndex = 0; SectionIndex != Sections.size();
       ++SectionIndex) {
    if (SectionIndex != 0 && LineIndex != Lines.size() &&
        Lines[LineIndex].empty())
      ++LineIndex;
    std::string ExpectedHeader =
        (Twine("module asm \".section ") + Sections[SectionIndex] +
         ",\\22\\22,@progbits\"")
            .str();
    if (LineIndex == Lines.size() || Lines[LineIndex] != ExpectedHeader)
      return pipelineError(
          Twine("exact MoE top-2 compiler module section order does not "
                "match at ") +
          Twine(SectionIndex));
    ++LineIndex;
    if (LineIndex == Lines.size() ||
        Lines[LineIndex] != "module asm \".balign 8\"")
      return pipelineError(
          "exact MoE top-2 compiler module section alignment does not match");
    ++LineIndex;

    constexpr StringLiteral BytePrefix = "module asm \".byte ";
    while (LineIndex != Lines.size() &&
           Lines[LineIndex].starts_with(BytePrefix)) {
      StringRef Line = Lines[LineIndex++];
      if (!Line.ends_with("\""))
        return pipelineError(
            "exact MoE top-2 compiler module byte record is malformed");
      SmallVector<StringRef, 16> ByteAtoms;
      Line.drop_front(BytePrefix.size())
          .drop_back()
          .split(ByteAtoms, ',', -1, false);
      if (ByteAtoms.empty() || ByteAtoms.size() > 16)
        return pipelineError(
            "exact MoE top-2 compiler module byte record is noncanonical");
      for (StringRef Atom : ByteAtoms) {
        Atom = Atom.trim();
        if (!Atom.consume_front("0x") || Atom.size() != 2)
          return pipelineError(
              "exact MoE top-2 compiler module byte atom is malformed");
        uint8_t Byte = 0;
        if (Atom.getAsInteger(16, Byte))
          return pipelineError(
              "exact MoE top-2 compiler module byte atom is malformed");
        Result[SectionIndex].push_back(Byte);
      }
    }
    if (Result[SectionIndex].empty())
      return pipelineError("exact MoE top-2 compiler module section is empty");
  }
  if (LineIndex != Lines.size() &&
      !(LineIndex + 1 == Lines.size() && Lines[LineIndex].empty()))
    return pipelineError(
        "exact MoE top-2 compiler module has trailing assembly");
  return Result;
}

Error validateExactMoeTop2V1CompilerInput(StringRef Text,
                                          const DataLayout &ExpectedLayout) {
  auto Sections = parseExactMoeTop2V1CompilerSections(Text);
  if (!Sections)
    return Sections.takeError();
  if ((*Sections)[0].size() > 64 * 1024)
    return pipelineError(
        "exact MoE top-2 compiler descriptor section is too large");
  for (size_t Index = 0; Index != ExactMoeTop2V1Identities.size(); ++Index)
    if (!bytesMatchLowerHex((*Sections)[Index + 1],
                            ExactMoeTop2V1Identities[Index]))
      return pipelineError(
          "exact MoE top-2 source/KIR/compiler/profile identity does not "
          "match");
  std::array<uint8_t, 32> ExpectedLayoutIdentity = SHA256::hash(
      arrayRefFromStringRef(ExpectedLayout.getStringRepresentation()));
  if (ArrayRef((*Sections).back()) != ArrayRef(ExpectedLayoutIdentity))
    return pipelineError(
        "exact MoE top-2 target-machine data-layout identity does not match");
  return Error::success();
}

Error validateExactMoeTop2V1Module(const Module &ModuleValue,
                                   const DataLayout &ExpectedLayout) {
  if (ModuleValue.getTargetTriple().getTriple() != AmdGpuTriple ||
      ModuleValue.getDataLayoutStr() !=
          ExpectedLayout.getStringRepresentation() ||
      ModuleValue.global_begin() != ModuleValue.global_end())
    return pipelineError("exact MoE top-2 LLVM module envelope does not match");

  static constexpr std::array<StringLiteral, 5> HelperNames = {
      "__fe2o3_moe_select_expert_v1", "__fe2o3_moe_requested_count_v1",
      "__fe2o3_moe_admitted_count_v1", "__fe2o3_moe_expert_offset_v1",
      "__fe2o3_moe_route_slot_v1"};
  const std::set<std::string> ExpectedHelpers = {
      HelperNames[0].str(), HelperNames[1].str(), HelperNames[2].str(),
      HelperNames[3].str(), HelperNames[4].str()};
  const std::set<std::string> ExpectedDeclarations = {
      "llvm.amdgcn.workitem.id.x", "llvm.trap"};
  std::set<std::string> Helpers;
  std::set<std::string> Declarations;
  const Function *Kernel = nullptr;
  for (const Function &FunctionValue : ModuleValue) {
    if (FunctionValue.isDeclaration()) {
      Declarations.insert(FunctionValue.getName().str());
      continue;
    }
    if (FunctionValue.getName() == ExactMoeTop2V1Entry) {
      if (Kernel)
        return pipelineError(
            "exact MoE top-2 LLVM kernel cardinality does not match");
      Kernel = &FunctionValue;
      continue;
    }
    if (!FunctionValue.hasInternalLinkage() ||
        !FunctionValue.hasFnAttribute(Attribute::AlwaysInline) ||
        !FunctionValue.onlyReadsMemory())
      return pipelineError(
          "exact MoE top-2 LLVM helper attributes do not match");
    Helpers.insert(FunctionValue.getName().str());
  }
  if (!Kernel || Helpers != ExpectedHelpers ||
      Declarations != ExpectedDeclarations ||
      Kernel->getCallingConv() != CallingConv::AMDGPU_KERNEL ||
      !Kernel->getReturnType()->isVoidTy() || Kernel->isVarArg() ||
      Kernel->arg_size() != 16)
    return pipelineError(
        "exact MoE top-2 LLVM function closure does not match");

  static constexpr std::array<StringLiteral, 16> ArgumentNames = {
      "logits.data",      "logits.len",      "top2.data",     "top2.len",
      "requested.data",   "requested.len",   "admitted.data", "admitted.len",
      "offsets.data",     "offsets.len",     "slots.data",    "slots.len",
      "permutation.data", "permutation.len", "inverse.data",  "inverse.len"};
  size_t ArgumentIndex = 0;
  for (const Argument &ArgumentValue : Kernel->args()) {
    Type *TypeValue = ArgumentValue.getType();
    bool IsPointer = ArgumentIndex % 2 == 0;
    if (ArgumentValue.getName() != ArgumentNames[ArgumentIndex] ||
        (IsPointer ? !TypeValue->isPointerTy() ||
                         TypeValue->getPointerAddressSpace() != 1
                   : !TypeValue->isIntegerTy(64)))
      return pipelineError("exact MoE top-2 LLVM argument ABI does not match");
    ++ArgumentIndex;
  }
  if (Kernel->getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
      Kernel->getFnAttribute("target-features").getValueAsString() !=
          "-wavefrontsize32,+wavefrontsize64,-xnack" ||
      Kernel->getFnAttribute("amdgpu-flat-work-group-size")
              .getValueAsString() != "64,64")
    return pipelineError("exact MoE top-2 LLVM target attributes do not match");
  MDNode *Workgroup = Kernel->getMetadata("reqd_work_group_size");
  static constexpr std::array<uint64_t, 3> WorkgroupShape = {64, 1, 1};
  if (!Workgroup || Workgroup->getNumOperands() != WorkgroupShape.size())
    return pipelineError(
        "exact MoE top-2 LLVM workgroup metadata does not match");
  for (size_t Index = 0; Index != WorkgroupShape.size(); ++Index) {
    auto *Value =
        mdconst::dyn_extract<ConstantInt>(Workgroup->getOperand(Index));
    if (!Value || Value->getZExtValue() != WorkgroupShape[Index])
      return pipelineError(
          "exact MoE top-2 LLVM workgroup metadata does not match");
  }

  size_t Loads = 0;
  size_t Stores = 0;
  size_t OrderedFloatComparisons = 0;
  size_t WorkitemIds = 0;
  size_t Traps = 0;
  for (const Function &FunctionValue : ModuleValue) {
    if (FunctionValue.isDeclaration())
      continue;
    for (const Instruction &InstructionValue : instructions(FunctionValue)) {
      if (isa<AllocaInst, FenceInst, AtomicRMWInst, AtomicCmpXchgInst>(
              InstructionValue))
        return pipelineError(
            "exact MoE top-2 LLVM forbidden memory effect is present");
      if (const auto *Load = dyn_cast<LoadInst>(&InstructionValue)) {
        if (Load->getPointerAddressSpace() != 1 || Load->isAtomic() ||
            Load->getAlign() != Align(4))
          return pipelineError(
              "exact MoE top-2 LLVM load effect does not match");
        ++Loads;
      }
      if (const auto *Store = dyn_cast<StoreInst>(&InstructionValue)) {
        if (Store->getPointerAddressSpace() != 1 || Store->isAtomic() ||
            Store->getAlign() != Align(4))
          return pipelineError(
              "exact MoE top-2 LLVM store effect does not match");
        ++Stores;
      }
      if (const auto *Compare = dyn_cast<FCmpInst>(&InstructionValue)) {
        if (Compare->getPredicate() != CmpInst::FCMP_OGT)
          return pipelineError(
              "exact MoE top-2 LLVM floating comparison does not match");
        ++OrderedFloatComparisons;
      }
      const auto *Call = dyn_cast<CallBase>(&InstructionValue);
      if (!Call)
        continue;
      const Function *Callee = Call->getCalledFunction();
      if (!Callee)
        return pipelineError("exact MoE top-2 LLVM has an indirect call");
      StringRef Name = Callee->getName();
      if (Name == "llvm.amdgcn.workitem.id.x")
        ++WorkitemIds;
      else if (Name == "llvm.trap")
        ++Traps;
      else if (!ExpectedHelpers.contains(Name.str()))
        return pipelineError(
            "exact MoE top-2 LLVM call closure does not match");
    }
  }
  if (Loads != 5 || Stores != 7 || OrderedFloatComparisons != 6 ||
      WorkitemIds != 1 || Traps != 1)
    return pipelineError("exact MoE top-2 LLVM effect closure does not match");
  return Error::success();
}

Error validateExactWorkgroupSyncModule(const Module &ModuleValue,
                                       const ExactWorkgroupSyncProfile &Profile,
                                       const DataLayout &ExpectedLayout) {
  if (ModuleValue.getTargetTriple().getTriple() != AmdGpuTriple ||
      ModuleValue.getDataLayoutStr() !=
          ExpectedLayout.getStringRepresentation())
    return pipelineError(
        "exact workgroup-sync LLVM module envelope does not match");

  const Function *Kernel = nullptr;
  std::set<std::string> Declarations;
  for (const Function &FunctionValue : ModuleValue) {
    if (FunctionValue.isDeclaration()) {
      Declarations.insert(FunctionValue.getName().str());
      continue;
    }
    if (Kernel)
      return pipelineError(
          "exact workgroup-sync LLVM module has multiple definitions");
    Kernel = &FunctionValue;
  }
  const std::set<std::string> ExpectedDeclarations =
      Profile.Kind == ExactWorkgroupSyncKind::LdsReduction
          ? std::set<std::string>{"llvm.amdgcn.workitem.id.x",
                                  "llvm.amdgcn.s.barrier", "llvm.trap"}
          : std::set<std::string>{"llvm.amdgcn.workitem.id.x", "llvm.trap"};
  if (!Kernel || Kernel->getName() != Profile.Entry ||
      Kernel->getCallingConv() != CallingConv::AMDGPU_KERNEL ||
      !Kernel->getReturnType()->isVoidTy() || Kernel->isVarArg() ||
      Kernel->arg_size() != 5 || Declarations != ExpectedDeclarations)
    return pipelineError(
        "exact workgroup-sync LLVM function closure does not match");

  const std::array<unsigned, 5> AddressSpaces =
      Profile.Kind == ExactWorkgroupSyncKind::LdsReduction
          ? std::array<unsigned, 5>{1, 0, 0, 1, 0}
          : std::array<unsigned, 5>{1, 0, 1, 0, 0};
  const std::array<StringRef, 5> Names =
      Profile.Kind == ExactWorkgroupSyncKind::LdsReduction
          ? std::array<StringRef, 5>{"values.data", "values.len", "epoch",
                                     "output.data", "output.len"}
          : std::array<StringRef, 5>{"values.data", "values.len",
                                     "eligible.data", "eligible.len",
                                     "target.address"};
  size_t ArgumentIndex = 0;
  for (const Argument &ArgumentValue : Kernel->args()) {
    Type *TypeValue = ArgumentValue.getType();
    const bool IsLdsEpoch =
        Profile.Kind == ExactWorkgroupSyncKind::LdsReduction &&
        ArgumentIndex == 2;
    if (ArgumentValue.getName() != Names[ArgumentIndex] ||
        (AddressSpaces[ArgumentIndex] == 1
             ? !TypeValue->isPointerTy() ||
                   TypeValue->getPointerAddressSpace() != 1
         : IsLdsEpoch ? !TypeValue->isIntegerTy(32)
                      : !TypeValue->isIntegerTy(64)))
      return pipelineError(
          "exact workgroup-sync LLVM argument ABI does not match");
    ++ArgumentIndex;
  }

  if (Kernel->getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
      Kernel->getFnAttribute("target-features").getValueAsString() !=
          "-wavefrontsize32,+wavefrontsize64,-xnack" ||
      Kernel->getFnAttribute("amdgpu-flat-work-group-size")
              .getValueAsString() != "64,64")
    return pipelineError(
        "exact workgroup-sync LLVM target attributes do not match");
  MDNode *Workgroup = Kernel->getMetadata("reqd_work_group_size");
  if (!Workgroup || Workgroup->getNumOperands() != 3)
    return pipelineError(
        "exact workgroup-sync LLVM workgroup metadata does not match");
  static constexpr std::array<uint64_t, 3> WorkgroupShape = {64, 1, 1};
  for (size_t Index = 0; Index != WorkgroupShape.size(); ++Index) {
    auto *Value =
        mdconst::dyn_extract<ConstantInt>(Workgroup->getOperand(Index));
    if (!Value || Value->getZExtValue() != WorkgroupShape[Index])
      return pipelineError(
          "exact workgroup-sync LLVM workgroup metadata does not match");
  }

  size_t WorkitemIds = 0;
  size_t Traps = 0;
  size_t Barriers = 0;
  size_t GlobalLoads = 0;
  size_t GlobalStores = 0;
  size_t LdsLoads = 0;
  size_t LdsStores = 0;
  size_t WorkgroupReleaseFences = 0;
  size_t WorkgroupAcquireFences = 0;
  size_t AtomicAdds = 0;
  size_t GlobalIntToPointers = 0;
  SyncScope::ID WorkgroupScope =
      ModuleValue.getContext().getOrInsertSyncScopeID("workgroup");
  for (const Instruction &InstructionValue : instructions(*Kernel)) {
    if (const auto *Load = dyn_cast<LoadInst>(&InstructionValue)) {
      if (!Load->getType()->isIntegerTy(32) || Load->getAlign() != Align(4) ||
          Load->isAtomic())
        return pipelineError(
            "exact workgroup-sync LLVM load effect does not match");
      if (Load->getPointerAddressSpace() == 1)
        ++GlobalLoads;
      else if (Load->getPointerAddressSpace() == 3)
        ++LdsLoads;
      else
        return pipelineError(
            "exact workgroup-sync LLVM load address space does not match");
    }
    if (const auto *Store = dyn_cast<StoreInst>(&InstructionValue)) {
      if (!Store->getValueOperand()->getType()->isIntegerTy(32) ||
          Store->getAlign() != Align(4) || Store->isAtomic())
        return pipelineError(
            "exact workgroup-sync LLVM store effect does not match");
      if (Store->getPointerAddressSpace() == 1)
        ++GlobalStores;
      else if (Store->getPointerAddressSpace() == 3)
        ++LdsStores;
      else
        return pipelineError(
            "exact workgroup-sync LLVM store address space does not match");
    }
    if (const auto *Fence = dyn_cast<FenceInst>(&InstructionValue)) {
      if (Fence->getSyncScopeID() != WorkgroupScope)
        return pipelineError(
            "exact workgroup-sync LLVM fence scope does not match");
      if (Fence->getOrdering() == AtomicOrdering::Release)
        ++WorkgroupReleaseFences;
      else if (Fence->getOrdering() == AtomicOrdering::Acquire)
        ++WorkgroupAcquireFences;
      else
        return pipelineError(
            "exact workgroup-sync LLVM fence ordering does not match");
    }
    if (const auto *Atomic = dyn_cast<AtomicRMWInst>(&InstructionValue)) {
      if (Atomic->getOperation() != AtomicRMWInst::Add ||
          !Atomic->getValOperand()->getType()->isIntegerTy(32) ||
          Atomic->getPointerAddressSpace() != 1 ||
          Atomic->getAlign() != Align(4) ||
          Atomic->getOrdering() != AtomicOrdering::Monotonic ||
          Atomic->getSyncScopeID() != SyncScope::System)
        return pipelineError(
            "exact scoped-atomic operation/order/scope/address space does "
            "not match");
      ++AtomicAdds;
    }
    if (const auto *Cast = dyn_cast<IntToPtrInst>(&InstructionValue)) {
      if (!Cast->getOperand(0)->getType()->isIntegerTy(64) ||
          !Cast->getType()->isPointerTy() ||
          Cast->getType()->getPointerAddressSpace() != 1)
        return pipelineError(
            "exact scoped-atomic pointer conversion does not match");
      ++GlobalIntToPointers;
    }
    const auto *Call = dyn_cast<CallBase>(&InstructionValue);
    if (!Call)
      continue;
    const Function *Callee = Call->getCalledFunction();
    if (!Callee)
      return pipelineError(
          "exact workgroup-sync LLVM module has an indirect call");
    if (Callee->getName() == "llvm.amdgcn.workitem.id.x")
      ++WorkitemIds;
    else if (Callee->getName() == "llvm.trap")
      ++Traps;
    else if (Callee->getName() == "llvm.amdgcn.s.barrier")
      ++Barriers;
    else
      return pipelineError(
          "exact workgroup-sync LLVM call closure does not match");
  }

  if (Profile.Kind == ExactWorkgroupSyncKind::LdsReduction) {
    const GlobalVariable *Scratch =
        ModuleValue.getNamedGlobal(ExactWorkgroupLdsReductionV1Scratch);
    const auto *Array =
        Scratch ? dyn_cast<ArrayType>(Scratch->getValueType()) : nullptr;
    if (!Scratch || Scratch->getAddressSpace() != 3 ||
        !Scratch->isDeclaration() || !Array || Array->getNumElements() != 0 ||
        !Array->getElementType()->isIntegerTy(32) ||
        Scratch->getAlign() != Align(4) ||
        std::distance(ModuleValue.global_begin(), ModuleValue.global_end()) !=
            1 ||
        WorkitemIds != 1 || Traps != 1 || Barriers != 2 || GlobalLoads != 1 ||
        GlobalStores != 1 || LdsLoads != 1 || LdsStores != 1 ||
        WorkgroupReleaseFences != 2 || WorkgroupAcquireFences != 2 ||
        AtomicAdds != 0 || GlobalIntToPointers != 0)
      return pipelineError(
          "exact LDS allocation/epoch/barrier operation closure does not "
          "match");
  } else if (ModuleValue.global_begin() != ModuleValue.global_end() ||
             WorkitemIds != 1 || Traps != 1 || Barriers != 0 ||
             GlobalLoads != 2 || GlobalStores != 0 || LdsLoads != 0 ||
             LdsStores != 0 || WorkgroupReleaseFences != 0 ||
             WorkgroupAcquireFences != 0 || AtomicAdds != 1 ||
             GlobalIntToPointers != 1) {
    return pipelineError("exact scoped-atomic effect closure does not match");
  }
  return Error::success();
}

Error validateExactWave64CollectivesModule(const Module &ModuleValue) {
  if (ModuleValue.getTargetTriple().getTriple() != AmdGpuTriple ||
      ModuleValue.getDataLayoutStr() != ExactLdsGemmSlice1ProducerDataLayout ||
      ModuleValue.global_begin() != ModuleValue.global_end())
    return pipelineError("exact Wave64 LLVM module envelope does not match");

  const Function *Kernel = nullptr;
  std::set<std::string> Declarations;
  for (const Function &FunctionValue : ModuleValue) {
    if (FunctionValue.isDeclaration()) {
      Declarations.insert(FunctionValue.getName().str());
      continue;
    }
    if (Kernel)
      return pipelineError("exact Wave64 LLVM module has multiple definitions");
    Kernel = &FunctionValue;
  }
  const std::set<std::string> ExpectedDeclarations = {
      "llvm.amdgcn.workitem.id.x", "llvm.amdgcn.ds.bpermute", "llvm.trap"};
  if (!Kernel || Kernel->getName() != ExactWave64CollectivesV1Entry ||
      Kernel->getCallingConv() != CallingConv::AMDGPU_KERNEL ||
      Kernel->getReturnType() != Type::getVoidTy(ModuleValue.getContext()) ||
      Kernel->isVarArg() || Kernel->arg_size() != 9 ||
      Declarations != ExpectedDeclarations)
    return pipelineError("exact Wave64 LLVM function closure does not match");

  static constexpr std::array<unsigned, 9> AddressSpaces = {1, 0, 0, 1, 0,
                                                            1, 0, 1, 0};
  static constexpr std::array<StringLiteral, 9> ArgumentNames = {
      "input.data",           "input.len",
      "active_mask",          "reduction_output.data",
      "reduction_output.len", "inclusive_output.data",
      "inclusive_output.len", "exclusive_output.data",
      "exclusive_output.len"};
  size_t ArgumentIndex = 0;
  for (const Argument &ArgumentValue : Kernel->args()) {
    Type *ArgumentType = ArgumentValue.getType();
    if (ArgumentValue.getName() != ArgumentNames[ArgumentIndex] ||
        (AddressSpaces[ArgumentIndex] == 0
             ? !ArgumentType->isIntegerTy(64)
             : !ArgumentType->isPointerTy() ||
                   ArgumentType->getPointerAddressSpace() !=
                       AddressSpaces[ArgumentIndex]))
      return pipelineError("exact Wave64 LLVM argument ABI does not match");
    ++ArgumentIndex;
  }

  if (Kernel->getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
      Kernel->getFnAttribute("target-features").getValueAsString() !=
          "-wavefrontsize32,+wavefrontsize64,-xnack" ||
      Kernel->getFnAttribute("amdgpu-flat-work-group-size")
              .getValueAsString() != "64,64" ||
      Kernel->getFnAttribute("fp-contract").getValueAsString() != "off")
    return pipelineError("exact Wave64 LLVM target attributes do not match");
  MDNode *Workgroup = Kernel->getMetadata("reqd_work_group_size");
  if (!Workgroup || Workgroup->getNumOperands() != 3)
    return pipelineError("exact Wave64 LLVM workgroup metadata does not match");
  static constexpr std::array<uint64_t, 3> WorkgroupShape = {64, 1, 1};
  for (size_t Index = 0; Index != WorkgroupShape.size(); ++Index) {
    auto *Value =
        mdconst::dyn_extract<ConstantInt>(Workgroup->getOperand(Index));
    if (!Value || Value->getZExtValue() != WorkgroupShape[Index])
      return pipelineError(
          "exact Wave64 LLVM workgroup metadata does not match");
  }

  size_t FAdds = 0;
  size_t Loads = 0;
  size_t Stores = 0;
  size_t BPermutes = 0;
  size_t WorkitemIds = 0;
  size_t Traps = 0;
  for (const Instruction &InstructionValue : instructions(*Kernel)) {
    if (InstructionValue.getOpcode() == Instruction::FAdd) {
      ++FAdds;
      if (cast<BinaryOperator>(InstructionValue).getFastMathFlags().any())
        return pipelineError(
            "exact Wave64 LLVM floating-point policy does not match");
    }
    if (const auto *Load = dyn_cast<LoadInst>(&InstructionValue)) {
      ++Loads;
      if (!Load->getType()->isFloatTy() || Load->getAlign() != Align(4) ||
          Load->getPointerAddressSpace() != 1 || Load->isAtomic())
        return pipelineError("exact Wave64 LLVM load effect does not match");
    }
    if (const auto *Store = dyn_cast<StoreInst>(&InstructionValue)) {
      ++Stores;
      if (!Store->getValueOperand()->getType()->isFloatTy() ||
          Store->getAlign() != Align(4) ||
          Store->getPointerAddressSpace() != 1 || Store->isAtomic())
        return pipelineError("exact Wave64 LLVM store effect does not match");
    }
    if (isa<AtomicRMWInst, AtomicCmpXchgInst, FenceInst, AllocaInst>(
            InstructionValue))
      return pipelineError(
          "exact Wave64 LLVM module contains a forbidden memory effect");
    const auto *Call = dyn_cast<CallBase>(&InstructionValue);
    if (!Call)
      continue;
    const Function *Callee = Call->getCalledFunction();
    if (!Callee)
      return pipelineError("exact Wave64 LLVM module has an indirect call");
    if (Callee->getName() == "llvm.amdgcn.ds.bpermute")
      ++BPermutes;
    else if (Callee->getName() == "llvm.amdgcn.workitem.id.x")
      ++WorkitemIds;
    else if (Callee->getName() == "llvm.trap")
      ++Traps;
    else
      return pipelineError("exact Wave64 LLVM call closure does not match");
  }
  if (FAdds != 12 || Loads != 1 || Stores != 3 || BPermutes != 13 ||
      WorkitemIds != 1 || Traps != 1)
    return pipelineError(
        "exact Wave64 LLVM collective operation closure does not match");
  return Error::success();
}

Error validateExactFlashAttentionModule(const Module &ModuleValue) {
  if (ModuleValue.getTargetTriple().getTriple() != AmdGpuTriple ||
      ModuleValue.getDataLayoutStr() != ExactLdsGemmSlice1ProducerDataLayout ||
      ModuleValue.global_begin() != ModuleValue.global_end())
    return pipelineError(
        "exact FlashAttention LLVM module envelope does not match");

  const Function *Kernel = nullptr;
  std::set<std::string> Declarations;
  for (const Function &FunctionValue : ModuleValue) {
    if (FunctionValue.isDeclaration()) {
      Declarations.insert(FunctionValue.getName().str());
      continue;
    }
    if (Kernel)
      return pipelineError(
          "exact FlashAttention LLVM module has multiple definitions");
    Kernel = &FunctionValue;
  }
  const std::set<std::string> ExpectedDeclarations = {
      "__ocml_exp_f32", "llvm.amdgcn.workitem.id.x", "llvm.trap"};
  if (!Kernel || Kernel->getName() != ExactFlashAttentionV1Entry ||
      Kernel->getCallingConv() != CallingConv::AMDGPU_KERNEL ||
      !Kernel->getReturnType()->isVoidTy() || Kernel->isVarArg() ||
      Kernel->arg_size() != 8 || Declarations != ExpectedDeclarations)
    return pipelineError(
        "exact FlashAttention LLVM function closure does not match");

  static constexpr std::array<unsigned, 8> AddressSpaces = {1, 0, 1, 0,
                                                            1, 0, 1, 0};
  static constexpr std::array<StringLiteral, 8> ArgumentNames = {
      "q.data", "q.len", "k.data",      "k.len",
      "v.data", "v.len", "output.data", "output.len"};
  size_t ArgumentIndex = 0;
  for (const Argument &ArgumentValue : Kernel->args()) {
    Type *ArgumentType = ArgumentValue.getType();
    if (ArgumentValue.getName() != ArgumentNames[ArgumentIndex] ||
        (AddressSpaces[ArgumentIndex] == 0
             ? !ArgumentType->isIntegerTy(64)
             : !ArgumentType->isPointerTy() ||
                   ArgumentType->getPointerAddressSpace() !=
                       AddressSpaces[ArgumentIndex]))
      return pipelineError(
          "exact FlashAttention LLVM argument ABI does not match");
    ++ArgumentIndex;
  }
  if (!Kernel->getArg(6)->hasAttribute(Attribute::NoAlias) ||
      Kernel->getArg(0)->hasAttribute(Attribute::NoAlias) ||
      Kernel->getArg(2)->hasAttribute(Attribute::NoAlias) ||
      Kernel->getArg(4)->hasAttribute(Attribute::NoAlias))
    return pipelineError(
        "exact FlashAttention LLVM alias policy does not match");

  if (Kernel->getFnAttribute("target-cpu").getValueAsString() != "gfx942" ||
      Kernel->getFnAttribute("target-features").getValueAsString() !=
          "-wavefrontsize32,+wavefrontsize64,-xnack" ||
      Kernel->getFnAttribute("amdgpu-flat-work-group-size")
              .getValueAsString() != "64,64" ||
      Kernel->getFnAttribute("fp-contract").getValueAsString() != "off")
    return pipelineError(
        "exact FlashAttention LLVM target attributes do not match");
  MDNode *Workgroup = Kernel->getMetadata("reqd_work_group_size");
  static constexpr std::array<uint64_t, 3> WorkgroupShape = {64, 1, 1};
  if (!Workgroup || Workgroup->getNumOperands() != WorkgroupShape.size())
    return pipelineError(
        "exact FlashAttention LLVM workgroup metadata does not match");
  for (size_t Index = 0; Index != WorkgroupShape.size(); ++Index) {
    auto *Value =
        mdconst::dyn_extract<ConstantInt>(Workgroup->getOperand(Index));
    if (!Value || Value->getZExtValue() != WorkgroupShape[Index])
      return pipelineError(
          "exact FlashAttention LLVM workgroup metadata does not match");
  }

  size_t FAdds = 0, FMuls = 0, FSubs = 0, FDivs = 0;
  size_t Loads = 0, Stores = 0, WorkitemIds = 0, Traps = 0, Exps = 0;
  for (const Instruction &InstructionValue : instructions(*Kernel)) {
    switch (InstructionValue.getOpcode()) {
    case Instruction::FAdd:
      ++FAdds;
      break;
    case Instruction::FMul:
      ++FMuls;
      break;
    case Instruction::FSub:
      ++FSubs;
      break;
    case Instruction::FDiv:
      ++FDivs;
      break;
    default:
      break;
    }
    if (const auto *FloatOperation =
            dyn_cast<FPMathOperator>(&InstructionValue))
      if (FloatOperation->getFastMathFlags().any())
        return pipelineError(
            "exact FlashAttention LLVM floating-point policy does not match");
    if (const auto *Load = dyn_cast<LoadInst>(&InstructionValue)) {
      ++Loads;
      if (!Load->getType()->isFloatTy() || Load->getAlign() != Align(4) ||
          Load->getPointerAddressSpace() != 1 || Load->isAtomic())
        return pipelineError(
            "exact FlashAttention LLVM load effect does not match");
    }
    if (const auto *Store = dyn_cast<StoreInst>(&InstructionValue)) {
      ++Stores;
      if (!Store->getValueOperand()->getType()->isFloatTy() ||
          Store->getAlign() != Align(4) ||
          Store->getPointerAddressSpace() != 1 || Store->isAtomic())
        return pipelineError(
            "exact FlashAttention LLVM store effect does not match");
    }
    if (isa<AtomicRMWInst, AtomicCmpXchgInst, FenceInst, AllocaInst>(
            InstructionValue))
      return pipelineError(
          "exact FlashAttention LLVM contains a forbidden memory effect");
    const auto *Call = dyn_cast<CallBase>(&InstructionValue);
    if (!Call)
      continue;
    const Function *Callee = Call->getCalledFunction();
    if (!Callee)
      return pipelineError(
          "exact FlashAttention LLVM contains an indirect call");
    if (Callee->getName() == "llvm.amdgcn.workitem.id.x")
      ++WorkitemIds;
    else if (Callee->getName() == "llvm.trap")
      ++Traps;
    else if (Callee->getName() == ExactFlashAttentionV1OcmlExp)
      ++Exps;
    else
      return pipelineError(
          "exact FlashAttention LLVM call closure does not match");
  }
  if (FAdds != 35 || FMuls != 39 || FSubs != 2 || FDivs != 2 || Loads != 71 ||
      Stores != 2 || WorkitemIds != 1 || Traps != 1 || Exps != 2)
    return pipelineError(
        "exact FlashAttention LLVM operation closure does not match");
  return Error::success();
}

uint32_t expectedGfx942Flags(const TargetParts &Parts) {
  uint32_t Flags = ELF::EF_AMDGPU_MACH_AMDGCN_GFX942;
  Flags |= Parts.Xnack ? (*Parts.Xnack ? ELF::EF_AMDGPU_FEATURE_XNACK_ON_V4
                                       : ELF::EF_AMDGPU_FEATURE_XNACK_OFF_V4)
                       : ELF::EF_AMDGPU_FEATURE_XNACK_ANY_V4;
  Flags |= Parts.SramEcc
               ? (*Parts.SramEcc ? ELF::EF_AMDGPU_FEATURE_SRAMECC_ON_V4
                                 : ELF::EF_AMDGPU_FEATURE_SRAMECC_OFF_V4)
               : ELF::EF_AMDGPU_FEATURE_SRAMECC_ANY_V4;
  return Flags;
}

Error validateRequest(const Request &RequestValue) {
  switch (RequestValue.LinkOptions.Optimization) {
  case OptimizationLevel::O0:
  case OptimizationLevel::O1:
  case OptimizationLevel::O2:
  case OptimizationLevel::O3:
    break;
  default:
    return pipelineError("unsupported optimization level");
  }
  if (RequestValue.CodeObjectVersion != 4 &&
      RequestValue.CodeObjectVersion != 5 &&
      RequestValue.CodeObjectVersion != 6)
    return pipelineError("unsupported code-object version");
  if (RequestValue.MaxOutputBytes == 0 ||
      RequestValue.MaxOutputBytes > MaxOutputBytes)
    return pipelineError("invalid output byte bound");
  if (RequestValue.Inputs.empty() || RequestValue.Inputs.size() > MaxInputs)
    return pipelineError("invalid worker input count");

  size_t Total = 0;
  for (size_t I = 0; I < RequestValue.Inputs.size(); ++I) {
    const Input &InputValue = RequestValue.Inputs[I];
    if (InputValue.Bytes.empty() ||
        InputValue.Bytes.size() > MaxTotalInputBytes ||
        Total > MaxTotalInputBytes - InputValue.Bytes.size())
      return pipelineError(Twine("input ") + Twine(I) +
                           " violates the byte bound");
    Total += InputValue.Bytes.size();
    if (SHA256::hash(InputValue.Bytes) != InputValue.Digest)
      return pipelineError(Twine("input ") + Twine(I) +
                           " digest does not match its bytes");
    StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                    InputValue.Bytes.size());
    file_magic Expected;
    switch (InputValue.Kind) {
    case InputKind::LlvmBitcode:
      Expected = file_magic::bitcode;
      break;
    case InputKind::AmdGpuRelocatable:
      Expected = file_magic::elf_relocatable;
      break;
    case InputKind::LlvmTextIr:
      if (llvm::any_of(InputValue.Bytes,
                       [](uint8_t Byte) { return Byte == 0 || Byte > 0x7f; }))
        return pipelineError(Twine("input ") + Twine(I) +
                             " has noncanonical textual LLVM IR bytes");
      continue;
    default:
      return pipelineError(Twine("input ") + Twine(I) +
                           " has an unsupported input kind");
    }
    if (identify_magic(Bytes) != Expected)
      return pipelineError(Twine("input ") + Twine(I) +
                           " has a file kind different from its declaration");
  }
  for (size_t I = 1; I < RequestValue.Inputs.size(); ++I) {
    const Input &Before = RequestValue.Inputs[I - 1];
    const Input &After = RequestValue.Inputs[I];
    auto BeforeKey =
        std::tuple(Before.Digest, Before.Bytes.size(), Before.Kind);
    auto AfterKey = std::tuple(After.Digest, After.Bytes.size(), After.Kind);
    if (BeforeKey >= AfterKey)
      return pipelineError("worker inputs are duplicate or noncanonical");
  }
  return Error::success();
}

uint8_t expectedAbiVersion(uint8_t CodeObjectVersion) {
  switch (CodeObjectVersion) {
  case 4:
    return ELF::ELFABIVERSION_AMDGPU_HSA_V4;
  case 5:
    return ELF::ELFABIVERSION_AMDGPU_HSA_V5;
  case 6:
    return ELF::ELFABIVERSION_AMDGPU_HSA_V6;
  }
  llvm_unreachable("validated code-object version");
}

CodeGenOptLevel codegenLevel(OptimizationLevel Level) {
  switch (Level) {
  case OptimizationLevel::O0:
    return CodeGenOptLevel::None;
  case OptimizationLevel::O1:
    return CodeGenOptLevel::Less;
  case OptimizationLevel::O2:
    return CodeGenOptLevel::Default;
  case OptimizationLevel::O3:
    return CodeGenOptLevel::Aggressive;
  }
  llvm_unreachable("validated optimization level");
}

llvm::OptimizationLevel irLevel(OptimizationLevel Level) {
  switch (Level) {
  case OptimizationLevel::O0:
    return llvm::OptimizationLevel::O0;
  case OptimizationLevel::O1:
    return llvm::OptimizationLevel::O1;
  case OptimizationLevel::O2:
    return llvm::OptimizationLevel::O2;
  case OptimizationLevel::O3:
    return llvm::OptimizationLevel::O3;
  }
  llvm_unreachable("validated optimization level");
}

Expected<std::unique_ptr<TargetMachine>>
createMachine(const Request &RequestValue) {
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
  if (!TargetValue)
    return pipelineError(Twine("AMDGPU target lookup failed: ") + LookupError);
  TargetParts Parts = parseTarget(RequestValue.Target);
  std::string Features = targetMachineFeatures(Parts);
  TargetOptions OptionsValue;
  std::unique_ptr<TargetMachine> Machine(TargetValue->createTargetMachine(
      TripleValue, Parts.Cpu, Features, OptionsValue, Reloc::PIC_,
      CodeModel::Small, codegenLevel(RequestValue.LinkOptions.Optimization)));
  if (!Machine)
    return pipelineError("AMDGPU target-machine creation failed");
  return Machine;
}

Error setAndCheckModuleContract(Module &ModuleValue,
                                const Request &RequestValue,
                                const TargetMachine &Machine,
                                bool MeasuredBuiltinProvider = false) {
  TargetParts Parts = parseTarget(RequestValue.Target);
  const Triple &ExistingTriple = ModuleValue.getTargetTriple();
  if (!ExistingTriple.getTriple().empty() &&
      Triple::normalize(ExistingTriple.getTriple()) != AmdGpuTriple)
    return pipelineError("bitcode target triple does not match AMDHSA");
  bool ExactProducerLayout =
      (isExactLdsGemmSlice1RequestCandidate(RequestValue) ||
       isExactWave64CollectivesV1RequestCandidate(RequestValue) ||
       isExactFlashAttentionV1RequestCandidate(RequestValue) ||
       exactWorkgroupSyncProfile(RequestValue) != nullptr) &&
      ModuleValue.getDataLayoutStr() == ExactLdsGemmSlice1ProducerDataLayout;
  if (!ModuleValue.getDataLayoutStr().empty() &&
      ModuleValue.getDataLayout() != Machine.createDataLayout() &&
      !ExactProducerLayout)
    return pipelineError(
        Twine("bitcode data layout does not match target machine: '") +
        ModuleValue.getDataLayoutStr() + "' != '" +
        Machine.createDataLayout().getStringRepresentation() + "'");
  Metadata *ExistingCodeObject =
      ModuleValue.getModuleFlag("amdhsa_code_object_version");
  if (ExistingCodeObject) {
    auto *Constant = mdconst::dyn_extract<ConstantInt>(ExistingCodeObject);
    uint64_t Expected =
        static_cast<uint64_t>(RequestValue.CodeObjectVersion) * 100;
    if (!Constant)
      return pipelineError(
          "bitcode code-object version does not match request");
    if (Constant->getZExtValue() != Expected) {
      if (!MeasuredBuiltinProvider || RequestValue.CodeObjectVersion != 6 ||
          Constant->getZExtValue() != 500)
        return pipelineError(
            "bitcode code-object version does not match request");
      ModuleValue.setModuleFlag(
          Module::Error, "amdhsa_code_object_version",
          ConstantAsMetadata::get(ConstantInt::get(Constant->getType(), 600)));
    }
  }
  ModuleValue.setTargetTriple(Triple(AmdGpuTriple));
  ModuleValue.setDataLayout(Machine.createDataLayout());
  if (!ExistingCodeObject)
    ModuleValue.addModuleFlag(Module::Error, "amdhsa_code_object_version",
                              RequestValue.CodeObjectVersion * 100);

  for (StringRef FlagName : {StringRef("sramecc"), StringRef("xnack")}) {
    std::optional<uint32_t> Requested =
        FlagName == "sramecc" ? std::optional<uint32_t>(Parts.SramEcc)
                              : std::optional<uint32_t>(Parts.Xnack);
    std::string ModuleFlag = (Twine("amdgpu.") + FlagName).str();
    Metadata *Existing = ModuleValue.getModuleFlag(ModuleFlag);
    if (!Requested) {
      if (Existing)
        return pipelineError(Twine("bitcode ") + ModuleFlag +
                             " constraint is absent from request target");
      continue;
    }
    if (Existing) {
      auto *Constant = mdconst::dyn_extract<ConstantInt>(Existing);
      if (!Constant || Constant->getZExtValue() != *Requested)
        return pipelineError(Twine("bitcode ") + ModuleFlag +
                             " does not match request target");
    } else {
      ModuleValue.addModuleFlag(Module::Error, ModuleFlag, *Requested);
    }
  }

  for (const Function &FunctionValue : ModuleValue) {
    Attribute Cpu = FunctionValue.getFnAttribute("target-cpu");
    if (Cpu.isStringAttribute() && Cpu.getValueAsString() != Parts.Cpu)
      return pipelineError(Twine("bitcode function '") +
                           FunctionValue.getName() +
                           "' target CPU does not match request");
    Attribute Features = FunctionValue.getFnAttribute("target-features");
    if (!Features.isStringAttribute())
      continue;
    SmallVector<StringRef, 32> FeatureParts;
    Features.getValueAsString().split(FeatureParts, ',', -1, true);
    SmallString<256> CheckedFeatures;
    for (StringRef Feature : FeatureParts) {
      if (Feature.size() < 2 ||
          (Feature.front() != '+' && Feature.front() != '-'))
        return pipelineError(Twine("bitcode function '") +
                             FunctionValue.getName() +
                             "' has a malformed target feature");
      StringRef Name = Feature.drop_front();
      std::optional<bool> Requested;
      if (Name == "sramecc")
        Requested = Parts.SramEcc;
      else if (Name == "xnack")
        Requested = Parts.Xnack;
      if (Requested || Name == "sramecc" || Name == "xnack") {
        if (!Requested || *Requested != (Feature.front() == '+'))
          return pipelineError(Twine("bitcode function '") +
                               FunctionValue.getName() + "' " + Name +
                               " feature does not match request");
        continue;
      }
      ArrayRef<SubtargetFeatureKV> KnownFeatures =
          Machine.getMCSubtargetInfo()->getAllProcessorFeatures();
      auto Known = llvm::lower_bound(KnownFeatures, Name);
      if (Known == KnownFeatures.end() || StringRef(Known->Key) != Name)
        return pipelineError(Twine("bitcode function '") +
                             FunctionValue.getName() +
                             "' names an unknown target feature");
      if (!CheckedFeatures.empty())
        CheckedFeatures.push_back(',');
      CheckedFeatures.append(Feature);
    }
    if (!CheckedFeatures.empty() &&
        !Machine.getMCSubtargetInfo()->checkFeatures(CheckedFeatures))
      return pipelineError(Twine("bitcode function '") +
                           FunctionValue.getName() +
                           "' target features are incompatible with target");
  }
  return Error::success();
}

Expected<std::unique_ptr<Module>>
parseModuleInput(const Input &InputValue, StringRef InputName,
                 const Request &RequestValue, LLVMContext &Context,
                 const TargetMachine &Machine,
                 bool MeasuredBuiltinProvider = false) {
  if (InputValue.Kind != InputKind::LlvmBitcode &&
      InputValue.Kind != InputKind::LlvmTextIr)
    return pipelineError(Twine(InputName) + " is not an LLVM module");
  StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                  InputValue.Bytes.size());
  if (!MeasuredBuiltinProvider &&
      isClosedExactRowSoftmaxV1Request(RequestValue))
    if (Error E = validateExactRowSoftmaxV1CompilerInput(
            Bytes, Machine.createDataLayout()))
      return E;
  if (!MeasuredBuiltinProvider &&
      isClosedExactWave64CollectivesV1Request(RequestValue))
    if (Error E = validateExactWave64CompilerInput(Bytes))
      return E;
  if (!MeasuredBuiltinProvider &&
      isClosedExactFlashAttentionV1Request(RequestValue))
    if (Error E = validateExactFlashAttentionCompilerInput(Bytes))
      return E;
  if (!MeasuredBuiltinProvider)
    if (const ExactWorkgroupSyncProfile *Profile =
            exactWorkgroupSyncProfile(RequestValue))
      if (Error E = validateExactWorkgroupSyncCompilerInput(
              Bytes, *Profile, Machine.createDataLayout()))
        return E;
  if (isClosedExactMoeTop2V1Request(RequestValue))
    if (Error E = validateExactMoeTop2V1CompilerInput(
            Bytes, Machine.createDataLayout()))
      return E;
  DataLayout ExpectedLayout = Machine.createDataLayout();
  bool AcceptedLayout = false;
  ParserCallbacks Callbacks([&](StringRef TripleValue, StringRef Layout) {
    std::string InferredAbsent = UpgradeDataLayoutString("", TripleValue);
    if (Layout == InferredAbsent) {
      AcceptedLayout = true;
    } else {
      auto ParsedLayout = DataLayout::parse(Layout);
      if (ParsedLayout)
        AcceptedLayout = *ParsedLayout == ExpectedLayout;
      else
        consumeError(ParsedLayout.takeError());
    }
    return std::optional<std::string>(ExpectedLayout.getStringRepresentation());
  });
  Expected<std::unique_ptr<Module>> Parsed =
      [&]() -> Expected<std::unique_ptr<Module>> {
    if (InputValue.Kind == InputKind::LlvmBitcode)
      return parseBitcodeFile(MemoryBufferRef(Bytes, InputName), Context,
                              std::move(Callbacks));
    SMDiagnostic Diagnostic;
    auto TextBuffer = MemoryBuffer::getMemBufferCopy(Bytes, InputName);
    auto TextModule =
        parseAssembly(TextBuffer->getMemBufferRef(), Diagnostic, Context);
    if (!TextModule) {
      std::string Message;
      raw_string_ostream Stream(Message);
      Diagnostic.print("fe2o3-llvm-link-worker", Stream, false, false);
      Stream.flush();
      return pipelineError(Twine("textual LLVM IR input ") + InputName + ": " +
                           Message);
    }
    StringRef ObservedLayout = TextModule->getDataLayoutStr();
    AcceptedLayout =
        ObservedLayout.empty() ||
        TextModule->getDataLayout() == ExpectedLayout ||
        ((isExactLdsGemmSlice1RequestCandidate(RequestValue) ||
          isExactWave64CollectivesV1RequestCandidate(RequestValue) ||
          isExactFlashAttentionV1RequestCandidate(RequestValue) ||
          exactWorkgroupSyncProfile(RequestValue) != nullptr) &&
         ObservedLayout == ExactLdsGemmSlice1ProducerDataLayout);
    return TextModule;
  }();
  if (!Parsed)
    return pipelineError(Twine(InputName) + ": " +
                         errorToDiagnostic(Parsed.takeError()));
  if (!AcceptedLayout)
    return pipelineError(
        "LLVM module data layout does not match target machine");
  if (!MeasuredBuiltinProvider &&
      isClosedExactWave64CollectivesV1Request(RequestValue))
    if (Error E = validateExactWave64CollectivesModule(**Parsed))
      return E;
  if (!MeasuredBuiltinProvider &&
      isClosedExactFlashAttentionV1Request(RequestValue))
    if (Error E = validateExactFlashAttentionModule(**Parsed))
      return E;
  if (isClosedExactMoeTop2V1Request(RequestValue))
    if (Error E = validateExactMoeTop2V1Module(**Parsed, ExpectedLayout))
      return E;
  if (!MeasuredBuiltinProvider)
    if (const ExactWorkgroupSyncProfile *Profile =
            exactWorkgroupSyncProfile(RequestValue))
      if (Error E = validateExactWorkgroupSyncModule(**Parsed, *Profile,
                                                     ExpectedLayout))
        return E;
  if (Error E = setAndCheckModuleContract(**Parsed, RequestValue, Machine,
                                          MeasuredBuiltinProvider))
    return E;
  if (RequestValue.LinkOptions.VerifyEach) {
    BoundedRawStream Stream(MaxDiagnosticBytes);
    if (verifyModule(**Parsed, &Stream)) {
      Stream.flush();
      return pipelineError(Twine("input LLVM module verification failed: ") +
                           Stream.str());
    }
  }
  return std::move(*Parsed);
}

bool hasOcmlF32Abi(const Function &FunctionValue) {
  FunctionType *Type = FunctionValue.getFunctionType();
  return Type->getReturnType()->isFloatTy() && !Type->isVarArg() &&
         Type->getNumParams() == 1 && Type->getParamType(0)->isFloatTy() &&
         FunctionValue.getCallingConv() == CallingConv::C;
}

Error validateOcmlImportAbi(const Module &Compiler, StringRef Name) {
  const auto *FunctionValue =
      dyn_cast_or_null<Function>(Compiler.getNamedValue(Name));
  if (!FunctionValue || !FunctionValue->isDeclaration() ||
      !hasOcmlF32Abi(*FunctionValue))
    return pipelineError(Twine("compiler module has the wrong OCML ABI for ") +
                         Name);
  return Error::success();
}

Error validateOcmlRootAbi(const Module &Provider, StringRef Name) {
  const auto *FunctionValue =
      dyn_cast_or_null<Function>(Provider.getNamedValue(Name));
  if (!FunctionValue || FunctionValue->isDeclaration())
    return pipelineError(Twine("gfx942 OCML provider does not define ") + Name);
  if (!hasOcmlF32Abi(*FunctionValue))
    return pipelineError(Twine("gfx942 OCML provider has the wrong ABI for ") +
                         Name);
  return Error::success();
}

Error reduceBuiltinProviderClosure(Module &Linked,
                                   const Request &RequestValue) {
  std::set<std::string> Preserved(RequestValue.ExpectedDefinedSymbols.begin(),
                                  RequestValue.ExpectedDefinedSymbols.end());
  for (const std::string &Import : RequestValue.ImportSymbols) {
    if (!isSupportedGfx942OcmlImport(Import))
      continue;
    auto *FunctionValue =
        dyn_cast_or_null<Function>(Linked.getNamedValue(Import));
    if (!FunctionValue || FunctionValue->isDeclaration())
      return pipelineError(Twine("gfx942 OCML import remained unresolved: ") +
                           Import);
    FunctionValue->setVisibility(GlobalValue::DefaultVisibility);
    FunctionValue->setDSOLocal(true);
  }

  internalizeModule(Linked, [&Preserved](const GlobalValue &Value) {
    return Value.hasName() && Preserved.contains(Value.getName().str());
  });
  ModuleAnalysisManager Analyses;
  GlobalDCEPass().run(Linked, Analyses);
  return Error::success();
}

Expected<std::unique_ptr<Module>> linkBitcode(const Request &RequestValue,
                                              ArrayRef<Input> BuiltinProviders,
                                              LLVMContext &Context,
                                              const TargetMachine &Machine) {
  std::unique_ptr<Module> Linked;
  for (size_t I = 0; I < RequestValue.Inputs.size(); ++I) {
    const Input &InputValue = RequestValue.Inputs[I];
    if (InputValue.Kind != InputKind::LlvmBitcode &&
        InputValue.Kind != InputKind::LlvmTextIr)
      continue;
    std::string InputName = (Twine("<bitcode-input-") + Twine(I) + ">").str();
    auto Parsed =
        parseModuleInput(InputValue, InputName, RequestValue, Context, Machine);
    if (!Parsed)
      return Parsed.takeError();
    if (!Linked)
      Linked = std::move(*Parsed);
    else if (Linker::linkModules(*Linked, std::move(*Parsed)))
      return pipelineError("LLVM module linking failed");
  }

  if (!BuiltinProviders.empty() && !Linked)
    return pipelineError(
        "measured device library has no compiler LLVM module to resolve");
  if (!BuiltinProviders.empty())
    for (const std::string &Import : RequestValue.ImportSymbols)
      if (isSupportedGfx942OcmlImport(Import))
        if (Error E = validateOcmlImportAbi(*Linked, Import))
          return E;

  for (size_t I = 0; I < BuiltinProviders.size(); ++I) {
    const Input &InputValue = BuiltinProviders[I];
    std::string InputName =
        (Twine("<measured-gfx942-device-library-") + Twine(I) + ">").str();
    auto Parsed = parseModuleInput(InputValue, InputName, RequestValue, Context,
                                   Machine, true);
    if (!Parsed)
      return Parsed.takeError();
    for (const std::string &Import : RequestValue.ImportSymbols)
      if (isSupportedGfx942OcmlImport(Import))
        if (const GlobalValue *Value = (*Parsed)->getNamedValue(Import);
            Value && !Value->isDeclaration())
          if (Error E = validateOcmlRootAbi(**Parsed, Import))
            return E;
    if (!Linked)
      return pipelineError(
          "measured device library has no compiler module to resolve");
    if (Linker::linkModules(*Linked, std::move(*Parsed),
                            Linker::Flags::LinkOnlyNeeded))
      return pipelineError("gfx942 device-library linking failed");
  }
  if (!BuiltinProviders.empty())
    if (Error E = reduceBuiltinProviderClosure(*Linked, RequestValue))
      return E;
  if (Linked) {
    BoundedRawStream Stream(MaxDiagnosticBytes);
    if (verifyModule(*Linked, &Stream)) {
      Stream.flush();
      return pipelineError(Twine("linked LLVM module verification failed: ") +
                           Stream.str());
    }
  }
  return Linked;
}

Error optimizeModule(Module &ModuleValue, const Request &RequestValue,
                     TargetMachine &Machine) {
  if (RequestValue.LinkOptions.StripDebug)
    StripDebugInfo(ModuleValue);
  SmallVector<GlobalValue *, 16> PreservedExports;
  for (const std::string &Name : RequestValue.ExpectedDefinedSymbols)
    if (GlobalValue *Value = ModuleValue.getNamedValue(Name);
        Value && !Value->isDeclaration()) {
      if (Value->hasLocalLinkage() ||
          Value->getVisibility() != GlobalValue::DefaultVisibility)
        return pipelineError(Twine("requested bitcode export '") + Name +
                             "' is not externally visible");
      PreservedExports.push_back(Value);
    }
  if (!PreservedExports.empty())
    appendToUsed(ModuleValue, PreservedExports);
  PassBuilder Builder(&Machine);
  LoopAnalysisManager Loops;
  FunctionAnalysisManager Functions;
  CGSCCAnalysisManager Cgscc;
  ModuleAnalysisManager Modules;
  Builder.registerModuleAnalyses(Modules);
  Builder.registerCGSCCAnalyses(Cgscc);
  Builder.registerFunctionAnalyses(Functions);
  Builder.registerLoopAnalyses(Loops);
  Builder.crossRegisterProxies(Loops, Functions, Cgscc, Modules);
  ModulePassManager Pipeline = Builder.buildPerModuleDefaultPipeline(
      irLevel(RequestValue.LinkOptions.Optimization));
  Pipeline.run(ModuleValue, Modules);

  // The compiler descriptor commits COV5/6 kernels to the complete 256-byte
  // implicit block. Optimization may infer amdgpu-no-implicitarg-ptr when a
  // kernel does not read that block; canonicalize the physical ABI after the
  // inference passes so metadata, native descriptors, and host admission stay
  // on the same contract.
  if (RequestValue.CodeObjectVersion >= 5) {
    for (Function &FunctionValue : ModuleValue) {
      if (FunctionValue.getCallingConv() != CallingConv::AMDGPU_KERNEL)
        continue;
      FunctionValue.removeFnAttr("amdgpu-no-implicitarg-ptr");
      FunctionValue.addFnAttr("amdgpu-implicitarg-num-bytes", "256");
    }
  }

  BoundedRawStream Stream(MaxDiagnosticBytes);
  if (verifyModule(ModuleValue, &Stream)) {
    Stream.flush();
    return pipelineError(Twine("optimized bitcode verification failed: ") +
                         Stream.str());
  }
  return Error::success();
}

Expected<std::vector<uint8_t>> emitObject(Module &ModuleValue,
                                          TargetMachine &Machine) {
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  legacy::PassManager Passes;
  if (Machine.addPassesToEmitFile(Passes, Stream, nullptr,
                                  CodeGenFileType::ObjectFile, false))
    return pipelineError("AMDGPU target does not support object emission");
  Passes.run(ModuleValue);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

struct ElfContract {
  uint32_t Flags;
  uint8_t OsAbi;
  uint8_t AbiVersion;
  uint8_t AddressBytes;
  bool LittleEndian;
  std::set<std::string> Definitions;
  std::set<std::string> PublicDefinitions;
  std::set<std::string> RequiredImports;
};

struct SymbolContract {
  std::set<std::string> Definitions;
  std::set<std::string> PublicDefinitions;
  std::set<std::string> RequiredImports;
};

std::string diagnosticAtom(StringRef Value) {
  static constexpr char Hex[] = "0123456789ABCDEF";
  std::string Result;
  for (unsigned char Byte : Value) {
    if (llvm::isAlnum(Byte) || Byte == '_' || Byte == '-' || Byte == '.' ||
        Byte == '$') {
      Result.push_back(static_cast<char>(Byte));
    } else {
      Result.push_back('%');
      Result.push_back(Hex[Byte >> 4]);
      Result.push_back(Hex[Byte & 0xf]);
    }
  }
  return Result;
}

std::string digestHex(ArrayRef<uint8_t> Digest) {
  static constexpr char Hex[] = "0123456789abcdef";
  std::string Result;
  Result.reserve(Digest.size() * 2);
  for (uint8_t Byte : Digest) {
    Result.push_back(Hex[Byte >> 4]);
    Result.push_back(Hex[Byte & 0x0f]);
  }
  return Result;
}

std::array<uint8_t, 32> bitcodeIdentity(const Module &ModuleValue) {
  SmallVector<char, 0> Bitcode;
  raw_svector_ostream Stream(Bitcode);
  WriteBitcodeToFile(ModuleValue, Stream, true);
  return SHA256::hash(ArrayRef(
      reinterpret_cast<const uint8_t *>(Bitcode.data()), Bitcode.size()));
}

std::string diagnosticList(const std::set<std::string> &Values) {
  std::string Result = "[";
  for (const std::string &Value : Values) {
    if (Result.size() != 1)
      Result.push_back(',');
    Result.append(diagnosticAtom(Value));
  }
  Result.push_back(']');
  return Result;
}

bool matches(const ElfContract &Left, const ElfContract &Right) {
  return Left.Flags == Right.Flags && Left.OsAbi == Right.OsAbi &&
         Left.AbiVersion == Right.AbiVersion &&
         Left.AddressBytes == Right.AddressBytes &&
         Left.LittleEndian == Right.LittleEndian;
}

Expected<ElfContract> inspectRelocatable(ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<object>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  if (!Elf || Elf->getEMachine() != ELF::EM_AMDGPU ||
      Elf->getEType() != ELF::ET_REL)
    return pipelineError("input is not an AMDGPU ELF relocatable");
  if (Bytes.size() < ELF::EI_NIDENT ||
      Bytes[ELF::EI_CLASS] != ELF::ELFCLASS64 ||
      Bytes[ELF::EI_DATA] != ELF::ELFDATA2LSB ||
      Bytes[ELF::EI_VERSION] != ELF::EV_CURRENT ||
      Elf->getBytesInAddress() != 8 || !Elf->isLittleEndian())
    return pipelineError("AMDGPU relocatable is not 64-bit little-endian ELF");
  uint8_t OsAbi = Bytes[ELF::EI_OSABI];
  if (OsAbi != ELF::ELFOSABI_AMDGPU_HSA)
    return pipelineError("AMDGPU relocatable does not use the AMDHSA OS ABI");

  ElfContract Result{Elf->getPlatformFlags(),
                     OsAbi,
                     Elf->getEIdentABIVersion(),
                     Elf->getBytesInAddress(),
                     Elf->isLittleEndian(),
                     {},
                     {},
                     {}};
  for (SymbolRef Symbol : Elf->symbols()) {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      return FlagsOrError.takeError();
    uint32_t Flags = *FlagsOrError;
    if ((Flags & SymbolRef::SF_FormatSpecific) != 0)
      continue;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (NameOrError->empty())
      continue;
    std::string Name = NameOrError->str();
    if ((Flags & SymbolRef::SF_Undefined) != 0) {
      Result.RequiredImports.insert(std::move(Name));
    } else if ((Flags & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) != 0) {
      Result.Definitions.insert(Name);
      if ((Flags & SymbolRef::SF_Hidden) == 0)
        Result.PublicDefinitions.insert(std::move(Name));
    }
  }
  return Result;
}

SymbolContract inspectModuleSymbols(const Module &ModuleValue) {
  SymbolContract Result;
  for (const GlobalValue &Value : ModuleValue.global_values()) {
    if (!Value.hasName() || Value.hasLocalLinkage() ||
        Value.hasAppendingLinkage())
      continue;

    if (Value.isDeclarationForLinker()) {
      const auto *FunctionValue = dyn_cast<Function>(&Value);
      if (!Value.use_empty() &&
          (!FunctionValue || !FunctionValue->isIntrinsic()))
        Result.RequiredImports.insert(Value.getName().str());
      continue;
    }

    std::string Name = Value.getName().str();
    Result.Definitions.insert(Name);
    if (Value.getVisibility() == GlobalValue::DefaultVisibility)
      Result.PublicDefinitions.insert(std::move(Name));
  }
  return Result;
}

Expected<SymbolContract> inspectModuleSymbols(const Input &InputValue,
                                              StringRef InputName) {
  StringRef Bytes(reinterpret_cast<const char *>(InputValue.Bytes.data()),
                  InputValue.Bytes.size());
  LLVMContext Context;
  Expected<std::unique_ptr<Module>> Parsed =
      [&]() -> Expected<std::unique_ptr<Module>> {
    if (InputValue.Kind == InputKind::LlvmBitcode)
      return parseBitcodeFile(MemoryBufferRef(Bytes, InputName), Context);
    SMDiagnostic Diagnostic;
    auto TextBuffer = MemoryBuffer::getMemBufferCopy(Bytes, InputName);
    auto TextModule =
        parseAssembly(TextBuffer->getMemBufferRef(), Diagnostic, Context);
    if (!TextModule) {
      std::string Message;
      raw_string_ostream Stream(Message);
      Diagnostic.print("fe2o3-llvm-link-worker", Stream, false, false);
      Stream.flush();
      return pipelineError(Message);
    }
    return TextModule;
  }();
  if (!Parsed)
    return pipelineError(Twine(InputName) + ": " +
                         errorToDiagnostic(Parsed.takeError()));

  return inspectModuleSymbols(**Parsed);
}

Error requireExactCompilerImports(const Request &RequestValue,
                                  const SymbolContract &CompilerModule,
                                  StringRef Source) {
  std::set<std::string> Declared(RequestValue.ImportSymbols.begin(),
                                 RequestValue.ImportSymbols.end());
  std::set<std::string> Omitted;
  std::set<std::string> Extra;
  for (const std::string &Name : CompilerModule.RequiredImports)
    if (!Declared.contains(Name))
      Omitted.insert(Name);
  for (const std::string &Name : Declared)
    if (!CompilerModule.RequiredImports.contains(Name))
      Extra.insert(Name);
  if (!Omitted.empty() || !Extra.empty())
    return pipelineError(
        Twine("compiler-module") +
        (Source.empty() ? "" : (Twine(" ") + Source).str()) +
        " import manifest mismatch: omitted=" + diagnosticList(Omitted) +
        " extra=" + diagnosticList(Extra));
  return Error::success();
}

Error validateExactDynamicLdsPseudoImport(const Request &RequestValue,
                                          std::set<std::string> &Imports,
                                          StringRef Source,
                                          bool MustBePresent) {
  const ExactWorkgroupSyncProfile *Profile =
      exactWorkgroupSyncProfile(RequestValue);
  if (!Profile || Profile->Kind != ExactWorkgroupSyncKind::LdsReduction)
    return Error::success();
  bool Present = Imports.erase(ExactWorkgroupLdsReductionV1Scratch.str()) == 1;
  if (Present != MustBePresent)
    return pipelineError(
        Twine("exact LDS reduction ") + Source +
        (MustBePresent
             ? " does not contain its single dynamic-LDS pseudo-import"
             : " retained its dynamic-LDS pseudo-import after codegen"));
  return Error::success();
}

Expected<SymbolContract>
inspectNormalizedCompilerModule(const Request &RequestValue,
                                TargetMachine &Machine) {
  LLVMContext Context;
  auto Parsed =
      parseModuleInput(RequestValue.CompilerModule, "<compiler-module>",
                       RequestValue, Context, Machine);
  if (!Parsed)
    return Parsed.takeError();

  BoundedRawStream Verification(MaxDiagnosticBytes);
  if (verifyModule(**Parsed, &Verification)) {
    Verification.flush();
    return pipelineError(Twine("compiler-module verification failed: ") +
                         Verification.str());
  }

  SymbolContract IrSymbols = inspectModuleSymbols(**Parsed);
  auto ObjectBytes = emitObject(**Parsed, Machine);
  if (!ObjectBytes)
    return ObjectBytes.takeError();
  auto ObjectSymbols = inspectRelocatable(*ObjectBytes);
  if (!ObjectSymbols)
    return ObjectSymbols.takeError();
  SymbolContract EmittedSymbols{std::move(ObjectSymbols->Definitions),
                                std::move(ObjectSymbols->PublicDefinitions),
                                std::move(ObjectSymbols->RequiredImports)};

  if (Error E = validateExactDynamicLdsPseudoImport(
          RequestValue, IrSymbols.RequiredImports, "LLVM module", true))
    return E;
  if (Error E = validateExactDynamicLdsPseudoImport(
          RequestValue, EmittedSymbols.RequiredImports, "relocatable object",
          false))
    return E;

  if (Error E = requireExactCompilerImports(RequestValue, IrSymbols, ""))
    return E;
  if (Error E =
          requireExactCompilerImports(RequestValue, EmittedSymbols, "object"))
    return E;
  if (IrSymbols.RequiredImports != EmittedSymbols.RequiredImports)
    return pipelineError(
        "compiler-module IR and object import sets do not match");
  return IrSymbols;
}

Expected<SymbolContract> inspectInputSymbols(const Input &InputValue,
                                             StringRef InputName) {
  if (InputValue.Kind == InputKind::LlvmBitcode ||
      InputValue.Kind == InputKind::LlvmTextIr)
    return inspectModuleSymbols(InputValue, InputName);
  auto Elf = inspectRelocatable(InputValue.Bytes);
  if (!Elf)
    return Elf.takeError();
  return SymbolContract{std::move(Elf->Definitions),
                        std::move(Elf->PublicDefinitions),
                        std::move(Elf->RequiredImports)};
}

Error validateV2SymbolRoles(const Request &RequestValue,
                            const SymbolContract &Module,
                            ArrayRef<Input> BuiltinProviders) {
  std::vector<SymbolContract> Providers;
  Providers.reserve(RequestValue.ExternalProviders.size() +
                    BuiltinProviders.size());
  for (size_t I = 0; I < RequestValue.ExternalProviders.size(); ++I) {
    std::string Name = (Twine("<external-provider-") + Twine(I) + ">").str();
    auto Provider =
        inspectInputSymbols(RequestValue.ExternalProviders[I], Name);
    if (!Provider)
      return Provider.takeError();
    Providers.push_back(std::move(*Provider));
  }
  for (size_t I = 0; I < BuiltinProviders.size(); ++I) {
    std::string Name =
        (Twine("<measured-gfx942-device-library-") + Twine(I) + ">").str();
    auto Provider = inspectInputSymbols(BuiltinProviders[I], Name);
    if (!Provider)
      return Provider.takeError();
    Providers.push_back(std::move(*Provider));
  }

  std::map<std::string, size_t> DefinitionCounts;
  for (const std::string &Name : Module.Definitions)
    ++DefinitionCounts[Name];
  for (const SymbolContract &Provider : Providers)
    for (const std::string &Name : Provider.Definitions)
      ++DefinitionCounts[Name];
  for (const auto &[Name, Count] : DefinitionCounts)
    if (Count > 1)
      return pipelineError(Twine("duplicate definition: ") + Name);

  for (const std::string &Name : RequestValue.ExportSymbols)
    if (!Module.PublicDefinitions.contains(Name))
      return pipelineError(Twine("compiler-module export is not defined by "
                                 "the compiler module: ") +
                           Name);

  for (const std::string &Name : RequestValue.ImportSymbols) {
    if (Module.Definitions.contains(Name))
      return pipelineError(Twine("compiler-module import is defined by the "
                                 "compiler module: ") +
                           Name);
    if (!Module.RequiredImports.contains(Name))
      return pipelineError(Twine("compiler-module import is not unresolved "
                                 "by the compiler module: ") +
                           Name);
    bool ResolvedByProvider =
        llvm::any_of(Providers, [&Name](const SymbolContract &Provider) {
          return Provider.Definitions.contains(Name);
        });
    if (!ResolvedByProvider)
      return pipelineError(Twine("compiler-module import has no external "
                                 "provider: ") +
                           Name);
  }
  return Error::success();
}

struct KernelArgumentContract {
  std::optional<std::string> Name;
  std::optional<std::string> TypeName;
  uint64_t Offset;
  uint64_t Size;
  std::optional<uint64_t> Align;
  std::string ValueKind;
  std::optional<std::string> ValueType;
  std::optional<std::string> AddressSpace;
  std::optional<std::string> Access;
  std::optional<std::string> ActualAccess;
  std::optional<uint64_t> PointeeAlign;
  std::optional<bool> IsConst;
  std::optional<bool> IsRestrict;
  std::optional<bool> IsVolatile;
  std::optional<bool> IsPipe;
};

struct KernelLaunchContract {
  std::string Name;
  std::string Symbol;
  uint64_t KernargSegmentSize;
  uint64_t GroupSegmentFixedSize;
  uint64_t PrivateSegmentFixedSize;
  uint64_t KernargSegmentAlign;
  uint64_t WavefrontSize;
  uint64_t MaxFlatWorkgroupSize;
  std::optional<std::array<uint64_t, 3>> RequiredWorkgroupSize;
  uint64_t SgprCount;
  uint64_t VgprCount;
  std::optional<uint64_t> AgprCount;
  std::optional<uint64_t> SgprSpillCount;
  std::optional<uint64_t> VgprSpillCount;
  std::optional<bool> UsesDynamicStack;
  std::optional<bool> WorkgroupProcessorMode;
  std::optional<bool> UniformWorkgroupSize;
  std::optional<std::vector<KernelArgumentContract>> Arguments;
};

struct MetadataContract {
  bool Present = false;
  std::optional<std::string> Target;
  std::vector<KernelLaunchContract> Kernels;
};

Error postLinkError(StringRef Check, StringRef Reason) {
  return pipelineError(Twine("post_link.check=") + Check +
                       " status=failed reason=" + diagnosticAtom(Reason));
}

std::string hexadecimal(uint32_t Value) {
  std::ostringstream Stream;
  Stream << "0x" << std::hex << std::nouppercase << Value;
  return Stream.str();
}

Expected<msgpack::DocNode *> requiredMetadataField(msgpack::MapDocNode &Map,
                                                   StringRef Name) {
  auto Field = Map.find(Name);
  if (Field == Map.end())
    return pipelineError(Twine("AMDGPU metadata is missing ") + Name);
  return &Field->second;
}

Expected<StringRef> metadataString(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = requiredMetadataField(Map, Name);
  if (!Field)
    return Field.takeError();
  if (!(**Field).isString())
    return pipelineError(Twine("AMDGPU metadata field ") + Name +
                         " is not a string");
  return (**Field).getString();
}

Expected<uint64_t> metadataUnsigned(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = requiredMetadataField(Map, Name);
  if (!Field)
    return Field.takeError();
  if ((**Field).getKind() == msgpack::Type::UInt)
    return (**Field).getUInt();
  if ((**Field).getKind() == msgpack::Type::Int && (**Field).getInt() >= 0)
    return static_cast<uint64_t>((**Field).getInt());
  return pipelineError(Twine("AMDGPU metadata field ") + Name +
                       " is not a nonnegative integer");
}

Expected<std::optional<uint64_t>>
metadataOptionalUnsigned(msgpack::MapDocNode &Map, StringRef Name) {
  if (Map.find(Name) == Map.end())
    return std::optional<uint64_t>{};
  auto Value = metadataUnsigned(Map, Name);
  if (!Value)
    return Value.takeError();
  return std::optional<uint64_t>(*Value);
}

Expected<std::optional<bool>> metadataOptionalBoolean(msgpack::MapDocNode &Map,
                                                      StringRef Name) {
  auto Field = Map.find(Name);
  if (Field == Map.end())
    return std::optional<bool>{};
  if (Field->second.getKind() != msgpack::Type::Boolean)
    return pipelineError(Twine("AMDGPU metadata field ") + Name +
                         " is not a boolean");
  return std::optional<bool>(Field->second.getBool());
}

Expected<std::optional<std::string>>
metadataOptionalString(msgpack::MapDocNode &Map, StringRef Name) {
  auto Field = Map.find(Name);
  if (Field == Map.end())
    return std::optional<std::string>{};
  if (!Field->second.isString())
    return pipelineError(Twine("AMDGPU metadata field ") + Name +
                         " is not a string");
  return std::optional<std::string>(Field->second.getString().str());
}

template <size_t Size>
Error rejectUnknownExactMetadataKeys(
    msgpack::MapDocNode &Map,
    const std::array<StringLiteral, Size> &AllowedKeys, StringRef Scope,
    StringRef Check) {
  for (const auto &Entry : Map) {
    if (!Entry.first.isString() ||
        !llvm::any_of(AllowedKeys, [&](StringRef Allowed) {
          return Entry.first.getString() == Allowed;
        }))
      return postLinkError(
          Check, (Twine("kernel_contract_unknown_") + Scope + "_key").str());
  }
  return Error::success();
}

Error validateExactMetadataKeys(msgpack::MapDocNode &Root, StringRef Check) {
  static constexpr std::array RootKeys = {StringLiteral("amdhsa.version"),
                                          StringLiteral("amdhsa.target"),
                                          StringLiteral("amdhsa.kernels")};
  static constexpr std::array KernelKeys = {
      StringLiteral(".name"),
      StringLiteral(".symbol"),
      StringLiteral(".args"),
      StringLiteral(".reqd_workgroup_size"),
      StringLiteral(".kernarg_segment_size"),
      StringLiteral(".group_segment_fixed_size"),
      StringLiteral(".private_segment_fixed_size"),
      StringLiteral(".uses_dynamic_stack"),
      StringLiteral(".workgroup_processor_mode"),
      StringLiteral(".kernarg_segment_align"),
      StringLiteral(".wavefront_size"),
      StringLiteral(".sgpr_count"),
      StringLiteral(".vgpr_count"),
      StringLiteral(".agpr_count"),
      StringLiteral(".max_flat_workgroup_size"),
      StringLiteral(".sgpr_spill_count"),
      StringLiteral(".vgpr_spill_count"),
      StringLiteral(".uniform_work_group_size"),
      StringLiteral(".language"),
      StringLiteral(".language_version")};
  static constexpr std::array ArgumentKeys = {
      StringLiteral(".name"),          StringLiteral(".type_name"),
      StringLiteral(".offset"),        StringLiteral(".size"),
      StringLiteral(".align"),         StringLiteral(".value_kind"),
      StringLiteral(".value_type"),    StringLiteral(".address_space"),
      StringLiteral(".access"),        StringLiteral(".actual_access"),
      StringLiteral(".pointee_align"), StringLiteral(".is_const"),
      StringLiteral(".is_restrict"),   StringLiteral(".is_volatile"),
      StringLiteral(".is_pipe")};

  if (Error E = rejectUnknownExactMetadataKeys(Root, RootKeys, "root", Check))
    return E;
  if (Check == ExactRowSoftmaxV1Check) {
    auto Version = Root.find("amdhsa.version");
    auto Target = Root.find("amdhsa.target");
    if (Version == Root.end() || !Version->second.isArray() ||
        Version->second.getArray().size() != 2)
      return postLinkError(Check, "kernel_contract_metadata_version");
    size_t Index = 0;
    for (msgpack::DocNode &Node : Version->second.getArray()) {
      const uint64_t Expected = Index++ == 0 ? 1 : 2;
      if (Node.getKind() != msgpack::Type::UInt || Node.getUInt() != Expected)
        return postLinkError(Check, "kernel_contract_metadata_version");
    }
    if (Target == Root.end() || !Target->second.isString() ||
        Target->second.getString() != "amdgcn-amd-amdhsa--gfx942:xnack-")
      return postLinkError(Check, "kernel_contract_target");
  }
  auto Kernels = Root.find("amdhsa.kernels");
  if (Kernels == Root.end() || !Kernels->second.isArray())
    return Error::success();
  for (msgpack::DocNode &KernelNode : Kernels->second.getArray()) {
    if (!KernelNode.isMap())
      continue;
    auto &Kernel = KernelNode.getMap();
    if (Error E =
            rejectUnknownExactMetadataKeys(Kernel, KernelKeys, "kernel", Check))
      return E;
    auto Language = Kernel.find(".language");
    auto LanguageVersion = Kernel.find(".language_version");
    if (Check == "flash_attention_v1_profile" ||
        Check == ExactRowSoftmaxV1Check) {
      if (Language == Kernel.end() || !Language->second.isString() ||
          Language->second.getString() != "OpenCL C" ||
          LanguageVersion == Kernel.end() ||
          !LanguageVersion->second.isArray() ||
          LanguageVersion->second.getArray().size() != 2)
        return postLinkError(Check, "kernel_contract_language");
      std::array<uint64_t, 2> ExpectedVersion = {2, 0};
      size_t VersionIndex = 0;
      for (msgpack::DocNode &Node : LanguageVersion->second.getArray()) {
        uint64_t Value = 0;
        if (Node.getKind() == msgpack::Type::UInt)
          Value = Node.getUInt();
        else if (Node.getKind() == msgpack::Type::Int && Node.getInt() >= 0)
          Value = static_cast<uint64_t>(Node.getInt());
        else
          return postLinkError(Check, "kernel_contract_language_version");
        if (Value != ExpectedVersion[VersionIndex++])
          return postLinkError(Check, "kernel_contract_language_version");
      }
    } else if (Language != Kernel.end() || LanguageVersion != Kernel.end()) {
      return postLinkError(Check, "kernel_contract_unexpected_language");
    }
    auto Arguments = Kernel.find(".args");
    if (Arguments == Kernel.end() || !Arguments->second.isArray())
      continue;
    for (msgpack::DocNode &ArgumentNode : Arguments->second.getArray()) {
      if (!ArgumentNode.isMap())
        continue;
      if (Error E = rejectUnknownExactMetadataKeys(
              ArgumentNode.getMap(), ArgumentKeys, "argument", Check))
        return E;
    }
  }
  return Error::success();
}

StringRef exactMetadataCheck(MetadataValidationPolicy Policy) {
  switch (Policy) {
  case MetadataValidationPolicy::ExactRowSoftmaxV1:
    return ExactRowSoftmaxV1Check;
  case MetadataValidationPolicy::ExactLdsGemmSlice1:
    return "lds_gemm_slice1_profile";
  case MetadataValidationPolicy::ExactWave64CollectivesV1:
    return "wave64_collectives_v1_profile";
  case MetadataValidationPolicy::ExactFlashAttentionV1:
    return "flash_attention_v1_profile";
  case MetadataValidationPolicy::ExactWorkgroupLdsReductionV1:
    return ExactWorkgroupLdsReductionV1.Check;
  case MetadataValidationPolicy::ExactScopedAtomicV1:
    return ExactScopedAtomicV1.Check;
  case MetadataValidationPolicy::ExactMoeTop2V1:
    return ExactMoeTop2V1Check;
  case MetadataValidationPolicy::Generic:
    break;
  }
  llvm_unreachable("generic metadata has no exact check name");
}

Expected<std::optional<std::vector<KernelArgumentContract>>>
metadataArguments(msgpack::MapDocNode &Kernel, uint64_t KernargSegmentSize,
                  MetadataValidationPolicy Policy) {
  auto Field = Kernel.find(".args");
  if (Field == Kernel.end())
    return std::optional<std::vector<KernelArgumentContract>>{};
  if (!Field->second.isArray())
    return pipelineError("AMDGPU metadata .args is not an array");

  std::vector<KernelArgumentContract> Result;
  uint64_t PreviousEnd = 0;
  for (msgpack::DocNode &Node : Field->second.getArray()) {
    if (!Node.isMap())
      return pipelineError("AMDGPU metadata argument is not a map");
    auto &Argument = Node.getMap();
    auto Name = metadataOptionalString(Argument, ".name");
    if (!Name)
      return Name.takeError();
    auto TypeName = metadataOptionalString(Argument, ".type_name");
    if (!TypeName)
      return TypeName.takeError();
    auto Offset = metadataUnsigned(Argument, ".offset");
    if (!Offset)
      return Offset.takeError();
    auto Size = metadataUnsigned(Argument, ".size");
    if (!Size)
      return Size.takeError();
    auto Align = metadataOptionalUnsigned(Argument, ".align");
    if (!Align)
      return Align.takeError();
    auto ValueKind = metadataString(Argument, ".value_kind");
    if (!ValueKind)
      return ValueKind.takeError();
    auto ValueType = metadataOptionalString(Argument, ".value_type");
    if (!ValueType)
      return ValueType.takeError();
    auto AddressSpace = metadataOptionalString(Argument, ".address_space");
    if (!AddressSpace)
      return AddressSpace.takeError();
    auto Access = metadataOptionalString(Argument, ".access");
    if (!Access)
      return Access.takeError();
    auto ActualAccess = metadataOptionalString(Argument, ".actual_access");
    if (!ActualAccess)
      return ActualAccess.takeError();
    auto PointeeAlign = metadataOptionalUnsigned(Argument, ".pointee_align");
    if (!PointeeAlign)
      return PointeeAlign.takeError();
    auto IsConst = metadataOptionalBoolean(Argument, ".is_const");
    if (!IsConst)
      return IsConst.takeError();
    auto IsRestrict = metadataOptionalBoolean(Argument, ".is_restrict");
    if (!IsRestrict)
      return IsRestrict.takeError();
    auto IsVolatile = metadataOptionalBoolean(Argument, ".is_volatile");
    if (!IsVolatile)
      return IsVolatile.takeError();
    auto IsPipe = metadataOptionalBoolean(Argument, ".is_pipe");
    if (!IsPipe)
      return IsPipe.takeError();

    if (Policy != MetadataValidationPolicy::Generic) {
      if (*Size == 0 ||
          *Offset > std::numeric_limits<uint64_t>::max() - *Size ||
          *Offset + *Size > KernargSegmentSize)
        return pipelineError("AMDGPU metadata argument range is invalid");
      if (*Offset < PreviousEnd)
        return pipelineError(
            "AMDGPU metadata arguments overlap or are unordered");
      if (*Align &&
          (**Align == 0 || !isPowerOf2_64(**Align) || *Offset % **Align != 0))
        return pipelineError("AMDGPU metadata argument alignment is invalid");
      if (*PointeeAlign &&
          (**PointeeAlign == 0 || !isPowerOf2_64(**PointeeAlign)))
        return pipelineError(
            "AMDGPU metadata argument pointee alignment is invalid");
    }
    PreviousEnd = *Offset + *Size;
    Result.push_back({std::move(*Name), std::move(*TypeName), *Offset, *Size,
                      *Align, ValueKind->str(), std::move(*ValueType),
                      std::move(*AddressSpace), std::move(*Access),
                      std::move(*ActualAccess), *PointeeAlign, *IsConst,
                      *IsRestrict, *IsVolatile, *IsPipe});
  }
  return std::optional<std::vector<KernelArgumentContract>>(std::move(Result));
}

Expected<std::optional<std::array<uint64_t, 3>>>
metadataWorkgroupSize(msgpack::MapDocNode &Map) {
  auto Field = Map.find(".reqd_workgroup_size");
  if (Field == Map.end())
    return std::optional<std::array<uint64_t, 3>>{};
  if (!Field->second.isArray() || Field->second.getArray().size() != 3)
    return pipelineError(
        "AMDGPU metadata .reqd_workgroup_size is not a three-element array");
  std::array<uint64_t, 3> Result{};
  size_t I = 0;
  for (msgpack::DocNode &Node : Field->second.getArray()) {
    if (Node.getKind() == msgpack::Type::UInt)
      Result[I] = Node.getUInt();
    else if (Node.getKind() == msgpack::Type::Int && Node.getInt() >= 0)
      Result[I] = static_cast<uint64_t>(Node.getInt());
    else
      return pipelineError("AMDGPU metadata workgroup dimension is not a "
                           "nonnegative integer");
    if (Result[I] == 0)
      return pipelineError("AMDGPU metadata workgroup dimension is zero");
    ++I;
  }
  return std::optional<std::array<uint64_t, 3>>(Result);
}

Error appendMetadataBlob(StringRef MetadataBlob, MetadataContract &Result,
                         std::set<std::string> &Names,
                         std::set<std::string> &Symbols,
                         MetadataValidationPolicy Policy) {
  if (MetadataBlob.empty())
    return pipelineError("linked output has an empty AMDGPU metadata note");
  msgpack::Document Document;
  if (!Document.readFromBlob(MetadataBlob, false))
    return pipelineError("linked output has malformed AMDGPU metadata");
  AMDGPU::HSAMD::V3::MetadataVerifier Verifier(true);
  if (!Verifier.verify(Document.getRoot()))
    return pipelineError("linked output has invalid AMDGPU metadata schema");

  auto &Root = Document.getRoot().getMap();
  if (Policy != MetadataValidationPolicy::Generic) {
    if (Error E = validateExactMetadataKeys(Root, exactMetadataCheck(Policy)))
      return E;
  }
  auto Target = metadataString(Root, "amdhsa.target");
  if (!Target)
    return Target.takeError();
  if (Result.Target && *Result.Target != *Target)
    return pipelineError(
        "linked output has conflicting AMDGPU metadata targets");
  Result.Target = Target->str();
  auto KernelsField = requiredMetadataField(Root, "amdhsa.kernels");
  if (!KernelsField)
    return KernelsField.takeError();
  if (!(**KernelsField).isArray())
    return pipelineError("AMDGPU metadata kernels field is not an array");

  for (msgpack::DocNode &KernelNode : (**KernelsField).getArray()) {
    if (!KernelNode.isMap())
      return pipelineError("AMDGPU metadata kernel is not a map");
    auto &Kernel = KernelNode.getMap();
    auto Name = metadataString(Kernel, ".name");
    if (!Name)
      return Name.takeError();
    auto Symbol = metadataString(Kernel, ".symbol");
    if (!Symbol)
      return Symbol.takeError();
    auto KernargSize = metadataUnsigned(Kernel, ".kernarg_segment_size");
    if (!KernargSize)
      return KernargSize.takeError();
    auto GroupSize = metadataUnsigned(Kernel, ".group_segment_fixed_size");
    if (!GroupSize)
      return GroupSize.takeError();
    auto PrivateSize = metadataUnsigned(Kernel, ".private_segment_fixed_size");
    if (!PrivateSize)
      return PrivateSize.takeError();
    auto KernargAlign = metadataUnsigned(Kernel, ".kernarg_segment_align");
    if (!KernargAlign)
      return KernargAlign.takeError();
    auto Wavefront = metadataUnsigned(Kernel, ".wavefront_size");
    if (!Wavefront)
      return Wavefront.takeError();
    auto MaxWorkgroup = metadataUnsigned(Kernel, ".max_flat_workgroup_size");
    if (!MaxWorkgroup)
      return MaxWorkgroup.takeError();
    auto RequiredWorkgroup = metadataWorkgroupSize(Kernel);
    if (!RequiredWorkgroup)
      return RequiredWorkgroup.takeError();
    auto SgprCount = metadataUnsigned(Kernel, ".sgpr_count");
    if (!SgprCount)
      return SgprCount.takeError();
    auto VgprCount = metadataUnsigned(Kernel, ".vgpr_count");
    if (!VgprCount)
      return VgprCount.takeError();
    auto AgprCount = metadataOptionalUnsigned(Kernel, ".agpr_count");
    if (!AgprCount)
      return AgprCount.takeError();
    auto SgprSpillCount = metadataOptionalUnsigned(Kernel, ".sgpr_spill_count");
    if (!SgprSpillCount)
      return SgprSpillCount.takeError();
    auto VgprSpillCount = metadataOptionalUnsigned(Kernel, ".vgpr_spill_count");
    if (!VgprSpillCount)
      return VgprSpillCount.takeError();
    auto UsesDynamicStack =
        metadataOptionalBoolean(Kernel, ".uses_dynamic_stack");
    if (!UsesDynamicStack)
      return UsesDynamicStack.takeError();
    auto WorkgroupProcessorMode =
        metadataOptionalBoolean(Kernel, ".workgroup_processor_mode");
    if (!WorkgroupProcessorMode)
      return WorkgroupProcessorMode.takeError();
    auto UniformWorkgroupSize =
        metadataOptionalBoolean(Kernel, ".uniform_work_group_size");
    if (!UniformWorkgroupSize)
      return UniformWorkgroupSize.takeError();
    auto Arguments = metadataArguments(Kernel, *KernargSize, Policy);
    if (!Arguments)
      return Arguments.takeError();

    if (!Names.insert(Name->str()).second)
      return pipelineError("AMDGPU metadata repeats a kernel name");
    if (!Symbols.insert(Symbol->str()).second)
      return pipelineError("AMDGPU metadata repeats a kernel symbol");
    if (*Symbol != (Twine(*Name) + ".kd").str())
      return pipelineError(
          "AMDGPU metadata kernel descriptor does not match its entry name");
    if (*KernargAlign == 0 || !isPowerOf2_64(*KernargAlign))
      return pipelineError("AMDGPU metadata kernarg alignment is invalid");
    if (*Wavefront != 32 && *Wavefront != 64)
      return pipelineError("AMDGPU metadata wavefront size is invalid");
    if (*MaxWorkgroup == 0)
      return pipelineError("AMDGPU metadata maximum workgroup size is zero");
    if (*RequiredWorkgroup) {
      uint64_t Product = 1;
      for (uint64_t Dimension : **RequiredWorkgroup) {
        if (Product > std::numeric_limits<uint64_t>::max() / Dimension)
          return pipelineError("AMDGPU metadata workgroup size overflows");
        Product *= Dimension;
      }
      if (Product > *MaxWorkgroup)
        return pipelineError(
            "AMDGPU metadata required workgroup size exceeds its maximum");
    }
    Result.Kernels.push_back(
        {Name->str(), Symbol->str(), *KernargSize, *GroupSize, *PrivateSize,
         *KernargAlign, *Wavefront, *MaxWorkgroup, *RequiredWorkgroup,
         *SgprCount, *VgprCount, *AgprCount, *SgprSpillCount, *VgprSpillCount,
         *UsesDynamicStack, *WorkgroupProcessorMode, *UniformWorkgroupSize,
         std::move(*Arguments)});
  }
  return Error::success();
}

Expected<MetadataContract>
inspectMetadata(const ELFObjectFile<ELF64LE> &ObjectValue,
                MetadataValidationPolicy Policy) {
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();

  std::vector<StringRef> MetadataBlobs;
  for (const ELF64LE::Shdr &Section : *Sections) {
    if (Section.sh_type != ELF::SHT_NOTE)
      continue;
    Error NoteError = Error::success();
    for (const ELF64LE::Note Note : File.notes(Section, NoteError)) {
      if (Note.getName() != "AMDGPU" ||
          Note.getType() != ELF::NT_AMDGPU_METADATA)
        continue;
      if (Section.sh_addralign != 4)
        return pipelineError(
            "AMDGPU metadata note has a noncanonical alignment");
      MetadataBlobs.push_back(Note.getDescAsStringRef(Section.sh_addralign));
    }
    if (NoteError)
      return NoteError;
  }

  MetadataContract Result;
  if (MetadataBlobs.empty())
    return Result;
  Result.Present = true;
  std::set<std::string> Names;
  std::set<std::string> Symbols;
  for (StringRef MetadataBlob : MetadataBlobs)
    if (Error E =
            appendMetadataBlob(MetadataBlob, Result, Names, Symbols, Policy))
      return E;
  llvm::sort(Result.Kernels, [](const KernelLaunchContract &Left,
                                const KernelLaunchContract &Right) {
    return std::tie(Left.Name, Left.Symbol) <
           std::tie(Right.Name, Right.Symbol);
  });
  return Result;
}

bool isPostLinkRelocationSection(uint32_t Type) {
  return Type == ELF::SHT_REL || Type == ELF::SHT_RELA ||
         Type == ELF::SHT_RELR || Type == ELF::SHT_CREL ||
         Type == ELF::SHT_ANDROID_REL || Type == ELF::SHT_ANDROID_RELA ||
         Type == ELF::SHT_ANDROID_RELR;
}

bool isPostLinkRelocationDynamicTag(int64_t Tag) {
  switch (Tag) {
  case 2:          // DT_PLTRELSZ
  case 7:          // DT_RELA
  case 8:          // DT_RELASZ
  case 9:          // DT_RELAENT
  case 17:         // DT_REL
  case 18:         // DT_RELSZ
  case 19:         // DT_RELENT
  case 20:         // DT_PLTREL
  case 23:         // DT_JMPREL
  case 35:         // DT_RELRSZ
  case 36:         // DT_RELR
  case 37:         // DT_RELRENT
  case 0x6000000f: // DT_ANDROID_REL
  case 0x60000010: // DT_ANDROID_RELSZ
  case 0x60000011: // DT_ANDROID_RELA
  case 0x60000012: // DT_ANDROID_RELASZ
    return true;
  default:
    return false;
  }
}

Error validateExactElfClosure(const ELFObjectFile<ELF64LE> &ObjectValue,
                              StringRef Check) {
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  for (const ELF64LE::Shdr &Section : *Sections) {
    if (isPostLinkRelocationSection(Section.sh_type) && Section.sh_size != 0)
      return postLinkError(Check, "residual_relocation_section");
    if (Section.sh_type != ELF::SHT_DYNAMIC)
      continue;
    auto Entries = File.getSectionContentsAsArray<ELF64LE::Dyn>(Section);
    if (!Entries)
      return Entries.takeError();
    for (const ELF64LE::Dyn &Entry : *Entries) {
      int64_t Tag = Entry.getTag();
      if (Tag == ELF::DT_NEEDED)
        return postLinkError(Check, "dynamic_dependency");
      if (isPostLinkRelocationDynamicTag(Tag))
        return postLinkError(Check, "dynamic_relocation_table");
    }
  }
  return Error::success();
}

Error validateExactLdsGemmSlice1ElfClosure(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  return validateExactElfClosure(ObjectValue, "lds_gemm_slice1_profile");
}

Error validateExactRowSoftmaxV1ElfClosure(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  return validateExactElfClosure(ObjectValue, ExactRowSoftmaxV1Check);
}

Error validateExactWave64CollectivesV1ElfClosure(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  return validateExactElfClosure(ObjectValue, "wave64_collectives_v1_profile");
}

Error validateExactFlashAttentionV1ElfClosure(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  return validateExactElfClosure(ObjectValue, "flash_attention_v1_profile");
}

Error validateExactWorkgroupSyncElfClosure(
    const ELFObjectFile<ELF64LE> &ObjectValue,
    const ExactWorkgroupSyncProfile &Profile) {
  return validateExactElfClosure(ObjectValue, Profile.Check);
}

Error validateExactMoeTop2V1ElfClosure(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  return validateExactElfClosure(ObjectValue, ExactMoeTop2V1Check);
}

struct Wave64CallScanner {
  std::unique_ptr<MCRegisterInfo> Registers;
  std::unique_ptr<MCAsmInfo> AsmInfo;
  std::unique_ptr<MCSubtargetInfo> Subtarget;
  std::unique_ptr<MCInstrInfo> Instructions;
  std::unique_ptr<MCContext> Context;
  std::unique_ptr<MCDisassembler> Disassembler;
};

Expected<Wave64CallScanner> createWave64CallScanner() {
  static bool Initialized = [] {
    LLVMInitializeAMDGPUTargetInfo();
    LLVMInitializeAMDGPUTarget();
    LLVMInitializeAMDGPUTargetMC();
    LLVMInitializeAMDGPUDisassembler();
    return true;
  }();
  (void)Initialized;

  Triple TripleValue(AmdGpuTriple);
  std::string LookupError;
  const Target *TargetValue =
      TargetRegistry::lookupTarget("amdgcn", TripleValue, LookupError);
  if (!TargetValue)
    return pipelineError(Twine("AMDGPU target unavailable: ") + LookupError);
  Wave64CallScanner Result;
  Result.Registers.reset(TargetValue->createMCRegInfo(TripleValue));
  Result.Instructions.reset(TargetValue->createMCInstrInfo());
  Result.Subtarget.reset(
      TargetValue->createMCSubtargetInfo(TripleValue, "gfx942", "-xnack"));
  if (!Result.Registers || !Result.Instructions || !Result.Subtarget)
    return pipelineError("AMDGPU MC tables are unavailable");
  MCTargetOptions Options;
  Result.AsmInfo.reset(
      TargetValue->createMCAsmInfo(*Result.Registers, TripleValue, Options));
  if (!Result.AsmInfo)
    return pipelineError("AMDGPU MC assembly info is unavailable");
  Result.Context = std::make_unique<MCContext>(
      TripleValue, Result.AsmInfo.get(), Result.Registers.get(),
      Result.Subtarget.get(), nullptr, &Options);
  Result.Disassembler.reset(
      TargetValue->createMCDisassembler(*Result.Subtarget, *Result.Context));
  if (!Result.Disassembler)
    return pipelineError("AMDGPU MC disassembler is unavailable");
  return Result;
}

Error validateExactWave64NoMachineCalls(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto SectionsOrError = File.sections();
  if (!SectionsOrError)
    return SectionsOrError.takeError();
  ArrayRef<ELF64LE::Shdr> Sections = *SectionsOrError;
  ArrayRef<uint8_t> KernelBytes;
  uint64_t KernelAddress = 0;
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Table : Sections) {
    if (Table.sh_type != ELF::SHT_SYMTAB)
      continue;
    auto Symbols = File.symbols(&Table);
    if (!Symbols)
      return Symbols.takeError();
    auto Strings = File.getStringTableForSymtab(Table, Sections);
    if (!Strings)
      return Strings.takeError();
    for (const ELF64LE::Sym &Symbol : *Symbols) {
      auto Name = Symbol.getName(*Strings);
      if (!Name)
        return Name.takeError();
      if (*Name != ExactWave64CollectivesV1Entry)
        continue;
      ++Matches;
      if (Symbol.getType() != ELF::STT_FUNC || Symbol.st_size == 0 ||
          Symbol.st_shndx == ELF::SHN_XINDEX ||
          Symbol.st_shndx >= Sections.size())
        return postLinkError("wave64_collectives_v1_profile",
                             "machine_entry_symbol");
      const ELF64LE::Shdr &Section = Sections[Symbol.st_shndx];
      if ((Section.sh_flags & (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR)) !=
              (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR) ||
          Symbol.st_value < Section.sh_addr)
        return postLinkError("wave64_collectives_v1_profile",
                             "machine_entry_section");
      uint64_t Offset = Symbol.st_value - Section.sh_addr;
      if (Offset > Section.sh_size || Symbol.st_size > Section.sh_size - Offset)
        return postLinkError("wave64_collectives_v1_profile",
                             "machine_entry_range");
      auto Contents = File.getSectionContents(Section);
      if (!Contents)
        return Contents.takeError();
      KernelBytes = Contents->slice(Offset, Symbol.st_size);
      KernelAddress = Symbol.st_value;
    }
  }
  if (Matches != 1)
    return postLinkError("wave64_collectives_v1_profile",
                         "machine_entry_cardinality");

  auto Scanner = createWave64CallScanner();
  if (!Scanner)
    return postLinkError("wave64_collectives_v1_profile",
                         errorToDiagnostic(Scanner.takeError()));
  uint64_t Offset = 0;
  size_t InstructionCount = 0;
  while (Offset < KernelBytes.size()) {
    if (llvm::all_of(KernelBytes.drop_front(Offset),
                     [](uint8_t Byte) { return Byte == 0; }))
      break;
    MCInst Instruction;
    uint64_t Size = 0;
    auto Status = Scanner->Disassembler->getInstruction(
        Instruction, Size, KernelBytes.drop_front(Offset),
        KernelAddress + Offset, nulls());
    if (Status != MCDisassembler::Success || Size == 0 ||
        Size > KernelBytes.size() - Offset)
      return postLinkError("wave64_collectives_v1_profile",
                           "machine_instruction_decode");
    const MCInstrDesc &Descriptor =
        Scanner->Instructions->get(Instruction.getOpcode());
    if (Descriptor.isCall())
      return postLinkError("wave64_collectives_v1_profile", "machine_call");
    Offset += Size;
    if (++InstructionCount > 1024 * 1024)
      return postLinkError("wave64_collectives_v1_profile",
                           "machine_instruction_bound");
  }
  if (InstructionCount == 0)
    return postLinkError("wave64_collectives_v1_profile",
                         "machine_instruction_empty");
  return Error::success();
}

Error validateExactFlashAttentionMachine(
    const ELFObjectFile<ELF64LE> &ObjectValue) {
  constexpr StringLiteral Check = "flash_attention_v1_profile";
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto SectionsOrError = File.sections();
  if (!SectionsOrError)
    return SectionsOrError.takeError();
  ArrayRef<ELF64LE::Shdr> Sections = *SectionsOrError;
  ArrayRef<uint8_t> KernelBytes;
  uint64_t KernelAddress = 0;
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Table : Sections) {
    if (Table.sh_type != ELF::SHT_SYMTAB)
      continue;
    auto Symbols = File.symbols(&Table);
    if (!Symbols)
      return Symbols.takeError();
    auto Strings = File.getStringTableForSymtab(Table, Sections);
    if (!Strings)
      return Strings.takeError();
    for (const ELF64LE::Sym &Symbol : *Symbols) {
      auto Name = Symbol.getName(*Strings);
      if (!Name)
        return Name.takeError();
      if (*Name != ExactFlashAttentionV1Entry)
        continue;
      ++Matches;
      if (Symbol.getType() != ELF::STT_FUNC || Symbol.st_size == 0 ||
          Symbol.st_shndx == ELF::SHN_XINDEX ||
          Symbol.st_shndx >= Sections.size())
        return postLinkError(Check, "machine_entry_symbol");
      const ELF64LE::Shdr &Section = Sections[Symbol.st_shndx];
      if ((Section.sh_flags & (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR)) !=
              (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR) ||
          Symbol.st_value < Section.sh_addr)
        return postLinkError(Check, "machine_entry_section");
      uint64_t Offset = Symbol.st_value - Section.sh_addr;
      if (Offset > Section.sh_size || Symbol.st_size > Section.sh_size - Offset)
        return postLinkError(Check, "machine_entry_range");
      auto Contents = File.getSectionContents(Section);
      if (!Contents)
        return Contents.takeError();
      KernelBytes = Contents->slice(Offset, Symbol.st_size);
      KernelAddress = Symbol.st_value;
    }
  }
  if (Matches != 1)
    return postLinkError(Check, "machine_entry_cardinality");
  if (SHA256::hash(KernelBytes) != ExactFlashMachineSha256)
    return postLinkError(Check, "machine_identity");

  auto Scanner = createWave64CallScanner();
  if (!Scanner)
    return postLinkError(Check, errorToDiagnostic(Scanner.takeError()));
  uint64_t Offset = 0;
  size_t InstructionCount = 0;
  while (Offset < KernelBytes.size()) {
    if (llvm::all_of(KernelBytes.drop_front(Offset),
                     [](uint8_t Byte) { return Byte == 0; }))
      break;
    MCInst Instruction;
    uint64_t Size = 0;
    auto Status = Scanner->Disassembler->getInstruction(
        Instruction, Size, KernelBytes.drop_front(Offset),
        KernelAddress + Offset, nulls());
    if (Status != MCDisassembler::Success || Size == 0 ||
        Size > KernelBytes.size() - Offset)
      return postLinkError(Check, "machine_instruction_decode");
    const MCInstrDesc &Descriptor =
        Scanner->Instructions->get(Instruction.getOpcode());
    if (Descriptor.isCall())
      return postLinkError(Check, "machine_call");
    StringRef Opcode = Scanner->Instructions->getName(Instruction.getOpcode());
    if (Opcode.starts_with("DS_") || Opcode.starts_with("FLAT_") ||
        Opcode.starts_with("TBUFFER_") || Opcode.starts_with("IMAGE_") ||
        Opcode.starts_with("SCRATCH_") || Opcode.contains("ATOMIC"))
      return postLinkError(Check, "machine_forbidden_opcode");
    Offset += Size;
    if (++InstructionCount > 1024 * 1024)
      return postLinkError(Check, "machine_instruction_bound");
  }
  if (Offset != KernelBytes.size() || InstructionCount != 482)
    return postLinkError(Check, "machine_instruction_identity");
  return Error::success();
}

Error validateExactWorkgroupSyncMachine(
    const ELFObjectFile<ELF64LE> &ObjectValue,
    const ExactWorkgroupSyncProfile &Profile) {
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto SectionsOrError = File.sections();
  if (!SectionsOrError)
    return SectionsOrError.takeError();
  ArrayRef<ELF64LE::Shdr> Sections = *SectionsOrError;
  ArrayRef<uint8_t> KernelBytes;
  uint64_t KernelAddress = 0;
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Table : Sections) {
    if (Table.sh_type != ELF::SHT_SYMTAB)
      continue;
    auto Symbols = File.symbols(&Table);
    if (!Symbols)
      return Symbols.takeError();
    auto Strings = File.getStringTableForSymtab(Table, Sections);
    if (!Strings)
      return Strings.takeError();
    for (const ELF64LE::Sym &Symbol : *Symbols) {
      auto Name = Symbol.getName(*Strings);
      if (!Name)
        return Name.takeError();
      if (*Name != Profile.Entry)
        continue;
      ++Matches;
      if (Symbol.getType() != ELF::STT_FUNC || Symbol.st_size == 0 ||
          Symbol.st_shndx == ELF::SHN_XINDEX ||
          Symbol.st_shndx >= Sections.size())
        return postLinkError(Profile.Check, "machine_entry_symbol");
      const ELF64LE::Shdr &Section = Sections[Symbol.st_shndx];
      if ((Section.sh_flags & (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR)) !=
              (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR) ||
          Symbol.st_value < Section.sh_addr)
        return postLinkError(Profile.Check, "machine_entry_section");
      uint64_t Offset = Symbol.st_value - Section.sh_addr;
      if (Offset > Section.sh_size || Symbol.st_size > Section.sh_size - Offset)
        return postLinkError(Profile.Check, "machine_entry_range");
      auto Contents = File.getSectionContents(Section);
      if (!Contents)
        return Contents.takeError();
      KernelBytes = Contents->slice(Offset, Symbol.st_size);
      KernelAddress = Symbol.st_value;
    }
  }
  if (Matches != 1)
    return postLinkError(Profile.Check, "machine_entry_cardinality");

  auto Scanner = createWave64CallScanner();
  if (!Scanner)
    return postLinkError(Profile.Check, errorToDiagnostic(Scanner.takeError()));
  uint64_t Offset = 0;
  size_t InstructionCount = 0;
  size_t BarrierCount = 0;
  size_t LdsReadCount = 0;
  size_t LdsWriteCount = 0;
  size_t AtomicAddCount = 0;
  while (Offset < KernelBytes.size()) {
    if (llvm::all_of(KernelBytes.drop_front(Offset),
                     [](uint8_t Byte) { return Byte == 0; }))
      break;
    MCInst Instruction;
    uint64_t Size = 0;
    auto Status = Scanner->Disassembler->getInstruction(
        Instruction, Size, KernelBytes.drop_front(Offset),
        KernelAddress + Offset, nulls());
    if (Status != MCDisassembler::Success || Size == 0 ||
        Size > KernelBytes.size() - Offset)
      return postLinkError(Profile.Check, "machine_instruction_decode");
    const MCInstrDesc &Descriptor =
        Scanner->Instructions->get(Instruction.getOpcode());
    if (Descriptor.isCall())
      return postLinkError(Profile.Check, "machine_call");
    StringRef Opcode = Scanner->Instructions->getName(Instruction.getOpcode());
    BarrierCount += Opcode.contains("S_BARRIER");
    LdsReadCount += Opcode.contains("DS_READ");
    LdsWriteCount += Opcode.contains("DS_WRITE");
    AtomicAddCount += Opcode.contains("ATOMIC_ADD");
    Offset += Size;
    if (++InstructionCount > 1024 * 1024)
      return postLinkError(Profile.Check, "machine_instruction_bound");
  }
  if (InstructionCount == 0)
    return postLinkError(Profile.Check, "machine_instruction_empty");
  std::array<uint8_t, 32> MachineIdentity = SHA256::hash(KernelBytes);
  if (MachineIdentity != Profile.MachineSha256)
    return postLinkError(
        Profile.Check,
        (Twine("machine_identity_") + digestHex(MachineIdentity)).str());
  if (Profile.Kind == ExactWorkgroupSyncKind::LdsReduction) {
    if (BarrierCount != 0 || LdsReadCount != 32 || LdsWriteCount != 1 ||
        AtomicAddCount != 0) {
      std::string Reason =
          (Twine("machine_lds_barrier_effect_barriers_") + Twine(BarrierCount) +
           "_reads_" + Twine(LdsReadCount) + "_writes_" + Twine(LdsWriteCount) +
           "_atomics_" + Twine(AtomicAddCount))
              .str();
      return postLinkError(Profile.Check, Reason);
    }
  } else if (BarrierCount != 0 || LdsReadCount != 0 || LdsWriteCount != 0 ||
             AtomicAddCount != 1) {
    return postLinkError(Profile.Check, "machine_atomic_effect");
  }
  return Error::success();
}

Error validateExactMoeTop2V1Machine(const ELFObjectFile<ELF64LE> &ObjectValue) {
  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto SectionsOrError = File.sections();
  if (!SectionsOrError)
    return SectionsOrError.takeError();
  ArrayRef<ELF64LE::Shdr> Sections = *SectionsOrError;
  ArrayRef<uint8_t> KernelBytes;
  uint64_t KernelAddress = 0;
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Table : Sections) {
    if (Table.sh_type != ELF::SHT_SYMTAB)
      continue;
    auto Symbols = File.symbols(&Table);
    if (!Symbols)
      return Symbols.takeError();
    auto Strings = File.getStringTableForSymtab(Table, Sections);
    if (!Strings)
      return Strings.takeError();
    for (const ELF64LE::Sym &Symbol : *Symbols) {
      auto Name = Symbol.getName(*Strings);
      if (!Name)
        return Name.takeError();
      if (*Name != ExactMoeTop2V1Entry)
        continue;
      ++Matches;
      if (Symbol.getType() != ELF::STT_FUNC || Symbol.st_size == 0 ||
          Symbol.st_shndx == ELF::SHN_XINDEX ||
          Symbol.st_shndx >= Sections.size())
        return postLinkError(ExactMoeTop2V1Check, "machine_entry_symbol");
      const ELF64LE::Shdr &Section = Sections[Symbol.st_shndx];
      if ((Section.sh_flags & (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR)) !=
              (ELF::SHF_ALLOC | ELF::SHF_EXECINSTR) ||
          Symbol.st_value < Section.sh_addr)
        return postLinkError(ExactMoeTop2V1Check, "machine_entry_section");
      uint64_t Offset = Symbol.st_value - Section.sh_addr;
      if (Offset > Section.sh_size || Symbol.st_size > Section.sh_size - Offset)
        return postLinkError(ExactMoeTop2V1Check, "machine_entry_range");
      auto Contents = File.getSectionContents(Section);
      if (!Contents)
        return Contents.takeError();
      KernelBytes = Contents->slice(Offset, Symbol.st_size);
      KernelAddress = Symbol.st_value;
    }
  }
  if (Matches != 1)
    return postLinkError(ExactMoeTop2V1Check, "machine_entry_cardinality");

  auto Scanner = createWave64CallScanner();
  if (!Scanner)
    return postLinkError(ExactMoeTop2V1Check,
                         errorToDiagnostic(Scanner.takeError()));
  uint64_t Offset = 0;
  size_t InstructionCount = 0;
  size_t MemoryLoads = 0;
  size_t MemoryStores = 0;
  while (Offset < KernelBytes.size()) {
    if (llvm::all_of(KernelBytes.drop_front(Offset),
                     [](uint8_t Byte) { return Byte == 0; }))
      break;
    MCInst Instruction;
    uint64_t Size = 0;
    auto Status = Scanner->Disassembler->getInstruction(
        Instruction, Size, KernelBytes.drop_front(Offset),
        KernelAddress + Offset, nulls());
    if (Status != MCDisassembler::Success || Size == 0 ||
        Size > KernelBytes.size() - Offset)
      return postLinkError(ExactMoeTop2V1Check, "machine_instruction_decode");
    const MCInstrDesc &Descriptor =
        Scanner->Instructions->get(Instruction.getOpcode());
    if (Descriptor.isCall())
      return postLinkError(ExactMoeTop2V1Check, "machine_call");
    StringRef Opcode = Scanner->Instructions->getName(Instruction.getOpcode());
    if (Opcode.contains("DS_") || Opcode.contains("BARRIER") ||
        Opcode.contains("ATOMIC") || Opcode.contains("SCRATCH"))
      return postLinkError(ExactMoeTop2V1Check,
                           "machine_forbidden_memory_effect");
    MemoryLoads += Descriptor.mayLoad();
    MemoryStores += Descriptor.mayStore();
    Offset += Size;
    if (++InstructionCount > 1024 * 1024)
      return postLinkError(ExactMoeTop2V1Check, "machine_instruction_bound");
  }
  if (InstructionCount == 0 || MemoryLoads == 0 || MemoryStores == 0)
    return postLinkError(ExactMoeTop2V1Check, "machine_effect_shape");
  std::array<uint8_t, 32> MachineIdentity = SHA256::hash(KernelBytes);
  if (!bytesMatchLowerHex(MachineIdentity, ExactMoeTop2V1MachineSha256))
    return postLinkError(
        ExactMoeTop2V1Check,
        (Twine("machine_identity_") + digestHex(MachineIdentity)).str());
  return Error::success();
}

Error validateExactWave64DescriptorBinding(
    const ELFObjectFile<ELF64LE> &ObjectValue, const Request &RequestValue) {
  StringRef CompilerBytes(
      reinterpret_cast<const char *>(RequestValue.CompilerModule.Bytes.data()),
      RequestValue.CompilerModule.Bytes.size());
  auto InputSections = parseExactWave64CompilerSections(CompilerBytes);
  if (!InputSections)
    return postLinkError("wave64_collectives_v1_profile",
                         errorToDiagnostic(InputSections.takeError()));
  ArrayRef<uint8_t> ExpectedDescriptor = (*InputSections)[0];

  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    if (*Name != ExactWave64DescriptorSection)
      continue;
    ++Matches;
    if (Section.sh_type != ELF::SHT_PROGBITS || Section.sh_addralign != 8)
      return postLinkError("wave64_collectives_v1_profile",
                           "descriptor_section_envelope");
    auto Contents = File.getSectionContents(Section);
    if (!Contents)
      return Contents.takeError();
    if (*Contents != ExpectedDescriptor)
      return postLinkError("wave64_collectives_v1_profile",
                           "descriptor_section_identity");
  }
  if (Matches != 1)
    return postLinkError("wave64_collectives_v1_profile",
                         "descriptor_section_cardinality");
  return Error::success();
}

Error validateExactRowSoftmaxV1DescriptorBinding(
    const ELFObjectFile<ELF64LE> &ObjectValue, const Request &RequestValue) {
  StringRef CompilerBytes(
      reinterpret_cast<const char *>(RequestValue.CompilerModule.Bytes.data()),
      RequestValue.CompilerModule.Bytes.size());
  auto InputSections = parseExactRowSoftmaxV1CompilerSections(CompilerBytes);
  if (!InputSections)
    return postLinkError(ExactRowSoftmaxV1Check,
                         errorToDiagnostic(InputSections.takeError()));
  ArrayRef<uint8_t> ExpectedDescriptor = (*InputSections)[0];

  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    if (*Name != ExactRowDescriptorSection)
      continue;
    ++Matches;
    if (Section.sh_type != ELF::SHT_PROGBITS || Section.sh_flags != 0 ||
        Section.sh_addralign != 8)
      return postLinkError(ExactRowSoftmaxV1Check,
                           "descriptor_section_envelope");
    auto Contents = File.getSectionContents(Section);
    if (!Contents)
      return Contents.takeError();
    if (*Contents != ExpectedDescriptor)
      return postLinkError(ExactRowSoftmaxV1Check,
                           "descriptor_section_identity");
  }
  if (Matches != 1)
    return postLinkError(ExactRowSoftmaxV1Check,
                         "descriptor_section_cardinality");
  return Error::success();
}

Error validateExactWorkgroupSyncDescriptorBinding(
    const ELFObjectFile<ELF64LE> &ObjectValue, const Request &RequestValue,
    const ExactWorkgroupSyncProfile &Profile) {
  StringRef CompilerBytes(
      reinterpret_cast<const char *>(RequestValue.CompilerModule.Bytes.data()),
      RequestValue.CompilerModule.Bytes.size());
  auto InputSections =
      parseExactWorkgroupSyncCompilerSections(CompilerBytes, Profile);
  if (!InputSections)
    return postLinkError(Profile.Check,
                         errorToDiagnostic(InputSections.takeError()));
  ArrayRef<uint8_t> ExpectedDescriptor = (*InputSections)[0];

  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    if (*Name != ExactWave64DescriptorSection)
      continue;
    ++Matches;
    if (Section.sh_type != ELF::SHT_PROGBITS || Section.sh_addralign != 8)
      return postLinkError(Profile.Check, "descriptor_section_envelope");
    auto Contents = File.getSectionContents(Section);
    if (!Contents)
      return Contents.takeError();
    if (*Contents != ExpectedDescriptor)
      return postLinkError(Profile.Check, "descriptor_section_identity");
  }
  if (Matches != 1)
    return postLinkError(Profile.Check, "descriptor_section_cardinality");
  return Error::success();
}

Error validateExactFlashAttentionDescriptorBinding(
    const ELFObjectFile<ELF64LE> &ObjectValue, const Request &RequestValue) {
  StringRef CompilerBytes(
      reinterpret_cast<const char *>(RequestValue.CompilerModule.Bytes.data()),
      RequestValue.CompilerModule.Bytes.size());
  auto InputSections = parseExactFlashAttentionCompilerSections(CompilerBytes);
  if (!InputSections)
    return postLinkError("flash_attention_v1_profile",
                         errorToDiagnostic(InputSections.takeError()));
  ArrayRef<uint8_t> ExpectedDescriptor = (*InputSections)[0];

  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    if (*Name != ExactFlashDescriptorSection)
      continue;
    ++Matches;
    if (Section.sh_type != ELF::SHT_PROGBITS || Section.sh_addralign != 8)
      return postLinkError("flash_attention_v1_profile",
                           "descriptor_section_envelope");
    auto Contents = File.getSectionContents(Section);
    if (!Contents)
      return Contents.takeError();
    if (*Contents != ExpectedDescriptor)
      return postLinkError("flash_attention_v1_profile",
                           "descriptor_section_identity");
  }
  if (Matches != 1)
    return postLinkError("flash_attention_v1_profile",
                         "descriptor_section_cardinality");
  return Error::success();
}

Error validateExactMoeTop2V1DescriptorBinding(
    const ELFObjectFile<ELF64LE> &ObjectValue, const Request &RequestValue) {
  StringRef CompilerBytes(
      reinterpret_cast<const char *>(RequestValue.CompilerModule.Bytes.data()),
      RequestValue.CompilerModule.Bytes.size());
  auto InputSections = parseExactMoeTop2V1CompilerSections(CompilerBytes);
  if (!InputSections)
    return postLinkError(ExactMoeTop2V1Check,
                         errorToDiagnostic(InputSections.takeError()));
  ArrayRef<uint8_t> ExpectedDescriptor = (*InputSections)[0];

  const ELFFile<ELF64LE> &File = ObjectValue.getELFFile();
  auto Sections = File.sections();
  if (!Sections)
    return Sections.takeError();
  size_t Matches = 0;
  for (const ELF64LE::Shdr &Section : *Sections) {
    auto Name = File.getSectionName(Section);
    if (!Name)
      return Name.takeError();
    if (*Name != ExactWave64DescriptorSection)
      continue;
    ++Matches;
    if (Section.sh_type != ELF::SHT_PROGBITS || Section.sh_addralign != 8)
      return postLinkError(ExactMoeTop2V1Check, "descriptor_section_envelope");
    auto Contents = File.getSectionContents(Section);
    if (!Contents)
      return Contents.takeError();
    if (*Contents != ExpectedDescriptor)
      return postLinkError(ExactMoeTop2V1Check, "descriptor_section_identity");
  }
  if (Matches != 1)
    return postLinkError(ExactMoeTop2V1Check, "descriptor_section_cardinality");
  return Error::success();
}

Error validateExactRowSoftmaxV1Metadata(const MetadataContract &Metadata) {
  static constexpr std::array<uint64_t, 3> Workgroup = {64, 1, 1};
  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 19> Hidden = {{
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
  auto Mismatch = [](StringRef Field) {
    return postLinkError(ExactRowSoftmaxV1Check,
                         (Twine("kernel_contract_") + Field).str());
  };
  auto HasOptionalArgumentField = [](const KernelArgumentContract &Argument) {
    return Argument.TypeName || Argument.Align || Argument.ValueType ||
           Argument.Access || Argument.ActualAccess || Argument.PointeeAlign ||
           Argument.IsConst || Argument.IsRestrict || Argument.IsVolatile ||
           Argument.IsPipe;
  };
  if (Metadata.Kernels.size() != 1)
    return postLinkError(ExactRowSoftmaxV1Check, "kernel_cardinality");
  const KernelLaunchContract &Kernel = Metadata.Kernels.front();
  if (Kernel.Name != ExactRowSoftmaxV1Entry ||
      Kernel.Symbol != ExactRowSoftmaxV1Descriptor)
    return Mismatch("symbols");
  if (!Kernel.RequiredWorkgroupSize ||
      *Kernel.RequiredWorkgroupSize != Workgroup)
    return Mismatch("reqd_workgroup_size");
  if (Kernel.MaxFlatWorkgroupSize != 64)
    return Mismatch("max_flat_workgroup_size");
  if (Kernel.WavefrontSize != 64)
    return Mismatch("wavefront_size");
  if (Kernel.KernargSegmentSize != 288)
    return Mismatch("kernarg_segment_size");
  if (Kernel.KernargSegmentAlign != 8)
    return Mismatch("kernarg_segment_align");
  if (Kernel.GroupSegmentFixedSize != 0)
    return Mismatch("group_segment_fixed_size");
  if (Kernel.PrivateSegmentFixedSize != 0)
    return Mismatch("private_segment_fixed_size");
  if (Kernel.SgprCount != 42)
    return Mismatch("sgpr_count");
  if (Kernel.VgprCount != 88)
    return Mismatch("vgpr_count");
  if (!Kernel.AgprCount || *Kernel.AgprCount != 44)
    return Mismatch("agpr_count");
  if (!Kernel.SgprSpillCount || *Kernel.SgprSpillCount != 44)
    return Mismatch("sgpr_spill_count");
  if (!Kernel.VgprSpillCount || *Kernel.VgprSpillCount != 28)
    return Mismatch("vgpr_spill_count");
  if (!Kernel.UsesDynamicStack || *Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack");
  if (Kernel.WorkgroupProcessorMode)
    return Mismatch("workgroup_processor_mode");
  if (Kernel.UniformWorkgroupSize)
    return Mismatch("uniform_work_group_size");

  if (!Kernel.Arguments || Kernel.Arguments->size() != 4 + Hidden.size())
    return Mismatch("args");
  const std::vector<KernelArgumentContract> &Arguments = *Kernel.Arguments;
  static constexpr std::array<StringLiteral, 2> PointerNames = {"arg0.data",
                                                                "arg1.data"};
  static constexpr std::array<StringLiteral, 2> LengthNames = {"arg0.len",
                                                               "arg1.len"};
  for (size_t Slice = 0; Slice != 2; ++Slice) {
    size_t PointerIndex = Slice * 2;
    const KernelArgumentContract &Pointer = Arguments[PointerIndex];
    if (!Pointer.Name || *Pointer.Name != PointerNames[Slice] ||
        Pointer.Offset != Slice * 16 || Pointer.Size != 8 ||
        Pointer.ValueKind != "global_buffer" || !Pointer.AddressSpace ||
        *Pointer.AddressSpace != "global" || HasOptionalArgumentField(Pointer))
      return Mismatch((Twine("arg") + Twine(PointerIndex) + "_pointer").str());

    const KernelArgumentContract &Length = Arguments[PointerIndex + 1];
    if (!Length.Name || *Length.Name != LengthNames[Slice] ||
        Length.Offset != Slice * 16 + 8 || Length.Size != 8 ||
        Length.ValueKind != "by_value" || Length.AddressSpace ||
        HasOptionalArgumentField(Length))
      return Mismatch(
          (Twine("arg") + Twine(PointerIndex + 1) + "_length").str());
  }
  for (size_t Index = 0; Index != Hidden.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[4 + Index];
    const HiddenArgumentShape &Expected = Hidden[Index];
    if (Argument.Offset != Expected.Offset || Argument.Size != Expected.Size ||
        Argument.ValueKind != Expected.ValueKind || Argument.Name ||
        Argument.AddressSpace || HasOptionalArgumentField(Argument))
      return Mismatch((Twine("hidden_arg") + Twine(Index)).str());
  }
  return Error::success();
}

Error validateExactLdsGemmSlice1Metadata(const MetadataContract &Metadata) {
  static constexpr std::array<uint64_t, 3> Workgroup = {64, 1, 1};
  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 13> RequiredHidden = {{
      {0, 4, "hidden_block_count_x"},
      {4, 4, "hidden_block_count_y"},
      {8, 4, "hidden_block_count_z"},
      {12, 2, "hidden_group_size_x"},
      {14, 2, "hidden_group_size_y"},
      {16, 2, "hidden_group_size_z"},
      {18, 2, "hidden_remainder_x"},
      {20, 2, "hidden_remainder_y"},
      {22, 2, "hidden_remainder_z"},
      {40, 8, "hidden_global_offset_x"},
      {48, 8, "hidden_global_offset_y"},
      {56, 8, "hidden_global_offset_z"},
      {64, 2, "hidden_grid_dims"},
  }};
  static constexpr std::array<HiddenArgumentShape, 10> OptionalHidden = {{
      {72, 8, "hidden_printf_buffer"},
      {80, 8, "hidden_hostcall_buffer"},
      {88, 8, "hidden_multigrid_sync_arg"},
      {96, 8, "hidden_heap_v1"},
      {104, 8, "hidden_default_queue"},
      {112, 8, "hidden_completion_action"},
      {120, 4, "hidden_dynamic_lds_size"},
      {192, 4, "hidden_private_base"},
      {196, 4, "hidden_shared_base"},
      {200, 8, "hidden_queue_ptr"},
  }};
  if (Metadata.Kernels.size() != 1)
    return postLinkError("lds_gemm_slice1_profile", "kernel_cardinality");
  const KernelLaunchContract &Kernel = Metadata.Kernels.front();
  auto Mismatch = [&](StringRef Field) {
    return postLinkError("lds_gemm_slice1_profile",
                         (Twine("kernel_contract_") + Field).str());
  };
  if (Kernel.Name != ExactLdsGemmSlice1Entry ||
      Kernel.Symbol != ExactLdsGemmSlice1Descriptor)
    return Mismatch("symbols");
  if (!Kernel.RequiredWorkgroupSize ||
      *Kernel.RequiredWorkgroupSize != Workgroup)
    return Mismatch("reqd_workgroup_size");
  if (Kernel.MaxFlatWorkgroupSize != 64)
    return Mismatch("max_flat_workgroup_size");
  if (Kernel.WavefrontSize != 64)
    return Mismatch("wavefront_size");
  if (Kernel.KernargSegmentSize != 304)
    return Mismatch("kernarg_segment_size");
  if (Kernel.KernargSegmentAlign != 8)
    return Mismatch("kernarg_segment_align");
  if (Kernel.GroupSegmentFixedSize != 1024)
    return Mismatch("group_segment_fixed_size");
  if (Kernel.PrivateSegmentFixedSize != 0)
    return Mismatch("private_segment_fixed_size");
  if (!Kernel.SgprSpillCount)
    return Mismatch("sgpr_spill_count_missing");
  if (*Kernel.SgprSpillCount != 0)
    return Mismatch("sgpr_spill_count");
  if (!Kernel.VgprSpillCount)
    return Mismatch("vgpr_spill_count_missing");
  if (*Kernel.VgprSpillCount != 0)
    return Mismatch("vgpr_spill_count");
  if (!Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack_missing");
  if (*Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack");
  if (!Kernel.Arguments)
    return Mismatch("args_missing");
  const std::vector<KernelArgumentContract> &Arguments = *Kernel.Arguments;
  if (Arguments.size() < 6 + RequiredHidden.size())
    return Mismatch("args_cardinality");

  auto ArgumentFailure = [&](size_t Role, StringRef Part, StringRef Field,
                             bool Missing = false) {
    return Mismatch((Twine("arg") + Twine(Role) + "_" + Part + "_" +
                     (Missing ? "missing_" : "") + Field)
                        .str());
  };
  for (size_t Role = 0; Role != 3; ++Role) {
    const KernelArgumentContract &Pointer = Arguments[Role * 2];
    const std::string PointerName =
        (Twine("arg") + Twine(Role) + ".data").str();
    const StringRef PointerTypeName = Role < 2 ? "ushort*" : "float*";
    const StringRef PointerValueType = Role < 2 ? "u16" : "f32";
    const StringRef PointerAccess = Role < 2 ? "read_only" : "read_write";
    const uint64_t PointeeAlign = Role < 2 ? 2 : 4;
    if (!Pointer.Name)
      return ArgumentFailure(Role, "data", "name", true);
    if (*Pointer.Name != PointerName)
      return ArgumentFailure(Role, "data", "name");
    if (Pointer.Offset != Role * 16)
      return ArgumentFailure(Role, "data", "offset");
    if (Pointer.Size != 8)
      return ArgumentFailure(Role, "data", "size");
    if (!Pointer.TypeName)
      return ArgumentFailure(Role, "data", "type_name", true);
    if (*Pointer.TypeName != PointerTypeName)
      return ArgumentFailure(Role, "data", "type_name");
    if (Pointer.Align && *Pointer.Align != 8)
      return ArgumentFailure(Role, "data", "align");
    if (Pointer.ValueKind != "global_buffer")
      return ArgumentFailure(Role, "data", "value_kind");
    if (Pointer.ValueType && *Pointer.ValueType != PointerValueType)
      return ArgumentFailure(Role, "data", "value_type");
    if (!Pointer.AddressSpace)
      return ArgumentFailure(Role, "data", "address_space", true);
    if (*Pointer.AddressSpace != "global")
      return ArgumentFailure(Role, "data", "address_space");
    if (!Pointer.Access)
      return ArgumentFailure(Role, "data", "access", true);
    if (*Pointer.Access != PointerAccess)
      return ArgumentFailure(Role, "data", "access");
    if (Role < 2 && !Pointer.ActualAccess)
      return ArgumentFailure(Role, "data", "actual_access", true);
    if (Pointer.ActualAccess &&
        (Role < 2 ? *Pointer.ActualAccess != "read_only"
                  : *Pointer.ActualAccess != "read_only" &&
                        *Pointer.ActualAccess != "write_only" &&
                        *Pointer.ActualAccess != "read_write"))
      return ArgumentFailure(Role, "data", "actual_access");
    if (Pointer.PointeeAlign && *Pointer.PointeeAlign != PointeeAlign)
      return ArgumentFailure(Role, "data", "pointee_align");
    if (Role < 2 && !Pointer.IsConst)
      return ArgumentFailure(Role, "data", "is_const", true);
    if (Pointer.IsConst && *Pointer.IsConst != (Role < 2))
      return ArgumentFailure(Role, "data", "is_const");
    if (Role == 2 && !Pointer.IsRestrict)
      return ArgumentFailure(Role, "data", "is_restrict", true);
    if (Pointer.IsRestrict && *Pointer.IsRestrict != (Role == 2))
      return ArgumentFailure(Role, "data", "is_restrict");
    if (Pointer.IsVolatile && *Pointer.IsVolatile)
      return ArgumentFailure(Role, "data", "is_volatile");
    if (Pointer.IsPipe && *Pointer.IsPipe)
      return ArgumentFailure(Role, "data", "is_pipe");

    const KernelArgumentContract &Length = Arguments[Role * 2 + 1];
    const std::string LengthName = (Twine("arg") + Twine(Role) + ".len").str();
    if (!Length.Name)
      return ArgumentFailure(Role, "len", "name", true);
    if (*Length.Name != LengthName)
      return ArgumentFailure(Role, "len", "name");
    if (Length.Offset != Role * 16 + 8)
      return ArgumentFailure(Role, "len", "offset");
    if (Length.Size != 8)
      return ArgumentFailure(Role, "len", "size");
    if (!Length.TypeName)
      return ArgumentFailure(Role, "len", "type_name", true);
    if (*Length.TypeName != "ulong")
      return ArgumentFailure(Role, "len", "type_name");
    if (Length.Align && *Length.Align != 8)
      return ArgumentFailure(Role, "len", "align");
    if (Length.ValueKind != "by_value")
      return ArgumentFailure(Role, "len", "value_kind");
    if (Length.ValueType && *Length.ValueType != "u64")
      return ArgumentFailure(Role, "len", "value_type");
    if (Length.AddressSpace || Length.Access || Length.ActualAccess ||
        Length.PointeeAlign || (Length.IsConst && *Length.IsConst) ||
        (Length.IsRestrict && *Length.IsRestrict) ||
        (Length.IsVolatile && *Length.IsVolatile) ||
        (Length.IsPipe && *Length.IsPipe))
      return ArgumentFailure(Role, "len", "pointer_qualifier");
  }

  const size_t HiddenBaseIndex = 6;
  const uint64_t HiddenBase = 48;
  auto ValidateHidden = [&](const KernelArgumentContract &Argument,
                            const HiddenArgumentShape &Expected) {
    return !Argument.Name && !Argument.TypeName &&
           Argument.Offset == HiddenBase + Expected.Offset &&
           Argument.Size == Expected.Size &&
           Argument.ValueKind == Expected.ValueKind && !Argument.Align &&
           !Argument.ValueType && !Argument.AddressSpace && !Argument.Access &&
           !Argument.ActualAccess && !Argument.PointeeAlign &&
           !Argument.IsConst && !Argument.IsRestrict && !Argument.IsVolatile &&
           !Argument.IsPipe;
  };
  for (size_t Index = 0; Index != RequiredHidden.size(); ++Index)
    if (!ValidateHidden(Arguments[HiddenBaseIndex + Index],
                        RequiredHidden[Index]))
      return Mismatch((Twine("hidden_arg") + Twine(Index)).str());
  for (size_t Index = HiddenBaseIndex + RequiredHidden.size();
       Index != Arguments.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto Expected = llvm::find_if(OptionalHidden, [&](const auto &Shape) {
      return Argument.Offset == HiddenBase + Shape.Offset;
    });
    if (Expected == OptionalHidden.end() ||
        !ValidateHidden(Argument, *Expected))
      return Mismatch(
          (Twine("hidden_arg") + Twine(Index - HiddenBaseIndex)).str());
  }
  return Error::success();
}

Error validateExactWave64CollectivesV1Metadata(
    const MetadataContract &Metadata) {
  constexpr StringLiteral Check = "wave64_collectives_v1_profile";
  static constexpr std::array<uint64_t, 3> Workgroup = {64, 1, 1};
  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 13> RequiredHidden = {{
      {0, 4, "hidden_block_count_x"},
      {4, 4, "hidden_block_count_y"},
      {8, 4, "hidden_block_count_z"},
      {12, 2, "hidden_group_size_x"},
      {14, 2, "hidden_group_size_y"},
      {16, 2, "hidden_group_size_z"},
      {18, 2, "hidden_remainder_x"},
      {20, 2, "hidden_remainder_y"},
      {22, 2, "hidden_remainder_z"},
      {40, 8, "hidden_global_offset_x"},
      {48, 8, "hidden_global_offset_y"},
      {56, 8, "hidden_global_offset_z"},
      {64, 2, "hidden_grid_dims"},
  }};
  static constexpr std::array<HiddenArgumentShape, 10> OptionalHidden = {{
      {72, 8, "hidden_printf_buffer"},
      {80, 8, "hidden_hostcall_buffer"},
      {88, 8, "hidden_multigrid_sync_arg"},
      {96, 8, "hidden_heap_v1"},
      {104, 8, "hidden_default_queue"},
      {112, 8, "hidden_completion_action"},
      {120, 4, "hidden_dynamic_lds_size"},
      {192, 4, "hidden_private_base"},
      {196, 4, "hidden_shared_base"},
      {200, 8, "hidden_queue_ptr"},
  }};
  auto Mismatch = [&](StringRef Field) {
    return postLinkError(Check, (Twine("kernel_contract_") + Field).str());
  };
  if (Metadata.Kernels.size() != 1)
    return postLinkError(Check, "kernel_cardinality");
  const KernelLaunchContract &Kernel = Metadata.Kernels.front();
  if (Kernel.Name != ExactWave64CollectivesV1Entry ||
      Kernel.Symbol != ExactWave64CollectivesV1Descriptor)
    return Mismatch("symbols");
  if (!Kernel.RequiredWorkgroupSize ||
      *Kernel.RequiredWorkgroupSize != Workgroup)
    return Mismatch("reqd_workgroup_size");
  if (Kernel.MaxFlatWorkgroupSize != 64)
    return Mismatch("max_flat_workgroup_size");
  if (Kernel.WavefrontSize != 64)
    return Mismatch("wavefront_size");
  if (Kernel.KernargSegmentSize != 328)
    return Mismatch("kernarg_segment_size");
  if (Kernel.KernargSegmentAlign != 8)
    return Mismatch("kernarg_segment_align");
  if (Kernel.GroupSegmentFixedSize != 0)
    return Mismatch("group_segment_fixed_size");
  if (Kernel.PrivateSegmentFixedSize != 0)
    return Mismatch("private_segment_fixed_size");
  if (!Kernel.SgprSpillCount)
    return Mismatch("sgpr_spill_count_missing");
  if (*Kernel.SgprSpillCount != 0)
    return Mismatch("sgpr_spill_count");
  if (!Kernel.VgprSpillCount)
    return Mismatch("vgpr_spill_count_missing");
  if (*Kernel.VgprSpillCount != 0)
    return Mismatch("vgpr_spill_count");
  if (!Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack_missing");
  if (*Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack");
  if (Kernel.UniformWorkgroupSize && *Kernel.UniformWorkgroupSize)
    return Mismatch("uniform_work_group_size");
  if (!Kernel.Arguments)
    return Mismatch("args_missing");
  const std::vector<KernelArgumentContract> &Arguments = *Kernel.Arguments;
  if (Arguments.size() < 9 + RequiredHidden.size())
    return Mismatch("args_cardinality");

  static constexpr std::array<StringLiteral, 9> Names = {
      "input.data",           "input.len",
      "active_mask",          "reduction_output.data",
      "reduction_output.len", "inclusive_output.data",
      "inclusive_output.len", "exclusive_output.data",
      "exclusive_output.len"};
  static constexpr std::array<uint64_t, 9> Offsets = {0,  8,  16, 24, 32,
                                                      40, 48, 56, 64};
  for (size_t Index = 0; Index != Names.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto ArgumentFailure = [&](StringRef Field, bool Missing = false) {
      return Mismatch((Twine("arg") + Twine(Index) + "_" +
                       (Missing ? "missing_" : "") + Field)
                          .str());
    };
    if (!Argument.Name)
      return ArgumentFailure("name", true);
    if (*Argument.Name != Names[Index])
      return ArgumentFailure("name");
    if (Argument.Offset != Offsets[Index])
      return ArgumentFailure("offset");
    if (Argument.Size != 8)
      return ArgumentFailure("size");
    if (!Argument.TypeName)
      return ArgumentFailure("type_name", true);
    if (Argument.Align && *Argument.Align != 8)
      return ArgumentFailure("align");

    bool IsPointer = Index == 0 || Index == 3 || Index == 5 || Index == 7;
    if (!IsPointer) {
      if (*Argument.TypeName != "ulong")
        return ArgumentFailure("type_name");
      if (Argument.ValueKind != "by_value")
        return ArgumentFailure("value_kind");
      if (Argument.ValueType && *Argument.ValueType != "u64")
        return ArgumentFailure("value_type");
      if (Argument.AddressSpace || Argument.Access || Argument.ActualAccess ||
          Argument.PointeeAlign || (Argument.IsConst && *Argument.IsConst) ||
          (Argument.IsRestrict && *Argument.IsRestrict) ||
          (Argument.IsVolatile && *Argument.IsVolatile) ||
          (Argument.IsPipe && *Argument.IsPipe))
        return ArgumentFailure("pointer_qualifier");
      continue;
    }

    bool IsInput = Index == 0;
    StringRef Access = IsInput ? "read_only" : "write_only";
    if (*Argument.TypeName != "float*")
      return ArgumentFailure("type_name");
    if (Argument.ValueKind != "global_buffer")
      return ArgumentFailure("value_kind");
    if (Argument.ValueType && *Argument.ValueType != "f32")
      return ArgumentFailure("value_type");
    if (!Argument.AddressSpace)
      return ArgumentFailure("address_space", true);
    if (*Argument.AddressSpace != "global")
      return ArgumentFailure("address_space");
    if (!Argument.Access)
      return ArgumentFailure("access", true);
    if (*Argument.Access != Access)
      return ArgumentFailure("access");
    if (Argument.ActualAccess && *Argument.ActualAccess != Access)
      return ArgumentFailure("actual_access");
    if (Argument.PointeeAlign && *Argument.PointeeAlign != 4)
      return ArgumentFailure("pointee_align");
    if (IsInput && !Argument.IsConst)
      return ArgumentFailure("is_const", true);
    if (Argument.IsConst && *Argument.IsConst != IsInput)
      return ArgumentFailure("is_const");
    if (!IsInput && !Argument.IsRestrict)
      return ArgumentFailure("is_restrict", true);
    if (Argument.IsRestrict && *Argument.IsRestrict == IsInput)
      return ArgumentFailure("is_restrict");
    if ((Argument.IsVolatile && *Argument.IsVolatile) ||
        (Argument.IsPipe && *Argument.IsPipe))
      return ArgumentFailure("qualifier");
  }

  constexpr size_t HiddenBaseIndex = 9;
  constexpr uint64_t HiddenBase = 72;
  auto ValidateHidden = [&](const KernelArgumentContract &Argument,
                            const HiddenArgumentShape &Expected) {
    return !Argument.Name && !Argument.TypeName &&
           Argument.Offset == HiddenBase + Expected.Offset &&
           Argument.Size == Expected.Size &&
           Argument.ValueKind == Expected.ValueKind && !Argument.Align &&
           !Argument.ValueType && !Argument.AddressSpace && !Argument.Access &&
           !Argument.ActualAccess && !Argument.PointeeAlign &&
           !Argument.IsConst && !Argument.IsRestrict && !Argument.IsVolatile &&
           !Argument.IsPipe;
  };
  for (size_t Index = 0; Index != RequiredHidden.size(); ++Index)
    if (!ValidateHidden(Arguments[HiddenBaseIndex + Index],
                        RequiredHidden[Index]))
      return Mismatch((Twine("hidden_arg") + Twine(Index)).str());
  for (size_t Index = HiddenBaseIndex + RequiredHidden.size();
       Index != Arguments.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto Expected = llvm::find_if(OptionalHidden, [&](const auto &Shape) {
      return Argument.Offset == HiddenBase + Shape.Offset;
    });
    if (Expected == OptionalHidden.end() ||
        !ValidateHidden(Argument, *Expected))
      return Mismatch(
          (Twine("hidden_arg") + Twine(Index - HiddenBaseIndex)).str());
  }
  return Error::success();
}

Error validateExactFlashAttentionV1Metadata(const MetadataContract &Metadata) {
  constexpr StringLiteral Check = "flash_attention_v1_profile";
  static constexpr std::array<uint64_t, 3> Workgroup = {64, 1, 1};
  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 13> RequiredHidden = {{
      {0, 4, "hidden_block_count_x"},
      {4, 4, "hidden_block_count_y"},
      {8, 4, "hidden_block_count_z"},
      {12, 2, "hidden_group_size_x"},
      {14, 2, "hidden_group_size_y"},
      {16, 2, "hidden_group_size_z"},
      {18, 2, "hidden_remainder_x"},
      {20, 2, "hidden_remainder_y"},
      {22, 2, "hidden_remainder_z"},
      {40, 8, "hidden_global_offset_x"},
      {48, 8, "hidden_global_offset_y"},
      {56, 8, "hidden_global_offset_z"},
      {64, 2, "hidden_grid_dims"},
  }};
  static constexpr std::array<HiddenArgumentShape, 10> OptionalHidden = {{
      {72, 8, "hidden_printf_buffer"},
      {80, 8, "hidden_hostcall_buffer"},
      {88, 8, "hidden_multigrid_sync_arg"},
      {96, 8, "hidden_heap_v1"},
      {104, 8, "hidden_default_queue"},
      {112, 8, "hidden_completion_action"},
      {120, 4, "hidden_dynamic_lds_size"},
      {192, 4, "hidden_private_base"},
      {196, 4, "hidden_shared_base"},
      {200, 8, "hidden_queue_ptr"},
  }};
  auto Mismatch = [&](StringRef Field) {
    return postLinkError(Check, (Twine("kernel_contract_") + Field).str());
  };
  if (Metadata.Kernels.size() != 1)
    return postLinkError(Check, "kernel_cardinality");
  const KernelLaunchContract &Kernel = Metadata.Kernels.front();
  if (Kernel.Name != ExactFlashAttentionV1Entry ||
      Kernel.Symbol != ExactFlashAttentionV1Descriptor)
    return Mismatch("symbols");
  if (!Kernel.RequiredWorkgroupSize ||
      *Kernel.RequiredWorkgroupSize != Workgroup)
    return Mismatch("reqd_workgroup_size");
  if (Kernel.MaxFlatWorkgroupSize != 64)
    return Mismatch("max_flat_workgroup_size");
  if (Kernel.WavefrontSize != 64)
    return Mismatch("wavefront_size");
  if (Kernel.KernargSegmentSize != 320)
    return Mismatch("kernarg_segment_size");
  if (Kernel.KernargSegmentAlign != 8)
    return Mismatch("kernarg_segment_align");
  if (Kernel.GroupSegmentFixedSize != 0)
    return Mismatch("group_segment_fixed_size");
  if (Kernel.PrivateSegmentFixedSize != 0)
    return Mismatch("private_segment_fixed_size");
  if (!Kernel.SgprSpillCount || *Kernel.SgprSpillCount != 0)
    return Mismatch("sgpr_spill_count");
  if (!Kernel.VgprSpillCount || *Kernel.VgprSpillCount != 0)
    return Mismatch("vgpr_spill_count");
  if (!Kernel.UsesDynamicStack || *Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack");
  if (!Kernel.Arguments)
    return Mismatch("args_missing");
  const std::vector<KernelArgumentContract> &Arguments = *Kernel.Arguments;
  if (Arguments.size() < 8 + RequiredHidden.size())
    return Mismatch("args_cardinality");

  static constexpr std::array<StringLiteral, 8> Names = {
      "q.data", "q.len", "k.data",      "k.len",
      "v.data", "v.len", "output.data", "output.len"};
  for (size_t Index = 0; Index != Names.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto Failure = [&](StringRef Field) {
      return Mismatch((Twine("arg") + Twine(Index) + "_" + Field).str());
    };
    if (!Argument.Name || *Argument.Name != Names[Index])
      return Failure("name");
    if (Argument.Offset != Index * 8 || Argument.Size != 8)
      return Failure("layout");
    if (!Argument.TypeName)
      return Failure("type_name");
    const bool IsPointer = Index % 2 == 0;
    if (!IsPointer) {
      if (*Argument.TypeName != "ulong" || Argument.ValueKind != "by_value" ||
          (Argument.ValueType && *Argument.ValueType != "u64") ||
          Argument.AddressSpace || Argument.Access || Argument.ActualAccess ||
          Argument.PointeeAlign || (Argument.IsConst && *Argument.IsConst) ||
          (Argument.IsRestrict && *Argument.IsRestrict) ||
          (Argument.IsVolatile && *Argument.IsVolatile) ||
          (Argument.IsPipe && *Argument.IsPipe))
        return Failure("length_contract");
      continue;
    }
    const bool IsOutput = Index == 6;
    StringRef Access = IsOutput ? "read_write" : "read_only";
    if (*Argument.TypeName != "float*" ||
        Argument.ValueKind != "global_buffer" ||
        (Argument.ValueType && *Argument.ValueType != "f32") ||
        !Argument.AddressSpace || *Argument.AddressSpace != "global" ||
        !Argument.Access || *Argument.Access != Access ||
        (IsOutput
             ? (!Argument.ActualAccess ||
                *Argument.ActualAccess != "write_only")
             : Argument.ActualAccess && *Argument.ActualAccess != Access) ||
        (Argument.PointeeAlign && *Argument.PointeeAlign != 4) ||
        (IsOutput ? (!Argument.IsRestrict || !*Argument.IsRestrict)
                  : (Argument.IsRestrict && *Argument.IsRestrict)) ||
        (IsOutput ? (Argument.IsConst && *Argument.IsConst)
                  : (!Argument.IsConst || !*Argument.IsConst)) ||
        (Argument.IsVolatile && *Argument.IsVolatile) ||
        (Argument.IsPipe && *Argument.IsPipe))
      return Failure("pointer_contract");
  }

  constexpr size_t HiddenBaseIndex = 8;
  constexpr uint64_t HiddenBase = 64;
  auto ValidateHidden = [&](const KernelArgumentContract &Argument,
                            const HiddenArgumentShape &Expected) {
    return !Argument.Name && !Argument.TypeName &&
           Argument.Offset == HiddenBase + Expected.Offset &&
           Argument.Size == Expected.Size &&
           Argument.ValueKind == Expected.ValueKind && !Argument.Align &&
           !Argument.ValueType && !Argument.AddressSpace && !Argument.Access &&
           !Argument.ActualAccess && !Argument.PointeeAlign &&
           !Argument.IsConst && !Argument.IsRestrict && !Argument.IsVolatile &&
           !Argument.IsPipe;
  };
  for (size_t Index = 0; Index != RequiredHidden.size(); ++Index)
    if (!ValidateHidden(Arguments[HiddenBaseIndex + Index],
                        RequiredHidden[Index]))
      return Mismatch((Twine("hidden_arg") + Twine(Index)).str());
  for (size_t Index = HiddenBaseIndex + RequiredHidden.size();
       Index != Arguments.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto Expected = llvm::find_if(OptionalHidden, [&](const auto &Shape) {
      return Argument.Offset == HiddenBase + Shape.Offset;
    });
    if (Expected == OptionalHidden.end() ||
        !ValidateHidden(Argument, *Expected))
      return Mismatch(
          (Twine("hidden_arg") + Twine(Index - HiddenBaseIndex)).str());
  }
  return Error::success();
}

Error validateExactWorkgroupSyncMetadata(
    const MetadataContract &Metadata,
    const ExactWorkgroupSyncProfile &Profile) {
  static constexpr std::array<uint64_t, 3> Workgroup = {64, 1, 1};
  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 13> RequiredHidden = {{
      {0, 4, "hidden_block_count_x"},
      {4, 4, "hidden_block_count_y"},
      {8, 4, "hidden_block_count_z"},
      {12, 2, "hidden_group_size_x"},
      {14, 2, "hidden_group_size_y"},
      {16, 2, "hidden_group_size_z"},
      {18, 2, "hidden_remainder_x"},
      {20, 2, "hidden_remainder_y"},
      {22, 2, "hidden_remainder_z"},
      {40, 8, "hidden_global_offset_x"},
      {48, 8, "hidden_global_offset_y"},
      {56, 8, "hidden_global_offset_z"},
      {64, 2, "hidden_grid_dims"},
  }};
  static constexpr std::array<HiddenArgumentShape, 10> OptionalHidden = {{
      {72, 8, "hidden_printf_buffer"},
      {80, 8, "hidden_hostcall_buffer"},
      {88, 8, "hidden_multigrid_sync_arg"},
      {96, 8, "hidden_heap_v1"},
      {104, 8, "hidden_default_queue"},
      {112, 8, "hidden_completion_action"},
      {120, 4, "hidden_dynamic_lds_size"},
      {192, 4, "hidden_private_base"},
      {196, 4, "hidden_shared_base"},
      {200, 8, "hidden_queue_ptr"},
  }};
  auto Mismatch = [&](StringRef Field) {
    return postLinkError(Profile.Check,
                         (Twine("kernel_contract_") + Field).str());
  };
  if (Metadata.Kernels.size() != 1)
    return postLinkError(Profile.Check, "kernel_cardinality");
  const KernelLaunchContract &Kernel = Metadata.Kernels.front();
  if (Kernel.Name != Profile.Entry || Kernel.Symbol != Profile.Descriptor)
    return Mismatch("symbols");
  if (!Kernel.RequiredWorkgroupSize ||
      *Kernel.RequiredWorkgroupSize != Workgroup)
    return Mismatch("reqd_workgroup_size");
  if (Kernel.MaxFlatWorkgroupSize != 64)
    return Mismatch("max_flat_workgroup_size");
  if (Kernel.WavefrontSize != 64)
    return Mismatch("wavefront_size");
  if (Kernel.KernargSegmentSize != 296)
    return Mismatch("kernarg_segment_size");
  if (Kernel.KernargSegmentAlign != 8)
    return Mismatch("kernarg_segment_align");
  if (Kernel.GroupSegmentFixedSize != 0)
    return Mismatch("group_segment_fixed_size");
  if (Kernel.PrivateSegmentFixedSize != 0)
    return Mismatch("private_segment_fixed_size");
  if (!Kernel.SgprSpillCount || *Kernel.SgprSpillCount != 0)
    return Mismatch("sgpr_spill_count");
  if (!Kernel.VgprSpillCount || *Kernel.VgprSpillCount != 0)
    return Mismatch("vgpr_spill_count");
  if (!Kernel.UsesDynamicStack || *Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack");
  if (Kernel.UniformWorkgroupSize && *Kernel.UniformWorkgroupSize)
    return Mismatch("uniform_work_group_size");
  if (!Kernel.Arguments)
    return Mismatch("args_missing");
  const std::vector<KernelArgumentContract> &Arguments = *Kernel.Arguments;
  if (Arguments.size() < 5 + RequiredHidden.size())
    return Mismatch("args_cardinality");

  auto ValidatePointer = [&](size_t Index, StringRef Name, uint64_t Offset,
                             StringRef TypeName, StringRef ValueType,
                             StringRef Access, bool Restrict) -> Error {
    const KernelArgumentContract &Argument = Arguments[Index];
    if (!Argument.Name || *Argument.Name != Name || Argument.Offset != Offset ||
        Argument.Size != 8 || !Argument.TypeName ||
        *Argument.TypeName != TypeName ||
        (Argument.Align && *Argument.Align != 8) ||
        Argument.ValueKind != "global_buffer" ||
        (Argument.ValueType && *Argument.ValueType != ValueType) ||
        !Argument.AddressSpace || *Argument.AddressSpace != "global" ||
        !Argument.Access || *Argument.Access != Access ||
        (Argument.ActualAccess &&
         (Access == "read_write" ? *Argument.ActualAccess != "read_write" &&
                                       *Argument.ActualAccess != "write_only"
                                 : *Argument.ActualAccess != Access)) ||
        (Argument.PointeeAlign && *Argument.PointeeAlign != 4) ||
        (Access == "read_only" ? !Argument.IsConst || !*Argument.IsConst
                               : Argument.IsConst && *Argument.IsConst) ||
        (Restrict ? !Argument.IsRestrict || !*Argument.IsRestrict
                  : Argument.IsRestrict && *Argument.IsRestrict) ||
        (Argument.IsVolatile && *Argument.IsVolatile) ||
        (Argument.IsPipe && *Argument.IsPipe))
      return Mismatch((Twine("arg") + Twine(Index) + "_pointer").str());
    return Error::success();
  };
  auto ValidateScalar = [&](size_t Index, StringRef Name, uint64_t Offset,
                            uint64_t Size, StringRef TypeName,
                            StringRef ValueType) -> Error {
    const KernelArgumentContract &Argument = Arguments[Index];
    if (!Argument.Name || *Argument.Name != Name || Argument.Offset != Offset ||
        Argument.Size != Size || !Argument.TypeName ||
        *Argument.TypeName != TypeName ||
        (Argument.Align && *Argument.Align != Size) ||
        Argument.ValueKind != "by_value" ||
        (Argument.ValueType && *Argument.ValueType != ValueType) ||
        Argument.AddressSpace || Argument.Access || Argument.ActualAccess ||
        Argument.PointeeAlign || (Argument.IsConst && *Argument.IsConst) ||
        (Argument.IsRestrict && *Argument.IsRestrict) ||
        (Argument.IsVolatile && *Argument.IsVolatile) ||
        (Argument.IsPipe && *Argument.IsPipe))
      return Mismatch((Twine("arg") + Twine(Index) + "_scalar").str());
    return Error::success();
  };

  if (Error E = ValidatePointer(
          0, "values.data", 0,
          Profile.Kind == ExactWorkgroupSyncKind::LdsReduction ? "int*"
                                                               : "uint*",
          Profile.Kind == ExactWorkgroupSyncKind::LdsReduction ? "i32" : "u32",
          "read_only", false))
    return E;
  if (Error E = ValidateScalar(1, "values.len", 8, 8, "ulong", "u64"))
    return E;
  if (Profile.Kind == ExactWorkgroupSyncKind::LdsReduction) {
    if (Error E = ValidateScalar(2, "epoch", 16, 4, "uint", "u32"))
      return E;
    if (Error E = ValidatePointer(3, "output.data", 24, "int*", "i32",
                                  "read_write", true))
      return E;
    if (Error E = ValidateScalar(4, "output.len", 32, 8, "ulong", "u64"))
      return E;
  } else {
    if (Error E = ValidatePointer(2, "eligible.data", 16, "uint*", "u32",
                                  "read_only", false))
      return E;
    if (Error E = ValidateScalar(3, "eligible.len", 24, 8, "ulong", "u64"))
      return E;
    if (Error E = ValidateScalar(4, "target.address", 32, 8, "ulong", "u64"))
      return E;
  }

  constexpr size_t HiddenBaseIndex = 5;
  constexpr uint64_t HiddenBase = 40;
  auto ValidateHidden = [&](const KernelArgumentContract &Argument,
                            const HiddenArgumentShape &Expected) {
    return !Argument.Name && !Argument.TypeName &&
           Argument.Offset == HiddenBase + Expected.Offset &&
           Argument.Size == Expected.Size &&
           Argument.ValueKind == Expected.ValueKind && !Argument.Align &&
           !Argument.ValueType && !Argument.AddressSpace && !Argument.Access &&
           !Argument.ActualAccess && !Argument.PointeeAlign &&
           !Argument.IsConst && !Argument.IsRestrict && !Argument.IsVolatile &&
           !Argument.IsPipe;
  };
  for (size_t Index = 0; Index != RequiredHidden.size(); ++Index)
    if (!ValidateHidden(Arguments[HiddenBaseIndex + Index],
                        RequiredHidden[Index]))
      return Mismatch((Twine("hidden_arg") + Twine(Index)).str());

  size_t DynamicLdsArguments = 0;
  for (size_t Index = HiddenBaseIndex + RequiredHidden.size();
       Index != Arguments.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto Expected = llvm::find_if(OptionalHidden, [&](const auto &Shape) {
      return Argument.Offset == HiddenBase + Shape.Offset;
    });
    if (Expected == OptionalHidden.end() ||
        !ValidateHidden(Argument, *Expected))
      return Mismatch(
          (Twine("hidden_arg") + Twine(Index - HiddenBaseIndex)).str());
    DynamicLdsArguments += Expected->ValueKind == "hidden_dynamic_lds_size";
  }
  if (Profile.Kind == ExactWorkgroupSyncKind::LdsReduction
          ? DynamicLdsArguments != 1
          : DynamicLdsArguments != 0)
    return Mismatch("hidden_dynamic_lds_size");
  return Error::success();
}

Error validateExactMoeTop2V1Metadata(const MetadataContract &Metadata) {
  static constexpr std::array<uint64_t, 3> Workgroup = {64, 1, 1};
  struct HiddenArgumentShape {
    uint64_t Offset;
    uint64_t Size;
    StringLiteral ValueKind;
  };
  static constexpr std::array<HiddenArgumentShape, 13> RequiredHidden = {{
      {0, 4, "hidden_block_count_x"},
      {4, 4, "hidden_block_count_y"},
      {8, 4, "hidden_block_count_z"},
      {12, 2, "hidden_group_size_x"},
      {14, 2, "hidden_group_size_y"},
      {16, 2, "hidden_group_size_z"},
      {18, 2, "hidden_remainder_x"},
      {20, 2, "hidden_remainder_y"},
      {22, 2, "hidden_remainder_z"},
      {40, 8, "hidden_global_offset_x"},
      {48, 8, "hidden_global_offset_y"},
      {56, 8, "hidden_global_offset_z"},
      {64, 2, "hidden_grid_dims"},
  }};
  static constexpr std::array<HiddenArgumentShape, 10> OptionalHidden = {{
      {72, 8, "hidden_printf_buffer"},
      {80, 8, "hidden_hostcall_buffer"},
      {88, 8, "hidden_multigrid_sync_arg"},
      {96, 8, "hidden_heap_v1"},
      {104, 8, "hidden_default_queue"},
      {112, 8, "hidden_completion_action"},
      {120, 4, "hidden_dynamic_lds_size"},
      {192, 4, "hidden_private_base"},
      {196, 4, "hidden_shared_base"},
      {200, 8, "hidden_queue_ptr"},
  }};
  auto Mismatch = [](StringRef Field) {
    return postLinkError(ExactMoeTop2V1Check,
                         (Twine("kernel_contract_") + Field).str());
  };
  if (Metadata.Kernels.size() != 1)
    return postLinkError(ExactMoeTop2V1Check, "kernel_cardinality");
  const KernelLaunchContract &Kernel = Metadata.Kernels.front();
  if (Kernel.Name != ExactMoeTop2V1Entry ||
      Kernel.Symbol != ExactMoeTop2V1Descriptor)
    return Mismatch("symbols");
  if (!Kernel.RequiredWorkgroupSize ||
      *Kernel.RequiredWorkgroupSize != Workgroup)
    return Mismatch("reqd_workgroup_size");
  if (Kernel.MaxFlatWorkgroupSize != 64)
    return Mismatch("max_flat_workgroup_size");
  if (Kernel.WavefrontSize != 64)
    return Mismatch("wavefront_size");
  if (Kernel.KernargSegmentSize != 384)
    return Mismatch("kernarg_segment_size");
  if (Kernel.KernargSegmentAlign != 8)
    return Mismatch("kernarg_segment_align");
  if (Kernel.GroupSegmentFixedSize != 0)
    return Mismatch("group_segment_fixed_size");
  if (Kernel.PrivateSegmentFixedSize != 0)
    return Mismatch("private_segment_fixed_size");
  if (!Kernel.SgprSpillCount || *Kernel.SgprSpillCount != 0)
    return Mismatch("sgpr_spill_count");
  if (!Kernel.VgprSpillCount || *Kernel.VgprSpillCount != 0)
    return Mismatch("vgpr_spill_count");
  if (!Kernel.UsesDynamicStack || *Kernel.UsesDynamicStack)
    return Mismatch("uses_dynamic_stack");
  if (Kernel.UniformWorkgroupSize && *Kernel.UniformWorkgroupSize)
    return Mismatch("uniform_work_group_size");
  if (!Kernel.Arguments)
    return Mismatch("args_missing");
  const std::vector<KernelArgumentContract> &Arguments = *Kernel.Arguments;
  if (Arguments.size() < 16 + RequiredHidden.size())
    return Mismatch("args_cardinality");

  static constexpr std::array<StringLiteral, 8> PointerNames = {
      "logits.data",  "top2.data",  "requested.data",   "admitted.data",
      "offsets.data", "slots.data", "permutation.data", "inverse.data"};
  static constexpr std::array<StringLiteral, 8> LengthNames = {
      "logits.len",  "top2.len",  "requested.len",   "admitted.len",
      "offsets.len", "slots.len", "permutation.len", "inverse.len"};
  for (size_t Role = 0; Role != PointerNames.size(); ++Role) {
    size_t PointerIndex = Role * 2;
    const KernelArgumentContract &Pointer = Arguments[PointerIndex];
    bool IsInput = Role == 0;
    StringRef TypeName = IsInput ? "float*" : "uint*";
    StringRef ValueType = IsInput ? "f32" : "u32";
    StringRef Access = IsInput ? "read_only" : "read_write";
    if (!Pointer.Name || *Pointer.Name != PointerNames[Role] ||
        Pointer.Offset != PointerIndex * 8 || Pointer.Size != 8 ||
        !Pointer.TypeName || *Pointer.TypeName != TypeName ||
        (Pointer.Align && *Pointer.Align != 8) ||
        Pointer.ValueKind != "global_buffer" ||
        (Pointer.ValueType && *Pointer.ValueType != ValueType) ||
        !Pointer.AddressSpace || *Pointer.AddressSpace != "global" ||
        !Pointer.Access || *Pointer.Access != Access ||
        (Pointer.ActualAccess &&
         (IsInput ? *Pointer.ActualAccess != "read_only"
                  : *Pointer.ActualAccess != "read_write" &&
                        *Pointer.ActualAccess != "write_only")) ||
        (Pointer.PointeeAlign && *Pointer.PointeeAlign != 4) ||
        (IsInput ? !Pointer.IsConst || !*Pointer.IsConst
                 : Pointer.IsConst && *Pointer.IsConst) ||
        (IsInput ? Pointer.IsRestrict && *Pointer.IsRestrict
                 : !Pointer.IsRestrict || !*Pointer.IsRestrict) ||
        (Pointer.IsVolatile && *Pointer.IsVolatile) ||
        (Pointer.IsPipe && *Pointer.IsPipe))
      return Mismatch((Twine("arg") + Twine(PointerIndex) + "_pointer").str());

    const KernelArgumentContract &Length = Arguments[PointerIndex + 1];
    if (!Length.Name || *Length.Name != LengthNames[Role] ||
        Length.Offset != (PointerIndex + 1) * 8 || Length.Size != 8 ||
        !Length.TypeName || *Length.TypeName != "ulong" ||
        (Length.Align && *Length.Align != 8) ||
        Length.ValueKind != "by_value" ||
        (Length.ValueType && *Length.ValueType != "u64") ||
        Length.AddressSpace || Length.Access || Length.ActualAccess ||
        Length.PointeeAlign || (Length.IsConst && *Length.IsConst) ||
        (Length.IsRestrict && *Length.IsRestrict) ||
        (Length.IsVolatile && *Length.IsVolatile) ||
        (Length.IsPipe && *Length.IsPipe))
      return Mismatch(
          (Twine("arg") + Twine(PointerIndex + 1) + "_length").str());
  }

  constexpr size_t HiddenBaseIndex = 16;
  constexpr uint64_t HiddenBase = 128;
  auto ValidateHidden = [&](const KernelArgumentContract &Argument,
                            const HiddenArgumentShape &Expected) {
    return !Argument.Name && !Argument.TypeName &&
           Argument.Offset == HiddenBase + Expected.Offset &&
           Argument.Size == Expected.Size &&
           Argument.ValueKind == Expected.ValueKind && !Argument.Align &&
           !Argument.ValueType && !Argument.AddressSpace && !Argument.Access &&
           !Argument.ActualAccess && !Argument.PointeeAlign &&
           !Argument.IsConst && !Argument.IsRestrict && !Argument.IsVolatile &&
           !Argument.IsPipe;
  };
  for (size_t Index = 0; Index != RequiredHidden.size(); ++Index)
    if (!ValidateHidden(Arguments[HiddenBaseIndex + Index],
                        RequiredHidden[Index]))
      return Mismatch((Twine("hidden_arg") + Twine(Index)).str());
  for (size_t Index = HiddenBaseIndex + RequiredHidden.size();
       Index != Arguments.size(); ++Index) {
    const KernelArgumentContract &Argument = Arguments[Index];
    auto Expected = llvm::find_if(OptionalHidden, [&](const auto &Shape) {
      return Argument.Offset == HiddenBase + Shape.Offset;
    });
    if (Expected == OptionalHidden.end() ||
        Expected->ValueKind == "hidden_dynamic_lds_size" ||
        !ValidateHidden(Argument, *Expected))
      return Mismatch(
          (Twine("hidden_arg") + Twine(Index - HiddenBaseIndex)).str());
  }
  return Error::success();
}

Expected<std::vector<std::string>> inspectOutput(ArrayRef<uint8_t> Bytes,
                                                 const ElfContract &Expected,
                                                 const Request &RequestValue) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<output>"));
  if (!ObjectOrError)
    return postLinkError("elf", errorToDiagnostic(ObjectOrError.takeError()));
  auto *Elf = dyn_cast<ELFObjectFileBase>(ObjectOrError->get());
  if (!Elf || Elf->getEMachine() != ELF::EM_AMDGPU ||
      Elf->getEType() != ELF::ET_DYN)
    return postLinkError("elf", "not_amdgpu_shared");
  if (Bytes.size() < ELF::EI_NIDENT ||
      Bytes[ELF::EI_CLASS] != ELF::ELFCLASS64 ||
      Bytes[ELF::EI_DATA] != ELF::ELFDATA2LSB ||
      Bytes[ELF::EI_VERSION] != ELF::EV_CURRENT ||
      Elf->getBytesInAddress() != 8 || !Elf->isLittleEndian())
    return postLinkError("elf", "invalid_envelope");

  TargetParts RequestedTarget = parseTarget(RequestValue.Target);
  uint32_t PlatformFlags = Elf->getPlatformFlags();
  if (RequestedTarget.Cpu == "gfx942") {
    uint32_t ExpectedFlags = expectedGfx942Flags(RequestedTarget);
    if (PlatformFlags != ExpectedFlags)
      return postLinkError("target", (Twine("e_flags expected=") +
                                      hexadecimal(ExpectedFlags) +
                                      " actual=" + hexadecimal(PlatformFlags))
                                         .str());
  }
  if (Elf->getPlatformFlags() != Expected.Flags ||
      Bytes[ELF::EI_OSABI] != Expected.OsAbi ||
      Elf->getEIdentABIVersion() != Expected.AbiVersion ||
      Elf->getBytesInAddress() != Expected.AddressBytes ||
      Elf->isLittleEndian() != Expected.LittleEndian)
    return postLinkError("target", "target_or_code_object_mismatch");

  std::set<std::string> Defined;
  std::set<std::string> StaticPublicDefinitions;
  std::set<std::string> UndefinedSymbols;
  auto InspectSymbol = [&](SymbolRef Symbol, bool Dynamic) -> Error {
    auto FlagsOrError = Symbol.getFlags();
    if (!FlagsOrError)
      return FlagsOrError.takeError();
    uint32_t Flags = *FlagsOrError;
    auto NameOrError = Symbol.getName();
    if (!NameOrError)
      return NameOrError.takeError();
    if (NameOrError->empty())
      return Error::success();
    std::string Name = NameOrError->str();
    if ((Flags & SymbolRef::SF_Undefined) != 0) {
      UndefinedSymbols.insert(std::move(Name));
      return Error::success();
    }
    if ((Flags & SymbolRef::SF_FormatSpecific) != 0)
      return Error::success();
    if ((Flags & (SymbolRef::SF_Global | SymbolRef::SF_Weak)) == 0)
      return Error::success();
    if (!Dynamic)
      StaticPublicDefinitions.insert(Name);
    if (Dynamic && (Flags & SymbolRef::SF_Hidden) == 0)
      Defined.insert(std::move(Name));
    return Error::success();
  };
  for (SymbolRef Symbol : Elf->symbols())
    if (Error E = InspectSymbol(Symbol, false))
      return postLinkError("symbols", errorToDiagnostic(std::move(E)));
  for (SymbolRef Symbol : Elf->getDynamicSymbolIterators()) {
    if (Error E = InspectSymbol(Symbol, true))
      return postLinkError("symbols", errorToDiagnostic(std::move(E)));
  }
  if (!UndefinedSymbols.empty())
    return pipelineError(
        Twine("post_link.check=unresolved status=failed symbols=") +
        diagnosticList(UndefinedSymbols));

  std::set<std::string> ExpectedSymbols(
      RequestValue.ExpectedDefinedSymbols.begin(),
      RequestValue.ExpectedDefinedSymbols.end());
  if (Defined != ExpectedSymbols)
    return pipelineError(
        Twine("post_link.check=exports status=failed expected=") +
        diagnosticList(ExpectedSymbols) + " actual=" + diagnosticList(Defined));

  auto *ConcreteElf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!ConcreteElf)
    return postLinkError("elf", "not_elf64le");
  auto Profile = selectPostLinkProfile(RequestValue, ExpectedSymbols);
  if (!Profile)
    return postLinkError("profile", errorToDiagnostic(Profile.takeError()));
  if (*Profile == PostLinkProfile::ExactRowSoftmaxV1) {
    if (Error E = validateExactRowSoftmaxV1ElfClosure(*ConcreteElf))
      return E;
    if (Error E = validateExactRowSoftmaxV1DescriptorBinding(*ConcreteElf,
                                                             RequestValue))
      return E;
    if (StaticPublicDefinitions != ExpectedSymbols)
      return pipelineError(
          Twine("post_link.check=") + ExactRowSoftmaxV1Check +
          " status=failed reason=static_symbol_closure expected=" +
          diagnosticList(ExpectedSymbols) +
          " actual=" + diagnosticList(StaticPublicDefinitions));
  }
  if (*Profile == PostLinkProfile::ExactLdsGemmSlice1 ||
      *Profile == PostLinkProfile::ExactWave64CollectivesV1) {
    const bool IsWave64 = *Profile == PostLinkProfile::ExactWave64CollectivesV1;
    Error Closure =
        IsWave64 ? validateExactWave64CollectivesV1ElfClosure(*ConcreteElf)
                 : validateExactLdsGemmSlice1ElfClosure(*ConcreteElf);
    if (Closure)
      return Closure;
    if (IsWave64) {
      if (Error E = validateExactWave64NoMachineCalls(*ConcreteElf))
        return E;
      if (Error E =
              validateExactWave64DescriptorBinding(*ConcreteElf, RequestValue))
        return E;
    }
    StringRef Check =
        IsWave64 ? "wave64_collectives_v1_profile" : "lds_gemm_slice1_profile";
    if (StaticPublicDefinitions != ExpectedSymbols)
      return pipelineError(
          Twine("post_link.check=") + Check + " status=failed " +
          "reason=static_symbol_closure expected=" +
          diagnosticList(ExpectedSymbols) +
          " actual=" + diagnosticList(StaticPublicDefinitions));
  }
  if (const ExactWorkgroupSyncProfile *WorkgroupProfile =
          postLinkWorkgroupSyncProfile(*Profile)) {
    if (Error E = validateExactWorkgroupSyncElfClosure(*ConcreteElf,
                                                       *WorkgroupProfile))
      return E;
    if (Error E =
            validateExactWorkgroupSyncMachine(*ConcreteElf, *WorkgroupProfile))
      return E;
    if (Error E = validateExactWorkgroupSyncDescriptorBinding(
            *ConcreteElf, RequestValue, *WorkgroupProfile))
      return E;
    if (StaticPublicDefinitions != ExpectedSymbols)
      return pipelineError(
          Twine("post_link.check=") + WorkgroupProfile->Check +
          " status=failed reason=static_symbol_closure expected=" +
          diagnosticList(ExpectedSymbols) +
          " actual=" + diagnosticList(StaticPublicDefinitions));
  }
  if (*Profile == PostLinkProfile::ExactFlashAttentionV1) {
    if (Error E = validateExactFlashAttentionV1ElfClosure(*ConcreteElf))
      return E;
    if (Error E = validateExactFlashAttentionMachine(*ConcreteElf))
      return E;
    if (Error E = validateExactFlashAttentionDescriptorBinding(*ConcreteElf,
                                                               RequestValue))
      return E;
    if (StaticPublicDefinitions != ExpectedSymbols)
      return pipelineError(
          Twine("post_link.check=flash_attention_v1_profile status=failed "
                "reason=static_symbol_closure expected=") +
          diagnosticList(ExpectedSymbols) +
          " actual=" + diagnosticList(StaticPublicDefinitions));
  }
  if (*Profile == PostLinkProfile::ExactMoeTop2V1) {
    if (Error E = validateExactMoeTop2V1ElfClosure(*ConcreteElf))
      return E;
    if (Error E = validateExactMoeTop2V1Machine(*ConcreteElf))
      return E;
    if (Error E =
            validateExactMoeTop2V1DescriptorBinding(*ConcreteElf, RequestValue))
      return E;
    if (StaticPublicDefinitions != ExpectedSymbols)
      return pipelineError(
          Twine("post_link.check=") + ExactMoeTop2V1Check +
          " status=failed reason=static_symbol_closure expected=" +
          diagnosticList(ExpectedSymbols) +
          " actual=" + diagnosticList(StaticPublicDefinitions));
  }
  MetadataValidationPolicy MetadataPolicy = MetadataValidationPolicy::Generic;
  if (*Profile == PostLinkProfile::ExactRowSoftmaxV1)
    MetadataPolicy = MetadataValidationPolicy::ExactRowSoftmaxV1;
  else if (*Profile == PostLinkProfile::ExactLdsGemmSlice1)
    MetadataPolicy = MetadataValidationPolicy::ExactLdsGemmSlice1;
  else if (*Profile == PostLinkProfile::ExactWave64CollectivesV1)
    MetadataPolicy = MetadataValidationPolicy::ExactWave64CollectivesV1;
  else if (*Profile == PostLinkProfile::ExactFlashAttentionV1)
    MetadataPolicy = MetadataValidationPolicy::ExactFlashAttentionV1;
  else if (*Profile == PostLinkProfile::ExactWorkgroupLdsReductionV1)
    MetadataPolicy = MetadataValidationPolicy::ExactWorkgroupLdsReductionV1;
  else if (*Profile == PostLinkProfile::ExactScopedAtomicV1)
    MetadataPolicy = MetadataValidationPolicy::ExactScopedAtomicV1;
  else if (*Profile == PostLinkProfile::ExactMoeTop2V1)
    MetadataPolicy = MetadataValidationPolicy::ExactMoeTop2V1;
  auto Metadata = inspectMetadata(*ConcreteElf, MetadataPolicy);
  if (!Metadata)
    return postLinkError("metadata", errorToDiagnostic(Metadata.takeError()));
  if (Metadata->Present) {
    std::string ExpectedMetadataTarget =
        (Twine(AmdGpuTriple) + "--" + RequestValue.Target).str();
    if (!Metadata->Target || *Metadata->Target != ExpectedMetadataTarget)
      return postLinkError(
          "metadata_target",
          (Twine("expected=") + ExpectedMetadataTarget +
           " actual=" + (Metadata->Target ? *Metadata->Target : "absent"))
              .str());
  }

  std::set<std::string> ExpectedDescriptors;
  for (const std::string &Name : ExpectedSymbols)
    if (StringRef(Name).ends_with(".kd"))
      ExpectedDescriptors.insert(Name);
  std::set<std::string> MetadataDescriptors;
  for (const KernelLaunchContract &Kernel : Metadata->Kernels) {
    if (!Defined.contains(Kernel.Name) || !Defined.contains(Kernel.Symbol))
      return postLinkError("metadata_bindings",
                           "kernel_or_descriptor_not_exported");
    MetadataDescriptors.insert(Kernel.Symbol);
  }
  if (!ExpectedDescriptors.empty() && !Metadata->Present)
    return postLinkError("metadata", "missing_for_descriptors");
  if (MetadataDescriptors != ExpectedDescriptors)
    return pipelineError(
        Twine("post_link.check=descriptors status=failed expected=") +
        diagnosticList(ExpectedDescriptors) +
        " actual=" + diagnosticList(MetadataDescriptors));

  if (RequestedTarget.Cpu == "gfx942" && !ExpectedDescriptors.empty() &&
      *Profile == PostLinkProfile::LegacyGfx942G1) {
    static constexpr std::array<uint64_t, 3> G1Workgroup = {256, 1, 1};
    for (const KernelLaunchContract &Kernel : Metadata->Kernels) {
      if (!Kernel.RequiredWorkgroupSize ||
          *Kernel.RequiredWorkgroupSize != G1Workgroup)
        return pipelineError(
            Twine("post_link.check=g1_profile status=failed kernel=") +
            diagnosticAtom(Kernel.Name) +
            " field=reqd_workgroup_size expected=[256,1,1]");
      if (Kernel.MaxFlatWorkgroupSize != 256)
        return pipelineError(
            Twine("post_link.check=g1_profile status=failed kernel=") +
            diagnosticAtom(Kernel.Name) +
            " field=max_flat_workgroup_size expected=256 actual=" +
            Twine(Kernel.MaxFlatWorkgroupSize));
      if (Kernel.WavefrontSize != 64)
        return pipelineError(
            Twine("post_link.check=g1_profile status=failed kernel=") +
            diagnosticAtom(Kernel.Name) +
            " field=wavefront_size expected=64 actual=" +
            Twine(Kernel.WavefrontSize));
    }
  }
  if (*Profile == PostLinkProfile::ExactLdsGemmSlice1)
    if (Error E = validateExactLdsGemmSlice1Metadata(*Metadata))
      return E;
  if (*Profile == PostLinkProfile::ExactRowSoftmaxV1)
    if (Error E = validateExactRowSoftmaxV1Metadata(*Metadata))
      return E;
  if (*Profile == PostLinkProfile::ExactWave64CollectivesV1)
    if (Error E = validateExactWave64CollectivesV1Metadata(*Metadata))
      return E;
  if (*Profile == PostLinkProfile::ExactFlashAttentionV1)
    if (Error E = validateExactFlashAttentionV1Metadata(*Metadata))
      return E;
  if (const ExactWorkgroupSyncProfile *WorkgroupProfile =
          postLinkWorkgroupSyncProfile(*Profile))
    if (Error E =
            validateExactWorkgroupSyncMetadata(*Metadata, *WorkgroupProfile))
      return E;
  if (*Profile == PostLinkProfile::ExactMoeTop2V1)
    if (Error E = validateExactMoeTop2V1Metadata(*Metadata))
      return E;

  std::vector<std::string> Diagnostics;
  Diagnostics.push_back(
      (Twine("post_link.check=target status=ok arch=") +
       diagnosticAtom(RequestedTarget.Cpu) + " code_object_version=" +
       Twine(static_cast<unsigned>(RequestValue.CodeObjectVersion)) +
       " e_flags=" + hexadecimal(Elf->getPlatformFlags()))
          .str());
  Diagnostics.push_back((Twine("post_link.check=exports status=ok symbols=") +
                         diagnosticList(Defined))
                            .str());
  Diagnostics.push_back("post_link.check=unresolved status=ok symbols=[]");
  Diagnostics.push_back(
      (Twine("post_link.check=metadata status=") +
       (Metadata->Present ? "ok" : "absent") +
       " kernels=" + Twine(Metadata->Kernels.size()) + " target=" +
       diagnosticAtom(Metadata->Target ? StringRef(*Metadata->Target)
                                       : StringRef("absent")))
          .str());
  if (*Profile == PostLinkProfile::ExactLdsGemmSlice1)
    Diagnostics.push_back(
        "post_link.check=lds_gemm_slice1_profile status=ok "
        "workgroup=[64,1,1] kernarg_size=304 kernarg_align=8 "
        "group_size=1024 private_size=0 wavefront_size=64 spills=0 "
        "dynamic_stack=false");
  if (*Profile == PostLinkProfile::ExactRowSoftmaxV1)
    Diagnostics.push_back(
        (Twine("post_link.check=row_softmax_v1_profile status=ok ") +
         "profile_identity=row-softmax-v1-gfx942-cov6-llvm22-v1 " +
         "llvm_build_identity=" + ExactRowSoftmaxV1PublishedLlvmBuildIdentity +
         " llvm_layout=" + diagnosticAtom(ExactRowSoftmaxV1ProducerDataLayout) +
         " abi_checks=exact "
         "descriptor_checks=section-envelope-and-byte-identity "
         "transcript=sha256-consistency-only "
         "descriptor_source_authentication=outside-worker-complete")
            .str());
  if (*Profile == PostLinkProfile::ExactWave64CollectivesV1)
    Diagnostics.push_back(
        "post_link.check=wave64_collectives_v1_profile status=ok "
        "workgroup=[64,1,1] retained_grid=[1,1,1] explicit_kernarg_size=72 "
        "kernarg_size=328 kernarg_align=8 group_size=0 private_size=0 "
        "wavefront_size=64 calls=0 spills=0 dynamic_stack=false "
        "descriptor_binding=byte_exact rust_descriptor_admission=required");
  if (*Profile == PostLinkProfile::ExactFlashAttentionV1)
    Diagnostics.push_back(
        "post_link.check=flash_attention_v1_profile status=ok "
        "shape=B1,H1,N8,D16 causal=true recurrence=online_strict_f32 "
        "workgroup=[64,1,1] retained_grid=[1,1,1] "
        "explicit_kernarg_size=64 kernarg_size=320 kernarg_align=8 "
        "group_size=0 private_size=0 wavefront_size=64 calls=0 spills=0 "
        "dynamic_stack=false descriptor_binding=byte_exact "
        "ocml_provider=measured_structural_only "
        "rust_descriptor_admission=required");
  if (*Profile == PostLinkProfile::ExactWorkgroupLdsReductionV1)
    Diagnostics.push_back(
        "post_link.check=workgroup_lds_reduction_v1_profile status=ok "
        "workgroup=[64,1,1] retained_grid=[1,1,1] "
        "explicit_kernarg_size=40 kernarg_size=296 kernarg_align=8 "
        "group_size=0 required_dynamic_lds=256 "
        "hidden_dynamic_lds_offset=160 hidden_dynamic_lds_value=256 "
        "private_size=0 wavefront_size=64 barriers=2 lds_bytes=256 "
        "calls=0 spills=0 dynamic_stack=false "
        "descriptor_binding=byte_exact rust_descriptor_admission=required");
  if (*Profile == PostLinkProfile::ExactScopedAtomicV1)
    Diagnostics.push_back(
        "post_link.check=scoped_atomic_v1_profile status=ok "
        "workgroup=[64,1,1] retained_grid=[1,1,1] "
        "explicit_kernarg_size=40 kernarg_size=296 kernarg_align=8 "
        "group_size=0 private_size=0 wavefront_size=64 atomic=add "
        "ordering=relaxed scope=system address_space=global calls=0 "
        "spills=0 dynamic_stack=false descriptor_binding=byte_exact "
        "rust_descriptor_admission=required");
  if (*Profile == PostLinkProfile::ExactMoeTop2V1)
    Diagnostics.push_back(
        "post_link.check=moe_top2_t8_e4_k2_c4_v1_profile status=ok "
        "tokens=8 experts=4 top_k=2 capacity=4 workgroup=[64,1,1] "
        "retained_grid=[1,1,1] explicit_kernarg_size=128 "
        "kernarg_size=384 kernarg_align=8 group_size=0 private_size=0 "
        "wavefront_size=64 calls=0 atomics=0 lds_bytes=0 spills=0 "
        "dynamic_stack=false provider_closure=none "
        "descriptor_binding=byte_exact rust_descriptor_admission=required");
  for (const KernelLaunchContract &Kernel : Metadata->Kernels) {
    std::string Required = "absent";
    if (Kernel.RequiredWorkgroupSize)
      Required = (Twine("[") + Twine((*Kernel.RequiredWorkgroupSize)[0]) + "," +
                  Twine((*Kernel.RequiredWorkgroupSize)[1]) + "," +
                  Twine((*Kernel.RequiredWorkgroupSize)[2]) + "]")
                     .str();
    Diagnostics.push_back(
        (Twine("post_link.kernel name=") + diagnosticAtom(Kernel.Name) +
         " symbol=" + diagnosticAtom(Kernel.Symbol) +
         " kernarg_size=" + Twine(Kernel.KernargSegmentSize) +
         " group_size=" + Twine(Kernel.GroupSegmentFixedSize) +
         " private_size=" + Twine(Kernel.PrivateSegmentFixedSize) +
         " kernarg_align=" + Twine(Kernel.KernargSegmentAlign) +
         " wavefront_size=" + Twine(Kernel.WavefrontSize) +
         " max_workgroup_size=" + Twine(Kernel.MaxFlatWorkgroupSize) +
         " reqd_workgroup_size=" + Required)
            .str());
  }
  return Diagnostics;
}

Error validateSymbolClosure(ArrayRef<ElfContract> Contracts,
                            const Request &RequestValue) {
  std::set<std::string> Definitions;
  std::set<std::string> PublicDefinitions;
  std::map<std::string, size_t> DefinitionCounts;
  for (const ElfContract &Contract : Contracts) {
    Definitions.insert(Contract.Definitions.begin(),
                       Contract.Definitions.end());
    PublicDefinitions.insert(Contract.PublicDefinitions.begin(),
                             Contract.PublicDefinitions.end());
    for (const std::string &Name : Contract.Definitions)
      ++DefinitionCounts[Name];
  }
  for (const auto &[Name, Count] : DefinitionCounts)
    if (Count > 1)
      return pipelineError(Twine("duplicate definition: ") + Name);
  for (const ElfContract &Contract : Contracts)
    for (const std::string &Name : Contract.RequiredImports)
      if (!Definitions.contains(Name))
        return pipelineError(Twine("unresolved required import: ") + Name);
  for (const std::string &Name : RequestValue.ExpectedDefinedSymbols)
    if (!PublicDefinitions.contains(Name))
      return pipelineError(Twine("requested output has no public provider: ") +
                           Name);
  return Error::success();
}

struct TemporaryDirectory {
  SmallString<128> Path;
  ~TemporaryDirectory() {
    if (!Path.empty()) {
      std::error_code ErrorCode = sys::fs::remove_directories(Path);
      if (ErrorCode)
        consumeError(errorCodeToError(ErrorCode));
    }
  }
};

Error writeBytes(StringRef Path, ArrayRef<uint8_t> Bytes) {
  std::error_code ErrorCode;
  raw_fd_ostream Stream(Path, ErrorCode, sys::fs::OF_None);
  if (ErrorCode)
    return errorCodeToError(ErrorCode);
  Stream.write(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  Stream.close();
  if (Stream.has_error())
    return pipelineError("failed to write private linker input");
  return Error::success();
}

Expected<std::vector<uint8_t>> readBytes(StringRef Path, uint64_t MaxBytes) {
  sys::fs::file_status Status;
  if (std::error_code ErrorCode = sys::fs::status(Path, Status))
    return errorCodeToError(ErrorCode);
  if (!sys::fs::is_regular_file(Status) || Status.getSize() == 0 ||
      Status.getSize() > MaxBytes)
    return pipelineError("linked output violates byte bound");
  auto BufferOrError = MemoryBuffer::getFile(Path, false, false);
  if (!BufferOrError)
    return errorCodeToError(BufferOrError.getError());
  StringRef Bytes = (*BufferOrError)->getBuffer();
  if (Bytes.size() != Status.getSize())
    return pipelineError("linked output changed while it was inspected");
  return std::vector<uint8_t>(Bytes.bytes_begin(), Bytes.bytes_end());
}

Expected<std::vector<uint8_t>>
nativeLink(ArrayRef<std::vector<uint8_t>> Objects, const Request &RequestValue,
           StringRef Directory, std::vector<std::string> &Diagnostics) {
  std::vector<std::string> Paths;
  Paths.reserve(Objects.size());
  for (size_t I = 0; I < Objects.size(); ++I) {
    SmallString<160> Path(Directory);
    sys::path::append(Path, (Twine("input-") + Twine(I) + ".o").str());
    if (Error E = writeBytes(Path, Objects[I]))
      return E;
    Paths.push_back(Path.str().str());
  }
  SmallString<160> OutputPath(Directory);
  sys::path::append(OutputPath, "linked.hsaco");

  std::vector<std::string> OwnedArguments = {
      "ld.lld",           "--shared",
      "-Bsymbolic",       "--no-undefined",
      "--export-dynamic", "--build-id=none",
      "--nostdlib",       "--no-dependent-libraries",
      "--fatal-warnings", "--threads=1"};
  if (RequestValue.LinkOptions.StripDebug)
    OwnedArguments.push_back("--strip-debug");
  for (const std::string &Name : RequestValue.ExpectedDefinedSymbols)
    OwnedArguments.push_back((Twine("--undefined=") + Name).str());
  OwnedArguments.insert(OwnedArguments.end(), Paths.begin(), Paths.end());
  OwnedArguments.push_back("-o");
  OwnedArguments.push_back(OutputPath.str().str());
  std::vector<const char *> Arguments;
  Arguments.reserve(OwnedArguments.size());
  for (const std::string &Argument : OwnedArguments)
    Arguments.push_back(Argument.c_str());

  BoundedRawStream OutputStream(MaxTotalDiagnosticBytes);
  BoundedRawStream ErrorStream(MaxTotalDiagnosticBytes);
  lld::Result LinkResult = lld::lldMain(Arguments, OutputStream, ErrorStream,
                                        {{lld::Gnu, &lld::elf::link}});
  detail::enforceReusableLldResult(LinkResult);
  OutputStream.flush();
  ErrorStream.flush();
  if (!OutputStream.str().empty())
    Diagnostics.push_back(OutputStream.str().str());
  if (OutputStream.truncated())
    Diagnostics.push_back("LLD stdout exceeded diagnostic byte bound");
  if (!ErrorStream.str().empty())
    Diagnostics.push_back(ErrorStream.str().str());
  if (ErrorStream.truncated())
    Diagnostics.push_back("LLD stderr exceeded diagnostic byte bound");
  if (LinkResult.retCode != 0)
    return pipelineError("LLD ELF link failed");
  return readBytes(OutputPath, RequestValue.MaxOutputBytes);
}

} // namespace

Error validateExactWave64CollectivesV1CompilerInputForTesting(
    ArrayRef<uint8_t> Bytes) {
  return validateExactWave64CompilerInput(
      StringRef(reinterpret_cast<const char *>(Bytes.data()), Bytes.size()));
}

Error validateExactFlashAttentionV1LlvmBuildIdentityForTesting(
    StringRef Identity) {
  return validateExactFlashAttentionLlvmBuildIdentity(Identity);
}

Expected<std::vector<uint8_t>>
makeExactRowSoftmaxV1CompilerInputForTesting(StringRef CanonicalBody,
                                             ArrayRef<uint8_t> Descriptor,
                                             ArrayRef<uint8_t> Transcript) {
  if (SHA256::hash(arrayRefFromStringRef(CanonicalBody)) != ExactRowBodySha256)
    return pipelineError("test fixture row-softmax body identity mismatch");
  if (Descriptor.empty() || Descriptor.size() > 64 * 1024)
    return pipelineError("test fixture row-softmax descriptor is invalid");
  if (Transcript.empty() || Transcript.size() > 4096)
    return pipelineError("test fixture row-softmax transcript is invalid");

  std::string Result = CanonicalBody.str();
  bool FirstSection = true;
  auto AppendSection = [&](StringRef Name, ArrayRef<uint8_t> Bytes) {
    std::string Header = (Twine(FirstSection ? "\nmodule asm \".section "
                                             : "module asm \".section ") +
                          Name +
                          ",\\22\\22,@progbits\"\n"
                          "module asm \".balign 8\"\n")
                             .str();
    FirstSection = false;
    Result.append(Header);
    static constexpr char Hex[] = "0123456789abcdef";
    for (size_t Offset = 0; Offset < Bytes.size(); Offset += 16) {
      std::string Line = "module asm \".byte ";
      size_t End = std::min(Bytes.size(), Offset + 16);
      for (size_t Index = Offset; Index != End; ++Index) {
        if (Index != Offset)
          Line.append(", ");
        Line.append("0x");
        Line.push_back(Hex[Bytes[Index] >> 4]);
        Line.push_back(Hex[Bytes[Index] & 0x0f]);
      }
      Line.append("\"\n");
      Result.append(Line);
    }
  };
  AppendSection(ExactRowDescriptorSection, Descriptor);
  AppendSection(ExactRowTranscriptSection, Transcript);
  std::array<uint8_t, 32> TranscriptDigest = SHA256::hash(Transcript);
  AppendSection(ExactRowTranscriptDigestSection, TranscriptDigest);
  AppendSection(ExactRowExpBoundarySection, ExactRowExpBoundaryIdentity);

  auto Layout = DataLayout::parse(ExactRowSoftmaxV1ProducerDataLayout);
  if (!Layout)
    return Layout.takeError();
  if (Error E = validateExactRowSoftmaxV1CompilerInput(Result, *Layout))
    return E;
  return std::vector<uint8_t>(Result.begin(), Result.end());
}

Error validateExactRowSoftmaxV1CompilerInputForTesting(
    ArrayRef<uint8_t> Bytes) {
  auto Layout = DataLayout::parse(ExactRowSoftmaxV1ProducerDataLayout);
  if (!Layout)
    return Layout.takeError();
  return validateExactRowSoftmaxV1CompilerInput(
      StringRef(reinterpret_cast<const char *>(Bytes.data()), Bytes.size()),
      *Layout);
}

Expected<std::string> exactWorkgroupSyncDataLayoutForTesting() {
  Request RequestValue;
  RequestValue.Target = "gfx942:xnack-";
  RequestValue.LinkOptions = {OptimizationLevel::O2, true, true};
  auto Machine = createMachine(RequestValue);
  if (!Machine)
    return Machine.takeError();
  return (*Machine)->createDataLayout().getStringRepresentation();
}

Expected<std::vector<uint8_t>> makeExactWorkgroupSyncCompilerInputForTesting(
    StringRef CanonicalBody, ArrayRef<uint8_t> Descriptor,
    ExactWorkgroupSyncProfileForTesting ProfileKind) {
  const ExactWorkgroupSyncProfile &Profile =
      ProfileKind == ExactWorkgroupSyncProfileForTesting::LdsReduction
          ? ExactWorkgroupLdsReductionV1
          : ExactScopedAtomicV1;
  if (SHA256::hash(arrayRefFromStringRef(CanonicalBody)) != Profile.BodySha256)
    return pipelineError("test fixture workgroup-sync body identity mismatch");
  if (Descriptor.empty() || Descriptor.size() > 64 * 1024)
    return pipelineError("test fixture workgroup-sync descriptor is invalid");

  const std::array<std::string, 13> Sections = {
      ExactWave64DescriptorSection.str(),
      (Twine(Profile.SectionPrefix) + ".source.v1").str(),
      (Twine(Profile.SectionPrefix) + ".namespace.v1").str(),
      (Twine(Profile.SectionPrefix) + ".authority.v1").str(),
      (Twine(Profile.SectionPrefix) + ".mir.v1").str(),
      (Twine(Profile.SectionPrefix) + ".fnabi.v1").str(),
      (Twine(Profile.SectionPrefix) + ".semantics.v1").str(),
      (Twine(Profile.SectionPrefix) + ".terminals.v3").str(),
      (Twine(Profile.SectionPrefix) + ".abi.v1").str(),
      (Twine(Profile.SectionPrefix) + ".effects.v1").str(),
      (Twine(Profile.SectionPrefix) + ".resources.v1").str(),
      (Twine(Profile.SectionPrefix) + ".kir.v1").str(),
      (Twine(Profile.SectionPrefix) + ".layout.v1").str()};
  std::vector<uint8_t> Result(CanonicalBody.bytes_begin(),
                              CanonicalBody.bytes_end());
  auto AppendSection = [&](StringRef Name, ArrayRef<uint8_t> Bytes) {
    std::string Header = (Twine("\nmodule asm \".section ") + Name +
                          ",\\22\\22,@progbits\"\n"
                          "module asm \".balign 8\"\n")
                             .str();
    llvm::append_range(Result, arrayRefFromStringRef(Header));
    static constexpr char Hex[] = "0123456789abcdef";
    for (size_t Offset = 0; Offset < Bytes.size(); Offset += 16) {
      std::string Line = "module asm \".byte ";
      size_t End = std::min(Bytes.size(), Offset + 16);
      for (size_t Index = Offset; Index != End; ++Index) {
        if (Index != Offset)
          Line.append(", ");
        Line.append("0x");
        Line.push_back(Hex[Bytes[Index] >> 4]);
        Line.push_back(Hex[Bytes[Index] & 0x0f]);
      }
      Line.append("\"\n");
      llvm::append_range(Result, arrayRefFromStringRef(Line));
    }
  };
  AppendSection(Sections[0], Descriptor);
  for (size_t Index = 0; Index != Profile.SectionIdentities.size(); ++Index)
    AppendSection(Sections[Index + 1], Profile.SectionIdentities[Index]);
  auto DataLayout = exactWorkgroupSyncDataLayoutForTesting();
  if (!DataLayout)
    return DataLayout.takeError();
  std::array<uint8_t, 32> LayoutIdentity =
      SHA256::hash(arrayRefFromStringRef(*DataLayout));
  AppendSection(Sections.back(), LayoutIdentity);
  auto ParsedLayout = llvm::DataLayout::parse(*DataLayout);
  if (!ParsedLayout)
    return ParsedLayout.takeError();
  if (Error E = validateExactWorkgroupSyncCompilerInput(
          StringRef(reinterpret_cast<const char *>(Result.data()),
                    Result.size()),
          Profile, *ParsedLayout))
    return E;
  return Result;
}

Error validateExactWorkgroupSyncCompilerInputForTesting(
    ArrayRef<uint8_t> Bytes, ExactWorkgroupSyncProfileForTesting ProfileKind) {
  const ExactWorkgroupSyncProfile &Profile =
      ProfileKind == ExactWorkgroupSyncProfileForTesting::LdsReduction
          ? ExactWorkgroupLdsReductionV1
          : ExactScopedAtomicV1;
  auto DataLayout = exactWorkgroupSyncDataLayoutForTesting();
  if (!DataLayout)
    return DataLayout.takeError();
  auto ParsedLayout = llvm::DataLayout::parse(*DataLayout);
  if (!ParsedLayout)
    return ParsedLayout.takeError();
  return validateExactWorkgroupSyncCompilerInput(
      StringRef(reinterpret_cast<const char *>(Bytes.data()), Bytes.size()),
      Profile, *ParsedLayout);
}

Error validateExactWorkgroupSyncModuleForTesting(
    StringRef Text, ExactWorkgroupSyncProfileForTesting ProfileKind) {
  const ExactWorkgroupSyncProfile &Profile =
      ProfileKind == ExactWorkgroupSyncProfileForTesting::LdsReduction
          ? ExactWorkgroupLdsReductionV1
          : ExactScopedAtomicV1;
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  auto Buffer = MemoryBuffer::getMemBufferCopy(Text, "<test-module>");
  std::unique_ptr<Module> ModuleValue =
      parseAssembly(Buffer->getMemBufferRef(), Diagnostic, Context);
  if (!ModuleValue) {
    std::string Message;
    raw_string_ostream Stream(Message);
    Diagnostic.print("fe2o3-workgroup-test", Stream, false, false);
    Stream.flush();
    return pipelineError(Message);
  }
  if (verifyModule(*ModuleValue))
    return pipelineError("test workgroup-sync module verification failed");
  auto DataLayout = exactWorkgroupSyncDataLayoutForTesting();
  if (!DataLayout)
    return DataLayout.takeError();
  auto ParsedLayout = llvm::DataLayout::parse(*DataLayout);
  if (!ParsedLayout)
    return ParsedLayout.takeError();
  return validateExactWorkgroupSyncModule(*ModuleValue, Profile, *ParsedLayout);
}

Error validateExactMoeTop2V1CompilerInputForTesting(ArrayRef<uint8_t> Bytes) {
  auto DataLayout = exactWorkgroupSyncDataLayoutForTesting();
  if (!DataLayout)
    return DataLayout.takeError();
  auto ParsedLayout = llvm::DataLayout::parse(*DataLayout);
  if (!ParsedLayout)
    return ParsedLayout.takeError();
  return validateExactMoeTop2V1CompilerInput(
      StringRef(reinterpret_cast<const char *>(Bytes.data()), Bytes.size()),
      *ParsedLayout);
}

Error validateExactMoeTop2V1ModuleForTesting(StringRef Text) {
  LLVMContext Context;
  SMDiagnostic Diagnostic;
  auto Buffer = MemoryBuffer::getMemBufferCopy(Text, "<test-moe-module>");
  std::unique_ptr<Module> ModuleValue =
      parseAssembly(Buffer->getMemBufferRef(), Diagnostic, Context);
  if (!ModuleValue) {
    std::string Message;
    raw_string_ostream Stream(Message);
    Diagnostic.print("fe2o3-moe-test", Stream, false, false);
    Stream.flush();
    return pipelineError(Message);
  }
  std::string VerificationMessage;
  raw_string_ostream VerificationStream(VerificationMessage);
  if (verifyModule(*ModuleValue, &VerificationStream)) {
    VerificationStream.flush();
    return pipelineError(Twine("test MoE top-2 module verification failed: ") +
                         VerificationMessage);
  }
  auto DataLayout = exactWorkgroupSyncDataLayoutForTesting();
  if (!DataLayout)
    return DataLayout.takeError();
  auto ParsedLayout = llvm::DataLayout::parse(*DataLayout);
  if (!ParsedLayout)
    return ParsedLayout.takeError();
  return validateExactMoeTop2V1Module(*ModuleValue, *ParsedLayout);
}

Error validateExactLdsGemmSlice1MetadataForTesting(StringRef MetadataBlob) {
  MetadataContract Metadata;
  Metadata.Present = true;
  std::set<std::string> Names;
  std::set<std::string> Symbols;
  if (Error E =
          appendMetadataBlob(MetadataBlob, Metadata, Names, Symbols,
                             MetadataValidationPolicy::ExactLdsGemmSlice1))
    return E;
  if (!Metadata.Target ||
      *Metadata.Target != "amdgcn-amd-amdhsa--gfx942:xnack-")
    return postLinkError("lds_gemm_slice1_profile",
                         "kernel_contract_metadata_target");
  llvm::sort(Metadata.Kernels, [](const KernelLaunchContract &Left,
                                  const KernelLaunchContract &Right) {
    return std::tie(Left.Name, Left.Symbol) <
           std::tie(Right.Name, Right.Symbol);
  });
  return validateExactLdsGemmSlice1Metadata(Metadata);
}

Error validateExactRowSoftmaxV1MetadataForTesting(StringRef MetadataBlob) {
  MetadataContract Metadata;
  Metadata.Present = true;
  std::set<std::string> Names;
  std::set<std::string> Symbols;
  if (Error E = appendMetadataBlob(MetadataBlob, Metadata, Names, Symbols,
                                   MetadataValidationPolicy::ExactRowSoftmaxV1))
    return E;
  if (!Metadata.Target ||
      *Metadata.Target != "amdgcn-amd-amdhsa--gfx942:xnack-")
    return postLinkError(ExactRowSoftmaxV1Check,
                         "kernel_contract_metadata_target");
  llvm::sort(Metadata.Kernels, [](const KernelLaunchContract &Left,
                                  const KernelLaunchContract &Right) {
    return std::tie(Left.Name, Left.Symbol) <
           std::tie(Right.Name, Right.Symbol);
  });
  return validateExactRowSoftmaxV1Metadata(Metadata);
}

Error validateExactWave64CollectivesV1MetadataForTesting(
    StringRef MetadataBlob) {
  MetadataContract Metadata;
  Metadata.Present = true;
  std::set<std::string> Names;
  std::set<std::string> Symbols;
  if (Error E = appendMetadataBlob(
          MetadataBlob, Metadata, Names, Symbols,
          MetadataValidationPolicy::ExactWave64CollectivesV1))
    return E;
  if (!Metadata.Target ||
      *Metadata.Target != "amdgcn-amd-amdhsa--gfx942:xnack-")
    return postLinkError("wave64_collectives_v1_profile",
                         "kernel_contract_metadata_target");
  llvm::sort(Metadata.Kernels, [](const KernelLaunchContract &Left,
                                  const KernelLaunchContract &Right) {
    return std::tie(Left.Name, Left.Symbol) <
           std::tie(Right.Name, Right.Symbol);
  });
  return validateExactWave64CollectivesV1Metadata(Metadata);
}

Error validateGenericMetadataForTesting(StringRef MetadataBlob) {
  MetadataContract Metadata;
  Metadata.Present = true;
  std::set<std::string> Names;
  std::set<std::string> Symbols;
  return appendMetadataBlob(MetadataBlob, Metadata, Names, Symbols,
                            MetadataValidationPolicy::Generic);
}

Error validateExactLdsGemmSlice1ElfClosureForTesting(ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<test-output>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!Elf)
    return pipelineError("test output is not ELF64LE");
  return validateExactLdsGemmSlice1ElfClosure(*Elf);
}

Error validateExactRowSoftmaxV1ElfClosureForTesting(ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<test-output>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!Elf)
    return pipelineError("test output is not ELF64LE");
  return validateExactRowSoftmaxV1ElfClosure(*Elf);
}

Error validateExactWave64CollectivesV1ElfClosureForTesting(
    ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<test-output>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!Elf)
    return pipelineError("test output is not ELF64LE");
  return validateExactWave64CollectivesV1ElfClosure(*Elf);
}

Error validateExactWorkgroupSyncElfClosureForTesting(
    ArrayRef<uint8_t> Bytes, ExactWorkgroupSyncProfileForTesting ProfileKind) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<test-output>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!Elf)
    return pipelineError("test output is not ELF64LE");
  const ExactWorkgroupSyncProfile &Profile =
      ProfileKind == ExactWorkgroupSyncProfileForTesting::LdsReduction
          ? ExactWorkgroupLdsReductionV1
          : ExactScopedAtomicV1;
  return validateExactWorkgroupSyncElfClosure(*Elf, Profile);
}

Error validateExactMoeTop2V1ElfClosureForTesting(ArrayRef<uint8_t> Bytes) {
  StringRef Data(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  auto ObjectOrError =
      ObjectFile::createObjectFile(MemoryBufferRef(Data, "<test-output>"));
  if (!ObjectOrError)
    return ObjectOrError.takeError();
  auto *Elf = dyn_cast<ELF64LEObjectFile>(ObjectOrError->get());
  if (!Elf)
    return pipelineError("test output is not ELF64LE");
  return validateExactMoeTop2V1ElfClosure(*Elf);
}

Expected<std::vector<std::string>>
inspectLinkedOutputForPublication(ArrayRef<uint8_t> Bytes,
                                  const Request &RequestValue) {
  if (Error E = validateRequest(RequestValue))
    return E;
  auto MachineOrError = createMachine(RequestValue);
  if (!MachineOrError)
    return MachineOrError.takeError();
  std::unique_ptr<TargetMachine> Machine = std::move(*MachineOrError);
  LLVMContext Context;
  Module Reference("fe2o3-publication-target-reference", Context);
  if (Error E = setAndCheckModuleContract(Reference, RequestValue, *Machine))
    return E;
  auto ReferenceObject = emitObject(Reference, *Machine);
  if (!ReferenceObject)
    return ReferenceObject.takeError();
  auto ExpectedElf = inspectRelocatable(*ReferenceObject);
  if (!ExpectedElf)
    return ExpectedElf.takeError();
  if (ExpectedElf->AbiVersion !=
      expectedAbiVersion(RequestValue.CodeObjectVersion))
    return pipelineError(
        "target machine emitted the wrong code-object version");
  return inspectOutput(Bytes, *ExpectedElf, RequestValue);
}

Response executeImpl(const Request &RequestValue,
                     const Gfx942DeviceLibraryPolicy *TestPolicy) {
  if (RequestValue.LlvmBuildIdentity != LlvmBuildIdentity)
    return failure(RequestValue, Stage::Toolchain,
                   {"request LLVM identity does not match worker measurement"});
  if (RequestValue.Protocol == ProtocolVersion::V2 &&
      RequestValue.WorkerBuildIdentity != WorkerBuildIdentity)
    return failure(
        RequestValue, Stage::Toolchain,
        {"request worker identity does not match worker measurement"});
  if (Error E = validateRequest(RequestValue))
    return failure(RequestValue, Stage::InputValidation, std::move(E));
  if (mentionsExactWave64CollectivesV1(RequestValue) &&
      !isClosedExactWave64CollectivesV1Request(RequestValue))
    return failure(RequestValue, Stage::InputValidation,
                   {"exact Wave64 collectives symbols require the closed "
                    "Worker V2 profile"});
  if (mentionsExactRowSoftmaxV1(RequestValue) &&
      !isClosedExactRowSoftmaxV1Request(RequestValue))
    return failure(RequestValue, Stage::InputValidation,
                   {"exact row-softmax V1 symbols or compiler markers require "
                    "the closed Worker V2 profile"});
  if (isClosedExactRowSoftmaxV1Request(RequestValue))
    if (Error E = validateExactRowSoftmaxV1LlvmBuildIdentity(LlvmBuildIdentity))
      return failure(RequestValue, Stage::Toolchain, std::move(E));
  if (mentionsExactFlashAttentionV1(RequestValue) &&
      !isClosedExactFlashAttentionV1Request(RequestValue))
    return failure(RequestValue, Stage::InputValidation,
                   {"exact FlashAttention V1 symbols require the closed "
                    "Worker V2 profile"});
  if (isClosedExactFlashAttentionV1Request(RequestValue))
    if (Error E =
            validateExactFlashAttentionLlvmBuildIdentity(LlvmBuildIdentity))
      return failure(RequestValue, Stage::Toolchain, std::move(E));
  if (mentionsExactWorkgroupSync(RequestValue)) {
    const ExactWorkgroupSyncProfile *Profile =
        exactWorkgroupSyncProfile(RequestValue);
    if (!Profile || !isClosedExactWorkgroupSyncRequest(RequestValue, *Profile))
      return failure(RequestValue, Stage::InputValidation,
                     {"exact workgroup-sync symbols require one closed Worker "
                      "V2 profile"});
  }
  if (mentionsExactMoeTop2V1(RequestValue) &&
      !isClosedExactMoeTop2V1Request(RequestValue))
    return failure(
        RequestValue, Stage::InputValidation,
        {"exact MoE top-2 symbols require the closed Worker V2 profile"});

  auto MachineOrError = createMachine(RequestValue);
  if (!MachineOrError)
    return failure(RequestValue, Stage::Toolchain, MachineOrError.takeError());
  std::unique_ptr<TargetMachine> Machine = std::move(*MachineOrError);

  std::optional<SymbolContract> CompilerModule;
  if (RequestValue.Protocol == ProtocolVersion::V2) {
    auto Inspected = inspectNormalizedCompilerModule(RequestValue, *Machine);
    if (!Inspected)
      return failure(RequestValue, Stage::InputValidation,
                     Inspected.takeError());
    CompilerModule = std::move(*Inspected);
  }

  std::set<std::string> BuiltinOcmlImports;
  const std::set<std::string> NoImports;
  const std::set<std::string> &MeasuredImports =
      CompilerModule ? CompilerModule->RequiredImports : NoImports;
  for (const std::string &Import : MeasuredImports) {
    if (isSupportedGfx942OcmlImport(Import)) {
      BuiltinOcmlImports.insert(Import);
      continue;
    }
    if (isOcmlImportNamespace(Import))
      return failure(RequestValue, Stage::InputValidation,
                     {"unsupported gfx942 OCML import: " + Import});
  }

  std::vector<Input> BuiltinProviders;
  std::optional<DeviceLibraryProviderEvidence> ProviderEvidence;
  if (!BuiltinOcmlImports.empty()) {
    TargetParts Parts = parseTarget(RequestValue.Target);
    if (RequestValue.Protocol != ProtocolVersion::V2 || Parts.Cpu != "gfx942" ||
        !isSupportedGfx942OcmlCodeObjectVersion(RequestValue.CodeObjectVersion))
      return failure(RequestValue, Stage::InputValidation,
                     {"measured OCML providers require Worker V2, gfx942, and "
                      "code-object V5 or V6"});
    Expected<Gfx942DeviceLibraryPolicy> Measured =
        TestPolicy ? Expected<Gfx942DeviceLibraryPolicy>(*TestPolicy)
                   : measuredGfx942DeviceLibraryPolicy();
    if (!Measured)
      return failure(RequestValue, Stage::Toolchain, Measured.takeError());
    std::vector<std::string> MeasuredImportList(MeasuredImports.begin(),
                                                MeasuredImports.end());
    auto Loaded = loadGfx942DeviceLibraries(MeasuredImportList, *Measured);
    if (!Loaded)
      return failure(RequestValue, Stage::Toolchain, Loaded.takeError());
    BuiltinProviders = std::move(*Loaded);

    DeviceLibraryProviderEvidence Evidence;
    Evidence.ProviderIdentity = "gfx942-ocml-v1";
    Evidence.Target = RequestValue.Target;
    Evidence.CodeObjectVersion = RequestValue.CodeObjectVersion;
    Evidence.ImportSymbols.assign(BuiltinOcmlImports.begin(),
                                  BuiltinOcmlImports.end());
    for (const PinnedDeviceLibraryFile &File : Measured->Files)
      Evidence.Files.push_back({File.Basename, File.Digest});
    auto ManifestIdentity = calculateProviderManifestIdentity(Evidence);
    if (!ManifestIdentity)
      return failure(RequestValue, Stage::Toolchain,
                     ManifestIdentity.takeError());
    Evidence.ManifestIdentity = *ManifestIdentity;
    ProviderEvidence = std::move(Evidence);
  }

  if (RequestValue.Protocol == ProtocolVersion::V2)
    if (Error E = validateV2SymbolRoles(RequestValue, *CompilerModule,
                                        BuiltinProviders))
      return failure(RequestValue, Stage::InputValidation, std::move(E));

  LLVMContext Context;
  auto LinkedModule =
      linkBitcode(RequestValue, BuiltinProviders, Context, *Machine);
  if (!LinkedModule)
    return failure(RequestValue, Stage::BitcodeLink, LinkedModule.takeError());
  std::optional<std::array<uint8_t, 32>> FlashLinkedBitcodeIdentity;
  if (isClosedExactFlashAttentionV1Request(RequestValue) && *LinkedModule)
    FlashLinkedBitcodeIdentity = bitcodeIdentity(**LinkedModule);

  Module Reference("fe2o3-target-reference", Context);
  if (Error E = setAndCheckModuleContract(Reference, RequestValue, *Machine))
    return failure(RequestValue, Stage::Toolchain, std::move(E));
  auto ReferenceObject = emitObject(Reference, *Machine);
  if (!ReferenceObject)
    return failure(RequestValue, Stage::Codegen, ReferenceObject.takeError());
  auto ExpectedElf = inspectRelocatable(*ReferenceObject);
  if (!ExpectedElf)
    return failure(RequestValue, Stage::Codegen, ExpectedElf.takeError());

  if (ExpectedElf->AbiVersion !=
      expectedAbiVersion(RequestValue.CodeObjectVersion))
    return failure(RequestValue, Stage::Codegen,
                   {"target machine emitted the wrong code-object version"});

  std::vector<std::vector<uint8_t>> Objects;
  std::vector<ElfContract> ObjectContracts;
  std::optional<std::array<uint8_t, 32>> FlashOptimizedBitcodeIdentity;
  std::optional<std::array<uint8_t, 32>> FlashObjectIdentity;
  for (const Input &InputValue : RequestValue.Inputs) {
    if (InputValue.Kind != InputKind::AmdGpuRelocatable)
      continue;
    auto Contract = inspectRelocatable(InputValue.Bytes);
    if (!Contract)
      return failure(RequestValue, Stage::InputValidation,
                     Contract.takeError());
    if (!matches(*Contract, *ExpectedElf))
      return failure(RequestValue, Stage::InputValidation,
                     {"native input target or code-object version mismatch"});
    Objects.push_back(InputValue.Bytes);
    ObjectContracts.push_back(std::move(*Contract));
  }

  if (*LinkedModule) {
    if (Error E = optimizeModule(**LinkedModule, RequestValue, *Machine))
      return failure(RequestValue, Stage::Optimization, std::move(E));
    if (isClosedExactFlashAttentionV1Request(RequestValue))
      FlashOptimizedBitcodeIdentity = bitcodeIdentity(**LinkedModule);
    auto GeneratedObject = emitObject(**LinkedModule, *Machine);
    if (!GeneratedObject)
      return failure(RequestValue, Stage::Codegen, GeneratedObject.takeError());
    auto Contract = inspectRelocatable(*GeneratedObject);
    if (!Contract)
      return failure(RequestValue, Stage::Codegen, Contract.takeError());
    if (!matches(*Contract, *ExpectedElf))
      return failure(RequestValue, Stage::Codegen,
                     {"generated object target contract mismatch"});
    if (Error E = validateExactDynamicLdsPseudoImport(
            RequestValue, Contract->RequiredImports,
            "optimized relocatable object", false))
      return failure(RequestValue, Stage::Codegen, std::move(E));
    if (isClosedExactFlashAttentionV1Request(RequestValue))
      FlashObjectIdentity = SHA256::hash(*GeneratedObject);
    Objects.push_back(std::move(*GeneratedObject));
    ObjectContracts.push_back(std::move(*Contract));
  }
  if (Objects.empty())
    return failure(RequestValue, Stage::InputValidation,
                   {"request produced no native link inputs"});
  if (Error E = validateSymbolClosure(ObjectContracts, RequestValue))
    return failure(RequestValue, Stage::InputValidation, std::move(E));

  TemporaryDirectory Temporary;
  if (std::error_code ErrorCode =
          sys::fs::createUniqueDirectory("fe2o3-llvm-link", Temporary.Path))
    return failure(RequestValue, Stage::NativeLink,
                   errorCodeToError(ErrorCode));
  std::vector<std::string> LinkDiagnostics;
  if (!BuiltinProviders.empty())
    LinkDiagnostics.push_back(
        (Twine("device_library.check=identity status=ok ") +
         "provider=gfx942-ocml-v1 roots=" + diagnosticList(BuiltinOcmlImports) +
         " files=" + Twine(BuiltinProviders.size()))
            .str());
  auto LinkedBytes =
      nativeLink(Objects, RequestValue, Temporary.Path, LinkDiagnostics);
  if (!LinkedBytes) {
    LinkDiagnostics.push_back(errorToDiagnostic(LinkedBytes.takeError()));
    return failure(RequestValue, Stage::NativeLink,
                   canonicalDiagnostics(LinkDiagnostics, Temporary.Path));
  }

  auto PublicationDiagnostics =
      inspectOutput(*LinkedBytes, *ExpectedElf, RequestValue);
  if (!PublicationDiagnostics)
    return failure(RequestValue, Stage::OutputInspection,
                   PublicationDiagnostics.takeError());
  LinkDiagnostics.insert(LinkDiagnostics.end(), PublicationDiagnostics->begin(),
                         PublicationDiagnostics->end());

  if (isClosedExactFlashAttentionV1Request(RequestValue)) {
    if (!FlashLinkedBitcodeIdentity || !FlashOptimizedBitcodeIdentity ||
        !FlashObjectIdentity)
      return failure(RequestValue, Stage::OutputInspection,
                     {"exact FlashAttention V1 reproducibility identities are "
                      "incomplete"});
    LinkDiagnostics.push_back(
        (Twine(
             "post_link.check=flash_attention_v1_reproducibility status=ok ") +
         "llvm_build_identity=" + diagnosticAtom(LlvmBuildIdentity) +
         " input_ir_sha256=" +
         digestHex(SHA256::hash(RequestValue.CompilerModule.Bytes)) +
         " linked_bitcode_sha256=" + digestHex(*FlashLinkedBitcodeIdentity) +
         " optimized_bitcode_sha256=" +
         digestHex(*FlashOptimizedBitcodeIdentity) +
         " object_sha256=" + digestHex(*FlashObjectIdentity) +
         " raw_hsaco_sha256=" + digestHex(SHA256::hash(*LinkedBytes)))
            .str());
  }

  Output ResultOutput{SHA256::hash(*LinkedBytes), std::move(*LinkedBytes)};
  Response Result{
      RequestValue.RequestId,
      RequestValue.Identity,
      (TestPolicy ? UnauthenticatedTestWorkerIdentity : WorkerBuildIdentity)
          .str(),
      Stage::Complete,
      canonicalDiagnostics(LinkDiagnostics, Temporary.Path),
      std::move(ResultOutput)};
  Result.Protocol = RequestValue.Protocol;
  Result.CompilerEnvelopeIdentity = RequestValue.CompilerEnvelopeIdentity;
  Result.DeviceLibraryProvider = std::move(ProviderEvidence);
  return Result;
}

Response execute(const Request &RequestValue) {
  return executeImpl(RequestValue, nullptr);
}

Response executeWithUnauthenticatedGfx942DeviceLibraryPolicyForTesting(
    const Request &RequestValue, const Gfx942DeviceLibraryPolicy &Policy) {
  return executeImpl(RequestValue, &Policy);
}

} // namespace fe2o3::worker
