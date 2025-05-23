use std::pin::Pin;

use futures::stream::FuturesOrdered;

use crate::process_manager::task::InstructionExecutor;

pub struct StreamInstruction<Input: Send + 'static> {
    batch: Vec<Input>
}

impl<Input: Send + 'static> StreamInstruction<Input> {
    pub fn new() -> Self {
        Self { batch: Vec::new() }
    }

    pub fn push(&mut self, item: Input) {
        self.batch.push(item);
    }

    pub fn push_many(&mut self, items: impl IntoIterator<Item = Input>) {
        self.batch.extend(items);
    }
}

impl<Input: Send + 'static, Output: Send + 'static, PP: Clone + Send + 'static> InstructionExecutor<Input, Output, FuturesOrdered<Pin<Box<dyn Future<Output = Output> + Send>>>, PP> for StreamInstruction<Input> {
    fn execute(
        self,
        preprocessed: PP,
        process: crate::process_manager::proc_manager_handle::UserProcess<Input, Output, PP>,
    ) -> impl Future<Output = FuturesOrdered<Pin<Box<dyn Future<Output = Output> + Send>>>> + Send 
    {
        let stream = self
            .batch
            .into_iter()
            .map(move |item| {
                let pre = preprocessed.clone();
                process(pre, item)
            })
            .collect::<FuturesOrdered<_>>();


        async move { stream }
    }
}