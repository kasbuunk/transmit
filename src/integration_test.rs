#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::prelude::*;
    use chrono::Duration;
    use futures::StreamExt;
    use mockall::predicate::*;
    use mockall::Sequence;
    use sqlx::postgres::PgPool;

    use crate::contract::*;
    use crate::grpc;
    use crate::postgres;
    use crate::repository_postgres;
    use crate::scheduler;
    use crate::transmitter_nats;

    #[tokio::test]
    // To monitor the transmitted messages, run nats via docker with:
    // `docker run -p 4222:4222 -ti nats:latest`
    // And observe live with:
    // `nats sub -s "nats://localhost:4222" INTEGRATION.schedule_message`
    async fn schedule_message() {
        let timestamp_now = Utc::now();
        let ten_seconds_later = timestamp_now + Duration::seconds(10);
        let transmit_after_30s = timestamp_now + Duration::seconds(30);
        let one_minute_later = timestamp_now + Duration::seconds(60);
        let two_minutes_later = timestamp_now + Duration::seconds(120);

        let mut now = MockNow::new();
        let mut sequence_now = Sequence::new();
        now.expect_now()
            .once()
            .in_sequence(&mut sequence_now)
            .returning(move || ten_seconds_later);
        now.expect_now()
            .once()
            .in_sequence(&mut sequence_now)
            .returning(move || one_minute_later);
        now.expect_now()
            .once()
            .in_sequence(&mut sequence_now)
            .returning(move || two_minutes_later);

        let mut metrics = MockMetrics::new();
        metrics
            .expect_count()
            .with(eq(MetricEvent::Polled(true)))
            .returning(|_| ())
            .times(3);
        metrics
            .expect_count()
            .with(eq(MetricEvent::Scheduled(true)))
            .returning(|_| ())
            .times(1);
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

        // Initialise postgres connection.
        let connection = test_db().await;
        // Construct repository.
        let repository = repository_postgres::RepositoryPostgres::new(connection);
        repository
            .migrate()
            .await
            .expect("could not run migrations");
        repository.clear_all().await.expect("could not clear table");

        // Initialise nats connection.
        let nats_connection = prepare_nats().await;
        // Construct transmitter.
        let transmitter = transmitter_nats::NatsPublisher::new(nats_connection.clone());

        // Construct scheduler.
        let mut scheduler = scheduler::MessageScheduler::new(
            Arc::new(repository),
            Arc::new(transmitter),
            Arc::new(now),
            Arc::new(metrics),
        );

        // Subscribe to test subject, to assert transmissions.
        let subject = "INTEGRATION.schedule_message";

        // Assert no transmission just yet.
        let flag_first_listening = Arc::new(Mutex::new(false));
        listen_for_transmission(
            &mut nats_connection.clone(),
            subject.into(),
            flag_first_listening,
        )
        .await;

        // Construct the grpc request, containing a schedule and message.
        let message_schedule_request =
            new_message_schedule(subject.to_string(), transmit_after_30s);
        let grpc_request = tonic::Request::new(message_schedule_request);

        // Schedule a message with delayed(transmit_after_30s).
        let scheduler_arc = Arc::new(scheduler.clone());
        let grpc_port = 50052;
        let grpc_config = grpc::Config { port: grpc_port };
        let grpc_server = grpc::GrpcServer::new(grpc_config, scheduler_arc.clone());
        tokio::task::spawn(async move {
            grpc_server
                .serve()
                .await
                .expect("failed to start grpc server");
        });
        // Allow time for the server to initialise.
        std::thread::sleep(std::time::Duration::from_millis(10));
        let host = "localhost";
        let address = format!("http://{}:{}", host, grpc_port);
        // Connect to serer.
        let mut grpc_client = grpc::proto::scheduler_client::SchedulerClient::connect(address)
            .await
            .expect("failed to connect to grpc server");
        // Do the request.
        let response = grpc_client
            .schedule_message(grpc_request)
            .await
            .expect("grpc server should handle request");
        let _ = uuid::Uuid::parse_str(&response.into_inner().schedule_entry_id)
            .expect("response should contain uuid");

        // Invoke poll-transmit (second: ten_seconds_later), assert nothing happened.
        let received_flag_after_10s = Arc::new(Mutex::new(false));
        let _handle_cannot_be_joined = listen_for_transmission(
            &mut nats_connection.clone(),
            subject.into(),
            received_flag_after_10s.clone(),
        )
        .await;
        scheduler.process_batch().await.expect("process should run");
        let timeout_duration = std::time::Duration::from_millis(25);
        let timeout_reached = tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_after_10s.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await;
        assert!(timeout_reached.is_err());

        // Invoke another poll-transmit (third: one_minute_later), assert the transmission.
        let received_flag_after_60s = Arc::new(Mutex::new(false));
        let _handle = listen_for_transmission(
            &mut nats_connection.clone(),
            subject.into(),
            received_flag_after_60s.clone(),
        )
        .await;
        scheduler.process_batch().await.expect("process should run");
        let timeout_duration = std::time::Duration::from_millis(25);
        let timeout_reached = tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_after_60s.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await;
        timeout_reached.expect("transmission should set flag before timeout");

        // Invoke another poll-transmit (fourth: two_minutes_later), assert happened.
        let received_flag_after_120s = Arc::new(Mutex::new(false));
        let _handle_cannot_be_joined = listen_for_transmission(
            &mut nats_connection.clone(),
            subject.into(),
            received_flag_after_120s.clone(),
        )
        .await;
        scheduler.process_batch().await.expect("process should run");
        let timeout_duration = std::time::Duration::from_millis(25);
        let timeout_reached = tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_after_120s.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await;
        assert!(timeout_reached.is_err());
    }

    fn new_message_schedule(
        subject: String,
        timestamp: DateTime<Utc>,
    ) -> grpc::proto::ScheduleMessageRequest {
        let delayed = grpc::proto::Delayed {
            transmit_at: Some(std::time::SystemTime::from(timestamp).into()),
        };
        let schedule = grpc::proto::schedule_message_request::Schedule::Delayed(delayed);
        let nats_event = grpc::proto::NatsEvent {
            subject: subject.to_string(),
            payload: "Integration test payload.".into(),
        };
        let message = grpc::proto::schedule_message_request::Message::NatsEvent(nats_event);
        let schedule_message_request = grpc::proto::ScheduleMessageRequest {
            schedule: Some(schedule),
            message: Some(message),
        };

        schedule_message_request
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
            name: "scheduler".into(),
            host: "localhost".into(),
            port: 5432,
            user: "postgres".into(),
            password: "postgres".into(),
            ssl: false,
        };
        let connection = postgres::connect_to_database(postgres_config)
            .await
            .expect("connecting to postgers failed. Is postgres running on port 5432?");

        connection
    }

    async fn prepare_nats() -> async_nats::Client {
        let nats_config = transmitter_nats::NatsConfig {
            port: 4222,
            host: "localhost".to_string(),
        };
        let address = format!("nats://{}:{}", nats_config.host, nats_config.port);
        let client = async_nats::connect(address)
            .await
            .expect("Nats connection failed. Is nats running on address {address}?");

        client
    }
}
