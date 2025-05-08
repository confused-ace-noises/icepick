use std::{marker::PhantomData, pin::Pin, sync::Arc};

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::process_manager::task::Task;

use super::proc_manager_handle::ProcManagerHandle;

pub struct ProcManager<Input, Output, PP, /*Proc, FutOut*/>
where
    Input: Send + 'static,
    Output: Send + 'static,
    // Proc: Fn(PP, Input) -> FutOut + Send + 'static + ?Sized,
    // FutOut: Future<Output = Output> + Send + 'static + ?Sized,
{
    preprocessed: PP,
    process: Arc<Box<dyn Fn(PP, Input) -> Pin<Box<dyn Future<Output = Output> + Send + 'static>> + Send + Sync + 'static>>,
    instruction_sender: Arc<mpsc::Sender<Task>>,
    _phantom: PhantomData<(Input, Output)>,
}

impl<Input, Output, PP, /*Proc, FutOut*/> ProcManager<Input, Output, PP, /*Proc, FutOut*/>
where
    Input: Send + 'static,
    Output: Send + 'static,
    // Proc: Fn(PP, Input) -> FutOut + Send + Sync + 'static,
    // FutOut: Future<Output = Output> + Send + 'static,
    PP: Clone
{
    pub async fn new_with_buffer_with_preprocessed_with_wrapper<PreWrap, WrapFunc, Proc>(
        buffer: usize,
        preprocessed: PreWrap,
        wrap_func: WrapFunc,
        process: Proc
    ) -> Self 
    where 
        WrapFunc: FnOnce(PreWrap) -> PP,
        Proc: Fn(PP, Input) -> Pin<Box<dyn Future<Output = Output> + Send + 'static>> + Send + Sync + 'static,
        // FutOut: dyn Future<Output = Output> + Send + 'static,
    {
        let (instruction_tx, mut instruction_rx) = mpsc::channel::<Task>(buffer);
        
        let pp = wrap_func(preprocessed);

        tokio::spawn(async move {
            while let Some(instruction) = instruction_rx.recv().await {
                instruction.run().await
            }
        });

        ProcManager { preprocessed: pp, instruction_sender: Arc::new(instruction_tx), process: Arc::new(Box::new(process) as Box<dyn Fn(PP, Input) -> Pin<Box<dyn Future<Output = Output> + Send + 'static>> + Send + Sync + 'static>), _phantom: PhantomData }
    }



    pub async fn get_handle(&self) -> ProcManagerHandle<Input, Output, PP> {
        ProcManagerHandle {
            id_handle: Uuid::now_v7(),
            pre_processed: self.preprocessed.clone(),
            process: self.process.clone(),
            sender: self.instruction_sender.clone(),
            _phantom: PhantomData,
        }
    }
}

mod test {
    use std::{sync::Arc, time::Duration};

    use dashmap::DashMap;
    use futures::future::join_all;

    use crate::process_manager::{instructions::{join_all_instr::JoinAllInstruction, race_instr::RaceInstruction}, proc_manager_new::ProcManager};

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    async fn test() {
        let preprocessed: Arc<DashMap<String, usize>> = Arc::new(DashMap::new());
        
        let proc_manager = ProcManager::new_with_buffer_with_preprocessed_with_wrapper(
            100, 
            preprocessed.clone(), 
            |x|x, 
            |pre, input: String| Box::pin( async move {
                let len = input.len();
                tokio::time::sleep(Duration::from_millis(200)).await;
                pre.insert(input, len);
            })
        ).await;

        let arc_proc = Arc::new(proc_manager);
        let proc_manager1 = arc_proc.clone();
        let proc_manager2 = arc_proc.clone();
        let proc_manager3 = arc_proc.clone();


        let user_1 = tokio::spawn(async move {
            let handle = proc_manager1.get_handle().await;

            let mut instruction_join: JoinAllInstruction<_, _> = handle.make_instruction();
            let mut instruction_race: RaceInstruction<_> = handle.make_instruction();

            instruction_join.push_many(vec![
                String::from("names"),
                "aaaaa".to_string(),
                "bbbbbbb".to_string(),
                "ccc".to_string(),
                "dd".to_string(),
            ].into_iter());

            instruction_race.push_many(vec![
                "one".to_string(),
                String::from("names_2"),
                "two".to_string(),
                "three".to_string(),
                "four".to_string(),
            ].into_iter());

            let handle1 = handle.get_task_handle(instruction_join);
            let handle2 = handle.get_task_handle(instruction_race);

            
            handle1.send().await.unwrap().resolve().await.unwrap();
            handle2.send().await.unwrap().resolve().await.unwrap();

            println!("done 1")
        });

        let user_2 = tokio::spawn(async move {
            let handle = proc_manager2.get_handle().await;

            let mut instruction_join: JoinAllInstruction<_, _> = handle.make_instruction();
            
            instruction_join.push_many(vec![
                String::from("names_3"),
                "five".to_string(),
                "six".to_string(),
                "seven".to_string(),
                "eight".to_string(),
            ].into_iter());

            let handle3 = handle.get_task_handle(instruction_join);

            handle3.send().await.unwrap().resolve().await.unwrap();

            println!("done 2")
        });


        let user_3 = tokio::spawn(async move {
            let handle = proc_manager3.get_handle().await;

            let mut instruction_race: RaceInstruction<_> = handle.make_instruction();
            
            instruction_race.push_many(vec![
                String::from("names_4"),
                "nine".to_string(),
                "ten".to_string(),
                "eleven".to_string(),
                "twelve".to_string(),
            ].into_iter());

            let handle4 = handle.get_task_handle(instruction_race);

            handle4.send().await.unwrap().resolve().await.unwrap();

            println!("done 3")
        });

        join_all(vec![user_1, user_2, user_3].into_iter()).await;

        println!("{:?}", preprocessed)
    }
}