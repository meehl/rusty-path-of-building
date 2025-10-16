use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

pub struct WorkerPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

impl WorkerPool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let (tx, rx) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, rx.clone()));
        }

        Self {
            workers,
            sender: Some(tx),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        drop(self.sender.take());

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
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv();

                match message {
                    Ok(job) => job(),
                    Err(_) => break,
                }
            }
        });

        Self {
            _id: id,
            thread: Some(thread),
        }
    }
}
