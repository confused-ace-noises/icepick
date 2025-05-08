use std::sync::Arc;

use futures::{StreamExt, channel::oneshot, future::select_all, stream::FuturesUnordered};
use tokio::sync::mpsc::Sender;

use super::super::task::{Executor, InstructionExecutor};

// TODO: FIXME: not fair, the first of the vector almost always wins
pub struct RaceInstruction<Input: Send + 'static>(Vec<Input>);

impl<I: Send + 'static, CO> InstructionExecutor<I, CO, Option<CO>> for RaceInstruction<I> {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn execute<Preprocessed>(
        self,
        preprocessed: Preprocessed,
        process: Arc<
            Box<
                dyn Fn(
                        Preprocessed,
                        I,
                    )
                        -> std::pin::Pin<Box<dyn Future<Output = CO> + Send + 'static>>
                    + Send
                    + Sync
                    + 'static,
            >,
        >,
    ) -> impl Future<Output = Option<CO>> + Send
    where
        Preprocessed: Clone + Send + 'static,
    {
        // let mut unordered_futs = FuturesUnordered::new();
        // unordered_futs.extend(self.0.into_iter().map(|input| {
        //     let cloned_preprocessed = preprocessed.clone();
        //     process(cloned_preprocessed, input)
        // }));

        async move {
            match self.0.len() {
                0 => None,
                _ => Some(
                    select_all(self.0.into_iter().map(|i| process(preprocessed.clone(), i))).await.0,
                ),
            }
        }
    }
}

impl<I: Send + 'static> RaceInstruction<I> {
    pub fn push(&mut self, value: I) {
        self.0.push(value);
    }

    pub fn push_many(&mut self, values: impl Iterator<Item = I>) {
        self.0.extend(values);
    }
}
