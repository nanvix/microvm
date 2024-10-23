// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::std::{
    io::{
        Read,
        Write,
    },
    mem,
    net::{
        SocketAddr,
        TcpStream,
    },
    sync::mpsc::{
        Receiver,
        Sender,
        TryRecvError,
    },
    thread::{
        self,
        JoinHandle,
    },
    time::Duration,
};
use ::sys::ipc::Message;

//==================================================================================================
// Structure
//==================================================================================================

///
/// # Description
///
/// Private data of the I/O thread.
///
pub struct IoThread {
    /// Connection to the gateway.
    conn: Option<TcpStream>,
    /// Gateway receiver.
    gateway_rx: Receiver<Message>,
    /// Gateway sender.
    gateway_tx: Sender<Message>,
}

//==================================================================================================
// Implementations
//==================================================================================================

impl IoThread {
    ///
    /// # Description
    ///
    /// Spawns a new I/O thread.
    ///
    /// # Parameters
    ///
    /// - `gateway_addr`: Gateway address.
    /// - `gateway_rx`:   Gateway receiver.
    /// - `gateway_tx`:   Gateway sender.
    /// - `read_timeout`: Read timeout.
    ///
    /// # Returns
    ///
    /// A handle to the I/O thread.
    ///
    pub fn spawn(
        gateway_addr: Option<SocketAddr>,
        gateway_rx: Receiver<Message>,
        gateway_tx: Sender<Message>,
        read_timeout: Duration,
    ) -> JoinHandle<Result<()>> {
        thread::spawn(move || {
            let mut io_thread: IoThread =
                IoThread::new(gateway_addr, gateway_rx, gateway_tx, read_timeout)?;
            io_thread.run()?;
            Ok(())
        })
    }

    ///
    /// # Description
    ///
    /// Creates a new I/O thread.
    ///
    /// # Parameters
    ///
    /// - `gateway_addr`: Gateway address.
    /// - `gateway_rx`:   Gateway receiver.
    /// - `gateway_tx`:   Gateway sender.
    /// - `read_timeout`: Read timeout.
    ///
    /// # Returns
    ///
    /// Upon success, a new I/O thread is returned. Otherwise, an error is returned.
    ///
    fn new(
        gateway_addr: Option<SocketAddr>,
        gateway_rx: Receiver<Message>,
        gateway_tx: Sender<Message>,
        read_timeout: Duration,
    ) -> Result<Self> {
        let conn: Option<TcpStream> = match gateway_addr {
            Some(addr) => match TcpStream::connect(addr) {
                Ok(conn) => {
                    conn.set_read_timeout(Some(read_timeout))?;
                    Some(conn)
                },
                Err(e) => {
                    let reason: String = format!("failed to connect to gateway (error={:?})", e);
                    error!("io_thread(): {}", reason);
                    anyhow::bail!(reason)
                },
            },
            None => None,
        };

        Ok(Self {
            conn,
            gateway_rx,
            gateway_tx,
        })
    }

    ///
    /// # Description
    ///
    /// Runs the I/O thread.
    ///
    /// # Returns
    ///
    /// Upon success, empty is returned. Otherwise, an error is returned instead.
    ///
    fn run(&mut self) -> Result<()> {
        loop {
            self.send()?;
            self.receive()?;
        }
    }

    ///
    /// # Description
    ///
    /// Attempts to send pending messages to the gateway.
    ///
    /// # Returns
    ///
    /// Upon success, empty is returned. Otherwise, an error is returned instead.
    ///
    /// # Errors
    ///
    /// If the message could not be sent, an error is returned.
    ///
    fn send(&mut self) -> Result<()> {
        match self.gateway_rx.try_recv() {
            Ok(msg) => {
                let bytes: [u8; mem::size_of::<Message>()] = msg.to_bytes();

                match self.conn {
                    Some(ref mut conn) => conn.write_all(&bytes)?,
                    None => {
                        warn!("send(): the microvm is not connected to a gateway");
                    },
                }
            },
            Err(TryRecvError::Empty) => {
                // No message available.
            },
            Err(TryRecvError::Disconnected) => {
                let reason: String = "the microvm has disconnected".to_string();
                error!("send(): {}", reason);
                anyhow::bail!(reason);
            },
        }
        Ok(())
    }

    ///
    /// # Description
    ///
    /// Attempts to receive messages from the gateway.
    ///
    /// # Returns
    ///
    /// Upon success, empty is returned. Otherwise, an error is returned instead.
    ///
    fn receive(&mut self) -> Result<()> {
        if let Some(ref mut conn) = self.conn {
            let mut bytes: [u8; mem::size_of::<Message>()] = [0; mem::size_of::<Message>()];
            match conn.read_exact(&mut bytes) {
                Ok(()) => {
                    let message: Message = match Message::try_from_bytes(bytes) {
                        Ok(message) => message,
                        Err(err) => {
                            let reason: String =
                                format!("failed to parse message (error={:?})", err);
                            warn!("receive(): {}", reason);
                            return Ok(());
                        },
                    };

                    if let Err(e) = self.gateway_tx.send(message) {
                        let reason: String =
                            format!("failed to send message to the microvm (error={:?})", e);
                        error!("receive(): {}", reason);
                        anyhow::bail!(reason);
                    }
                },
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        return Ok(());
                    },
                    _ => {
                        let reason: String =
                            format!("failed to receive message from the gateway (error={:?})", e);
                        error!("receive(): {}", reason);
                        anyhow::bail!(reason);
                    },
                },
            }
        }
        Ok(())
    }
}
