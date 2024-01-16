use std::error::Error;

use chrono::prelude::*;
#[allow(unused_imports)]
use mockall::{automock, mock, predicate::*};
use uuid::Uuid;

use crate::model::{Message, MessageSchedule};

#[cfg_attr(test, automock)]
pub trait Repository {
    fn store_schedule(&self, schedule: MessageSchedule) -> Result<(), Box<dyn Error>>;
    fn poll_batch(&self, batch_size: u32) -> Result<Vec<MessageSchedule>, Box<dyn Error>>;
    fn save(&self, schedule: &MessageSchedule) -> Result<(), Box<dyn Error>>;
    fn reschedule(&self, schedule_id: &Uuid) -> Result<(), Box<dyn Error>>;
}

#[cfg_attr(test, automock)]
pub trait Transmitter {
    fn transmit(&self, message: Message) -> Result<(), Box<dyn Error>>;
}

#[cfg_attr(test, automock)]
pub trait Now {
    fn now(&self) -> DateTime<Utc>;
}

impl<F> Now for F
where
    F: Fn() -> DateTime<Utc>,
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
pub trait Metrics {
    fn count(&self, event: MetricEvent);
}
