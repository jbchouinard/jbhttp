use std::fmt;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
    NewJob(Job),
    Terminate,
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
}

pub struct ExecutionError {
    message: String,
}

impl ExecutionError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Thread execution error: {}", &self.message)
    }
}

impl From<mpsc::SendError<Message>> for ExecutionError {
    fn from(send_error: mpsc::SendError<Message>) -> Self {
        let message = match send_error.0 {
            Message::NewJob(..) => "failed to send job message",
            Message::Terminate => "failed to send termination message",
        };
        ExecutionError::new(message)
    }
}

impl ThreadPool {
    /// ThreadPool manages a pool of worker threads where Jobs can be executed.
    ///
    /// # Arguments
    /// * `size`: number of worker threads in pool
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (0..size)
            .map(|id| Worker::new(id, Arc::clone(&receiver)))
            .collect();
        ThreadPool { workers, sender }
    }

    pub fn execute<F>(&self, f: F) -> Result<(), ExecutionError>
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(Message::NewJob(job))?;
        Ok(())
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &self.workers {
            self.sender.send(Message::Terminate).unwrap();
        }

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();

            match message {
                Message::NewJob(job) => {
                    // TODO: catch and pass errors back
                    job();
                }
                Message::Terminate => {
                    break;
                }
            }
        });

        Worker {
            _id: id,
            thread: Some(thread),
        }
    }
}
