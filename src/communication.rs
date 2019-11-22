use std::os::unix::net::UnixStream;

/// Queable tasks
pub enum Task {
    /// Worker inter communication
    Message(Message),
    /// Incoming socket from the UDS provider
    Socket(UnixStream),
}

#[derive(Debug, Clone, PartialEq)]
/// Output of the future
pub enum Message {
    /// Should not receive further requests and quit after current queue is processed
    ShouldQuit,
    /// Execution with no errors
    Success,
}
