use std::{
    collections::BTreeMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gateway_plugin_sdk::{PluginFault, client::HostClient};
use serde::Serialize;

use crate::{
    collector::{Batch, Collector},
    records::AccountRecords,
    settings::Settings,
    store,
};

pub struct AppState {
    collector: Mutex<Collector>,
    pub storage: tokio::sync::Mutex<()>,
    pub storage_failures: AtomicU64,
    epoch: Instant,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            collector: Mutex::new(Collector::default()),
            storage: tokio::sync::Mutex::new(()),
            storage_failures: AtomicU64::new(0),
            epoch: Instant::now(),
        }
    }
}

impl AppState {
    pub fn collector(&self) -> MutexGuard<'_, Collector> {
        self.collector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn elapsed(&self) -> Duration {
        self.epoch.elapsed()
    }

    pub fn diagnostics(&self) -> Diagnostics {
        let collector = self.collector();
        Diagnostics {
            unattributed: collector.unattributed,
            dropped: collector.dropped,
            storage_failures: self.storage_failures.load(Ordering::Relaxed),
        }
    }

    pub async fn flush(&self, host: &HostClient, batches: Vec<Batch>) {
        if batches.is_empty() {
            return;
        }
        // 观测写入失败只累计诊断，不改变原有响应；期限包含等待本插件的写锁。
        let result =
            tokio::time::timeout(Duration::from_millis(100), self.persist(host, batches)).await;
        if !matches!(result, Ok(Ok(()))) {
            self.storage_failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    async fn persist(&self, host: &HostClient, batches: Vec<Batch>) -> Result<(), PluginFault> {
        let _guard = self.storage.lock().await;
        let mut accounts = BTreeMap::<String, Vec<Batch>>::new();
        for batch in batches {
            accounts
                .entry(batch.owner.account.clone())
                .or_default()
                .push(batch);
        }
        for (account, batches) in accounts {
            let (settings, _) = store::get::<Settings>(host, "settings", &account).await?;
            if !settings.enabled {
                continue;
            }
            let (mut records, version) =
                store::get::<AccountRecords>(host, "observations", &account).await?;
            for batch in batches {
                records.record(&batch.owner.model, batch.samples, &settings);
            }
            store::put(host, ("observations", &account, version), &records).await?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub unattributed: u64,
    pub dropped: u64,
    pub storage_failures: u64,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
