# RESP Wire Format — Universal Rules

These rules apply to every RESP message, regardless of version or data type.

## 1. First byte determines the type

The very first byte of a message identifies the data type that follows.
A parser reads exactly one byte, then dispatches to the handler for that type.

| First byte | Type          |
| ---------- | ------------- |
| `+`        | Simple String |
| `-`        | Simple Error  |
| `:`        | Integer       |
| `$`        | Bulk String   |
| `*`        | Array         |

## 2. CRLF terminates every part

`\r\n` (CRLF) is the protocol's only terminator. It separates the parts of a
message and marks the end of every line, header, and payload. Parsers must
never treat a bare `\r` or bare `\n` as a terminator.

## 3. Request/response, one message at a time

RESP is a request/response protocol: the client sends one command (encoded as
an array of bulk strings), the server replies with a single typed reply.
There is no explicit message length for the connection as a whole — the
structure of each type tells the parser when the message is complete.

## 4. Binary safety

RESP is binary-safe. Bulk string payloads may contain any byte sequence,
including `\r` and `\n`, because their length is declared up front
(`$<length>\r\n<payload>\r\n`). The payload is read by byte count, never by
scanning for a terminator. Only the declared-length prefix and the trailing
CRLF are structural.

## 5. Lengths and integers are decimal

All numeric prefixes (lengths, element counts, integer values) are encoded as
base-10 ASCII digits with an optional leading `-` for negative values,
terminated by CRLF. No leading zeros, no hex, no binary-encoded numbers.

## 6. Null is distinct from empty

A zero-length value is not the same as a null value:

- Empty bulk string: `$0\r\n\r\n`
- Null bulk string: `$-1\r\n`
- Null array: `*-1\r\n`

## 7. Simple strings and errors carry no CRLF inside

Simple strings and simple errors are single-line values: their content runs
from the first byte after the type prefix to the next CRLF, and must not
contain `\r` or `\n` itself. Anything binary or multi-line must use a bulk
string.

## 8. Replies must be parseable without the request

A client that lost track of what it sent must still be able to parse a reply.
Each message is self-describing: type from the first byte, extent from the
type's own framing rules.
