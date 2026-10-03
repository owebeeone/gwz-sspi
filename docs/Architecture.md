# Architecture and package boundary

Role: integration library with owned contract/protocol values and private narrow
OS, IPC and clock ports. Public types expose no OS handles, core/Git types or
runtime-specific objects. See CallerValues.md and Supervision.md for the caller
surface. The production executable still refuses; native provider work is deferred.

Approved dependency allowlist: zeroize =1.9.0, defaults disabled, alloc only;
target-Windows windows-sys =0.61.2, defaults disabled, and its windows-link =0.2.1
transitive dependency. The lane owner reviewed released source before addition.
Allowed Windows features are Win32_Foundation, Win32_Security,
Win32_Storage_FileSystem, Win32_System_IO, Win32_System_JobObjects,
Win32_System_Pipes, Win32_System_Threading, Win32_System_SystemInformation and
Win32_System_SystemServices. No provider binding, runtime, build or dev dependency
is introduced. Any additional dependency kind requires explicit source review.

The accepted design is GWZ SSPI revision 2, including its caller-identity rule.
Workspace development documents are not build/CI dependencies. FFI unsafe code is
confined to enclosing Windows modules with lifetime/safety comments. Disabled
platform branches are checked by source-scope lint as well as target compilation.

Secret storage lives in src/secret.rs and src/values.rs. The private protocol under
src/protocol is generated from taut IR. Borrowed records retain the zeroizing frame;
profile/context admission precedes owned copies. Encoding validates and computes
its entire size before allocating zeroed fixed storage. There is no generic owned
CBOR tree or live-secret reallocation. Schema/IR/WireProtocol/fingerprints are
unchanged by supervision. The codec checks fingerprint/identity/cap/package/round;
supervision adds allowed kind, Error.phase and terminal publication eligibility.
The parent's provider ceiling is the independent absolute 65,536-byte bound, not a
claim that the parent queried a provider. Native worker narrowing remains step 3.

src/supervisor/kernel.rs contains deterministic phase/terminal/proof transitions.
Context owns a short state Mutex for admission and kernels, context-scoped checked
IDs and 256 FIFO tombstones. Payload, launch-handle and task ownership Mutexes are
never nested with that state lock or with one another. Payload locks guard bytes,
not transitions. No blocking/native call, handle destruction, join or host callback
runs under the state lock. Waker cloning, replacement/drop and waking occur outside
it; wake/drop panics are contained. Wait tickets hold Weak metadata without an
under-lock upgrade whose temporary might become the final host-waker owner.

Supervisor construction starts a deadline/control thread and separate bounded
dispatcher. Poll registers a charged record then queues its owned launch ticket.
The dispatcher performs OS thread creation and completed launch joins; a stalled
creation/join retains its slot while the independent control thread continues
arbitrating deadlines. Each admitted slot owns one launch/reaper and at most one
reader and writer. The launch owner holds late CreateProcess results, I/O handles
and fixed buffers until actual completion; dropping caller futures, runtime or
Supervisor does not abandon that ownership. Native resource disposal and payload
wiping finish on the charged owner before its launch thread is joined. Kill/EOF/
Finished/cancellation requests are not proof. Quarantine retains permits until held
exit, empty Job and every actual launch/I/O owner finish. No replacement capacity
is admitted against outstanding records.

Windows new/start use synchronous token metadata probes. start retains the actual
originating-thread handle; the charged launch owner rechecks its impersonation and
current process primary immediately before CreateProcess. Future::poll does no
metadata/provider/creation/IPC/process wait or join. A refused unregistered start
or its Drop may synchronously dispose the captured handle outside state locks,
without a hard OS time bound. Identity scratch is aligned, initialized, fixed and zeroizing; copied
SID/LUID owners are likewise wiping. No secret UTF-16 conversion is needed in this
parent phase. Provider/native credential buffers and disposal remain deferred.

Windows creation uses an unnamed KILL_ON_JOB_CLOSE Job, no breakaway, suspended
STARTUPINFOEX JOB_LIST plus an explicit HANDLE_LIST for exactly the child pipe ends
and NUL stderr. Membership is verified before resume; there is no Assign fallback.
Exact trusted absolute application path, private numeric bootstrap, OS-derived
minimal environment/cwd and anonymous pipes are owned internally. Child-end copies
close promptly. Attribute memory and boxed referenced arrays survive until Delete,
as required by [Microsoft's attribute lifetime contract](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute).
[CancelSynchronousIo is advisory](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio);
[ReadFile storage must remain live through completion](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfile).
The held Job/process and I/O owners remain charged until those actual observations,
not just requests. Cross-compilation supports inspection but does not qualify any
Windows runtime row. Publication, packaging integration and activation remain gated.

Remediation exposes only private bounded iterations of existing launch admission,
read/write frame handling, reaping and launch join. Native drivers call those same
iterations around their thread creation/wait loops. The private Owner port holds
the actual JoinHandle in production and requires actual completion before consuming
its join; fake owners provide explicit completion/join outcomes only in unit tests.
There is no public injection, new disposal queue, dependency or native/wire policy.
