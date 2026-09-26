use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::settings::{LengthRules, Settings, Validation};

pub const MAX_MODELS: usize = 16;
pub const MAX_HISTORY: usize = 12;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    HttpHeader,
    SseMetadata,
    WebsocketMetadata,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sample {
    pub observed_at_ms: u64,
    pub length: u32,
    pub fingerprint: String,
    pub source: Source,
    pub validation: Validation,
    pub event_id: String,
}

impl Sample {
    // 仅保留不可用于重放的摘要，State 原文不进入持久化或管理响应。
    pub fn capture(value: &[u8], source: Source, observed_at_ms: u64) -> Option<Self> {
        if value.is_empty() || value.len() > 16 * 1024 {
            return None;
        }
        let hash = Sha256::digest(value);
        let fingerprint = hash[..6].iter().map(|byte| format!("{byte:02x}")).collect();
        Some(Self {
            observed_at_ms,
            length: u32::try_from(value.len()).ok()?,
            fingerprint,
            source,
            validation: Validation::Unrestricted,
            event_id: String::new(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRecord {
    pub model: String,
    pub history: Vec<Sample>,
}

#[derive(Default, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRecords {
    pub models: Vec<ModelRecord>,
}

impl AccountRecords {
    pub fn record(&mut self, model: &str, samples: Vec<Sample>, settings: &Settings) {
        if !settings.observes(model) || samples.is_empty() {
            return;
        }
        if !self.models.iter().any(|entry| entry.model == model) {
            if self.models.len() >= MAX_MODELS {
                self.models
                    .sort_by_key(|entry| entry.history.last().map(|sample| sample.observed_at_ms));
                self.models.remove(0);
            }
            self.models.push(ModelRecord {
                model: model.to_owned(),
                history: Vec::new(),
            });
        }
        if let Some(entry) = self.models.iter_mut().find(|entry| entry.model == model) {
            let Ok(rules) = settings.validate() else {
                return;
            };
            for mut sample in samples {
                if entry
                    .history
                    .iter()
                    .any(|old| old.event_id == sample.event_id)
                {
                    continue;
                }
                sample.validation = rules.classify(sample.length);
                entry.history.push(sample);
            }
            entry.history.sort_by_key(|sample| sample.observed_at_ms);
            let excess = entry.history.len().saturating_sub(MAX_HISTORY);
            entry.history.drain(..excess);
        }
    }

    pub fn views(&self, settings: &Settings, now_ms: u64) -> Vec<ModelView> {
        let mut result: Vec<_> = self
            .models
            .iter()
            .filter_map(|entry| {
                let latest = entry.history.last()?;
                let expires_at_ms = latest
                    .observed_at_ms
                    .saturating_add(settings.retention_ms());
                let retained: Vec<_> = entry
                    .history
                    .iter()
                    .filter(|sample| {
                        sample
                            .observed_at_ms
                            .saturating_add(settings.retention_ms())
                            > now_ms
                    })
                    .cloned()
                    .collect();
                let mut counts = BTreeMap::<u32, u64>::new();
                for sample in &retained {
                    *counts.entry(sample.length).or_default() += 1;
                }
                let matched = retained
                    .iter()
                    .filter(|sample| sample.validation == Validation::Matched)
                    .count();
                let mismatched = retained
                    .iter()
                    .filter(|sample| sample.validation == Validation::Mismatch)
                    .count();
                Some(ModelView {
                    model: entry.model.clone(),
                    observations: retained.len(),
                    matched,
                    mismatched,
                    last_observed_at_ms: latest.observed_at_ms,
                    expires_at_ms,
                    latest_length: latest.length,
                    latest_fingerprint: latest.fingerprint.clone(),
                    source: latest.source,
                    validation: latest.validation,
                    expired: expires_at_ms <= now_ms,
                    lengths: counts
                        .into_iter()
                        .map(|(length, count)| LengthCount { length, count })
                        .collect(),
                    history: retained.into_iter().rev().collect(),
                })
            })
            .collect();
        result.sort_by(|left, right| {
            right
                .last_observed_at_ms
                .cmp(&left.last_observed_at_ms)
                .then_with(|| left.model.cmp(&right.model))
        });
        result
    }

    pub fn reclassify(&mut self, rules: &LengthRules) {
        for entry in &mut self.models {
            for sample in &mut entry.history {
                sample.validation = rules.classify(sample.length);
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    pub model: String,
    pub observations: usize,
    pub matched: usize,
    pub mismatched: usize,
    pub last_observed_at_ms: u64,
    pub expires_at_ms: u64,
    pub latest_length: u32,
    pub latest_fingerprint: String,
    pub source: Source,
    pub validation: Validation,
    pub expired: bool,
    pub lengths: Vec<LengthCount>,
    pub history: Vec<Sample>,
}

#[derive(Serialize)]
pub struct LengthCount {
    pub length: u32,
    pub count: u64,
}
