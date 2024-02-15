use std::error::Error;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::contract::Repository;
use crate::model::*;

pub struct RepositoryPostgres {
    conn: PgPool,
}

impl RepositoryPostgres {
    pub fn new(conn: PgPool) -> RepositoryPostgres {
        info!("constructing new repository");

        RepositoryPostgres { conn }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        info!("migrating");

        sqlx::migrate!().run(&self.conn).await?;

        Ok(())
    }

    pub async fn clear_all(&self) -> Result<(), Box<dyn Error>> {
        info!("deleting all transmissions");

        let _ = sqlx::query!("delete from transmission;")
            .execute(&self.conn)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl Repository for RepositoryPostgres {
    async fn store_transmission(
        &self,
        schedule: &Transmission,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("storing transmission");

        let schedule_sql = TransmissionSql::from(schedule);

        let _ = sqlx::query!(
            "
INSERT INTO transmission (
    id, message, next, schedule, transmission_count, inserted_at, is_locked
) VALUES (
    $1, $2, $3, $4, $5, now(), false
);
        ",
            schedule_sql.id,
            schedule_sql.message,
            schedule_sql.next,
            schedule_sql.schedule,
            schedule_sql.transmission_count as i32,
        )
        .execute(&self.conn)
        .await?;

        Ok(())
    }

    async fn poll_transmissions(
        &self,
        before: DateTime<Utc>,
        batch_size: u32,
    ) -> Result<Vec<Transmission>, Box<dyn Error>> {
        debug!("polling batch");
        let message_schedules_sql = sqlx::query_as!(
            TransmissionSql,
            "
WITH locked_schedules AS (
    UPDATE transmission
    SET is_locked = true
    WHERE id IN (
        SELECT id
        FROM (
            SELECT id, MAX(inserted_at) AS latest_inserted_at
            FROM transmission
            GROUP BY id
        ) latest_entries
        WHERE inserted_at = latest_inserted_at
    )
    AND next IS NOT NULL
    AND next < $1
    AND is_locked = false
    RETURNING id, message, next, schedule, transmission_count
)
SELECT * FROM locked_schedules
LIMIT $2;
        ",
            before,
            batch_size as i64,
        )
        .fetch_all(&self.conn)
        .await?;

        let message_schedules: Vec<Transmission> = message_schedules_sql
            .iter()
            .map(|message_schedule_sql| Transmission::from((*message_schedule_sql).clone()))
            .collect();

        Ok(message_schedules)
    }

    async fn save(&self, schedule: &Transmission) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.store_transmission(schedule).await
    }

    async fn reschedule(
        &self,
        transmission_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = sqlx::query!(
            "
UPDATE transmission
SET is_locked = true
WHERE id = $1
  AND is_locked = false
  AND (id, inserted_at) = (
    SELECT id, MAX(inserted_at)
    FROM transmission
    WHERE id = $2
      AND is_locked = false
    GROUP BY id
  );
        ",
            transmission_id,
            transmission_id,
        )
        .execute(&self.conn)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
struct TransmissionSql {
    id: Uuid,
    message: String,
    schedule: String,
    next: Option<DateTime<Utc>>,
    transmission_count: i32,
}

impl From<&Transmission> for TransmissionSql {
    fn from(schedule: &Transmission) -> TransmissionSql {
        TransmissionSql {
            id: schedule.id,
            message: serde_json::to_string(&schedule.message).expect("Failed to serialize message"),
            schedule: serde_json::to_string(&schedule.schedule)
                .expect("Failed to serialize schedule"),
            transmission_count: schedule.transmission_count as i32,
            next: schedule.next,
        }
    }
}

impl From<TransmissionSql> for Transmission {
    fn from(schedule_sql: TransmissionSql) -> Transmission {
        Transmission {
            id: schedule_sql.id,
            schedule: match serde_json::from_str(&schedule_sql.schedule) {
                Ok(schedule) => schedule,
                Err(err) => panic!("Failed to deserialize schedule: {:?}", err),
            },
            message: match serde_json::from_str(&schedule_sql.message) {
                Ok(message) => message,
                Err(err) => panic!("Failed to deserialize message: {:?}", err),
            },
            transmission_count: schedule_sql.transmission_count as u32,
            next: schedule_sql.next,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::postgres;

    #[tokio::test]
    async fn test_store() {
        let config = postgres::Config {
            name: "transmit".into(),
            host: "localhost".into(),
            port: 5432,
            user: "postgres".into(),
            password: "postgres".into(),
            ssl: false,
        };
        let connection = postgres::connect_to_test_database(config)
            .await
            .expect("connecting to postgres failed. Is postgres running on port 5432?");

        let repository = RepositoryPostgres::new(connection);
        repository
            .migrate()
            .await
            .expect("could not run migrations");
        repository.clear_all().await.expect("could not clear table");

        let now = Utc::now();
        let past = now - chrono::Duration::milliseconds(100);
        let future = now + chrono::Duration::milliseconds(100);

        let polled_schedules_empty = repository
            .poll_transmissions(now, 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_schedules_empty.len(), 0);

        let schedules = vec![
            Transmission::new(
                Schedule::Delayed(Delayed::new(past)),
                Message::NatsEvent(NatsEvent::new(
                    "ARBITRARY.subject".into(),
                    "first payload".into(),
                )),
            ),
            Transmission::new(
                Schedule::Delayed(Delayed::new(future)),
                Message::NatsEvent(NatsEvent::new(
                    "ARBITRARY.subject".into(),
                    "second payload".into(),
                )),
            ),
        ];
        let expected_polled_schedules: Vec<Transmission> = vec![Transmission {
            id: schedules[0].id.clone(),
            schedule: Schedule::Delayed(Delayed::new(past)),
            message: schedules[0].message.clone(),
            next: Some(past),
            transmission_count: 0,
        }];

        for schedule in schedules.iter() {
            repository
                .store_transmission(schedule)
                .await
                .expect("store schedule should be ok");
        }

        let polled_schedules = repository
            .poll_transmissions(now, 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_schedules, expected_polled_schedules);

        for schedule in schedules.iter() {
            let transmitted_message = schedule.transmitted();
            match transmitted_message {
                Ok(schedule) => {
                    assert!(schedule.next.is_none());
                    match repository.save(&schedule).await {
                        Ok(()) => (),
                        Err(err) => panic!("failed to save: {err}"),
                    }
                }
                Err(err) => panic!("failed to transition to transmitted state: {err}"),
            };
        }

        let polled_schedules_transmitted = repository
            .poll_transmissions(now, 100)
            .await
            .expect("last poll batch should be ok");
        assert_eq!(polled_schedules_transmitted, vec![]);
    }
}
