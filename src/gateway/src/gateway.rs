// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use crate::route::GatewayLookupTable;
use ::anyhow::Result;
use ::std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
};
use ::sys::{
    ipc::Message,
    pm::ProcessIdentifier,
};
use ::tokio::{
    net::{
        TcpListener,
        TcpStream,
    },
    sync::{
        mpsc,
        mpsc::{
            UnboundedReceiver,
            UnboundedSender,
        },
    },
};

//==================================================================================================
// Traits
//==================================================================================================

///
/// Gateway Client
///
pub trait GatewayClient: Sized + Send {
    ///
    /// # Description
    ///
    /// Creates a new gateway client.
    ///
    /// # Parameters
    ///
    /// - `addr`: Address of the client.
    /// - `tx`: Transmit endpoint for messages to clients.
    /// - `rx`: Receive endpoint for messages from clients.
    ///
    /// # Returns
    ///
    /// A new gateway client.
    ///
    fn new(
        addr: SocketAddr,
        tx: UnboundedSender<(SocketAddr, Message)>,
        rx: UnboundedReceiver<Message>,
    ) -> Self;

    ///
    /// # Description
    ///
    /// Runs the gateway client.
    ///
    /// # Parameters
    ///
    /// - `client`: Gateway client.
    /// - `stream`: TCP stream associated with the client.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(())` on success, or `Err(e)` on failure.
    ///
    fn run(
        client: Self,
        stream: TcpStream,
    ) -> Pin<Box<(dyn Future<Output = Result<(), anyhow::Error>> + std::marker::Send)>>;
}

//==================================================================================================
// Structures
//==================================================================================================

///
/// Gateway
///
pub struct Gateway<T: GatewayClient> {
    /// Address of the gateway.
    addr: SocketAddr,
    /// Transmit endpoint for messages to clients.
    gateway_client_tx: UnboundedSender<(SocketAddr, Message)>,
    /// Receive endpoint for messages from clients.
    gateway_client_rx: UnboundedReceiver<(SocketAddr, Message)>,
    /// Transmit endpoint for messages to the service.
    gateway_service_tx: UnboundedSender<Message>,
    /// Receive endpoint for messages from the service.
    gateway_service_rx: UnboundedReceiver<Message>,
    /// Lookup tables.
    lookup_tables: GatewayLookupTable,
    /// Marker to force ownership over [`GatewayClient`].
    _phantom: std::marker::PhantomData<T>,
}

//==================================================================================================
// Implementations
//==================================================================================================

// Type aliases to make clippy happy.
type ClientGatewayRx = UnboundedReceiver<(SocketAddr, Message)>;
type ClientGatewayTx = UnboundedSender<(SocketAddr, Message)>;

impl<T: GatewayClient> Gateway<T> {
    ///
    /// # Description
    ///
    /// Creates a new gateway.
    ///
    /// # Parameters
    ///
    /// - `addr`: Address of the gateway.
    ///
    /// # Returns
    ///
    /// A new gateway.
    ///
    pub fn new(
        addr: SocketAddr,
    ) -> (Gateway<T>, UnboundedSender<Message>, UnboundedReceiver<Message>) {
        // Create an asynchronous channel to enable communication from the gateway to the service.
        let (gateway_service_tx, service_rx): (
            UnboundedSender<Message>,
            UnboundedReceiver<Message>,
        ) = mpsc::unbounded_channel();

        // Create an asynchronous channel to enable communication from the service to the gateway.
        let (service_tx, gateway_service_rx): (
            UnboundedSender<Message>,
            UnboundedReceiver<Message>,
        ) = mpsc::unbounded_channel();

        // Create an asynchronous channel to enable communication from the client to the gateway.
        let (gateway_client_tx, gateway_client_rx): (ClientGatewayTx, ClientGatewayRx) =
            mpsc::unbounded_channel();

        (
            Self {
                addr,
                gateway_client_rx,
                gateway_client_tx,
                gateway_service_tx,
                gateway_service_rx,
                lookup_tables: GatewayLookupTable::new(),
                _phantom: std::marker::PhantomData,
            },
            service_tx,
            service_rx,
        )
    }

    ///
    /// # Description
    ///
    /// Runs the gateway.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(())` on success, or `Err(e)` on failure.
    ///
    #[tokio::main]
    pub async fn run(&mut self) -> Result<()> {
        let listener: TcpListener = TcpListener::bind(self.addr).await?;
        loop {
            tokio::select! {
                // Attempt to accept a new client.
                Ok((stream, addr)) = listener.accept() => {
                   if let Err(e) = self.handle_accept(stream, addr).await {
                        warn!("run(): {:?}", e);
                   }
                },
                // Attempt from receive a message from any client.
                Some((addr, message)) = self.gateway_client_rx.recv() => {
                    if let Err(e) = self.handle_client_message(addr, message).await {
                        warn!("run(): {:?}", e);
                    }
                },
                // Attempt to receive a message from the service.
                Some(message) = self.gateway_service_rx.recv() => {
                    if let Err(e) = self.handle_service_message(message).await {
                        warn!("run(): {:?}", e);
                    }
                }
            }
        }
    }

    ///
    /// # Description
    ///
    /// Handles accept.
    ///
    /// # Parameters
    ///
    /// - `stream`: TCP stream associated with the client.
    /// - `addr`: Address of the client.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(())` on success, or `Err(e)` on failure.
    ///
    async fn handle_accept(&mut self, stream: TcpStream, addr: SocketAddr) -> Result<()> {
        trace!("handle_accept(): addr={:?}", addr);

        // Create an asynchronous channel to enable communication from the gateway to the client.
        let (client_tx, client_rx): (UnboundedSender<Message>, UnboundedReceiver<Message>) =
            mpsc::unbounded_channel::<Message>();

        let client: Pin<Box<dyn Future<Output = std::result::Result<(), anyhow::Error>> + Send>> =
            T::run(T::new(addr, self.gateway_client_tx.clone(), client_rx), stream);

        // Attempt to register the client.
        self.lookup_tables.register_addr(addr, client_tx).await?;

        let lookup_tables: GatewayLookupTable = self.lookup_tables.clone();
        tokio::task::spawn(async move {
            if let Err(e) = client.await {
                error!("failed to run client: {:?}", e);
            }

            // Handle client disconnection.
            Self::handle_disconnect(&lookup_tables, addr).await
        });

        Ok(())
    }

    ///
    /// # Description
    ///
    /// Handles a client disconnection.
    ///
    /// # Parameters
    ///
    /// - `lookup_tables`: Lookup tables.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(())` on success, or `Err(e)` on failure.
    ///
    async fn handle_disconnect(lookup_tables: &GatewayLookupTable, addr: SocketAddr) -> Result<()> {
        trace!("handle_disconnect(): addr={:?}", addr);

        GatewayLookupTable::remove(lookup_tables, addr).await?;

        Ok(())
    }

    ///
    /// # Description
    ///
    /// Handles a message from a client.
    ///
    /// # Parameters
    ///
    /// - `message`: Message to handle.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(())` on success, or `Err(e)` on failure.
    ///
    async fn handle_client_message(&mut self, addr: SocketAddr, message: Message) -> Result<()> {
        trace!(
            "handle_client_message(): addr={:?}, message.source={:?}, message.destination={:?}",
            addr,
            message.source,
            message.destination
        );

        let pid: ProcessIdentifier = message.source;
        self.lookup_tables.register_pid(pid, addr).await?;

        // Forward message to the service.
        self.gateway_service_tx.send(message)?;

        Ok(())
    }

    ///
    /// # Description
    ///
    /// Handles a message from the service.
    ///
    /// # Parameters
    ///
    /// - `message`: Message to handle.
    ///
    /// # Returns
    ///
    /// A future that resolves to `Ok(())` on success, or `Err(e)` on failure.
    ///
    async fn handle_service_message(&mut self, message: Message) -> Result<()> {
        trace!(
            "handle_service_message(): message.source={:?}, message.destination={:?}",
            message.source,
            message.destination
        );

        // Retrieve client.
        let client: UnboundedSender<Message> =
            self.lookup_tables.lookup(message.destination).await?;

        // Forward the message to the client.
        if let Err(e) = client.send(message) {
            let reason: String = format!("failed to send message to client (error={:?})", e);
            panic!("handle_service_message(): {}", reason);
        }

        Ok(())
    }
}
