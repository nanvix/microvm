// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

use crate::mshv::partition::VirtualPartition;
use ::anyhow::Result;
use ::windows::Win32::System::{
    Hypervisor,
    Hypervisor::{
        WHV_PARTITION_HANDLE,
        WHV_RUN_VP_EXIT_CONTEXT,
    },
};
use std::{
    cell::RefCell,
    mem,
    rc::Rc,
};

pub enum VirtualProcessorExitReason {
    PmioAccess,
    Unknown,
}

#[derive(Default)]
pub struct VirtualExitProcessorContext {
    context: WHV_RUN_VP_EXIT_CONTEXT,
}

impl VirtualExitProcessorContext {
    pub fn vp_context(&self) -> &Hypervisor::WHV_VP_EXIT_CONTEXT {
        &self.context.VpContext
    }

    pub fn pmio_context(&self) -> &Hypervisor::WHV_X64_IO_PORT_ACCESS_CONTEXT {
        unsafe { &self.context.Anonymous.IoPortAccess }
    }

    pub fn reason(&self) -> VirtualProcessorExitReason {
        match self.context.ExitReason {
            Hypervisor::WHvRunVpExitReasonX64IoPortAccess => VirtualProcessorExitReason::PmioAccess,
            _ => VirtualProcessorExitReason::Unknown,
        }
    }
}

pub struct MshvRegisters<'a> {
    pub names: &'a [Hypervisor::WHV_REGISTER_NAME],
    pub values: &'a [Hypervisor::WHV_REGISTER_VALUE],
}

//==================================================================================================
// MshvVirtualProcessor
//==================================================================================================

pub struct VirtualProcessor(Rc<RefCell<VirtualPartition>>, u32, bool);

impl VirtualProcessor {
    pub fn new(partition: Rc<RefCell<VirtualPartition>>, index: u32) -> Result<Self> {
        trace!("new(): index={:?}", index);
        let p = partition.borrow().into_raw();
        unsafe { Hypervisor::WHvCreateVirtualProcessor(p, index, 0)? };

        Ok(Self(partition, index, true))
    }

    pub fn is_online(&self) -> bool {
        self.2
    }

    pub fn poweroff(&mut self) {
        trace!("poweroff()");
        self.2 = false;
    }

    pub fn reset(&self, entry: u64) -> Result<()> {
        trace!("reset(): entry={:#010x}", entry);

        const REGISTERS_COUNT: usize = 10;

        // Reset registers.
        let mut names: [Hypervisor::WHV_REGISTER_NAME; REGISTERS_COUNT] =
            [Hypervisor::WHV_REGISTER_NAME::default(); REGISTERS_COUNT];

        let mut values: [Hypervisor::WHV_REGISTER_VALUE; REGISTERS_COUNT] =
            [Hypervisor::WHV_REGISTER_VALUE::default(); REGISTERS_COUNT];

        let mut idx: usize = 0;

        names[idx] = Hypervisor::WHvX64RegisterRax;
        values[idx].Reg64 = 0x0c0ffee;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRcx;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRdx;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRbx;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRsp;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRbp;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRsi;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRdi;
        values[idx].Reg64 = 0;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRip;
        values[idx].Reg64 = entry;
        idx += 1;

        names[idx] = Hypervisor::WHvX64RegisterRflags;
        values[idx].Reg64 = 0x2;
        idx += 1;

        assert_eq!(idx, names.len());

        let registers: MshvRegisters = MshvRegisters {
            names: &names,
            values: &values,
        };

        // Set registers.
        self.set_registers(&registers)?;

        Ok(())
    }

    pub fn set_registers<'a>(&self, registers: &'a MshvRegisters) -> Result<()> {
        // Set registers.
        unsafe {
            let p: WHV_PARTITION_HANDLE = self.0.borrow().into_raw();
            Hypervisor::WHvSetVirtualProcessorRegisters(
                p,
                0,
                registers.names.as_ptr(),
                registers.names.len() as u32,
                registers.values.as_ptr(),
            )?
        };

        Ok(())
    }

    pub fn run(&self) -> Result<VirtualExitProcessorContext> {
        // Run virtual processor.
        let mut exit_context = VirtualExitProcessorContext::default();

        unsafe {
            let p: WHV_PARTITION_HANDLE = self.0.borrow().into_raw();
            Hypervisor::WHvRunVirtualProcessor(
                p,
                0,
                &mut exit_context.context as *mut _ as *mut std::ffi::c_void,
                mem::size_of::<WHV_RUN_VP_EXIT_CONTEXT>() as u32,
            )?
        };

        Ok(exit_context)
    }
}

impl Drop for VirtualProcessor {
    fn drop(&mut self) {
        unsafe {
            let p: WHV_PARTITION_HANDLE = self.0.borrow().into_raw();
            Hypervisor::WHvDeleteVirtualProcessor(p, self.1).unwrap();
        }
    }
}
