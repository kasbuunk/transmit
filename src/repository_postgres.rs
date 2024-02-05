use std::error::Error;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use log::info;
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
        info!("Constructing new repository.");

        RepositoryPostgres { conn }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        info!("Running migrations.");

        sqlx::migrate!().run(&self.conn).await?;

        Ok(())
    }

    #[cfg(test)]
    pub async fn clear_all(&self) -> Result<(), Box<dyn Error>> {
        info!("deleting all message schedules");
        let _ = sqlx::query!("delete from message_schedule;")
            .execute(&self.conn)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl Repository for RepositoryPostgres {
    async fn store_schedule(
        &self,
        schedule: &MessageSchedule,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("storing schedule");

        let schedule_sql = MessageScheduleSql::from(schedule);

        let _ = sqlx::query!(
            "
INSERT INTO message_schedule (
    id, message, next, schedule_pattern, transmission_count, inserted_at, is_locked
) VALUES (
    $1, $2, $3, $4, $5, now(), false
);
        ",
            schedule_sql.id,
            schedule_sql.message,
            schedule_sql.next,
            schedule_sql.schedule_pattern,
            schedule_sql.transmission_count as i32,
        )
        .execute(&self.conn)
        .await?;

        Ok(())
    }

    async fn poll_batch(
        &self,
        before: DateTime<Utc>,
        batch_size: u32,
    ) -> Result<Vec<MessageSchedule>, Box<dyn Error>> {
        info!("polling batch");
        let message_schedules_sql = sqlx::query_as!(
            MessageScheduleSql,
            "
WITH locked_schedules AS (
    UPDATE message_schedule
    SET is_locked = true
    WHERE id IN (
        SELECT id
        FROM (
            SELECT id, MAX(inserted_at) AS latest_inserted_at
            FROM message_schedule
            GROUP BY id
        ) latest_entries
        WHERE inserted_at = latest_inserted_at
    )
    AND next IS NOT NULL
    AND next < $1
    AND is_locked = false
    RETURNING id, message, next, schedule_pattern, transmission_count
)
SELECT * FROM locked_schedules
LIMIT $2;
        ",
            before,
            batch_size as i64,
        )
        .fetch_all(&self.conn)
        .await?;

        let message_schedules: Vec<MessageSchedule> = message_schedules_sql
            .iter()
            .map(|message_schedule_sql| MessageSchedule::from((*message_schedule_sql).clone()))
            .collect();

        Ok(message_schedules)
    }

    async fn save(&self, schedule: &MessageSchedule) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.store_schedule(schedule).await
    }

    async fn reschedule(
        &self,
        schedule_id: &uuid::Uuid,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let _ = sqlx::query!(
            "
UPDATE message_schedule
SET is_locked = true
WHERE id = $1
  AND is_locked = false
  AND (id, inserted_at) = (
    SELECT id, MAX(inserted_at)
    FROM message_schedule
    WHERE id = $2
      AND is_locked = false
    GROUP BY id
  );
        ",
            schedule_id,
            schedule_id,
        )
        .execute(&self.conn)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
struct MessageScheduleSql {
    id: Uuid,
    message: String,
    schedule_pattern: String,
    next: Option<DateTime<Utc>>,
    transmission_count: i32,
}

impl From<&MessageSchedule> for MessageScheduleSql {
    fn from(schedule: &MessageSchedule) -> MessageScheduleSql {
        MessageScheduleSql {
            id: schedule.id,
            message: serde_json::to_string(&schedule.message).expect("Failed to serialize message"),
            schedule_pattern: serde_json::to_string(&schedule.schedule_pattern)
                .expect("Failed to serialize schedule pattern"),
            transmission_count: schedule.transmission_count as i32,
            next: schedule.next,
        }
    }
}

impl From<MessageScheduleSql> for MessageSchedule {
    fn from(schedule_sql: MessageScheduleSql) -> MessageSchedule {
        MessageSchedule {
            id: schedule_sql.id,
            schedule_pattern: match serde_json::from_str(&schedule_sql.schedule_pattern) {
                Ok(pattern) => pattern,
                Err(err) => panic!("Failed to deserialize schedule pattern: {:?}", err),
            },
            message: match serde_json::from_str(&schedule_sql.message) {
                Ok(msg) => msg,
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
    use tokio::time::pause;

    #[tokio::test]
    async fn test_store() {
        let config = postgres::Config {
            name: "scheduler".into(),
            host: "localhost".into(),
            port: 5432,
            user: "postgres".into(),
            password: "postgres".into(),
            ssl: false,
        };
        let connection = postgres::connect_to_database(config)
            .await
            .expect("connecting to postgers failed. Is postgres running on port 5432?");

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
            .poll_batch(now, 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_schedules_empty.len(), 0);

        let schedules = vec![
            MessageSchedule::new(
                SchedulePattern::Delayed(Delayed::new(past)),
                Message::NatsEvent(NatsEvent::new(
                    "ARBITRARY.subject".into(),
                    "first payload".into(),
                )),
            ),
            MessageSchedule::new(
                SchedulePattern::Delayed(Delayed::new(future)),
                Message::NatsEvent(NatsEvent::new(
                    "ARBITRARY.subject".into(),
                    "second payload".into(),
                )),
            ),
        ];
        let expected_polled_schedules: Vec<MessageSchedule> = vec![MessageSchedule {
            id: schedules[0].id.clone(),
            schedule_pattern: SchedulePattern::Delayed(Delayed::new(past)),
            message: schedules[0].message.clone(),
            next: Some(past),
            transmission_count: 0,
        }];

        for schedule in schedules.iter() {
            repository
                .store_schedule(schedule)
                .await
                .expect("store schedule should be ok");
        }

        let polled_schedules = repository
            .poll_batch(now, 100)
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

        pause();

        let polled_schedules_transmitted = repository
            .poll_batch(now, 100)
            .await
            .expect("poll batch should be ok");
        assert_eq!(polled_schedules_transmitted, vec![]);
    }
}
