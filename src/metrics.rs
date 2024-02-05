use crate::contract;
use crate::model::MetricEvent;

use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Bytes, Request, Response};
use hyper_util::rt::TokioIo;
use log::info;
use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::registry::Registry;
use serde::Deserialize;
use std::{future::Future, io, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::net::TcpListener;

const METRIC_NAME: &str = "procedure";
const METRIC_HELP_TEXT: &str = "Number of procedure calls";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub port: u16,
    pub endpoint: String,
}

pub fn new(config: Config) -> (MetricClient, MetricServer) {
    let mut registry = <Registry>::default();

    let procedure_metric = Family::<ResultLabel, Counter>::default();

    registry.register(METRIC_NAME, METRIC_HELP_TEXT, procedure_metric.clone());

    (
        MetricClient { procedure_metric },
        MetricServer { config, registry },
    )
}

pub struct MetricClient {
    procedure_metric: Family<ResultLabel, Counter>,
}

pub struct MetricServer {
    config: Config,
    registry: Registry,
}

impl MetricServer {
    pub async fn start_server(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let registry = Arc::new(self.registry);

        let address = SocketAddr::from(([0, 0, 0, 0], self.config.port));

        // We create a TcpListener and bind it to the address.
        let listener = TcpListener::bind(address).await?;

        info!("Starting metrics server on {address}");

        // We start a loop to continuously accept incoming connections.
        loop {
            let (stream, _) = listener.accept().await?;

            // Use an adapter to access something implementing `tokio::io` traits as if they implement.
            // `hyper::rt` IO traits.
            let io = TokioIo::new(stream);

            let registry = registry.clone();
            // Spawn a tokio task to serve multiple connections concurrently.
            tokio::task::spawn(async move {
                // Finally, we bind the incoming connection to our service.
                if let Err(err) = http1::Builder::new()
                    // `service_fn` converts our function in a `Service`.
                    .serve_connection(io, service_fn(make_handler(registry.clone())))
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

/// make_handler returns a HTTP handler that returns the stored metrics from the registry.
fn make_handler(
    registry: Arc<Registry>,
) -> impl Fn(
    Request<hyper::body::Incoming>,
) -> Pin<Box<dyn Future<Output = io::Result<Response<Full<Bytes>>>> + Send>> {
    // This closure accepts a request and responds with the OpenMetrics encoding of our metrics.
    move |_req: Request<hyper::body::Incoming>| {
        let reg = registry.clone();

        Box::pin(async move {
            let mut buf = String::new();
            encode(&mut buf, &reg.clone())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                .map(|_| {
                    Response::builder()
                        .header(
                            hyper::header::CONTENT_TYPE,
                            "application/openmetrics-text; version=1.0.0; charset=utf-8",
                        )
                        .body(Full::new(buf.into()))
                        .unwrap()
                })
        })
    }
}

impl contract::Metrics for MetricClient {
    fn count(&self, metric_event: MetricEvent) {
        let metric_label = ResultLabel::from(metric_event);
        self.procedure_metric.get_or_create(&metric_label).inc();
    }
}

#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelSet, Debug)]
pub struct ResultLabel {
    pub procedure: Procedure,
    pub result: ResultStatus,
}

#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelValue, Debug)]
pub enum Procedure {
    Scheduled,
    Polled,
    Transmitted,
    ScheduleStateSaved,
    Rescheduled,
}

impl From<MetricEvent> for ResultLabel {
    fn from(metric_event: MetricEvent) -> ResultLabel {
        match metric_event {
            MetricEvent::Scheduled(success) => ResultLabel {
                procedure: Procedure::Scheduled,
                result: ResultStatus::from(success),
            },
            MetricEvent::Polled(success) => ResultLabel {
                procedure: Procedure::Polled,
                result: ResultStatus::from(success),
            },
            MetricEvent::Transmitted(success) => ResultLabel {
                procedure: Procedure::Transmitted,
                result: ResultStatus::from(success),
            },
            MetricEvent::ScheduleStateSaved(success) => ResultLabel {
                procedure: Procedure::ScheduleStateSaved,
                result: ResultStatus::from(success),
            },
            MetricEvent::Rescheduled(success) => ResultLabel {
                procedure: Procedure::Rescheduled,
                result: ResultStatus::from(success),
            },
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, EncodeLabelValue, Debug)]
pub enum ResultStatus {
    Success,
    Failure,
}

impl From<bool> for ResultStatus {
    fn from(value: bool) -> ResultStatus {
        match value {
            true => ResultStatus::Success,
            false => ResultStatus::Failure,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use prometheus_client::encoding::text::encode;

    use crate::contract::Metrics;

    #[test]
    fn test_metric_increment() {
        let config = Config {
            port: 0,
            endpoint: "/".to_string(),
        };
        let (prometheus_client, prometheus_server) = new(config);
        prometheus_client.count(MetricEvent::Polled(false));

        let mut buffer = String::new();
        encode(&mut buffer, &prometheus_server.registry).unwrap();

        let expected = format!(
            "# HELP {METRIC_NAME} {METRIC_HELP_TEXT}.
# TYPE {METRIC_NAME} counter
{METRIC_NAME}_total{{procedure=\"Polled\",result=\"Failure\"}} 1
# EOF
",
        );
        assert_eq!(expected, buffer);
    }

    #[tokio::test]
    async fn test_metric_server() -> Result<(), Box<dyn std::error::Error>> {
        let port = 8083;
        let config = Config {
            port,
            endpoint: "".to_string(),
        };

        let (prometheus_client, prometheus_server) = new(config);

        // Spawn thread.
        let _handle = tokio::spawn(async move {
            prometheus_server
                .start_server()
                .await
                .expect("prometheus server must start");
        });

        std::thread::sleep(std::time::Duration::from_millis(5));

        // Cause arbitrary metric mutation.
        prometheus_client.count(MetricEvent::Scheduled(true));

        // Send metric request.
        let address = format!("http://localhost:{}", port);
        let response_body = reqwest::get(address).await?.text().await?;

        // Assert metric body content.
        let expected_body = format!(
            "# HELP {METRIC_NAME} {METRIC_HELP_TEXT}.
# TYPE {METRIC_NAME} counter
{METRIC_NAME}_total{{procedure=\"Scheduled\",result=\"Success\"}} 1
# EOF
",
        );
        assert_eq!(expected_body, response_body);

        Ok(())
    }
}
