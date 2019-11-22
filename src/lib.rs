#![feature(external_doc)]
#![deny(missing_docs)]
#![doc(include = "../README.md")]

#[macro_use]
extern crate log;

use std::{future::Future, os::unix::net::UnixStream};

pub use communication::{Message, Task};
pub use options::Options;
pub use uds::UnixDomainSocket;

mod communication;
mod options;
mod uds;
mod worker;

/// Future provider to the UDS implementation
pub trait TaskProvider: Send + Sync + Clone + Future<Output = Message> {
    /// Receive a socket to handle it during the future poll call
    fn set_socket(&mut self, socket: UnixStream);
}
