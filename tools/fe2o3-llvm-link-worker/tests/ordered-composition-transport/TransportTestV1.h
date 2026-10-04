#ifndef FE2O3_PRIVATE_COMPOSITION_TRANSPORT_TEST_V1_H
#define FE2O3_PRIVATE_COMPOSITION_TRANSPORT_TEST_V1_H
// Test-only diagnostic interface. No serialized protocol or source/execution authority.
#include "WorkerMachineEffect.h"
#include <array>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <optional>
#include <utility>
namespace fe2o3::worker::transport_test_v1 {
inline constexpr uint64_t StorageBytes = 65536;
inline constexpr uint64_t WorkUnits = 280576;
enum class Profile : uint8_t { ConditionalHelperO0 = 1, InlineRootO3 = 2, CallerToHelperO0 = 3 };
enum class Program : uint8_t { XorAnd = 1, MoveInput2 = 2 };
// These are inert observations emitted by the distinct real-source role mode.
// The caller must retain/recheck the exact fresh publication, normal LLVM,
// handoff and descriptor joins. This struct cannot reconstruct that owner.
struct SourceProjection {
  std::array<std::array<uint32_t,4>,64> Rows{};
  uint16_t RowCount = 0;
  llvm::StringRef Root;
  llvm::StringRef Helper;
  std::array<uint8_t,32> Canonical{}, Llvm{}, SourceReport{}, Descriptor{};
  Profile Selected = Profile::ConditionalHelperO0;
  Program SelectedProgram = Program::XorAnd;
};
enum class Refusal : uint8_t {
  None, ResourceStorage, ResourceWork, SourceProjection, Table, Register,
  Shape, Encoding, Cfg, Undefined, Pending, Alias, Domain, Descriptor,
  Roster, Reentry, Aggregate
};
// A separate bounded transport domain, not LLVM/worker RSS. No reset/refund of
// work and no public reservation operation. The account outlives every result.
class Account {
  uint64_t StorageLimit, WorkLimit, Storage, Peak, Work, Denials = 0;
  Refusal LastDenial = Refusal::None;
  friend class Reservation;
public:
  Account(uint64_t StorageLimit, uint64_t WorkLimit,
          uint64_t ExistingStorage = 0, uint64_t ExistingWork = 0)
      : StorageLimit(StorageLimit), WorkLimit(WorkLimit),
        Storage(ExistingStorage), Peak(ExistingStorage), Work(ExistingWork) {}
  Account(const Account &) = delete;
  Account &operator=(const Account &) = delete;
  uint64_t storage() const { return Storage; }
  uint64_t peak() const { return Peak; }
  uint64_t work() const { return Work; }
  uint64_t denials() const { return Denials; }
  Refusal last_denial() const { return LastDenial; }
};
class Reservation {
  Account *Owner = nullptr;
  explicit Reservation(Account &A) : Owner(&A) {}
public:
  Reservation() = delete;
  Reservation(const Reservation &) = delete;
  Reservation &operator=(const Reservation &) = delete;
  Reservation(Reservation &&Other) noexcept
      : Owner(std::exchange(Other.Owner,nullptr)) {}
  Reservation &operator=(Reservation &&) = delete;
  ~Reservation() { if (Owner) Owner->Storage -= StorageBytes; }
  // No native capability: this only prepays the complete logical domain.
  static std::optional<Reservation> acquire(Account &A);
};
struct Summary {
  Profile Selected{};
  Program SelectedProgram{};
  uint16_t CachedRegisters = 0, WholeWords = 0, Whole32Classes = 0;
  uint16_t Functions = 0, Blocks = 0, Instructions = 0;
  uint64_t SelectedOffset = 0, SelectedBytes = 0, DataUseOffset = 0;
  uint64_t ProgramOffset = 0;
  // Only CallerToHelperO0 fills these exact decoded static-call coordinates.
  uint64_t CallerOffset = 0, CallOffset = 0, CallTargetOffset = 0;
  std::array<uint8_t,32> Canonical{}, Llvm{}, SourceReport{}, Descriptor{}, NativeDescriptor{};
  // Scope is encoded by Selected, not by a "full proof" flag. O0 is conditional
  // on ordinary C ABI inputs on entry EXEC. Profile 3 derives those inputs from
  // the fixed caller prefix; it does not cover the post-call caller or store.
  // O3 covers DATA, never its address.
};
// Cannot be default-constructed or copied; no summary survives a failed
// aggregate analysis. The reservation remains live through output consumption.
class Result {
  PhysicalMachineEffectEvidence Effects;
  Summary Observation;
  Reservation Storage;
  friend llvm::Expected<Result> analyze(const PhysicalMachineEffectRequest &,
      const SourceProjection &, Account &);
  Result(PhysicalMachineEffectEvidence &&E, Summary S, Reservation &&R)
      : Effects(std::move(E)), Observation(S), Storage(std::move(R)) {}
public:
  Result(const Result &) = delete;
  Result &operator=(const Result &) = delete;
  Result(Result &&) = default;
  Result &operator=(Result &&) = delete;
  const PhysicalMachineEffectEvidence &effects() const { return Effects; }
  const Summary &summary() const { return Observation; }
};
// Only compiled by the separate private test observer, never the normal worker.
// It does not weaken any existing effect/trace/loader/aggregate check.
llvm::Expected<Result> analyze(
    const PhysicalMachineEffectRequest &, const SourceProjection &, Account &);
} // namespace fe2o3::worker::transport_test_v1
#endif
