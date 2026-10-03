use gwz_sspi::{ErrorKind, SecretBytes, SecretText, TokenLimit};

#[test]
fn checked_cap_and_caller_source_ownership() {
    assert_eq!(
        TokenLimit::new(0).err().unwrap().kind(),
        ErrorKind::InvalidRequest
    );
    assert_eq!(
        TokenLimit::new(65537).err().unwrap().kind(),
        ErrorKind::InvalidRequest
    );
    assert_eq!(TokenLimit::new(65536).unwrap().raw_bytes(), 65536);
    let source = b"synthetic-only";
    let owned = SecretBytes::new(source);
    assert_eq!(owned.as_bytes(), source);
    drop(owned);
    assert_eq!(source, b"synthetic-only");
    let text = SecretText::new("synthetic-only").unwrap();
    assert_eq!(text.as_str(), "synthetic-only");
    assert!(SecretText::new("bad\0text").is_err());
}
