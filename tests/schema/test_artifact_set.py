import importlib.util
from pathlib import Path
import tempfile
import unittest
SPEC=importlib.util.spec_from_file_location('artifact_set',Path(__file__).resolve().parents[2]/'scripts/artifact_set.py')
MODULE=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
class ArtifactSetTests(unittest.TestCase):
    def test_relevant_sources_contract_options_and_relocation(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            packages=[]
            for name in ['gwz','gwz-core','gwz-py','gwz-sspi']:
                path=root/name; (path/'src').mkdir(parents=True)
                (path/'Cargo.toml').write_text('[package]\nname="'+name+'"\n')
                (path/'src/lib.rs').write_text('source')
                packages.append({'name':name,'version':'1','manifest_path':str(path/'Cargo.toml')})
            contract=root/'gwz-sspi/protocol';contract.mkdir();(contract/'contract.json').write_text('contract')
            options={'target':'host','profile':'release','features':[],'rustflags':''}
            original=MODULE.fingerprint(packages,options)
            for file in [root/'gwz-core/src/lib.rs',root/'gwz-sspi/protocol/contract.json',root/'gwz-py/src/lib.rs']:
                before=file.read_bytes();file.write_bytes(before+b'changed')
                self.assertNotEqual(original,MODULE.fingerprint(packages,options));file.write_bytes(before)
            self.assertNotEqual(original,MODULE.fingerprint(packages,{**options,'target':'another'}))
            import shutil
            copied=root/'relocated';shutil.copytree(root/'gwz-sspi',copied)
            moved=[{**p,'manifest_path':str(copied/'Cargo.toml')} if p['name']=='gwz-sspi' else p for p in packages]
            self.assertEqual(original,MODULE.fingerprint(moved,options))
    def test_local_fork_root_and_native_vendor_are_fingerprint_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'Cargo.toml').write_text('[package]')
            (root/'lib.rs').write_text('root source');(root/'libgit2').mkdir();(root/'libgit2/native.c').write_text('native source')
            packages=[{'name':'gwz-libgit2-sys','version':'1','manifest_path':str(root/'Cargo.toml')}]
            original=MODULE.fingerprint(packages,{})
            for file in [root/'lib.rs',root/'libgit2/native.c']:
                before=file.read_bytes();file.write_bytes(before+b'changed')
                self.assertNotEqual(original,MODULE.fingerprint(packages,{}));file.write_bytes(before)
    def test_compiler_overrides_refuse_before_identification_and_config_needs_no_tomllib(self):
        from unittest import mock
        with mock.patch.dict(MODULE.os.environ,{'RUSTC':'alternate'}):
            with self.assertRaisesRegex(RuntimeError,'compiler override'):
                MODULE.identify(Path('/unused'),target='host',profile='dev')
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'Cargo.toml').write_text('[package]');(root/'Cargo.lock').write_text('lock')
            config=root/'.cargo';config.mkdir();(config/'config.toml').write_text('[build]\nrustc = "alternate"\n')
            with mock.patch.object(MODULE,'resolve',return_value=({'workspace_root':str(root)},[],root/'Cargo.toml')),mock.patch.object(MODULE.subprocess,'check_output',return_value='compiler'):
                with self.assertRaisesRegex(RuntimeError,'configured compiler'):
                    MODULE.identify(root/'Cargo.toml',target='host',profile='dev')
                (config/'config.toml').write_text('[profile.release]\nlto = "thin"\n')
                _,_,inputs=MODULE.identify(root/'Cargo.toml',target='host',profile='dev')
                self.assertTrue(inputs['cargo_configuration'])
    def test_every_conventional_rustflags_channel_distinguishes_inputs(self):
        from unittest import mock
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'Cargo.toml').write_text('[package]');(root/'Cargo.lock').write_text('lock')
            packages=[{'name':'gwz-sspi','version':'1','manifest_path':str(root/'Cargo.toml')}]
            def identify(environment):
                with mock.patch.dict(MODULE.os.environ,environment,clear=True),mock.patch.object(MODULE,'resolve',return_value=({'workspace_root':str(root)},packages,root/'Cargo.toml')),mock.patch.object(MODULE.subprocess,'check_output',return_value='fixed compiler'):
                    return MODULE.identify(root/'Cargo.toml',target='host',profile='release')[0]
            baseline=identify({})
            for flags in ['-C target-cpu=generic','-C target-cpu=native']:
                self.assertNotEqual(baseline,identify({'CARGO_BUILD_RUSTFLAGS':flags}))
            for higher in [{},{'RUSTFLAGS':'higher'},{'CARGO_ENCODED_RUSTFLAGS':'highest'},{'CARGO_TARGET_HOST_RUSTFLAGS':'target'}]:
                self.assertNotEqual(identify({**higher,'CARGO_BUILD_RUSTFLAGS':'one'}),identify({**higher,'CARGO_BUILD_RUSTFLAGS':'two'}))
    def test_wheel_executable_receipt_and_every_record_hash_are_installed_together(self):
        import base64,csv,hashlib,io,json,zipfile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);wheel=root/'fixture.whl';worker=root/'gwz-sspi-worker'
            worker.write_bytes(b'synthetic-executable')
            with zipfile.ZipFile(wheel,'w') as archive:
                archive.writestr('gwz/_gwz_core.abi3.so',b'synthetic-extension')
                archive.writestr('gwz-1.dist-info/RECORD',b'')
            MODULE.bundle(wheel,worker,'42'*32,{'target':'synthetic'})
            with zipfile.ZipFile(wheel) as archive:
                self.assertEqual(archive.read('gwz/gwz-sspi-worker'),worker.read_bytes())
                self.assertEqual(json.loads(archive.read('gwz/sspi-artifact-set.json'))['build_fingerprint'],'42'*32)
                self.assertEqual((archive.getinfo('gwz/gwz-sspi-worker').external_attr>>16)&0o777,0o755)
                for name,encoded,size in csv.reader(io.StringIO(archive.read('gwz-1.dist-info/RECORD').decode())):
                    if encoded:
                        data=archive.read(name)
                        self.assertEqual(encoded,'sha256='+base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b'=').decode())
                        self.assertEqual(size,str(len(data)))
            with self.assertRaises(RuntimeError):MODULE.bundle(wheel,worker,'00'*32,{})
