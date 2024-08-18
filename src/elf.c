// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#include "microvm.h"
#include <elf.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

int load_elf32(struct vm *vm, const char *filename, uint32_t *entry)
{
    int fd;
    struct stat st;
    Elf32_Ehdr *ehdr;
    Elf32_Phdr *phdr;
    char *phdr_table;
    uint32_t first_address = UINT32_MAX;
    uint32_t last_address = 0;

    // Check if pointer to virtual machine is valid.
    if (vm == NULL) {
        perror("invalid virtual machine pointer\n");
        return (-1);
    }

    /* Check if entry pointer storage location is valid. */
    if (entry == NULL) {
        perror("invalid entry pointer storage location\n");
        return (-1);
    }

    /* Open ELF file. */
    if ((fd = open(filename, O_RDONLY)) < 0) {
        perror("open");
        return (-1);
    }

    /* Get ELF file size. */
    if (fstat(fd, &st) < 0) {
        perror("fstat");
        return (-1);
    }

    /* Map ELF file into memory. */
    if ((ehdr = mmap(NULL, st.st_size, PROT_READ, MAP_PRIVATE, fd, 0)) ==
        MAP_FAILED) {
        perror("mmap");
        return (-1);
    }

    /* Get entry point. */
    *entry = ehdr->e_entry;

    /* Check ELF magic number. */
    if (ehdr->e_ident[EI_MAG0] != ELFMAG0 ||
        ehdr->e_ident[EI_MAG1] != ELFMAG1 ||
        ehdr->e_ident[EI_MAG2] != ELFMAG2 ||
        ehdr->e_ident[EI_MAG3] != ELFMAG3) {
        fprintf(stderr, "not an ELF file\n");
        return (-1);
    }

    /* Check ELF class. */
    if (ehdr->e_ident[EI_CLASS] != ELFCLASS32) {
        fprintf(stderr, "not a 32-bit ELF file\n");
        return (-1);
    }

    /* Check ELF data encoding. */
    if (ehdr->e_ident[EI_DATA] != ELFDATA2LSB) {
        fprintf(stderr, "not a little-endian ELF file\n");
        return (-1);
    }

    /* Check ELF version. */
    if (ehdr->e_ident[EI_VERSION] != EV_CURRENT) {
        fprintf(stderr, "invalid ELF version\n");
        return (-1);
    }

    /* Check ELF type. */
    if (ehdr->e_type != ET_EXEC) {
        fprintf(stderr, "not an executable ELF file\n");
        return (-1);
    }

    /* Check ELF machine architecture. */
    if (ehdr->e_machine != EM_386) {
        fprintf(stderr, "not an x86 ELF file\n");
        return (-1);
    }

    /* Check ELF version. */
    if (ehdr->e_version != EV_CURRENT) {
        fprintf(stderr, "invalid ELF version\n");
        return (-1);
    }

    /* Get program header table. */
    phdr_table = (char *)ehdr + ehdr->e_phoff;

    /* Load program segments. */
    for (size_t i = 0; i < ehdr->e_phnum; i++) {
        phdr = (Elf32_Phdr *)(phdr_table + i * ehdr->e_phentsize);

        /* Check if segment is loadable. */
        if (phdr->p_type != PT_LOAD)
            continue;

        /* Check if segment is within memory bounds. */
        if (phdr->p_vaddr + phdr->p_memsz > vm->mem_size) {
            fprintf(stderr, "segment %zu is out of memory bounds\n", i);
            return (-1);
        }

        /* Copy segment into memory. */
        memcpy(vm->mem + phdr->p_vaddr,
               (char *)ehdr + phdr->p_offset,
               phdr->p_filesz);

        // Update first address.
        if (phdr->p_vaddr < first_address) {
            first_address = phdr->p_vaddr;
        }

        // Update last address.
        uint32_t segment_end = phdr->p_vaddr + phdr->p_memsz;
        if (segment_end > last_address) {
            last_address = segment_end;
        }
    }

    /* Unmap ELF file. */
    if (munmap(ehdr, st.st_size) < 0) {
        perror("munmap");
        return (-1);
    }

    /* Close ELF file. */
    if (close(fd) < 0) {
        perror("close");
        return (-1);
    }

    // Set size of loaded ELF file.
    vm->mmap.kernel_base = first_address;
    vm->mmap.kernel_size = last_address - first_address;

    return (0);
}
