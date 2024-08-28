// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//!
//! # MicroVM
//!
//! MicroVM is a ultra-lightweight virtual machine that is designed to run the
//! [Nanvix](https://github.com/nanvix/) operating system. Currently Linux KVM is supported as
//! backend.
//!

//==================================================================================================
// Configuration
//==================================================================================================

#![deny(clippy::all)]

//==================================================================================================
// External Crates
//==================================================================================================

//==================================================================================================
// Macros
//==================================================================================================

/// Use this macro to add the current scope to profiling.
#[allow(unused)]
#[macro_export]
macro_rules! timer {
    ($name:expr) => {
        #[cfg(feature = "profiler")]
        let _guard = $crate::profiler::PROFILER.with(|p| p.borrow_mut().sync_scope($name));
    };
}

//==================================================================================================
// Modules
//==================================================================================================

mod args;
mod config;
mod elf;
mod file;
mod logging;
mod microvm;
mod pal;

#[cfg(feature = "profiler")]
mod profiler;

#[cfg(target_os = "linux")]
mod kvm;

//==================================================================================================
// Imports
//==================================================================================================

// Must come first.
#[macro_use]
extern crate log;

#[cfg(target_os = "linux")]
extern crate kvm_bindings;
#[cfg(target_os = "linux")]
extern crate kvm_ioctls;

use crate::{
    args::Args,
    microvm::MicroVm,
};
use ::anyhow::Result;
use ::std::{
    env,
    fs::File,
    io::Write,
    sync::mpsc,
    thread::{
        self,
        JoinHandle,
    },
};

//==================================================================================================
// Standalone Functions
//==================================================================================================

fn main() -> Result<()> {
    // Initialize logger before doing anything else, to have rich log support from the very
    // beginning. If this fails, the program will panic.
    logging::initialize();

    let mut args: Args = args::Args::parse(env::args().collect())?;

    // Create a channel to connect the VM to the standard input device.
    let (tx_channel_to_vm, rx_channel_from_stdin): (
        mpsc::Sender<std::result::Result<u8, anyhow::Error>>,
        mpsc::Receiver<std::result::Result<u8, anyhow::Error>>,
    ) = mpsc::channel::<Result<u8>>();

    // Create a channel to connect the VM to the standard output device.
    let (tx_channel_to_stdout, rx_channel_from_vm): (
        mpsc::Sender<std::result::Result<u8, anyhow::Error>>,
        mpsc::Receiver<std::result::Result<u8, anyhow::Error>>,
    ) = mpsc::channel::<Result<u8>>();

    // Spawn I/O thread.
    let _io_thread: JoinHandle<()> = {
        let vm_stdin: Option<String> = args.take_vm_stdin();
        let vm_stdout: Option<String> = args.take_vm_stdout();
        thread::spawn(move || {
            if let Err(e) =
                file::file_server(vm_stdin, vm_stdout, tx_channel_to_vm, rx_channel_from_vm)
            {
                error!("file server has failed: {:?}", e);
            }
        })
    };

    run_vmm(args, rx_channel_from_stdin, tx_channel_to_stdout)?;

    Ok(())
}

///
/// # Description
///
/// This function runs the virtual machine monitor (VMM) with the given arguments.
///
/// # Parameters
///
/// * `args` - Arguments for the virtual machine monitor.
pub fn run_vmm(
    mut args: Args,
    rx_channel_from_stdin: mpsc::Receiver<std::result::Result<u8, anyhow::Error>>,
    tx_channel_to_stdout: mpsc::Sender<std::result::Result<u8, anyhow::Error>>,
) -> Result<()> {
    crate::timer!("main");

    // Input function used for emulating I/O port reads.
    let input = move |size| -> Result<u32> {
        const EOF: u8 = 1 << 7;
        // Check for invalid operand size.
        if size < 2 {
            let reason: String = format!("invalid operand size (size={:?})", size);
            error!("input(): {}", reason);
            anyhow::bail!(reason);
        }

        let mut buf: [u8; 4] = [0; 4];

        match rx_channel_from_stdin.try_recv() {
            Ok(Ok(message)) => {
                buf[0] = message;
                buf[3] = 0;
                return Ok(u32::from_ne_bytes(buf));
            },
            Ok(Err(err)) => {
                let reason: String = format!("failed to receive message: {:?}", err);
                error!("input(): {}", reason);
                anyhow::bail!(reason);
            },
            Err(mpsc::TryRecvError::Empty) => {
                buf[3] = EOF;
                return Ok(u32::from_ne_bytes(buf));
            },
            Err(mpsc::TryRecvError::Disconnected) => {
                let reason: String = format!("channel has been disconnected");
                error!("input(): {}", reason);
                anyhow::bail!(reason);
            },
        };
    };

    // Obtain a buffered write for the virtual machine's standard error device.
    let mut file_writer: Box<dyn Write> = get_vm_stderr_writer(args.take_vm_stderr())?;

    // Output function used for emulating I/O port writes.
    let output = move |data, size| -> Result<()> {
        // Parse operand size do determine how to handle the operation.
        if size == 1 {
            // Write to the standard error device.

            // Convert data to a character.
            let ch: char = match char::from_u32(data) {
                // Valid character.
                Some(ch) => ch,
                // Invalid character.
                None => {
                    let reason: String = format!("invalid character (data={:?})", data);
                    error!("output(): {}", reason);
                    anyhow::bail!(reason);
                },
            };

            let buf: &[u8] = &[ch as u8];

            file_writer.write(buf)?;

            Ok(())
        } else {
            // Write to the standard output device.
            tx_channel_to_stdout.send(Ok(data as u8))?;
            Ok(())
        }
    };

    let mut microvm: MicroVm = MicroVm::new(args.memory_size(), Box::new(input), Box::new(output))?;

    let rip: u64 = microvm.load_kernel(args.kernel_filename())?;
    if let Some(ref initrd_filename) = args.initrd_filename() {
        microvm.load_initrd(initrd_filename)?;
    }

    microvm.reset(rip)?;

    microvm.run()?;

    Ok(())
}

///
/// # Description
///
/// Obtains a buffered writer for the virtual machine's standard error device. If the standard
/// error device is set to a file, the function attempts to open the file and create a buffered
/// writer. If the standard error device is not set to a file, the function falls back to stderr.
///
/// # Parameters
///
/// * `vm_stderr` - The path to the file where the standard error device is set.
///
/// # Returns
///
/// On success, the function returns a buffered writer for the virtual machine's standard error
///
fn get_vm_stderr_writer(vm_stderr: Option<String>) -> Result<Box<dyn Write>> {
    // Obtain a buffered writer for the virtual machine's standard error device.
    let file_writer: Box<dyn Write> = if let Some(vm_stderr) = vm_stderr {
        // Standard error was set to a file. Attempt to open file and create a writer.
        let file = File::options()
            .read(false)
            .write(true)
            .create(true)
            .open(&vm_stderr)?;
        Box::new(file)
    } else {
        // Standard error was not set to a file. Fallback to stderr.
        Box::new(std::io::stderr())
    };
    Ok(file_writer)
}
