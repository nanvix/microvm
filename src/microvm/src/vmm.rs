// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

#[cfg(target_os = "linux")]
extern crate kvm_bindings;
#[cfg(target_os = "linux")]
extern crate kvm_ioctls;

use crate::{
    kvm::vmem::VirtualMemory,
    microvm::{
        self,
        MicroVm,
    },
};
use ::anyhow::Result;
use ::gateway::{
    gateway::GatewayReceiver,
    GatewaySender,
};
use ::std::{
    cell::RefCell,
    collections::VecDeque,
    fs::File,
    io::Write,
    mem,
    net::SocketAddr,
    rc::Rc,
    thread::{
        self,
        JoinHandle,
    },
};
use ::sys::ipc::{
    Message,
    MessageType,
};
use ::tokio::sync::mpsc::Sender;

//==================================================================================================
// Structure
//==================================================================================================

pub struct Vmm {
    microvm: MicroVm,
}

type MessageQueue = Rc<RefCell<VecDeque<Sender<Result<Message, anyhow::Error>>>>>;

//==================================================================================================
// Implementations
//==================================================================================================

impl Vmm {
    pub fn new(
        memory_size: usize,
        kernel_filename: &str,
        initrd_filename: Option<String>,
        stderr: Option<String>,
        sockaddr: SocketAddr,
    ) -> Result<Self> {
        crate::timer!("vmm_creation");

        // Create gateway.
        let (receiver, sender): (GatewayReceiver, GatewaySender) = gateway::new(sockaddr);

        // Spawn I/O thread.
        let _io_thread: JoinHandle<()> = thread::spawn(move || {
            if let Err(e) = receiver.run() {
                error!("gateway thread failed: {:?}", e);
            }
        });

        let queue: MessageQueue = Rc::new(RefCell::new(VecDeque::new()));

        // Input function used for emulating I/O port reads.
        let input: Box<microvm::InputFn> = Self::build_input_fn(queue.clone(), sender);

        // Output function used for emulating I/O port writes.
        let output: Box<microvm::OutputFn> =
            Self::build_output_fn(Self::get_stderr_writer(stderr.clone())?, queue);

        let mut microvm: MicroVm = MicroVm::new(memory_size, input, output)?;

        let rip: u64 = microvm.load_kernel(kernel_filename)?;
        if let Some(ref initrd_filename) = initrd_filename {
            microvm.load_initrd(initrd_filename)?;
        }

        microvm.reset(rip)?;

        Ok(Self { microvm })
    }

    ///
    /// # Description
    ///
    /// This function runs the virtual machine monitor (VMM) with the given arguments.
    ///
    /// # Parameters
    ///
    /// * `args` - Arguments for the virtual machine monitor.
    pub fn run(&mut self) -> Result<()> {
        self.microvm.run()?;

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
    fn get_stderr_writer(vm_stderr: Option<String>) -> Result<Box<dyn Write>> {
        // Obtain a buffered writer for the virtual machine's standard error device.
        let file_writer: Box<dyn Write> = if let Some(vm_stderr) = vm_stderr {
            // Standard error was set to a file. Attempt to open file and create a writer.
            let file = File::options()
                .read(false)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&vm_stderr)?;
            Box::new(file)
        } else {
            // Standard error was not set to a file. Fallback to stderr.
            Box::new(std::io::stderr())
        };
        Ok(file_writer)
    }

    fn build_input_fn(
        input_queue: MessageQueue,
        mut sender: GatewaySender,
    ) -> Box<microvm::InputFn> {
        // Input function used for emulating I/O port reads.
        let input = move |vm: &Rc<RefCell<VirtualMemory>>, data, size| -> Result<()> {
            // Check for invalid operand size.
            if size != 4 {
                let reason: String = format!("invalid operand size (size={:?})", size);
                error!("input(): {}", reason);
                anyhow::bail!(reason);
            }

            match sender.try_recv() {
                Ok((mut msg, tx)) => {
                    msg.message_type = MessageType::Ikc;
                    vm.borrow_mut().write_bytes(data as u64, &msg.to_bytes())?;

                    input_queue.borrow_mut().push_back(tx);
                },
                // No message available.
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => return Ok(()),
                // Channel has disconnected.
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    let reason: String = "channel has been disconnected".to_string();
                    error!("input(): {}", reason);
                    anyhow::bail!(reason);
                },
            }

            Ok(())
        };

        Box::new(input)
    }

    fn build_output_fn(
        mut file_writer: Box<dyn Write>,
        queue: MessageQueue,
    ) -> Box<microvm::OutputFn> {
        // Output function used for emulating I/O port writes.
        let output = move |vm: &Rc<RefCell<VirtualMemory>>, data, size| -> Result<()> {
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

                file_writer.write_all(buf)?;

                Ok(())
            } else {
                // Write to the standard output device.
                let mut bytes: [u8; mem::size_of::<Message>()] = [0; mem::size_of::<Message>()];
                vm.borrow_mut().read_bytes(data as u64, &mut bytes)?;

                let message: Message = match Message::try_from_bytes(bytes) {
                    Ok(message) => message,
                    Err(err) => {
                        let reason: String = format!("failed to parse message: {:?}", err);
                        error!("output(): {}", reason);
                        anyhow::bail!(reason);
                    },
                };

                if let Some(tx) = queue.borrow_mut().pop_front() {
                    if let Err(e) = tx.blocking_send(Ok(message)) {
                        let reason: String = format!("failed to send message: {:?}", e);
                        error!("output(): {}", reason);
                        anyhow::bail!(reason);
                    }
                }

                Ok(())
            }
        };

        Box::new(output)
    }
}
