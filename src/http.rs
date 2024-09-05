// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//==================================================================================================
// Imports
//==================================================================================================

use ::anyhow::Result;
use ::serde_json::Value;
use ::std::{
    self,
    io::{
        BufRead,
        BufReader,
        Read,
        Write,
    },
    mem,
    net::{
        SocketAddr,
        TcpListener,
        TcpStream,
    },
    sync::mpsc,
};
use ::sys::ipc::Message;

//==================================================================================================
// Http Response
//==================================================================================================

struct HttpResponse {
    status_code: u16,
    reason: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: u16, reason: &str) -> Self {
        Self {
            status_code,
            reason: reason.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn add_header(&mut self, key: &str, value: String) {
        self.headers.push((key.to_string(), value.to_string()));
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = body;
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Status line
        bytes.extend_from_slice(
            format!("HTTP/1.1 {} {}\r\n", self.status_code, self.reason).as_bytes(),
        );

        // Headers
        for (key, value) in &self.headers {
            bytes.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }

        // Body
        bytes.extend_from_slice(b"\r\n");
        bytes.extend_from_slice(&self.body);

        bytes
    }

    pub fn trace(&self) {
        trace!("HTTP/1.1 {} {}", self.status_code, self.reason);
        for (key, value) in &self.headers {
            trace!("{}: {}", key, value);
        }
        trace!("body: {}", String::from_utf8_lossy(&self.body));
    }
}

fn message_to_json(message: &Message) -> serde_json::Map<String, Value> {
    let mut json = serde_json::Map::new();
    json.insert(
        "source".to_string(),
        Value::Number(serde_json::Number::from(u32::from(message.source))),
    );
    json.insert(
        "destination".to_string(),
        Value::Number(serde_json::Number::from(u32::from(message.destination))),
    );
    let message_type_str: String = format!("{:?}", message.message_type);
    json.insert("message_type".to_string(), Value::String(message_type_str));

    let bytes = Vec::<u8>::from(&message.payload[..]);
    json.insert(
        "payload".to_string(),
        Value::Array(
            bytes
                .iter()
                .map(|b| Value::Number(serde_json::Number::from(*b)))
                .collect(),
        ),
    );

    json
}

//==================================================================================================
// Standalone Functions
//==================================================================================================

pub struct HttpServer {
    addr: SocketAddr,
    tx_channel_to_vm:
        mpsc::Sender<std::result::Result<[u8; mem::size_of::<Message>()], anyhow::Error>>,
    rx_channel_from_vm:
        mpsc::Receiver<std::result::Result<[u8; mem::size_of::<Message>()], anyhow::Error>>,
}

impl HttpServer {
    pub fn new(
        addr: SocketAddr,
        tx_channel_to_vm: mpsc::Sender<
            std::result::Result<[u8; mem::size_of::<Message>()], anyhow::Error>,
        >,
        rx_channel_from_vm: mpsc::Receiver<
            std::result::Result<[u8; mem::size_of::<Message>()], anyhow::Error>,
        >,
    ) -> Self {
        Self {
            addr,
            tx_channel_to_vm,
            rx_channel_from_vm,
        }
    }

    pub fn run(&self) -> Result<()> {
        loop {
            let listener: TcpListener = TcpListener::bind(self.addr)?;

            let (mut stream, _) = listener.accept()?;
            trace!("http_server(): accepted connection from {}", stream.peer_addr()?);

            loop {
                handle_connection(&mut stream, &self.tx_channel_to_vm, &self.rx_channel_from_vm)?;
            }
        }
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    tx_channel_to_vm: &mpsc::Sender<
        std::result::Result<[u8; mem::size_of::<Message>()], anyhow::Error>,
    >,
    rx_channel_from_vm: &mpsc::Receiver<
        std::result::Result<[u8; mem::size_of::<Message>()], anyhow::Error>,
    >,
) -> Result<()> {
    let mut buf_reader: BufReader<&mut TcpStream> = BufReader::new(stream);

    // Print the request line and headers
    let mut content_length: usize = 0;
    for line in buf_reader.by_ref().lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        trace!("{}", line);

        // Check for Content-Length header
        if line.to_lowercase().starts_with("content-length:") {
            let parts: Vec<&str> = line.split(':').collect();
            if let Some(length_str) = parts.get(1) {
                content_length = length_str.trim().parse().unwrap_or(0);
            }
        }
    }

    // Read the body if Content-Length is specified
    if content_length > 0 {
        let mut body = vec![0; content_length];
        buf_reader.read_exact(&mut body)?;
        let body_str = String::from_utf8_lossy(&body);
        trace!("body: {}", body_str);

        // Parse the body as JSON
        let json: Value = serde_json::from_str(&body_str)?;

        // Extract destination process.
        let pid: u32 = match json.get("destination").and_then(Value::as_u64) {
            Some(pid) => pid as u32,
            None => {
                println!("PID key not found or not a number");
                return Ok(());
            },
        };
        let mut message: sys::ipc::Message = sys::ipc::Message::default();
        message.destination = sys::pm::ProcessIdentifier::from(pid);
        message.source = sys::pm::ProcessIdentifier::from(0);
        message.message_type = sys::ipc::MessageType::Ikc;

        // Write "Payload" value as a raw array of bytes.
        if let Some(array) = json.get("payload").and_then(Value::as_array) {
            for (i, value) in array.iter().enumerate() {
                match value.as_u64() {
                    Some(byte) => message.payload[i] = byte as u8,
                    None => {
                        println!("Value at index {} is not a number", i);
                        return Ok(());
                    },
                }
            }
        }

        // Send message to virtual machine.
        let bytes: [u8; mem::size_of::<Message>()] = message.to_bytes();
        tx_channel_to_vm.send(Ok(bytes))?;

        // Receive a message from the virtual machine.
        let bytes: [u8; mem::size_of::<Message>()] = rx_channel_from_vm.recv()??;

        trace!("received message from VM: {:?}", bytes.len());

        // Convert message to Message struct.
        match Message::try_from_bytes(bytes) {
            Ok(message) => {
                if let Err(e) = handle_message(stream, &message) {
                    println!("Failed to handle message: {:?}", e);
                }
            },
            Err(e) => println!("Failed to parse message: {:?}", e),
        }
    }

    Ok(())
}

fn handle_message(
    stream: &mut TcpStream,
    message: &Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = message_to_json(message);

    // Send HTTP response
    let content: Vec<u8> = serde_json::to_vec(&json)?;
    let mut response: HttpResponse = HttpResponse::new(200, "OK");
    response.add_header("Content-Type", "application/json".to_owned());
    response.add_header("Content-Length", content.len().to_string());
    response.set_body(content);

    response.trace();

    stream.write_all(&response.to_bytes())?;
    Ok(())
}
