use std::{process::Output, sync::Arc, time::Duration};

use futures::future::join_all;
use tokio::{
    sync::{
        RwLock,
        mpsc::{self, Receiver, Sender, error::SendError},
    },
    time::sleep,
};

#[derive(Debug)]
pub enum Instruction<P: Send, O> {
    Process(P),
    Exec {
        sender: Sender<O>,
        to_exec: P
    },
    EndOfProcessStream,
    PoliteKill,
    ForceKill,
}
// ok so this is one 
// ooops
// impl<P: Send> Copy for Instruction<P> where P: Copy {}

impl<P, O> Clone for Instruction<P, O>
where
    P: Clone + Send,
    O: Clone + Send,
{
    fn clone(&self) -> Self {
        match self {
            Self::Process(arg0) => Self::Process(arg0.clone()),
            Self::Exec { sender, to_exec } => Self::Exec{sender: sender.clone(), to_exec: to_exec.clone()},
            Self::EndOfProcessStream => Self::EndOfProcessStream,
            Self::PoliteKill => Self::PoliteKill,
            Self::ForceKill => Self::ForceKill,
        }
    }
}

pub enum BatchInfo<O> {
    Info(O),
    EndOfBatch,
}

pub trait PreProcessClone {
    type Output;

    fn clone(&self) -> Self::Output; 
}

pub struct ProcessManager<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    info_sender: Sender<Instruction<I, O>>,
    processed_receiver: Receiver<BatchInfo<O>>,
}
impl<I, O> ProcessManager<I, O>
where
    I: Send + 'static,
    O: Send + 'static,
{
    pub const BUFFER_SIZE: usize = 200;

    pub async fn new_with_buffer_with_preprocessed_with_wrapper<Pre, ProcFunc, ProcOut, Wrap, WrapFn>(
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
        let (info_sender, mut info_receiver) = mpsc::channel::<Instruction<I, O>>(buffer);
        let (processed_sender, processed_receiver) = mpsc::channel::<BatchInfo<O>>(buffer);

        tokio::spawn(async move {
            let preprocess_val = wrapper_func(preprocessed);

            let mut to_process_buffer = Vec::new();

            // TODO: DRY mayhaps?
            loop {
                match info_receiver.recv().await {
                    Some(Instruction::Process(to_process)) => to_process_buffer.push(to_process),
                    Some(Instruction::Exec {sender, to_exec }) => {
                        sender.send(process(preprocess_val.clone(), to_exec).await).await;
                    }
                    Some(Instruction::EndOfProcessStream) => {
                        if !to_process_buffer.is_empty() {
                            let processed = futures::future::join_all(
                                to_process_buffer
                                    .drain(..)
                                    .map(|item| process(preprocess_val.clone(), item)),
                            )
                            .await;
                            // processed_sender.send(processed).await.expect("Error: receiver for processed items was dropped.");

                            for info in processed.into_iter() {
                                processed_sender.send(BatchInfo::Info(info)).await;
                            }

                            processed_sender.send(BatchInfo::EndOfBatch).await;
                        }
                    }
                    Some(Instruction::PoliteKill) => {
                        info_receiver.close();

                        if !to_process_buffer.is_empty() {
                            let processed = futures::future::join_all(
                                to_process_buffer
                                    .drain(..)
                                    .map(|item| process(preprocess_val.clone(), item)),
                            )
                            .await;

                            //processed_sender.send(processed).await.expect("Error: receiver for processed items was dropped.");
                            for info in processed.into_iter() {
                                processed_sender.send(BatchInfo::Info(info)).await;
                            }

                            processed_sender.send(BatchInfo::EndOfBatch).await;
                        };
                        break;
                    }
                    Some(Instruction::ForceKill) | None => break,
                }
            }
        });

        Self {
            processed_receiver,
            info_sender,
        }
    }

    pub async fn new_with_buffer<ProcFunc, ProcOut,>(buffer: usize, mut process: ProcFunc) -> Self
    where
        ProcFunc: FnMut(I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        Self::new_with_buffer_with_preprocessed_with_wrapper((), |_| (), move |_, y| process(y), buffer).await
    }

    pub async fn new_with_preporcessed_with_wrapper<Pre, ProcFunc, ProcOut, Wrap, WrapFn>(
        preprocessed: Pre,
        wrapper_func: WrapFn,
        process: ProcFunc,
    ) -> Self
    where
        Wrap: Send + Clone,
        WrapFn: FnOnce(Pre) -> Wrap + Send + 'static,
        Pre: Send + Sync + 'static,
        ProcFunc: FnMut(Wrap, I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        Self::new_with_buffer_with_preprocessed_with_wrapper(preprocessed, wrapper_func, process, Self::BUFFER_SIZE).await
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
        Self::new_with_buffer_with_preprocessed_with_wrapper(preprocessed, |pre| Arc::new(RwLock::new(pre)),process, Self::BUFFER_SIZE).await
    }
    
    pub async fn new_simple<ProcFunc, ProcOut>(process: ProcFunc) -> Self
    where
        ProcFunc: FnMut(I) -> ProcOut + Send + 'static,
        ProcOut: Future<Output = O> + Send,
    {
        Self::new_with_buffer(Self::BUFFER_SIZE, process).await
    }




    async fn send_task(&self, value: Instruction<I, O>) -> Result<(), SendError<Instruction<I, O>>> {
        self.info_sender.send(value).await
    }

    pub async fn push(&self, value: I) {
        self.send_task(Instruction::Process(value))
            .await
            .expect(Self::UNEXPECTED_CHANNEL_CLOSE_STRING)
    }

    pub async fn push_many(&self, values: impl Iterator<Item = I>) {
        join_all(values.map(|value| self.push(value))).await;
    } 

    pub async fn process(&self) {
        self.send_task(Instruction::EndOfProcessStream)
            .await
            .expect(Self::UNEXPECTED_CHANNEL_CLOSE_STRING);
    }

    pub async fn exec(&self, value: I) -> Result<Receiver<O>, ()> {
        let (sender, receiver) = tokio::sync::mpsc::channel(Self::BUFFER_SIZE);
        self.send_task(Instruction::Exec { sender, to_exec: value }).await.map_err(|_|())?;
        Ok(receiver)
    }

    pub async fn get_one(&mut self) -> O {
        loop {
            match self.processed_receiver.recv().await {
                Some(BatchInfo::EndOfBatch) => continue,
                Some(BatchInfo::Info(info)) => return info,
                None => Self::unexpected_channel_close(),
            }
        }
    }

    pub async fn get_batch(&mut self) -> Vec<O> {
        let mut buffer = Vec::new();

        loop {
            match self.processed_receiver.recv().await {
                Some(BatchInfo::EndOfBatch) => continue,
                Some(BatchInfo::Info(info)) => {
                    buffer.push(info);
                    break;
                }
                None => Self::unexpected_channel_close(),
            }
        }

        loop {
            match self.processed_receiver.recv().await {
                Some(BatchInfo::EndOfBatch) => break,
                Some(BatchInfo::Info(info)) => {
                    buffer.push(info);
                    continue;
                }
                None => Self::unexpected_channel_close(),
            }
        }

        buffer
    }

    /// includes the last one
    pub async fn get_until<B, FutB>(&mut self, evaluate: B) -> Vec<O>
    where
        B: Fn(&O) -> FutB,
        FutB: Future<Output = bool>,
    {
        let mut buffer = Vec::new();

        loop {
            let current_item = self.get_one().await;

            if !evaluate(&current_item).await {
                buffer.push(current_item);
                continue;
            } else {
                buffer.push(current_item);
                break;
            }
        }

        buffer
    }

    /// doesn't include last one
    pub async fn get_while<B, FutB>(&mut self, evaluate: B) -> Vec<O>
    where
        B: Fn(&O) -> FutB,
        FutB: Future<Output = bool>,
    {
        let mut buffer = Vec::new();

        loop {
            let current_item = self.get_one().await;

            if evaluate(&current_item).await {
                buffer.push(current_item);
                continue;
            } else {
                //buffer.push(current_item);
                break;
            }
        }

        buffer
    }

    fn unexpected_channel_close() -> ! {
        panic!("Expected channel to stay open, but it was closed unexpectedly.")
    }

    const UNEXPECTED_CHANNEL_CLOSE_STRING: &str =
        "Expected channel to stay open, but it was closed unexpectedly.";
}

impl<I, O> Drop for ProcessManager<I, O>
where
    I: Send,
    O: Send,
{
    fn drop(&mut self) {
        let sender = self.info_sender.clone();
        tokio::spawn(async move {
            let _ = sender.send(Instruction::ForceKill).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use tokio::{sync::RwLock, time::sleep};

    use crate::process_manager::manager::ProcessManager;

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    async fn test_reversing() {
        let fn_to_do = |x: String| {
            x.as_bytes().to_vec().into_iter().rev().map(|c| c as char).collect::<String>()
        };
        
        let mut proc_manager = ProcessManager::<String, String>::new_with_buffer_with_preprocessed_with_wrapper(
            fn_to_do, 
            Arc::new, 
            |x, y| async move {
                let x_cloned = Arc::clone(&x);

                x_cloned(y)
            }, 
            30
        ).await; 

        let vals = vec!["reversed!".to_string(), "idk".to_string(), "hiiiii".to_string()]; 
        let vals2 = String::from("this should come after the first");

        proc_manager.push_many(vals.clone().into_iter()).await;
        proc_manager.process().await;

        proc_manager.push(vals2.clone()).await;
        proc_manager.process().await;

        let expected_1 = vals.into_iter().map(fn_to_do).collect::<Vec<_>>();
        let expected_2 = vec![fn_to_do(vals2)];

        let batch_1 = proc_manager.get_batch().await;
        let batch_2 = proc_manager.get_batch().await;

        assert_eq!(expected_1, batch_1);
        assert_eq!(expected_2, batch_2);
    }
    
    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    async fn test_fruit_recognition() {
        let valid_words = vec![
            "apple".to_string(),
            "banana".to_string(),
            "orange".to_string(),
        ];
    
        let mut manager = ProcessManager::new_with_preprocessed_no_wrapper(
            valid_words,
    
            |valid_words: Arc<RwLock<Vec<String>>>, input: String| async move  {
                sleep(Duration::from_millis(50)).await; // pretend it's expensive lol
                let valid_words = valid_words.read().await;
                valid_words.contains(&input)
            },
    
        )
        .await;
    
        manager
            .push_many(
                vec![
                    "apple".to_string(),
                    "grape".to_string(),
                    "banana".to_string(),
                    "carrot".to_string(),
                ]
                .into_iter(),
            )
            .await;
    
        manager.process().await;
    
        let results = manager.get_batch().await;
    
        let expected = vec![true, false, true, false];

        assert_eq!(results, expected)
    }
}
// bottommmmmmmmmmmmmmmmmmmmm

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
pub async fn thingy_colonthree() {
    // ok so it works like this :3333
    // *                                                            typing:          Input   , Output <-- this one :3
    let mut process_manager = ProcessManager::<String, String>::new_simple(
        |input| async move {
            // here, you give a function (or closure, like in this case) that takes the Input 
            // and transforms it in a Future that returns the Output

            // aka, in non-nerd language, a thing that when you .await it it gives the Output
            // "in non-nerd language"

            // so, to reverse a string you'd do:

            input.chars().rev().collect() // <- this reverses the string

        } // <-- here
    ).await;
    // ok so, demonstrating the thing now

    process_manager.push(String::from("autumn is a nerd :3")).await;
    process_manager.push(String::from("skye is very cute :3")).await;

    // now, those two strings are loaded into the buffer
    process_manager.process().await; // <-- here, the 2 strings are flushed from the buffer and the function
    // we defined there will get applied to them in a multithreaded way.

    let result_strings = process_manager.get_batch().await;

    let autumn_string = result_strings[0].clone();
    let skye_string = result_strings[1].clone();

    println!("string 1: {}", autumn_string);
    println!("string 2: {}", skye_string);
}
