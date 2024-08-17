// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#include <elf.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

uint32_t load_elf32(char *memory, size_t mem_size, const char *filename)
{
    int fd;
    struct stat st;
    Elf32_Ehdr *ehdr;
    Elf32_Phdr *phdr;
    char *phdr_table;

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
    const uint32_t entry = ehdr->e_entry;

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
        if (phdr->p_vaddr + phdr->p_memsz > mem_size) {
            fprintf(stderr, "segment %zu is out of memory bounds\n", i);
            return (-1);
        }

        /* Copy segment into memory. */
        memcpy(memory + phdr->p_vaddr,
               (char *)ehdr + phdr->p_offset,
               phdr->p_filesz);
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

    return (entry);
}
