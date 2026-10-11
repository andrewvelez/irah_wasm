# irah-wasm

Minimal browser WebAssembly bindings for [Iroh](https://www.iroh.computer/).
The npm package is `@andrewvelez/irah-wasm`; the repository spelling is intentional.
JavaScript runs in the browser or a dedicated module Web Worker. Node.js and Bun
are packaging tools, not the runtime for these bindings.

## Install and initialize

Download the `irah-wasm-npm` artifact from a successful GitHub Actions build,
extract its `.tgz`, and install it with `npm install ./andrewvelez-irah-wasm-0.1.0.tgz`.
The archive includes compiled Wasm and JavaScript; consumers do not need Rust.
This package has not been published to npm.

Import the default `init` function and named classes from `@andrewvelez/irah-wasm`.
Await `init()` once in each JavaScript context before calling `BrowserNode.bind()`.
For unbundled ES modules, initialization loads `irah_wasm_bg.wasm` beside the JS
module. When bundling, copy the Wasm asset into your app's output and pass its URL
as `init({ module_or_path: wasmUrl })`. The asset is also exported as
`@andrewvelez/irah-wasm/wasm`; handling this asset depends on your bundler.
Serve assets over HTTP(S), preferably with `application/wasm` for the Wasm file.

## API

All asynchronous methods return Promises. Errors reject with JavaScript `Error`
objects. Bytes are `Uint8Array` values. Each exported handle has a generated
`free()` method; call it after its pending operations finish and the handle is
no longer needed. `free()` releases a handle; explicitly close connections and
nodes to shut down their network activity.

### BrowserNode

| Member | Behavior |
| --- | --- |
| `BrowserNode.bind(options?)` | Create an endpoint with a random identity by default. |
| `nodeId` | Read-only Ed25519 endpoint ID. |
| `exportSecretKey()` | Return the 32-byte identity seed for secure persistence. |
| `online()` | Wait for a relay connection; it can remain pending while offline. |
| `address()` | Return current Iroh endpoint-address JSON; await `online()` first. |
| `connect(peer, alpn)` | Connect using `address()` JSON or an endpoint ID. |
| `accept()` | Accept an authenticated connection; returns `undefined` after shutdown. |
| `close()` | Shut down the endpoint and its connections. |

`options` accepts `alpns` (an array of protocol strings), `secretKey` (a saved
32-byte seed), and `relayUrls` (a nonempty array of Iroh relay URLs). Omitted
options use Iroh's N0 preset: public relays and public endpoint-address discovery.
Custom `relayUrls` replace the relay list; they do not disable public discovery.
An empty or omitted `alpns` list creates a client-only node. To receive
connections, declare the accepted protocols. Both peers must use the same ALPN,
which must contain 1–255 UTF-8 bytes.

### Connection and streams

| Member | Behavior |
| --- | --- |
| `connection.remoteId` | Authenticated peer ID. |
| `connection.alpn` | Negotiated application protocol. |
| `connection.openBi()` / `acceptBi()` | Open or accept a bidirectional stream. |
| `connection.openUni()` / `acceptUni()` | Open a send-only or accept a receive-only stream. |
| `connection.close()` | Close immediately; pending stream operations reject. |
| `connection.closed()` | Wait for closure and return its reason as a string. |
| `biStream.send` / `biStream.recv` | Get handles to the stream's two halves. |
| `send.write(bytes)` | Write all bytes with QUIC backpressure. |
| `send.finish()` | Signal end of outgoing data. |
| `send.stopped()` | Wait for acknowledgement, rejecting if the peer stops the stream. |
| `recv.read(maxBytes)` | Read up to 1–1,048,576 bytes; return `undefined` at EOF. |

Write some data or finish an opened stream before the other peer can accept it.
Reads return chunks, not application messages: your application supplies framing.
Operations on each stream half are serialized; sending and receiving can proceed
concurrently. A write completing means data was queued, not that your application
on the other peer processed it. Use application acknowledgements when needed.

## Browser and PWA constraints

Browser endpoints use encrypted connections through Iroh relays. They cannot
perform native UDP hole punching. This package supplies transport and identity,
not gossip, blob storage, user authorization, or a messaging delivery protocol.
Check `remoteId` against your application's authorized peers before handling data.

Use a dedicated Web Worker to move networking off the UI thread. A service worker
does not provide a persistent background process; browsers may suspend tabs,
terminate workers, and stop networking when the PWA is closed. Cache both the JS
and Wasm assets for offline initialization. Communication still needs connectivity.
Do not place exported identity seeds in public assets, logs, or peer addresses.

## Build and publish

Install Rust using rustup, a Wasm-capable Clang compiler, Node.js/npm, and wasm-pack
0.13.1. The toolchain file pins Rust and installs the Wasm target; `Cargo.lock`
pins dependencies. Build with `npm run build`. The generated `pkg/` directory
contains ES modules, Wasm, and editor declarations generated by wasm-bindgen.
The bindings have no runtime npm dependencies.

`npm pack` builds and creates the installable archive. After a build,
`npm pack --ignore-scripts` packages the existing output. `npm publish` builds and
publishes under the `andrewvelez` npm scope, requiring your npm credentials and
permission to publish there. Publish from the repository root, not from `pkg/`.

The GitHub Actions workflow checks Rust formatting, builds the package, and
uploads the npm archive. The source and generated package use the repository's
MIT license. Iroh and its dependencies retain their own licenses.
