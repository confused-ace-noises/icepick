#![warn(clippy::all)]
#![warn(clippy::pedantic)]

use std::marker::PhantomData;

use futures::{future::join_all, FutureExt};

use crate::process_manager::proc_manager_handle::UserProcess;

use super::super::task::InstructionExecutor;

pub struct JoinAllInstruction<Input, ClosureOutput> {
    batch: Vec<Input>,
    _phantom: PhantomData<ClosureOutput>,
}

impl<Input, ClosureOutput> JoinAllInstruction<Input, ClosureOutput> {
    pub fn new() -> Self {
        Self {
            batch: Vec::new(),
            _phantom: PhantomData
        }
    }

    pub fn push(&mut self, item: Input) {
        self.batch.push(item);
    }

    pub fn push_many(&mut self, items: impl IntoIterator<Item = Input>) {
        self.batch.extend(items);
    }
}

impl<Input, ProcessOutput, PP> InstructionExecutor<Input, ProcessOutput, Vec<ProcessOutput>, PP> for JoinAllInstruction<Input, ProcessOutput>
where
    Input: Send + 'static,
    ProcessOutput: Send + 'static,
    PP: Send + Clone + 'static
{
    fn execute(
        self,
        preprocessed: PP,
        process: UserProcess<Input, ProcessOutput, PP>
    ) -> impl Future<Output = Vec<ProcessOutput>> + Send
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