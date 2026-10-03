"""Storage-neutral taut IR -> private owned/borrowed direct Rust codec."""
import json
import re
import subprocess


def snake(name):
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()


def variant(name):
    return ''.join(part.capitalize() for part in name.split('_'))


def artifacts(ir, root, contract):
    schema = json.loads(ir)
    messages = {m['name']: m for m in schema['messages']}
    def sensitive(name):
        return any(f['type'].get('scalar') in ('str', 'bytes') or
                   (f['type']['k'] == 'msg' and sensitive(f['type']['name']))
                   for f in messages[name]['fields'])
    def ty(f, owned=False):
        t = f['type']
        if t['k'] == 'scalar':
            base = {'int': 'u64', 'bool': 'bool', 'str': 'SecretText' if owned else "&'a str",
                    'bytes': 'SecretBytes' if owned else "&'a [u8]"}[t['scalar']]
        elif t['k'] == 'enum':
            base = t['name']
        else:
            base = t['name'] + ('Owned' if owned else 'Ref')
            if not owned and sensitive(t['name']):
                base += "<'a>"
        return f'Option<{base}>' if f['optional'] else base
    outputs = {}
    header = '// Generated from protocol/sspi.ir.json by rust_projection.py; do not edit.\n'
    mod = header + 'use super::cbor::{Reader, Sink};\nuse crate::{SecretBytes, SecretText};\nuse crate::{Error as CodecError, ErrorKind as CodecErrorKind};\n'
    for e in schema['enums']:
        mod += '#[derive(Clone, Copy, PartialEq, Eq)]\npub(super) enum ' + e['name'] + ' {\n'
        for name, number in sorted(e['members'].items(), key=lambda item: item[1]):
            mod += f'{variant(name)} = {number},\n'
        mod += '}\nimpl ' + e['name'] + ' {\nfn read(r: &mut Reader<\'_>) -> Result<Self, CodecError> {\nmatch r.uint()? {\n'
        for name, number in e['members'].items():
            mod += f'{number} => Ok(Self::{variant(name)}),\n'
        mod += '_ => Err(CodecError::new(CodecErrorKind::Protocol)),\n} } }\n'
    for name in messages:
        module = snake(name)
        mod += f'mod {module};\npub(super) use {module}::{{{name}Ref, {name}Owned}};\n'
        m = messages[name]
        lifetime = "<'a>" if sensitive(name) else ''
        text = header + 'use super::*;\n'
        for owned in (False, True):
            text += f'pub(in crate::protocol) struct {name}{"Owned" if owned else "Ref"}{"" if owned else lifetime} {{\n'
            for f in m['fields']:
                text += f'pub(in crate::protocol) {f["name"]}: {ty(f, owned)},\n'
            text += '}\n'
        text += f'impl<\'a> {name}Ref{lifetime} {{\npub(in crate::protocol) fn read(r: &mut Reader<\'a>, depth: usize) -> Result<Self, CodecError> {{\nr.map({len(m["fields"])}, depth)?;\n'
        for f in m['fields']:
            t = f['type']
            expr = {'int': 'r.uint()?', 'bool': 'r.boolean()?', 'str': 'r.text()?', 'bytes': 'r.bytes()?'}[t['scalar']] if t['k'] == 'scalar' else (f'{t["name"]}::read(r)?' if t['k'] == 'enum' else f'{t["name"]}Ref::read(r, depth + 1)?')
            if f['optional']:
                expr = 'if r.null()? { None } else { Some(' + expr + ') }'
            text += f'r.key({f["tag"]})?;\nlet {f["name"]} = {expr};\n'
        text += 'Ok(Self { ' + ', '.join(f['name'] for f in m['fields']) + ' })\n}\n'
        text += 'pub(in crate::protocol) fn emit(&self, sink: &mut impl Sink) -> Result<(), CodecError> {\nsink.head(5, ' + str(len(m['fields'])) + ')?;\n'
        for f in m['fields']:
            t = f['type']; v = 'value' if f['optional'] else 'self.' + f['name']
            text += f'sink.head(0, {f["tag"]})?;\n'
            if f['optional']:
                text += f'if let Some(value) = &self.{f["name"]} {{\n'
            expr = {'int': f'sink.head(0, {"*" if f["optional"] else ""}{v})?', 'bool': f'sink.raw(&[if {"*" if f["optional"] else ""}{v} {{ 0xf5 }} else {{ 0xf4 }}])?', 'str': f'sink.string(3, ({v}).as_bytes())?', 'bytes': f'sink.string(2, {v})?'}[t['scalar']] if t['k'] == 'scalar' else (f'sink.head(0, {"*" if f["optional"] else ""}{v} as u64)?' if t['k'] == 'enum' else f'({v}).emit(sink)?')
            # Dereference is required before an enum cast / scalar dereference of a field reference.
            expr = expr.replace('*&self.', 'self.')
            text += expr + ';\n'
            if f['optional']:
                text += '} else { sink.raw(&[0xf6])?; }\n'
        text += 'Ok(())\n}\npub(in crate::protocol) fn own(&self) -> ' + name + 'Owned {\n' + name + 'Owned {\n'
        for f in m['fields']:
            t = f['type']; v = 'value' if f['optional'] else 'self.' + f['name']
            expr = (f'SecretBytes::new({v})' if t.get('scalar') == 'bytes' else f'SecretText::admitted({v})' if t.get('scalar') == 'str' else f'{v}.own()' if t['k'] == 'msg' else ('*value' if f['optional'] else v))
            if f['optional']:
                expr = f'self.{f["name"]}.as_ref().map(|value| {expr})'
            text += f'{f["name"]}: {expr},\n'
        text += '}\n}\n}\n'
        text += f'impl {name}Owned {{\npub(in crate::protocol) fn borrow(&self) -> {name}Ref{("<\'_>" if sensitive(name) else "")} {{\n{name}Ref {{\n'
        for f in m['fields']:
            t=f['type']; v='value' if f['optional'] else 'self.'+f['name']
            expr=f'{v}.as_bytes()' if t.get('scalar')=='bytes' else f'{v}.as_str()' if t.get('scalar')=='str' else f'{v}.borrow()' if t['k']=='msg' else ('*value' if f['optional'] else v)
            if f['optional']:
                expr=f'self.{f["name"]}.as_ref().map(|value| {expr})'
            text+=f'{f["name"]}: {expr},\n'
        text+='}\n}\n}\n'
        outputs[root / 'src/protocol/generated' / (module+'.rs')] = text.encode()
    mod += f'pub(super) const MAX_BODY: usize = {schema["options"]["max_encoded_len"]};\npub(super) const MAX_DEPTH: usize = {schema["options"]["max_depth"]};\n'
    mod += 'pub(super) const CONTRACT: [u8; 32] = [' + ','.join(str(b) for b in bytes.fromhex(contract)) + '];\n'
    outputs[root / 'src/protocol/generated/mod.rs'] = mod.encode()
    # Rustfmt canonicalizes generated bytes before checking/writing, never edits
    # authored schema or IR. Cargo itself never executes this generator.
    for path, source in list(outputs.items()):
        result = subprocess.run(['rustfmt', '--edition', '2024', '--config', 'skip_children=true'], input=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
        outputs[path] = result.stdout
    return outputs
