//! Transparent reverse proxy for Ollama. Listens on a chosen port, forwards
//! every request to the upstream `--ollama-url`, and watches `/api/generate`
//! and `/api/chat` responses for `eval_count` / `eval_duration` so we can
//! report live tokens/sec per model.
//!
//! Streaming (NDJSON) and non-streaming responses are both handled. Bytes
//! are forwarded to the client unchanged so existing Ollama clients keep
//! working — we just tee the stream into a parser.

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    response::Response,
    routing::any,
};
use color_eyre::eyre::Result;
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default, Clone, Debug)]
pub struct ModelTok {
    pub last_tok_s: f64,
    pub total_tokens: u64,
    pub last_seen: Option<Instant>,
}

pub type Sink = Arc<Mutex<HashMap<String, ModelTok>>>;

pub fn new_sink() -> Sink {
    Arc::new(Mutex::new(HashMap::new()))
}

#[derive(Clone)]
struct AppState {
    upstream: String,
    http: reqwest::Client,
    sink: Sink,
}

pub async fn run(listen_port: u16, upstream: String, sink: Sink) -> Result<()> {
    let state = AppState {
        upstream,
        http: reqwest::Client::builder().build()?,
        sink,
    };
    let app = Router::new().fallback(any(handler)).with_state(state);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", listen_port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handler(State(state): State<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(|q| q.to_string());
    let upstream = state.upstream.trim_end_matches('/');
    let url = match query {
        Some(q) => format!("{}{}?{}", upstream, path, q),
        None => format!("{}{}", upstream, path),
    };

    let method = req.method().clone();
    let headers = req.headers().clone();
    let is_completion = path == "/api/generate" || path == "/api/chat";

    let body_bytes = match axum::body::to_bytes(req.into_body(), 64 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return error_response(413),
    };

    let mut rb = state.http.request(method, &url).body(body_bytes.to_vec());
    for (k, v) in headers.iter() {
        // host/content-length get set by reqwest from the URL/body.
        if k != "host" && k != "content-length" {
            rb = rb.header(k, v);
        }
    }
    let resp = match rb.send().await {
        Ok(r) => r,
        Err(_) => return error_response(502),
    };

    let status = resp.status();
    let resp_headers = resp.headers().clone();

    let mut builder = Response::builder().status(status.as_u16());
    for (k, v) in resp_headers.iter() {
        // We re-stream, so let hyper recompute framing.
        if k != "content-length" && k != "transfer-encoding" {
            builder = builder.header(k, v);
        }
    }

    if !is_completion {
        // Non-completion endpoints: pass through verbatim, no parsing.
        let stream = resp.bytes_stream().map(|c| c.map_err(std::io::Error::other));
        return builder.body(Body::from_stream(stream)).unwrap_or_else(|_| error_response(500));
    }

    // Completion: tee bytes into a NDJSON parser while forwarding to the client.
    let sink = state.sink.clone();
    let parser = Arc::new(Mutex::new(ParserState::default()));
    let parser_done = parser.clone();
    let sink_done = sink.clone();

    let stream = resp.bytes_stream().map(move |chunk| {
        if let Ok(bytes) = &chunk
            && let Ok(mut p) = parser.lock()
        {
            p.feed(bytes, &sink);
        }
        chunk.map_err(std::io::Error::other)
    });
    // Wrap so we can flush the parser buffer once the upstream stream ends —
    // catches non-streaming responses (single JSON object, no trailing newline).
    let stream = stream.chain(futures_util::stream::once(async move {
        if let Ok(mut p) = parser_done.lock() {
            p.flush(&sink_done);
        }
        Ok(bytes::Bytes::new())
    }));
    builder
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| error_response(500))
}

#[derive(Default)]
struct ParserState {
    buf: Vec<u8>,
}

impl ParserState {
    fn feed(&mut self, chunk: &[u8], sink: &Sink) {
        self.buf.extend_from_slice(chunk);
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            try_record(&line, sink);
        }
    }

    fn flush(&mut self, sink: &Sink) {
        if !self.buf.is_empty() {
            try_record(&self.buf, sink);
            self.buf.clear();
        }
    }
}

fn try_record(line: &[u8], sink: &Sink) {
    let trimmed = line
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|s| &line[s..])
        .unwrap_or(line);
    if trimmed.is_empty() {
        return;
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(trimmed) else {
        return;
    };
    if !v.get("done").and_then(|d| d.as_bool()).unwrap_or(false) {
        return;
    }
    let model = v
        .get("model")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let eval_count = v.get("eval_count").and_then(|x| x.as_u64()).unwrap_or(0);
    let eval_dur = v.get("eval_duration").and_then(|x| x.as_u64()).unwrap_or(0);
    if model.is_empty() || eval_count == 0 || eval_dur == 0 {
        return;
    }
    let tok_s = eval_count as f64 / (eval_dur as f64 / 1e9);
    if let Ok(mut map) = sink.lock() {
        let entry = map.entry(model).or_default();
        entry.last_tok_s = tok_s;
        entry.total_tokens += eval_count;
        entry.last_seen = Some(Instant::now());
    }
}

fn error_response(status: u16) -> Response {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}
