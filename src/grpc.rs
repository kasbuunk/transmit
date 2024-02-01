use std::error::Error;
use std::sync::Arc;
use std::time::SystemTime;

#[cfg(test)]
use mockall::predicate::*;

use chrono::prelude::*;
use log::{error, info};
use serde::Deserialize;
use tonic::{transport::Server, Request, Response, Status};

use crate::contract::Scheduler;
use crate::model::*;

pub mod proto {
    tonic::include_proto!("scheduler");
}
use proto::scheduler_server::SchedulerServer;
use proto::{ScheduleMessageRequest, ScheduleMessageResponse};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub port: u16,
}

pub struct GrpcServer {
    config: Config,
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl GrpcServer {
    pub fn new(config: Config, scheduler: Arc<dyn Scheduler + Send + Sync>) -> GrpcServer {
        GrpcServer { config, scheduler }
    }

    pub async fn serve(self) -> Result<(), Box<dyn Error>> {
        let host = "127.0.0.1";
        let address = format!("{}:{}", host, self.config.port).parse()?;
        let scheduler_server = SchedulerServer::new(self);

        info!("Start listening for incoming messages at {}.", address);

        Server::builder()
            .add_service(scheduler_server)
            .serve(address)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl proto::scheduler_server::Scheduler for GrpcServer {
    async fn schedule_message(
        &self,
        request: Request<ScheduleMessageRequest>,
    ) -> Result<Response<ScheduleMessageResponse>, Status> {
        let request_data = request.into_inner();
        let schedule_proto = match request_data.schedule {
            None => return Err(Status::invalid_argument("schedule is required")),
            Some(schedule) => schedule,
        };
        let schedule = match schedule_proto {
            proto::schedule_message_request::Schedule::Delayed(delayed) => {
                let timestamp = match delayed.transmit_at {
                    None => {
                        return Err(Status::invalid_argument("delayed.transmit_at is required"))
                    }
                    Some(timestamp) => timestamp,
                };
                let system_time = match SystemTime::try_from(timestamp) {
                    Err(err) => {
                        error!("failed to parse as system time: {err}");

                        return Err(Status::invalid_argument(
                            "delayed.transmit_at could not be parsed as SystemTime",
                        ));
                    }
                    Ok(system_time) => system_time,
                };
                let timestamp_utc = DateTime::<Utc>::from(system_time);

                SchedulePattern::Delayed(Delayed::new(timestamp_utc))
            }
        };
        let message_proto = match request_data.message {
            None => return Err(Status::invalid_argument("message is required")),
            Some(message) => message,
        };
        let message = match message_proto {
            proto::schedule_message_request::Message::NatsEvent(event) => {
                let subject = event.subject;
                let payload = event.payload;
                Message::NatsEvent(NatsEvent {
                    subject: subject.into(),
                    payload: payload.into(),
                })
            }
        };

        match self.scheduler.schedule(schedule, message).await {
            Ok(id) => {
                info!("Scheduled message: {id}");

                Ok(Response::new(ScheduleMessageResponse {
                    schedule_entry_id: id.to_string(),
                }))
            }
            Err(err) => {
                error!("Failed to schedule message: {err}");

                Err(Status::internal("internal server error"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use chrono::Utc;

    use crate::contract::*;
    use crate::grpc::proto::scheduler_server::Scheduler;

    #[tokio::test]
    async fn test_schedule_message() {
        let expected_id = uuid::uuid!("a23bfa0f-a906-429a-ab90-66322dfa72e5");
        let id_clone = expected_id.clone();
        let mut scheduler = MockScheduler::new();

        let event_subject = "some_subject".to_string();
        let event_payload = Bytes::from("some_payload");

        let now = Utc::now();
        let schedule = SchedulePattern::Delayed(Delayed::new(now));
        let message = Message::NatsEvent(NatsEvent::new(
            event_subject.clone(),
            event_payload.clone().into(),
        ));

        scheduler
            .expect_schedule()
            .with(eq(schedule), eq(message))
            .return_once(move |_, _| Ok(id_clone))
            .once();

        let config = Config { port: 8081 };
        let grpc_server = GrpcServer::new(config, Arc::new(scheduler));

        let request = ScheduleMessageRequest {
            schedule: Some(proto::schedule_message_request::Schedule::Delayed(
                proto::Delayed {
                    transmit_at: Some(std::time::SystemTime::from(now).into()),
                },
            )),
            message: Some(proto::schedule_message_request::Message::NatsEvent(
                proto::NatsEvent {
                    subject: event_subject,
                    payload: event_payload.into(),
                },
            )),
        };

        let response = grpc_server
            .schedule_message(tonic::Request::new(request))
            .await
            .expect("unexpected failure");

        assert_eq!(
            response.into_inner().schedule_entry_id,
            expected_id.to_string()
        );
    }
}
