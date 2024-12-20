use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time;

use bytes::Bytes;
use chrono::prelude::*;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Transmission {
    pub id: Uuid,
    pub schedule: Schedule,
    pub next: Option<DateTime<Utc>>,
    pub transmission_count: u32,
    pub message: Message,
}

impl Transmission {
    pub fn new(schedule: Schedule, message: Message) -> Transmission {
        let next = match schedule {
            Schedule::Delayed(ref delayed) => Some(delayed.transmit_at),
            Schedule::Interval(ref interval) => Some(interval.first_transmission),
            Schedule::Cron(ref cron_schedule) => cron_schedule
                .expression
                .after(&cron_schedule.first_transmission_after)
                .next(),
        };
        Transmission {
            id: Uuid::new_v4(),
            schedule,
            message,
            next,
            transmission_count: 0,
        }
    }

    // transmitted transitions the MessageSchedule to the next state, appropriate when transmitted.
    //
    // Returns an error if the current next field is None.
    pub fn transmitted(&self) -> Result<Transmission, Box<dyn Error + Send + Sync>> {
        if self.next.is_none() {
            return Err("The message was not in a state to be transmitted, because the next datetime was None.".into());
        }

        let new_transmission_count = self.transmission_count + 1;
        let new_next = self.schedule.next(new_transmission_count);

        Ok(Transmission {
            id: self.id,
            schedule: self.schedule.clone(),
            message: self.message.clone(),
            next: new_next,
            transmission_count: new_transmission_count,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Message {
    NatsEvent(NatsEvent),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NatsEvent {
    pub subject: async_nats::Subject,
    pub payload: Bytes,
}

impl NatsEvent {
    pub fn new(subject: String, payload: Bytes) -> NatsEvent {
        NatsEvent {
            subject: subject.into(),
            payload,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Schedule {
    Delayed(Delayed),
    Interval(Interval),
    Cron(Cron),
}

impl Schedule {
    // next is called after transmission of the message, to transition the Schedule to
    // contain an easily comparable timestamp, such that it is easily retrieved by filtering
    // on the next datetime. If None, the Transmission is completed.
    pub fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match &self {
            Schedule::Delayed(delayed) => delayed.next(transmission_count),
            Schedule::Interval(interval) => interval.next(transmission_count),
            Schedule::Cron(cron_schedule) => cron_schedule.next(transmission_count),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Delayed {
    pub transmit_at: DateTime<Utc>,
}

impl Delayed {
    pub fn new(transmit_at: DateTime<Utc>) -> Delayed {
        Delayed { transmit_at }
    }

    fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match transmission_count {
            0 => Some(self.transmit_at),
            _ => None,
        }
    }
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub first_transmission: DateTime<Utc>,
    pub interval: time::Duration,
    pub iterate: Iterate,
}

impl Interval {
    pub fn new(
        first_transmission: DateTime<Utc>,
        interval: time::Duration,
        iterate: Iterate,
    ) -> Interval {
        Interval {
            first_transmission,
            interval,
            iterate,
        }
    }

    fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match self.iterate {
            Iterate::Times(repetitions) => match transmission_count >= repetitions {
                true => None,
                false => Some(self.first_transmission + self.interval * transmission_count),
            },
            Iterate::Infinitely => {
                Some(self.first_transmission + self.interval * transmission_count)
            }
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Cron {
    pub first_transmission_after: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_custom_field")]
    pub expression: cron::Schedule,
    pub iterate: Iterate,
}

fn deserialize_custom_field<'de, D>(deserializer: D) -> Result<cron::Schedule, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let cron_str: String = Deserialize::deserialize(deserializer)?;

    let cron_schedule = cron::Schedule::from_str(&cron_str).expect("FIXME");

    Ok(cron_schedule)
}

impl Serialize for Cron {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Cron", 3)?;
        state.serialize_field("first_transmission_after", &self.first_transmission_after)?;

        // Serialize the cron schedule as a string representation
        state.serialize_field("expression", &self.expression.to_string())?;

        state.serialize_field("iterate", &self.iterate)?;
        state.end()
    }
}

impl Cron {
    pub fn new(
        first_transmission_after: DateTime<Utc>,
        expression: cron::Schedule,
        iterate: Iterate,
    ) -> Cron {
        Cron {
            first_transmission_after,
            expression,
            iterate,
        }
    }

    fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match self.iterate {
            Iterate::Times(repetitions) => match transmission_count >= repetitions {
                true => None,
                false => self
                    .expression
                    .after(&self.first_transmission_after)
                    .nth((transmission_count) as usize),
            },
            Iterate::Infinitely => self
                .expression
                .after(&self.first_transmission_after)
                .nth((transmission_count) as usize),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Iterate {
    // Infinitely dictates the schedule iterates indefinitely.
    Infinitely,
    // Times contains the number of transmissions planned.
    Times(u32),
}

#[derive(Debug)]
pub enum ScheduleError {
    AgedSchedule,
    TooShortInterval,
    NatsInvalidSubject,
    Other(Box<dyn Error>),
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScheduleError::AgedSchedule => {
                write!(f, "first transmission should not be in the past")
            }
            ScheduleError::TooShortInterval => write!(f, "interval must be sufficiently large"),
            ScheduleError::NatsInvalidSubject => {
                write!(f, "subject not allowed")
            }
            ScheduleError::Other(err) => write!(f, "err: {}", err),
        }
    }
}

impl PartialEq for ScheduleError {
    fn eq(&self, other: &Self) -> bool {
        match self {
            ScheduleError::AgedSchedule => match other {
                ScheduleError::AgedSchedule => true,
                _ => false,
            },
            ScheduleError::TooShortInterval => match other {
                ScheduleError::TooShortInterval => true,
                _ => false,
            },
            ScheduleError::NatsInvalidSubject => match other {
                ScheduleError::NatsInvalidSubject => true,
                _ => false,
            },
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum MetricEvent {
    Scheduled(bool),
    Polled(bool),
    Transmitted(bool),
    ScheduleStateSaved(bool),
    Rescheduled(bool),
}
