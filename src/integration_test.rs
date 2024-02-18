#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time;

    use chrono::prelude::*;
    use chrono::Duration;
    use futures::StreamExt;
    use mockall::Sequence;
    use sqlx::postgres::PgPool;
    use tokio_util::sync::CancellationToken;

    use crate::contract::*;
    use crate::grpc;
    use crate::grpc::proto::transmit_client::TransmitClient;
    use crate::metrics;
    use crate::nats;
    use crate::postgres;
    use crate::repository_postgres;
    use crate::scheduler;
    use crate::scheduler::TransmissionScheduler;
    use crate::transmitter_nats;

    // To monitor the transmitted messages, run nats via docker with:
    // `docker run -p 4222:4222 -ti nats:latest`
    // And observe live with:
    // `nats sub -s "nats://localhost:4222" {see subject defined in test}`

    #[tokio::test]
    async fn schedule_delayed_transmission() {
        let subject = "INTEGRATION.delayed_transmission";
        let grpc_port = 50053;
        let timestamp_now = Utc::now();
        let listen_timeout_duration = std::time::Duration::from_millis(25);

        let transmission_timestamp = timestamp_now + Duration::seconds(30);
        let process_listen_iterations = vec![
            // First poll is too soon, expect no tranmsision.
            ("FirstTooSoon", timestamp_now + Duration::seconds(10), false),
            // Second poll is after the tranmission is due. Expect transmission.
            (
                "SecondTransmissionDue",
                timestamp_now + Duration::seconds(60),
                true,
            ),
            // Third poll is after the transmission, but that should not repeat.
            (
                "ThirdAlreadyDone",
                timestamp_now + Duration::seconds(120),
                false,
            ),
        ];

        let mut now = MockNow::new();
        let mut sequence_now = Sequence::new();
        for process_listen_iteration in process_listen_iterations.clone() {
            now.expect_now()
                .once()
                .in_sequence(&mut sequence_now)
                .returning(move || process_listen_iteration.1);
        }

        let (scheduler, mut grpc_client, nats_connection) = initialise(now, grpc_port).await;

        // Construct the grpc request, containing a schedule and message.
        let schedule_transmission_request =
            new_delayed_transmission_request(subject.to_string(), transmission_timestamp);
        let grpc_request = tonic::Request::new(schedule_transmission_request);

        // Do the request.
        let response = grpc_client
            .schedule_transmission(grpc_request)
            .await
            .expect("grpc server should handle request");
        let _ = uuid::Uuid::parse_str(&response.into_inner().transmission_id)
            .expect("response should contain uuid");

        for process_listen_iteration in process_listen_iterations {
            process_and_listen(
                scheduler.clone(),
                subject,
                &nats_connection,
                listen_timeout_duration,
                process_listen_iteration.2,
                process_listen_iteration.0,
            )
            .await;
        }
    }

    #[tokio::test]
    async fn schedule_interval_transmission() {
        let subject = "INTEGRATION.interval_transmission";
        let grpc_port = 50052;
        let timestamp_now = Utc::now();
        let listen_timeout_duration = std::time::Duration::from_millis(25);

        let first_transmission_timestamp = timestamp_now + Duration::seconds(30);
        let interval_duration = time::Duration::from_secs(10);
        let repetitions = 3;
        let process_listen_iterations = vec![
            ("FirstTooSoon", timestamp_now + Duration::seconds(10), false),
            (
                "SecondShouldProcess",
                timestamp_now + Duration::seconds(31),
                true,
            ),
            ("ThirdTooSoon", timestamp_now + Duration::seconds(36), false),
            (
                "FourthShouldRepeat",
                timestamp_now + Duration::seconds(42),
                true,
            ),
            ("FifthTooSoon", timestamp_now + Duration::seconds(45), false),
            (
                "SixthAlreadyDone",
                timestamp_now + Duration::seconds(51),
                true,
            ),
            (
                "SeventhAlreadyDone",
                timestamp_now + Duration::seconds(59),
                false,
            ),
            (
                "EightAlreadyDone",
                timestamp_now + Duration::seconds(61),
                false,
            ),
            (
                "NinthAlreadyDone",
                timestamp_now + Duration::seconds(71),
                false,
            ),
        ];

        let mut now = MockNow::new();
        let mut sequence_now = Sequence::new();
        for process_listen_iteration in process_listen_iterations.clone() {
            now.expect_now()
                .once()
                .in_sequence(&mut sequence_now)
                .returning(move || process_listen_iteration.1);
        }

        let (scheduler, mut grpc_client, nats_connection) = initialise(now, grpc_port).await;

        // Construct the grpc request, containing a schedule and message.
        let schedule_transmission_request = new_interval_transmission_request(
            subject.to_string(),
            first_transmission_timestamp,
            interval_duration,
            repetitions,
        );
        let grpc_request = tonic::Request::new(schedule_transmission_request);

        // Do the request.
        let response = grpc_client
            .schedule_transmission(grpc_request)
            .await
            .expect("grpc server should handle request");
        let _ = uuid::Uuid::parse_str(&response.into_inner().transmission_id)
            .expect("response should contain uuid");

        for process_listen_iteration in process_listen_iterations {
            process_and_listen(
                scheduler.clone(),
                subject,
                &nats_connection,
                listen_timeout_duration,
                process_listen_iteration.2,
                process_listen_iteration.0,
            )
            .await;
        }
    }
    #[tokio::test]
    async fn schedule_cron_transmission() {
        let subject = "INTEGRATION.cron_transmission";
        let grpc_port = 50054;
        let timestamp_now: DateTime<Utc> =
            DateTime::<Utc>::from_timestamp(1431648000, 0).expect("invalid timestamp");
        assert_eq!(timestamp_now.to_string(), "2015-05-15 00:00:00 UTC");

        let listen_timeout_duration = std::time::Duration::from_millis(25);

        let first_transmission_after = timestamp_now + Duration::seconds(30);
        let cron_expression = "3 5 14 * * * *"; // "At 14h05:03."
        let repetitions = 3;
        let process_listen_iterations = vec![
            ("TooSoon", timestamp_now + Duration::seconds(10), false),
            (
                "StillTooSoon",
                timestamp_now + Duration::hours(14) + Duration::minutes(4) + Duration::seconds(59),
                false,
            ),
            (
                "ShouldProcess",
                timestamp_now + Duration::hours(14) + Duration::minutes(5) + Duration::seconds(4),
                true,
            ),
            (
                "TooSoonAfterFirst",
                timestamp_now + Duration::hours(28),
                false,
            ),
            (
                "ShouldProcessAgain",
                timestamp_now
                    + Duration::hours(14 + 24)
                    + Duration::minutes(5)
                    + Duration::seconds(4),
                true,
            ),
            (
                "TooSoonAfterSecond",
                timestamp_now + Duration::hours(14 + 24 + 22),
                false,
            ),
            (
                "ShouldProcessLastTime",
                timestamp_now
                    + Duration::hours(14 + 2 * 24)
                    + Duration::minutes(5)
                    + Duration::seconds(4),
                true,
            ),
            (
                "ShouldNeverTransmitAgain",
                timestamp_now + Duration::days(1000),
                false,
            ),
        ];

        let mut now = MockNow::new();
        let mut sequence_now = Sequence::new();
        for process_listen_iteration in process_listen_iterations.clone() {
            now.expect_now()
                .once()
                .in_sequence(&mut sequence_now)
                .returning(move || process_listen_iteration.1);
        }

        let (scheduler, mut grpc_client, nats_connection) = initialise(now, grpc_port).await;

        // Construct the grpc request, containing a schedule and message.
        let schedule_transmission_request = new_cron_transmission_request(
            subject.to_string(),
            first_transmission_after,
            &cron_expression,
            repetitions,
        );
        let grpc_request = tonic::Request::new(schedule_transmission_request);

        // Do the request.
        let response = grpc_client
            .schedule_transmission(grpc_request)
            .await
            .expect("grpc server should handle request");
        let _ = uuid::Uuid::parse_str(&response.into_inner().transmission_id)
            .expect("response should contain uuid");

        for process_listen_iteration in process_listen_iterations {
            process_and_listen(
                scheduler.clone(),
                subject,
                &nats_connection,
                listen_timeout_duration,
                process_listen_iteration.2,
                process_listen_iteration.0,
            )
            .await;
        }
    }

    fn new_cron_transmission_request(
        subject: String,
        first_transmission_after: DateTime<Utc>,
        cron_schedule: &str,
        iterations: u32,
    ) -> grpc::proto::ScheduleTransmissionRequest {
        let schedule = grpc::proto::Cron {
            first_transmission_after: Some(
                std::time::SystemTime::from(first_transmission_after).into(),
            ),
            expression: cron_schedule
                .try_into()
                .expect("interval is not too large to be prost duration"),

            iterate: Some(grpc::proto::cron::Iterate::Times(iterations)),
        };
        let schedule = grpc::proto::schedule_transmission_request::Schedule::Cron(schedule);
        let nats_event = grpc::proto::NatsEvent {
            subject: subject.to_string(),
            payload: "Integration test payload.".into(),
        };
        let message = grpc::proto::schedule_transmission_request::Message::NatsEvent(nats_event);
        let schedule_transmission_request = grpc::proto::ScheduleTransmissionRequest {
            schedule: Some(schedule),
            message: Some(message),
        };

        schedule_transmission_request
    }

    fn new_interval_transmission_request(
        subject: String,
        timestamp: DateTime<Utc>,
        interval: time::Duration,
        iterations: u32,
    ) -> grpc::proto::ScheduleTransmissionRequest {
        let schedule = grpc::proto::Interval {
            first_transmission: Some(std::time::SystemTime::from(timestamp).into()),
            interval: Some(
                interval
                    .try_into()
                    .expect("interval is not too large to be prost duration"),
            ),
            iterate: Some(grpc::proto::interval::Iterate::Times(iterations)),
        };
        let schedule = grpc::proto::schedule_transmission_request::Schedule::Interval(schedule);
        let nats_event = grpc::proto::NatsEvent {
            subject: subject.to_string(),
            payload: "Integration test payload.".into(),
        };
        let message = grpc::proto::schedule_transmission_request::Message::NatsEvent(nats_event);
        let schedule_transmission_request = grpc::proto::ScheduleTransmissionRequest {
            schedule: Some(schedule),
            message: Some(message),
        };

        schedule_transmission_request
    }

    fn new_delayed_transmission_request(
        subject: String,
        timestamp: DateTime<Utc>,
    ) -> grpc::proto::ScheduleTransmissionRequest {
        let delayed = grpc::proto::Delayed {
            transmit_at: Some(std::time::SystemTime::from(timestamp).into()),
        };
        let schedule = grpc::proto::schedule_transmission_request::Schedule::Delayed(delayed);
        let nats_event = grpc::proto::NatsEvent {
            subject: subject.to_string(),
            payload: "Integration test payload.".into(),
        };
        let message = grpc::proto::schedule_transmission_request::Message::NatsEvent(nats_event);
        let schedule_transmission_request = grpc::proto::ScheduleTransmissionRequest {
            schedule: Some(schedule),
            message: Some(message),
        };

        schedule_transmission_request
    }

    async fn listen_for_transmission(
        nats_connection: &async_nats::Client,
        subject: String,
        received_flag: Arc<Mutex<bool>>,
    ) -> tokio::task::JoinHandle<()> {
        let mut subscriber = nats_connection
            .subscribe(async_nats::Subject::from(subject.clone()))
            .await
            .expect("subscribing should succeed");

        let handle = tokio::spawn(async move {
            while let Some(message) = subscriber.next().await {
                assert_eq!(message.subject, subject.clone().into());
                *received_flag.lock().expect("failed to lock") = true;
            }
        });
        handle
    }

    async fn test_db() -> PgPool {
        let postgres_config = postgres::Config {
            name: "transmit".into(),
            host: "localhost".into(),
            port: 5432,
            user: "postgres".into(),
            password: "postgres".into(),
            ssl: false,
        };
        let connection = postgres::connect_to_test_database(postgres_config)
            .await
            .expect("connecting to postgres failed. Is postgres running on port 5432?");

        connection
    }

    fn metric_client() -> metrics::MetricClient {
        let (prometheus_client, _) = metrics::new(metrics::Config {
            port: 9090,
            endpoint: "/metrics".to_string(),
        });

        prometheus_client
    }

    async fn nats_connection() -> async_nats::Client {
        let nats_connection = nats::connect_to_nats(nats::Config {
            port: 4222,
            host: "0.0.0.0".to_string(),
        })
        .await
        .expect("could not connect to nats; is the server running on port 4222?");

        nats_connection
    }

    fn nats_publisher(nats_connection: &async_nats::Client) -> transmitter_nats::NatsPublisher {
        transmitter_nats::NatsPublisher::new(nats_connection.clone())
    }

    async fn postgres_repository() -> repository_postgres::RepositoryPostgres {
        // Initialise postgres connection.
        let postgres_connection = test_db().await;
        // Construct repository.
        let repository = repository_postgres::RepositoryPostgres::new(postgres_connection);
        repository
            .migrate()
            .await
            .expect("could not run migrations");

        repository
    }

    async fn new_scheduler(
        now: MockNow,
        nats_connection: &async_nats::Client,
    ) -> Arc<TransmissionScheduler> {
        let scheduler = scheduler::TransmissionScheduler::new(
            Arc::new(postgres_repository().await),
            Arc::new(nats_publisher(&nats_connection)),
            Arc::new(now),
            Arc::new(metric_client()),
        );

        Arc::new(scheduler)
    }

    async fn start_server(transmission_scheduler: Arc<TransmissionScheduler>, grpc_port: u16) {
        let grpc_config = grpc::Config { port: grpc_port };
        let grpc_server = grpc::GrpcServer::new(grpc_config, transmission_scheduler.clone());

        tokio::task::spawn(async move {
            grpc_server
                .serve(CancellationToken::new())
                .await
                .expect("failed to start grpc server");
        });

        // Allow time for the server to initialise.
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    async fn new_grpc_client(port: u16) -> TransmitClient<tonic::transport::Channel> {
        let host = "localhost";
        let address = format!("http://{}:{}", host, port);
        // Connect to serer.
        let grpc_client = grpc::proto::transmit_client::TransmitClient::connect(address)
            .await
            .expect("failed to connect to grpc server");

        grpc_client
    }

    async fn initialise(
        now: MockNow,
        grpc_port: u16,
    ) -> (
        Arc<TransmissionScheduler>,
        TransmitClient<tonic::transport::Channel>,
        async_nats::Client,
    ) {
        let nats_connection = nats_connection().await;
        let scheduler = new_scheduler(now, &nats_connection).await;
        start_server(scheduler.clone(), grpc_port).await;
        let grpc_client = new_grpc_client(grpc_port).await;

        (scheduler, grpc_client, nats_connection)
    }

    async fn process_and_listen(
        scheduler: Arc<TransmissionScheduler>,
        subject: &str,
        nats_connection: &async_nats::Client,
        listen_duration: std::time::Duration,
        expect_received: bool,
        test_case_name: &str,
    ) {
        let received_flag_before_transmission = Arc::new(Mutex::new(false));
        let _handle = listen_for_transmission(
            &nats_connection.clone(),
            subject.into(),
            received_flag_before_transmission.clone(),
        )
        .await;

        scheduler.process_batch().await.expect("process should run");

        let timeout_reached = tokio::time::timeout(listen_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_before_transmission.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await;
        assert_eq!(timeout_reached.is_ok(), expect_received, "{test_case_name}",);
    }
}
