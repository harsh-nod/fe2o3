/* SPDX-License-Identifier: GPL-3.0-or-later
   Exact retained Rust memory locator grammar, not an address capability. */
#ifndef GDB_AMD_DBGAPI_ONE_STOP_LOCATOR_V1_H
#define GDB_AMD_DBGAPI_ONE_STOP_LOCATOR_V1_H
#include <cstddef>
#include <cstdint>
namespace amd_owned_one_stop_v1 {
inline bool fixed_memory_locator (const char *text,std::size_t bytes,
                                  std::uint64_t pid,std::uint64_t &address) noexcept {
  if (!text || bytes<34 || bytes>128 || text[bytes-1]!=0 || !pid) return false;
  std::size_t at=0;
  auto literal=[&] (const char *s) {
    for (std::size_t n=0;s[n];++n) {
      if (at>=bytes-1 || text[at++]!=s[n]) return false;
    }
    return true;
  };
  auto number=[&] (unsigned base,std::uint64_t &out) {
    const auto start=at; out=0;
    while (at<bytes-1) {
      const char c=text[at]; unsigned digit=base;
      if (c>='0' && c<='9') digit=static_cast<unsigned> (c-'0');
      else if (base==16 && c>='a' && c<='f') digit=10+static_cast<unsigned> (c-'a');
      if (digit>=base) break;
      if (out>(UINT64_MAX-digit)/base) return false;
      out=out*base+digit; ++at;
    }
    return at>start && !(at-start>1 && text[start]=='0');
  };
  std::uint64_t actual_pid=0,actual_address=0,actual_size=0;
  if (!literal ("memory://") || !number (10,actual_pid) || actual_pid!=pid
      || !literal ("#offset=0x") || !number (16,actual_address) || !actual_address
      || !literal ("&size=") || !number (10,actual_size) || actual_size!=5312
      || at!=bytes-1 || actual_address>UINT64_MAX-actual_size) return false;
  address=actual_address; return true;
}
}
#endif
