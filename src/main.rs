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
    io,
    io::{
        Read,
        Write,
    },
};

//==================================================================================================

fn main() -> Result<()> {
    // Initialize logger before doing anything else, to have rich log support from the very
    // beginning. If this fails, the program will panic.
    logging::initialize();

    let mut args: Args = args::Args::parse(env::args().collect())?;

    // Input function used for emulating I/O port reads.
    let mut vm_stdin: Option<File> = args.take_vm_stdin();
    let input = move |size| -> Result<u32> {
        // Check for invalid operand size.
        if size != 1 {
            let reason: String = format!("invalid operand size (size={:?})", size);
            error!("input(): {}", reason);
            anyhow::bail!(reason);
        }

        let mut buf: [u8; 1] = [0; 1];

        // Forward request to backend device.
        match vm_stdin {
            // Read from file.
            Some(ref mut file) => {
                file.read_exact(&mut buf)?;
                Ok(buf[0] as u32)
            },
            // Fallback and read from standard input.
            None => {
                let _guard: std::io::StdinLock<'_> = std::io::stdin().lock();
                io::stdin().read_exact(&mut buf)?;
                Ok(buf[0] as u32)
            },
        }
    };

    // Output function used for emulating I/O port writes.
    let mut vm_stdout: Option<File> = args.take_vm_stdout();
    let output = move |data, size| -> Result<()> {
        // Check for invalid operand size.
        if size != 1 {
            let reason: String = format!("invalid operand size (data={:?}, size={:?})", data, size);
            error!("output(): {}", reason);
            anyhow::bail!(reason);
        }

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

        // Forward request to backend device.
        match vm_stdout {
            // Write to file.
            Some(ref mut file) => {
                file.write(buf)?;
            },
            // Fallback and write to standard output.
            None => {
                let _guard: std::io::StdoutLock<'_> = std::io::stdout().lock();
                io::stdout().write(buf)?;
            },
        }

        Ok(())
    };

    {
        crate::timer!("main");
        let mut microvm: MicroVm =
            MicroVm::new(args.memory_size(), Box::new(input), Box::new(output))?;

        let rip: u64 = microvm.load_kernel(args.kernel_filename())?;
        if let Some(ref initrd_filename) = args.initrd_filename() {
            microvm.load_initrd(initrd_filename)?;
        }

        microvm.reset(rip)?;

        microvm.run()?;
    }

    Ok(())
}
