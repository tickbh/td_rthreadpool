extern crate libc;

use std::sync::mpsc::{channel, Sender, Receiver, SyncSender, sync_channel, RecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

mod mutex;
pub use mutex::ReentrantMutex;

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

type Thunk<'a> = Box<FnBox + Send + 'a>;

enum Message {
    NewJob(Thunk<'static>),
    Join,
}

pub struct ThreadPool {
    threads      : Vec<ThreadData>,
    job_sender   : Sender<Message>,
    job_receiver : Arc<Mutex<Receiver<Message>>>,
    active_count: Arc<Mutex<usize>>,
    max_count: Arc<Mutex<usize>>,
}


struct ThreadData {
    _thread_join_handle: JoinHandle<()>,
    pool_sync_rx: Receiver<()>,
    thread_sync_tx: SyncSender<()>,
}

fn create_thread(job_receiver : Arc<Mutex<Receiver<Message>>>, active_count: Arc<Mutex<usize>>) -> ThreadData {
    let job_receiver = job_receiver.clone();
    let (pool_sync_tx, pool_sync_rx) =
        sync_channel::<()>(0);
    let (thread_sync_tx, thread_sync_rx) =
        sync_channel::<()>(0);

    let thread = thread::spawn(move || {
        loop {
            let message = {
                // Only lock jobs for the time it takes
                // to get a job, not run it.
                let lock = job_receiver.lock().unwrap();
                lock.recv()
            };

            match message {
                Ok(Message::NewJob(job)) => {

                    *active_count.lock().unwrap() += 1;
                    job.call_box();
                    *active_count.lock().unwrap() -= 1;
                }
                Ok(Message::Join) => {
                    // Syncronize/Join with pool.
                    // This has to be a two step
                    // process to ensure that all threads
                    // finished their work before the pool
                    // can continue

                    // Wait until the pool started syncing with threads
                    if pool_sync_tx.send(()).is_err() {
                        // The pool was dropped.
                        break;
                    }

                    // Wait until the pool finished syncing with threads
                    if thread_sync_rx.recv().is_err() {
                        // The pool was dropped.
                        break;
                    }
                }
                Err(..) => {
                    // The pool was dropped.
                    break
                }
            }
        }
    });
    ThreadData {
        _thread_join_handle: thread,
        pool_sync_rx: pool_sync_rx,
        thread_sync_tx: thread_sync_tx,
    }
}
impl ThreadPool {
    /// Construct a threadpool with the given number of threads.
    /// Minimum value is `1`.
    pub fn new(n: usize) -> ThreadPool {
        assert!(n >= 1);

        let (job_sender, job_receiver) = channel();
        let job_receiver = Arc::new(Mutex::new(job_receiver));
        let active_count = Arc::new(Mutex::new(0));
        let max_count = Arc::new(Mutex::new(n as usize));
        let mut threads = Vec::with_capacity(n as usize);
        // spawn n threads, put them in waiting mode
        for _ in 0..n {
            let thread = create_thread(job_receiver.clone(), active_count.clone());
            threads.push(thread);
        }

        ThreadPool {
            threads      : threads,
            job_sender   : job_sender,
            job_receiver : job_receiver.clone(),
            active_count : active_count,
            max_count    : max_count,
        }
    }

    /// Returns the number of threads inside this pool.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Executes the function `job` on a thread in the pool.
    pub fn execute<F>(&self, job: F)
        where F : FnOnce() + Send + 'static
    {
        self.job_sender.send(Message::NewJob(Box::new(job))).unwrap();
    }

    pub fn join_all(&self) {
        for _ in 0..self.threads.len() {
            self.job_sender.send(Message::Join).unwrap();
        }

        // Synchronize/Join with threads
        // This has to be a two step process
        // to make sure _all_ threads received _one_ Join message each.

        // This loop will block on every thread until it
        // received and reacted to its Join message.
        let mut worker_panic = false;
        for thread_data in &self.threads {
            if let Err(RecvError) = thread_data.pool_sync_rx.recv() {
                worker_panic = true;
            }
        }
        if worker_panic {
            // Now that all the threads are paused, we can safely panic
            panic!("Thread pool worker panicked");
        }

        // Once all threads joined the jobs, send them a continue message
        for thread_data in &self.threads {
            thread_data.thread_sync_tx.send(()).unwrap();
        }
    }

    /// Returns the number of currently active threads.
    pub fn active_count(&self) -> usize {
        *self.active_count.lock().unwrap()
    }

    /// Returns the number of created threads
    pub fn max_count(&self) -> usize {
        *self.max_count.lock().unwrap()
    }

    /// Sets the number of threads to use as `threads`.
    /// Can be used to change the threadpool size during runtime
    pub fn set_threads(&mut self, threads: usize) -> i32 {
        assert!(threads >= 1);
        if threads <= self.thread_count() {
            return -1;
        }
        for _ in 0 .. (threads - self.thread_count()) {
            let thread = create_thread(self.job_receiver.clone(), self.active_count.clone());
            self.threads.push(thread);
        }

        *self.max_count.lock().unwrap() = threads;
        0
    }

}