use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use tokio::{sync::Semaphore, task::JoinSet};

use crate::{
    job::{Job, JobType},
    server::AppState,
};

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<JobOutcome>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    Completed,
    Retry {
        failure_reason: String,
        retry_after: chrono::DateTime<Utc>,
    },
    Failed {
        failure_reason: String,
    },
}

pub trait JobHandler: Send + Sync + 'static {
    fn handle(&self, job: Job, state: AppState) -> HandlerFuture;
}

impl<F, Fut> JobHandler for F
where
    F: Fn(Job, AppState) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<JobOutcome>> + Send + 'static,
{
    fn handle(&self, job: Job, state: AppState) -> HandlerFuture {
        Box::pin((self)(job, state))
    }
}

#[derive(Clone)]
pub struct WorkerRuntime {
    worker_id: String,
    poll_interval: std::time::Duration,
    lease_duration: Duration,
    max_attempts: i32,
    concurrency_limit: usize,
    handlers: Arc<HashMap<JobType, Arc<dyn JobHandler>>>,
}

impl WorkerRuntime {
    pub fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            poll_interval: std::time::Duration::from_millis(25),
            lease_duration: Duration::minutes(5),
            max_attempts: 3,
            concurrency_limit: 1,
            handlers: Arc::new(HashMap::new()),
        }
    }

    pub fn with_handler(mut self, job_type: JobType, handler: impl JobHandler) -> Self {
        let mut handlers = (*self.handlers).clone();
        handlers.insert(job_type, Arc::new(handler));
        self.handlers = Arc::new(handlers);
        self
    }

    pub fn with_concurrency_limit(mut self, concurrency_limit: usize) -> Self {
        self.concurrency_limit = concurrency_limit.max(1);
        self
    }

    pub fn with_poll_interval(mut self, poll_interval: std::time::Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    pub fn with_lease_duration(mut self, lease_duration: Duration) -> Self {
        self.lease_duration = lease_duration;
        self
    }

    pub fn with_max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub async fn run(
        &self,
        state: AppState,
        shutdown: impl Future<Output = ()> + Send,
    ) -> Result<()> {
        let stopping = Arc::new(AtomicBool::new(false));
        let stop_flag = stopping.clone();
        tokio::pin!(shutdown);
        let mut join_set = JoinSet::new();
        let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    stop_flag.store(true, Ordering::SeqCst);
                }
                _ = tokio::time::sleep(self.poll_interval), if !stop_flag.load(Ordering::SeqCst) => {
                    self.claim_and_spawn(state.clone(), &mut join_set, semaphore.clone()).await?;
                }
                result = join_set.join_next(), if !join_set.is_empty() => {
                    if let Some(result) = result {
                        result??;
                    }
                }
            }

            if stop_flag.load(Ordering::SeqCst) && join_set.is_empty() {
                break;
            }
        }
        Ok(())
    }

    async fn claim_and_spawn(
        &self,
        state: AppState,
        join_set: &mut JoinSet<Result<()>>,
        semaphore: Arc<Semaphore>,
    ) -> Result<()> {
        while semaphore.available_permits() > 0 {
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => break,
            };
            let lease_expires_at = Utc::now() + self.lease_duration;
            let claimed = state
                .database
                .jobs()
                .claim_next(&self.worker_id, lease_expires_at)
                .await?;
            let Some(job) = claimed else {
                drop(permit);
                break;
            };
            let handlers = self.handlers.clone();
            let state_clone = state.clone();
            let runtime = self.clone();
            join_set.spawn(async move {
                let _permit = permit;
                runtime.execute_job(state_clone, handlers, job).await
            });
        }
        Ok(())
    }

    async fn execute_job(
        self,
        state: AppState,
        handlers: Arc<HashMap<JobType, Arc<dyn JobHandler>>>,
        job: Job,
    ) -> Result<()> {
        let handler = handlers
            .get(&job.job_type)
            .cloned()
            .ok_or_else(|| anyhow!("unknown job type: {:?}", job.job_type))?;
        let outcome = handler.handle(job.clone(), state.clone()).await?;
        match outcome {
            JobOutcome::Completed => {
                let _ = state.database.jobs().complete(job.id).await?;
            }
            JobOutcome::Failed { failure_reason } => {
                let _ = state.database.jobs().fail(job.id, &failure_reason).await?;
            }
            JobOutcome::Retry {
                failure_reason,
                retry_after,
            } => {
                let _ = state
                    .database
                    .jobs()
                    .schedule_retry(job.id, &failure_reason, retry_after, self.max_attempts)
                    .await?;
            }
        }
        Ok(())
    }
}

pub fn generate_worker_id() -> String {
    format!("worker-{}", Uuid::new_v4().simple())
}

use uuid::Uuid;

pub fn default_worker_runtime() -> WorkerRuntime {
    WorkerRuntime::new(generate_worker_id())
}
