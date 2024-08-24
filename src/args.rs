// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//!
//! # Arguments
//!
//! This module provides utilities for parsing command-line arguments that were supplied to the
//! program.
//!

//==================================================================================================
// Imports
//==================================================================================================

use crate::config;
use ::anyhow::Result;
use ::std::{
    env,
    fs::File,
    process,
};

//==================================================================================================
// Public Structures
//==================================================================================================

///
/// # Description
///
/// This structure packs the command-line arguments that were passed to the program.
///
pub struct Args {
    /// Kernel filename.
    kernel_filename: String,
    /// Initrd filename.
    initrd_filename: Option<String>,
    /// Memory size.
    memory_size: usize,
    /// Standard output.
    vm_stdout: Option<File>,
    /// Standard input.
    vm_stdin: Option<File>,
}

//==================================================================================================
// Implementations
//==================================================================================================

impl Args {
    /// Command-line option for printing the help message.
    const OPT_HELP: &'static str = "-help";
    /// Command-line option for the kernel file.
    const OPT_KERNEL: &'static str = "-kernel";
    /// Command-line option for initrd file.
    const OPT_INITRD: &'static str = "-initrd";
    /// Command-line option for the memory size.
    const OPT_MEMORY_SIZE: &'static str = "-memory";
    /// Command-line option for the standard output.
    const OPT_STDOUT: &'static str = "-stdout";
    /// Command-line option for the standard input.
    const OPT_STDIN: &'static str = "-stdin";

    ///
    /// # Description
    ///
    /// Parses the command-line arguments that were passed to the program.
    ///
    /// # Returns
    ///
    /// Upon successful completion, this method returns the command-line arguments that were passed
    /// to the program. Otherwise, it returns an error.
    ///
    pub fn parse(args: Vec<String>) -> Result<Self> {
        trace!("parse(): args={:?}", args);

        let mut kernel_filename: String = String::new();
        let mut initrd_filename: Option<String> = None;
        let mut memory_size: usize = config::DEFAULT_MEMORY_SIZE;
        let mut vm_stdout: Option<File> = None;
        let mut vm_stdin: Option<File> = None;

        // Parse command-line arguments.
        let mut i: usize = 1;
        while i < args.len() {
            match args[i].as_str() {
                // Print help message and exit.
                Self::OPT_HELP => {
                    Self::usage();
                    process::exit(0);
                },
                // Set initrd file.
                Self::OPT_INITRD if i + 1 < args.len() => {
                    initrd_filename = Some(args[i + 1].clone());
                    i += 1;
                },
                // Set kernel file.
                Self::OPT_KERNEL if i + 1 < args.len() => {
                    kernel_filename = args[i + 1].clone();
                    i += 1;
                },
                // Set memory size.
                Self::OPT_MEMORY_SIZE if i + 1 < args.len() => {
                    let mem_arg: &String = &args[i + 1];

                    // Parse memory size.
                    memory_size = match mem_arg[..mem_arg.len() - 1].parse::<usize>() {
                        Ok(size) => size,
                        Err(e) => {
                            let reason: String = format!("invalid memory size (error={})", e);
                            error!("parse(): {}", reason);
                            anyhow::bail!(reason);
                        },
                    };

                    // Parse memory size suffix.
                    let endptr: char = match mem_arg.chars().last() {
                        Some(c) => c,
                        None => {
                            let reason: String = format!("invalid memory size '{}'", mem_arg);
                            error!("parse(): {}", reason);
                            anyhow::bail!(reason);
                        },
                    };
                    match endptr {
                        'K' | 'k' => memory_size *= 1024,
                        'M' | 'm' => memory_size *= 1024 * 1024,
                        'G' | 'g' => memory_size *= 1024 * 1024 * 1024,
                        ch => {
                            let reason: String = format!("invalid memory size suffix '{}'", ch);
                            error!("parse(): {}", reason);
                            anyhow::bail!(reason);
                        },
                    }
                    i += 1;
                },
                // Set output file.
                Self::OPT_STDOUT if i + 1 < args.len() => {
                    // Attempt to open output file.
                    vm_stdout = match File::options().read(false).write(true).open(&args[i + 1]) {
                        Ok(file) => Some(file),
                        Err(e) => {
                            anyhow::bail!("fopen: {}", e);
                        },
                    };
                    i += 1;
                },
                // Set input file.
                Self::OPT_STDIN if i + 1 < args.len() => {
                    // Attempt to open input file.
                    vm_stdin = match File::options().read(true).write(false).open(&args[i + 1]) {
                        Ok(file) => Some(file),
                        Err(e) => {
                            anyhow::bail!("fopen: {}", e);
                        },
                    };
                    i += 1;
                },
                // Invalid argument.
                _ => {
                    Self::usage();
                    let reason: String = format!("invalid argument {}", args[i]);
                    error!("parse(): {}", reason);
                    anyhow::bail!(reason);
                },
            }

            i += 1;
        }

        // Check if kernel file is missing.
        if kernel_filename.is_empty() {
            Self::usage();
            anyhow::bail!("kernel file is missing");
        }

        // Check if memory size is invalid.
        if memory_size == 0 {
            Self::usage();
            anyhow::bail!("invalid memory size");
        }

        Ok(Self {
            kernel_filename,
            initrd_filename,
            memory_size,
            vm_stdout,
            vm_stdin,
        })
    }

    ///
    /// # Description
    ///
    /// Prints program usage.
    ///
    pub fn usage() {
        eprintln!(
            "Usage: {} {} <kernel> [{} <size>] [{} <file>] [{} <file>]",
            env::args()
                .next()
                .unwrap_or(config::PROGRAM_NAME.to_string()),
            Self::OPT_KERNEL,
            Self::OPT_MEMORY_SIZE,
            Self::OPT_STDOUT,
            Self::OPT_STDIN
        );
    }

    ///
    /// # Description
    ///
    /// Returns the initrd filename that was passed as a command-line argument to the program.
    ///
    /// # Returns
    ///
    /// The initrd filename that was passed as a command-line argument to the program. If no initrd
    /// filename was passed, this method returns `None`.
    ///
    pub fn initrd_filename(&self) -> Option<&str> {
        self.initrd_filename.as_deref()
    }

    ///
    /// # Description
    ///
    /// Returns the kernel filename that was passed as a command-line argument to the program.
    ///
    /// # Returns
    ///
    /// The kernel filename that was passed as a command-line argument to the program.
    ///
    pub fn kernel_filename(&self) -> &str {
        &self.kernel_filename
    }

    ///
    /// # Description
    ///
    /// Returns the memory size that was passed as a command-line argument to the program.
    ///
    /// # Returns
    ///
    /// The memory size that was passed as a command-line argument to the program.
    ///
    pub fn memory_size(&self) -> usize {
        self.memory_size
    }

    ///
    /// # Description
    ///
    /// Returns the output file file that was passed as a command-line argument to the program.
    ///
    /// # Returns
    ///
    /// The output file that was passed as a command-line argument to the program. If no output file
    /// was passed, or this option was already taken, this method returns `None`.
    ///
    pub fn take_vm_stdout(&mut self) -> Option<File> {
        self.vm_stdout.take()
    }

    ///
    /// # Description
    ///
    /// Returns the input file that was passed as a command-line argument to the program.
    ///
    /// # Returns
    ///
    /// The input file that was passed as a command-line argument to the program. If no input file
    /// was passed, or this option was already taken, this method returns `None`.
    ///
    pub fn take_vm_stdin(&mut self) -> Option<File> {
        self.vm_stdin.take()
    }
}
