// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#include "microvm.h"
#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int load_elf32(struct vm *vm, const char *filename, uint32_t *entry);
extern int load_initrd(struct vm *vm, const char *filename);

int main(int argc, char **argv)
{
    struct vm vm = {};
    struct vcpu vcpu;
    bool real_mode = true;
    FILE *vm_stdout = stdout;
    FILE *vm_stdin = stdin;

    size_t memory_size = DEFAULT_MEMORY_SIZE; // Default memory size
    char *kernel_filename = NULL;
    char *initrd_filename = NULL;

    // Parse command-line arguments.
    for (int i = 1; i < argc; i++) {
        /* Kernel image. */
        if (strcmp(argv[i], "-kernel") == 0 && i + 1 < argc) {
            kernel_filename = argv[i + 1];
            i++;
        }
        /* Init RAM Disk. */
        else if (strcmp(argv[i], "-initrd") == 0 && i + 1 < argc) {
            initrd_filename = argv[i + 1];
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
        /* Protected mode. */
        else if (strcmp(argv[i], "-protected") == 0) {
            real_mode = false;
        }
        /* Stdout. */
        else if (strcmp(argv[i], "-stdout") == 0 && i + 1 < argc) {
            vm_stdout = fopen(argv[i + 1], "w");
            if (vm_stdout == NULL) {
                perror("fopen");
                return 1;
            }
            i++;
        }
        /* Stdin. */
        else if (strcmp(argv[i], "-stdin") == 0 && i + 1 < argc) {
            vm_stdin = fopen(argv[i + 1], "r");
            if (vm_stdin == NULL) {
                perror("fopen");
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

    vm_init(&vm, memory_size, vm_stdout, vm_stdin);
    vcpu_init(&vm, &vcpu);
    uint32_t entry = 0;
    if (load_elf32(&vm, kernel_filename, &entry) != 0) {
        exit(1);
    }

    // Load initrd.
    if (initrd_filename != NULL) {
        if (load_initrd(&vm, initrd_filename) != 0) {
            exit(1);
        }
    }

    vm_run(real_mode, &vm, &vcpu, entry);

    uint64_t total_end = rdtsc();

    uint64_t cycles = total_end - total_start;
    printf("%ld cycles, %f us\n", cycles, ((double)cycles / 2.6e9) * 1e6);

    // Cleanup.
    fclose(vm_stdout);
    fclose(vm_stdin);

    return (0);
}
