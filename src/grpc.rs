use std::error::Error;
use std::sync::Arc;
use std::time::SystemTime;

#[cfg(test)]
use mockall::predicate::*;

use chrono::prelude::*;
use futures_util::FutureExt;
use log::{error, info};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tonic::{transport::Server, Request, Response, Status};

use crate::contract::Scheduler;
use crate::model::*;

pub mod proto {
    tonic::include_proto!("scheduler");
}
use proto::health_server::HealthServer;
use proto::scheduler_server::SchedulerServer;
use proto::{HealthCheckRequest, HealthCheckResponse};
use proto::{ScheduleTransmissionRequest, ScheduleTransmissionResponse};

use self::proto::health_check_response::ServingStatus;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub port: u16,
}

#[derive(Clone)]
pub struct GrpcServer {
    config: Config,
    scheduler: Arc<dyn Scheduler + Send + Sync>,
}

impl GrpcServer {
    pub fn new(config: Config, scheduler: Arc<dyn Scheduler + Send + Sync>) -> GrpcServer {
        GrpcServer { config, scheduler }
    }

    pub async fn serve(&self, cancel_token: CancellationToken) -> Result<(), Box<dyn Error>> {
        let host = "0.0.0.0";
        let address = format!("{}:{}", host, self.config.port).parse()?;
        let scheduler_server = SchedulerServer::new(self.clone());
        let health_server = HealthServer::new(self.clone());

        info!("Start listening for incoming messages at {}.", address);

        let server = Server::builder()
            .add_service(health_server)
            .add_service(scheduler_server);

        // Create a signal channel for graceful shutdown.
        let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<()>();

        // Spawn a task to wait for the cancellation token or shutdown signal.
        let serve_task = tokio::spawn(async move {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Handle cancellation
                    let _ = shutdown_sender.send(());
                    info!("Shutting down gRPC server");
                }
                _ = server.serve_with_shutdown(address, shutdown_receiver.map(drop)) => {
                }
            }
        });

        serve_task.await?;

        info!("gRPC server was shut down.");

        Ok(())
    }
}

#[tonic::async_trait]
impl proto::scheduler_server::Scheduler for GrpcServer {
    async fn schedule_transmission(
        &self,
        request: Request<ScheduleTransmissionRequest>,
    ) -> Result<Response<ScheduleTransmissionResponse>, Status> {
        info!("ScheduleMessage request received");

        let request_data = request.into_inner();
        let schedule_proto = match request_data.schedule {
            None => return Err(Status::invalid_argument("schedule is required")),
            Some(schedule) => schedule,
        };
        let schedule = match schedule_proto {
            proto::schedule_transmission_request::Schedule::Delayed(delayed) => {
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

                Schedule::Delayed(Delayed::new(timestamp_utc))
            }
        };
        let message_proto = match request_data.message {
            None => return Err(Status::invalid_argument("message is required")),
            Some(message) => message,
        };
        let message = match message_proto {
            proto::schedule_transmission_request::Message::NatsEvent(event) => {
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

                Ok(Response::new(ScheduleTransmissionResponse {
                    transmission_id: id.to_string(),
                }))
            }
            Err(err) => {
                error!("Failed to schedule message: {err}");

                Err(Status::internal("internal server error"))
            }
        }
    }
}

#[tonic::async_trait]
impl proto::health_server::Health for GrpcServer {
    type WatchStream = tonic::Streaming<HealthCheckResponse>;

    async fn check(
        &self,
        request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        // Implement the logic to check the health status of the specified service
        let _request_service_name = request.into_inner().service;
        // Here you should check if the service is healthy and return the appropriate status
        let serving_status = ServingStatus::Serving; // Or NOT_SERVING based on your logic

        Ok(tonic::Response::new(HealthCheckResponse {
            status: serving_status.into(),
        }))
    }

    async fn watch(
        &self,
        _request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<Self::WatchStream>, tonic::Status> {
        // Implement the logic to watch the health status of the specified service
        // Here you should return a stream that notifies the client whenever the service's health status changes
        // This could be implemented using tokio::sync::watch or any other suitable mechanism
        unimplemented!()
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
    async fn test_schedule_transmission() {
        let expected_id = uuid::uuid!("a23bfa0f-a906-429a-ab90-66322dfa72e5");
        let id_clone = expected_id.clone();
        let mut scheduler = MockScheduler::new();

        let event_subject = "some_subject".to_string();
        let event_payload = Bytes::from("some_payload");

        let now = Utc::now();
        let schedule = Schedule::Delayed(Delayed::new(now));
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

        let request = ScheduleTransmissionRequest {
            schedule: Some(proto::schedule_transmission_request::Schedule::Delayed(
                proto::Delayed {
                    transmit_at: Some(std::time::SystemTime::from(now).into()),
                },
            )),
            message: Some(proto::schedule_transmission_request::Message::NatsEvent(
                proto::NatsEvent {
                    subject: event_subject,
                    payload: event_payload.into(),
                },
            )),
        };

        let response = grpc_server
            .schedule_transmission(tonic::Request::new(request))
            .await
            .expect("unexpected failure");

        assert_eq!(
            response.into_inner().transmission_id,
            expected_id.to_string()
        );
    }
}
