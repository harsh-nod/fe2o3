#include <cerrno>
#include <climits>
#include <cstdlib>
#include <dlfcn.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

namespace {

const char *rewriteTmp(const char *path, char (&storage)[PATH_MAX]) {
  if (path == nullptr || path[0] != '/' || path[1] != 't' || path[2] != 'm' ||
      path[3] != 'p' || (path[4] != '\0' && path[4] != '/'))
    return path;

  const char *root = ::getenv("FE2O3_PRIVATE_TMP");
  if (root == nullptr || root[0] != '/') {
    errno = EACCES;
    return nullptr;
  }

  size_t rootLength = 0;
  while (root[rootLength] != '\0')
    ++rootLength;
  size_t common = 0;
  while (common < rootLength && path[common] == root[common])
    ++common;
  if (common == rootLength && (path[common] == '\0' || path[common] == '/'))
    return path;

  size_t length = 0;
  while (root[length] != '\0') {
    if (length + 1 >= sizeof(storage)) {
      errno = ENAMETOOLONG;
      return nullptr;
    }
    storage[length] = root[length];
    ++length;
  }
  for (size_t index = 4; path[index] != '\0'; ++index) {
    if (length + 1 >= sizeof(storage)) {
      errno = ENAMETOOLONG;
      return nullptr;
    }
    storage[length++] = path[index];
  }
  storage[length] = '\0';
  return storage;
}

template <typename Function> Function resolveNext(const char *name) {
  void *symbol = ::dlsym(RTLD_NEXT, name);
  if (symbol == nullptr) {
    errno = EACCES;
    return nullptr;
  }
  static_assert(sizeof(Function) == sizeof(symbol));
  Function function = nullptr;
  __builtin_memcpy(&function, &symbol, sizeof(function));
  return function;
}

} // namespace

#define FE2O3_EXPORT extern "C" __attribute__((visibility("default")))

FE2O3_EXPORT ssize_t readlink(const char *path, char *buffer, size_t size) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return -1;
  return static_cast<ssize_t>(
      ::syscall(SYS_readlinkat, AT_FDCWD, effective, buffer, size));
}

FE2O3_EXPORT ssize_t readlinkat(int directoryFd, const char *path, char *buffer,
                                size_t size) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return -1;
  if (effective != path)
    directoryFd = AT_FDCWD;
  return static_cast<ssize_t>(
      ::syscall(SYS_readlinkat, directoryFd, effective, buffer, size));
}

FE2O3_EXPORT int stat(const char *path, struct stat *status) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return -1;
  return static_cast<int>(
      ::syscall(SYS_newfstatat, AT_FDCWD, effective, status, 0));
}

FE2O3_EXPORT int lstat(const char *path, struct stat *status) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return -1;
  return static_cast<int>(::syscall(SYS_newfstatat, AT_FDCWD, effective, status,
                                    AT_SYMLINK_NOFOLLOW));
}

FE2O3_EXPORT int fstatat(int directoryFd, const char *path, struct stat *status,
                         int flags) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return -1;
  if (effective != path)
    directoryFd = AT_FDCWD;
  return static_cast<int>(
      ::syscall(SYS_newfstatat, directoryFd, effective, status, flags));
}

FE2O3_EXPORT char *realpath(const char *path, char *resolved) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return nullptr;
  using Realpath = char *(*)(const char *, char *);
  const Realpath next = resolveNext<Realpath>("realpath");
  return next == nullptr ? nullptr : next(effective, resolved);
}

FE2O3_EXPORT char *__realpath_chk(const char *path, char *resolved,
                                  size_t resolvedLength) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return nullptr;
  using CheckedRealpath = char *(*)(const char *, char *, size_t);
  const CheckedRealpath next = resolveNext<CheckedRealpath>("__realpath_chk");
  return next == nullptr ? nullptr : next(effective, resolved, resolvedLength);
}

FE2O3_EXPORT char *canonicalize_file_name(const char *path) {
  char rewritten[PATH_MAX];
  const char *effective = rewriteTmp(path, rewritten);
  if (effective == nullptr)
    return nullptr;
  using Canonicalize = char *(*)(const char *);
  const Canonicalize next = resolveNext<Canonicalize>("canonicalize_file_name");
  return next == nullptr ? nullptr : next(effective);
}
