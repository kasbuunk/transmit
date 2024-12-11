use std::error::Error;

use async_trait::async_trait;

use chrono::prelude::*;
#[allow(unused_imports)]
use mockall::{automock, mock, predicate::*};
use uuid::Uuid;

use crate::model::{Message, MetricEvent, Schedule, ScheduleError, Transmission};

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn schedule(&self, schedule: Schedule, message: Message) -> Result<Uuid, ScheduleError>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Repository: Send + Sync {
    async fn store_transmission(
        &self,
        schedule: &Transmission,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn poll_transmissions(
        &self,
        before: DateTime<Utc>,
        batch_size: u32,
    ) -> Result<Vec<Transmission>, Box<dyn Error>>;
    async fn save(&self, schedule: &Transmission) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn reschedule(&self, transmission_id: &Uuid) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait Transmitter: Send + Sync {
    async fn transmit(&self, message: Message) -> Result<(), Box<dyn Error + Send + Sync>>;
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

#[cfg_attr(test, automock)]
pub trait Metrics: Send + Sync {
    fn count(&self, event: MetricEvent);
}
