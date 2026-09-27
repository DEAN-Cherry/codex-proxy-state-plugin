use std::{collections::BTreeMap, time::Duration};

use sha2::{Digest as _, Sha256};

use crate::{records::Sample, settings::valid_identifier};

const MAX_REQUESTS: usize = 1024;
const MAX_SAMPLES: usize = 64;
const PENDING_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribution {
    pub account: String,
    pub model: String,
}

impl Attribution {
    pub fn new(account: Option<&str>, model: Option<&str>) -> Option<Self> {
        let (account, model) = (account?, model?);
        (valid_identifier(account) && valid_identifier(model)).then(|| Self {
            account: account.to_owned(),
            model: model.to_owned(),
        })
    }
}

pub struct Batch {
    pub owner: Attribution,
    pub samples: Vec<Sample>,
}

struct Pending {
    created: Duration,
    headers: Option<Vec<Sample>>,
    terminal: Option<Option<Attribution>>,
    batches: Vec<Batch>,
    count: usize,
}

#[derive(Default)]
pub struct Collector {
    requests: BTreeMap<String, Pending>,
    attempts: BTreeMap<(String, u32), (Duration, Attribution)>,
    pub dropped: u64,
    pub unattributed: u64,
}

impl Collector {
    fn entry(&mut self, id: &str, now: Duration) -> &mut Pending {
        let before = self.requests.len();
        self.requests
            .retain(|_, entry| now.saturating_sub(entry.created) < PENDING_TTL);
        self.dropped = self
            .dropped
            .saturating_add(u64::try_from(before - self.requests.len()).unwrap_or(u64::MAX));
        if !self.requests.contains_key(id)
            && self.requests.len() >= MAX_REQUESTS
            && let Some(oldest) = self
                .requests
                .iter()
                .min_by_key(|(_, entry)| entry.created)
                .map(|(key, _)| key.clone())
        {
            self.requests.remove(&oldest);
            self.dropped = self.dropped.saturating_add(1);
        }
        self.requests
            .entry(id.to_owned())
            .or_insert_with(|| Pending {
                created: now,
                headers: Some(Vec::new()),
                terminal: None,
                batches: Vec::new(),
                count: 0,
            })
    }

    pub fn begin(&mut self, id: &str, now: Duration) {
        self.entry(id, now).headers = None;
    }

    pub fn attempt(&mut self, id: &str, index: u32, owner: Attribution, now: Duration) {
        self.attempts
            .retain(|_, (created, _)| now.saturating_sub(*created) < PENDING_TTL);
        let key = (id.to_owned(), index);
        if !self.attempts.contains_key(&key)
            && self.attempts.len() >= MAX_REQUESTS
            && let Some(oldest) = self
                .attempts
                .iter()
                .min_by_key(|(_, (created, _))| *created)
                .map(|(key, _)| key.clone())
        {
            self.attempts.remove(&oldest);
            self.dropped = self.dropped.saturating_add(1);
        }
        self.attempts.insert(key, (now, owner));
    }

    pub fn attempt_owner(&mut self, id: &str, index: u32, now: Duration) -> Option<Attribution> {
        self.attempts
            .retain(|_, (created, _)| now.saturating_sub(*created) < PENDING_TTL);
        self.attempts
            .get(&(id.to_owned(), index))
            .map(|(_, owner)| owner.clone())
    }

    pub fn headers(&mut self, id: &str, mut samples: Vec<Sample>, now: Duration) -> Vec<Batch> {
        for (index, sample) in samples.iter_mut().enumerate() {
            sample.event_id = event_id(id, &format!("header:{index}"));
        }
        self.entry(id, now).headers = Some(samples);
        self.claim(id)
    }

    pub fn terminal(&mut self, id: &str, owner: Option<Attribution>, now: Duration) -> Vec<Batch> {
        self.entry(id, now).terminal = Some(owner);
        self.claim(id)
    }

    pub fn samples(&mut self, id: &str, batch: Batch, now: Duration) {
        let entry = self.entry(id, now);
        let remaining = MAX_SAMPLES.saturating_sub(entry.count);
        let skipped = batch.samples.len().saturating_sub(remaining);
        let samples: Vec<_> = batch.samples.into_iter().take(remaining).collect();
        entry.count += samples.len();
        if !samples.is_empty() {
            entry.batches.push(Batch {
                owner: batch.owner,
                samples,
            });
        }
        self.dropped = self
            .dropped
            .saturating_add(u64::try_from(skipped).unwrap_or(u64::MAX));
    }

    fn claim(&mut self, id: &str) -> Vec<Batch> {
        if !self
            .requests
            .get(id)
            .is_some_and(|entry| entry.headers.is_some() && entry.terminal.is_some())
        {
            return Vec::new();
        }
        let Some(entry) = self.requests.remove(id) else {
            return Vec::new();
        };
        let mut batches = entry.batches;
        let headers = entry.headers.unwrap_or_default();
        match entry.terminal.flatten() {
            Some(owner) if !headers.is_empty() => batches.push(Batch {
                owner,
                samples: headers,
            }),
            None => {
                self.unattributed = self
                    .unattributed
                    .saturating_add(u64::try_from(headers.len()).unwrap_or(u64::MAX))
            }
            Some(_) => {}
        }
        batches
    }
}

pub fn event_id(request: &str, source: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(request.as_bytes());
    hash.update([0]);
    hash.update(source.as_bytes());
    format!("{:x}", hash.finalize())
}
