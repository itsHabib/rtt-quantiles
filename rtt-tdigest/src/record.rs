use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tdigest::TDigest;

#[derive(Serialize, Deserialize)]
pub(crate) struct TDigestRecord {
    pub key: String,
    pub app: String,
    pub agg_level: String,
    pub created_at: DateTime<Utc>,
    pub node_id: String,
    pub tdigest: TDigest,
}
