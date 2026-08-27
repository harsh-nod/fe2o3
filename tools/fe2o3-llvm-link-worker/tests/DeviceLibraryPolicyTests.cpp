#include "WorkerDeviceLibraryPolicy.h"

#include "llvm/ADT/StringRef.h"
#include "llvm/Bitcode/BitcodeWriter.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/FileSystem.h"
#include "llvm/Support/Path.h"
#include "llvm/Support/SHA256.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/TargetParser/Triple.h"

#include <array>
#include <cstdlib>
#include <string>
#include <unistd.h>
#include <vector>

using namespace fe2o3::worker;
using namespace llvm;

namespace {

constexpr std::array<StringLiteral, 4> Basenames = {
    "ocml.bc",
    "oclc_isa_version_942.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
};

constexpr std::array<StringLiteral, 9> Gfx950Basenames = {
    "ocml.bc",
    "ockl.bc",
    "oclc_daz_opt_off.bc",
    "oclc_unsafe_math_off.bc",
    "oclc_finite_only_off.bc",
    "oclc_correctly_rounded_sqrt_on.bc",
    "oclc_wavefrontsize64_on.bc",
    "oclc_isa_version_950.bc",
    "oclc_abi_version_600.bc",
};

[[noreturn]] void fail(StringRef Message) {
  errs() << "device-library policy test failed: " << Message << '\n';
  std::abort();
}

void require(bool Condition, StringRef Message) {
  if (!Condition)
    fail(Message);
}

struct TemporaryDirectory {
  SmallString<128> Path;

  TemporaryDirectory() {
    if (std::error_code Error =
            sys::fs::createUniqueDirectory("fe2o3-device-libs", Path))
      fail(Error.message());
  }

  ~TemporaryDirectory() {
    if (std::error_code Error = sys::fs::remove_directories(Path))
      errs() << "could not remove test directory: " << Error.message() << '\n';
  }
};

std::vector<uint8_t> bitcode(StringRef Name) {
  LLVMContext Context;
  Module ModuleValue(Name, Context);
  ModuleValue.setTargetTriple(Triple("amdgcn-amd-amdhsa"));
  SmallVector<char, 0> Buffer;
  raw_svector_ostream Stream(Buffer);
  WriteBitcodeToFile(ModuleValue, Stream);
  return std::vector<uint8_t>(Buffer.begin(), Buffer.end());
}

void writeFile(StringRef Path, ArrayRef<uint8_t> Bytes) {
  std::error_code Error;
  raw_fd_ostream Stream(Path, Error, sys::fs::OF_None);
  if (Error)
    fail(Error.message());
  Stream.write(reinterpret_cast<const char *>(Bytes.data()), Bytes.size());
  Stream.close();
  if (Stream.has_error())
    fail("could not write synthetic device library");
}

std::string path(StringRef Directory, StringRef Basename) {
  SmallString<160> Result(Directory);
  sys::path::append(Result, Basename);
  return Result.str().str();
}

struct Fixture {
  TemporaryDirectory Directory;
  std::array<std::vector<uint8_t>, 4> Bytes;
  Gfx942DeviceLibraryPolicy Policy;

  Fixture() {
    Policy.Directory = Directory.Path.str().str();
    for (size_t I = 0; I < Basenames.size(); ++I) {
      Bytes[I] = bitcode(Basenames[I]);
      writeFile(path(Policy.Directory, Basenames[I]), Bytes[I]);
      Policy.Files.push_back({Basenames[I].str(), SHA256::hash(Bytes[I]),
                              MaxDeviceLibraryFileBytes});
    }
  }
};

void requireFailure(Expected<std::vector<Input>> Result, StringRef Message) {
  require(!Result, "adversarial provider policy was accepted");
  std::string Error = toString(Result.takeError());
  if (!StringRef(Error).contains(Message)) {
    errs() << "unexpected diagnostic: " << Error << '\n';
    fail(Message);
  }
}

void checkConfiguredGfx950Provider() {
  auto Policy = measuredGfx950DeviceLibraryPolicy();
  if (!Policy) {
    std::string Error = toString(Policy.takeError());
    require(StringRef(Error).contains("built without the reviewed gfx950"),
            "disabled gfx950 provider returned an unrelated error");
    return;
  }
  auto Loaded = loadGfx950DeviceLibraries({"__ocml_exp_f32"}, *Policy);
  require(static_cast<bool>(Loaded),
          "configured reviewed gfx950 provider failed to load");
  require(Loaded->size() == Gfx950Basenames.size(),
          "configured gfx950 provider loaded the wrong file count");
  for (size_t I = 0; I < Loaded->size(); ++I) {
    require(Policy->Files[I].Basename == Gfx950Basenames[I],
            "configured gfx950 provider changed canonical file order");
    require((*Loaded)[I].Digest == Policy->Files[I].Digest,
            "configured gfx950 provider changed a reviewed digest");
  }
}

} // namespace

int main() {
  static constexpr std::array<StringLiteral, 7> Supported = {
      "__ocml_cos_f32",   "__ocml_exp2_f32", "__ocml_exp_f32",
      "__ocml_log10_f32", "__ocml_log2_f32", "__ocml_log_f32",
      "__ocml_sin_f32",
  };
  for (StringRef Symbol : Supported) {
    require(isSupportedGfx942OcmlImport(Symbol),
            "supported OCML import is absent from the static map");
    require(isOcmlImportNamespace(Symbol),
            "supported OCML import is outside its namespace");
  }
  require(!isSupportedGfx942OcmlImport("__ocml_sqrt_f32"),
          "unsupported OCML spelling entered the static map");
  require(isOcmlImportNamespace("__ocml_sqrt_f32"),
          "OCML namespace classification failed");
  require(!isOcmlImportNamespace("ordinary_external"),
          "ordinary import was classified as OCML");
  require(isSupportedGfx942OcmlCodeObjectVersion(5),
          "gfx942 OCML rejected code-object V5");
  require(isSupportedGfx942OcmlCodeObjectVersion(6),
          "gfx942 OCML rejected code-object V6");
  require(!isSupportedGfx942OcmlCodeObjectVersion(4),
          "gfx942 OCML accepted code-object V4");
  require(!isSupportedGfx942OcmlCodeObjectVersion(7),
          "gfx942 OCML accepted an unknown code-object version");
  require(isSupportedGfx950OcmlImport("__ocml_exp_f32"),
          "gfx950 OCML exp import is absent from the closed map");
  require(!isSupportedGfx950OcmlImport("__ocml_sin_f32"),
          "gfx950 accepted an OCML import outside the production envelope");
  require(isSupportedGfx950OcmlCodeObjectVersion(6),
          "gfx950 OCML rejected code-object V6");
  require(!isSupportedGfx950OcmlCodeObjectVersion(5),
          "gfx950 OCML accepted code-object V5");

  Gfx950DeviceLibraryPolicy SubstitutedGfx950;
  SubstitutedGfx950.Directory = "/nonexistent";
  for (StringRef Basename : Gfx950Basenames)
    SubstitutedGfx950.Files.push_back(
        {Basename.str(), {}, MaxDeviceLibraryFileBytes});
  requireFailure(
      loadGfx950DeviceLibraries({"__ocml_exp_f32"}, SubstitutedGfx950),
      "digest is not reviewed");
  requireFailure(
      loadGfx950DeviceLibraries({"__ocml_sin_f32"}, SubstitutedGfx950),
      "unsupported gfx950 OCML import");
  checkConfiguredGfx950Provider();

  Fixture Valid;
  auto Loaded = loadGfx942DeviceLibraries({"__ocml_sin_f32"}, Valid.Policy);
  require(static_cast<bool>(Loaded), "exact provider policy failed to load");
  require(Loaded->size() == Basenames.size(),
          "exact provider policy loaded the wrong file count");
  for (size_t I = 0; I < Loaded->size(); ++I) {
    require((*Loaded)[I].Bytes == Valid.Bytes[I],
            "loaded provider bytes changed identity");
    require((*Loaded)[I].Digest == Valid.Policy.Files[I].Digest,
            "loaded provider digest changed identity");
  }

  Gfx942DeviceLibraryPolicy Irrelevant;
  auto Empty = loadGfx942DeviceLibraries({"ordinary_external"}, Irrelevant);
  require(static_cast<bool>(Empty) && Empty->empty(),
          "ordinary imports attempted built-in provider discovery");
  requireFailure(loadGfx942DeviceLibraries({"__ocml_sqrt_f32"}, Valid.Policy),
                 "unsupported gfx942 OCML import");

  Gfx942DeviceLibraryPolicy WrongDigest = Valid.Policy;
  WrongDigest.Files[0].Digest[0] ^= 0xff;
  requireFailure(loadGfx942DeviceLibraries({"__ocml_sin_f32"}, WrongDigest),
                 "digest does not match");

  Gfx942DeviceLibraryPolicy WrongOrder = Valid.Policy;
  std::swap(WrongOrder.Files[0], WrongOrder.Files[1]);
  requireFailure(loadGfx942DeviceLibraries({"__ocml_sin_f32"}, WrongOrder),
                 "noncanonical");

  Fixture Missing;
  require(::unlink(path(Missing.Policy.Directory, Basenames[0]).c_str()) == 0,
          "could not remove adversarial provider");
  requireFailure(loadGfx942DeviceLibraries({"__ocml_sin_f32"}, Missing.Policy),
                 "cannot open pinned");

  Fixture Symlink;
  std::string OcmlPath = path(Symlink.Policy.Directory, Basenames[0]);
  std::string BackingPath = path(Symlink.Policy.Directory, "backing.bc");
  require(::rename(OcmlPath.c_str(), BackingPath.c_str()) == 0,
          "could not stage symlink adversary");
  require(::symlink("backing.bc", OcmlPath.c_str()) == 0,
          "could not create symlink adversary");
  requireFailure(loadGfx942DeviceLibraries({"__ocml_sin_f32"}, Symlink.Policy),
                 "cannot open pinned");

  Fixture HardLink;
  OcmlPath = path(HardLink.Policy.Directory, Basenames[0]);
  std::string AliasPath = path(HardLink.Policy.Directory, "alias.bc");
  require(::link(OcmlPath.c_str(), AliasPath.c_str()) == 0,
          "could not create hard-link adversary");
  requireFailure(loadGfx942DeviceLibraries({"__ocml_sin_f32"}, HardLink.Policy),
                 "violates its file contract");
  return 0;
}
