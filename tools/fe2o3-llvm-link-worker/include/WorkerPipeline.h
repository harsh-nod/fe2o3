#ifndef FE2O3_LLVM_LINK_WORKER_PIPELINE_H
#define FE2O3_LLVM_LINK_WORKER_PIPELINE_H

#include "WorkerDeviceLibraryPolicy.h"
#include "WorkerProtocol.h"

namespace fe2o3::worker {

llvm::Expected<std::string> exactProducerDataLayoutForTesting();

llvm::Expected<std::vector<std::string>>
inspectLinkedOutputForPublication(llvm::ArrayRef<uint8_t> Bytes,
                                  const Request &RequestValue);

llvm::Error
validateExactLdsGemmSlice1MetadataForTesting(llvm::StringRef MetadataBlob);

llvm::Error validateExactFlashAttentionV1LlvmBuildIdentityForTesting(
    llvm::StringRef Identity);

llvm::Expected<std::vector<uint8_t>>
makeExactRowSoftmaxV1CompilerInputForTesting(
    llvm::StringRef CanonicalBody, llvm::ArrayRef<uint8_t> Descriptor,
    llvm::ArrayRef<uint8_t> Transcript);

llvm::Error
validateExactRowSoftmaxV1CompilerInputForTesting(llvm::ArrayRef<uint8_t> Bytes);

llvm::Error
validateExactRowSoftmaxV1MetadataForTesting(llvm::StringRef MetadataBlob);

llvm::Error validateProductionGeneralGemmV1CompilerInputForTesting(
    llvm::ArrayRef<uint8_t> Bytes);

llvm::Error
validateProductionGeneralGemmV1MetadataForTesting(llvm::StringRef MetadataBlob);

llvm::Error
validateExactMoeTop2V1CompilerInputForTesting(llvm::ArrayRef<uint8_t> Bytes);

llvm::Error validateExactMoeTop2V1ModuleForTesting(llvm::StringRef Text);

llvm::Error validateGenericMetadataForTesting(llvm::StringRef MetadataBlob);

llvm::Error
validateExactLdsGemmSlice1ElfClosureForTesting(llvm::ArrayRef<uint8_t> Bytes);

llvm::Error
validateExactRowSoftmaxV1ElfClosureForTesting(llvm::ArrayRef<uint8_t> Bytes);

llvm::Error
validateExactMoeTop2V1ElfClosureForTesting(llvm::ArrayRef<uint8_t> Bytes);

Response execute(const Request &RequestValue);

Response executeWithUnauthenticatedGfx942DeviceLibraryPolicyForTesting(
    const Request &RequestValue, const Gfx942DeviceLibraryPolicy &Policy);

Response executeWithUnauthenticatedGfx950DeviceLibraryPolicyForTesting(
    const Request &RequestValue, const Gfx950DeviceLibraryPolicy &Policy);

} // namespace fe2o3::worker

#endif
