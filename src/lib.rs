use std::{rc::Rc, str::FromStr};

use iroh::{
    Endpoint, EndpointAddr, RelayMap, RelayUrl, SecretKey,
    endpoint::{self, RelayMode, presets},
};
use serde::Deserialize;
use tokio::sync::Mutex;
use wasm_bindgen::prelude::*;

fn js_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

fn alpn_bytes(alpn: &str) -> Result<Vec<u8>, JsError> {
    if alpn.is_empty() || alpn.len() > 255 {
        return Err(JsError::new("ALPN must contain 1–255 UTF-8 bytes"));
    }
    Ok(alpn.as_bytes().to_vec())
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    alpns: Vec<String>,
    secret_key: Option<Vec<u8>>,
    relay_urls: Option<Vec<String>>,
}

/// An Iroh endpoint running in a browser or dedicated Web Worker.
#[wasm_bindgen]
pub struct BrowserNode {
    endpoint: Endpoint,
}

#[wasm_bindgen]
impl BrowserNode {
    /// Bind an endpoint. Omitted options use Iroh's public relays and discovery.
    pub async fn bind(options: JsValue) -> Result<BrowserNode, JsError> {
        console_error_panic_hook::set_once();
        let options: Options = if options.is_null() || options.is_undefined() {
            Options::default()
        } else {
            serde_wasm_bindgen::from_value(options).map_err(js_error)?
        };
        let alpns = options
            .alpns
            .iter()
            .map(|alpn| alpn_bytes(alpn))
            .collect::<Result<Vec<_>, _>>()?;
        let mut builder = Endpoint::builder(presets::N0).alpns(alpns);

        if let Some(key) = options.secret_key {
            let key: [u8; 32] = key
                .try_into()
                .map_err(|_| JsError::new("secretKey must contain exactly 32 bytes"))?;
            builder = builder.secret_key(SecretKey::from_bytes(&key));
        }
        if let Some(urls) = options.relay_urls {
            if urls.is_empty() {
                return Err(JsError::new("relayUrls must contain at least one relay"));
            }
            let urls = urls
                .iter()
                .map(|url| RelayUrl::from_str(url).map_err(js_error))
                .collect::<Result<Vec<_>, _>>()?;
            builder = builder.relay_mode(RelayMode::Custom(RelayMap::from_iter(urls)));
        }

        Ok(Self {
            endpoint: builder.bind().await.map_err(js_error)?,
        })
    }

    #[wasm_bindgen(getter, js_name = nodeId)]
    pub fn node_id(&self) -> String {
        self.endpoint.id().to_string()
    }

    /// Export the identity seed for persistence. Treat it as a private credential.
    #[wasm_bindgen(js_name = exportSecretKey)]
    pub fn export_secret_key(&self) -> Vec<u8> {
        self.endpoint.secret_key().to_bytes().to_vec()
    }

    /// Wait until a relay is ready before sharing address().
    pub async fn online(&self) {
        self.endpoint.online().await;
    }

    /// Serialize the current endpoint address, including its relay, as JSON.
    pub fn address(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.endpoint.addr()).map_err(js_error)
    }

    /// Connect using address() JSON or an endpoint ID resolved by Iroh discovery.
    pub async fn connect(&self, peer: String, alpn: String) -> Result<Connection, JsError> {
        let address: EndpointAddr = if peer.trim_start().starts_with('{') {
            serde_json::from_str(&peer).map_err(js_error)?
        } else {
            EndpointAddr::new(peer.parse().map_err(js_error)?)
        };
        let alpn = alpn_bytes(&alpn)?;
        Ok(Connection {
            inner: self
                .endpoint
                .connect(address, &alpn)
                .await
                .map_err(js_error)?,
        })
    }

    /// Accept the next authenticated connection, or return undefined after close().
    pub async fn accept(&self) -> Result<Option<Connection>, JsError> {
        match self.endpoint.accept().await {
            Some(incoming) => Ok(Some(Connection {
                inner: incoming.await.map_err(js_error)?,
            })),
            None => Ok(None),
        }
    }

    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

#[wasm_bindgen]
pub struct Connection {
    inner: endpoint::Connection,
}

#[wasm_bindgen]
impl Connection {
    #[wasm_bindgen(getter, js_name = remoteId)]
    pub fn remote_id(&self) -> String {
        self.inner.remote_id().to_string()
    }

    #[wasm_bindgen(getter)]
    pub fn alpn(&self) -> String {
        String::from_utf8_lossy(self.inner.alpn()).into_owned()
    }

    #[wasm_bindgen(js_name = openBi)]
    pub async fn open_bi(&self) -> Result<BiStream, JsError> {
        let (send, recv) = self.inner.open_bi().await.map_err(js_error)?;
        Ok(BiStream::new(send, recv))
    }

    #[wasm_bindgen(js_name = acceptBi)]
    pub async fn accept_bi(&self) -> Result<BiStream, JsError> {
        let (send, recv) = self.inner.accept_bi().await.map_err(js_error)?;
        Ok(BiStream::new(send, recv))
    }

    #[wasm_bindgen(js_name = openUni)]
    pub async fn open_uni(&self) -> Result<SendStream, JsError> {
        Ok(SendStream::new(
            self.inner.open_uni().await.map_err(js_error)?,
        ))
    }

    #[wasm_bindgen(js_name = acceptUni)]
    pub async fn accept_uni(&self) -> Result<RecvStream, JsError> {
        Ok(RecvStream::new(
            self.inner.accept_uni().await.map_err(js_error)?,
        ))
    }

    /// Close immediately; pending stream operations reject.
    pub fn close(&self) {
        self.inner.close(0u8.into(), b"closed");
    }

    /// Resolve to the reason when either peer closes the connection.
    pub async fn closed(&self) -> String {
        self.inner.closed().await.to_string()
    }
}

#[wasm_bindgen]
pub struct BiStream {
    send: SendStream,
    recv: RecvStream,
}

impl BiStream {
    fn new(send: endpoint::SendStream, recv: endpoint::RecvStream) -> Self {
        Self {
            send: SendStream::new(send),
            recv: RecvStream::new(recv),
        }
    }
}

#[wasm_bindgen]
impl BiStream {
    #[wasm_bindgen(getter)]
    pub fn send(&self) -> SendStream {
        self.send.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn recv(&self) -> RecvStream {
        self.recv.clone()
    }
}

// Cloned JS handles share each stream. Async locks serialize operations without
// holding mutable wasm-bindgen borrows across an await.
#[derive(Clone)]
#[wasm_bindgen]
pub struct SendStream {
    inner: Rc<Mutex<endpoint::SendStream>>,
}

impl SendStream {
    fn new(stream: endpoint::SendStream) -> Self {
        Self {
            inner: Rc::new(Mutex::new(stream)),
        }
    }
}

#[wasm_bindgen]
impl SendStream {
    /// Write all bytes with QUIC backpressure. This does not finish the stream.
    pub async fn write(&self, bytes: Vec<u8>) -> Result<(), JsError> {
        self.inner
            .lock()
            .await
            .write_all(&bytes)
            .await
            .map_err(js_error)
    }

    /// Signal EOF; call stopped() to wait for the peer's acknowledgement.
    pub async fn finish(&self) -> Result<(), JsError> {
        self.inner.lock().await.finish().map_err(js_error)
    }

    pub async fn stopped(&self) -> Result<(), JsError> {
        let stopped = self.inner.lock().await.stopped();
        match stopped.await.map_err(js_error)? {
            None => Ok(()),
            Some(code) => Err(JsError::new(&format!("Peer stopped stream: {code}"))),
        }
    }
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct RecvStream {
    inner: Rc<Mutex<endpoint::RecvStream>>,
}

impl RecvStream {
    fn new(stream: endpoint::RecvStream) -> Self {
        Self {
            inner: Rc::new(Mutex::new(stream)),
        }
    }
}

#[wasm_bindgen]
impl RecvStream {
    /// Read up to maxBytes (1–1,048,576), returning undefined at EOF.
    pub async fn read(&self, max_bytes: u32) -> Result<Option<Vec<u8>>, JsError> {
        if !(1..=1_048_576).contains(&max_bytes) {
            return Err(JsError::new("maxBytes must be between 1 and 1048576"));
        }
        let mut bytes = vec![0; max_bytes as usize];
        match self
            .inner
            .lock()
            .await
            .read(&mut bytes)
            .await
            .map_err(js_error)?
        {
            Some(count) => {
                bytes.truncate(count);
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }
}
