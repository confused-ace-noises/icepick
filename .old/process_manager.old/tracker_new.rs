use std::{env::var, sync::Arc};
use tokio::sync::RwLock;

use dashmap::DashMap;
use uuid::Uuid;

use super::manager::ProcessManager;

pub struct BatchTracker(Vec<Uuid>);

pub struct Tracked<T> {
    id: Uuid,
    value: T,
}
impl<T> Tracked<T> {
    pub fn new(item: T) -> Self {
        Self {
            id: Uuid::now_v7(),
            value: item,
        }
    }
}

pub struct ProcessTracker<I, O>
where
    I: Send + 'static,
    O: Send + Sync + 'static,
{
    proc_manager: ProcessManager<Tracked<I>, ()>,
    processed_pool: Arc<DashMap<Uuid, O>>,
    current_batch_tracker: Arc<RwLock<BatchTracker>>,
}

impl<I, O> ProcessTracker<I, O>
where
    I: Send + 'static,
    O: Send + Sync + 'static,
{
    pub async fn new_with_buffer_with_preprocessed_with_wrapper<
        Pre,
        ProcFunc,
        ProcOut,
        Wrap,
        WrapFn,
    >(
        preprocessed: Pre,
        wrapper_func: WrapFn,
        mut process: ProcFunc,
        buffer: usize,
    ) -> Self
    where
        Wrap: Send + Clone,
        WrapFn: FnOnce(Pre) -> Wrap + Send + 'static,
        Pre: Send + Sync + 'static,
        ProcFunc: FnMut(Wrap, I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        let done_map = Arc::new(DashMap::new());
        let done_map_clone = Arc::clone(&done_map);

        let proc_manager =
            ProcessManager::<Tracked<I>, ()>::new_with_buffer_with_preprocessed_with_wrapper(
                preprocessed,
                wrapper_func,
                move |x, y| {
                    let output = process(x, y.value);

                    {
                        let done_map_clone = Arc::clone(&done_map_clone);
                        async move {
                            let tracked_id = y.id;
                            let out = output.await;

                            done_map_clone.insert(tracked_id, out);
                        }
                    }
                },
                buffer,
            )
            .await;

        Self {
            proc_manager,
            processed_pool: done_map,
            current_batch_tracker: Arc::new(RwLock::new(BatchTracker(vec![])))
        }
    }

    pub async fn new_with_preprocessed_with_wrapper<
        Pre,
        ProcFunc,
        ProcOut,
        Wrap,
        WrapFn,
    >(
        preprocessed: Pre,
        wrapper_func: WrapFn,
        mut process: ProcFunc,
    ) -> Self
    where
        Wrap: Send + Clone,
        WrapFn: FnOnce(Pre) -> Wrap + Send + 'static,
        Pre: Send + Sync + 'static,
        ProcFunc: FnMut(Wrap, I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send, 
    {
        Self::new_with_buffer_with_preprocessed_with_wrapper(preprocessed, wrapper_func, process, ProcessManager::<I, O>::BUFFER_SIZE).await
    }

    pub async fn new_with_preprocessed_no_wrapper<Pre, ProcFunc, ProcOut>(
        preprocessed: Pre,
        process: ProcFunc,
    ) -> Self
    where
        Pre: Send + Sync + 'static,
        ProcFunc: FnMut(Arc<RwLock<Pre>>, I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        Self::new_with_buffer_with_preprocessed_with_wrapper(preprocessed, |pre| Arc::new(RwLock::new(pre)),process, ProcessManager::<I, O>::BUFFER_SIZE).await
    }

    pub async fn new_with_buffer<ProcFunc, ProcOut>(buffer: usize, mut process: ProcFunc) -> Self
    where
        ProcFunc: FnMut(I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        Self::new_with_buffer_with_preprocessed_with_wrapper((), |_| (), move |_, y| process(y) , buffer).await
    }

    pub async fn new_simple<ProcFunc, ProcOut>(process: ProcFunc) -> Self
    where
        ProcFunc: FnMut(I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        Self::new_with_buffer(ProcessManager::<I, O>::BUFFER_SIZE, process).await
    }


    pub async fn push(&self, value: I) {
        let tracked = Tracked::new(value);
        let mut x = self.current_batch_tracker.write().await;
        x.0.push(tracked.id);
        drop(x);
        self.proc_manager.push(tracked).await; 
    }

    pub async fn push_many(&self, value: impl Iterator<Item = I>) {
        let tracked: (Vec<_>, Vec<_>) = value.map(|v| {let t = Tracked::new(v); (t.id, t.value)}).unzip();
        
        let ids = tracked.0.clone();
        let mut x = self.current_batch_tracker.write().await;
        x.0.extend(ids);
        drop(x);

        let finalized_tracked = tracked.0.into_iter().zip(tracked.1.into_iter()).map(|(id, value)| Tracked { id, value });
        self.proc_manager.push_many(finalized_tracked).await 
    }

    pub async fn process(&self) -> BatchTracker {
        let mut x = self.current_batch_tracker.write().await;
        let batch = std::mem::take(&mut x.0);
        drop(x);

        self.proc_manager.process().await;
        BatchTracker(batch)
    }

    pub fn get(&self, tracker: BatchTracker) -> Vec<O> {
        tracker.0.into_iter().filter_map(|x| self.processed_pool.remove(&x).and_then(|y| Some(y.1))).collect()
    }

    pub async fn exec(&self, input: I) -> O {
        let tracked = Tracked::new(input);
        let id = tracked.id;
        
        let mut recv = self.proc_manager.exec(tracked).await.expect("expected channel to be open, it is closed. `exec()` in ProcessTracker");

        recv.recv().await;

        self.processed_pool.remove(&id).expect("expected value of output to be in pool by now; `exec()` in ProcessTracker").1
    }
}
