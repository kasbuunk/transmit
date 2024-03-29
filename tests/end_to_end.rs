#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::prelude::*;
    use chrono::Duration;
    use futures::StreamExt;

    use transmit::grpc;
    use transmit::nats;

    #[tokio::test]
    // To monitor the transmitted messages, run nats via docker with:
    // `docker run -p 4222:4222 -ti nats:latest`
    // And observe live with:
    // `nats sub -s "nats://localhost:4222" INTEGRATION.schedule_transmission`
    async fn schedule_transmission() {
        let timestamp_now = Utc::now();
        let transmit_after_seconds = timestamp_now + Duration::milliseconds(800); // Initialise nats connection.
        let nats_connection = nats::connect_to_nats(nats::Config {
            port: 4222,
            // host is overridden, because the config file contains how the program itself
            // can find nats, which is through Docker's DNS.
            host: "0.0.0.0".to_string(),
        })
        .await
        .expect("could not connect to nats on port 4222");

        // Subscribe to test subject, to assert transmissions.
        let subject = "ENDTOEND.schedule_transmission";

        // Schedule a message with delayed(transmit_after_30s).
        let grpc_port = 8080;
        let host = "localhost";
        let address = format!("http://{}:{}", host, grpc_port);

        // Check health.
        let mut health_grpc_client =
            grpc::proto::health_client::HealthClient::connect(address.clone())
                .await
                .expect(&format!(
                    "failed to connect to grpc server on address {}",
                    &address
                ));
        let service = "transmit".to_string();
        let health_check_request = tonic::Request::new(grpc::proto::HealthCheckRequest { service });
        let response = health_grpc_client
            .check(health_check_request)
            .await
            .expect("health should be ok");
        assert_eq!(
            response.into_inner().status(),
            grpc::proto::health_check_response::ServingStatus::Serving
        );

        // Connect to serer.
        let mut grpc_client =
            grpc::proto::transmit_client::TransmitClient::connect(address.clone())
                .await
                .expect(&format!(
                    "failed to connect to grpc server on address {}",
                    &address
                ));

        // Construct the grpc request, containing a schedule and message.
        let message_schedule_request =
            new_message_schedule(subject.to_string(), transmit_after_seconds);
        let grpc_request = tonic::Request::new(message_schedule_request);

        // Do the request.
        let response = grpc_client
            .schedule_transmission(grpc_request)
            .await
            .expect("grpc server should handle request");
        let _ = uuid::Uuid::parse_str(&response.into_inner().transmission_id)
            .expect("response should contain a valid uuid");

        // Invoke poll-transmit (second: ten_seconds_later), assert nothing happened.
        let received_flag_after_10s = Arc::new(Mutex::new(false));
        let _handle_cannot_be_joined = listen_for_transmission(
            &mut nats_connection.clone(),
            subject.into(),
            received_flag_after_10s.clone(),
        )
        .await;
        // Wait too little time to assert no transmission until the schedule is met.
        let timeout_duration = std::time::Duration::from_millis(600);
        let timeout_reached = tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_after_10s.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
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
        let timeout_duration = std::time::Duration::from_millis(300);
        let timeout_reached = tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_after_60s.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        timeout_reached.expect(
            "transmission not received in expected timelapse; should set flag before timeout",
        );

        // Invoke another poll-transmit (fourth: two_minutes_later), assert happened.
        let received_flag_after_120s = Arc::new(Mutex::new(false));
        let _handle_cannot_be_joined = listen_for_transmission(
            &mut nats_connection.clone(),
            subject.into(),
            received_flag_after_120s.clone(),
        )
        .await;
        let timeout_duration = std::time::Duration::from_millis(500);
        let timeout_reached = tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag_after_120s.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(timeout_reached.is_err());
    }

    fn new_message_schedule(
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
}
