// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#include "microvm.h"
#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

int load_initrd(struct vm *vm, const char *filename)
{
    int fd;
    struct stat st;
    char *initrd;

    // Open initrd file.
    if ((fd = open(filename, O_RDONLY)) < 0) {
        perror("open");
        return (-1);
    }

    // Get initrd file size.
    if (fstat(fd, &st) < 0) {
        perror("fstat");
        return (-1);
    }

    // Map initrd file into memory.
    if ((initrd = mmap(NULL, st.st_size, PROT_READ, MAP_PRIVATE, fd, 0)) ==
        MAP_FAILED) {
        perror("mmap");
        return (-1);
    }

    // Check if initrd overlaps with the kernel.
    if (INITRD_BASE >= vm->mmap.kernel_base &&
        INITRD_BASE < (vm->mmap.kernel_base + vm->mmap.kernel_size)) {
        perror("initrd overlaps with the kernel");
        return (-1);
    }

    // Check if if initrd is within the guest memory.
    if ((size_t)(INITRD_BASE + st.st_size) > vm->mem_size) {
        perror("initrd does not fit in guest memory");
        return (-1);
    }

    // Copy initrd file into guest memory.
    memcpy(vm->mem + INITRD_BASE, initrd, st.st_size);

    vm->mmap.initrd_base = INITRD_BASE;
    vm->mmap.initrd_size =
        (st.st_size % PAGE_SIZE)
            ? (st.st_size + PAGE_SIZE - (st.st_size % PAGE_SIZE))
            : st.st_size;

    // Unmap initrd file.
    if (munmap(initrd, st.st_size) < 0) {
        perror("munmap");
        return (-1);
    }

    // Close initrd file.
    if (close(fd) < 0) {
        perror("close");
        return (-1);
    }

    fprintf(vm->vm_stdout,
            "initrd loaded (base=0x%x, size=%lu)\n",
            INITRD_BASE,
            st.st_size);

    return (0);
}
