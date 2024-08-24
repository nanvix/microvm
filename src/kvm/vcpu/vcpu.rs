// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use crate::kvm::{
    partition::VirtualPartition,
    vcpu::{
        VirtualProcessorExitContext,
        VirtualProcessorRegister,
    },
};
use ::anyhow::Result;
use ::kvm_bindings::{
    kvm_regs,
    kvm_sregs,
};
use ::kvm_ioctls::{
    VcpuExit,
    VcpuFd,
};
use ::std::{
    cell::RefCell,
    rc::Rc,
};

//==================================================================================================
// Structures
//==================================================================================================

///
/// # Description
///
/// A structure that represents a virtual processor.
///
pub struct VirtualProcessor {
    // Handle to underlying virtual partition.
    _partition: Rc<RefCell<VirtualPartition>>,
    // Handle to underlying virtual processor.
    fd: VcpuFd,
    // Processor state.
    online: bool,
}

impl VirtualProcessor {
    pub fn new(partition: Rc<RefCell<VirtualPartition>>, id: u64) -> Result<Self> {
        trace!("new(): id={}", id);
        crate::timer!("vcpu_creation");
        let fd: VcpuFd = partition.borrow().vm().create_vcpu(id)?;
        Ok(Self {
            _partition: partition,
            fd,
            online: false,
        })
    }

    ///
    /// # Description
    ///
    /// Resets the virtual processor.
    ///
    /// # Parameters
    ///
    /// - `rip`: Value to the the `rip` register.
    /// - `rax`: Value to set the `rax` register.
    /// - `rbx`: Value to set the `rbx` register.
    ///
    /// # Returns
    ///
    /// Upon successful completion, this method returns empty. Otherwise, it returns an error.
    ///
    pub fn reset(&mut self, rip: u64, rax: u64, rbx: u64) -> Result<()> {
        trace!("reset(): rip={:#010x}, rax={:#010x}, rbx={:#010x}", rip, rax, rbx);
        crate::timer!("vcpu_reset");

        // Reset system registers.
        let mut vcpu_sregs: kvm_sregs = self.fd.get_sregs()?;
        vcpu_sregs.cs.base = 0;
        vcpu_sregs.cs.selector = 0;
        self.fd.set_sregs(&vcpu_sregs)?;

        // Reset general purpose registers.
        let mut vcpu_regs: kvm_regs = self.fd.get_regs()?;
        vcpu_regs.rip = rip;
        vcpu_regs.rax = rax;
        vcpu_regs.rbx = rbx;
        vcpu_regs.rflags = 2;
        self.fd.set_regs(&vcpu_regs)?;

        // Processor is now online.
        self.online = true;

        Ok(())
    }

    ///
    /// # Description
    ///
    /// Sets the value of a register.
    ///
    /// # Parameters
    ///
    /// - `register`: Register to set.
    /// - `value`: Value to set.
    ///
    /// # Returns
    ///
    /// Upon successful completion, this method returns empty. Otherwise, it returns an error.
    ///
    pub fn set_register(&mut self, register: VirtualProcessorRegister, value: u64) -> Result<()> {
        crate::timer!("vcpu_set_register");

        // Get current state of registers.
        let mut vcpu_regs: kvm_regs = self.fd.get_regs()?;

        // Set new value of register.
        match register {
            VirtualProcessorRegister::Rax => vcpu_regs.rax = value,
            VirtualProcessorRegister::Rbx => vcpu_regs.rbx = value,
            VirtualProcessorRegister::Rcx => vcpu_regs.rcx = value,
            VirtualProcessorRegister::Rdx => vcpu_regs.rdx = value,
            VirtualProcessorRegister::Rsi => vcpu_regs.rsi = value,
            VirtualProcessorRegister::Rdi => vcpu_regs.rdi = value,
            VirtualProcessorRegister::Rbp => vcpu_regs.rbp = value,
            VirtualProcessorRegister::Rsp => vcpu_regs.rsp = value,
            VirtualProcessorRegister::Rip => vcpu_regs.rip = value,
            VirtualProcessorRegister::Rflags => vcpu_regs.rflags = value,
        }

        // Set new state of registers.
        self.fd.set_regs(&vcpu_regs)?;

        Ok(())
    }

    ///
    /// # Description
    ///
    /// Powers off the virtual processor.
    ///
    pub fn poweroff(&mut self) {
        trace!("poweroff()");
        self.online = false;
    }

    ///
    /// # Description
    ///
    /// Checks if the virtual processor is online.
    ///
    /// # Returns
    ///
    /// If the virtual processor is online, this method returns `true`. Otherwise, it returns
    /// `false` instead.
    pub fn is_online(&self) -> bool {
        self.online
    }

    ///
    /// # Description
    ///
    /// Runs the virtual processor until it exits.
    ///
    /// # Returns
    ///
    /// Upon successful completion, this method returns the context in which the virtual processor
    /// exited. Otherwise, it returns an error.
    ///
    ///
    pub fn run(&mut self) -> Result<VirtualProcessorExitContext> {
        crate::timer!("vcpu_run");
        match self.fd.run()? {
            VcpuExit::IoIn(port, data) => Ok(VirtualProcessorExitContext::PmioIn(port, data.len())),
            VcpuExit::IoOut(port, data) => {
                let mut value: u32 = 0;
                for (i, b) in data.iter().enumerate() {
                    value |= (*b as u32) << (i * 8);
                }
                Ok(VirtualProcessorExitContext::PmioOut(port, value, data.len()))
            },
            _ => Ok(VirtualProcessorExitContext::Unknown),
        }
    }
}
