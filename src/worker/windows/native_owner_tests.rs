#[test]
#[ignore = "opt-in real Windows secur32; no remote authentication"]
fn native_ntlm_owners_check_status_and_wipe_before_provider_free() {
    use super::super::super::super::native::Provider;
    use crate::secret::audit::Probe;
    use crate::{SecretBytes, SecretText, TokenLimit};
    for explicit in [false, true] {
        let mut binding = crate::secret::Storage::zeroed(53);
        binding.as_mut()[..21].copy_from_slice(b"tls-server-end-point:");
        let request = AuthRequest {
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
            channel_binding: SecretBytes(binding),
            token_limit: TokenLimit::new(65536).unwrap(),
            digest: None,
        };
        let maximum = super::super::provider::System
            .maximum(Package::Ntlm)
            .unwrap();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut native =
            Conversation::acquire_audited(&request, &Probe(Some(events.clone()))).unwrap();
        assert!(native.credential_live);
        let mut raw = native.initialize(&[]).unwrap();
        assert_eq!(raw.status, 0x90312);
        assert!(native.context_live);
        let len = raw.output.bytes().unwrap().len();
        assert!(len > 0 && len <= maximum.min(65536) as usize);
        let token = SecretBytes::new(raw.output.bytes().unwrap());
        raw.output.dispose().unwrap();
        drop(raw);
        drop(token);
        native.cleanup().unwrap();
        assert!(!native.context_live && !native.credential_live);
        assert!(native.cleanup_status.is_none());
        drop(native);
        let events = events.lock().unwrap();
        assert!(events.contains(&(len, true)));
        assert!(events.iter().all(|(_, zero)| *zero));
        assert!(
            events.contains(&(88, true)),
            "entire padded CBT allocation wiped"
        );
    }
}
