use std::{marker::PhantomData, pin::{self, Pin}, sync::Arc};

use futures::{future::join_all, FutureExt};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use super::super::task::{Executor, InstructionExecutor};

pub struct JoinAllInstruction<Input, ClosureOutput> {
    batch: Vec<Input>,
    _phantom: PhantomData<ClosureOutput>,
}

impl<Input, ClosureOutput> JoinAllInstruction<Input, ClosureOutput> {
    pub fn push(&mut self, item: Input) {
        self.batch.push(item);
    }

    pub fn push_many(&mut self, items: impl IntoIterator<Item = Input>) {
        self.batch.extend(items);
    }
}

impl<Input, ProcessOutput> InstructionExecutor<Input, ProcessOutput, Vec<ProcessOutput>> for JoinAllInstruction<Input, ProcessOutput>
where
    Input: Send + 'static,
    ProcessOutput: Send + 'static,
{
    fn new() -> Self {
        Self {
            batch: Vec::new(),
            _phantom: PhantomData,
        }
    }

    fn execute<Preprocessed>(
        self,
        preprocessed: Preprocessed,
        process: Arc<Box<dyn Fn(Preprocessed, Input) -> Pin<Box<dyn Future<Output = ProcessOutput> + Send + 'static>> + Send + Sync + 'static>>,
    ) -> impl Future<Output = Vec<ProcessOutput>> + Send
    where
        Preprocessed: Clone + Send  + 'static,
        // F: Fn(Preprocessed, Input) -> Fut + Send + Sync + 'static,
        // Fut: Future<Output = ProcessOutput> + Send + 'static,
    {
        let futures = self
            .batch
            .into_iter()
            .map(move |item| {
                let preprocessed_clone = preprocessed.clone();
                // process(preprocessed_clone, item).boxed()

                process(preprocessed_clone, item).boxed()
            });

        async move {
            join_all(futures).await
        }
    }
}