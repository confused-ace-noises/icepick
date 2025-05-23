use futures::future::select_all;
use crate::process_manager::proc_manager_handle::UserProcess;

use super::super::task::InstructionExecutor;

// TODO: FIXME: not fair, the first of the vector almost always wins
pub struct RaceInstruction<Input: Send + 'static>(Vec<Input>);

impl<I: Send + 'static, CO: Send + 'static, PP: Send + Clone + 'static> InstructionExecutor<I, CO, Option<CO>, PP> for RaceInstruction<I> {
    fn execute(
        self,
        preprocessed: PP,
        process: UserProcess<I, CO, PP>
    ) -> impl Future<Output = Option<CO>> + Send
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
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, value: I) {
        self.0.push(value);
    }

    pub fn push_many(&mut self, values: impl Iterator<Item = I>) {
        self.0.extend(values);
    }
}
