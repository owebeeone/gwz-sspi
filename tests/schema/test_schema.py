"""Synthetic tooling/schema tests, NOT the production secret codec."""
import hashlib
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import regen_schema

PACKAGE = regen_schema.released_taut()
from taut.ir.load import load_schema, schema_from_json
from taut.ir.export import schema_json
from taut.ir.model import MsgRef
from taut.ir.validate import validate_or_raise
from taut.wire import codec, cbor
regen_schema.verify_taut_modules(PACKAGE)
SCHEMA = load_schema(ROOT / "protocol/sspi.taut.py")
regen_schema.verify_taut_modules(PACKAGE)


def envelope(kind, body):
    value = {field.name: None for field in SCHEMA.messages["Envelope"].fields}
    value.update(version=1, kind=kind)
    value[kind] = body
    return value


class SchemaTests(unittest.TestCase):
    def test_generated_artifacts_match_authored_schema_and_semantics(self):
        validate_or_raise(SCHEMA)
        outputs = regen_schema.artifacts(SCHEMA)
        for path, expected in outputs.items():
            self.assertEqual(path.read_bytes(), expected, path.name)
        exported = json.loads((ROOT / "protocol/sspi.ir.json").read_bytes())
        self.assertEqual(schema_json(schema_from_json(exported)), schema_json(SCHEMA))

    def test_all_seven_wire_kinds_and_envelope_body_tags_are_fixed(self):
        kinds = ["hello", "begin", "challenge", "token", "finish", "finished", "error"]
        self.assertEqual(SCHEMA.enums["MessageKind"].members,
                         dict(zip(kinds, range(1, 8))))
        fields = SCHEMA.messages["Envelope"].fields
        self.assertEqual([(f.name, f.tag, f.optional) for f in fields],
                         [("version", 1, False), ("kind", 2, False)] +
                         [(kind, tag, True) for kind, tag in zip(kinds, range(10, 17))])

    def test_digest_and_mechanism_fields_are_not_lost(self):
        self.assertEqual([(f.name, f.tag) for f in SCHEMA.messages["DigestInput"].fields],
                         [("initial_challenge", 1), ("method", 2), ("uri", 3)])
        self.assertEqual([f.name for f in SCHEMA.messages["MechanismObservation"].fields],
                         ["kind", "mechanism", "authoritative"])
        self.assertIn("token_limit", [f.name for f in SCHEMA.messages["Begin"].fields])

    def test_synthetic_round_trips_all_message_kinds(self):
        fixtures = {
            "hello": {"protocol_version": 1, "schema_fingerprint": b"s" * 32,
                      "build_fingerprint": b"b" * 32,
                      "primary_identity": {"sid": bytes([1, 0]) + b"\0" * 6,
                                           "authentication_luid": b"\0" * 8, "session_id": 0}},
            "begin": {"package": "digest", "target": "fixture.invalid",
                      "identity": {"mode": "explicit", "user": "synthetic",
                                   "domain": "", "password": "synthetic-only"},
                      "channel_binding": b"fixture-binding", "token_limit": 1024,
                      "digest": {"initial_challenge": b"fixture-challenge",
                                 "method": "GET", "uri": "/fixture%20path"}},
            "challenge": {"round": 2, "payload": b"fixture-challenge"},
            "token": {"round": 2, "status": "complete", "attributes": 0,
                      "observation": {"kind": "selected", "mechanism": "digest",
                                      "authoritative": True}, "payload": b"fixture-token"},
            "finish": {}, "finished": {},
            "error": {"kind": "protocol", "phase": "begin", "native_status": None},
        }
        for kind, body in fixtures.items():
            with self.subTest(kind=kind):
                value = envelope(kind, body)
                wire = codec.encode(SCHEMA, "Envelope", value)
                self.assertEqual(codec.decode(SCHEMA, "Envelope", wire), value)

    def test_finish_has_stable_canonical_wire_vector(self):
        wire = codec.encode(SCHEMA, "Envelope", envelope("finish", {}))
        # Map of nine keys, all inactive bodies explicit null, Finish empty map.
        self.assertEqual(wire.hex(), "a9010102050af60bf60cf60df60ea00ff610f6")

    def test_taut_refuses_unknown_enum_wrong_type_and_missing_required(self):
        cases = [{1: 1, 2: 99}, {1: True, 2: 5}, {2: 5}]
        for case in cases:
            with self.subTest(case=case):
                with self.assertRaises(codec.DecodeError):
                    codec.decode(SCHEMA, "Envelope", cbor.dumps(case))

    def test_declared_frame_and_depth_limits_are_enforced_by_reference(self):
        self.assertEqual(codec.bounds(SCHEMA, MsgRef("Envelope")), (8, 100000))
        with self.assertRaises(codec.DecodeError):
            codec.decode(SCHEMA, "Envelope", b"x" * 100001)
        nested = None
        for _ in range(12):
            nested = [nested]
        with self.assertRaises(codec.DecodeError):
            codec.decode(SCHEMA, "Envelope", cbor.dumps(nested))

    def test_reference_unknown_fields_are_not_production_profile_admission(self):
        value = codec.encode_struct(SCHEMA, "Envelope", envelope("finish", {}))
        value[99] = "synthetic-only"
        decoded = codec.decode(SCHEMA, "Envelope", cbor.dumps(value))
        self.assertIn("__unknown__", decoded)
        # Production must reject this; never mistake reference round-trip for admission.

    def test_semantic_document_changes_contract_fingerprint(self):
        ir = b"synthetic-ir"
        before = regen_schema.contract_digest(ir, b"semantics-a")
        after = regen_schema.contract_digest(ir, b"semantics-b")
        self.assertNotEqual(before, after)
        self.assertEqual(before, hashlib.sha256(b"gwz-sspi-contract-v1\0" + ir + b"semantics-a").hexdigest())

    def test_import_origin_check_rejects_shadow(self):
        class Shadow:
            __file__ = "/untrusted/taut/fake.py"
        with patch.dict(sys.modules, {"taut.fake": Shadow()}):
            with self.assertRaises(SystemExit):
                regen_schema.verify_taut_modules(PACKAGE)


if __name__ == "__main__":
    unittest.main()
