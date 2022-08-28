use std::{
    thread::{self, Builder}, 
    sync::{mpsc, Arc, Mutex},
};

#[derive(Debug)]
pub struct ThreadPool {
    workers: Vec<Worker>,
    /// Functions as the queue of jobs
    sender: Option<mpsc::Sender<Job>>,
}

/// Type alias for a trait object that holds the type of closure execute recives
type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    /// Creates a new ThreadPool.
    /// 
    /// Size is the number of threads in the pool.
    /// 
    /// # Returns
    /// On success this will return a new instance of a ThreadPool.
    /// On error this will return a PoolCreationError.
    pub fn build(size: usize) -> Result<ThreadPool, Box<dyn std::error::Error>> {
        // Return error if the pool was attempted to be created with no threads
        if size == 0 { 
            return Err(
                String::from("Tried to create a pool with 0 threads").into()
            ) 
        }

        // Create a channel
        let (sender, receiver) = mpsc::channel();

        // Create a reciver that is contained inside a Mutex contained in a 
        // Arc, the mutex is so we dont run into any race conditions, and the Arc
        // is so we can share the receiver through multiple threads.
        let receiver = Arc::new(Mutex::new(receiver));

        // Create the vector of threads, with a preallocated size
        let mut workers = Vec::with_capacity(size);

        // Create the workers and push them onto the workers vector
        for i in 0..size {
            // thread::spawn cannot be used here since it expects to get some 
            // code that the thread should run immediately. But we just want to
            // create threads and have them wait until we create code later.
            workers.push(Worker::new(i, Arc::clone(&receiver))?);
        }

        // All the workers have now been created and are waiting for jobs

        Ok(ThreadPool { workers, sender: Some(sender) })
    }

    /// Sends the job you want to execute through the sender
    pub fn execute<F>(&self, f: F)
    where
        // FnOnce() since that is what the Thread::spawn function uses as its 
        // trait bound for the closure argument it takes in, and we will be 
        // passing this closure into Thread::spawn. Next we have Send as a trait
        // bound because we need to transfer the closure from one thread to 
        // another. Lastly we have 'static because we dont know how long the
        // thread will take to execute.
        F: FnOnce() + Send + 'static
    {
        // Create a new job instance using the closure provided
        let job = Box::new(f);

        // Send the job down the channel where it will then be recived and
        // executed by one of the workers (threads)
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Explicitly drop sender, this will close the channel, which indicates
        // no more messages will be sent.
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            // Call take on the Option value holding the JoinHandle to move 
            // thread out of worker.
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

#[derive(Debug)]
struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, recieiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Result<Worker, Box<dyn std::error::Error>> {
        let builder = Builder::new();
        let thread = builder.spawn(move || loop {
            let message = recieiver
                // Block the current thread until we can 
                // aquire the mutex this mutex is so we
                // can access the receiver (this is behind
                // a mutex since channels can only have a
                // single consumer)
                .lock().unwrap()
                // Wait to receive a job from the channel
                // Jobs are sent down the channel from 
                // ThreadPool::execute
                .recv();
                
            match message { 
                // Channel is still running (got job)
                Ok(job) => {
                    println!(
                        "Worker {id} got a job; executing"
                    );
                    // Execute the job closure "extracted" 
                    // above.
                    job();
                },
                // Channel was shut down
                Err(_) => { 
                    println!(
                        "Worker {id} dissconnected; shutting down"
                    );
                    break;
                }
            }
        })?;

        Ok(Worker {
            id,
            thread: Some(thread),
        })
    }
}