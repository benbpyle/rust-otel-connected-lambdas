use std::{collections::HashMap, env};

use aws_lambda_events::sqs::SqsEvent;
use lambda_runtime::{LambdaEvent, Runtime};
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_datadog::{new_pipeline, ApiVersion};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use serde::Deserialize;
use tower::{service_fn, BoxError};
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{prelude::*, Registry};

#[derive(Deserialize, Clone)]
struct MessageBody {
    timestamp: i64,
    description: String,
    id: String,
    correlation_id: String,
}

impl std::fmt::Display for MessageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(Timestamp)={}|(Description)={}|(Id)={}|(CorrelationId)={}",
            self.timestamp, self.description, self.id, self.correlation_id
        )
    }
}

#[instrument(name = "Handler")]
async fn handler(event: LambdaEvent<SqsEvent>) -> Result<(), &'static str> {
    event.payload.records.into_iter().for_each(|record| {
        let r: MessageBody = serde_json::from_str(record.body.unwrap().as_ref()).unwrap();
        let mut fields: HashMap<String, String> = HashMap::new();
        fields.insert("traceparent".to_string(), r.correlation_id.clone());

        let propagator = TraceContextPropagator::new();
        let context = propagator.extract(&fields);
        let span = tracing::Span::current();
        span.set_parent(context);
        span.record("otel.kind", "SERVER");
        tracing::info!("(Body)={}", r.clone());
        tracing::info_span!("Processing Record");
    });
    Ok(())
}

fn init_datadog_pipeline() -> opentelemetry_sdk::trace::Tracer {
    let agent_address = env::var("AGENT_ADDRESS").expect("AGENT_ADDRESS is required");
    match new_pipeline()
        .with_service_name(env::var("FUNCTION_NAME").expect("FUNCTION_NAME is required"))
        .with_agent_endpoint(format!("http://{}:8126", agent_address))
        .with_api_version(ApiVersion::Version05)
        .install_simple()
    {
        Ok(a) => a,
        Err(e) => {
            panic!("error starting! {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(init_datadog_pipeline());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .with_current_span(false)
        .without_time();

    Registry::default()
        .with(telemetry_layer)
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let runtime = Runtime::new(service_fn(handler));
    runtime.run().await?;
    Ok(())
}
