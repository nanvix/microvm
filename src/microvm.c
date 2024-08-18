// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

#include <elf.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/kvm.h>
#include <microvm.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

//==================================================================================================
// vm_init()
//==================================================================================================

void vm_init(struct vm *vm, size_t mem_size, FILE *vm_stdout, FILE *vm_stdin)
{
    /* Open KVM endpoint. */
    vm->sys_fd = open("/dev/kvm", O_RDWR);
    if (vm->sys_fd < 0) {
        perror("open /dev/kvm");
        exit(1);
    }

    /* Get API version. */
    const int api_ver = ioctl(vm->sys_fd, KVM_GET_API_VERSION, 0);
    if (api_ver < 0) {
        perror("KVM_GET_API_VERSION");
        exit(1);
    }

    /* Check API version. */
    if (api_ver != KVM_API_VERSION) {
        fprintf(stderr,
                "Got KVM api version %d, expected %d\n",
                api_ver,
                KVM_API_VERSION);
        exit(1);
    }

    vm->fd = ioctl(vm->sys_fd, KVM_CREATE_VM, 0);
    if (vm->fd < 0) {
        perror("KVM_CREATE_VM");
        exit(1);
    }

    vm->vm_stdout = vm_stdout;
    vm->vm_stdin = vm_stdin;

    vm->mem = mmap(NULL,
                   mem_size,
                   PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE,
                   -1,
                   0);
    if (vm->mem == MAP_FAILED) {
        perror("mmap mem");
        exit(1);
    }

    vm->mem_size = mem_size;
    madvise(vm->mem, mem_size, MADV_MERGEABLE);

    struct kvm_userspace_memory_region memreg = {};
    memreg.slot = 0;
    memreg.flags = 0;
    memreg.guest_phys_addr = 0;
    memreg.memory_size = mem_size;
    memreg.userspace_addr = (unsigned long)vm->mem;
    if (ioctl(vm->fd, KVM_SET_USER_MEMORY_REGION, &memreg) < 0) {
        perror("KVM_SET_USER_MEMORY_REGION");
        exit(1);
    }
}

//==================================================================================================
// vcpu_init()
//==================================================================================================

void vcpu_init(struct vm *vm, struct vcpu *vcpu)
{
    int vcpu_mmap_size;

    vcpu->fd = ioctl(vm->fd, KVM_CREATE_VCPU, 0);
    if (vcpu->fd < 0) {
        perror("KVM_CREATE_VCPU");
        exit(1);
    }

    vcpu_mmap_size = ioctl(vm->sys_fd, KVM_GET_VCPU_MMAP_SIZE, 0);
    if (vcpu_mmap_size <= 0) {
        perror("KVM_GET_VCPU_MMAP_SIZE");
        exit(1);
    }

    vcpu->kvm_run = mmap(
        NULL, vcpu_mmap_size, PROT_READ | PROT_WRITE, MAP_SHARED, vcpu->fd, 0);
    if (vcpu->kvm_run == MAP_FAILED) {
        perror("mmap kvm_run");
        exit(1);
    }
}

static void setup_real_mode(struct kvm_sregs *sregs)
{
    sregs->cs.selector = 0;
    sregs->cs.base = 0;
}

static void setup_protected_mode(struct kvm_sregs *sregs)
{
    struct kvm_segment seg = {
        .base = 0,
        .limit = 0xffffffff,
        .selector = 1 << 3,
        .present = 1,
        .type = 11,
        .dpl = 0,
        .db = 1,
        .s = 1,
        .l = 0,
        .g = 1,
    };

    sregs->cr0 |= 1u;

    sregs->cs = seg;

    seg.type = 3;
    seg.selector = 2 << 3;
    sregs->ds = sregs->es = sregs->fs = sregs->gs = sregs->ss = seg;
}

//==================================================================================================
// vm_run()
//==================================================================================================

int vm_run(bool real_mode, struct vm *vm, struct vcpu *vcpu, uint32_t entry)
{
    struct kvm_sregs sregs;
    struct kvm_regs regs;

    if (ioctl(vcpu->fd, KVM_GET_SREGS, &sregs) < 0) {
        perror("KVM_GET_SREGS");
        exit(1);
    }

    if (real_mode) {
        setup_real_mode(&sregs);
    } else {
        setup_protected_mode(&sregs);
    }

    if (ioctl(vcpu->fd, KVM_SET_SREGS, &sregs) < 0) {
        perror("KVM_SET_SREGS");
        exit(1);
    }

    /* Clear all general purpose registers. */
    memset(&regs, 0, sizeof(regs));

    /* Clear all FLAGS bits, except bit 1 which is always set. */
    regs.rflags = 2;
    regs.rip = entry;
    regs.rax = 0x0c00ffee;

    // Encode initrd location and size:
    // - Lower 12 bits encode the size in 4KB pages
    // - Higher bits encode the base address
    regs.rbx = (vm->mmap.initrd_base & 0xfffff000) |
               ((vm->mmap.initrd_size >> 12) & 0xfff);

    if (ioctl(vcpu->fd, KVM_SET_REGS, &regs) < 0) {
        perror("KVM_SET_REGS");
        exit(1);
    }

    for (;;) {
        /* Run the VM. */
        if (ioctl(vcpu->fd, KVM_RUN, 0) < 0) {
            perror("KVM_RUN");
            exit(1);
        }

        /* Handle VM exits. */
        switch (vcpu->kvm_run->exit_reason) {
        case KVM_EXIT_HLT:
            continue;

        /* I/O request. */
        case KVM_EXIT_IO:
            /* Check if I/O is an output. */
            if (vcpu->kvm_run->io.direction == KVM_EXIT_IO_OUT) {
                /* Check if debug command was issued. */
                if (vcpu->kvm_run->io.port == STDOUT_PORT) {
                    char *p =
                        (char *)vcpu->kvm_run + vcpu->kvm_run->io.data_offset;
                    size_t size = vcpu->kvm_run->io.size;
                    uint32_t value;
                    memcpy(&value, p, size);
                    fwrite(p, size, 1, vm->vm_stdout);
                    fflush(vm->vm_stdout);
                }
                /* Check if shutdown command was issued. */
                else if (vcpu->kvm_run->io.port == 0x604) {
                    uint8_t size = vcpu->kvm_run->io.size;
                    uint32_t value = 0;
                    memcpy(&value,
                           (char *)vcpu->kvm_run +
                               vcpu->kvm_run->io.data_offset,
                           size);
                    if (value == 0x2000) {
                        return (0);
                    }
                }
            } else {
                if (vcpu->kvm_run->io.port == STDIN_PORT) {
                    void *p =
                        (char *)vcpu->kvm_run + vcpu->kvm_run->io.data_offset;
                    size_t size = vcpu->kvm_run->io.size;
                    uint32_t value = 0;
                    if (fread(&value, size, 1, vm->vm_stdin) == 0) {
                        if (!feof(stdin)) {
                            perror("failed to read from vm_stdin");
                            exit(1);
                        }
                    }
                    memcpy(p, &value, size);
                }
            }
            break;

        default:
            fprintf(stderr,
                    "Unexpected exit reason %d,",
                    vcpu->kvm_run->exit_reason);
            exit(1);
        }
    }

    return 0;
}
