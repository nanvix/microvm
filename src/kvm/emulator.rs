// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use crate::{
    kvm::vcpu::{
        VirtualProcessor,
        VirtualProcessorExitContext,
        VirtualProcessorRegister,
    },
    microvm::MicroVm,
};
use ::anyhow::Result;

//==================================================================================================
// Structures
//==================================================================================================

///
/// # Description
///
/// A structure that represents an instruction emulator for the virtual machine.
///
pub struct Emulator {
    /// Input function used for emulating I/O port reads.
    input: Box<dyn FnMut(usize) -> Result<u32>>,
    /// Output function used for emulating I/O port writes.
    output: Box<dyn FnMut(u32, usize) -> Result<()>>,
}

//==================================================================================================
// Implementations
//==================================================================================================

impl Emulator {
    ///
    /// # Description
    ///
    /// Creates a new emulator.
    ///
    /// # Parameters
    ///
    /// - `input`: Input function used for emulating I/O port reads.
    /// - `output`: Output function used for emulating I/O port writes.
    ///
    /// # Returns
    ///
    /// Upon successful completion, this method returns the new emulator. Otherwise, it returns an
    /// error.
    ///
    pub fn new(
        input: Box<dyn FnMut(usize) -> Result<u32>>,
        output: Box<dyn FnMut(u32, usize) -> Result<()>>,
    ) -> Result<Self> {
        trace!("new()");
        Ok(Self { input, output })
    }

    ///
    /// # Description
    ///
    /// Emulates an I/O port access.
    ///
    /// # Parameters
    ///
    /// - `vcpu`: Virtual processor on which the I/O port access occurred.
    /// - `exit_context`: Context in which the I/O port access occurred.
    ///
    /// # Returns
    ///
    /// Upon successful completion, this method returns empty. Otherwise, it returns an error.
    ///
    pub fn handle_pmio_access(
        &mut self,
        vcpu: &mut VirtualProcessor,
        exit_context: VirtualProcessorExitContext,
    ) -> Result<()> {
        // Parse context.
        match exit_context {
            // Read from an I/O port.
            VirtualProcessorExitContext::PmioIn(port, size) => match port {
                // Read from standard input.
                MicroVm::STDIN_PORT => {
                    let data: u32 = (self.input)(size)?;
                    vcpu.set_register(VirtualProcessorRegister::Rdx, data as u64)?;
                },
                // Read from an I/O port that is not supported.
                _ => {
                    let reason: String = format!("read from unsupported port i/o (port={})", port);
                    error!("handle_pmio_access(): {}", reason);
                    anyhow::bail!(reason);
                },
            },
            // Write to an I/O port.
            VirtualProcessorExitContext::PmioOut(port, data, size) => match port {
                // Write to standard output.
                MicroVm::STDOUT_PORT => {
                    (self.output)(data, size)?;
                },
                // Write to the virtual machine monitor port.
                MicroVm::VMM_PORT => {
                    // TODO: check if data matches an expected command.
                    vcpu.poweroff();
                },
                // Write to an I/O port that is not supported.
                _ => {
                    let reason: String = format!("write to unsupported port i/o (port={})", port);
                    error!("handle_pmio_access(): {}", reason);
                    anyhow::bail!(reason);
                },
            },
            // Unexpected I/O port access.
            _ => {
                // This should never happen, as all I/O port accesses are emulated above.
                unreachable!("unexpected i/O port access");
            },
        }

        Ok(())
    }
}
