// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use crate::mshv::{
    partition::VirtualPartition,
    vcpu::{
        MshvRegisters,
        VirtualExitProcessorContext,
        VirtualProcessor,
    },
};
use ::anyhow::Result;
use ::std::{
    fmt::{
        self,
        Formatter,
    },
    ptr,
    slice,
};
use ::windows::{
    core::HRESULT,
    Win32::System::{
        Hypervisor,
        Hypervisor::{
            WHV_EMULATOR_STATUS,
            WHV_VP_EXIT_CONTEXT,
        },
    },
};

//==================================================================================================
// Private Structures
//==================================================================================================

struct MshvEmulatorContext<'a> {
    emulator: &'a mut Emulator,
    vcpu: &'a mut VirtualProcessor,
}

impl<'a> MshvEmulatorContext<'a> {
    pub fn new(emulator: &'a mut Emulator, vcpu: &'a mut VirtualProcessor) -> Self {
        Self { emulator, vcpu }
    }

    pub unsafe fn from_raw(context: *const std::ffi::c_void) -> &'a mut Self {
        &mut *(context as *mut Self)
    }

    pub fn as_ptr(&self) -> *const std::ffi::c_void {
        self as *const _ as *const std::ffi::c_void
    }
}

#[derive(PartialEq, Eq)]
enum MshvEmulatorStatus {
    EmulationSuccessful = Self::EMULATION_SUCCESSFUL,
    InternalEmulationFailure = Self::INTERNAL_EMULATION_FAILURE,
    IoPortCallbackFailed = Self::IO_PORT_CALLBACK_FAILED,
    MemoryCallbackFailed = Self::MEMORY_CALLBACK_FAILED,
    TranslateGvaPageCallbackFailed = Self::TRANSLATE_GVA_PAGE_CALLBACK_FAILED,
    TranslateGvaPageCallbackGpaPageIsNotAligned =
        Self::TRANSLATE_GVA_PAGE_CALLBACK_GPA_PAGE_IS_NOT_ALIGNED,
    GetVirtualProcessorRegistersCallbackFailed =
        Self::GET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK_FAILED,
    SetVirtualProcessorRegistersCallbackFailed =
        Self::SET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK_FAILED,
    InterruptCausedIntercept = Self::INTERRUPT_CAUSED_INTERCEPT,
    GuestCannotBeFaulted = Self::GUEST_CANNOT_BE_FAULTED,
    Unknown,
}

impl From<WHV_EMULATOR_STATUS> for MshvEmulatorStatus {
    fn from(status: WHV_EMULATOR_STATUS) -> Self {
        match unsafe { status.AsUINT32 as isize } {
            MshvEmulatorStatus::EMULATION_SUCCESSFUL => MshvEmulatorStatus::EmulationSuccessful,
            MshvEmulatorStatus::INTERNAL_EMULATION_FAILURE => {
                MshvEmulatorStatus::InternalEmulationFailure
            },
            MshvEmulatorStatus::IO_PORT_CALLBACK_FAILED => MshvEmulatorStatus::IoPortCallbackFailed,
            MshvEmulatorStatus::MEMORY_CALLBACK_FAILED => MshvEmulatorStatus::MemoryCallbackFailed,
            MshvEmulatorStatus::TRANSLATE_GVA_PAGE_CALLBACK_FAILED => {
                MshvEmulatorStatus::TranslateGvaPageCallbackFailed
            },
            MshvEmulatorStatus::TRANSLATE_GVA_PAGE_CALLBACK_GPA_PAGE_IS_NOT_ALIGNED => {
                MshvEmulatorStatus::TranslateGvaPageCallbackGpaPageIsNotAligned
            },
            MshvEmulatorStatus::GET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK_FAILED => {
                MshvEmulatorStatus::GetVirtualProcessorRegistersCallbackFailed
            },
            MshvEmulatorStatus::SET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK_FAILED => {
                MshvEmulatorStatus::SetVirtualProcessorRegistersCallbackFailed
            },
            MshvEmulatorStatus::INTERRUPT_CAUSED_INTERCEPT => {
                MshvEmulatorStatus::InterruptCausedIntercept
            },
            MshvEmulatorStatus::GUEST_CANNOT_BE_FAULTED => MshvEmulatorStatus::GuestCannotBeFaulted,
            _ => MshvEmulatorStatus::Unknown,
        }
    }
}

impl MshvEmulatorStatus {
    const EMULATION_SUCCESSFUL: isize = 1 << 0;
    const INTERNAL_EMULATION_FAILURE: isize = 1 << 1;
    const IO_PORT_CALLBACK_FAILED: isize = 1 << 2;
    const MEMORY_CALLBACK_FAILED: isize = 1 << 3;
    const TRANSLATE_GVA_PAGE_CALLBACK_FAILED: isize = 1 << 4;
    const TRANSLATE_GVA_PAGE_CALLBACK_GPA_PAGE_IS_NOT_ALIGNED: isize = 1 << 5;
    const GET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK_FAILED: isize = 1 << 6;
    const SET_VIRTUAL_PROCESSOR_REGISTERS_CALLBACK_FAILED: isize = 1 << 7;
    const INTERRUPT_CAUSED_INTERCEPT: isize = 1 << 8;
    const GUEST_CANNOT_BE_FAULTED: isize = 1 << 9;

    fn to_human_readable(&self) -> &str {
        match self {
            MshvEmulatorStatus::EmulationSuccessful => "emulation successful",
            MshvEmulatorStatus::InternalEmulationFailure => "internal emulation failure",
            MshvEmulatorStatus::IoPortCallbackFailed => "io port callback failed",
            MshvEmulatorStatus::MemoryCallbackFailed => "memory callback failed",
            MshvEmulatorStatus::TranslateGvaPageCallbackFailed => {
                "translate gva page callback failed"
            },
            MshvEmulatorStatus::TranslateGvaPageCallbackGpaPageIsNotAligned => {
                "translate gva page callback gpa page is not aligned"
            },
            MshvEmulatorStatus::GetVirtualProcessorRegistersCallbackFailed => {
                "get virtual processor registers callback failed"
            },
            MshvEmulatorStatus::SetVirtualProcessorRegistersCallbackFailed => {
                "set virtual processor registers callback failed"
            },
            MshvEmulatorStatus::InterruptCausedIntercept => "interrupt caused intercept",
            MshvEmulatorStatus::GuestCannotBeFaulted => "guest cannot be faulted",
            MshvEmulatorStatus::Unknown => "unknown",
        }
    }
}

impl fmt::Display for MshvEmulatorStatus {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.to_human_readable())
    }
}

//==================================================================================================
// Public Structures
//==================================================================================================

pub struct Emulator {
    handle: *mut std::ffi::c_void,
    input: Box<dyn FnMut(u32) -> Result<()>>,
    output: Box<dyn FnMut(u32) -> Result<()>>,
}

impl Emulator {
    pub fn new(
        input: Box<dyn FnMut(u32) -> Result<()>>,
        output: Box<dyn FnMut(u32) -> Result<()>>,
    ) -> Result<Self> {
        let mut handle: *mut std::ffi::c_void = ptr::null_mut();
        unsafe { Hypervisor::WHvEmulatorCreateEmulator(&CALLBACKS, &mut handle)? };

        Ok(Self {
            handle,
            input,
            output,
        })
    }

    pub fn handle_pmio_access(
        &mut self,
        vcpu: &mut VirtualProcessor,
        exit_context: VirtualExitProcessorContext,
    ) -> Result<()> {
        // let context = partition as *const _ as *const std::ffi::c_void;
        let handle: *mut std::ffi::c_void = self.handle;
        let context = MshvEmulatorContext::new(self, vcpu);
        unsafe {
            let status: WHV_EMULATOR_STATUS = Hypervisor::WHvEmulatorTryIoEmulation(
                handle,
                context.as_ptr(),
                exit_context.vp_context() as *const WHV_VP_EXIT_CONTEXT,
                exit_context.pmio_context(),
            )?;

            if MshvEmulatorStatus::from(status) != MshvEmulatorStatus::EmulationSuccessful {
                return Err(anyhow::anyhow!(
                    "failed to emulate pmio access (status={})",
                    MshvEmulatorStatus::from(status)
                ));
            }
        }

        Ok(())
    }
}

//==================================================================================================
// Standalone Functions
//==================================================================================================

extern "system" fn io_port_emulator(
    context: *const std::ffi::c_void,
    ioaccess: *mut Hypervisor::WHV_EMULATOR_IO_ACCESS_INFO,
) -> HRESULT {
    unsafe {
        let context: &mut MshvEmulatorContext = MshvEmulatorContext::from_raw(context);
        let emulator: &mut Emulator = context.emulator;
        let vcpu: &mut VirtualProcessor = context.vcpu;
        let port = (*ioaccess).Port;
        let size = (*ioaccess).AccessSize;
        let direction = (*ioaccess).Direction;

        match direction {
            0 => match port {
                VirtualPartition::STDIN_PORT => {
                    if let Err(_) = (emulator.input)(size as u32) {
                        return HRESULT(1);
                    }
                },
                _ => {
                    return HRESULT(1);
                },
            },
            1 => match port {
                VirtualPartition::STDOUT_PORT => {
                    if let Err(_) = (emulator.output)((*ioaccess).Data) {
                        return HRESULT(1);
                    }
                },
                VirtualPartition::HYPERCALL_PORT => {
                    vcpu.poweroff();
                },
                _ => {
                    return HRESULT(1);
                },
            },
            _ => {
                return HRESULT(1);
            },
        }
    }

    HRESULT(0)
}

#[allow(unused)]
extern "system" fn mmio_emulator(
    context: *const std::ffi::c_void,
    mmioaccess: *mut Hypervisor::WHV_EMULATOR_MEMORY_ACCESS_INFO,
) -> HRESULT {
    // TODO: implement this functionality, if required.

    HRESULT(1)
}

#[allow(unused)]
extern "system" fn get_virtual_processor_registers(
    context: *const std::ffi::c_void,
    names: *const Hypervisor::WHV_REGISTER_NAME,
    name_count: u32,
    values: *mut Hypervisor::WHV_REGISTER_VALUE,
) -> HRESULT {
    // TODO: implement this functionality, if required.

    HRESULT(1)
}

extern "system" fn set_virtual_processor_registers(
    context: *const std::ffi::c_void,
    names: *const Hypervisor::WHV_REGISTER_NAME,
    name_count: u32,
    values: *const Hypervisor::WHV_REGISTER_VALUE,
) -> HRESULT {
    // Set registers.
    unsafe {
        let context: &mut MshvEmulatorContext = MshvEmulatorContext::from_raw(context);
        let vcpu: &mut VirtualProcessor = context.vcpu;
        // let vcpu: &MshvVirtualProcessor = &*(context as *const MshvVirtualProcessor);
        let registers = MshvRegisters {
            names: slice::from_raw_parts(names, name_count as usize),
            values: slice::from_raw_parts(values, name_count as usize),
        };
        vcpu.set_registers(&registers).unwrap();
    };

    HRESULT(0)
}

#[allow(unused)]
extern "system" fn translate_gva_page(
    context: *const std::ffi::c_void,
    gva: u64,
    translateflags: Hypervisor::WHV_TRANSLATE_GVA_FLAGS,
    translationresult: *mut Hypervisor::WHV_TRANSLATE_GVA_RESULT_CODE,
    gpa: *mut u64,
) -> HRESULT {
    HRESULT(0)
}

const CALLBACKS: Hypervisor::WHV_EMULATOR_CALLBACKS = Hypervisor::WHV_EMULATOR_CALLBACKS {
    Size: std::mem::size_of::<Hypervisor::WHV_EMULATOR_CALLBACKS>() as u32,
    Reserved: 0,
    WHvEmulatorIoPortCallback: Some(io_port_emulator),
    WHvEmulatorMemoryCallback: Some(mmio_emulator),
    WHvEmulatorGetVirtualProcessorRegisters: Some(get_virtual_processor_registers),
    WHvEmulatorSetVirtualProcessorRegisters: Some(set_virtual_processor_registers),
    WHvEmulatorTranslateGvaPage: Some(translate_gva_page),
};
