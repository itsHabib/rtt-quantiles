use ::aws_sdk_dynamodb::Client;
use anyhow::Result;
use aws_config::{from_env, meta::region::RegionProviderChain};
use axum::{
    Router,
    extract::{Json, Query, State},
    http::StatusCode,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::usize;
use tdigest::TDigest;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    let svc = tdigest_svc().await;
    let rtr = Router::new()
        .route("/quantiles", get(get_quantiles))
        .with_state(svc);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, rtr).await?;

    Ok(())
}
async fn get_quantiles(
    Query(q): Query<QuantilesRequest>,
    State(svc): State<rtt_tdigest::Service>,
) -> Result<Json<QuantilesResponse>, StatusCode> {
    println!("[GET] /quantiles from: {}, to: {}", q.from, q.to);

    let tdigests = match svc.query_digests("1m", q.from, q.to).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error querying digests: {}", e);

            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match tdigests.len() {
        0 => Ok(Json(QuantilesResponse {
            agg_level: "1m".to_string(),
            sample_count: 0,
            quantiles: HashMap::new(),
        })),
        _ => {
            let merged = TDigest::merge_digests(tdigests);
            let quantiles = HashMap::from([
                (
                    "p99".to_string(),
                    format!("{:.3}", merged.estimate_quantile(0.99)),
                ),
                (
                    "p95".to_string(),
                    format!("{:.3}", merged.estimate_quantile(0.95)),
                ),
                (
                    "p90".to_string(),
                    format!("{:.3}", merged.estimate_quantile(0.90)),
                ),
                (
                    "p75".to_string(),
                    format!("{:.3}", merged.estimate_quantile(0.75)),
                ),
                (
                    "p50".to_string(),
                    format!("{:.3}", merged.estimate_quantile(0.50)),
                ),
            ]);

            Ok(Json(QuantilesResponse {
                agg_level: "1m".to_string(),
                sample_count: merged.count() as usize,
                quantiles,
            }))
        }
    }
}

async fn tdigest_svc() -> rtt_tdigest::Service {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = from_env().region(region_provider).load().await;
    let client = Client::new(&config);

    rtt_tdigest::Service::new(client, "sample-app".to_string(), "local".to_string())
}

#[derive(Deserialize)]
struct QuantilesRequest {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

#[derive(Serialize)]
struct QuantilesResponse {
    agg_level: String,
    sample_count: usize,
    quantiles: HashMap<String, String>,
}
