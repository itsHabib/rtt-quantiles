use crate::record::TDigestRecord;
use anyhow::Result;
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::AttributeValue;
use chrono;
use chrono::{DateTime, Duration, DurationRound, Utc};
use serde_json;
use std::collections::HashMap;
use tdigest::TDigest;

const TABLE_NAME: &str = "rtt-tdigests";

#[derive(Clone)]
pub struct Service {
    client: Client,
    app: String,
    node: String,
}

impl Service {
    pub fn new(client: Client, app: String, node: String) -> Self {
        Self { client, app, node }
    }

    pub async fn store_tdigest(&self, agg_level: String, tdigest: TDigest) -> Result<()> {
        let now = chrono::Utc::now();
        let created_at = now
            .duration_trunc(Duration::minutes(1))
            .unwrap_or_else(|_| now);

        let record = TDigestRecord {
            key: self.record_key(agg_level.clone()),
            app: self.app.clone(),
            agg_level,
            created_at,
            node_id: self.node.clone(),
            tdigest,
        };

        let dynanmo_hashmap = record_to_item(&record)?;

        match self
            .client
            .put_item()
            .table_name(TABLE_NAME)
            .set_item(Some(dynanmo_hashmap))
            .send()
            .await
        {
            Ok(_) => {
                println!("Successfully stored digest for {}/{}", self.app, self.node);
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to store digest: {}", e);
                Err(anyhow::anyhow!("DynamoDB storage failed: {}", e))
            }
        }
    }

    pub async fn query_digests(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<TDigest>> {
        // This function would implement querying logic to retrieve TDigestRecords
        // from DynamoDB based on the application and node.
        // For now, we will return an empty vector as a placeholder.
        Ok(vec![])
    }

    fn record_key(&self, agg_level: String) -> String {
        format!("{}:{}:{}", self.app, agg_level, self.node)
    }
}

/// Converts a TDigestRecord into a HashMap of AttributeValues ready for DynamoDB
fn record_to_item(record: &TDigestRecord) -> Result<HashMap<String, AttributeValue>> {
    let mut item = HashMap::new();

    item.insert("key".to_string(), AttributeValue::S(record.key.clone()));
    item.insert("app".to_string(), AttributeValue::S(record.app.clone()));
    item.insert(
        "agg_level".to_string(),
        AttributeValue::S(record.agg_level.clone()),
    );
    item.insert(
        "created_at".to_string(),
        AttributeValue::S(record.created_at.to_rfc3339()),
    );
    item.insert(
        "node_id".to_string(),
        AttributeValue::S(record.node_id.clone()),
    );

    let digest_json = serde_json::to_string(&record.tdigest.clone())
        .map_err(|e| anyhow::anyhow!("Failed to serialize TDigest to JSON: {}", e))?;
    item.insert("tdigest".to_string(), AttributeValue::S(digest_json));

    Ok(item)
}
