use super::Shared;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{net::TcpStream, sync::mpsc, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};

pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub struct Frame {
    pub pcm: Vec<u8>,
    pub start: f64,
}
pub async fn open(key: &str, remote: bool, participants: &[String]) -> Result<Socket, String> {
    open_at(
        "wss://streaming.assemblyai.com/v3/ws",
        key,
        remote,
        participants,
    )
    .await
}
async fn open_at(
    endpoint: &str,
    key: &str,
    remote: bool,
    participants: &[String],
) -> Result<Socket, String> {
    let mut url =
        url::Url::parse(endpoint).map_err(|_| "Invalid transcription endpoint.".to_string())?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("sample_rate", "48000")
            .append_pair("encoding", "pcm_s16le")
            .append_pair("speech_model", "universal-3-6-pro")
            .append_pair("mode", "balanced")
            .append_pair("continuous_partials", "true");
        if remote {
            q.append_pair("speaker_labels", "true")
                .append_pair("speaker_labels_revision_interval_ms", "120000");
        }
        if !participants.is_empty() {
            q.append_pair(
                "keyterms_prompt",
                &serde_json::to_string(participants).unwrap(),
            );
        }
    }
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| "Could not prepare the transcription connection.".to_string())?;
    request.headers_mut().insert(
        "Authorization",
        key.parse()
            .map_err(|_| "The AssemblyAI key contains invalid characters.".to_string())?,
    );
    let (mut socket, _) = timeout(Duration::from_secs(15), connect_async(request))
        .await
        .map_err(|_| "AssemblyAI connection timed out.".to_string())?
        .map_err(|e| match e {
            tokio_tungstenite::tungstenite::Error::Http(r) => format!(
                "AssemblyAI rejected the connection (HTTP {}). Check your API key and credits.",
                r.status().as_u16()
            ),
            _ => "Could not connect to AssemblyAI. Check your internet connection.".into(),
        })?;
    let ready=timeout(Duration::from_secs(15),async {
        while let Some(message)=socket.next().await {
            let message=message.map_err(|_|"AssemblyAI closed the connection before starting.".to_string())?;
            if let Message::Text(text)=message {
                let value:Value=serde_json::from_str(&text).map_err(|_|"AssemblyAI returned an invalid message.".to_string())?;
                if value["type"]=="Begin" {return Ok(());}
                if value["type"]=="Error" || value.get("error").is_some(){return Err("AssemblyAI could not start transcription. Check the model and account credits.".into());}
            }
        }
        Err("AssemblyAI closed the connection before starting.".into())
    }).await.unwrap_or_else(|_|Err("AssemblyAI did not start the session in time.".into()));
    if let Err(e) = ready {
        let _ = socket.close(None).await;
        return Err(e);
    }
    Ok(socket)
}
pub async fn terminate(mut socket: Socket) {
    let _ = timeout(
        Duration::from_secs(3),
        socket.send(Message::Text(
            json!({"type":"Terminate"}).to_string().into(),
        )),
    )
    .await;
    let _ = timeout(Duration::from_secs(3), socket.close(None)).await;
}
async fn consume(
    shared: &Shared,
    channel: &str,
    generation: u32,
    offset: f64,
    text: &str,
) -> Result<bool, String> {
    let event: Value =
        serde_json::from_str(text).map_err(|_| "Invalid transcription response.".to_string())?;
    if event["type"] == "Termination" {
        return Ok(true);
    }
    if event["type"] == "Error" || event.get("error").is_some() {
        return Err("AssemblyAI reported an error. Audio is still being saved locally.".into());
    }
    let persist = event["end_of_turn"] == true || event["type"] == "SpeakerRevision";
    let changed = shared
        .meeting
        .lock()
        .await
        .apply(channel, generation, offset, &event);
    if changed {
        shared.publish(persist).await;
        if persist {
            shared.notes_wake.notify_one();
        }
    }
    Ok(false)
}
pub async fn run(
    shared: Arc<Shared>,
    channel: &'static str,
    key: String,
    mut socket: Socket,
    mut receiver: mpsc::Receiver<Frame>,
) {
    let mut generation = 0;
    let mut offset = 0.0;
    let mut first = true;
    let mut expected = 0.0;
    let mut pending: Option<Frame> = None;
    shared.connection(channel, "Connected").await;
    loop {
        let failed = tokio::select! {
            frame=async { if let Some(frame)=pending.take(){Some(frame)}else{receiver.recv().await} }=>match frame {
                Some(frame)=>{
                    if first {offset=frame.start;expected=frame.start;first=false;}
                    if (frame.start-expected).abs()>0.025 {
                        // A dropped frame must not compress all later transcript timestamps.
                        // Restart at the next retained frame with a fresh speaker namespace.
                        pending=Some(frame);true
                    } else {
                        expected=frame.start + frame.pcm.len() as f64 / 96_000.0;
                        match timeout(Duration::from_secs(5),socket.send(Message::Binary(frame.pcm.into()))).await {
                            Ok(Ok(_))=>false,_=>true
                        }
                    }
                }
                None=>{
                    let drain=finish(&mut socket,|text| { let shared=shared.clone(); async move {consume(&shared,channel,generation,offset,&text).await} }).await;
                    if let Err(e)=drain{shared.warning(e).await;}
                    let _=timeout(Duration::from_secs(2),socket.close(None)).await; shared.connection(channel,"Stopped").await; return;
                }
            },
            message=socket.next()=>match message {
                Some(Ok(Message::Text(text)))=>consume(&shared,channel,generation,offset,&text).await.unwrap_or(true),
                Some(Ok(Message::Ping(data)))=>socket.send(Message::Pong(data)).await.is_err(),
                Some(Ok(Message::Close(_)))|Some(Err(_))|None=>true,
                _=>false,
            }
        };
        if !failed {
            continue;
        }
        let _ = timeout(Duration::from_secs(1), socket.close(None)).await;
        shared.warning(format!("{} transcription connection interrupted. Local audio continues; some live text may be missing.",if channel=="me"{"Microphone"}else{"Mac audio"})).await;
        shared.connection(channel, "Reconnecting").await;
        // Bound retries and queue size. New sessions get separate speaker IDs: a new A is not necessarily the old A.
        let mut replacement = None;
        for delay in [1, 2, 4, 8] {
            if receiver.is_closed() {
                break;
            }
            tokio::select! {_ = shared.stop.cancelled()=>break,_ = tokio::time::sleep(Duration::from_secs(delay))=>{}}
            let participants = shared.meeting.lock().await.participants.clone();
            let result = tokio::select! {_ = shared.stop.cancelled()=>break,r=open(&key,channel=="remote",&participants)=>r};
            if let Ok(ws) = result {
                replacement = Some(ws);
                break;
            }
        }
        match replacement {
            Some(ws) => {
                socket = ws;
                generation += 1;
                first = true;
                shared.connection(channel, "Connected").await;
            }
            None => {
                shared
                    .connection(channel, "Offline — audio saved locally")
                    .await;
                return;
            }
        }
    }
}

async fn finish<F, Fut>(socket: &mut Socket, mut on_text: F) -> Result<(), String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<bool, String>>,
{
    timeout(
        Duration::from_secs(3),
        socket.send(Message::Text(
            json!({"type":"Terminate"}).to_string().into(),
        )),
    )
    .await
    .map_err(|_| "Could not finish the transcription connection in time.".to_string())?
    .map_err(|_| {
        "The transcription connection closed before the final words arrived.".to_string()
    })?;
    timeout(Duration::from_secs(15),async {
        while let Some(message)=socket.next().await {
            match message {
                Ok(Message::Text(text))=>if on_text(text.to_string()).await?{return Ok(());},
                Ok(Message::Ping(data))=>{let _=socket.send(Message::Pong(data)).await;},
                Ok(Message::Close(_))|Err(_)=>break,_=>{}
            }
        }
        Err("The transcription connection ended before final speaker corrections arrived. Review the saved transcript.".into())
    }).await.map_err(|_|"Final speaker corrections timed out. Review the saved transcript.".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn websocket_auth_pcm_and_termination_drain() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("ws://{}/v3/ws", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_hdr_async(
                tcp,
                |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.headers()["Authorization"], "test-only-key");
                    let query = request.uri().query().unwrap();
                    assert!(query.contains("speaker_labels=true"));
                    assert!(!query.contains("max_speakers"));
                    assert!(!query.contains("test-only-key"));
                    Ok(response)
                },
            )
            .await
            .unwrap();
            ws.send(Message::Text(json!({"type":"Begin"}).to_string().into()))
                .await
                .unwrap();
            assert!(
                matches!(ws.next().await.unwrap().unwrap(),Message::Binary(data) if data.len()==9600)
            );
            let end = ws.next().await.unwrap().unwrap();
            assert!(matches!(end,Message::Text(t) if t.contains("Terminate")));
            for event in [
                json!({"type":"Turn","end_of_turn":true}),
                json!({"type":"SpeakerRevision","revisions":[]}),
                json!({"type":"Termination"}),
            ] {
                ws.send(Message::Text(event.to_string().into()))
                    .await
                    .unwrap();
            }
        });
        let mut ws = open_at(&endpoint, "test-only-key", true, &["Nicole".into()])
            .await
            .unwrap();
        ws.send(Message::Binary(vec![0; 9600].into()))
            .await
            .unwrap();
        let seen = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        finish(&mut ws, |text| {
            let seen = seen.clone();
            async move {
                let v: Value = serde_json::from_str(&text).unwrap();
                let kind = v["type"].as_str().unwrap().to_string();
                let done = kind == "Termination";
                seen.lock().await.push(kind);
                Ok(done)
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(
            *seen.lock().await,
            vec!["Turn", "SpeakerRevision", "Termination"]
        );
    }
}
