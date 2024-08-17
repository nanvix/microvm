// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#ifndef MICROVM_H_
#define MICROVM_H_

#include <stddef.h>
#include <stdint.h>

#define DEFAULT_MEMORY_SIZE (128 * 1024 * 1024)

struct vm {
    int sys_fd;
    int fd;
    char *mem;
};

struct vcpu {
    int fd;
    struct kvm_run *kvm_run;
};

extern int vm_run(struct vm *vm, struct vcpu *vcpu, uint32_t entry);
extern void vcpu_init(struct vm *vm, struct vcpu *vcpu);
extern void vm_init(struct vm *vm, size_t mem_size);

static inline uint64_t rdtsc(void)
{
    uint32_t lo, hi;
    __asm__ __volatile__("rdtsc" : "=a"(lo), "=d"(hi));
    return ((uint64_t)hi << 32) | lo;
}

#endif /* MICROVM_H_ */
