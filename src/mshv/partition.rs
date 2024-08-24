// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::windows::Win32::System::{
    Hypervisor,
    Hypervisor::{
        WHV_PARTITION_HANDLE,
        WHV_PARTITION_PROPERTY,
    },
};

//==================================================================================================
// MshvPartition
//==================================================================================================

pub struct VirtualPartition {
    partition: WHV_PARTITION_HANDLE,
}

impl VirtualPartition {
    pub const STDOUT_PORT: u16 = 0xe9;
    pub const STDIN_PORT: u16 = 0xe9;
    pub const HYPERCALL_PORT: u16 = 0x604;

    pub fn new() -> Result<Self> {
        let ncpus = 1;
        trace!("new(): ncpus={:?}", ncpus);

        let partition: Hypervisor::WHV_PARTITION_HANDLE =
            unsafe { Hypervisor::WHvCreatePartition()? };

        let mut property: Hypervisor::WHV_PARTITION_PROPERTY = WHV_PARTITION_PROPERTY::default();
        property.ProcessorCount = ncpus as u32;

        // Setup partition property.
        unsafe {
            Hypervisor::WHvSetPartitionProperty(
                partition,
                Hypervisor::WHvPartitionPropertyCodeProcessorCount,
                &property as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<WHV_PARTITION_PROPERTY>() as u32,
            )?
        };

        unsafe { Hypervisor::WHvSetupPartition(partition)? };

        Ok(Self { partition })
    }

    pub fn into_raw(&self) -> WHV_PARTITION_HANDLE {
        self.partition
    }
}

impl Drop for VirtualPartition {
    fn drop(&mut self) {
        trace!("delete partition");
        unsafe {
            Hypervisor::WHvDeletePartition(self.partition).unwrap();
        }
    }
}
