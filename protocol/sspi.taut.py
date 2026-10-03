"""Private SSPI worker IPC v1. Semantics and bounds: docs/WireProtocol.md.

These declarations contain no real credentials. The reference bindings are not
secret-safe and must not be used for production authentication.
"""
from taut.ir.dsl import BOOL, BYTES, INT, STR, Enum, F, Msg, Ref, option, schema

SCHEMA = schema(
    option.max_depth(8),
    option.max_encoded_len(100_000),
    MessageKind=Enum(hello=1, begin=2, challenge=3, token=4,
                     finish=5, finished=6, error=7),
    Package=Enum(negotiate=1, ntlm=2, digest=3),
    IdentityMode=Enum(current_logon=1, explicit=2),
    TokenStatus=Enum(continue_needed=1, complete=2),
    ObservationKind=Enum(unresolved=1, selected=2),
    Mechanism=Enum(kerberos=1, ntlm=2, digest=3),
    ErrorKind=Enum(invalid_request=1, protocol=2, identity_mismatch=3,
                   provider_rejected=4, internal=5),
    ErrorPhase=Enum(bootstrap=1, begin=2, challenge=3, finish=4),
    PrimaryIdentity=Msg(sid=F(1, BYTES), authentication_luid=F(2, BYTES),
                        session_id=F(3, INT)),
    Hello=Msg(protocol_version=F(1, INT), schema_fingerprint=F(2, BYTES),
              build_fingerprint=F(3, BYTES), primary_identity=F(4, Ref.PrimaryIdentity)),
    Identity=Msg(mode=F(1, Ref.IdentityMode), user=F(2, STR, optional=True),
                 domain=F(3, STR, optional=True), password=F(4, STR, optional=True)),
    DigestInput=Msg(initial_challenge=F(1, BYTES), method=F(2, STR), uri=F(3, STR)),
    Begin=Msg(package=F(1, Ref.Package), target=F(2, STR),
              identity=F(3, Ref.Identity), channel_binding=F(4, BYTES),
              token_limit=F(5, INT), digest=F(6, Ref.DigestInput, optional=True)),
    Challenge=Msg(round=F(1, INT), payload=F(2, BYTES)),
    MechanismObservation=Msg(kind=F(1, Ref.ObservationKind),
                             mechanism=F(2, Ref.Mechanism, optional=True),
                             authoritative=F(3, BOOL)),
    Token=Msg(round=F(1, INT), status=F(2, Ref.TokenStatus), attributes=F(3, INT),
              observation=F(4, Ref.MechanismObservation), payload=F(5, BYTES)),
    Finish=Msg(),
    Finished=Msg(),
    Error=Msg(kind=F(1, Ref.ErrorKind), phase=F(2, Ref.ErrorPhase),
              native_status=F(3, INT, optional=True)),
    # All optional fields are present on wire; inactive bodies are CBOR null.
    Envelope=Msg(version=F(1, INT), kind=F(2, Ref.MessageKind),
                 hello=F(10, Ref.Hello, optional=True),
                 begin=F(11, Ref.Begin, optional=True),
                 challenge=F(12, Ref.Challenge, optional=True),
                 token=F(13, Ref.Token, optional=True),
                 finish=F(14, Ref.Finish, optional=True),
                 finished=F(15, Ref.Finished, optional=True),
                 error=F(16, Ref.Error, optional=True)),
)
