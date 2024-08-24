// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::kvm_ioctls::{
    Kvm,
    VmFd,
};

//==================================================================================================
// Structures
//==================================================================================================

///
/// # Description
///
/// A structure that represents a virtual partition.
///
pub struct VirtualPartition {
    // Handle to the KVM.
    _kvm: Kvm,
    // Handle to the virtual machine.
    vm: VmFd,
}

//==================================================================================================
// Implementations
//==================================================================================================

impl VirtualPartition {
    ///
    /// # Description
    ///
    /// Creates a new virtual partition.
    ///
    /// # Returns
    ///
    /// A new virtual partition.
    ///
    pub fn new() -> Result<Self> {
        trace!("new()");
        crate::timer!("partition_creation");
        let kvm: Kvm = Kvm::new()?;
        let vm: VmFd = kvm.create_vm()?;

        Ok(Self { _kvm: kvm, vm })
    }

    ///
    /// # Description
    ///
    /// Gets a handle to the virtual machine.
    ///
    pub fn vm(&self) -> &VmFd {
        &self.vm
    }
}
