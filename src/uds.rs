use crate::{worker::worker, Options, Task, TaskProvider};

use std::{
    fs,
    io::{self, Error as IoError},
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
};

/// Boilerplate for [`UnixListener`].
///
/// Will receive a path to bind to, a set of options and an implementation of future that will
/// handle the incoming sockets.
pub struct UnixDomainSocket<T: TaskProvider + 'static> {
    path: PathBuf,
    options: Options,
    provider: T,
}

impl<T: TaskProvider> UnixDomainSocket<T> {
    /// Default constructor.
    pub fn new<P: Into<PathBuf>>(path: P, options: Option<Options>, provider: T) -> Self {
        let path = path.into();
        let options = options.unwrap_or(Options::default());

        UnixDomainSocket {
            path,
            options,
            provider,
        }
    }

    /// Will remove the [`UnixDomainSocket::path`], if it exists, so it cant bind properly to that
    /// location.
    ///
    /// If the future returns a [`crate::Message::ShouldQuit`], the worker threads will be finished after
    /// the current queue of sockets and the main loop will end.
    pub fn bind(self) -> Result<(), IoError> {
        // Grant the provided path is available to the process
        if self.path.as_path().exists() {
            fs::remove_file(self.path.as_path())?;
        }

        // Prepare the path to bind
        let path = self
            .path
            .to_str()
            .map(|p| Ok(p))
            .unwrap_or(Err(IoError::new(
                io::ErrorKind::Other,
                "Invalid path returned by the buffer",
            )))?;

        // Create the task queue channel that will be share amongst the worker threads
        let (tx, rx) = mpsc::channel();
        let rx = Mutex::new(rx);
        let rx = Arc::new(rx);

        // Perform the bind
        let listener = UnixListener::bind(path)?;
        info!("UnixDomainSocket bound on {}", path);

        // Spawn the workers, each opne with an ownership to the queue channel, and the future
        // provider
        let workers: Vec<thread::JoinHandle<_>> = (0..self.options.workers)
            .map(|_| {
                let t = tx.clone();
                let r = Arc::clone(&rx);
                let p = self.provider.clone();

                thread::spawn(move || worker(t, r, p))
            })
            .collect();

        // Spawn a thread to perform the actual listening.
        //
        // When there is an incoming socket, transform it to a Task and send to the queue channel
        let t = tx.clone();
        thread::spawn(move || {
            for socket in listener.incoming() {
                socket
                    .and_then(|s| {
                        t.send(Task::Socket(s))
                            .map_err(|e| IoError::new(io::ErrorKind::Other, e))
                    })
                    .unwrap_or_else(|e| {
                        error!("Error receiving the UDS socket: {}", e);
                    });
            }
        });

        // Wait until all the workers are finished
        for w in workers {
            w.join().unwrap_or_else(|e| {
                error!("Error ending the worker thread gracefully: {:?}", e);
            });
        }

        Ok(info!("Unbinding UDS"))
    }
}
