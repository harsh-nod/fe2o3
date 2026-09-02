#ifndef FE2O3_LLVM_LINK_WORKER_MACHINE_EFFECT_H
#define FE2O3_LLVM_LINK_WORKER_MACHINE_EFFECT_H

#include "llvm/ADT/ArrayRef.h"
#include "llvm/ADT/StringRef.h"
#include "llvm/Support/Error.h"

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace fe2o3::worker {

inline constexpr size_t MaxPhysicalMachineEffectPayloadBytes = 64 * 1024 * 1024;
inline constexpr size_t MaxPhysicalMachineEffectEvidenceBytes = 8 * 1024 * 1024;
inline constexpr size_t MaxPhysicalMachineEffectFunctions = 64;
inline constexpr size_t MaxPhysicalMachineEffectEffects = 16 * 1024;
inline constexpr size_t MaxPhysicalMachineTraceBlocks = 4 * 1024;
inline constexpr size_t MaxPhysicalMachineTraceInstructions = 16 * 1024;
inline constexpr size_t MaxPhysicalMachineTraceBytes = 16 * 1024 * 1024;
inline constexpr size_t MaxPhysicalMachineAnalysisBundleBytes =
    MaxPhysicalMachineEffectEvidenceBytes + MaxPhysicalMachineTraceBytes +
    1024;

struct PhysicalMachineEffectIdentities {
  std::array<uint8_t, 32> Analyzer{};
  std::array<uint8_t, 32> Toolchain{};
};

struct PhysicalMachineEffectBudget {
  uint32_t GlobalAddresses = 0;
  uint32_t GlobalReads = 0;
  uint32_t GlobalWrites = 0;
  uint32_t Returns = 0;
  uint32_t DirectCalls = 0;
};

struct PhysicalMachineEffectEntryRequest {
  std::string Symbol;
  PhysicalMachineEffectBudget Budget;
};

struct PhysicalMachineEffectRequest {
  std::array<uint8_t, 32> ExecutionChallenge{};
  std::array<uint8_t, 32> AnalyzerIdentity{};
  std::array<uint8_t, 32> ToolchainIdentity{};
  std::array<uint8_t, 32> PayloadDigest{};
  uint64_t PayloadBytes = 0;
  std::vector<PhysicalMachineEffectEntryRequest> Entries;
  std::vector<uint8_t> Payload;
  std::array<uint8_t, 32> RequestIdentity{};
  uint64_t RequestBytes = 0;
};

enum class PhysicalMachineEffectKind : uint8_t {
  GlobalAddress = 1,
  GlobalRead = 2,
  GlobalWrite = 3,
  Return = 4,
};

struct PhysicalMachineEntryEvidence {
  std::string Symbol;
  std::array<uint8_t, 32> DescriptorIdentity{};
  uint64_t CodeOffset = 0;
  uint64_t CodeSize = 0;
};

struct PhysicalMachineFunctionEvidence {
  std::string Symbol;
  uint64_t CodeOffset = 0;
  uint64_t CodeSize = 0;
  std::vector<std::string> DirectCallees;
};

struct PhysicalMachineEffect {
  std::string EntrySymbol;
  std::string FunctionSymbol;
  uint64_t InstructionOffset = 0;
  PhysicalMachineEffectKind Kind = PhysicalMachineEffectKind::GlobalAddress;
  uint16_t ByteWidth = 0;
};

enum class PhysicalMachineOperandKind : uint8_t {
  Register = 1,
  SignedImmediate = 2,
  SingleFloatImmediate = 3,
  DoubleFloatImmediate = 4,
  AbsoluteExpression = 5,
};

struct PhysicalMachineOperandTrace {
  PhysicalMachineOperandKind Kind = PhysicalMachineOperandKind::Register;
  std::string Register;
  uint64_t Value = 0;
  int32_t TiedTo = -1;
};

enum class PhysicalMachineBranchKind : uint8_t {
  None = 0,
  ConditionalDirect = 1,
  UnconditionalDirect = 2,
  DirectCall = 3,
  Return = 4,
};

enum class PhysicalMachineMemoryAccess : uint8_t {
  None = 0,
  Read = 1,
  Write = 2,
  ReadWrite = 3,
  WorkgroupRead = 4,
  WorkgroupWrite = 5,
  WorkgroupReadWrite = 6,
};

struct PhysicalMachineBasicBlockTrace {
  std::string FunctionSymbol;
  uint32_t Ordinal = 0;
  uint64_t FirstInstructionOffset = 0;
  uint32_t InstructionCount = 0;
  std::vector<uint32_t> Successors;
};

struct PhysicalMachineInstructionTrace {
  std::string FunctionSymbol;
  uint64_t InstructionOffset = 0;
  uint32_t BlockOrdinal = 0;
  std::string Opcode;
  std::vector<uint8_t> Encoding;
  uint16_t ExplicitDefinitionCount = 0;
  std::vector<PhysicalMachineOperandTrace> Operands;
  std::vector<std::string> ImplicitDefinitions;
  std::vector<std::string> ImplicitUses;
  PhysicalMachineBranchKind BranchKind = PhysicalMachineBranchKind::None;
  uint64_t BranchTarget = 0;
  uint16_t Flags = 0;
  PhysicalMachineMemoryAccess MemoryAccess = PhysicalMachineMemoryAccess::None;
  uint16_t MemoryWidth = 0;
};

struct PhysicalMachineEffectEvidence {
  std::array<uint8_t, 32> ExecutionChallenge{};
  std::array<uint8_t, 32> RequestIdentity{};
  uint64_t RequestBytes = 0;
  std::array<uint8_t, 32> PayloadDigest{};
  uint64_t PayloadBytes = 0;
  std::array<uint8_t, 32> AnalyzerIdentity{};
  std::array<uint8_t, 32> ToolchainIdentity{};
  std::vector<PhysicalMachineEntryEvidence> Entries;
  std::vector<PhysicalMachineFunctionEvidence> Functions;
  std::vector<PhysicalMachineEffect> Effects;
  std::vector<PhysicalMachineBasicBlockTrace> Blocks;
  std::vector<PhysicalMachineInstructionTrace> Instructions;
};

// Closed structural opcode grammars used by analysis and native tests.
uint16_t classifyGfx942GlobalAtomicOpcodeWidth(llvm::StringRef Name);
uint16_t classifyGfx942DsAtomicOpcodeWidth(llvm::StringRef Name);
bool classifyGfx942DsCollectiveOpcode(llvm::StringRef Name);
bool classifyGfx942WorkgroupBarrierOpcode(llvm::StringRef Name);

PhysicalMachineEffectIdentities physicalMachineEffectIdentities();

// Materializes the same LLVM AMDGPU Object/MC runtime used by analysis. The
// authenticated entrypoint calls this before READY so runtime-map custody
// observes the complete analyzer closure rather than lazy first-use mappings.
llvm::Error initializePhysicalMachineEffectRuntime();

llvm::Expected<std::vector<uint8_t>>
encodePhysicalMachineEffectIdentityResponse(llvm::ArrayRef<uint8_t> Request);

bool matchesPhysicalMachineEffectMetadataTargetV1(llvm::StringRef Target);

llvm::Expected<PhysicalMachineEffectRequest>
decodePhysicalMachineEffectRequest(llvm::ArrayRef<uint8_t> Bytes);

// Evidence enumerates reachable static instruction sites in the exact payload.
// It does not prove concrete addresses, OOB absence, race freedom, dynamic
// execution counts, compiler refinement, source properties, or launch safety.
llvm::Expected<PhysicalMachineEffectEvidence>
analyzeGfx942PhysicalMachineEffects(
    const PhysicalMachineEffectRequest &Request);

llvm::Expected<std::vector<uint8_t>> encodePhysicalMachineEffectEvidence(
    const PhysicalMachineEffectEvidence &Evidence);

llvm::Expected<std::vector<uint8_t>> encodePhysicalMachineTraceEvidence(
    const PhysicalMachineEffectEvidence &Evidence,
    llvm::ArrayRef<uint8_t> CanonicalEffectEvidence);

llvm::Expected<std::vector<uint8_t>> encodePhysicalMachineAnalysisBundle(
    const PhysicalMachineEffectEvidence &Evidence);

} // namespace fe2o3::worker

#endif
