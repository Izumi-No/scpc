# RFC-DRAFT: Secure Command Session Protocol (SCSP)

**Status:** Draft v0.1
**Categoria:** Standards Track
**Autor:** Izumi / Community Draft
**Data:** May 2026

---

# Abstract

The Secure Command Session Protocol (SCSP) defines a lightweight, secure, persistent client-server protocol optimized for delivering commands, configuration, and operational data from brokers to large fleets of connected clients.

SCSP is transport-agnostic and can operate over:

* TCP + TLS
* QUIC

SCSP is designed for:

* millions of concurrent clients
* low idle resource cost
* resumable sessions
* secure command delivery
* bidirectional structured messaging
* future extensibility for queueing and pub/sub systems

---

# 1. Terminology

| Term         | Meaning                            |
| ------------ | ---------------------------------- |
| Broker       | Server node handling sessions      |
| Client       | Connected agent/device/application |
| Session      | Authenticated logical connection   |
| Frame        | Binary protocol unit               |
| Stream       | Logical channel                    |
| Resume Token | Token allowing reconnection        |
| Capability   | Granted permission set             |

---

# 2. Goals

SCSP aims to provide:

1. Efficient long-lived connections
2. Secure authenticated sessions
3. Server-to-client command delivery
4. Reliable acknowledgements
5. Low memory overhead
6. Multi-transport compatibility
7. Extensible binary framing

---

# 3. Non Goals

SCSP v0.1 does not define:

* distributed queues
* pub/sub topics
* durable retention
* consensus clustering
* file transfer semantics
* RPC schemas

These may be added via extensions.

---

# 4. Transport Bindings

SCSP MAY operate over:

## 4.1 TCP Binding

Reliable byte stream over TLS 1.2+ (TLS 1.3 RECOMMENDED)

## 4.2 QUIC Binding

Reliable streams over QUIC with TLS 1.3.

## 4.3 Transport Selection

Clients MAY prefer QUIC and fallback to TCP/TLS.

---

# 5. Session Lifecycle

```text id="rfc1"
DISCONNECTED
  -> CONNECTING
  -> AUTHENTICATING
  -> ACTIVE
  -> RESUMING
  -> CLOSED
```

---

# 6. Frame Format

All SCSP messages are frames.

## 6.1 Fixed Header (16 bytes)

```text id="rfc2"
0       Version      u8
1       Type         u8
2..3    Flags        u16
4..7    Stream ID    u32
8..11   Message ID   u32
12..15  Payload Len  u32
```

All integers are network byte order (big-endian).

---

# 7. Frame Types

| Type | Name    |
| ---- | ------- |
| 0x01 | HELLO   |
| 0x02 | AUTH    |
| 0x03 | AUTH_OK |
| 0x04 | PING    |
| 0x05 | PONG    |
| 0x06 | SEND    |
| 0x07 | ACK     |
| 0x08 | ERROR   |
| 0x09 | RESUME  |
| 0x0A | CLOSE   |
| 0x0B | META    |

---

# 8. Payload Encoding

Payloads MAY be encoded as:

* raw bytes
* Protocol Buffers
* CBOR
* MessagePack

Protocol Buffers are RECOMMENDED.

---

# 9. Connection Establishment

## 9.1 Client sends HELLO

Contains:

* protocol version
* client software version
* transport capabilities
* optional metadata

## 9.2 Server responds

* HELLO accepted
* supported version
* auth methods

---

# 10. Authentication

Client sends AUTH containing one of:

* bearer token
* certificate proof
* signed nonce
* API credential

Server replies:

* AUTH_OK
* ERROR(auth_failed)

AUTH_OK includes:

* session_id
* heartbeat interval
* resume token
* granted capabilities

---

# 11. Heartbeats

PING/PONG frames maintain liveness.

Heartbeat interval negotiated at authentication.

Recommended:

* active clients: 30s
* idle clients: 120s
* constrained devices: configurable

---

# 12. Messaging Semantics

SEND frames carry arbitrary application messages.

Fields:

* target stream
* priority flags
* payload

---

# 13. Reliability Levels

## QoS0

No acknowledgement required.

## QoS1

Receiver MUST ACK.

## QoS2

Application-level idempotent delivery using Message ID.

---

# 14. Acknowledgements

ACK references Message ID.

```text id="rfc3"
ACK(msg_id=55)
```

Optional status:

* accepted
* processing
* completed
* rejected

---

# 15. Resume

Client reconnects using RESUME:

* previous session_id
* resume token

Server MAY restore prior session state.

---

# 16. Capabilities

Capabilities define permissions.

Examples:

```text id="rfc4"
receive_commands
send_telemetry
stream_logs
update_config
bulk_transfer
```

---

# 17. Error Codes

| Code | Meaning             |
| ---- | ------------------- |
| 1001 | Unsupported Version |
| 1002 | Auth Failed         |
| 1003 | Invalid Frame       |
| 1004 | Unauthorized        |
| 1005 | Session Expired     |
| 1006 | Rate Limited        |

---

# 18. Security Considerations

Implementations MUST:

* require TLS or QUIC crypto
* validate resume tokens
* rate limit auth attempts
* isolate tenants
* audit privileged commands
* enforce capability checks

---

# 19. Resource Efficiency Recommendations

Implementations SHOULD:

* use event-driven I/O
* allocate buffers lazily
* shard sessions per core
* batch writes
* use timer wheels

---

# 20. Future Extensions

Reserved extension namespaces:

```text id="rfc5"
ext.queue
ext.pubsub
ext.rpc
ext.file
ext.mesh
```

---

# 21. Example Session

```text id="rfc6"
C -> HELLO
S -> HELLO

C -> AUTH(token)
S -> AUTH_OK(session=abc)

S -> SEND(command=restart,id=10)
C -> ACK(id=10)
C -> SEND(result=success,id=11)

C disconnects

C -> RESUME(session=abc)
S -> AUTH_OK(resumed=true)
```

---

# 22. IANA Considerations

Future drafts may request:

* port assignments
* frame type registries
* extension registries

---

# 23. Summary

SCSP provides a modern, lightweight, multi-transport control protocol for large-scale fleets of connected clients.
