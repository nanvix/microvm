// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

//!
//! # Description
//!
//! This program demonstrates how to use the `gateway` module to create a simple echo server.
//!

//==================================================================================================
// Configuration
//==================================================================================================

#![deny(clippy::all)]

//==================================================================================================
// Modules
//==================================================================================================

mod args;
mod config;
mod logging;

//==================================================================================================
// Imports
//==================================================================================================

// Must come first.
#[macro_use]
extern crate log;

use crate::args::Args;
use ::anyhow::Result;
use ::gateway::{
    gateway::GatewayReceiver,
    GatewaySender,
};
use ::std::{
    env,
    net::SocketAddr,
    thread::{
        self,
        JoinHandle,
    },
};

//==================================================================================================
// Standalone Functions
//==================================================================================================

fn main() -> Result<()> {
    // Initialize logger before doing anything else. If this fails, the program will panic.
    logging::initialize();

    // Parse command line arguments.
    let mut args: Args = Args::parse(env::args().collect())?;

    // Parse socket address.
    let sockaddr: SocketAddr = args.sockaddr().parse()?;

    // Create gateway.
    let (gateway, mut rx): (GatewayReceiver, GatewaySender) = gateway::new(sockaddr);

    // Spawn a thread to run the gateway and handle incoming messages.
    let _gateway_thread: JoinHandle<()> = thread::spawn(move || {
        if let Err(e) = gateway.run() {
            error!("gateway thread failed: {:?}", e);
        }
    });

    // Process incoming messages.
    loop {
        // Attempt to receive a message from the gateway.
        let (tx, message) = match rx.try_recv() {
            Ok((msg, tx)) => {
                info!("received message from stdin: {:?}", msg);

                (tx, msg)
            },
            // No message available.
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => continue,
            // Channel has disconnected.
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                info!("stdin channel has disconnected");
                break;
            },
        };

        // Swap the source and destination of the message.
        let source: ProcessIdentifier = message.destination;
        message.destination = message.source;
        message.source = source;

        // Send the message back to the gateway.
        if let Err(e) = tx.blocking_send(Ok(message)) {
            error!("failed to send message (error={:?})", e);
        }
    }

    Ok(())
}
