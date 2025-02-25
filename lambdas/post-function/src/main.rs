use std::{collections::HashMap, env};

use aws_config::BehaviorVersion;
use aws_sdk_sqs::operation::send_message::SendMessageError;
use lambda_http::{run, service_fn, Body, Request, Response};
use opentelemetry::global;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry_datadog::{new_pipeline, ApiVersion};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use serde::{Deserialize, Serialize};
use tracing::{instrument, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};
use uuid::Uuid;

#[derive(Deserialize)]
struct LambdaResponse {
    timestamp: i64,
    description: String,
}

#[derive(Serialize, Debug)]
struct MessageBody {
    timestamp: i64,
    description: String,
    id: String,
    correlation_id: String,
}

#[tokio::main]
async fn main() -> Result<(), lambda_http::Error> {
    global::set_text_map_propagator(opentelemetry_sdk::propagation::TraceContextPropagator::new());
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(init_datadog_pipeline());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .without_time();

    Registry::default()
        .with(telemetry_layer)
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let sdk_config = aws_config::load_defaults(BehaviorVersion::v2024_03_28()).await;
    let config = aws_sdk_sqs::config::Builder::from(&sdk_config).build();
    let client = aws_sdk_sqs::Client::from_conf(config);
    let reqwest_client = reqwest::Client::builder().build().unwrap();
    let http_client = ClientBuilder::new(reqwest_client)
        .with(TracingMiddleware::default())
        .build();
    let shared_client = &client;
    let shared_http_client = &http_client;
    let service_url = env::var("API_URL").expect("API_URL is required");
    let queue_url = env::var("QUEUE_URL").expect("QUEUE_URL is required");
    let shared_service_url = service_url.as_str();
    let shared_queue_url = queue_url.as_str();

    run(service_fn(move |event: Request| async move {
        handler(
            shared_client,
            shared_http_client,
            shared_service_url,
            shared_queue_url,
            event,
        )
        .await
    }))
    .await
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

#[instrument(name = "Handler")]
async fn handler(
    client: &aws_sdk_sqs::Client,
    http_client: &reqwest_middleware::ClientWithMiddleware,
    service_url: &str,
    queue_url: &str,
    _request: Request,
) -> Result<Response<Body>, lambda_http::Error> {
    let ctx = Span::current().context();
    let propagator = TraceContextPropagator::new();
    let mut fields = HashMap::new();

    let mut trace_parent: Option<String> = None;

    propagator.inject_context(&ctx, &mut fields);
    let headers = fields
        .into_iter()
        .map(|(k, v)| {
            if k == "traceparent" {
                trace_parent = Some(v.clone());
            }
            return (
                HeaderName::try_from(k).unwrap(),
                HeaderValue::try_from(v).unwrap(),
            );
        })
        .collect();

    let response = http_client
        .get(format!("{}/demo", service_url))
        .headers(headers)
        .send()
        .await;

    match response {
        Ok(r) => {
            let m: Result<LambdaResponse, reqwest::Error> = r.json().await;
            match m {
                Ok(b) => {
                    let _ = post_message(
                        client,
                        queue_url,
                        MessageBody {
                            timestamp: b.timestamp,
                            description: b.description,
                            id: Uuid::new_v4().to_string(),
                            correlation_id: "".to_string(),
                        },
                        trace_parent,
                    )
                    .await;
                }
                Err(e) => {
                    tracing::error!("Error parsing: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::error!("(Error)={}", e);
        }
    }
    let response = Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body("".into())
        .map_err(Box::new)?;
    Ok(response)
}

#[instrument(name = "Post Message")]
async fn post_message(
    client: &aws_sdk_sqs::Client,
    queue_url: &str,
    mut payload: MessageBody,
    trace_parent: Option<String>,
) -> Result<(), aws_sdk_sqs::error::SdkError<SendMessageError>> {
    match trace_parent {
        Some(x) => {
            payload.correlation_id = x;
        }
        None => payload.correlation_id = "".to_string(),
    }
    let span = tracing::info_span!("SQS");
    let message = serde_json::to_string(&payload).unwrap();
    client
        .send_message()
        .queue_url(queue_url)
        .message_body(&message)
        .send()
        .instrument(span)
        .await?;

    Ok(())
}
