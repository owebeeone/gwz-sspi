use super::{
    supervision::{Reply, reply},
    test_vectors::VECTORS,
};
use crate::secret::Storage;
use crate::supervisor::{kernel::Expected, ports::Primary};
use crate::{ErrorKind, Package, SecretBytes, TokenLimit};
fn vector(name: &str) -> Storage {
    Storage::copy(VECTORS.iter().find(|(key, _)| *key == name).unwrap().1)
}
fn primary() -> Primary {
    Primary {
        sid: SecretBytes::new(&[1, 0, 0, 0, 0, 0, 0, 0]),
        luid: SecretBytes::new(&[0; 8]),
        session: 0,
    }
}
#[test]
fn exact_phase_kind_error_phase_and_round_matrix_before_owned_secret_copy() {
    let primary = primary();
    let cap = TokenLimit::new(64).unwrap();
    for (name, expected, accepted) in [
        ("hello", Expected::Bootstrap, true),
        ("hello", Expected::Begin, false),
        ("token_kerberos", Expected::Challenge(2), true),
        ("token_kerberos", Expected::Begin, false),
        ("token_kerberos", Expected::Challenge(3), false),
        ("token_kerberos", Expected::Finish, false),
        ("finished", Expected::Finish, true),
        ("finished", Expected::Bootstrap, false),
        ("error", Expected::Begin, true),
        ("error", Expected::Bootstrap, false),
        ("error_provider", Expected::Challenge(2), true),
        ("error_provider", Expected::Finish, false),
        ("finish", Expected::Finish, false),
        ("begin_current", Expected::Bootstrap, false),
    ] {
        let result = reply(
            &vector(name),
            expected,
            &primary,
            &[b'b'; 32],
            Package::Negotiate,
            cap,
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "name={name},expected={expected:?}"
        );
        if !accepted {
            assert_eq!(result.err().unwrap().kind(), ErrorKind::Protocol);
        }
    }
    for expected in [
        Expected::Bootstrap,
        Expected::Begin,
        Expected::Challenge(2),
        Expected::Finish,
    ] {
        let mut frame = vector("error");
        let len = frame.as_slice().len();
        frame.as_mut()[len - 3] = match expected {
            Expected::Bootstrap => 1,
            Expected::Begin => 2,
            Expected::Challenge(_) => 3,
            Expected::Finish => 4,
        };
        assert!(matches!(
            reply(
                &frame,
                expected,
                &primary,
                &[b'b'; 32],
                Package::Negotiate,
                cap
            ),
            Ok(Reply::Failure(_))
        ));
    }
}
#[test]
fn bootstrap_fingerprint_identity_and_malformed_refusal_are_distinct() {
    let primary = primary();
    let cap = TokenLimit::new(64).unwrap();
    let hello = vector("hello");
    assert!(matches!(
        reply(
            &hello,
            Expected::Bootstrap,
            &primary,
            &[b'b'; 32],
            Package::Ntlm,
            cap
        ),
        Ok(Reply::Hello)
    ));
    assert_eq!(
        reply(
            &hello,
            Expected::Bootstrap,
            &primary,
            &[b'x'; 32],
            Package::Ntlm,
            cap
        )
        .err()
        .unwrap()
        .kind(),
        ErrorKind::WorkerMismatch
    );
    let wrong = Primary {
        sid: SecretBytes::new(&[1, 0, 0, 0, 0, 0, 0, 1]),
        luid: SecretBytes::new(&[0; 8]),
        session: 0,
    };
    assert_eq!(
        reply(
            &hello,
            Expected::Bootstrap,
            &wrong,
            &[b'b'; 32],
            Package::Ntlm,
            cap
        )
        .err()
        .unwrap()
        .kind(),
        ErrorKind::IdentityMismatch
    );
    for cut in 0..hello.as_slice().len() {
        assert_eq!(
            reply(
                &Storage::copy(&hello.as_slice()[..cut]),
                Expected::Bootstrap,
                &primary,
                &[b'b'; 32],
                Package::Ntlm,
                cap
            )
            .err()
            .unwrap()
            .kind(),
            ErrorKind::Protocol,
            "cut={cut}"
        );
    }
}
