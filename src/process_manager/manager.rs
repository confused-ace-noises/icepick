use core::panic;
use std::{
    collections::VecDeque, sync::Arc, time::Duration
};

use futures::future::join_all;
use tokio::sync::{
    mpsc::{self, Receiver, Sender}, Mutex
};

enum Task {
    PoliteKill,
    ForceKill,
    Resume(usize),
}

enum Status {
    Idle,
    Processing,
} impl Status {
    fn is_idle(&self) -> bool {
        if let Status::Idle = self {
            true
        } else {
            false
        }
    }

    fn is_processing(&self) -> bool {
        if let Status::Processing = self {
            true
        } else {
            false
        }
    }
}

// made this! :D
pub struct Manager<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
    // F: Fn(I) -> Fut + Send + Sync + 'static,
    // Fut: Future<Output = O> + Send + 'static,
{
    // event_pool: Arc<Mutex<VecDeque<Task>>>,
    task_sender: Sender<Task>,
    to_process_pool: Arc<Mutex<VecDeque<I>>>,
    info_receiver: Receiver<O>,
    process_status: Arc<Mutex<Status>>
}

impl<I, O> Manager<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
    
{
    pub const STANDARD_BUFFER: usize = 100;
    pub const STANDARD_POLL_DELAY_MS: u64 = 400;

    pub fn new_with_buffer<F, Fut>(buffer: usize, process_fn: F) -> Self 
    where 
        F: Fn(I) -> Fut + Send + 'static,
        Fut: Future<Output = O> + Send + 'static,
    {
        let (info_sender, info_receiver) = mpsc::channel::<O>(buffer);
        let (event_sender, mut event_receiver) = mpsc::channel::<Task>(1);

        let to_process_pool = Arc::new(Mutex::new(VecDeque::<I>::new()));
        let to_process_pool_to_move = Arc::clone(&to_process_pool);

        let status = Arc::new(Mutex::new(Status::Idle));
        let status_out = Arc::clone(&status);
        tokio::spawn(async move {
            let status_clone = Arc::clone(&status);
            loop {
                if let Some(task) = event_receiver.recv().await {

                    let mut status_lock = status_clone.lock().await;
                    *status_lock = Status::Processing;
                    drop(status_lock);

                    match task {
                        // TODO: Don't Repeat Yourself
                        Task::Resume(advance_of) => {
                            
                            let process_pool = Arc::clone(&to_process_pool_to_move);
                            let mut process_pool_lock = process_pool.lock().await;
                            let process_pool_len = process_pool_lock.len();

                            let batch = process_pool_lock.drain(..advance_of.min(process_pool_len)).collect::<Vec<_>>();

                            drop(process_pool_lock);

                            let futures = batch.into_iter().map(|x| process_fn(x)).collect::<Vec<_>>();

                            let joined = join_all(futures).await;

                            // *                 rev because that way it retruns them the right order
                            let to_send = joined.into_iter().rev().map(|sendee| info_sender.send(sendee));
                            let sent= join_all(to_send).await; // use these results somehow?
                            
                            if sent.iter().any(|results| results.is_err()) {
                                panic!("Internal fatal error; resume internal execution: receiver of batch of results must still be alive at time of sending");
                            }
                        },

                        Task::PoliteKill => {
                            let process_pool = Arc::clone(&to_process_pool_to_move);
                            let mut process_pool_lock = process_pool.lock().await;

                            let batch = process_pool_lock.drain(..).collect::<Vec<_>>();

                            // drop(process_pool_lock); // don't drop the lock to avoid any more addings

                            let futures = batch.into_iter().map(|x| process_fn(x)).collect::<Vec<_>>();

                            let joined = join_all(futures).await;

                            let to_send = joined.into_iter().rev().map(|sendee| info_sender.send(sendee));
                            let sent= join_all(to_send).await; // use these results somehow?
                            
                            if sent.iter().any(|results| results.is_err()) {
                                panic!("Internal fatal error; resume internal execution: receiver of batch of results must still be alive at time of sending");
                            }
                        }

                        Task::ForceKill => {
                            break;
                        }
                    }
                } else {
                    let mut status_lock = status_clone.lock().await;
                    *status_lock = Status::Processing;

                    drop(status_lock);
                }
            }
        });

        Self { task_sender: event_sender, to_process_pool, info_receiver, process_status: status_out}
    }

    pub fn new<F, Fut>(process_fn: F) -> Self 
    where 
        F: Fn(I) -> Fut + Send + 'static,
        Fut: Future<Output = O> + Send + 'static,
    {
        Self::new_with_buffer(Self::STANDARD_BUFFER, process_fn)
    }

    pub async fn push(&mut self, to_push: I) {
        let pool_clone = Arc::clone(&self.to_process_pool);
        let mut pool_lock = pool_clone.lock().await;

        pool_lock.push_back(to_push);
    }

    pub async fn push_many(&mut self, to_push: impl Iterator<Item = I>) {
        let pool_clone = Arc::clone(&self.to_process_pool);
        let mut pool_lock = pool_clone.lock().await;

        to_push.into_iter().for_each(|pushee| pool_lock.push_back(pushee));
    }

    pub async fn resume(&self, to_resume: usize) -> usize {
        let pool_lock = self.to_process_pool.lock().await;
        let len = pool_lock.len();

        drop(pool_lock);

        self.task_sender.send(Task::Resume(to_resume)).await.expect("Internal fatal error; resume: receiver of resume task must still be alive at time of sending");

        to_resume.min(len)
    }

    pub async fn resume_all(&self) -> usize {
        let pool_lock = self.to_process_pool.lock().await;
        let len = pool_lock.len();

        drop(pool_lock);

        self.task_sender.send(Task::Resume(len)).await.expect("Internal fatal error; resume: receiver of resume task must still be alive at time of sending");

        len
    } 
    
    pub async fn poll(&mut self, to_receive: usize) -> Vec<O> {
        let mut buffer = Vec::with_capacity(to_receive);
        self.info_receiver.recv_many(&mut buffer, to_receive).await; //should i return the result of this?

        buffer
    }

    pub async fn poll_available(&mut self) -> Vec<O> {
        let mut buffer = Vec::new();

        while let Some(output) = self.info_receiver.recv().await {
            buffer.push(output);
        }

        buffer
    }

    pub async fn poll_drain(&mut self) -> Vec<O> {
        self.wait_until(|mng| async {mng.process_status.lock().await.is_idle()}).await;

        self.poll_available().await
    }

    pub async fn wait_until<'a, B, Fut>(&'a self, func: B)
    where
        B: Fn(&'a Self) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'a,
    {
        self.wait_until_interval(Self::STANDARD_POLL_DELAY_MS, func).await
    }

    pub async fn wait_until_interval<'a, B, Fut>(&'a self, polling_interval: u64, func: B)
    where
        B: Fn(&'a Self) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'a,
    {
        let mut ticker = tokio::time::interval(Duration::from_millis(polling_interval));

        loop {
            ticker.tick().await;

            if func(self).await {
                break;
            }
        }
    }

    pub async fn kill_polite(mut self) -> Vec<O> {
        self.task_sender.send(Task::PoliteKill).await.expect("Internal fatal error; kill: receiver of kill task must still be alive at time of sending");

        let output = self.poll_drain().await;
        self.kill_forced().await;
        output
    }

    pub async fn kill_forced(self) {
        self.task_sender.send(Task::ForceKill).await.expect("Internal fatal error; kill: receiver of kill task must still be alive at time of sending");
    }
}

impl<I, O> Drop for Manager<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    fn drop(&mut self) {
        let sender = self.task_sender.clone();
        tokio::spawn(async move {
            let _ = sender.send(Task::ForceKill).await;
        });
    }
}