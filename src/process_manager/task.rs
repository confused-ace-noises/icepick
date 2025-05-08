use std::{marker::PhantomData, pin::Pin, sync::Arc};
use tokio::sync::{mpsc::{error::SendError, Sender}, oneshot};
use futures::Future;
use uuid::Uuid;

/// A task to be processed.
pub struct Task {
    pub id: Uuid,
    fut: Pin<Box<dyn Future<Output = ()> + Send>>
}

pub struct Executor<Instruction, Input, ClosureOutput, TaskOutput>
where 
    Instruction: InstructionExecutor<Input, ClosureOutput, TaskOutput>
{
    id: Uuid,
    instruction: Instruction,
    _phantom: PhantomData<(Input, ClosureOutput, TaskOutput)>
}

/// Trait defining how to create and execute task types.
pub trait InstructionExecutor<Input, ClosureOutput, TaskOutput>: Sized + Send + 'static {
    fn new() -> Self;

    fn execute<Preprocessed>(
        self,
        preprocessed: Preprocessed,
        process: Arc<Box<dyn Fn(Preprocessed, Input) -> Pin<Box<dyn Future<Output = ClosureOutput> + Send +  'static>> + Send + Sync + 'static>>,
    ) -> impl Future<Output = TaskOutput> + Send
    where
        Preprocessed: Clone + Send + 'static;
        // F: Fn(Preprocessed, Input) -> Fut + Send + Sync + 'static,
        // Fut: Future<Output = ClosureOutput> + Send + 'static;
}

// impl<Instr, Input, ClosureOutput, TaskOutput> Task<Instr, Input, ClosureOutput, TaskOutput>
// where
//     Instr: InstructionExecutor<Input, ClosureOutput, TaskOutput>,
// {
//     pub fn new(sender: Arc<Sender<Executor<Instr, Input, ClosureOutput, TaskOutput>>>) -> Self {
//         let id = Uuid::now_v7();
//         let instruction = Instr::new(id, sender.clone());
//         Self {
//             id,
//             sender,
//             instruction,
//         }
//     }

//     pub async fn send(self) -> Result<(), SendError<Executor<Instr, Input, ClosureOutput, TaskOutput>>> {
//         let sender = self.sender.clone();

//         sender.send(self.into()).await
//     }
// }


// impl<T: InstructionExecutor<Input, ClosureOutput, TaskOutput>, Input, ClosureOutput, TaskOutput> From<Task<T, Input, ClosureOutput, TaskOutput>> for Executor<T, Input, ClosureOutput, TaskOutput> {
//     fn from(value: Task<T, Input, ClosureOutput, TaskOutput>) -> Self {
//         Self { id: value.id, instruction: value.instruction, _phantom: PhantomData }
//     }
// }

impl Task {
    pub fn boxed(fut: impl Future<Output = ()> + Send + 'static) -> Self {
        Self {
            id: Uuid::now_v7(),
            fut: Box::pin(fut),
        }
    }

    pub fn build_task<I, Input, CO, TO, PP, /*F, Fut*/>(
        instruction: I,
        preprocessed: PP,
        process: Arc<Box<dyn Fn(PP, Input) -> Pin<Box<dyn Future<Output = CO> + Send + 'static>> + Send + Sync + 'static>>,
    ) -> (Self, oneshot::Receiver<TO>)
    where
        I: InstructionExecutor<Input, CO, TO> + 'static,
        Input: Send + 'static,
        CO: Send + 'static,
        TO: Send + 'static,
        PP: Clone + Send + 'static,
        // F: Fn(PP, Input) -> Fut + Send + Sync + 'static,
        // Fut: Future<Output = CO> + Send + 'static,
    {
        // let id = Uuid::now_v7();
        let (tx, rx) = oneshot::channel();
    
        let fut = async move {
            let result = instruction.execute(preprocessed, process).await;
            let _ = tx.send(result);
        };
    
        (Self::boxed(fut), rx)
    }

    pub(super) async fn run(self) {
        self.fut.await
    } 
}