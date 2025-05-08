use core::task;
use std::{marker::PhantomData, pin::Pin, process::Output, sync::Arc};

use tokio::sync::{mpsc::{Receiver, Sender}, oneshot};
use uuid::Uuid;

use super::task::{Executor, InstructionExecutor, Task};

pub struct ProcManagerHandle<Input, Output, PP> 
where 
    Input: Send + 'static,
    PP: Clone,
    // Proc: Fn(PP, Input) -> Fut + Send + Sync + 'static,
    // Fut: Future<Output = Output> + Send + 'static,
{
    pub(super) id_handle: Uuid,
    pub(super) pre_processed: PP,
    pub(super) process: Arc<Box<dyn Fn(PP, Input) -> Pin<Box<dyn Future<Output = Output> + Send + 'static>> + Send + Sync + 'static>>,
    pub(super) sender: Arc<Sender<Task>>,
    pub(super) _phantom: PhantomData<(Input, Output)>,
}

impl<Input, Output, PP> ProcManagerHandle<Input, Output, PP> 
where 
    Input: Send + 'static,
    Output: Send + 'static,
    PP: Clone + Send + 'static,
    // Proc: Fn(PP, Input) -> Fut + Clone + Send + Sync + 'static, 
    // Fut: Future<Output = Output> + Send + 'static,
{
    pub fn get_task_handle<T: Send + 'static, Instruction: InstructionExecutor<Input, Output, T>>(&self, instruction: Instruction) -> TaskHandle<T> {
        let pp = self.pre_processed.clone(); // usually arc clone
        let (task, recv) = Task::build_task(instruction, pp, self.process.clone());
    
        TaskHandle { task, recv, sender: Arc::clone(&self.sender) }
    }

    pub fn make_instruction<T: Send + 'static, Instruction: InstructionExecutor<Input, Output, T>>(&self) -> Instruction {
        Instruction::new()
    }
}

pub struct TaskHandle<T> {
    task: Task,
    recv: oneshot::Receiver<T>,
    sender: Arc<Sender<Task>>
} impl<T> TaskHandle<T> {
    pub async fn send(self) -> Result<TaskPromise<T>, tokio::sync::mpsc::error::SendError<Task>> {
        self.sender.send(self.task).await?;
        Ok(TaskPromise(self.recv))
    }
}

pub struct TaskPromise<T>(oneshot::Receiver<T>);

impl<T> TaskPromise<T> {
    pub async fn resolve(self) -> Result<T, oneshot::error::RecvError> {
        self.0.await
    } 
}

mod tests {
    #[tokio::test]
    async fn test() {

    }
}