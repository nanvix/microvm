// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#include "microvm.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern uint32_t load_elf32(char *memory, size_t mem_size, const char *filename);

int main(int argc, char **argv)
{
    struct vm vm;
    struct vcpu vcpu;

    size_t memory_size = DEFAULT_MEMORY_SIZE; // Default memory size
    char *kernel_filename = NULL;

    // Parse command-line arguments.
    for (int i = 1; i < argc; i++) {
        /* Kernel image. */
        if (strcmp(argv[i], "-kernel") == 0 && i + 1 < argc) {
            kernel_filename = argv[i + 1];
            i++;
        }
        /* Memory size. */
        else if (strcmp(argv[i], "-memory") == 0 && i + 1 < argc) {
            char *mem_arg = argv[i + 1];
            char *endptr;
            memory_size = strtoul(mem_arg, &endptr, 10);

            if (*endptr == 'K' || *endptr == 'k') {
                memory_size *= 1024;
            } else if (*endptr == 'M' || *endptr == 'm') {
                memory_size *= 1024 * 1024;
            } else if (*endptr == 'G' || *endptr == 'g') {
                memory_size *= 1024 * 1024 * 1024;
            } else {
                fprintf(stderr, "Invalid memory size suffix: %s\n", mem_arg);
                return 1;
            }

            i++;
        }
    }

    if (kernel_filename == NULL) {
        fprintf(
            stderr, "Usage: %s -kernel <filename> [-mem <size>]\n", argv[0]);
        return 1;
    }

    uint64_t total_start = rdtsc();

    vm_init(&vm, memory_size);
    vcpu_init(&vm, &vcpu);
    uint32_t entry = load_elf32(vm.mem, memory_size, kernel_filename);

    vm_run(&vm, &vcpu, entry);

    uint64_t total_end = rdtsc();

    uint64_t cycles = total_end - total_start;
    printf("%ld cycles, %f us\n", cycles, ((double)cycles / 2.6e9) * 1e6);

    return (0);
}
