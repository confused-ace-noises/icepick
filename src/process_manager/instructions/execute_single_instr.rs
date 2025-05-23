use crate::process_manager::{proc_manager_handle::UserProcess, task::InstructionExecutor};

pub struct ExecuteSingleInstruction<Input: Send + 'static> {
    value: Input
}

impl<Input: Send + 'static> ExecuteSingleInstruction<Input> {
    pub fn new(value: Input) -> Self {
        ExecuteSingleInstruction { value }
    }
}

impl<Input: Send + 'static, Output: Send + 'static, PP: Send + Clone + 'static> InstructionExecutor<Input, Output, Output, PP> for ExecuteSingleInstruction<Input> {
    fn execute(
        self,
        preprocessed: PP,
        process: UserProcess<Input, Output, PP>,
    ) -> impl Future<Output = Output> + Send
    {
        async move {
            process(preprocessed, self.value).await
        }
    }
}

pub struct HijackOnceInstruction<Input, Output, PP> 
where 
    Input: Send + 'static,
    Output: Send + 'static,
    PP: Clone + Send + 'static,
{
    value: Input,
    function: UserProcess<Input, Output, PP>
}

impl<Input, ClosureOutput, TaskOutput, Preprocessed> InstructionExecutor<Input, ClosureOutput, TaskOutput, Preprocessed> for HijackOnceInstruction<Input, TaskOutput, Preprocessed> 
where
    Input: Send + 'static,
    TaskOutput: Send + 'static,
    ClosureOutput: Send + 'static,
    Preprocessed: Clone + Send + 'static,
{
    fn execute(
        self,
        preprocessed: Preprocessed,
        _: UserProcess<Input, ClosureOutput, Preprocessed>,
    ) -> impl Future<Output = TaskOutput> + Send
    {
        (self.function)(preprocessed, self.value)
    }
}

impl<Input, Output, PP> HijackOnceInstruction<Input, Output, PP> 
where 
    Input: Send + 'static,
    Output: Send + 'static,
    PP: Clone + Send + 'static,
{
    pub fn new(value: Input, function: UserProcess<Input, Output, PP>) -> Self {
        Self {
            value,
            function
        }
    }
}