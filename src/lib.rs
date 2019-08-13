#![feature(external_doc)]
#![deny(missing_docs)]
#![doc(include = "../README.md")]

#[macro_use]
extern crate log;

use std::fs;
use std::io;
use std::io::Error as IoError;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, RwLock};
use std::thread;

#[derive(Debug, Clone)]
/// Possible states of the unix domain socket.
///
/// The main loop will continue while the state is [`State::Listening`]
pub enum State {
    /// No [`UnixDomainSocket::bind`] was performed yet.
    Closed = 0x00,
    /// The [`UnixDomainSocket`] is currently listening.
    Listening = 0x01,
    /// The [`UnixDomainSocket`] should interrupt the listening after the next iteration.
    ShouldQuit = 0x02,
}

#[derive(Debug, Clone)]
/// Possible messages that can be sent through the [`mpsc`] channel.
pub enum Message {
    /// Request the [`UnixDomainSocket`] to change its state.
    ChangeState(State),
}

/// Boilerplate for [`UnixListener`].
///
/// Will be converted from an implementation of [`ToString`], or will receive a [`PathBuf`] via the
/// constructor [`UnixDomainSocket::new`].
pub struct UnixDomainSocket {
    /// The OS path for the socket.
    path: PathBuf,
    /// The current state.
    ///
    /// Since its [`Arc`]/[`RwLock`] protected, we don't need a mutable reference to update the
    /// state.
    state: Arc<RwLock<State>>,
    /// [`mpsc`] channel to send messages.
    tx: mpsc::Sender<Message>,
    /// [`mpsc`] channel to receive messages.
    rx: mpsc::Receiver<Message>,
}

impl UnixDomainSocket {
    /// Default constructor. Receives a [`PathBuf`] and never fails, because no
    /// [`UnixListener::bind`] is performed.
    pub fn new(path: PathBuf) -> Self {
        let state = State::Closed;
        let state = RwLock::new(state);
        let state = Arc::new(state);

        let (tx, rx) = mpsc::channel();

        UnixDomainSocket {
            path,
            state,
            tx,
            rx,
        }
    }

    /// Returns the current [`State`] of the socket.
    ///
    /// It's fallible in case the inner [`RwLock`] is poisoned.
    ///
    /// Also, it's blocking, if there is a concurrent read/write for the state.
    pub fn get_state(&self) -> Result<State, IoError> {
        self.state
            .read()
            .map_err(|e| IoError::new(io::ErrorKind::Other, e.to_string()))
            .and_then(|state| Ok(state.clone()))
    }

    /// Set the current [`State`] of the socket.
    ///
    /// Same constraints as [`UnixDomainSocket::get_state`]
    pub fn set_state(&self, state: State) -> Result<(), IoError> {
        self.state
            .write()
            .map_err(|e| IoError::new(io::ErrorKind::Other, e.to_string()))
            .and_then(|mut self_state| Ok(*self_state = state))
    }

    /// Check if the current state is [`State::Listening`].
    ///
    /// Same constraints as [`UnixDomainSocket::get_state`]
    pub fn is_listening(&self) -> Result<bool, IoError> {
        self.get_state().and_then(|state| Ok(state as u8 == 0x01))
    }

    /// Will be called by the [`mpsc`] channel to process any incoming [`Message`].
    pub fn receive_message(&self, message: Message) -> Result<(), IoError> {
        match message {
            Message::ChangeState(state) => self.set_state(state),
        }
    }

    /// Will remove the [`UnixDomainSocket::path`], if it exists, so it cant bind properly to that
    /// location.
    ///
    /// While the state is [`State::Listening`], listen for incoming data.
    ///
    /// Should it receive a [`Message`] with [`State::ShouldQuit`], it will interrupt the loop only after the next
    /// iteration, because the message is handled asynchronously.
    ///
    /// If the socket fails, the loop wont be interrupted, and the error will be reported to the
    /// log facade.
    ///
    /// Minimal example:
    /// ```
    /// use dusk_uds::UnixDomainSocket;
    ///
    /// UnixDomainSocket::from("/dev/null").bind(move |_stream, _sender| {
    ///     // Code goes here
    /// }).unwrap_or(());
    /// ```
    pub fn bind<F>(&self, handler: F) -> Result<(), IoError>
    where
        F: FnOnce(UnixStream, mpsc::Sender<Message>),
        F: Send + 'static,
        F: Copy + 'static,
    {
        if self.path.as_path().exists() {
            fs::remove_file(self.path.as_path())?;
        }

        let path = self
            .path
            .to_str()
            .map(|p| Ok(p))
            .unwrap_or(Err(IoError::new(
                io::ErrorKind::Other,
                "Invalid path returned by the buffer",
            )))?;

        UnixListener::bind(path).and_then(|listener| {
            info!("UnixDomainSocket bound on {}", path);
            self.set_state(State::Listening)?;

            for socket in listener.incoming() {
                let tx = self.tx.clone();

                socket
                    .and_then(|s| Ok(thread::spawn(move || handler(s, tx))))
                    .map(|_| ())
                    .map_err(|e| error!("{}", e))
                    .unwrap_or(());

                for msg in self.rx.try_iter() {
                    self.receive_message(msg)?;
                }

                if !self.is_listening()? {
                    break;
                }

                debug!("Thread spawned");
            }

            Ok(info!("UnixDomainSocket closed"))
        })
    }
}

impl<T: ToString> From<T> for UnixDomainSocket {
    fn from(path: T) -> Self {
        let path = PathBuf::from(path.to_string());
        UnixDomainSocket::new(path)
    }
}
