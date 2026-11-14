# WebSocket Time Protocol (WST) Server

A (very) simple server implementation of the WebSocket Time Protocol (WST) in Rust, based on the work by M. Gutbrod, et al. in [A light-weight time protocol based on common web standards](https://uhr.ptb.de/wst/paper).

## Development

Run `nix develop` from the repository root on x86-64 or ARM64 Linux, or on Apple
Silicon macOS. The shell provides Cargo, rustc, rustfmt, Clippy, and `websocat`.
Run `cargo test --locked` inside the shell to run the tests.

## Install

```bash
cargo install
```

## Example

Server:

```bash
$ ./wst 127.0.0.1:8080
```

Client:

```bash
$ echo '{"c":1683229952425}' | websocat 'ws://127.0.0.1:8080/wst'
{"c":1683229952425,"s":1683232267249,"e":null,"l":3}
```

## Sponsor

Thanks to our sponsor [Cignals footprint charts](https://cignals.io/) cryptocurrency charting platform.

