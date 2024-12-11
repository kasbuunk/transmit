use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::prelude::*;
use log::{error, info, trace, warn};
#[cfg(test)]
use mockall::predicate::*;
use std::time;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::contract::{Metrics, Now, Repository, Scheduler, Transmitter};
use crate::model::{Message, MetricEvent, Schedule, ScheduleError, Transmission};

static BATCH_SIZE: u32 = 100;
static MAX_DELAYED_AGE: chrono::Duration = chrono::Duration::seconds(1);
static MIN_INTERVAL: time::Duration = time::Duration::from_millis(10);
static MAX_NATS_SUBJECT_LENGTH: u32 = 256;

#[derive(Clone)]
pub struct TransmissionScheduler {
    // repository keeps the program stateless, by providing a storage interface to store and
    // retrieve transmissions.
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
    async fn schedule(&self, when: Schedule, what: Message) -> Result<Uuid, ScheduleError> {
        validate_schedule(self.now.now(), &when)?;
        validate_message(&what)?;

        let transmission = Transmission::new(when, what);
        match self.repository.store_transmission(&transmission).await {
            Ok(_) => {
                self.metrics.count(MetricEvent::Scheduled(true));
                Ok(transmission.id)
            }
            Err(err) => {
                self.metrics.count(MetricEvent::Scheduled(false));
                Err(ScheduleError::Other(err))
            }
        }
    }
}

fn validate_schedule(now: DateTime<Utc>, schedule: &Schedule) -> Result<(), ScheduleError> {
    match schedule {
        Schedule::Delayed(delayed) => {
            if delayed.transmit_at - now < -MAX_DELAYED_AGE {
                return Err(ScheduleError::AgedSchedule);
            }

            Ok(())
        }
        Schedule::Interval(interval) => {
            if interval.first_transmission - now < -MAX_DELAYED_AGE {
                return Err(ScheduleError::AgedSchedule);
            }
            if interval.interval < MIN_INTERVAL {
                return Err(ScheduleError::TooShortInterval);
            }

            Ok(())
        }
        Schedule::Cron(cron_schedule) => {
            if cron_schedule.first_transmission_after - now < -MAX_DELAYED_AGE {
                return Err(ScheduleError::AgedSchedule);
            }

            Ok(())
        }
    }
}
fn validate_message(message: &Message) -> Result<(), ScheduleError> {
    match message {
        Message::NatsEvent(event) => {
            if event.subject.len() as u32 > MAX_NATS_SUBJECT_LENGTH {
                return Err(ScheduleError::NatsInvalidSubject);
            }

            if let Some('$') = event.subject.chars().next() {
                return Err(ScheduleError::NatsInvalidSubject);
            }

            if event.subject.contains('\0') {
                return Err(ScheduleError::NatsInvalidSubject);
            }

            if event.subject.contains(' ') {
                return Err(ScheduleError::NatsInvalidSubject);
            }

            if event.subject.contains('>') {
                return Err(ScheduleError::NatsInvalidSubject);
            }

            if event.subject.contains('*') {
                return Err(ScheduleError::NatsInvalidSubject);
            }

            Ok(())
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
    // to doing/queued, transmits them and then transitions them back to scheduled, done or
    // error. Or, errors should have a separate thing. We don't want any errors to meddle with
    // things that may errored as a one-off problem; likewise we need errors to be transparent by
    // metrics and logging.
    pub async fn process_batch(&self) -> Result<(), Box<dyn Error>> {
        let now = self.now.now();

        let schedules = match self.repository.poll_transmissions(now, BATCH_SIZE).await {
            Ok(schedules) => {
                self.metrics.count(MetricEvent::Polled(true));
                schedules
            }
            Err(err) => {
                self.metrics.count(MetricEvent::Polled(false));
                return Err(err);
            }
        };

        trace!("Polled {} schedules to transmit", schedules.len());

        let relevant_schedules: Vec<Transmission> = schedules
            .clone()
            .into_iter()
            .filter(
                |schedule| match schedule.schedule.next(schedule.transmission_count) {
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

    async fn transmit(&self, schedule: &Transmission) -> Result<(), Box<dyn Error + Send + Sync>> {
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

    use mockall::Sequence;
    use std::time;
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
        let interval = time::Duration::from_millis(100);
        let bit_later = time::Duration::from_millis(10);

        let test_cases = vec![
            vec![],
            vec![Transmission::new(
                Schedule::Interval(Interval::new(
                    timestamp_now + bit_later,
                    interval,
                    Iterate::Times(repetitions),
                )),
                Message::NatsEvent(NatsEvent::new(
                    "SUBJECT.arbitrary".into(),
                    "arbitrary payload".into(),
                )),
            )],
            vec![Transmission::new(
                Schedule::Interval(Interval::new(
                    DateTime::<Utc>::MAX_UTC,
                    interval,
                    Iterate::Times(repetitions),
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
                .expect_poll_transmissions()
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
        let interval = time::Duration::from_secs(30);

        let schedules_iterated_interval = vec![Transmission::new(
            Schedule::Interval(Interval::new(
                timestamp_now + chrono::Duration::seconds(10),
                interval,
                Iterate::Times(repetitions),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )];
        assert_eq!(
            schedules_iterated_interval[0].next.unwrap().to_string(),
            "2015-05-15 00:00:10 UTC"
        );

        let mut now = MockNow::new();
        now.expect_now().times(1).returning(move || timestamp_now);
        now.expect_now()
            .times(1)
            .returning(move || timestamp_some_time_later);

        let mut repository = MockRepository::new();
        repository
            .expect_poll_transmissions()
            .times(2)
            .returning(move |_, _| Ok(schedules_iterated_interval.clone()));
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
        let interval = time::Duration::from_millis(100);

        let original_schedule = Transmission::new(
            Schedule::Interval(Interval::new(just_now, interval, Iterate::Infinitely)),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        );

        let expected_transmission_0 = Transmission {
            id: original_schedule.id,
            schedule: Schedule::Interval(Interval {
                first_transmission: just_now,
                interval,
                iterate: Iterate::Infinitely,
            }),
            next: Some(just_now + interval),
            transmission_count: 1,
            message: original_schedule.message.clone(),
        };
        let expected_transmission_1 = Transmission {
            id: original_schedule.id,
            schedule: Schedule::Interval(Interval {
                first_transmission: just_now,
                interval,
                iterate: Iterate::Infinitely,
            }),
            next: Some(just_now + interval + interval),
            transmission_count: 2,
            message: original_schedule.message.clone(),
        };
        let expected_transmission_2 = Transmission {
            id: original_schedule.id,
            schedule: Schedule::Interval(Interval {
                first_transmission: just_now,
                interval,
                iterate: Iterate::Infinitely,
            }),
            next: Some(just_now + interval + interval + interval),
            transmission_count: 3,
            message: original_schedule.message.clone(),
        };

        repository
            .expect_save()
            .with(eq(expected_transmission_0.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_transmission_1.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_transmission_2))
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
        let result = scheduler.transmit(&expected_transmission_0).await;
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_transmission_1).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_finite_interval() {
        let mut transmitter = MockTransmitter::new();
        let mut repository = MockRepository::new();
        let mut metrics = MockMetrics::new();

        let repetitions = 3;
        let just_now = Utc::now() - chrono::Duration::milliseconds(10);
        let interval = time::Duration::from_millis(100);

        let original_schedule = Schedule::Interval(Interval::new(
            just_now,
            interval,
            Iterate::Times(repetitions),
        ));
        let original_transmission = Transmission::new(
            original_schedule.clone(),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        );

        let expected_transmission_second_to_last = Transmission {
            id: original_transmission.id,
            schedule: original_schedule.clone(),
            next: Some(just_now + interval),
            transmission_count: 1,
            message: original_transmission.message.clone(),
        };
        let expected_transmission_last = Transmission {
            id: original_transmission.id,
            schedule: original_schedule.clone(),
            next: Some(just_now + interval + interval),
            transmission_count: 2,
            message: original_transmission.message.clone(),
        };
        let expected_transmission_done = Transmission {
            id: original_transmission.id,
            schedule: original_schedule,
            next: None,
            transmission_count: 3,
            message: original_transmission.message.clone(),
        };

        repository
            .expect_save()
            .with(eq(expected_transmission_second_to_last.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_transmission_last.clone()))
            .returning(|_| Ok(()))
            .times(1);
        repository
            .expect_save()
            .with(eq(expected_transmission_done))
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

        let result = scheduler.transmit(&original_transmission).await;
        assert!(result.is_ok());
        let result = scheduler
            .transmit(&expected_transmission_second_to_last)
            .await;
        assert!(result.is_ok());
        let result = scheduler.transmit(&expected_transmission_last).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_schedule() {
        let mut repository = MockRepository::new();
        repository
            .expect_store_transmission()
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
        let schedule = Schedule::Delayed(Delayed::new(now));

        let result = scheduler.schedule(schedule, arbitrary_message()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_schedule_fail() {
        let mut repository = MockRepository::new();
        repository
            .expect_store_transmission()
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
        let schedule = Schedule::Delayed(Delayed::new(now));

        let result = scheduler.schedule(schedule, arbitrary_message()).await;
        assert!(result.is_err());
    }

    fn arbitrary_message() -> Message {
        Message::NatsEvent(NatsEvent::new(
            "SUBJECT.arbitrary".into(),
            "arbitrary payload".into(),
        ))
    }

    fn new_delayed(now: DateTime<Utc>) -> Schedule {
        Schedule::Delayed(Delayed::new(now))
    }

    fn new_interval_infinite(now: DateTime<Utc>) -> Schedule {
        Schedule::Interval(Interval::new(
            now,
            time::Duration::from_millis(100),
            Iterate::Infinitely,
        ))
    }

    fn new_transmission_delayed() -> Transmission {
        Transmission::new(
            Schedule::Delayed(Delayed::new(
                Utc::now() - chrono::Duration::milliseconds(10),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_schedule_interval_infinitely() -> Transmission {
        Transmission::new(
            Schedule::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                time::Duration::from_millis(100),
                Iterate::Infinitely,
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_nats_message() -> Message {
        Message::NatsEvent(NatsEvent::new(
            "SUBJECT.arbitrary".into(),
            "arbitrary payload".into(),
        ))
    }

    fn new_schedule_interval_n() -> Transmission {
        Transmission::new(
            Schedule::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                time::Duration::from_millis(100),
                Iterate::Times(5),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_schedule_interval_last() -> Transmission {
        Transmission::new(
            Schedule::Interval(Interval::new(
                Utc::now() - chrono::Duration::milliseconds(10),
                time::Duration::from_millis(100),
                Iterate::Times(0),
            )),
            Message::NatsEvent(NatsEvent::new(
                "SUBJECT.arbitrary".into(),
                "arbitrary payload".into(),
            )),
        )
    }

    fn new_schedule_cron(now: DateTime<Utc>) -> Schedule {
        let expression = "5 * * * May Fri 2015";
        let cron_schedule =
            cron::Schedule::from_str(expression).expect("should be valid cron schedule");

        Schedule::Cron(Cron::new(now, cron_schedule.clone(), Iterate::Infinitely))
    }

    type ScheduleSaveFn =
        dyn Fn(&Transmission) -> Result<(), Box<dyn Error + Send + Sync + 'static>> + Send + Sync;
    type RescheduleFn =
        dyn Fn(&Uuid) -> Result<(), Box<dyn Error + Send + Sync + 'static>> + Send + Sync;

    enum ScheduleStateTransition {
        Save(Box<ScheduleSaveFn>, bool),
        Reschedule(Box<RescheduleFn>, bool),
    }

    struct TransmissionTestCase {
        name: String,
        transmission:
            Box<dyn Fn(Message) -> Result<(), Box<dyn Error + Send + Sync + 'static>> + Send>,
        schedule_state_transition: ScheduleStateTransition,
        success: bool,
    }

    #[tokio::test]
    async fn test_transmission_and_appropriate_state_transition() {
        let test_cases = vec![
            TransmissionTestCase {
                name: "success".into(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: true,
            },
            TransmissionTestCase {
                name: "fail_and_reschedule".into(),
                transmission: Box::new(move |_| Err("Let's hope this gets rescheduled.".into())),
                schedule_state_transition: ScheduleStateTransition::Reschedule(
                    Box::new(move |_| Ok(())),
                    true,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "transmit_but_fail_mark_done".into(),
                transmission: Box::new(move |_| Ok(())),
                schedule_state_transition: ScheduleStateTransition::Save(
                    Box::new(move |_| Err("The schedule is stuck in doing now.".into())),
                    false,
                ),
                success: false,
            },
            TransmissionTestCase {
                name: "transmit_fail_and_reschedule_fail".into(),
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

            let result = scheduler.transmit(&new_transmission_delayed()).await;
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
        let transmission_list = vec![new_transmission_delayed(), new_transmission_delayed()];
        let amount_transmissions = transmission_list.len();
        let transmission_list_clone = transmission_list.clone();

        let mut repository = MockRepository::new();
        repository
            .expect_poll_transmissions()
            .times(1)
            .returning(move |_, batch_size| {
                assert_eq!(batch_size, BATCH_SIZE);
                Ok(transmission_list_clone.clone())
            });

        let mut transmitter = MockTransmitter::new();
        transmitter
            .expect_transmit()
            .times(amount_transmissions)
            .returning(|_message| Ok(()));

        repository
            .expect_save()
            .times(amount_transmissions)
            .returning(move |schedule| {
                // This schedule belongs to the original list constructed.
                assert!(transmission_list
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
            .times(amount_transmissions);
        metrics
            .expect_count()
            .with(eq(MetricEvent::ScheduleStateSaved(true)))
            .returning(|_| ())
            .times(amount_transmissions);

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
    async fn test_poll_transmit_fail() {
        let now = Utc::now();
        let ten_milliseconds = chrono::Duration::milliseconds(10);

        let schedules = vec![Transmission::new(
            Schedule::Delayed(Delayed::new(now - ten_milliseconds)),
            arbitrary_message(),
        )];

        let mut repository = MockRepository::new();
        repository
            .expect_poll_transmissions()
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
            Transmission::new(
                // Ready to process.
                Schedule::Delayed(Delayed::new(now - ten_milliseconds)),
                Message::NatsEvent(NatsEvent::new(
                    message_subject_success.into(),
                    "random_payload".into(),
                )),
            ),
            Transmission::new(
                // Ready to process.
                Schedule::Delayed(Delayed::new(now - ten_milliseconds)),
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
            .expect_poll_transmissions()
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

        let schedule = Transmission::new(
            Schedule::Cron(Cron::new(
                timestamp_first_poll,
                cron_schedule.clone(),
                Iterate::Times(5),
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
            .expect_poll_transmissions()
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
        let expected_schedule = Transmission {
            id: schedule.id,
            message: schedule.message,
            schedule: Schedule::Cron(Cron {
                first_transmission_after: timestamp_first_poll,
                iterate: Iterate::Times(5),
                expression: cron_schedule.clone(),
            }),
            next: Some(Utc.with_ymd_and_hms(2015, 5, 15, 0, 1, 5).unwrap()),
            transmission_count: 1,
        };
        repository
            .expect_poll_transmissions()
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

    #[test]
    fn test_validate_message() {
        struct TestCase {
            name: String,
            message: Message,
            expected_result: Result<(), ScheduleError>,
        }

        let test_cases = vec![
            TestCase {
                name: String::from("valid nats"),
                message: new_nats_message(),
                expected_result: Ok(()),
            },
            TestCase {
                name: String::from("nats too large subject"),
                message: Message::NatsEvent(NatsEvent::new(
                    String::from("C".repeat((MAX_NATS_SUBJECT_LENGTH + 1) as usize)),
                    "arbitrary payload".into(),
                )),
                expected_result: Err(ScheduleError::NatsInvalidSubject),
            },
            TestCase {
                name: String::from("start with $"),
                message: Message::NatsEvent(NatsEvent::new(
                    String::from("$reserved"),
                    "arbitrary payload".into(),
                )),
                expected_result: Err(ScheduleError::NatsInvalidSubject),
            },
            TestCase {
                name: String::from("contains null"),
                message: Message::NatsEvent(NatsEvent::new(
                    String::from("HAS.\0null"),
                    "arbitrary payload".into(),
                )),
                expected_result: Err(ScheduleError::NatsInvalidSubject),
            },
            TestCase {
                name: String::from("contains space"),
                message: Message::NatsEvent(NatsEvent::new(
                    String::from("HAS space"),
                    "arbitrary payload".into(),
                )),
                expected_result: Err(ScheduleError::NatsInvalidSubject),
            },
            TestCase {
                name: String::from("contains *"),
                message: Message::NatsEvent(NatsEvent::new(
                    String::from("HAS.*asterisk"),
                    "arbitrary payload".into(),
                )),
                expected_result: Err(ScheduleError::NatsInvalidSubject),
            },
            TestCase {
                name: String::from("contains >"),
                message: Message::NatsEvent(NatsEvent::new(
                    String::from("HAS.>gt"),
                    "arbitrary payload".into(),
                )),
                expected_result: Err(ScheduleError::NatsInvalidSubject),
            },
        ];

        for test_case in test_cases {
            let valid = validate_message(&test_case.message);
            match test_case.expected_result {
                Ok(()) => assert!(
                    valid.is_ok(),
                    "{}: expected schedule to be valid",
                    test_case.name
                ),
                Err(expected_err) => {
                    let actual_err = valid.expect_err("expect validation error");
                    assert_eq!(
                        expected_err, actual_err,
                        "{}: expected schedule to be invalid",
                        test_case.name
                    );
                }
            }
        }
    }
    #[test]
    fn test_validate_schedule() {
        let now = DateTime::from_timestamp(1431648000, 0).expect("should be valid timestamp");
        let long_ago = now - chrono::Duration::hours(1);

        struct TestCase {
            name: String,
            schedule: Schedule,
            expected_result: Result<(), ScheduleError>,
        }

        let test_cases = vec![
            TestCase {
                name: String::from("valid delayed"),
                schedule: new_delayed(now),
                expected_result: Ok(()),
            },
            TestCase {
                name: String::from("aged delayed"),
                schedule: new_delayed(long_ago),
                expected_result: Err(ScheduleError::AgedSchedule),
            },
            TestCase {
                name: String::from("valid interval"),
                schedule: Schedule::Interval(Interval::new(
                    now,
                    time::Duration::from_millis(100),
                    Iterate::Infinitely,
                )),
                expected_result: Ok(()),
            },
            TestCase {
                name: String::from("aged interval"),
                schedule: Schedule::Interval(Interval::new(
                    long_ago,
                    time::Duration::from_millis(100),
                    Iterate::Infinitely,
                )),
                expected_result: Err(ScheduleError::AgedSchedule),
            },
            TestCase {
                name: String::from("too short interval"),
                schedule: Schedule::Interval(Interval::new(
                    now,
                    time::Duration::from_nanos(10),
                    Iterate::Infinitely,
                )),
                expected_result: Err(ScheduleError::TooShortInterval),
            },
            TestCase {
                name: String::from("valid cron"),
                schedule: new_schedule_cron(now),
                expected_result: Ok(()),
            },
            TestCase {
                name: String::from("aged cron"),
                schedule: new_schedule_cron(long_ago),
                expected_result: Err(ScheduleError::AgedSchedule),
            },
        ];

        for test_case in test_cases {
            let valid = validate_schedule(now, &test_case.schedule);
            match test_case.expected_result {
                Ok(()) => assert!(
                    valid.is_ok(),
                    "{}: expected schedule to be valid",
                    test_case.name
                ),
                Err(expected_err) => {
                    let actual_err = valid.expect_err("expect validation error");
                    assert_eq!(
                        expected_err, actual_err,
                        "{}: expected schedule to be invalid",
                        test_case.name
                    );
                }
            }
        }
    }
}
