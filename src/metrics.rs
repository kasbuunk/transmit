use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::{Atomic, Counter};
use prometheus_client::metrics::family::Family;
use prometheus_client::registry::Registry;

use crate::contract;
use crate::model::MetricEvent;

const METRIC_NAME: &str = "procedure";
const METRIC_HELP_TEXT: &str = "Number of procedure calls";

pub struct PrometheusClient {
    registry: Registry,
    procedure_metric: Family<ResultLabel, Counter>,
}

impl PrometheusClient {
    pub fn new() -> PrometheusClient {
        let mut registry = <Registry>::default();

        let procedure_metric = Family::<ResultLabel, Counter>::default();

        registry.register(METRIC_NAME, METRIC_HELP_TEXT, procedure_metric.clone());

        PrometheusClient {
            registry,
            procedure_metric,
        }
    }
}

impl contract::Metrics for PrometheusClient {
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

    use crate::contract::Metrics;

    use prometheus_client::encoding::text::encode;

    #[test]
    fn test_metric() {
        let prometheus_client = PrometheusClient::new();
        prometheus_client.count(MetricEvent::Polled(false));

        let mut buffer = String::new();
        encode(&mut buffer, &prometheus_client.registry).unwrap();

        let expected = format!(
            "# HELP {METRIC_NAME} {METRIC_HELP_TEXT}.
# TYPE {METRIC_NAME} counter
{METRIC_NAME}_total{{procedure=\"Polled\",result=\"Failure\"}} 1
# EOF
",
        );
        assert_eq!(expected, buffer);
    }
}
