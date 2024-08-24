// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::std::{
    ptr,
    usize,
};
use ::windows::{
    core::HSTRING,
    Win32::{
        Foundation,
        Foundation::HANDLE,
        Storage::{
            FileSystem,
            FileSystem::{
                FILE_ATTRIBUTE_NORMAL,
                FILE_FLAG_SEQUENTIAL_SCAN,
                FILE_SHARE_READ,
                OPEN_EXISTING,
            },
        },
        System::{
            Memory,
            Memory::{
                FILE_MAP_READ,
                MEMORY_MAPPED_VIEW_ADDRESS,
                PAGE_READONLY,
            },
        },
    },
};

//==================================================================================================
// Structures
//==================================================================================================

pub struct FileMapping {
    fd: HANDLE,
    file_mapping: HANDLE,
    file_view: MEMORY_MAPPED_VIEW_ADDRESS,
}

impl FileMapping {
    pub fn mmap(filename: &str) -> Result<Self> {
        trace!("opening file");
        let lp_file_name = &HSTRING::from(filename);

        // Open the file.
        let fd: HANDLE = unsafe {
            FileSystem::CreateFileW(
                lp_file_name,
                Foundation::GENERIC_READ.0,
                FILE_SHARE_READ,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_SEQUENTIAL_SCAN,
                HANDLE(ptr::null_mut()),
            )?
        };

        trace!("getting file size");
        // Get file size.
        let file_size = unsafe {
            let mut file_size = 0;
            FileSystem::GetFileSizeEx(fd, &mut file_size)?;
            file_size
        };

        trace!("file size: {}", file_size);

        trace!("mapping file");
        // Map the file.
        let file_mapping: HANDLE =
            unsafe { Memory::CreateFileMappingW(fd, None, PAGE_READONLY, 0, 0, None)? };

        trace!("viewing file {:?}", file_mapping);

        // Map file
        let file_view =
            unsafe { Memory::MapViewOfFile(file_mapping, FILE_MAP_READ, 0, 0, file_size as usize) };

        Ok(Self {
            fd,
            file_mapping,
            file_view,
        })
    }

    pub fn ptr(&self) -> *const u8 {
        self.file_view.Value as *const u8
    }
}

impl Drop for FileMapping {
    fn drop(&mut self) {
        unsafe {
            trace!("unmapping file");
            if let Err(e) = Memory::UnmapViewOfFile(self.file_view) {
                warn!("failed to unmap view of file (error={:?})", e);
            }

            if let Err(e) = Foundation::CloseHandle(self.file_mapping) {
                warn!("failed to close file mapping (error={:?})", e);
            }

            if let Err(e) = Foundation::CloseHandle(self.fd) {
                warn!("failed to close file (error={:?})", e);
            }
        }
    }
}
