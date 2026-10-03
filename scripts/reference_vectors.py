"""Synthetic golden vectors from the pinned taut reference, not production data."""
import json


def vectors(schema, root, contract):
    from taut.wire import codec
    def envelope(kind, body):
        value = {f.name: None for f in schema.messages['Envelope'].fields}
        value.update(version=1, kind=kind)
        value[kind] = body
        return value
    explicit = dict(mode='explicit', user='synthetic', domain='', password='synthetic-only')
    current = dict(mode='current_logon', user=None, domain=None, password=None)
    def begin(package, identity, digest=None):
        return dict(package=package, target='fixture.invalid' if package == 'digest' else 'HTTP/fixture.invalid',
                    identity=identity, channel_binding=b'tls-server-end-point:' + b'b'*32,
                    token_limit=64, digest=digest)
    def token(status, kind, mechanism, authoritative, payload=b'fixture-token'):
        return dict(round=2, status=status, attributes=4294967295,
                    observation=dict(kind=kind, mechanism=mechanism, authoritative=authoritative), payload=payload)
    fixtures = {
        'hello': ('hello', dict(protocol_version=1, schema_fingerprint=bytes.fromhex(contract),
                  build_fingerprint=b'b'*32, primary_identity=dict(sid=bytes([1,0])+b'\0'*6,
                  authentication_luid=b'\0'*8, session_id=0))),
        'begin_current': ('begin', begin('ntlm', current)),
        'begin_explicit': ('begin', begin('negotiate', explicit)),
        'begin_digest': ('begin', begin('digest', explicit, dict(initial_challenge=b'fixture-challenge', method='GET', uri='/fixture%20path'))),
        'challenge': ('challenge', dict(round=2, payload=b'fixture-challenge')),
        'token_unresolved': ('token', token('continue_needed','unresolved',None,False)),
        'token_provisional': ('token', token('continue_needed','selected','kerberos',False)),
        'token_kerberos': ('token', token('complete','selected','kerberos',True)),
        'token_ntlm': ('token', token('complete','selected','ntlm',True,b'')),
        'token_digest': ('token', token('complete','selected','digest',True)),
        'finish': ('finish', {}), 'finished': ('finished', {}),
        'error': ('error', dict(kind='protocol', phase='begin', native_status=None)),
        'error_provider': ('error', dict(kind='provider_rejected',phase='challenge',native_status=4294967295)),
    }
    text = '// Generated synthetic pinned-taut reference vectors; do not edit.\n'
    text += 'pub(super) const VECTORS: &[(&str, &[u8])] = &[\n'
    for name, (kind, body) in fixtures.items():
        encoded = codec.encode(schema, 'Envelope', envelope(kind, body))
        text += '(' + json.dumps(name) + ', &[' + ','.join(str(b) for b in encoded) + ']),\n'
    text += '];\n'
    import subprocess
    result = subprocess.run(['rustfmt','--edition','2024'],input=text.encode(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
    return {root/'src/protocol/test_vectors.rs': result.stdout}
