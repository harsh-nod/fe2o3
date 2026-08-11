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
};

PhysicalMachineEffectIdentities physicalMachineEffectIdentities();

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

} // namespace fe2o3::worker

#endif
