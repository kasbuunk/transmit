use std::error::Error;

use async_trait::async_trait;

use crate::contract::Transmitter;
use crate::model::Message;

pub struct NatsPublisher {
    client: async_nats::Client,
}

impl NatsPublisher {
    pub fn new(client: async_nats::Client) -> NatsPublisher {
        NatsPublisher { client }
    }
}

#[async_trait]
impl Transmitter for NatsPublisher {
    async fn transmit(&self, event: Message) -> Result<(), Box<dyn Error>> {
        match event {
            Message::NatsEvent(nats_event) => {
                self.client
                    .publish(nats_event.subject, nats_event.data)
                    .await?;

                Ok(())
            }
            _ => panic!("implement me"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::NatsEvent;
    use futures::StreamExt;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use bytes::Bytes;

    #[tokio::test]
    // This is a sociable unit test, i.e. it integrates with nats, which is expected to run and be
    // accessible.
    //
    // Run with `docker run -p 4222:4222 -ti nats:latest`.
    async fn test_transmitter() {
        let port = 4222;
        let address = format!("nats://localhost:{port}");
        let client = async_nats::connect(address)
            .await
            .expect("Nats connection failed. Is nats running on port {port}?");

        let subject = "EVENTS.published".into();

        let event = NatsEvent {
            subject,
            data: Bytes::from("structured bytes containing order information").into(),
        };
        let subject_clone = event.subject.clone();

        let subscription_client = client.clone();
        let mut subscriber = subscription_client
            .subscribe(subject_clone.clone())
            .await
            .expect("subscribing should succeed");
        subscriber
            .unsubscribe_after(1)
            .await
            .expect("unsubscribing should succeed");

        let received_flag = Arc::new(Mutex::new(false));
        let received_flag_clone = Arc::clone(&received_flag);

        let handle = tokio::spawn(async move {
            while let Some(message) = subscriber.next().await {
                assert_eq!(message.subject, subject_clone);
                *received_flag_clone.lock().expect("failed to lock") = true;
            }
        });

        let transmitter = NatsPublisher::new(client);
        transmitter
            .transmit(Message::NatsEvent(event))
            .await
            .expect("transmission should succeed");

        // Wait for the message to be received.
        let timeout_duration = Duration::from_millis(500);
        tokio::time::timeout(timeout_duration, async {
            // Wait until the flag is set to true (message received)
            while !*received_flag.lock().unwrap() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("timeout reached");

        handle.await.expect("could not join threads");
    }
}
