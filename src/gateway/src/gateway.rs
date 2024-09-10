// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::http_body_util::{
    BodyExt,
    Full,
};
use ::hyper::{
    body::{
        Bytes,
        Incoming,
    },
    server::conn::http1,
    service::Service,
    Request,
    Response,
};
use ::hyper_util::rt::TokioIo;
use ::std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
};
use ::sys::ipc::Message;
use ::tokio::{
    net::{
        TcpListener,
        TcpStream,
    },
    sync::mpsc,
};
use anyhow::anyhow;
use hyper::StatusCode;
use serde::Deserialize;
use tokio::sync::mpsc::{
    Receiver,
    Sender,
};

#[derive(Deserialize)]
struct MessageJson {
    source: u32,
    destination: u32,
    payload: Option<Vec<u8>>,
}

//==================================================================================================
// Standalone Functions
//==================================================================================================

type ChannelType = (Message, mpsc::Sender<Result<Message, anyhow::Error>>);

pub fn new(addr: SocketAddr) -> (GatewayReceiver, GatewaySender) {
    let (tx_channel_to_vm, rx_from_stdin): (Sender<ChannelType>, Receiver<ChannelType>) =
        mpsc::channel::<ChannelType>(1024);
    (
        GatewayReceiver::new(addr, tx_channel_to_vm),
        GatewaySender {
            output: rx_from_stdin,
        },
    )
}

pub struct GatewaySender {
    output: mpsc::Receiver<(Message, mpsc::Sender<Result<Message, anyhow::Error>>)>,
}

impl GatewaySender {
    pub fn try_recv(
        &mut self,
    ) -> Result<(Message, mpsc::Sender<Result<Message, anyhow::Error>>), mpsc::error::TryRecvError>
    {
        self.output.try_recv()
    }
}

#[derive(Debug, Clone)]
pub struct GatewayReceiver {
    addr: SocketAddr,
    tx_channel_to_vm: mpsc::Sender<(Message, mpsc::Sender<Result<Message, anyhow::Error>>)>,
}

impl GatewayReceiver {
    fn new(
        addr: SocketAddr,
        tx_channel_to_vm: mpsc::Sender<(Message, mpsc::Sender<Result<Message, anyhow::Error>>)>,
    ) -> Self {
        Self {
            addr,
            tx_channel_to_vm,
        }
    }

    #[tokio::main]
    pub async fn run(&self) -> Result<()> {
        let listener: TcpListener = TcpListener::bind(self.addr).await?;
        loop {
            let (stream, _) = listener.accept().await?;
            trace!("http_server(): accepted connection from {}", stream.peer_addr()?);

            let io: TokioIo<TcpStream> = TokioIo::new(stream);

            let input = self.clone();

            // Spawn a tokio task to serve multiple connections concurrently
            tokio::task::spawn(async move {
                // Finally, we bind the incoming connection to our `hello` service
                if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io,  input)
                .await
                {
                    error!("Error serving connection: {:?}", err);
                }
            });
        }
    }
    fn bad_request() -> Response<Full<Bytes>> {
        let mut bad_request: Response<Full<Bytes>> = Response::new(Full::new(Bytes::new()));
        *bad_request.status_mut() = hyper::StatusCode::BAD_REQUEST;
        bad_request
    }

    fn internal_server_error() -> Response<Full<Bytes>> {
        let mut internal_server_error: Response<Full<Bytes>> =
            Response::new(Full::new(Bytes::new()));
        *internal_server_error.status_mut() = hyper::StatusCode::INTERNAL_SERVER_ERROR;
        internal_server_error
    }

    fn bytes_to_message(body: Bytes) -> Result<Message> {
        // Deserialize the JSON directly into the struct
        let message_json: MessageJson = serde_json::from_slice(body.as_ref())
            .map_err(|_| anyhow!("failed to parse request"))?;

        let mut message: sys::ipc::Message = sys::ipc::Message::new(
            sys::pm::ProcessIdentifier::from(message_json.source),
            sys::pm::ProcessIdentifier::from(message_json.destination),
            sys::ipc::MessageType::Ikc,
            [0; Message::PAYLOAD_SIZE],
        );

        // Write "Payload" value as a raw array of bytes.
        if let Some(payload) = message_json.payload {
            let len = payload.len().min(Message::PAYLOAD_SIZE);
            message.payload[..len].copy_from_slice(&payload[..len]);
        }

        Ok(message)
    }

    fn message_to_bytes(message: Message) -> Result<Bytes> {
        // Convert message to JSON.
        let json = serde_json::json!({
            "source": u32::from(message.source),
            "destination": u32::from(message.destination),
            "message_type": format!("{:?}", message.message_type),
            "payload": message.payload.iter().copied().collect::<Vec<_>>(),
        });

        // Convert JSON to bytes.
        match serde_json::to_vec(&json) {
            Ok(bytes) => Ok(Bytes::from(bytes)),
            Err(_) => anyhow::bail!("failed to parse request"),
        }
    }

    async fn process(&self, incoming: Message) -> Result<Option<Message>> {
        let (tx, mut rx) = mpsc::channel::<Result<Message, anyhow::Error>>(1);

        self.tx_channel_to_vm.send((incoming, tx)).await?;

        match rx.recv().await {
            Some(message) => Ok(Some(message?)),
            None => Ok(None),
        }
    }
}

impl Service<Request<Incoming>> for GatewayReceiver {
    type Response = Response<Full<Bytes>>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, request: Request<Incoming>) -> Self::Future {
        let self_clone = self.clone();
        let future = async move {
            let body: Bytes = request.collect().await?.to_bytes();

            let message: Message = match Self::bytes_to_message(body) {
                Ok(message) => message,
                Err(_) => {
                    return Ok(Self::bad_request());
                },
            };

            let message: Option<Message> = match self_clone.process(message).await {
                Ok(message) => message,
                Err(_) => {
                    return Ok(Self::internal_server_error());
                },
            };

            let bytes: Bytes = if let Some(message) = message {
                match Self::message_to_bytes(message) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        return Ok(Self::internal_server_error());
                    },
                }
            } else {
                Bytes::new()
            };

            match Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .header("Content-Length", bytes.len())
                .body(Full::new(bytes))
                .map_err(|_| Self::bad_request()).map_err(|_|Self::bad_request())
            {
                Ok(response) => Ok(response),
                Err(_) => Ok(Self::bad_request()),
            }
        };

        Box::pin(future)
    }
}
