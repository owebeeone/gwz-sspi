# Changelog

## 0.1.1

Windows launch only; no change to the public API, the wire protocol or the worker.

- The worker's pipes are created non-inheritable, and so is its NUL stderr handle. The three
  child-side handles (the child's stdin and stdout pipe ends, and NUL) are made inheritable
  only for the `CreateProcessW` call, by a guard that closes on every path, a failed call
  included. Before, they were inheritable from creation until they were dropped after the
  call, so a thread that spawned another process in that interval (a std `Command`, the git
  command line) with every inheritable handle passed on would hand that process the worker's
  token-carrying pipes, and a worker that died would not give the parent an end-of-file while
  the other process held them. The worker's own inheritance is unchanged: its explicit
  `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` still names exactly those three handles.
- What remains: during the `CreateProcessW` call itself (milliseconds, longer under an
  antivirus scan) a concurrent spawn that names no handle list still inherits the three
  child-side handles. A host closes that only by serializing such spawns with worker launch;
  0.1.1 has no API for the host to do so.
- New Windows tests: no end of a new pipe is inheritable, the NUL handle is not inheritable,
  the child ends are inheritable only inside the creation window, and a forced
  `CreateProcessW` failure leaves them closed to inheritance.

## 0.1.0

First crates.io release: the contained Windows SSPI authentication worker, its supervisor and
the packaged-worker composition checks (RELEASE.md).
