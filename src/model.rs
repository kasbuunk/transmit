use std::error::Error;
use std::str::FromStr;

use bytes::Bytes;
use chrono::prelude::*;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Repeat {
    // Infinitely dictates the schedule repeats indefinitely.
    Infinitely,
    // Times contains the number of transmissions planned.
    Times(u32),
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub first_transmission: DateTime<Utc>,
    #[serde_as(as = "serde_with::DurationNanoSeconds<i64>")]
    pub interval: chrono::Duration,
    pub repeat: Repeat,
}

impl Interval {
    pub fn new(
        first_transmission: DateTime<Utc>,
        interval: chrono::Duration,
        repeat: Repeat,
    ) -> Interval {
        Interval {
            first_transmission,
            interval,
            repeat,
        }
    }

    fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match self.repeat {
            Repeat::Times(repetitions) => match transmission_count >= repetitions {
                true => None,
                false => {
                    Some(self.first_transmission + self.interval * (transmission_count as i32))
                }
            },
            Repeat::Infinitely => {
                Some(self.first_transmission + self.interval * (transmission_count as i32))
            }
        }
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Cron {
    pub first_transmission_after: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_custom_field")]
    pub schedule: cron::Schedule,
    pub repeat: Repeat,
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
        state.serialize_field("schedule", &self.schedule.to_string())?;

        state.serialize_field("repeat", &self.repeat)?;
        state.end()
    }
}

impl Cron {
    pub fn new(
        first_transmission_after: DateTime<Utc>,
        schedule: cron::Schedule,
        repeat: Repeat,
    ) -> Cron {
        Cron {
            first_transmission_after,
            schedule,
            repeat,
        }
    }

    fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match self.repeat {
            Repeat::Times(repetitions) => match transmission_count >= repetitions {
                true => None,
                false => self
                    .schedule
                    .after(&self.first_transmission_after)
                    .nth((transmission_count) as usize),
            },
            Repeat::Infinitely => self
                .schedule
                .after(&self.first_transmission_after)
                .nth((transmission_count) as usize),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Delayed {
    transmit_at: DateTime<Utc>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SchedulePattern {
    Delayed(Delayed),
    Cron(Cron),
    Interval(Interval),
}

impl SchedulePattern {
    // next is called after transmission of the message, to transition the SchedulePattern to
    // contain an easily comparable timestamp, such that it is easily retrieved by filtering
    // on the next datetime. If None, the message schedule is completed.
    pub fn next(&self, transmission_count: u32) -> Option<DateTime<Utc>> {
        match &self {
            SchedulePattern::Delayed(delayed) => delayed.next(transmission_count),
            SchedulePattern::Interval(interval) => interval.next(transmission_count),
            SchedulePattern::Cron(cron_schedule) => cron_schedule.next(transmission_count),
        }
    }
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
pub enum Message {
    NatsEvent(NatsEvent),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MessageSchedule {
    pub id: Uuid,
    pub schedule_pattern: SchedulePattern,
    pub next: Option<DateTime<Utc>>,
    pub transmission_count: u32,
    pub message: Message,
}

impl MessageSchedule {
    pub fn new(schedule_pattern: SchedulePattern, message: Message) -> MessageSchedule {
        let next = match schedule_pattern {
            SchedulePattern::Delayed(ref delayed) => Some(delayed.transmit_at),
            SchedulePattern::Interval(ref interval) => Some(interval.first_transmission),
            SchedulePattern::Cron(ref cron_schedule) => cron_schedule
                .schedule
                .after(&cron_schedule.first_transmission_after)
                .next(),
        };
        MessageSchedule {
            id: Uuid::new_v4(),
            schedule_pattern,
            message,
            next,
            transmission_count: 0,
        }
    }

    // transmitted transitions the MessageSchedule to the next state, appropriate when transmitted.
    //
    // Returns an error if the current next field is None.
    pub fn transmitted(&self) -> Result<MessageSchedule, Box<dyn Error>> {
        if self.next.is_none() {
            return Err("The message was not in a state to be transmitted, because the next datetime was None.".into());
        }

        let new_transmission_count = self.transmission_count + 1;
        let new_next = self.schedule_pattern.next(new_transmission_count);

        Ok(MessageSchedule {
            id: self.id,
            schedule_pattern: self.schedule_pattern.clone(),
            message: self.message.clone(),
            next: new_next,
            transmission_count: new_transmission_count,
        })
    }
}
