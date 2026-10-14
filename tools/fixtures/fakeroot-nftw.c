/* Build-only bridge: libc nftw passes internal stat results past fakeroot.
 * Re-read each callback's metadata through its interposed stat/lstat symbols.
 * This library never ships in the Luna image. */
#define _XOPEN_SOURCE 700
#include <dlfcn.h>
#include <errno.h>
#include <ftw.h>
#include <sys/stat.h>

typedef int (*visitor)(const char *, const struct stat *, int, struct FTW *);
typedef int (*walker)(const char *, visitor, int, int);
static _Thread_local visitor active;
static _Thread_local int physical;

static int visit(const char *path, const struct stat *original, int type,
                 struct FTW *entry)
{
    struct stat metadata;
    /* An unreadable entry has no valid metadata; preserve nftw's contract. */
    if (type == FTW_NS)
        return active(path, original, type, entry);
    int result = physical || type == FTW_SL || type == FTW_SLN
        ? lstat(path, &metadata) : stat(path, &metadata);
    if (result != 0)
        return -1;
    return active(path, &metadata, type, entry);
}

int nftw(const char *path, visitor callback, int descriptors, int flags)
{
    walker next = (walker)dlsym(RTLD_NEXT, "nftw");
    if (!next) {
        errno = ENOSYS;
        return -1;
    }
    visitor previous = active;
    int previous_physical = physical;
    active = callback;
    physical = flags & FTW_PHYS;
    int result = next(path, visit, descriptors, flags);
    active = previous;
    physical = previous_physical;
    return result;
}
