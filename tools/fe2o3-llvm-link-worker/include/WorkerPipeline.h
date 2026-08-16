#ifndef FE2O3_LLVM_LINK_WORKER_PIPELINE_H
#define FE2O3_LLVM_LINK_WORKER_PIPELINE_H

#include "WorkerDeviceLibraryPolicy.h"
#include "WorkerProtocol.h"

namespace fe2o3::worker {

enum class ExactWorkgroupSyncProfileForTesting { LdsReduction, ScopedAtomic };

llvm::Expected<std::string> exactWorkgroupSyncDataLayoutForTesting();

llvm::Expected<std::vector<std::string>>
inspectLinkedOutputForPublication(llvm::ArrayRef<uint8_t> Bytes,
                                  const Request &RequestValue);

llvm::Error
validateExactLdsGemmSlice1MetadataForTesting(llvm::StringRef MetadataBlob);

llvm::Error validateExactWave64CollectivesV1CompilerInputForTesting(
    llvm::ArrayRef<uint8_t> Bytes);

llvm::Expected<std::vector<uint8_t>>
makeExactWorkgroupSyncCompilerInputForTesting(
    llvm::StringRef CanonicalBody, llvm::ArrayRef<uint8_t> Descriptor,
    ExactWorkgroupSyncProfileForTesting Profile);

llvm::Error validateExactWorkgroupSyncCompilerInputForTesting(
    llvm::ArrayRef<uint8_t> Bytes, ExactWorkgroupSyncProfileForTesting Profile);

llvm::Error validateExactWorkgroupSyncModuleForTesting(
    llvm::StringRef Text, ExactWorkgroupSyncProfileForTesting Profile);

llvm::Error
validateExactMoeTop2V1CompilerInputForTesting(llvm::ArrayRef<uint8_t> Bytes);

llvm::Error validateExactMoeTop2V1ModuleForTesting(llvm::StringRef Text);

llvm::Error validateExactWave64CollectivesV1MetadataForTesting(
    llvm::StringRef MetadataBlob);

llvm::Error validateGenericMetadataForTesting(llvm::StringRef MetadataBlob);

llvm::Error
validateExactLdsGemmSlice1ElfClosureForTesting(llvm::ArrayRef<uint8_t> Bytes);

llvm::Error validateExactWave64CollectivesV1ElfClosureForTesting(
    llvm::ArrayRef<uint8_t> Bytes);

llvm::Error validateExactWorkgroupSyncElfClosureForTesting(
    llvm::ArrayRef<uint8_t> Bytes, ExactWorkgroupSyncProfileForTesting Profile);

llvm::Error
validateExactMoeTop2V1ElfClosureForTesting(llvm::ArrayRef<uint8_t> Bytes);

Response execute(const Request &RequestValue);

Response executeWithUnauthenticatedGfx942DeviceLibraryPolicyForTesting(
    const Request &RequestValue, const Gfx942DeviceLibraryPolicy &Policy);

} // namespace fe2o3::worker

#endif
