use crate::{Message, Task, TaskProvider};

use std::sync::{mpsc, Arc, Mutex};

use futures::executor::block_on;

/// This function will panic if the task channel is broken.
///
/// There is no point in preserving the event loop in case there is no available channel.
///
/// Therefore, this function should be called from a thread.
pub fn worker<T: TaskProvider>(
    tx: mpsc::Sender<Task>,
    rx: Arc<Mutex<mpsc::Receiver<Task>>>,
    provider: T,
) {
    loop {
        let task = rx
            .lock()
            .map_err(|e| {
                error!("Error trying to lock the task channel: {}", e);
                e
            })
            .unwrap()
            .recv()
            .map_err(|e| {
                error!(
                    "Error trying to receive a task from the respective channel: {}",
                    e
                );
                e
            })
            .unwrap();

        match task {
            Task::Socket(stream) => {
                let mut p = provider.clone();

                p.set_socket(stream);

                // TODO - Naive implementation, will not reschedule if the poll returns pending
                let message = block_on(p);

                if Message::ShouldQuit == message {
                    tx.send(Task::Message(Message::ShouldQuit))
                        .map_err(|e| {
                            error!(
                                "Error trying to send a ShouldQuit message to the task channel: {}",
                                e
                            );
                            e
                        })
                        .unwrap();
                }
            }

            Task::Message(m) if m == Message::ShouldQuit => {
                tx.send(Task::Message(Message::ShouldQuit))
                    .map_err(|e| {
                        error!(
                            "Error trying to send a ShouldQuit message to the task channel: {}",
                            e
                        );
                        e
                    })
                    .unwrap();
                break;
            }

            _ => (),
        }
    }
}
