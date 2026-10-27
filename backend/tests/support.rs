use std::time::Duration;

use codex_proxy_state_plugin::{PLUGIN_ID, manifest, plugin};
use gateway_plugin_sdk::{
    CallContext, Frame, Handshake, Message, PROTOCOL_VERSION, PluginFault, Stage,
    client::{PluginSession, SessionConfig, read_frame, write_frame},
};
use serde_json::{Value, json};
use tokio::{
    io::{DuplexStream, ReadHalf, WriteHalf},
    task::JoinHandle,
};

pub type Reply = Result<(Value, Vec<u8>), PluginFault>;

pub struct Peer {
    reader: ReadHalf<DuplexStream>,
    writer: WriteHalf<DuplexStream>,
    task: JoinHandle<()>,
    id: u64,
}

impl Peer {
    pub async fn start() -> Self {
        let (host, transport) = tokio::io::duplex(2 * 1024 * 1024);
        let (reader, writer) = tokio::io::split(transport);
        let task = tokio::spawn(async move {
            let session = PluginSession::accept(reader, writer, SessionConfig::default())
                .await
                .unwrap();
            session.run(plugin().unwrap()).await.unwrap();
        });
        let (reader, writer) = tokio::io::split(host);
        let mut peer = Self {
            reader,
            writer,
            task,
            id: 0,
        };
        let manifest = manifest().unwrap();
        peer.send(Frame::control(Message::Hello {
            handshake: Handshake {
                protocol_version: PROTOCOL_VERSION,
                artifact_sha256: "a".repeat(64),
                plugin_id: PLUGIN_ID.into(),
                instance_id: "test".into(),
                generation: 1,
                incarnation: "inc".into(),
                configuration: json!({}),
                contributes: manifest.contributes,
            },
        }))
        .await;
        assert!(matches!(peer.recv().await.message, Message::Ready { .. }));
        peer
    }

    pub async fn send(&mut self, frame: Frame) {
        write_frame(&mut self.writer, &frame).await.unwrap();
    }

    pub async fn recv(&mut self) -> Frame {
        tokio::time::timeout(Duration::from_secs(10), read_frame(&mut self.reader))
            .await
            .unwrap()
            .unwrap()
    }

    /// 等到插件主动发起的宿主回调；期间应答挂起与恢复，忽略已消费的回收通知
    #[allow(dead_code)] // 仅 protocol 测试需要，其他测试目标共享本文件
    pub async fn recv_callback(&mut self) -> (u64, String, Value, Vec<u8>) {
        loop {
            let frame = self.recv().await;
            match frame.message {
                Message::Callback {
                    id, method, params, ..
                } => return (id, method, params, frame.payload),
                Message::Resume { id, epoch } => {
                    self.send(Frame::control(Message::Resumed { id, epoch }))
                        .await;
                }
                Message::Suspend { .. } | Message::CallbackConsumed { .. } => {}
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    pub async fn begin(
        &mut self,
        method: &str,
        stage: Stage,
        params: Value,
        payload: Vec<u8>,
    ) -> u64 {
        self.id += 2;
        let id = self.id - 1;
        // 协议 v7 起宿主声明的 timeout_ms 不能超过该阶段的默认处理预算
        let timeout_ms = match stage {
            Stage::Registration => 5_000,
            Stage::Observation | Stage::Routing | Stage::Scheduling | Stage::Retry => 1_500,
            _ => 10_000,
        };
        self.send(Frame {
            message: Message::Call {
                id,
                method: method.into(),
                context: CallContext {
                    request: None,
                    call_id: id,
                    instance_id: "test".into(),
                    generation: 1,
                    incarnation: "inc".into(),
                    stage,
                    timeout_ms,
                    resource_stream: false,
                    resource_scope_id: "scope".into(),
                    request_id: Some("request".into()),
                    attempt_id: None,
                    account_id: params
                        .get("account_id")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    credential_revision: None,
                },
                params,
            },
            payload,
        })
        .await;
        if method == "middleware.handle" {
            self.send(Frame::control(Message::Credit {
                id,
                bytes: 1024 * 1024,
                frames: 128,
            }))
            .await;
        }
        id
    }

    pub async fn callback(&mut self, id: u64, reply: Reply) {
        let frame = match reply {
            Ok((result, payload)) => Frame {
                message: Message::Result { id, result },
                payload,
            },
            Err(error) => Frame::control(Message::Error { id, error }),
        };
        self.send(frame).await;
    }

    pub async fn finish(
        &mut self,
        id: u64,
        mut callback: impl FnMut(&str, &Value, &[u8]) -> Reply,
    ) -> Frame {
        loop {
            let frame = self.recv().await;
            match &frame.message {
                Message::Callback {
                    id: callback_id,
                    parent_id,
                    method,
                    params,
                } => {
                    assert_eq!(*parent_id, id);
                    let reply = callback(method, params, &frame.payload);
                    self.callback(*callback_id, reply).await;
                }
                // 父任务暂时让出执行名额，宿主无需应答
                Message::Suspend { .. } | Message::CallbackConsumed { .. } => {}
                // 插件请求恢复，宿主授权后才能继续同一调用；
                // 授权必须回传插件给出的调用编号与 epoch，不能借当前等待的调用编号
                Message::Resume {
                    id: resume_id,
                    epoch,
                } => {
                    let (resume_id, epoch) = (*resume_id, *epoch);
                    self.send(Frame::control(Message::Resumed {
                        id: resume_id,
                        epoch,
                    }))
                    .await;
                }
                Message::Result {
                    id: response_id, ..
                } => {
                    assert_eq!(*response_id, id);
                    return frame;
                }
                Message::End { error, .. } => assert!(error.is_none()),
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    pub async fn call(
        &mut self,
        method: &str,
        stage: Stage,
        params: Value,
        payload: Vec<u8>,
        callback: impl FnMut(&str, &Value, &[u8]) -> Reply,
    ) -> Frame {
        let id = self.begin(method, stage, params, payload).await;
        self.finish(id, callback).await
    }

    pub async fn api(
        &mut self,
        method: &str,
        path: &str,
        data: Option<Value>,
        callback: impl FnMut(&str, &Value, &[u8]) -> Reply,
    ) -> (u64, Value) {
        let payload = data
            .as_ref()
            .map_or_else(Vec::new, |value| serde_json::to_vec(value).unwrap());
        let frame = self
            .call(
                "management.handle",
                Stage::Management,
                json!({
                    "method":method, "path":path, "query":"",
                    "content_type":data.as_ref().map(|_|"application/json"),
                }),
                payload,
                callback,
            )
            .await;
        let Message::Result { result, .. } = frame.message else {
            panic!("missing result")
        };
        (
            result["status"].as_u64().unwrap(),
            serde_json::from_slice(&frame.payload).unwrap(),
        )
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
