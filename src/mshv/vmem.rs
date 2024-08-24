// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use std::{
    cell::RefCell,
    rc::Rc,
};

use crate::{
    elf,
    mshv::partition::VirtualPartition,
    pal::FileMapping,
};
use ::anyhow::Result;
use ::windows::Win32::System::{
    Hypervisor,
    Memory,
};

pub struct VirtualMemory(Rc<RefCell<VirtualPartition>>, *mut std::ffi::c_void, usize);

impl VirtualMemory {
    pub fn new(partition: Rc<RefCell<VirtualPartition>>, size: usize) -> Result<Self> {
        let ptr: *mut std::ffi::c_void = unsafe {
            Memory::VirtualAlloc(
                None,
                size,
                Memory::MEM_COMMIT | Memory::MEM_RESERVE,
                Memory::PAGE_READWRITE,
            )
        };

        trace!("new()");
        unsafe {
            Hypervisor::WHvMapGpaRange(
                partition.borrow().into_raw(),
                ptr,
                0,
                size as u64,
                Hypervisor::WHvMapGpaRangeFlagRead
                    | Hypervisor::WHvMapGpaRangeFlagWrite
                    | Hypervisor::WHvMapGpaRangeFlagExecute,
            )?
        };

        Ok(Self(partition, ptr, size))
    }

    pub fn load(&self, filename: &str) -> Result<u64> {
        trace!("loading ELF file");
        let elf: FileMapping = FileMapping::mmap(filename)?;
        let entry: u64 = unsafe { elf::load(self.1 as *mut ::std::ffi::c_void, elf.ptr())? as u64 };

        Ok(entry)
    }
}

impl Drop for VirtualMemory {
    fn drop(&mut self) {
        unsafe {
            Hypervisor::WHvUnmapGpaRange(self.0.borrow().into_raw(), self.1 as u64, self.2 as u64)
                .unwrap();
            if let Err(e) = Memory::VirtualFree(self.1, 0, Memory::MEM_RELEASE) {
                error!("failed to free memory: {:?}", e);
            }
        }
    }
}
