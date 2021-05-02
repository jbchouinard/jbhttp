//! Runners implement threading strategies for Servers.
use std::thread;

use log::error;

use threadpool::ThreadPool;

mod threadpool;

pub struct SimpleRunner;

impl SimpleRunner {
    pub fn run<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        f();
    }
}

pub struct ThreadRunner {
    threads: Vec<Option<thread::JoinHandle<()>>>,
}

impl Default for ThreadRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadRunner {
    pub fn new() -> Self {
        Self { threads: vec![] }
    }

    pub fn run<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.threads.push(Some(thread::spawn(f)));
    }
}

impl Drop for ThreadRunner {
    fn drop(&mut self) {
        for thread in &mut self.threads {
            if let Some(thread) = thread.take() {
                match thread.join() {
                    Ok(_) => (),
                    Err(e) => error!("Error joining thread: {:?}", e),
                }
            }
        }
    }
}

pub struct ThreadPoolRunner {
    threadpool: ThreadPool,
}

impl ThreadPoolRunner {
    pub fn new(pool_size: usize) -> Self {
        Self {
            threadpool: ThreadPool::new(pool_size),
        }
    }
    pub fn run<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        match self.threadpool.execute(f) {
            Ok(_) => (),
            Err(e) => error!("thread pool error: {}", e),
        }
    }
}

pub enum Runner {
    Simple(SimpleRunner),
    Thread(ThreadRunner),
    ThreadPool(ThreadPoolRunner),
}

impl Runner {
    pub fn run<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        match self {
            Self::Simple(runner) => runner.run(f),
            Self::Thread(runner) => runner.run(f),
            Self::ThreadPool(runner) => runner.run(f),
        }
    }

    /// Create a new runner using the specified number of threads.
    /// 0 is infinite, a new thread will be created for each job.
    /// 1 runs in the main thread.
    /// Any other number creates a thread pool of the specified size.
    pub fn new(n_threads: usize) -> Self {
        match n_threads {
            0 => Self::Thread(ThreadRunner::new()),
            1 => Self::Simple(SimpleRunner),
            n => Self::ThreadPool(ThreadPoolRunner::new(n)),
        }
    }
}
