use std::{collections::HashMap, sync::Arc};

use crate::process_manager::old_manager::Manager;
use futures::future::join_all;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct Tracked<T> {
    id: Uuid,
    value: T,
}

impl<T> Tracked<T> {
    pub fn new(item: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            value: item
        }
    }
}

pub struct Tracker<I, O> 
where
    I: Send + 'static,
    O: Send + 'static,
{
    manager: Manager<Tracked<I>, Tracked<O>>,
    results_available: Arc<Mutex<HashMap<Uuid, O>>> // TODO: maybe use dashmap here?
}

impl<I, O> Tracker<I, O> 
where
    I: Send + 'static,
    O: Send + 'static,
{
    pub fn new_with_buffer<F, Fut>(buffer: usize, process_fn: F) -> Self 
    where 
        F: Fn(I) -> Fut + Send + 'static,
        Fut: Future<Output = O> + Send + 'static,

    {
        let func = move |tracked: Tracked<I>| {
            let id = tracked.id;
            let value = tracked.value;
            let output = process_fn(value);
            async move { Tracked { id, value: output.await } }
        };

        let manager = Manager::new_with_buffer(buffer, func);
        let results_available = Arc::new(Mutex::new(HashMap::<Uuid, O>::new()));

        Self {
            manager,
            results_available
        }
    }

    pub fn new<F, Fut>(process_fn: F) -> Self 
    where 
        F: Fn(I) -> Fut + Send + 'static,
        Fut: Future<Output = O> + Send + 'static,
    {
        Self::new_with_buffer(Manager::<Tracked<I>, Tracked<O>>::STANDARD_BUFFER, process_fn)
    }

    pub async fn push(&mut self, to_push: I) -> Uuid {
        let tracked = Tracked::new(to_push);
        let tracked_id = tracked.id;
        self.manager.push(tracked).await;

        tracked_id
    }

    pub async fn push_many(&mut self, to_push: impl Iterator<Item = I>) -> Vec<Uuid> {
        let mut tracked_uuids = Vec::new();
        let to_push = to_push.map(|item| {let tracked = Tracked::new(item); tracked_uuids.push(tracked.id); tracked});
        self.manager.push_many(to_push).await;
        
        tracked_uuids
    }

    pub async fn resume(&self, to_resume: usize) -> usize {
        self.manager.resume(to_resume).await
    }

    pub async fn resume_all(&self) -> usize {
        self.manager.resume_all().await
    }

    pub async fn poll(&mut self, to_receive: usize) {
        let output = self.manager.poll(to_receive).await;
    
        let entries = output.into_iter().map(|item| (item.id, item.value));
    
        let map_clone = Arc::clone(&self.results_available);
        let mut map = map_clone.lock().await;
        map.extend(entries);
    }

    pub async fn poll_until(&mut self, found: Uuid) -> O {
        let result: O;
        
        'poll_loop: loop {
            self.poll(1).await;
            if let Some(tracked) = self.take(found).await {
                result = tracked;
                break 'poll_loop;
            }
        }

        result
    }

    pub async fn poll_available(&mut self) {
        let output = self.manager.poll_available().await;
    
        let entries = output.into_iter().map(|item| (item.id, item.value));
    
        let map_clone = Arc::clone(&self.results_available);
        let mut map = map_clone.lock().await;
        map.extend(entries);
    }

    pub async fn poll_drain(&mut self) {
        let output = self.manager.poll_drain().await;
    
        let entries = output.into_iter().map(|item| (item.id, item.value));
    
        let map_clone = Arc::clone(&self.results_available);
        let mut map = map_clone.lock().await;
        map.extend(entries);
    }

    pub async fn take(&self, id: Uuid) -> Option<O> {
        let map_clone = Arc::clone(&self.results_available);
        let mut map = map_clone.lock().await;
        map.remove(&id)
    }

    pub async fn take_many(&self, ids: Vec<Uuid>) -> Vec<Option<O>> {
        let mut map = self.results_available.lock().await;
        ids.into_iter()
            .map(|id| map.remove(&id))
            .collect()
    }

    pub async fn exec(&mut self, input: I) -> Uuid {
        let tracked = Tracked::new(input);
        let id = tracked.id;
        self.manager.exec(tracked).await;

        id
    }
    
    pub async fn submit(&mut self, input: I) -> O {
        let id = self.exec(input).await;
        self.poll_until(id).await
    }

    async fn wait_until_interval<'a, B, Fut>(&'a self, polling_interval: u64, func: B)
    where 
        B: Fn(&'a Manager<Tracked<I>, Tracked<O>>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'a,
    {
        self.manager.wait_until_interval(polling_interval, func).await
    }
    
    async fn wait_until<'a, B, Fut>(&'a self, func: B)
    where 
        B: Fn(&'a Manager<Tracked<I>, Tracked<O>>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'a,
    {
        self.manager.wait_until(func).await
    }

    pub async fn kill_polite(self) -> Vec<Tracked<O>> {
        self.manager.kill_polite().await
    }

    pub async fn kill_forced(self) {
        self.manager.kill_forced().await
    }
}