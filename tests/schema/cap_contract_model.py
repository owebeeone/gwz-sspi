"""Synthetic executable caller-cap specification, NOT a production Rust API.

Only fixed fixture data enters this model. Native/codec enforcement is unproved.
"""
from dataclasses import dataclass


@dataclass(frozen=True)
class TokenLimitModel:
    raw_bytes: int

    def __post_init__(self):
        if type(self.raw_bytes) is not int or not 1 <= self.raw_bytes <= 65536:
            raise ValueError("InvalidRequest")


@dataclass(frozen=True)
class AuthRequestModel:
    token_limit: TokenLimitModel  # required, no default

    def begin(self):
        return {
            "package": "negotiate", "target": "HTTP/fixture.invalid",
            "identity": {"mode": "current_logon", "user": None,
                         "domain": None, "password": None},
            "channel_binding": b"tls-server-end-point:" + b"f" * 32,
            "token_limit": self.token_limit.raw_bytes, "digest": None,
        }


def admit_length(length, request, provider_maximum, source):
    """Model only the caller/native/wire boundaries specified in the design."""
    cap = min(request.token_limit.raw_bytes, provider_maximum)
    if length < 0 or length > cap:
        errors = {"caller": "InvalidRequest", "provider": "ProviderRejected",
                  "wire": "Protocol"}
        raise ValueError(errors[source])
