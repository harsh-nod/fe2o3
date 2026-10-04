#ifndef FE2O3_HOST_LLD_SECURE_PROTOCOL_H
#define FE2O3_HOST_LLD_SECURE_PROTOCOL_H

#include <cstddef>

namespace fe2o3::host_lld {

inline constexpr char ProtocolArgument[] = "--fe2o3-host-lld-elf-v2";
inline constexpr char InputPrefix[] = "--fe2o3-input-v1=";
inline constexpr char ResultSocketPrefix[] = "--fe2o3-result-socket-v1=";
inline constexpr char RequestPrefix[] = "--fe2o3-request-v1=";
inline constexpr char ResultRecordPrefix[] = "fe2o3-host-lld-result-v1";
inline constexpr char ResultCopyPolicy[] =
    "receiver-owned-memfd-v1";
inline constexpr int ResultSocketFd = 91;
inline constexpr int FirstInputFd = 100;
inline constexpr std::size_t Sha256HexLength = 64;
inline constexpr unsigned InputSeals = 0x0001U | 0x0002U | 0x0004U | 0x0008U;
inline constexpr unsigned OutputMode = 0555U;

} // namespace fe2o3::host_lld

#endif
