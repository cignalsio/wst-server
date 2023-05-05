# WebSocket Time Protocol (WST) Server

A (very) simple server implementation of the WebSocket Time Protocol (WST) in Rust, based on the work by M. Gutbrod, et al. in [A light-weight time protocol based on common web standards](https://uhr.ptb.de/wst/paper).

Used on and sponsored by [Cignals footprint charts](https://cignals.io/) cryptocurrency charting platform.

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
