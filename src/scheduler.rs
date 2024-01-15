use std::error::Error;
use std::thread;
use std::time;

use chrono::prelude::*;
use cron::Schedule;
use uuid::Uuid;

#[allow(unused_imports)]
use mockall::{automock, mock, predicate::*};

static BATCH_SIZE: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Repeat {
    Infinitely,
    Times(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub start: DateTime<Utc>,
    pub next: Option<DateTime<Utc>>,
    pub interval: chrono::Duration,
    pub transmission_count: u32,
    pub repeat: Repeat,
}

impl Interval {
    pub fn new(start: DateTime<Utc>, interval: chrono::Duration, repeat: Repeat) -> Interval {
        Interval {
            start,
            next: Some(start),
            interval,
            transmission_count: 0,
            repeat,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cron {
    pub start: DateTime<Utc>,
    pub next: Option<DateTime<Utc>>,
    pub schedule: Schedule,
    pub transmission_count: u32,
    pub repeat: Repeat,
}

impl Cron {
    pub fn new(start: DateTime<Utc>, schedule: Schedule, repeat: Repeat) -> Cron {
        Cron {
            start,
            next: schedule.after(&start).next(),
            schedule,
            transmission_count: 0,
            repeat,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulePattern {
    Delayed(DateTime<Utc>),
    Cron(Cron),
    Interval(Interval),
}

impl Iterator for SchedulePattern {
    type Item = SchedulePattern;

    // next is called after transmission of the message, to transition the SchedulePattern to
    // contain an easily comparable timestamp, such that it is easily retrieved by filtering
    // on the next datetime. If None, the message schedule is completed.
    fn next(&mut self) -> Option<SchedulePattern> {
        match &self {
            SchedulePattern::Delayed(_) => None,

            SchedulePattern::Interval(period) => match period.repeat {
                Repeat::Times(times) => match period.transmission_count >= times {
                    true => None,
                    false => Some(SchedulePattern::Interval(Interval {
                        start: period.start,
                        next: Some(
                            period.start + period.interval * (period.transmission_count as i32 + 1),
                        ),
                        interval: period.interval,
                        repeat: Repeat::Times(times),
                        transmission_count: period.transmission_count + 1,
                    })),
                },
                Repeat::Infinitely => Some(SchedulePattern::Interval(Interval {
                    start: period.start,
                    next: Some(
                        period.start + period.interval * (period.transmission_count as i32 + 1),
                    ),
                    interval: period.interval,
                    repeat: Repeat::Infinitely,
                    transmission_count: period.transmission_count + 1,
                })),
            },
            SchedulePattern::Cron(cron_schedule) => match cron_schedule.repeat {
                Repeat::Times(repetitions) => {
                    match cron_schedule.transmission_count < repetitions {
                        false => None,
                        true => Some(SchedulePattern::Cron(Cron {
                            start: cron_schedule.start,
                            next: cron_schedule
                                .schedule
                                .after(&cron_schedule.start)
                                .nth((cron_schedule.transmission_count + 1) as usize),
                            schedule: cron_schedule.schedule.clone(),
                            transmission_count: cron_schedule.transmission_count + 1,
                            repeat: cron_schedule.repeat.clone(),
                        })),
                    }
                }
                Repeat::Infinitely => Some(SchedulePattern::Cron(Cron {
                    start: cron_schedule.start,
                    next: cron_schedule
                        .schedule
                        .upcoming(Utc)
                        .nth((cron_schedule.transmission_count + 1) as usize),
                    schedule: cron_schedule.schedule.clone(),
                    transmission_count: cron_schedule.transmission_count + 1,
                    repeat: Repeat::Infinitely,
                })),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Event(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageSchedule {
    id: Uuid,
    pub schedule_pattern: Option<SchedulePattern>,
    pub message: Message,
}

impl MessageSchedule {
    pub fn new(schedule_pattern: SchedulePattern, message: Message) -> MessageSchedule {
        MessageSchedule {
            id: Uuid::new_v4(),
            schedule_pattern: Some(schedule_pattern),
            message,
        }
    }
}

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

pub struct MessageScheduler {
    // repository keeps the program stateless, by providing a storage interface to store and
    // retrieve message schedules.
    repository: Box<dyn Repository>,
    // transmitter sends the message according to the configured communication protocol.
    transmitter: Box<dyn Transmitter>,
    // now is used to inject a mockable function to simulate the system's current datetime.
    now: Box<dyn Now>,
    // metrics measures events of interest.
    metrics: Box<dyn Metrics>,
}

impl MessageScheduler {
    pub fn new(
        repository: Box<dyn Repository>,
        transmitter: Box<dyn Transmitter>,
        now: Box<dyn Now>,
        metrics: Box<dyn Metrics>,
    ) -> MessageScheduler {
        MessageScheduler {
            repository,
            transmitter,
            now,
            metrics,
        }
    }

    pub fn run(&self) -> ! {
        loop {
            match self.process_batch() {
                Ok(_) => (),
                Err(err) => println!("error: {:?}", err),
            };

            thread::sleep(time::Duration::from_millis(100));
        }
    }

    pub fn schedule(&self, when: SchedulePattern, what: Message) -> Result<(), Box<dyn Error>> {
        let schedule = MessageSchedule::new(when, what);
        match self.repository.store_schedule(schedule) {
            Ok(_) => {
                self.metrics.count(MetricEvent::Scheduled(true));
                Ok(())
            }
            Err(err) => {
                self.metrics.count(MetricEvent::Scheduled(false));
                Err(err)
            }
        }
    }

    // process_batch retrieves schedules that are overdue, and in scheduled state. It transitions them
    // to doing/queued, calls transmits them and then transitions it back to scheduled, done or
    // error. Or, errors should have a separate thing. We don't want any errors to meddle with
    // things that may errored as a one-off problem; likewise we need errors to be transparent by
    // metrics and logging.
    fn process_batch(&self) -> Result<(), Box<dyn Error>> {
        let schedules = match self.repository.poll_batch(BATCH_SIZE) {
            Ok(schedules) => {
                self.metrics.count(MetricEvent::Polled(true));
                schedules
            }
            Err(err) => {
                self.metrics.count(MetricEvent::Polled(false));
                return Err(err);
            }
        };

        schedules
            .iter()
            .filter(|schedule| match &schedule.schedule_pattern {
                Some(SchedulePattern::Delayed(datetime)) => datetime < &(self.now).now(),
                Some(SchedulePattern::Interval(interval)) => match interval.next {
                    None => false,
                    Some(next) => next < (self.now).now(),
                },
                Some(SchedulePattern::Cron(cron_schedule)) => match cron_schedule.next {
                    None => false,
                    Some(next) => next < (self.now).now(),
                },
                None => false,
            })
            .map(|schedule| match self.transmit(schedule) {
                Ok(_) => {
                    self.metrics.count(MetricEvent::Transmitted(true));
                    Ok(())
                }
                Err(err) => {
                    self.metrics.count(MetricEvent::Transmitted(false));
                    Err(err)
                }
            })
            .filter(|result| result.is_err())
            .for_each(|err| println!("{:?}", err));

        Ok(())
    }

    fn transmit(&self, schedule: &MessageSchedule) -> Result<(), Box<dyn Error>> {
        let transmission_result = self.transmitter.transmit(schedule.message.clone());

        match transmission_result {
            Ok(_) => {
                let schedule_pattern_clone = schedule.schedule_pattern.clone();
                let next_schedule_pattern = match schedule_pattern_clone {
                    None => None,
                    Some(mut s) => s.next(),
                };
                let next_schedule = MessageSchedule {
                    id: schedule.id,
                    schedule_pattern: next_schedule_pattern,
                    message: schedule.message.clone(),
                };

                let state_transition_result = self.repository.save(&next_schedule);
                match state_transition_result {
                    Ok(_) => {
                        self.metrics.count(MetricEvent::ScheduleStateSaved(true));
                        Ok(())
                    }
                    Err(err) => {
                        self.metrics.count(MetricEvent::ScheduleStateSaved(false));
                        Err(err)
                    }
                }
            }
            Err(transmission_err) => {
                match self.repository.reschedule(&schedule.id) {
                    Ok(_) => {
                        self.metrics.count(MetricEvent::Rescheduled(true));
                        Err(transmission_err)
                    }
                    Err(err) => {
                        self.metrics.count(MetricEvent::Rescheduled(false));
                        // This requires extra attention, since unrestored non-transmitted
                        // scheduled messages will remain stuck in a DOING state.
                        // It should be logged for manual intervention, retried or
                        // forcibly rescheduled.
                        Err(format!("{:?}: {:?}", transmission_err, err).into())
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::Sequence;
    use std::str::FromStr;

    #[test]
    fn test_poll_non_ready_schedule() {
        let timestamp_now =
            DateTime::from_timestamp(1431648000, 0).expect("should be valid timestamp");
        assert_eq!(timestamp_now.to_string(), "2015-05-15 00:00:00 UTC");
        let timestamp_now_clone = timestamp_now.clone();

        let repetitions = 5;
        let interval = chrono::Duration::milliseconds(100);
        let bit_later = chrono::Duration::milliseconds(10);

        let test_cases = vec![
            vec![],
            vec![MessageSchedule::new(
                SchedulePattern::Interval(Interval::new(
                    timestamp_now + bit_later,
                    interval,
                    Repeat::Times(repetitions),
                )),
                Message::Event("ArbitraryData".into()),
            )],
            vec![MessageSchedule::new(
                SchedulePattern::Interval(Interval::new(
                    DateTime::<Utc>::MAX_UTC,
                    interval,
                    Repeat::Times(repetitions),
                )),
                Message::Event("ArbitraryData".into()),
            )],
        ];

        for test_case_schedules in test_cases {
            let mut now = MockNow::new();
            now.expect_now()
                .times(test_case_schedules.len())
                .returning(move || timestamp_now_clone);

            let mut repository = MockRepository::new();
            repository
                .expect_poll_batch()
                .times(1)
                .returning(move |batch_size| {
                    assert_eq!(batch_size, BATCH_SIZE);
                    Ok(test_case_schedules.clone())
                });

            let transmitter = MockTransmitter::new();

            let mut metrics = MockMetrics::new();
            metrics
                .expect_count()
                .with(eq(MetricEvent::Polled(true)))
                .returning(|_| ())
                .times(1);

            let scheduler = MessageScheduler::new(
                Box::new(repository),
                Box::new(transmitter),
                Box::new(now),
                Box::new(metrics),
            );

            let result = scheduler.process_batch();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_poll_before_and_after_next() {
        let timestamp_now =
            DateTime::from_timestamp(1431648000, 0).expect("should be valid timestamp");
        assert_eq!(timestamp_now.to_string(), "2015-05-15 00:00:00 UTC");
        let timestamp_some_time_later = timestamp_now.clone() + chrono::Duration::milliseconds(15);

        let repetitions = 5;
        let interval = chrono::Duration::milliseconds(100);

        let schedules_repeated_interval = vec![MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                timestamp_now + chrono::Duration::milliseconds(10),
                interval,
                Repeat::Times(repetitions),
            )),
            Message::Event("ArbitraryData".into()),
        )];

        let mut now = MockNow::new();
        now.expect_now().times(1).returning(move || timestamp_now);
        now.expect_now()
            .times(1)
            .returning(move || timestamp_some_time_later);

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(2)
            .returning(move |batch_size| {
                assert_eq!(batch_size, BATCH_SIZE);
                Ok(schedules_repeated_interval.clone())
            });
        repository.expect_save().returning(|_| Ok(())).times(1);

        let mut transmitter = MockTransmitter::new();
        transmitter.expect_transmit().returning(|_| Ok(())).times(1);

        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .returning(|_| ())
            .times(2);
        metrics
            .expect_count()
            .with(eq(MetricEvent::Transmitted(true)))
            .returning(|_| ())
            .times(1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(1);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(now), // First call to now will be too early for the given time, the next will
            // be right when the schedule is met.
            Box::new(metrics),
        );

        let result = scheduler.process_batch();
        assert!(result.is_ok());
        let result = scheduler.process_batch();
        assert!(result.is_ok());
    }

    #[test]
    fn test_infinite_interval() {
        let mut transmitter = MockTransmitter::new();
        let mut repository = MockRepository::new();
        let mut metrics = MockMetrics::new();

        let just_now = Utc::now() - chrono::Duration::milliseconds(10);
        let interval = chrono::Duration::milliseconds(100);

        let original_schedule = MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(just_now, interval, Repeat::Infinitely)),
            Message::Event("ArbitraryData".into()),
        );

        let expected_schedule_0 = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: Some(SchedulePattern::Interval(Interval {
                start: just_now,
                next: Some(just_now + interval),
                interval,
                repeat: Repeat::Infinitely,
                transmission_count: 1,
            })),
            message: original_schedule.message.clone(),
        };
        let expected_schedule_1 = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: Some(SchedulePattern::Interval(Interval {
                start: just_now,
                next: Some(just_now + interval + interval),
                interval,
                repeat: Repeat::Infinitely,
                transmission_count: 2,
            })),
            message: original_schedule.message.clone(),
        };
        let expected_schedule_2 = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: Some(SchedulePattern::Interval(Interval {
                start: just_now,
                next: Some(just_now + interval + interval + interval),
                interval,
                repeat: Repeat::Infinitely,
                transmission_count: 3,
            })),
            message: original_schedule.message.clone(),
        };

        repository
            .expect_save()
            .with(eq(expected_schedule_0.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_schedule_1.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_schedule_2))
            .returning(|_| Ok(()))
            .times(1);

        transmitter.expect_transmit().returning(|_| Ok(())).times(3);

        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(3);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let result = scheduler.transmit(&original_schedule);
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_0);
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_finite_interval() {
        let mut transmitter = MockTransmitter::new();
        let mut repository = MockRepository::new();
        let mut metrics = MockMetrics::new();

        let repetitions = 2;
        let just_now = Utc::now() - chrono::Duration::milliseconds(10);
        let interval = chrono::Duration::milliseconds(100);

        let original_schedule = MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                just_now,
                interval,
                Repeat::Times(repetitions),
            )),
            Message::Event("ArbitraryData".into()),
        );

        let expected_schedule_second_to_last = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: Some(SchedulePattern::Interval(Interval {
                start: just_now,
                next: Some(just_now + interval),
                interval,
                repeat: Repeat::Times(repetitions),
                transmission_count: 1,
            })),
            message: original_schedule.message.clone(),
        };
        let expected_schedule_last = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: Some(SchedulePattern::Interval(Interval {
                start: just_now,
                next: Some(just_now + interval + interval),
                interval,
                repeat: Repeat::Times(repetitions),
                transmission_count: 2,
            })),
            message: original_schedule.message.clone(),
        };
        let expected_schedule_done = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: None,
            message: original_schedule.message.clone(),
        };

        repository
            .expect_save()
            .with(eq(expected_schedule_second_to_last.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_schedule_last.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_schedule_done))
            .returning(|_| Ok(()))
            .times(1);

        transmitter.expect_transmit().returning(|_| Ok(())).times(3);

        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(3);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let result = scheduler.transmit(&original_schedule);
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_second_to_last);
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_last);
        assert!(result.is_ok());
    }

    #[test]
    fn test_schedule() {
        let mut repository = MockRepository::new();
        repository
            .expect_store_schedule()
            .returning(|_| Ok(()))
            .times(1);

        let transmitter = MockTransmitter::new();
        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Scheduled(true)))
            .returning(|_| ())
            .times(1);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let now = Utc::now();
        let pattern = SchedulePattern::Delayed(now);

        let result = scheduler.schedule(pattern, Message::Event("DataBytes".into()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_schedule_fail() {
        let mut repository = MockRepository::new();
        repository
            .expect_store_schedule()
            .returning(|_| Err("Failed to schedule message".into()))
            .times(1);

        let transmitter = MockTransmitter::new();
        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Scheduled(false)))
            .returning(|_| ())
            .times(1);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let now = Utc::now();
        let pattern = SchedulePattern::Delayed(now);

        let result = scheduler.schedule(pattern, Message::Event("DataBytes".into()));
        assert!(result.is_err());
    }

    fn new_schedule_delayed() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Delayed(Utc::now() - chrono::Duration::milliseconds(10)),
            Message::Event("ArbitraryData".into()),
        )
    }

    fn new_schedule_interval_infinitely() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                chrono::Duration::milliseconds(100),
                Repeat::Infinitely,
            )),
            Message::Event("ArbitraryData".into()),
        )
    }

    fn new_schedule_interval_n() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                chrono::Duration::milliseconds(100),
                Repeat::Times(5),
            )),
            Message::Event("ArbitraryData".into()),
        )
    }

    fn new_schedule_interval_last() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                chrono::Duration::milliseconds(100),
                Repeat::Times(0),
            )),
            Message::Event("ArbitraryData".into()),
        )
    }

    type ScheduleSaveFn = dyn Fn(&MessageSchedule) -> Result<(), Box<dyn Error + 'static>> + Send;
    type RescheduleFn = dyn Fn(&Uuid) -> Result<(), Box<dyn Error + 'static>> + Send;

    enum ScheduleStateTransition {
        Save(Box<ScheduleSaveFn>, bool),
        Reschedule(Box<RescheduleFn>, bool),
    }

    struct TransmissionTestCase {
        name: String,
        schedule: MessageSchedule,
        transmission: Box<dyn Fn(Message) -> Result<(), Box<dyn Error + 'static>> + Send>,
        schedule_state_transition: ScheduleStateTransition,
        success: bool,
    }

    #[test]
    fn test_transmission_and_appropriate_state_transition() {
        let test_cases = vec![
            // Delayed message schedules.
            TransmissionTestCase {
                name: "delayed_success".into(),
                schedule: new_schedule_delayed(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: true,
            },
            TransmissionTestCase {
                name: "delayed_fail_and_reschedule".into(),
                schedule: new_schedule_delayed(),
                transmission: Box::new(move |_| Err("Let's hope this gets rescheduled.".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "delayed_transmit_but_fail_mark_done".into(),
                schedule: new_schedule_delayed(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "delayed_transmit_fail_and_reschedule_fail".into(),
                schedule: new_schedule_delayed(),
                transmission: Box::new(move |_| Err("Even the reschedule hereafter fails".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            // Periodic interval schedules.
            TransmissionTestCase {
                name: "periodic_success".into(),
                schedule: new_schedule_interval_infinitely(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: true,
            },
            TransmissionTestCase {
                name: "periodic_fail_and_reschedule".into(),
                schedule: new_schedule_interval_infinitely(),
                transmission: Box::new(move |_| Err("Let's hope this gets rescheduled.".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "periodic_transmit_but_fail_state_transition".into(),
                schedule: new_schedule_interval_infinitely(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "periodic_transmit_fail_and_reschedule_fail".into(),
                schedule: new_schedule_interval_infinitely(),
                transmission: Box::new(move |_| Err("Even the reschedule hereafter fails".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            // Repeat finitely
            TransmissionTestCase {
                name: "repeat_n_times_success".into(),
                schedule: new_schedule_interval_n(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: true,
            },
            TransmissionTestCase {
                name: "repeat_n_times_fail_and_reschedule".into(),
                schedule: new_schedule_interval_n(),
                transmission: Box::new(move |_| Err("Let's hope this gets rescheduled.".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "repeat_n_times_fail_state_transition".into(),
                schedule: new_schedule_interval_n(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "repeat_n_times_reschedule_fail".into(),
                schedule: new_schedule_interval_n(),
                transmission: Box::new(move |_| Err("Even the reschedule hereafter fails".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            // Repeat for the last time.
            TransmissionTestCase {
                name: "repeat_last_success".into(),
                schedule: new_schedule_interval_last(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: true,
            },
            TransmissionTestCase {
                name: "repeat_last_fail_reschedule".into(),
                schedule: new_schedule_interval_last(),
                transmission: Box::new(move |_| Err("Let's hope this gets rescheduled.".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "repeat_last_fail_state_transition".into(),
                schedule: new_schedule_interval_last(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "repeat_last_reschedule_fail".into(),
                schedule: new_schedule_interval_last(),
                transmission: Box::new(move |_| Err("Even the reschedule hereafter fails".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
        ];

        for test_case in test_cases {
            let mut transmitter = MockTransmitter::new();
            let mut repository = MockRepository::new();
            let mut metrics = MockMetrics::new();

            transmitter
                .expect_transmit()
                .returning(test_case.transmission)
                .times(1);

            match test_case.schedule_state_transition {
                ScheduleStateTransition::Save(callback, transition_succeeds) => {
                    repository.expect_save().returning(callback).times(1);
                    metrics
                        .expect_count()
                        .with(eq(MetricEvent::ScheduleStateSaved(transition_succeeds)))
                        .returning(|_| ())
                        .times(1);
                }
                ScheduleStateTransition::Reschedule(callback, transition_succeeds) => {
                    repository.expect_reschedule().returning(callback).times(1);
                    metrics
                        .expect_count()
                        .with(eq(MetricEvent::Rescheduled(transition_succeeds)))
                        .returning(|_| ())
                        .times(1);
                }
            };

            let scheduler = MessageScheduler::new(
                Box::new(repository),
                Box::new(transmitter),
                Box::new(Utc::now),
                Box::new(metrics),
            );

            let result = scheduler.transmit(&test_case.schedule);
            assert_eq!(
                result.is_ok(),
                test_case.success,
                "Test case failed: {}",
                test_case.name
            );
        }
    }

    #[test]
    fn test_poll_datetimes_schedules_success() {
        let schedule_list = vec![new_schedule_delayed(), new_schedule_delayed()];
        let amount_schedules = schedule_list.len();
        let schedule_list_clone = schedule_list.clone();

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(1)
            .returning(move |batch_size| {
                assert_eq!(batch_size, BATCH_SIZE);
                Ok(schedule_list_clone.clone())
            });

        let mut publisher = MockTransmitter::new();
        publisher
            .expect_transmit()
            .times(amount_schedules)
            .returning(|_message| Ok(()));

        repository
            .expect_save()
            .times(amount_schedules)
            .returning(move |schedule| {
                // This schedule belongs to the original list constructed.
                assert!(schedule_list
                    .clone()
                    .iter()
                    .any(|schedule_iter| &schedule_iter.id == &schedule.id));

                Ok(())
            });

        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .returning(|_| ())
            .times(1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::Transmitted(true)))
            .returning(|_| ())
            .times(amount_schedules);
        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(amount_schedules);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(publisher),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let result = scheduler.process_batch();
        assert!(result.is_ok());
    }

    #[test]
    fn test_poll_transmit_fail() {
        let now = Utc::now();
        let message_data = "This is an arbitrary message";
        let ten_milliseconds = chrono::Duration::milliseconds(10);

        let schedules = vec![MessageSchedule::new(
            SchedulePattern::Delayed(now - ten_milliseconds),
            Message::Event(message_data.into()),
        )];

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(1)
            .returning(move |_batch_size| Ok(schedules.clone()));
        repository
            .expect_reschedule()
            .times(1)
            .returning(move |_id| Ok(()));

        let mut transmitter = MockTransmitter::new();
        transmitter
            .expect_transmit()
            .times(1)
            .returning(move |_message| Err("Message fails to transmit".into()));

        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .returning(|_| ())
            .times(1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::Transmitted(false)))
            .returning(|_| ())
            .times(1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::Rescheduled(true)))
            .returning(|_| ())
            .times(1);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let result = scheduler.process_batch();
        assert!(result.is_ok());
    }

    #[test]
    // The first message will succeed, the second fails to transmit.
    fn test_poll_datetimes_schedules_fail_transmit() {
        let now = Utc::now();
        let ten_milliseconds = chrono::Duration::milliseconds(10);

        let message_data_success = "This message will transmit";
        let message_data_failure = "This message will fail to transmit";
        let index_message_failure = 1;

        let schedule_list = vec![
            MessageSchedule::new(
                // Ready to process.
                SchedulePattern::Delayed(now - ten_milliseconds),
                Message::Event(message_data_success.into()),
            ),
            MessageSchedule::new(
                // Ready to process.
                SchedulePattern::Delayed(now - ten_milliseconds),
                Message::Event(message_data_failure.into()),
            ),
        ];
        let amount_schedules = schedule_list.len();
        let schedule_list_clone = schedule_list.clone();
        let schedule_list_clone_0 = schedule_list.clone();

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(1)
            .returning(move |batch_size| {
                assert_eq!(batch_size, BATCH_SIZE);
                Ok(schedule_list_clone.clone())
            });

        let mut transmitter = MockTransmitter::new();
        transmitter
            .expect_transmit()
            .times(amount_schedules)
            .returning(move |message| match message {
                Message::Event(data) if &data == message_data_success => Ok(()),
                Message::Event(data) if &data == message_data_failure => {
                    Err("Second message fails to transmit".into())
                }
                _ => panic!("Unexpected transmission"),
            });

        repository
            .expect_save()
            .times(amount_schedules - 1)
            .returning(move |schedule| match schedule.id {
                id if &id != &schedule_list[index_message_failure].id => Ok(()),
                _ => panic!("Unexpected message marked done"),
            });
        repository
            .expect_reschedule()
            .times(1)
            .returning(move |schedule_id| match schedule_id {
                id if id == &schedule_list_clone_0[index_message_failure].id => Ok(()),
                _ => panic!("Unexpected restoration of scheduled message"),
            });

        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .returning(|_| ())
            .times(1);

        // Metrics caused by success.
        metrics
            .expect_count()
            .with(eq(MetricEvent::Transmitted(true)))
            .returning(|_| ())
            .times(amount_schedules - 1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(amount_schedules - 1);

        // Metrics caused by failure.
        metrics
            .expect_count()
            .with(eq(MetricEvent::Transmitted(false)))
            .returning(|_| ())
            .times(1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::Rescheduled(true)))
            .returning(|_| ())
            .times(1);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(Utc::now),
            Box::new(metrics),
        );

        let result = scheduler.process_batch();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cron() {
        // Friday 15 May 2015;
        let timestamp_first_poll = Utc.with_ymd_and_hms(2015, 5, 15, 0, 0, 0).unwrap();
        assert_eq!(timestamp_first_poll.to_string(), "2015-05-15 00:00:00 UTC");
        let timestamp_first_poll_clone = timestamp_first_poll.clone();

        // Every minute at 5 seconds of every hour, starting midnight, on any Fridays in May 2015.
        let expression = "5 * * * May Fri 2015";
        let cron_schedule =
            cron::Schedule::from_str(expression).expect("should be valid cron schedule");

        let schedule = MessageSchedule::new(
            SchedulePattern::Cron(Cron::new(
                timestamp_first_poll,
                cron_schedule.clone(),
                Repeat::Times(5),
            )),
            Message::Event("ArbitraryData".into()),
        );

        let mut sequence = Sequence::new();
        let mut repository = MockRepository::new();
        let mut now = MockNow::new();
        let mut transmitter = MockTransmitter::new();
        let mut metrics = MockMetrics::new();

        let schedule_first_poll = schedule.clone();
        let schedule_second_poll = schedule.clone();

        // First poll: no ready schedules yet, so no transmission.
        repository
            .expect_poll_batch()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |_| Ok(vec![schedule_first_poll.clone()]));
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .times(1)
            .returning(|_| ());
        now.expect_now()
            .times(1)
            .returning(move || timestamp_first_poll_clone.clone());

        // Second poll: schedule is ready: so transmit.
        let expected_cron_schedule_pattern = Cron {
            start: timestamp_first_poll,
            next: Some(Utc.with_ymd_and_hms(2015, 5, 15, 0, 1, 5).unwrap()),
            repeat: Repeat::Times(5),
            transmission_count: 1,
            schedule: cron_schedule.clone(),
        };
        repository
            .expect_poll_batch()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |_| Ok(vec![schedule_second_poll.clone()]));
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .times(1)
            .returning(|_| ());
        now.expect_now()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move || timestamp_first_poll_clone + chrono::Duration::seconds(10));
        transmitter
            .expect_transmit()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |_| Ok(()));
        repository
            .expect_save()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |schedule| {
                match &schedule.schedule_pattern {
                    Some(pattern) => match pattern {
                        SchedulePattern::Cron(cron_message_schedule) => {
                            assert_eq!(cron_message_schedule, &expected_cron_schedule_pattern);
                        }
                        _ => panic!("unexpected schedule pattern"),
                    },
                    None => panic!("expected some schedule pattern"),
                }
                Ok(())
            });
        metrics
            .expect_count()
            .with(eq(MetricEvent::Transmitted(true)))
            .returning(|_| ())
            .times(1);
        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(1);

        let scheduler = MessageScheduler::new(
            Box::new(repository),
            Box::new(transmitter),
            Box::new(now),
            Box::new(metrics),
        );

        // First poll.
        let result = scheduler.process_batch();
        assert!(result.is_ok());
        // Second poll.
        let result = scheduler.process_batch();
        assert!(result.is_ok());
    }
}
