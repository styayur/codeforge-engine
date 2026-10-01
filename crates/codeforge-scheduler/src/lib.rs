//! Priority-aware asynchronous scheduling with cancellation, timeouts, and debouncing.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};
use tokio::task::JoinSet;

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("scheduler is shut down")]
    Closed,
    #[error("task result channel was dropped")]
    ResultDropped,
}

#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CancellationToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

impl Eq for CancellationToken {}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, AtomicOrdering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(AtomicOrdering::Acquire)
    }
}

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub id: u64,
    pub key: Option<String>,
    pub cancellation: CancellationToken,
    pub timeout: Option<Duration>,
}

impl TaskContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutcome {
    Completed(String),
    Cancelled,
    TimedOut,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub id: u64,
    pub key: Option<String>,
    pub cancellation: CancellationToken,
    pub outcome: TaskOutcome,
    pub elapsed: Duration,
}

pub type JobFuture = Pin<Box<dyn Future<Output = TaskOutcome> + Send + 'static>>;

pub struct JobRequest {
    pub id: u64,
    pub key: Option<String>,
    pub priority: i32,
    pub timeout: Option<Duration>,
    pub operation: Box<dyn FnOnce(TaskContext) -> JobFuture + Send + 'static>,
}

impl JobRequest {
    pub fn new<P, F, Fut>(priority: P, operation: F) -> Self
    where
        P: Into<i32>,
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = TaskOutcome> + Send + 'static,
    {
        Self {
            id: 0,
            key: None,
            priority: priority.into(),
            timeout: None,
            operation: Box::new(move |context| Box::pin(operation(context))),
        }
    }

    pub fn keyed(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

struct Envelope {
    request: JobRequest,
    cancellation: CancellationToken,
    sequence: u64,
    result: oneshot::Sender<TaskResult>,
}

impl Eq for Envelope {}

impl PartialEq for Envelope {
    fn eq(&self, other: &Self) -> bool {
        self.request.priority == other.request.priority && self.sequence == other.sequence
    }
}

impl Ord for Envelope {
    fn cmp(&self, other: &Self) -> Ordering {
        self.request
            .priority
            .cmp(&other.request.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for Envelope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone)]
pub struct TaskScheduler {
    sender: mpsc::UnboundedSender<Envelope>,
    active_keys: Arc<Mutex<HashMap<String, CancellationToken>>>,
    next_id: Arc<AtomicU64>,
}

impl TaskScheduler {
    pub fn new(concurrency: usize) -> Self {
        let concurrency = concurrency.max(1);
        let (sender, receiver) = mpsc::unbounded_channel();
        let active_keys = Arc::new(Mutex::new(HashMap::<String, CancellationToken>::new()));
        let dispatcher_active = active_keys.clone();
        tokio::spawn(dispatcher(receiver, concurrency, dispatcher_active));
        Self {
            sender,
            active_keys,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub async fn submit(
        &self,
        mut request: JobRequest,
    ) -> Result<oneshot::Receiver<TaskResult>, SchedulerError> {
        let id = self.next_id.fetch_add(1, AtomicOrdering::Relaxed);
        request.id = id;
        let cancellation = CancellationToken::new();
        if let Some(key) = request.key.clone() {
            let mut active = self.active_keys.lock().await;
            if let Some(previous) = active.insert(key, cancellation.clone()) {
                previous.cancel();
            }
        }
        let (result, receiver) = oneshot::channel();
        self.sender
            .send(Envelope {
                request,
                cancellation,
                sequence: id,
                result,
            })
            .map_err(|_| SchedulerError::Closed)?;
        Ok(receiver)
    }
}

async fn dispatcher(
    mut receiver: mpsc::UnboundedReceiver<Envelope>,
    concurrency: usize,
    active_keys: Arc<Mutex<HashMap<String, CancellationToken>>>,
) {
    let permits = Arc::new(Semaphore::new(concurrency));
    let mut queue = BinaryHeap::new();
    let mut running = JoinSet::<TaskResult>::new();

    loop {
        while queue.len() < concurrency.saturating_mul(4) {
            match receiver.try_recv() {
                Ok(envelope) => queue.push(envelope),
                Err(_) => break,
            }
        }

        while running.len() < concurrency {
            let Some(envelope) = queue.pop() else {
                break;
            };
            let cancellation = envelope.cancellation.clone();
            let timeout = envelope.request.timeout;
            let result_sender = envelope.result;
            let permit = permits
                .clone()
                .acquire_owned()
                .await
                .expect("scheduler semaphore is never closed");
            running.spawn(async move {
                let started = Instant::now();
                let context = TaskContext {
                    id: envelope.request.id,
                    key: envelope.request.key.clone(),
                    cancellation: cancellation.clone(),
                    timeout,
                };
                let future = (envelope.request.operation)(context);
                let outcome = if cancellation.is_cancelled() {
                    TaskOutcome::Cancelled
                } else if let Some(duration) = timeout {
                    match tokio::time::timeout(duration, future).await {
                        Ok(outcome) => outcome,
                        Err(_) => TaskOutcome::TimedOut,
                    }
                } else {
                    future.await
                };
                let result = TaskResult {
                    id: envelope.request.id,
                    key: envelope.request.key.clone(),
                    cancellation: cancellation.clone(),
                    outcome,
                    elapsed: started.elapsed(),
                };
                drop(permit);
                let _ = result_sender.send(result.clone());
                result
            });
        }

        tokio::select! {
            maybe = receiver.recv() => {
                if let Some(envelope) = maybe {
                    queue.push(envelope);
                } else {
                    while let Some(result) = running.join_next().await {
                        match result {
                            Ok(result) => cleanup_key(&active_keys, &result).await,
                            Err(error) => tracing::warn!(%error, "scheduled task join failure"),
                        }
                    }
                    break;
                }
            }
            Some(result) = running.join_next(), if !running.is_empty() => {
                match result {
                    Ok(result) => cleanup_key(&active_keys, &result).await,
                    Err(error) => tracing::warn!(%error, "scheduled task join failure"),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(5)), if !queue.is_empty() => {}
        }
    }
}

async fn cleanup_key(active_keys: &Mutex<HashMap<String, CancellationToken>>, result: &TaskResult) {
    let Some(key) = result.key.as_ref() else {
        return;
    };
    let mut active = active_keys.lock().await;
    if active.get(key) == Some(&result.cancellation) {
        active.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runs_tasks_with_timeout() {
        let scheduler = TaskScheduler::new(1);
        let receiver = scheduler
            .submit(JobRequest::new(0, |_| async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                TaskOutcome::Completed("ok".to_owned())
            }))
            .await
            .expect("submit");
        let result = receiver.await.expect("result");
        assert_eq!(result.outcome, TaskOutcome::Completed("ok".to_owned()));
    }

    #[tokio::test]
    async fn cancels_previous_debounced_task() {
        let scheduler = TaskScheduler::new(2);
        let first = scheduler
            .submit(
                JobRequest::new(0, |context| async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    if context.is_cancelled() {
                        TaskOutcome::Cancelled
                    } else {
                        TaskOutcome::Completed("stale".to_owned())
                    }
                })
                .keyed("file.rs"),
            )
            .await
            .expect("first");
        let second = scheduler
            .submit(
                JobRequest::new(0, |_| async { TaskOutcome::Completed("fresh".to_owned()) })
                    .keyed("file.rs"),
            )
            .await
            .expect("second");
        let first_result = first.await.expect("first result");
        let second_result = second.await.expect("second result");
        assert_eq!(
            second_result.outcome,
            TaskOutcome::Completed("fresh".to_owned())
        );
        assert_eq!(first_result.outcome, TaskOutcome::Cancelled);
    }
}
