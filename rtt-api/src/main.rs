use anyhow::Result;
use std::collections::HashMap;
use std::usize;
use std::net::SocketAddr;
use tdigest::TDigest;
use axum::{
    extract::{
        Query,
        Json,
        State,
    },
    Router,
    routing::get,
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use::aws_sdk_dynamodb::Client;
use aws_config::{
    from_env,
    meta::region::RegionProviderChain
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    let svc = tdigest_svc().await;
    let rtr = Router::new().
        route("/quantiles", get(get_quantiles)).
        with_state(svc);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, rtr).await?;
    
    Ok(())
}

#[derive(Deserialize)]
struct QuantilesRequest {
    from: DateTime<Utc>,
    to: DateTime<Utc>
}

#[derive(Serialize)]
struct QuantilesResponse {
    agg_level: String,
    sample_count: usize,
    quantiles: HashMap<String, f64>,
}

async fn get_quantiles(
    Query(q): Query<QuantilesRequest>,
    State(svc): State<rtt_tdigest::Service>
) -> Result<Json<QuantilesResponse>, StatusCode> {
    let tdigests = match svc.query_digests(q.from, q.to).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error querying digests: {}", e);

            return Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    };

    if tdigests.len() == 0 {
        return Ok(Json(QuantilesResponse{
            agg_level: "1m".to_string(),
            sample_count: 0,
            quantiles: HashMap::new(),
        }));
    }

    let merged = TDigest::merge_digests(tdigests);
    let mut quantiles = HashMap::new();
    quantiles.insert("p99".to_string(), merged.estimate_quantile(0.99));
    quantiles.insert("p95".to_string(), merged.estimate_quantile(0.95));
    quantiles.insert("p90".to_string(), merged.estimate_quantile(0.90));
    quantiles.insert("p75".to_string(), merged.estimate_quantile(0.75));
    quantiles.insert("p50".to_string(), merged.estimate_quantile(0.50));

    Ok(Json(QuantilesResponse{
        agg_level: "1m".to_string(),
        sample_count: merged.count() as usize,
        quantiles,
    }))
}

async fn tdigest_svc() -> rtt_tdigest::Service {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = from_env().region(region_provider).load().await;
    let client = Client::new(&config);

    rtt_tdigest::Service::new(client, "sample-app".to_string(), "local".to_string())
}
