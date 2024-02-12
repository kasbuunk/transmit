use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::prelude::*;

use crate::contract::Repository;
use crate::model::*;

pub struct RepositoryInMemory {
    transmissions: Arc<Mutex<Vec<Transmission>>>,
}

impl RepositoryInMemory {
    pub fn new() -> RepositoryInMemory {
        RepositoryInMemory {
            transmissions: Arc::new(Mutex::new(vec![])),
        }
    }
}

#[async_trait]
impl Repository for RepositoryInMemory {
    async fn store_transmission(
        &self,
        transmission: &Transmission,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.transmissions
            .lock()
            .expect("mutex is poisoned")
            .push(transmission.clone());

        Ok(())
    }

    async fn poll_transmissions(
        &self,
        before: DateTime<Utc>,
        batch_size: u32,
    ) -> Result<Vec<Transmission>, Box<dyn Error>> {
        Ok(self
            .transmissions
            .clone()
            .lock()
            .expect("mutex is poisoned")
            .iter()
            .filter(|transmission| match transmission.next {
                None => false,
                Some(next) => next <= before,
            })
            .map(|transmission_reference| transmission_reference.clone())
            .take(batch_size as usize)
            .collect())
    }

    async fn save(&self, schedule: &Transmission) -> Result<(), Box<dyn Error + Send + Sync>> {
        for stored_schedule in self.transmissions.lock().unwrap().iter_mut() {
            if stored_schedule.id == schedule.id {
                *stored_schedule = schedule.clone();
            }
        }

        Ok(())
    }

    // reschedule is unnecessary for an in-memory implementation.
    async fn reschedule(
        &self,
        _schedule_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::time::pause;

    #[tokio::test]
    async fn test_store() {
        let repository = RepositoryInMemory::new();

        let now = Utc::now();
        let past = now - chrono::Duration::milliseconds(100);
        let future = now + chrono::Duration::milliseconds(100);

        let polled_transmissions_empty = repository
            .poll_transmissions(Utc::now(), 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_transmissions_empty.len(), 0);

        let transmissions = vec![
            Transmission::new(
                Schedule::Delayed(Delayed::new(past)),
                Message::NatsEvent(NatsEvent::new(
                    "ARBITRARY.subject".into(),
                    "arbitrary payload".into(),
                )),
            ),
            Transmission::new(
                Schedule::Delayed(Delayed::new(future)),
                Message::NatsEvent(NatsEvent::new(
                    "ARBITRARY.subject".into(),
                    "arbitrary payload".into(),
                )),
            ),
        ];
        let expected_polled_transmissions: Vec<Transmission> = vec![Transmission {
            id: transmissions[0].id.clone(),
            schedule: Schedule::Delayed(Delayed::new(past)),
            message: transmissions[0].message.clone(),
            next: Some(past),
            transmission_count: 0,
        }];

        for transmission in transmissions.iter() {
            repository
                .store_transmission(transmission)
                .await
                .expect("store transmission should be ok");
        }

        let polled_transmissions = repository
            .poll_transmissions(now, 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_transmissions, expected_polled_transmissions);

        for transmission in transmissions.iter() {
            let transmitted_message = transmission.transmitted();
            match transmitted_message {
                Ok(transmission) => match repository.save(&transmission).await {
                    Ok(()) => (),
                    Err(err) => panic!("failed to save: {err}"),
                },
                Err(err) => panic!("failed to transition to transmitted state: {err}"),
            };
        }

        pause();

        let polled_transmissions_transmitted = repository
            .poll_transmissions(Utc::now(), 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_transmissions_transmitted, vec![]);
    }
}
