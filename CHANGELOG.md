# Changelog

## 0.1.2

Windows launch only; additive public API, no change to the wire protocol or the worker.

- New `Supervisor::with_launch_guard(executable, options, guard)` and `LaunchGuard`. The guard
  is a host callback that returns an owned value. The launch takes it after the
  originating-thread recheck and before the worker's pipe ends become inheritable, and drops
  it after they have turned back and `CreateProcessW` has returned, on every path that reaches
  the window, a failed call included. A host fills it with its spawn gate and makes its other
  spawns that name no handle list (std `Command`, the git command line) take the same gate, which
  closes the residual window that 0.1.1 left open for the length of `CreateProcessW`.
  `LaunchGuard::hold` takes the guard once, for a host to test the callback it installs.
- Without a guard (`Supervisor::new`) behaviour is exactly 0.1.1's. `Options` is unchanged: it is
  `Copy` and has public fields, so the callback is a separate argument.
- New tests: on Windows, the guard is taken before the window opens and released after it has
  closed and the worker exists, on success and on a forced `CreateProcessW` failure, and is not
  taken by a launch refused before the window; off Windows, the supervisor hands the platform
  the guard it was given; an opt-in row runs the production worker under a guard.

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
  0.1.1 has no API for the host to do so (0.1.2 adds `LaunchGuard`).
- New Windows tests: no end of a new pipe is inheritable, the NUL handle is not inheritable,
  the child ends are inheritable only inside the creation window, and a forced
  `CreateProcessW` failure leaves them closed to inheritance.

## 0.1.0

First crates.io release: the contained Windows SSPI authentication worker, its supervisor and
the packaged-worker composition checks (RELEASE.md).
