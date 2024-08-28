// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::std::{
    self,
    fs::File,
    io::{
        BufReader,
        BufWriter,
        Read,
        Write,
    },
    mem,
    sync::mpsc,
};
use ::sys::ipc::Message;

//==================================================================================================
// Standalone Functions
//==================================================================================================

// TODO: decouple send and receive logic.
pub fn file_server(
    vm_stdin: Option<String>,
    vm_stdout: Option<String>,
    tx_channel_to_vm: mpsc::Sender<std::result::Result<u8, anyhow::Error>>,
    rx_channel_from_vm: mpsc::Receiver<std::result::Result<u8, anyhow::Error>>,
) -> Result<()> {
    // Obtain a buffered writer for the virtual machine's standard output device.
    let mut file_writer: BufWriter<Box<dyn Write>> = get_vm_stdout_writer(vm_stdout)?;

    // Obtain a buffered reader for the virtual machine's standard input device.
    let mut file_reader: BufReader<Box<dyn Read>> = get_vm_stdin_reader(vm_stdin)?;

    // Read a message from the input device.
    loop {
        let mut message: sys::ipc::Message = Default::default();
        message.destination = sys::pm::ProcessIdentifier::from(1); // TODO: read this from the wire.
        message.source = sys::pm::ProcessIdentifier::from(0);
        message.message_type = sys::ipc::MessageType::Ikc;

        // Read message payload from the input device and check for errors.
        if let Err(e) = file_reader.read_exact(&mut message.payload) {
            // Parse error.
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                // End of file. Log a debug message and gracefully exit.
                debug!("file_server(): reached end of file");
                break Ok(());
            } else {
                // Other error. Log an error message and continue.
                let reason: String =
                    format!("failed to read message from input device (error={})", e);
                error!("file_server(): {}", reason);
                continue;
            }
        }

        // Send message to virtual machine.
        let bytes: [u8; mem::size_of::<Message>()] = message.to_bytes();
        for b in bytes {
            tx_channel_to_vm.send(Ok(b))?;
        }

        // Receive a message from the virtual machine.
        let mut bytes: [u8; mem::size_of::<Message>()] = [0; mem::size_of::<Message>()];
        for b in &mut bytes {
            *b = rx_channel_from_vm.recv()??;
        }

        // Parse message.
        let message: Message = match Message::try_from_bytes(bytes) {
            Ok(message) => message,
            Err(e) => {
                let reason: String = format!("failed to parse message (error={:?})", e);
                error!("file_server(): {}", reason);
                continue;
            },
        };

        // Write message payload to the output device and check for errors.
        if let Err(e) = file_writer.write_all(&message.payload) {
            let reason: String = format!("failed to write message to output device (error={})", e);
            error!("file_server(): {}", reason);
            break Err(anyhow::anyhow!(reason));
        }
    }
}

///
/// # Description
///
/// Obtains a buffered writer for the virtual machine's standard output device. If the standard
/// output device is set to a file, the function attempts to open the file and create a buffered
/// writer. If the standard output device is not set to a file, the function falls back to stdout.
///
/// # Parameters
///
/// * `vm_stdout` - The path to the file where the standard output device is set.
///
/// # Returns
///
/// On success, the function returns a buffered writer for the virtual machine's standard output
/// device. On error, the function returns an error.
///
fn get_vm_stdout_writer(vm_stdout: Option<String>) -> Result<BufWriter<Box<dyn Write>>> {
    // Obtain a buffered writer for the virtual machine's standard output device.
    let file_writer: BufWriter<Box<dyn Write>> = if let Some(vm_stdout) = vm_stdout {
        // Standard output was set to a file. Attempt to open file and create a buffered writer.
        let file = File::options()
            .read(false)
            .write(true)
            .create(true)
            .open(&vm_stdout)?;
        BufWriter::new(Box::new(file))
    } else {
        // Standard output was not set to a file. Fallback to stdout.
        BufWriter::new(Box::new(std::io::stdout()))
    };
    Ok(file_writer)
}

///
/// # Description
///
/// Obtains a buffered reader for the virtual machine's standard input device. If the standard input
/// device is set to a file, the function attempts to open the file and create a buffered reader. If
/// the standard input device is not set to a file, the function falls back to stdin.
///
/// # Parameters
///
/// * `vm_stdin` - The path to the file where the standard input device is set.
///
/// # Returns
///
/// On success, the function returns a buffered reader for the virtual machine's standard input
/// device. On error, the function returns an error.
///
fn get_vm_stdin_reader(vm_stdin: Option<String>) -> Result<BufReader<Box<dyn Read>>> {
    // Obtain a buffered reader for the virtual machine's standard input device.
    let file_reader: BufReader<Box<dyn Read>> = if let Some(vm_stdin) = vm_stdin {
        // Standard input was set to a file. Attempt to open file and create a buffered reader.
        let file = File::open(&vm_stdin)?;
        BufReader::new(Box::new(file))
    } else {
        // Standard input was not set to a file. Fallback to stdin.
        BufReader::new(Box::new(std::io::stdin()))
    };
    Ok(file_reader)
}
