# Dusk Unix Domain Sockets

[![Crate](https://img.shields.io/crates/v/dusk-uds.svg)](https://crates.io/crates/dusk-uds)
[![Documentation](https://docs.rs/dusk-uds/badge.svg)](https://docs.rs/dusk-uds)

Minimalistic boilerplate for [`std::os::unix::net::UnixListener`] bindings.

Complies with the log facade.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
dusk-uds = "0.1"
```

## Example

```rust
use std::{
    future::Future,
    io::{Read, Write},
    os::unix::net::UnixStream,
    pin::Pin,
    task::Context,
    task::Poll,
};

use dusk_uds::*;

// This structure will handle the incoming sockets
struct MyFuture {
    socket: Option<UnixStream>,
}

// Optional implementation of default to facilitate the clone
impl Default for MyFuture {
    fn default() -> Self {
        MyFuture { socket: None }
    }
}

// Clone implementation is mandatory, for this instance will be shared amongst the worker threads
// with provided ownership.
impl Clone for MyFuture {
    fn clone(&self) -> Self {
        MyFuture::default()
    }
}

// Allow the UDS provider to send the socket to the structure before the poll
impl TaskProvider for MyFuture {
    fn set_socket(&mut self, socket: UnixStream) {
        self.socket.replace(socket);
    }
}

// Standard future implementation.
//
// Will read a byte from the socket, multiply it by 2, and write the result.
//
// If the provided byte is 0, will quit
impl Future for MyFuture {
    type Output = Message;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        let mut buffer = [0x00u8];
        let socket = self.socket.as_mut().unwrap();

        socket.read_exact(&mut buffer).unwrap();
        buffer[0] *= 2;

        if buffer[0] == 0 {
            return Poll::Ready(Message::ShouldQuit);
        }

        socket.write_all(&buffer).unwrap();
        Poll::Ready(Message::Success)
    }
}

fn main() {
    // The first parameter is the path of the socket
    // The second, the options, if there is the need to customize the UDS somehow
    // The third, is the Future implementation that will handle the incoming sockets
    UnixDomainSocket::new("/tmp/dusk-socket", None, MyFuture::default())
        .bind()
        .unwrap();
}
```
