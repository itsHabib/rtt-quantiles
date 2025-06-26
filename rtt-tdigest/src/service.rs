use crate::record::TDigestRecord;
use anyhow::Result;
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::AttributeValue;
use chrono;
use chrono::{DateTime, Duration, DurationRound, Utc};
use serde_json::{self, ser};
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
        agg_level: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<TDigest>> {
        let from_str = from.to_rfc3339();
        let to_str = to.to_rfc3339();

        // prepare expression attribute values
        let mut expr_values = HashMap::new();
        expr_values.insert(":app".to_string(), AttributeValue::S(self.app.clone()));
        expr_values.insert(
            ":agg_level".to_string(),
            AttributeValue::S(agg_level.to_string()),
        );
        expr_values.insert(":from".to_string(), AttributeValue::S(from_str));
        expr_values.insert(":to".to_string(), AttributeValue::S(to_str));

        // prepare expression attribute names
        let mut expr_names = HashMap::new();
        expr_names.insert("#app".to_string(), "app".to_string());
        expr_names.insert("#agg_level".to_string(), "agg_level".to_string());
        expr_names.insert("#created_at".to_string(), "created_at".to_string());

        // form the filter expression
        let filter_expr =
            "#app = :app AND #agg_level = :agg_level AND #created_at BETWEEN :from AND :to"
                .to_string();

        let scan_output = match self
            .client
            .scan()
            .table_name(TABLE_NAME)
            .filter_expression(filter_expr)
            .set_expression_attribute_names(Some(expr_names))
            .set_expression_attribute_values(Some(expr_values))
            .send()
            .await
        {
            Ok(output) => output,
            Err(err) => {
                eprintln!("DynamoDB error details: {:?}", err);
                return Err(err.into());
            }
        };

        // get and deserialize from results
        let tdigests = scan_output
            .items
            .unwrap_or_default()
            .iter()
            .filter_map(|item| match item.get("tdigest") {
                Some(AttributeValue::S(v)) => Some(v),
                _ => None,
            })
            .map(|tdigest| {
                serde_json::from_str::<TDigest>(tdigest)
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize TDigest: {}", e))
            })
            .collect::<Result<Vec<TDigest>>>()?;

        println!("found {} digests to merge", tdigests.len());

        Ok(tdigests)
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
