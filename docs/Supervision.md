# Parent supervision

The parent lifecycle is implemented for Windows, using the strict private codec.
Shared worker_entry and serial Negotiate/NTLM native processing are implemented,
pending native implementation review. Fake tokens do not establish native or HTTP authentication.
No runtime, network, core/Git or Python dependency is introduced.

| Public value | Construction or operation |
|---|---|
| WorkerExecutable | new(PathBuf, [u8;32]) -> Result<WorkerExecutable, Error> checks an absolute path; path() borrows it. Trusted host packaging supplies build_fingerprint. |
| Options | max_workers: u8; Default is eight, Supervisor::new checks 1–64. |
| Deadline | new(Instant), instant(); immutable, no default or round extension. |
| Cancellation | new()/Default, Clone, cancel(), is_cancelled(); shared owned signal. |
| Supervisor | new(executable, options); capture_caller(); start_captured(&CallerCapture, request, deadline, cancellation); start(request, deadline, cancellation); cleanup_status(RecordId); shutdown(separate_deadline). |
| CallerCapture | Opaque owned original-thread provenance; share through Arc, scoped to its issuing Supervisor. |
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
After registration, metadata checks, process creation, pipe calls and native child
cleanup are charged and offloaded. Poll performs no metadata/provider query,
worker/thread creation, IPC, process wait or join. Before registration, refusal
or Drop can synchronously dispose the captured originating-thread handle outside
state locks. This handle-close exception has no hard OS time bound. Non-Windows construction returns
UnsupportedPlatform after options validation; owned values remain cross-platform.

start and shutdown return owned Send + 'static futures, independent of the
Supervisor receiver lifetime. They can be retained or transferred after dropping
Supervisor; that Drop still initiates cancellation. step keeps its required mutable
Conversation borrow. start owns the request and waits FIFO for capacity and a matching Hello. No Begin
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
finishes after the first token: an early Finish is legal. A matching trusted worker can produce the initial native token; packaging/refusal
and provider qualification remain separate from HTTP authentication.

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
Opt-in Windows fixtures exercise process/native cleanup; installed-worker
composition and complete native/provider qualification remain separate gates.

## Trusted packaging input and owned handoff

The expected 32 bytes come from the host's trusted packaging manifest or build
output field `build_fingerprint`. The packaging producer must supply identical
bytes to the matching worker's Hello. This interface does not prescribe hashing
an executable at runtime, and there is no exported library constant to substitute
for trusted host metadata. The actual installation/packaging producer remains
step 4; a worker built without valid trusted metadata refuses before Hello. The host verifies
and installs its matching artifact set together, then passes that trusted metadata
and its exact absolute worker path to WorkerExecutable::new.

This synthetic host-owned record names those inputs without inventing a hash rule.
The functions compile on every platform but are not invoked by the example.
Calling configure uses synchronous metadata/thread setup on Windows; elsewhere it
returns UnsupportedPlatform. The fixture is not production packaging evidence.

```rust
use gwz_sspi::{AuthRequest, Cancellation, Deadline, Error, Options, Supervisor,
               WorkerExecutable};
use std::{future::Future, path::PathBuf};

struct TrustedHostPackaging {
    absolute_worker_path: PathBuf,
    build_fingerprint: [u8; 32], // Exact bytes provisioned to matching worker Hello.
}

fn configure(metadata: TrustedHostPackaging) -> Result<Supervisor, Error> {
    let worker = WorkerExecutable::new(metadata.absolute_worker_path,
                                       metadata.build_fingerprint)?;
    Supervisor::new(worker, Options::default())
}

fn retain_after_supervisor_drop(
    supervisor: Supervisor,
    request: AuthRequest,
    deadline: Deadline,
    cancellation: Cancellation,
) {
    fn require_owned<F: Future + Send + 'static>(_: F) {}
    let start = supervisor.start(request, deadline, cancellation);
    let shutdown = supervisor.shutdown(deadline);
    drop(supervisor);
    require_owned(start); // A real host can instead enqueue/retain these futures.
    require_owned(shutdown);
}

let synthetic_metadata = TrustedHostPackaging {
    absolute_worker_path: std::env::current_dir().unwrap().join("synthetic-worker"),
    build_fingerprint: [0x42; 32], // Synthetic fixture, not an installed artifact set.
};
drop(synthetic_metadata); // Do not launch or query native metadata in this example.
```

Completed failed steps dispose their owned challenge before returning, even if the
caller retains the completed future. Publication that loses a record to reaping
recovers its saved terminal cause; it cannot replace cancellation/expiry with a
new Protocol error. These obligations are checked with private fake ports and live
wipe probes, independently of deferred Windows runtime qualification.

## Capture before host handoff

Capture on the original CLI or Python entry before fanout, detach, or submission.
`capture_caller` does synchronous original-thread metadata/handle work without a
hard OS time bound. It allocates no worker or capacity record. Its final Drop may
close the held handle synchronously outside locks. `start_captured` synchronously validates only issuer identity, request profile
and admission state; a different Supervisor returns InvalidRequest. Original
held-thread liveness and identity are rechecked at charged launch, with no
metadata query in captured-start or Future::poll. Concurrent starts share that Supervisor's FIFO
capacity. Dropping the capture after creating Start does not invalidate its owned
origin; dropping the issuing Supervisor still cancels its admitted work.

The following recipe is compiled as a crate doctest. It creates the owned Start
before host submission and proves positive Send + Sync + 'static capture traits.
CallerCapture's negative Clone and Debug probes are on its API documentation.

```rust
use gwz_sspi::{AuthRequest, CallerCapture, Cancellation, Deadline, Failure,
               Supervisor, TokenStep};
use std::{future::Future, sync::Arc};
fn capture_on_entry(supervisor: &Supervisor) -> Result<Arc<CallerCapture>, gwz_sspi::Error> {
    fn require<T: Send + Sync + 'static>() {}
    require::<CallerCapture>();
    Ok(Arc::new(supervisor.capture_caller()?))
}
fn submit_owned(
    supervisor: &Supervisor, caller: &CallerCapture, request: AuthRequest,
    deadline: Deadline, cancellation: Cancellation,
) -> impl Future<Output = Result<TokenStep, Failure>> + Send + 'static {
    let start = supervisor.start_captured(caller, request, deadline, cancellation);
    async move {
        let mut conversation = start.await?;
        let token = conversation.step(None).await?;
        conversation.finish().await?;
        Ok(token)
    }
}
```

Native Complete describes provider token production. A network host must
independently validate the remote response, source/method consent, final-origin
channel binding and the exclusive physical connection generation. The HTTPS
composition keeps the one positive logical Open deadline through discovery,
helpers and every native round; zero refuses native Begin. Nonempty initial
Negotiate/NTLM offers and Digest are refused before Begin because this serial
API cannot consume their initial challenge via `step(None)`.

The disposal operations have these signatures: `Conversation::cancel(self) ->
CancellationReceipt`, `Supervisor::cleanup_status(&self, RecordId) -> CleanupStatus`,
and `Supervisor::shutdown(&self, Deadline) -> impl Future<Output = ShutdownReport>
+ Send + 'static`. `CancellationReceipt` exposes `record_id: RecordId` and
`cleanup: CleanupStatus`; `ShutdownReport` exposes `confirmed: usize` (the lifetime
confirmed count) and `outstanding: Vec<RecordId>`. Cancel is synchronous; shutdown
has a separate immutable bound. This recipe preserves the receipt for accounting:

```rust
use gwz_sspi::{CancellationReceipt, CleanupStatus, Conversation, Deadline,
               ShutdownReport, Supervisor};
async fn cancel_and_account(
    supervisor: &Supervisor, conversation: Conversation, shutdown_deadline: Deadline,
) -> (CancellationReceipt, CleanupStatus, ShutdownReport) {
    let receipt = conversation.cancel();
    let observed = supervisor.cleanup_status(receipt.record_id.clone());
    let report = supervisor.shutdown(shutdown_deadline).await;
    (receipt, observed, report)
}
```

Neither Pending nor Unknown permits release of a retained owner. The returned
shutdown report lists records whose cleanup is still unconfirmed at that bound;
its outstanding list may be nonempty. Dropping the shutdown future retains
supervision. An evicted tombstone can become Unknown even after earlier confirmation,
so a host must retain its own previously observed proof rather than invent it.
