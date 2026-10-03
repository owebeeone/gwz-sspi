use gwz_sspi::{
    AuthRequest, Cancellation, Deadline, Identity, Mechanism, MechanismObservation, Options,
    Package, SecretBytes, SecretText, Supervisor, TokenLimit, TokenStatus, WorkerExecutable,
};
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};
struct Notify(std::thread::Thread);
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(Notify(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::park(),
        }
    }
}
fn request(explicit: bool) -> AuthRequest {
    let mut binding = [0; 53];
    binding[..21].copy_from_slice(b"tls-server-end-point:");
    AuthRequest {
        package: Package::Ntlm,
        target: SecretText::new("HTTP/localhost").unwrap(),
        identity: if explicit {
            Identity::Explicit {
                user: SecretText::new("synthetic😀").unwrap(),
                domain: SecretText::new("").unwrap(),
                password: SecretText::new("synthetic-not-a-credential😀").unwrap(),
            }
        } else {
            Identity::CurrentLogon
        },
        channel_binding: SecretBytes::new(&binding),
        token_limit: TokenLimit::new(65536).unwrap(),
        digest: None,
    }
}
fn supervisor() -> Supervisor {
    assert_eq!(
        option_env!("GWZ_SSPI_BUILD_FINGERPRINT"),
        Some("4242424242424242424242424242424242424242424242424242424242424242"),
        "build fixture with documented synthetic compile-time metadata"
    );
    Supervisor::new(
        WorkerExecutable::new(
            std::path::PathBuf::from(env!("CARGO_BIN_EXE_gwz-sspi-worker")),
            [0x42; 32],
        )
        .unwrap(),
        Options::default(),
    )
    .unwrap()
}
fn deadline() -> Deadline {
    Deadline::new(Instant::now() + Duration::from_secs(20))
}
#[test]
#[ignore = "opt-in production Windows worker, primary Hello, Job and native cleanup"]
fn production_worker_finishes_before_begin_and_after_initial_ntlm() {
    for mode in [0, 1, 2] {
        let supervisor = supervisor();
        let mut conversation =
            wait(supervisor.start(request(mode == 2), deadline(), Cancellation::new())).unwrap();
        if mode != 0 {
            let token = wait(conversation.step(None)).unwrap();
            assert_eq!(token.status, TokenStatus::Continue);
            assert!(matches!(
                token.observation,
                MechanismObservation::Selected {
                    mechanism: Mechanism::Ntlm,
                    authoritative: true
                }
            ));
            assert!(!token.payload.as_bytes().is_empty());
            drop(token);
        }
        wait(conversation.finish()).unwrap();
        let report = wait(supervisor.shutdown(deadline()));
        assert!(report.outstanding.is_empty());
        assert_eq!(report.confirmed, 1);
    }
}
#[test]
#[ignore = "opt-in local initial production Negotiate, no remote authentication"]
fn production_worker_initial_negotiate_observation_is_local() {
    let supervisor = supervisor();
    let mut request = request(true);
    request.package = Package::Negotiate;
    let mut conversation =
        wait(supervisor.start(request, deadline(), Cancellation::new())).unwrap();
    let token = wait(conversation.step(None)).unwrap();
    assert!(matches!(
        token.observation,
        MechanismObservation::Unresolved
            | MechanismObservation::Selected {
                mechanism: Mechanism::Kerberos | Mechanism::Ntlm,
                ..
            }
    ));
    if token.status == TokenStatus::Complete {
        assert!(matches!(
            token.observation,
            MechanismObservation::Selected {
                authoritative: true,
                ..
            }
        ));
    }
    println!(
        "initial production Negotiate observation={:?} status={:?}",
        token.observation, token.status
    );
    drop(token);
    wait(conversation.finish()).unwrap();
    assert!(wait(supervisor.shutdown(deadline())).outstanding.is_empty());
}
