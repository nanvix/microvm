// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#ifndef MICROVM_H_
#define MICROVM_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#define DEFAULT_MEMORY_SIZE (128 * 1024 * 1024)

// Base address of initrd.
#define INITRD_BASE 0x00800000

// Page size.
#define PAGE_SIZE 4096

#define STDOUT_PORT 0xE9
#define STDIN_PORT 0xE9

struct vm {
    struct {
        uint32_t kernel_base;
        size_t kernel_size;
        uint32_t initrd_base;
        size_t initrd_size;
    } mmap;
    int sys_fd;
    int fd;
    char *mem;
    size_t mem_size;
    FILE *vm_stdout;
    FILE *vm_stdin;
};

struct vcpu {
    int fd;
    struct kvm_run *kvm_run;
};

extern int vm_run(bool real_mode, struct vm *vm, struct vcpu *vcpu,
                  uint32_t entry);
extern void vcpu_init(struct vm *vm, struct vcpu *vcpu);
extern void vm_init(struct vm *vm, size_t mem_size, FILE *vm_stdout,
                    FILE *vm_stdin);

static inline uint64_t rdtsc(void)
{
    uint32_t lo, hi;
    __asm__ __volatile__("rdtsc" : "=a"(lo), "=d"(hi));
    return ((uint64_t)hi << 32) | lo;
}

#endif /* MICROVM_H_ */
