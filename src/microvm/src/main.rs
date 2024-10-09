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
mod vmm;

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
    vmm::Vmm,
};
use ::anyhow::Result;
use ::std::{
    env,
    net::SocketAddr,
};

//==================================================================================================
// Standalone Functions
//==================================================================================================

fn main() -> Result<()> {
    // Initialize logger before doing anything else. If this fails, the program will panic.
    logging::initialize();

    let mut args: Args = args::Args::parse(env::args().collect())?;
    let kernel_filename: String = args.kernel_filename().to_string();
    let initrd_filename: Option<String> = args.initrd_filename();
    let memory_size: usize = args.memory_size();
    let stderr: Option<String> = args.take_vm_stderr();
    let http_addr: SocketAddr = args.http_addr().parse()?;

    let mut vmm: Vmm =
        vmm::Vmm::new(memory_size, &kernel_filename, initrd_filename, stderr, http_addr)?;

    vmm.run()?;

    Ok(())
}
