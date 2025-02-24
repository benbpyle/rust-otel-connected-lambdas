use std::{collections::HashMap, env};

use aws_lambda_events::{
    apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse},
    http::HeaderMap,
};
use chrono::Utc;
use lambda_runtime::{Error, LambdaEvent, Runtime};
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_datadog::{new_pipeline, ApiVersion};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use serde::{Deserialize, Serialize};
use tower::service_fn;
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

#[derive(Serialize, Deserialize)]
struct AddedContext {
    timestamp: i64,
    description: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
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
    // Initialize the Lambda runtime and add OpenTelemetry tracing
    let runtime = Runtime::new(service_fn(handler));
    runtime.run().await?;
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

#[instrument(name = "AddContext")]
fn generate_context() -> AddedContext {
    AddedContext {
        timestamp: Utc::now().timestamp_millis(),
        description: "From Read".to_string(),
    }
}

#[instrument(name = "Handler")]
async fn handler(
    request: LambdaEvent<ApiGatewayProxyRequest>,
) -> Result<ApiGatewayProxyResponse, Error> {
    let mut fields: HashMap<String, String> = HashMap::new();
    fields.insert(
        "traceparent".to_string(),
        String::from(
            request
                .payload
                .headers
                .get("traceparent")
                .unwrap()
                .to_str()
                .unwrap(),
        ),
    );

    let propagator = TraceContextPropagator::new();
    let context = propagator.extract(&fields);
    let span = tracing::Span::current();
    span.set_parent(context);
    let s = generate_context();
    let out = serde_json::to_string(&s).unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());
    Ok(ApiGatewayProxyResponse {
        body: Some(out.into()),
        headers,
        status_code: 200,
        is_base64_encoded: false,
        multi_value_headers: HeaderMap::new(),
    })
}
