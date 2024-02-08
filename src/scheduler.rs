use std::error::Error;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use async_trait::async_trait;
use log::{error, info, warn};
#[cfg(test)]
use mockall::predicate::*;
use uuid::Uuid;

use crate::contract::{Metrics, Now, Repository, Scheduler, Transmitter};
use crate::model::{Message, MessageSchedule, MetricEvent, SchedulePattern};

static BATCH_SIZE: u32 = 100;

#[derive(Clone)]
pub struct TransmissionScheduler {
    // repository keeps the program stateless, by providing a storage interface to store and
    // retrieve message schedules.
    repository: Arc<dyn Repository>,
    // transmitter sends the message according to the configured communication protocol.
    transmitter: Arc<dyn Transmitter>,
    // now is used to inject a mockable function to simulate the system's current datetime.
    now: Arc<dyn Now>,
    // metrics measures events of interest.
    metrics: Arc<dyn Metrics>,
}

#[async_trait]
impl Scheduler for TransmissionScheduler {
    async fn schedule(&self, when: SchedulePattern, what: Message) -> Result<Uuid, Box<dyn Error>> {
        let schedule = MessageSchedule::new(when, what);
        match self.repository.store_schedule(&schedule).await {
            Ok(_) => {
                self.metrics.count(MetricEvent::Scheduled(true));
                Ok(schedule.id)
            }
            Err(err) => {
                self.metrics.count(MetricEvent::Scheduled(false));
                Err(err)
            }
        }
    }
}

impl TransmissionScheduler {
    pub fn new(
        repository: Arc<dyn Repository>,
        transmitter: Arc<dyn Transmitter>,
        now: Arc<dyn Now>,
        metrics: Arc<dyn Metrics>,
    ) -> TransmissionScheduler {
        TransmissionScheduler {
            repository,
            transmitter,
            now,
            metrics,
        }
    }

    pub async fn run(&self, cancel_token: CancellationToken) -> () {
        loop {
            match self.process_batch().await {
                Ok(_) => (),
                Err(err) => error!("error: {:?}", err),
            };

            // Graceful shutdown on interrupt signal.
            tokio::select! {
                _ = cancel_token.cancelled() => {

                    info!("Cancellation signal received. Shutting down application.");

                    break;
                }

                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
            }
        }

        info!("Application was shut down.");
    }

    // process_batch retrieves schedules that are overdue, and in scheduled state. It transitions them
    // to doing/queued, calls transmits them and then transitions it back to scheduled, done or
    // error. Or, errors should have a separate thing. We don't want any errors to meddle with
    // things that may errored as a one-off problem; likewise we need errors to be transparent by
    // metrics and logging.
    pub async fn process_batch(&self) -> Result<(), Box<dyn Error>> {
        let now = self.now.now();

        let schedules = match self.repository.poll_batch(now, BATCH_SIZE).await {
            Ok(schedules) => {
                self.metrics.count(MetricEvent::Polled(true));
                schedules
            }
            Err(err) => {
                self.metrics.count(MetricEvent::Polled(false));
                return Err(err);
            }
        };

        info!("Polled {} schedules to transmit", schedules.len());

        let relevant_schedules: Vec<MessageSchedule> = schedules
            .clone()
            .into_iter()
            .filter(
                |schedule| match schedule.schedule_pattern.next(schedule.transmission_count) {
                    None => false,
                    Some(next_datetime) => next_datetime <= now,
                },
            )
            .collect();

        for schedule in &relevant_schedules {
            match self.transmit(schedule).await {
                Ok(_) => {
                    self.metrics.count(MetricEvent::Transmitted(true));
                }
                Err(err) => {
                    self.metrics.count(MetricEvent::Transmitted(false));
                    error!("failed to transmit: {:?}", err)
                }
            }
        }

        Ok(())
    }

    async fn transmit(
        &self,
        schedule: &MessageSchedule,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let transmission_result = self.transmitter.transmit(schedule.message.clone()).await;

        info!("Transmitted message from schedule with id: {}", schedule.id);

        match transmission_result {
            Ok(_) => {
                let transmitted_message = schedule.transmitted();
                let transmitted_message = match transmitted_message {
                    Ok(message) => message,
                    Err(err) => {
                        warn!("Transmitted message that ought not to have been sent: {err}");
                        return Err(err);
                    }
                };

                let state_transition_result = self.repository.save(&transmitted_message).await;
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
                match self.repository.reschedule(&schedule.id).await {
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

    use std::str::FromStr;

    use chrono::prelude::*;
    use mockall::Sequence;
    use uuid::Uuid;

    use crate::contract::*;
    use crate::model::*;

    #[tokio::test]
    async fn test_poll_non_ready_schedule() {
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
                Message::NatsEvent(NatsEvent::new(
                    "SUBJECT.arbitrary".into(),
                    "arbitrary payload".into(),
                )),
            )],
            vec![MessageSchedule::new(
                SchedulePattern::Interval(Interval::new(
                    DateTime::<Utc>::MAX_UTC,
                    interval,
                    Repeat::Times(repetitions),
                )),
                Message::NatsEvent(NatsEvent::new(
                    "SUBJECT.arbitrary".into(),
                    "arbitrary payload".into(),
                )),
            )],
        ];

        for test_case_schedules in test_cases {
            let mut now = MockNow::new();
            now.expect_now()
                .times(1)
                .returning(move || timestamp_now_clone);

            let schedules = test_case_schedules.clone();
            let mut repository = MockRepository::new();
            repository
                .expect_poll_batch()
                .times(1)
                .returning(move |_, _| Ok(schedules.clone()));

            let transmitter = MockTransmitter::new();

            let mut metrics = MockMetrics::new();
            metrics
                .expect_count()
                .with(eq(MetricEvent::Polled(true)))
                .returning(|_| ())
                .times(1);

            let scheduler = TransmissionScheduler::new(
                Arc::new(repository),
                Arc::new(transmitter),
                Arc::new(now),
                Arc::new(metrics),
            );

            let result = scheduler.process_batch().await;
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_poll_before_and_after_next() {
        let timestamp_now =
            DateTime::from_timestamp(1431648000, 0).expect("should be valid timestamp");
        assert_eq!(timestamp_now.to_string(), "2015-05-15 00:00:00 UTC");
        let timestamp_some_time_later = timestamp_now.clone() + chrono::Duration::seconds(15);

        let repetitions = 5;
        let interval = chrono::Duration::seconds(30);

        let schedules_repeated_interval = vec![MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                timestamp_now + chrono::Duration::seconds(10),
                interval,
                Repeat::Times(repetitions),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )];
        assert_eq!(
            schedules_repeated_interval[0].next.unwrap().to_string(),
            "2015-05-15 00:00:10 UTC"
        );

        let mut now = MockNow::new();
        now.expect_now().times(1).returning(move || timestamp_now);
        now.expect_now()
            .times(1)
            .returning(move || timestamp_some_time_later);

        // let schedules_repeated_interval_clone = schedules_repeated_interval.clone()
        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(2)
            .returning(move |_, _| Ok(schedules_repeated_interval.clone()));
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(now), // First call to now will be too early for the given time, the next will
            // be right when the schedule is met.
            Arc::new(metrics),
        );

        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_infinite_interval() {
        let mut transmitter = MockTransmitter::new();
        let mut repository = MockRepository::new();
        let mut metrics = MockMetrics::new();

        let just_now = Utc::now() - chrono::Duration::milliseconds(10);
        let interval = chrono::Duration::milliseconds(100);

        let original_schedule = MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(just_now, interval, Repeat::Infinitely)),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        );

        let expected_schedule_0 = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: SchedulePattern::Interval(Interval {
                first_transmission: just_now,
                interval,
                repeat: Repeat::Infinitely,
            }),
            next: Some(just_now + interval),
            transmission_count: 1,
            message: original_schedule.message.clone(),
        };
        let expected_schedule_1 = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: SchedulePattern::Interval(Interval {
                first_transmission: just_now,
                interval,
                repeat: Repeat::Infinitely,
            }),
            next: Some(just_now + interval + interval),
            transmission_count: 2,
            message: original_schedule.message.clone(),
        };
        let expected_schedule_2 = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: SchedulePattern::Interval(Interval {
                first_transmission: just_now,
                interval,
                repeat: Repeat::Infinitely,
            }),
            next: Some(just_now + interval + interval + interval),
            transmission_count: 3,
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let result = scheduler.transmit(&original_schedule).await;
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_0).await;
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_finite_interval() {
        let mut transmitter = MockTransmitter::new();
        let mut repository = MockRepository::new();
        let mut metrics = MockMetrics::new();

        let repetitions = 3;
        let just_now = Utc::now() - chrono::Duration::milliseconds(10);
        let interval = chrono::Duration::milliseconds(100);

        let original_schedule_pattern = SchedulePattern::Interval(Interval::new(
            just_now,
            interval,
            Repeat::Times(repetitions),
        ));
        let original_schedule = MessageSchedule::new(
            original_schedule_pattern.clone(),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        );

        let expected_schedule_second_to_last = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: original_schedule_pattern.clone(),
            next: Some(just_now + interval),
            transmission_count: 1,
            message: original_schedule.message.clone(),
        };
        let expected_schedule_last = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: original_schedule_pattern.clone(),
            next: Some(just_now + interval + interval),
            transmission_count: 2,
            message: original_schedule.message.clone(),
        };
        let expected_schedule_done = MessageSchedule {
            id: original_schedule.id,
            schedule_pattern: original_schedule_pattern,
            next: None,
            transmission_count: 3,
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let result = scheduler.transmit(&original_schedule).await;
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_second_to_last).await;
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_schedule_last).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_schedule() {
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let now = Utc::now();
        let pattern = SchedulePattern::Delayed(Delayed::new(now));

        let result = scheduler.schedule(pattern, arbitrary_message()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_schedule_fail() {
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let now = Utc::now();
        let pattern = SchedulePattern::Delayed(Delayed::new(now));

        let result = scheduler.schedule(pattern, arbitrary_message()).await;
        assert!(result.is_err());
    }

    fn arbitrary_message() -> Message {
        Message::NatsEvent(NatsEvent::new(
            "SUBJECT.arbitrary".into(),
            "arbitrary payload".into(),
        ))
    }

    fn new_schedule_delayed() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Delayed(Delayed::new(
                Utc::now() - chrono::Duration::milliseconds(10),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_schedule_interval_infinitely() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                chrono::Duration::milliseconds(100),
                Repeat::Infinitely,
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_schedule_interval_n() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                chrono::Duration::milliseconds(100),
                Repeat::Times(5),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_schedule_interval_last() -> MessageSchedule {
        MessageSchedule::new(
            SchedulePattern::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                chrono::Duration::milliseconds(100),
                Repeat::Times(0),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    type ScheduleSaveFn = dyn Fn(&MessageSchedule) -> Result<(), Box<dyn Error + Send + Sync + 'static>>
        + Send
        + Sync;
    type RescheduleFn =
        dyn Fn(&Uuid) -> Result<(), Box<dyn Error + Send + Sync + 'static>> + Send + Sync;

    enum ScheduleStateTransition {
        Save(Box<ScheduleSaveFn>, bool),
        Reschedule(Box<RescheduleFn>, bool),
    }

    struct TransmissionTestCase {
        name: String,
        schedule: MessageSchedule,
        transmission:
            Box<dyn Fn(Message) -> Result<(), Box<dyn Error + Send + Sync + 'static>> + Send>,
        schedule_state_transition: ScheduleStateTransition,
        success: bool,
    }

    #[tokio::test]
    async fn test_transmission_and_appropriate_state_transition() {
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

            let scheduler = TransmissionScheduler::new(
                Arc::new(repository),
                Arc::new(transmitter),
                Arc::new(Utc::now),
                Arc::new(metrics),
            );

            let result = scheduler.transmit(&test_case.schedule).await;
            assert_eq!(
                result.is_ok(),
                test_case.success,
                "Test case failed: {}",
                test_case.name
            );
        }
    }

    #[tokio::test]
    async fn test_poll_datetimes_schedules_success() {
        let schedule_list = vec![new_schedule_delayed(), new_schedule_delayed()];
        let amount_schedules = schedule_list.len();
        let schedule_list_clone = schedule_list.clone();

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(1)
            .returning(move |_, batch_size| {
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(publisher),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_poll_transmit_fail() {
        let now = Utc::now();
        let ten_milliseconds = chrono::Duration::milliseconds(10);

        let schedules = vec![MessageSchedule::new(
            SchedulePattern::Delayed(Delayed::new(now - ten_milliseconds)),
            arbitrary_message(),
        )];

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(1)
            .returning(move |_, _| Ok(schedules.clone()));
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    // The first message will succeed, the second fails to transmit.
    async fn test_poll_datetimes_schedules_fail_transmit() {
        let now = Utc::now();
        let ten_milliseconds = chrono::Duration::milliseconds(10);

        let message_subject_success = "This message will transmit";
        let message_subject_failure = "This message will fail to transmit";
        let index_message_failure = 1;

        let schedule_list = vec![
            MessageSchedule::new(
                // Ready to process.
                SchedulePattern::Delayed(Delayed::new(now - ten_milliseconds)),
                Message::NatsEvent(NatsEvent::new(
                    message_subject_success.into(),
                    "random_payload".into(),
                )),
            ),
            MessageSchedule::new(
                // Ready to process.
                SchedulePattern::Delayed(Delayed::new(now - ten_milliseconds)),
                Message::NatsEvent(NatsEvent::new(
                    message_subject_failure.into(),
                    "random_payload".into(),
                )),
            ),
        ];
        let amount_schedules = schedule_list.len();
        let schedule_list_clone = schedule_list.clone();
        let schedule_list_clone_0 = schedule_list.clone();

        let mut repository = MockRepository::new();
        repository
            .expect_poll_batch()
            .times(1)
            .returning(move |_, batch_size| {
                assert_eq!(batch_size, BATCH_SIZE);
                Ok(schedule_list_clone.clone())
            });

        let mut transmitter = MockTransmitter::new();
        transmitter
            .expect_transmit()
            .times(amount_schedules)
            .returning(move |message| match message {
                Message::NatsEvent(data) if data.subject == message_subject_success.into() => {
                    Ok(())
                }
                Message::NatsEvent(data) if data.subject == message_subject_failure.into() => {
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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(Utc::now),
            Arc::new(metrics),
        );

        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cron() {
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
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
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
            .returning(move |_, _| Ok(vec![schedule_first_poll.clone()]));
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .times(1)
            .returning(|_| ());
        now.expect_now()
            .times(1)
            .returning(move || timestamp_first_poll_clone.clone());

        // Second poll: schedule is ready: so transmit.
        let expected_schedule = MessageSchedule {
            id: schedule.id,
            message: schedule.message,
            schedule_pattern: SchedulePattern::Cron(Cron {
                first_transmission_after: timestamp_first_poll,
                repeat: Repeat::Times(5),
                schedule: cron_schedule.clone(),
            }),
            next: Some(Utc.with_ymd_and_hms(2015, 5, 15, 0, 1, 5).unwrap()),
            transmission_count: 1,
        };
        repository
            .expect_poll_batch()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |_, _| Ok(vec![schedule_second_poll.clone()]));
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .times(1)
            .returning(|_| ());
        now.expect_now()
            .times(1)
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
                assert_eq!(schedule, &expected_schedule);

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

        let scheduler = TransmissionScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(now),
            Arc::new(metrics),
        );

        // First poll.
        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
        // Second poll.
        let result = scheduler.process_batch().await;
        assert!(result.is_ok());
    }
}
