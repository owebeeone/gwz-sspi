# Private SSPI message contract v1

2026-10-03. DRAFT for schema/design review. This standalone document refines
GWZ SSPI design revision 2 §4, without implementing authentication or changing
its process/cleanup contract. It includes a bounded caller-input amendment:
AuthRequest.token_limit: TokenLimit, required with no default (see §2). `protocol/sspi.taut.py` is the sole field/tag/type authority;
this document defines its additional closed-profile checks and lifecycle.
The exported IR and contract fingerprint are generated, never hand-edited.
Acceptance covers this schema and semantics only. The secret codec/API gate,
supervision, Windows worker, host integration and release remain subsequent gates.

## 1. Boundary and framing

This is one contained worker conversation over two private, ordered anonymous
pipes. It is not carried in gwz-core/CLI/Python application messages or through
gwz-transport. Each direction has one writer; the parent serializes commands.
No multiplexing, reconnection, retry, stream ID, message resequencing or RPC service
is needed. Neither endpoint accepts another conversation on these pipes.

A frame is a four-byte unsigned little-endian body length followed by one taut
CBOR Envelope. Body length must be 1–100,000 inclusive, checked before allocation.
The schema declares max_encoded_len=100000 and max_depth=8. A future codec must
also admit the exact profile below before constructing owned field values.
Partial reads/writes preserve the single frame; EOF partway through header/body
is terminal Protocol. EOF without Finished is never normal-cleanup evidence.
There is no resynchronization after malformed input. No bytes follow the decoded
Envelope inside a frame. Each framing scratch allocation is zeroizing, including
Hello and failures; a four-byte length header contains no secret.

Use taut's deterministic CBOR subset: definite maps, integer field keys in tag
order, canonical scalar encodings. Every declared field must be present; optional
fields carry CBOR null when inactive. Reject duplicate/unknown fields at every
level, missing fields, unknown enum values, wrong scalar types, negative/out-of-
range unsigned values, excessive depth, trailing bytes and extra body variants.
Envelope.version is exactly 1; exactly one nonnull body matches kind. A version
or schema/build mismatch refuses before Begin. No extension preservation or
compatibility fallback is allowed in this private closed profile.

Taut reference codecs preserve unknown fields and reference Rust bindings use
ordinary Clone/Debug/String/Vec storage. They are NOT this production decoder.
Only synthetic test values may enter them. A future IR-driven Rust projection
must implement zeroizing storage and the stricter closed-profile admission;
no handwritten alternative field/tag schema or edits to generated code.

## 2. Wire values and bounds

All INT fields below are nonnegative, exact integers (never bools). All STR values
are valid UTF-8, with no NUL; conversion to native UTF-16 uses separately owned
zeroizing storage, not a borrowed temporary. No normalization of passwords,
opaque challenges/tokens or binding bytes. Length limits are UTF-8/byte lengths,
not code-point counts; the total encoded-frame bound additionally applies.

| Value | Required profile |
|---|---|
| Envelope.version / Hello.protocol_version | Exactly 1, both equal |
| Hello.schema_fingerprint | Exactly 32 bytes, exact expected contract digest from protocol/contract.json |
| Hello.build_fingerprint | Exactly 32 bytes, host-supplied expected matching installed artifact-set digest; not Cargo version alone |
| PrimaryIdentity.sid | Valid binary Windows SID, revision 1, subauthority count ≤15, exact length 8+4*count, maximum 68 bytes |
| PrimaryIdentity.authentication_luid | Exactly 8 bytes: low DWORD then high DWORD, both little-endian bit patterns; no signed integer ambiguity |
| PrimaryIdentity.session_id / Token.attributes / Error.native_status | Unsigned 32-bit values; native status preserves SECURITY_STATUS bit pattern |
| Identity.user / domain / password | Each ≤8,192 UTF-8 bytes; explicit user nonempty; empty domain/password allowed; CurrentLogon requires all three null, Explicit requires all three nonnull |
| Begin.target | Nonempty, ≤1,024 UTF-8 bytes; for Negotiate/NTLM exactly HTTP/<core-canonical-host>, no port/path or reverse DNS; Digest native target is supplied by core under its existing HTTP contract |
| Begin.channel_binding | Exactly ASCII `tls-server-end-point:` (21 bytes) plus the actual verified origin certificate digest (32/48/64 bytes), total 53/69/85 bytes; worker checks the prefix and constructs native SEC_CHANNEL_BINDINGS, never accepts serialized native pointers/offsets |
| Begin.token_limit | 1–65,536 inclusive; core supplies its existing HTTP token limit translated to raw bytes after scheme/base64 overhead; no new configuration knob |
| DigestInput.initial_challenge / Challenge.payload | Nonempty, ≤Begin.token_limit and provider maximum; never parsed/rebuilt by this library |
| DigestInput.method | Nonempty ASCII HTTP token, ≤64 bytes; actual request method, no case conversion |
| DigestInput.uri | Nonempty ASCII percent-encoded request URI, ≤8,192 bytes, no whitespace/control characters; actual URI, no decoding/re-encoding |
| Token.payload | Opaque bytes, empty permitted; ≤Begin.token_limit and provider maximum |
| Challenge.round / Token.round | 1–8 overall; Challenge uses only 2–8, exactly next expected round; first Token is round 1 |

Package values are Negotiate=1, Ntlm=2, Digest=3. The schema's Identity.user/domain
are already split by the caller (DOMAIN\\user once; UPN with empty domain).
AuthRequest owns a required token_limit: TokenLimit, constructed by the host via
TokenLimit::new(raw_bytes: u32). Valid range is 1–65,536 inclusive; 0 and 65,537
return InvalidRequest before start/registration. There is no default. Host derives
raw bytes from its existing HTTP header bound after scheme/base64 overhead,
without transferring HTTP types or policy into this library. Parent stores the
immutable value and copies it exactly to Begin.token_limit. Two otherwise equal
requests with different declared caps therefore carry different Begin values.
Parent refuses oversized initial/subsequent caller input as InvalidRequest before
sending; worker also checks it before credential/context processing. Worker
provider overproduction is ProviderRejected; an oversized Token received by the
parent is Protocol before caller publication. These are terminal conversation
failures with the existing cleanup contract; no cap expansion or retry.

No textual identity enters Hello: parent verifies the child's actual SID/LUID/
session against its captured primary token before sending any Begin/secret.
Do not accept identity asserted by bootstrap arguments. Fingerprints identify
matching artifacts, not trust an arbitrary executable: the trusted installed
path and creation-time containment remain independent preconditions.

Begin.DigestInput is nonnull iff package=Digest; Digest also requires Explicit
identity. The entire initial challenge, method and URI travel in Begin and feed
the first native step. Other packages forbid Digest fields. Parent validates hard
and HTTP bounds before sending; worker validates again, obtains package maximum
from native package metadata and intersects it with token_limit before credential
acquisition/context initialization. Package metadata query is not authentication.
Neither side enlarges a bound. Values that individually fit but exceed total
100,000 encoded bytes refuse, rather than fragmenting a message.

MechanismObservation.Unresolved requires null mechanism and authoritative=false.
Selected requires a nonnull Mechanism (Kerberos=1, Ntlm=2, Digest=3). Negotiate
permits Kerberos or Ntlm only; direct Ntlm/Digest require their own mechanism.
Complete requires Selected and authoritative=true. Continue may be unresolved or
provisional; no per-mechanism restriction is added. Native observation/disposal
rules stay those of the accepted design. Token attributes are numeric flags,
not proof of mechanism, HTTP acceptance or Git success.

## 3. Ordered conversation

| State | Allowed next input/output | Transition |
|---|---|---|
| Worker starts, parent awaits Hello | Worker Hello, or Error(bootstrap) | Parent checks version/fingerprints/actual identity, then Ready; mismatch terminates contained worker before Begin |
| Ready, no native credentials/context | Parent Begin, or Finish | Begin performs first step and waits for Token(round=1); Finish enters Closing without native initialization |
| Waiting for native step r | Worker Token(round=r), or Error(begin for r=1, challenge otherwise) | Continue at r<8 → BetweenSteps; Complete → Complete; Continue at r=8 is refused, not published |
| BetweenSteps after Continue r | Parent Challenge(round=r+1), or Finish | Challenge performs next step; Finish enters Closing |
| Complete | Parent Finish only | Closing; another Challenge/Begin refuses |
| Closing | Worker Finished, or Error(finish) | Finished marks acknowledged normal disposal, then worker closes pipes/exits; error enters terminal failure |
| Terminal / cancellation / deadline | No publishable messages | Late frames are discarded in zeroizing storage; no token, completion or capacity release can result |

Hello is exactly once, worker first. Begin is exactly once. Worker Error is
terminal, can replace the expected worker reply in its stated phase, and carries
only fixed ErrorKind (InvalidRequest, Protocol, IdentityMismatch, ProviderRejected,
Internal) plus nullable numeric native_status. Native status is nonnull only for
ProviderRejected; other kinds require null. No freeform message/provider/identity
string, stdout logging or stderr diagnostic payload. Invalid message/state
refuses with fixed Protocol when safe to respond, otherwise closes IPC; parent
revokes publication and terminates/reaps. Parent cancellation and timeout are
local supervisor results, not commands or worker error classes. No IPC Cancel,
worker clock, timeout allowance or secret-bearing recovery record is introduced.

One command and corresponding response may be pending. Public mutable Conversation
ownership serializes step and finish. Finish during negotiation is legal between
steps; it does not pass an outstanding command. Dropping a pending step cancels
the whole conversation and kills the Job instead of sending a competing Finish.
Control/deadline observation never waits for a worker reply or write completion.
Round numbering validates correlation on ordered pipes; it is not an OOO network
protocol. Wrong/repeated/skipped rounds refuse before native processing and before
parent token publication. Native API credentials/context handles never cross IPC.

Finished is an empty acknowledgement of successful normal native cleanup, not
proof of process exit. finish still waits for held process exit, empty Job and
completed launch/read/write threads. Error/EOF/kill request also never release
capacity by themselves. Pending/quarantined cleanup follows the accepted supervisor
contract. Forced exit is containment, not physical erasure or external-agent abort.
The immutable parent deadline bounds all commands, including Finish; no retry or
per-round reset is introduced.

## 4. Secret ownership and generated-code stop

The schema classifies user/domain/password, SID/LUID, challenges, tokens, binding,
method/URI/target and whole frames as sensitive. The IR describes storage-neutral
wire types, not permission to use ordinary generated owned strings/bytes. The
future Rust projection must use owned non-Clone/non-Debug zeroizing storage for
all those values; no raw payload in errors, panic formatting, traces or captures.
Zeroization must cover partial decode, refused semantic/state checks, successful
handoff, native UTF-16 conversion, temporary encode buffers, abandoned writes and
worker output-buffer copy/disposal. Validate declared string/byte lengths before
allocation/copy; allocate bounded zeroizing buffers directly, not a generic CBOR
value tree with ordinary intermediate Vec/String copies. Bounded references into
an owned frame may exist only while that zeroizing frame is held; they cannot
escape into blocking I/O/native work. Safe normal drops wipe owned storage;
forced process termination does not establish physical erasure.

Each slot owns at most one outbound and one inbound frame with no growing queue.
Frames stay charged/owned until dedicated I/O threads finish, including cancellation.
A stalled partial frame therefore holds that worker's existing slot and storage.
No command can publish after parent terminal arbitration even if fully written
before cancellation. The parent does not expose Hello/Token/Error IPC structures
as its public caller API; owned caller values cross a narrow adapter later.

No Rust codec or reference Rust binding is generated in this checkpoint. The
accepted design's stop before secrets enter production is active. Next implement
an IR-driven secret projection, prove its zeroization/error/admission paths with
fake ports, and obtain dual Code/State GO together with caller secret values
before supervision implementation. This schema GO cannot satisfy that gate.

## 5. Artifacts and checks

`python -B scripts/regen_schema.py` under the pinned taut-proto 0.10.0 release
validates the DSL and writes canonical sorted-key, indent-2, UTF-8 JSON IR ending
in one newline. It also writes protocol/contract.json with SHA-256 hashes of exact
IR and this document. schema_fingerprint = SHA256 of the ASCII domain separator
`gwz-sspi-contract-v1` followed by a NUL byte, followed by IR bytes, followed by
this document's bytes. Semantic changes therefore change the contract fingerprint
as well as schema edits. Do not hash the manifest itself. A host embeds the expected
32-byte contract fingerprint; the matched worker embeds the same value. Host
build_fingerprint comes from matching installed release artifacts; it is not
selected from untrusted message content. Different library/worker semantics never
negotiate a lesser mode; a future intentional protocol change revises version.

`--check` refuses drift without writing. Generator provenance checks installed
release version and imported module origins, rejecting PYTHONPATH/workspace shadows.
Cargo builds remain Python-free and depend on no other GWZ member. Development/CI
schema checks use pinned Python tooling only; no Cargo dependency is introduced.

Synthetic schema tests prove authored/exported identity, exact field/tag profiles,
canonical reference bytes, enum/required/type/length/depth refusal at the taut
layer and known reference-codec limitations. They do not prove private-profile
admission, framing reassembly, zeroization, lifecycle implementation or native
behavior. Future codec tests must add every bound±1, duplicates/unknown keys,
all zero/one/two body cases, malformed/truncated/noncanonical frames, package and
mechanism combinations, every state/round, partial I/O, terminal arbitration and
seeded chunking with exact failure replay. Real process/native tests stay separate.
