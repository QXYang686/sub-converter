use worker::Context;

use crate::application::{BackgroundTask, BackgroundTasks};

pub struct WorkerBackgroundTasks {
    context: Context,
}

impl WorkerBackgroundTasks {
    pub fn new(context: Context) -> Self {
        Self { context }
    }
}

impl BackgroundTasks for WorkerBackgroundTasks {
    fn spawn(&self, task: BackgroundTask) {
        self.context.wait_until(task);
    }
}
