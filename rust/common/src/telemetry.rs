//! Observability: structured logging + Prometheus metrics.
//!
//! - Logs: JSON in production (machine-parseable), pretty in dev. Level via
//!   `LOG_LEVEL` / `RUST_LOG`.
//! - Metrics: a Prometheus recorder is installed process-wide; `/metrics`
//!   renders it. An axum middleware records the RED signals
//!   (Rate, Errors, Duration) per route.
//!
//! OpenTelemetry/OTLP export to Jaeger is wired at the architecture level; the
//! trace/correlation id is propagated on every request via the `x-request-id`
//! header (see `http` module) so logs can be correlated across services.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;
use metrics_exporter_prometheus::PrometheusBuilder;
pub use metrics_exporter_prometheus::PrometheusHandle;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initialise logging + metrics. Returns the Prometheus handle used by the
/// `/metrics` endpoint. Call once at process start.
pub fn init(service_name: &str) -> PrometheusHandle {
    let filter = EnvFilter::try_from_env("LOG_LEVEL")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,hyper=warn,tower_http=info"));

    let json = crate::env::or_default("LOG_FORMAT", "json") == "json";
    let registry = tracing_subscriber::registry().with(filter);

    if json {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_target(true),
            )
            .init();
    } else {
        registry
            .with(tracing_subscriber::fmt::layer().with_target(true))
            .init();
    }

    let handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    tracing::info!(service = service_name, "telemetry initialised");
    handle
}

/// axum middleware recording the RED metrics for every request, labelled by the
/// matched route template (not the raw path) to keep cardinality bounded.
pub async fn track_metrics(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().as_str().to_owned();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());

    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status.clone(),
    )
    .increment(1);

    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => path,
        "status" => status,
    )
    .record(elapsed);

    response
}
