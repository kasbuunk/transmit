use std::error::Error;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::SystemTime;

#[cfg(test)]
use mockall::predicate::*;

use chrono::prelude::*;
use futures_util::FutureExt;
use log::{error, info};
use serde::Deserialize;
use tokio_stream::Stream;
use tokio_util::sync::CancellationToken;
use tonic::{transport::Server, Request, Response, Status};

use crate::contract::Scheduler;
use crate::model::*;

pub mod proto {
    tonic::include_proto!("transmit");
}
use proto::health_server::HealthServer;
use proto::transmit_server::TransmitServer;
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
        let transmit_server = TransmitServer::new(self.clone());
        let health_server = HealthServer::new(self.clone());

        info!("Start listening for incoming messages at {}.", address);

        let server = Server::builder()
            .add_service(health_server)
            .add_service(transmit_server);

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
impl proto::transmit_server::Transmit for GrpcServer {
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
                        return Err(Status::invalid_argument("delayed.transmit_at is required"));
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
            proto::schedule_transmission_request::Schedule::Interval(schedule) => {
                let timestamp = match schedule.first_transmission {
                    None => {
                        return Err(Status::invalid_argument(
                            "interval.first_transmission is required",
                        ));
                    }
                    Some(timestamp) => timestamp,
                };
                let system_time = match SystemTime::try_from(timestamp) {
                    Err(err) => {
                        error!("failed to parse as system time: {err}");

                        return Err(Status::invalid_argument(
                            "interval.first_transmission could not be parsed as SystemTime",
                        ));
                    }
                    Ok(system_time) => system_time,
                };
                let timestamp_utc = DateTime::<Utc>::from(system_time);

                let interval_length = match schedule.interval {
                    None => {
                        return Err(Status::invalid_argument("interval.interval is required"));
                    }
                    Some(interval_length) => match std::time::Duration::try_from(interval_length) {
                        Err(err) => {
                            return Err(Status::invalid_argument(format!(
                                "parsing interval.interval as std::time::Duration: {}",
                                err
                            )));
                        }
                        Ok(duration) => duration,
                    },
                };

                let iterate = match schedule.iterate {
                    None => return Err(Status::invalid_argument("interval.iterate is required")),
                    Some(iterate) => match iterate {
                        proto::interval::Iterate::Infinitely(should_be_true) => {
                            if !should_be_true {
                                return Err(Status::invalid_argument(
                                    "interval.iterate.infinitely should be true if set",
                                ));
                            }

                            Iterate::Infinitely
                        }
                        proto::interval::Iterate::Times(iterations) => Iterate::Times(iterations),
                    },
                };

                Schedule::Interval(Interval::new(timestamp_utc, interval_length, iterate))
            }
            proto::schedule_transmission_request::Schedule::Cron(schedule) => {
                let timestamp = match schedule.first_transmission_after {
                    None => {
                        return Err(Status::invalid_argument(
                            "cron.first_transmission_after is required",
                        ));
                    }
                    Some(timestamp) => timestamp,
                };
                let system_time = match SystemTime::try_from(timestamp) {
                    Err(err) => {
                        error!("failed to parse as system time: {err}");

                        return Err(Status::invalid_argument(
                            "cron.first_transmission_after could not be parsed as SystemTime",
                        ));
                    }
                    Ok(system_time) => system_time,
                };
                let timestamp_utc = DateTime::<Utc>::from(system_time);

                let cron_expression = match cron::Schedule::from_str(&schedule.expression) {
                    Err(err) => {
                        return Err(Status::invalid_argument(format!(
                            "parsing cron schedule as cron::Schedule: {}. The format must include seven specifiers: `sec  min   hour   day of month   month   day of week   year` (including the first field for seconds and the last for years). For example: `0   30   9,12,15     1,15       May-Aug  Mon,Wed,Fri  2018/2`;
",
                            err
                        )));
                    }
                    Ok(cron_expression) => cron_expression,
                };

                let iterate = match schedule.iterate {
                    None => return Err(Status::invalid_argument("interval.iterate is required")),
                    Some(iterate) => match iterate {
                        proto::cron::Iterate::Infinitely(should_be_true) => {
                            if !should_be_true {
                                return Err(Status::invalid_argument(
                                    "interval.iterate.infinitely should be true if set",
                                ));
                            }

                            Iterate::Infinitely
                        }
                        proto::cron::Iterate::Times(repetitions) => Iterate::Times(repetitions),
                    },
                };

                Schedule::Cron(Cron::new(timestamp_utc, cron_expression, iterate))
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
            Err(ScheduleError::AgedSchedule) => Err(Status::invalid_argument(
                "aged schedule; provide a timestamp in the future",
            )),
            Err(ScheduleError::TooShortInterval) => Err(Status::invalid_argument(
                "too short interval; provide a greater duration between transmissions",
            )),
            Err(ScheduleError::NatsInvalidSubject) => Err(Status::invalid_argument(
                "provided nats subject not allowed",
            )),
            Err(err) => {
                error!("Failed to schedule message: {err}");

                Err(Status::internal("internal server error"))
            }
        }
    }
}

#[tonic::async_trait]
impl proto::health_server::Health for GrpcServer {
    type WatchStream =
        Pin<Box<dyn Stream<Item = Result<HealthCheckResponse, Status>> + Send + 'static>>;

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
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let _service_name = request.get_ref().service.as_str();

        let output = async_stream::try_stream! {
            let status = ServingStatus::Serving as i32;

            yield HealthCheckResponse { status }
        };

        Ok(Response::new(Box::pin(output) as Self::WatchStream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use chrono::Utc;

    use crate::contract::*;
    use crate::grpc::proto::transmit_server::Transmit;

    struct TestCase {
        name: String,
        schedule_proto: proto::schedule_transmission_request::Schedule,
        message_proto: proto::schedule_transmission_request::Message,
        expected_schedule: Schedule,
        expected_message: Message,
    }

    #[tokio::test]
    async fn test_schedule_transmissions() {
        let expected_id = uuid::uuid!("a23bfa0f-a906-429a-ab90-66322dfa72e5");
        let id_clone = expected_id.clone();

        let event_subject = "some_subject".to_string();
        let event_payload = Bytes::from("some_payload");
        let expected_message = Message::NatsEvent(NatsEvent::new(
            event_subject.clone(),
            event_payload.clone().into(),
        ));

        let now = Utc::now();

        let message_proto =
            proto::schedule_transmission_request::Message::NatsEvent(proto::NatsEvent {
                subject: event_subject,
                payload: event_payload.into(),
            });

        let test_cases = vec![
            TestCase {
                name: "delayed".to_string(),
                schedule_proto: proto::schedule_transmission_request::Schedule::Delayed(
                    proto::Delayed {
                        transmit_at: Some(std::time::SystemTime::from(now).into()),
                    },
                ),
                message_proto: message_proto.clone(),
                expected_schedule: Schedule::Delayed(Delayed::new(now)),
                expected_message: expected_message.clone(),
            },
            TestCase {
                name: "interval_infinitely".to_string(),
                schedule_proto: proto::schedule_transmission_request::Schedule::Interval(
                    proto::Interval {
                        first_transmission: Some(std::time::SystemTime::from(now).into()),
                        interval: Some(
                            std::time::Duration::from_secs(2)
                                .try_into()
                                .expect("interval is not too large to be prost duration"),
                        ),
                        iterate: Some(proto::interval::Iterate::Times(3)),
                    },
                ),
                message_proto: message_proto.clone(),
                expected_schedule: Schedule::Interval(Interval::new(
                    now,
                    std::time::Duration::from_secs(2),
                    Iterate::Times(3),
                )),
                expected_message: expected_message.clone(),
            },
            TestCase {
                name: "interval_times_3".to_string(),
                schedule_proto: proto::schedule_transmission_request::Schedule::Interval(
                    proto::Interval {
                        first_transmission: Some(std::time::SystemTime::from(now).into()),
                        interval: Some(
                            std::time::Duration::from_secs(2)
                                .try_into()
                                .expect("interval is not too large to be prost duration"),
                        ),
                        iterate: Some(proto::interval::Iterate::Times(3)),
                    },
                ),
                message_proto: message_proto.clone(),
                expected_schedule: Schedule::Interval(Interval::new(
                    now,
                    std::time::Duration::from_secs(2),
                    Iterate::Times(3),
                )),
                expected_message: expected_message.clone(),
            },
            TestCase {
                name: "cron_repeated".to_string(),
                schedule_proto: proto::schedule_transmission_request::Schedule::Cron(proto::Cron {
                    first_transmission_after: Some(std::time::SystemTime::from(now).into()),
                    expression: "0 30 9,12,15 1,15 May-Aug Mon,Wed,Fri 2018/2".to_string(),
                    iterate: Some(proto::cron::Iterate::Times(3)),
                }),
                message_proto: message_proto.clone(),
                expected_schedule: Schedule::Cron(Cron::new(
                    now,
                    cron::Schedule::from_str("0 30 9,12,15 1,15 May-Aug Mon,Wed,Fri 2018/2")
                        .expect("should compile"),
                    Iterate::Times(3),
                )),
                expected_message: expected_message.clone(),
            },
        ];

        for test_case in test_cases {
            let mut scheduler = MockScheduler::new();
            scheduler
                .expect_schedule()
                .with(
                    eq(test_case.expected_schedule),
                    eq(test_case.expected_message),
                )
                .return_once(move |_, _| Ok(id_clone))
                .once();

            let config = Config { port: 8081 };
            let grpc_server = GrpcServer::new(config, Arc::new(scheduler));

            let request = ScheduleTransmissionRequest {
                schedule: Some(test_case.schedule_proto),
                message: Some(test_case.message_proto),
            };

            let response = grpc_server
                .schedule_transmission(tonic::Request::new(request))
                .await
                .expect("unexpected failure");

            assert_eq!(
                response.into_inner().transmission_id,
                expected_id.to_string(),
                "Test case failed: {}",
                test_case.name,
            );
        }
    }
}
