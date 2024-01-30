use std::error::Error;

use async_trait::async_trait;

use chrono::prelude::*;
#[allow(unused_imports)]
use mockall::{automock, mock, predicate::*};
use uuid::Uuid;

use crate::model::{Message, MessageSchedule, SchedulePattern};

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn schedule(
        &self,
        schedule: SchedulePattern,
        message: Message,
    ) -> Result<Uuid, Box<dyn Error>>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Repository: Send + Sync {
    async fn store_schedule(&self, schedule: &MessageSchedule) -> Result<(), Box<dyn Error>>;
    async fn poll_batch(
        &self,
        before: DateTime<Utc>,
        batch_size: u32,
    ) -> Result<Vec<MessageSchedule>, Box<dyn Error>>;
    async fn save(&self, schedule: &MessageSchedule) -> Result<(), Box<dyn Error>>;
    async fn reschedule(&self, schedule_id: &Uuid) -> Result<(), Box<dyn Error>>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Transmitter: Send + Sync {
    async fn transmit(&self, message: Message) -> Result<(), Box<dyn Error>>;
}

#[cfg_attr(test, automock)]
pub trait Now: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

impl<F> Now for F
where
    F: Fn() -> DateTime<Utc> + Send + Sync,
{
    fn now(&self) -> DateTime<Utc> {
        self()
    }
}

pub struct UtcNow;

impl Now for UtcNow {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MetricEvent {
    Scheduled(bool),
    Polled(bool),
    Transmitted(bool),
    ScheduleStateSaved(bool),
    Rescheduled(bool),
}

#[cfg_attr(test, automock)]
pub trait Metrics: Send + Sync {
    fn count(&self, event: MetricEvent);
}
