# Parent supervision

The parent lifecycle is implemented for Windows, using the strict private codec.
The production worker still exits with a fixed refusal; native SSPI and worker_entry
are deferred. Successful fake tokens do not establish native or HTTP authentication.
No runtime, network, core/Git or Python dependency is introduced.

| Public value | Construction or operation |
|---|---|
| WorkerExecutable | new(PathBuf, [u8;32]) checks an absolute path; path() borrows it. The host supplies the trusted installed artifact-set fingerprint. |
| Options | max_workers: u8; Default is eight, Supervisor::new checks 1–64. |
| Deadline | new(Instant), instant(); immutable, no default or round extension. |
| Cancellation | new()/Default, Clone, cancel(), is_cancelled(); shared owned signal. |
| Supervisor | new(executable, options); start(request, deadline, cancellation); cleanup_status(RecordId); shutdown(separate_deadline). |
| Conversation | step(&mut self, Option<SecretBytes>); finish(self); cancel(self). |
| RecordId | opaque context-scoped checked monotonic identity; Clone/Eq/Debug, no caller constructor. |
| Failure | kind(), native_status(), record_id(): Option<&RecordId>, cleanup_status(). Fixed diagnostics, no credential text. |
| CancellationReceipt | record_id and cleanup fields. |
| ShutdownReport | confirmed lifetime count and outstanding Vec<RecordId>. |
| CleanupStatus | Pending, Confirmed, Unknown. |

Supervisor::new synchronously captures primary token SID/authentication LUID/session
and refuses impersonation. start synchronously validates the request, snapshots and
refuses caller-thread impersonation/current-primary change, and captures an owned
originating-thread handle. These are synchronous OS metadata probes without a
numeric time bound. Moving a returned future never substitutes the executor thread.
All later metadata checks, process creation, pipe calls and native cleanup are
charged and offloaded. Future::poll performs no native metadata/process/thread
creation call and does not wait on IPC. Non-Windows construction returns
UnsupportedPlatform after options validation; owned values remain cross-platform.

start owns the request and waits FIFO for capacity and a matching Hello. No Begin
or credentials are written before verified protocol/schema/build/actual-primary
identity. step(None) begins; each subsequent step requires an opaque nonempty
challenge. The mutable borrow serializes steps, and the immutable operation deadline
covers all eight permitted rounds. Complete prohibits another step; it describes
native token generation, not HTTP acceptance. finish is legal before Begin, between
steps and after Complete; it uses the same operation deadline and confirms actual
cleanup. Dropping a step or Conversation cancels the entire conversation. Dropping a
start before registration removes its waiter; afterward retained supervision owns
queued launch and all created resources. Dropping a completed failed start releases
its unregistered input/snapshot immediately during refusal, even if the future is held.

Cancellation and expiry revoke publication atomically at the parent state guard.
The first recorded terminal error wins; late Hello/Token/Finished cannot revive it.
A ready result is rechecked when polled, including after normal reaping. Cancellation
is chosen when cancellation and expiry are first observed together. Failures after
registration include a RecordId and may report Pending. Killing, EOF, Finished or
successful advisory I/O cancellation alone cannot establish Confirmed. Held process
exit, zero active Job processes, actual launch/read/write completion and their
ownership disposal are required. Quarantined records retain capacity indefinitely
until those proofs exist. Confirmed tombstones are a 256-entry FIFO; evicted and
foreign IDs are Unknown. shutdown closes admission immediately and waits only to
its explicitly separate deadline; dropping its future leaves supervision running.

The following recipe compiles without an async runtime. A host executor may call
it on Windows with an already trusted Supervisor and request. It deliberately
finishes after the first token: an early Finish is legal. The current refusing
worker produces a Failure rather than the fake success used by unit tests.

```rust
use gwz_sspi::{AuthRequest, Cancellation, Deadline, Failure, Supervisor, TokenStep};

async fn first_token_then_dispose(
    supervisor: &Supervisor,
    request: AuthRequest,
    deadline: Deadline,
    cancellation: Cancellation,
) -> Result<TokenStep, Failure> {
    let mut conversation = supervisor.start(request, deadline, cancellation).await?;
    let token = conversation.step(None).await?;
    conversation.finish().await?;
    Ok(token) // Owned payload; host still decides HTTP/mechanism acceptance.
}
```

Supervisor and Conversation have neither Clone nor Debug; both are Send. Supervisor
is Sync. Caller-owned source buffers retain their separate wiping obligation.
Windows process runtime tests, provider UTF-16/native buffers, installed-worker
composition and native SSPI qualification are separate future gates.
