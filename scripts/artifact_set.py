"""Build-only trusted artifact-set producer; stdlib, no Git or runtime lookup.

Cargo resolution supplies package roots, including extracted/registry sources.
The identifier covers first-party sources/contracts, resolved lock, target,
profile/features/options and compiler. It is not a hash of the finished binary.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess

INPUTS=('Cargo.toml','Cargo.lock','build.rs','lib.rs','.cargo/config','.cargo/config.toml','libgit2','src','native','build_support','protocol','pyproject.toml','dist-workspace.toml','scripts/artifact_set.py','scripts/build_sspi.py','docs/WireProtocol.md')

def fingerprint(packages, options):
    identities=[(p['name'],p['version']) for p in packages]
    if len(identities)!=len(set(identities)):raise RuntimeError('ambiguous package source identity')
    digest=hashlib.sha256(b'gwz-sspi-artifact-set-v1\0')
    def add(key,data):
        for value in (key.encode(),data):
            digest.update(len(value).to_bytes(8,'big'));digest.update(value)
    add('build-options',json.dumps(options,sort_keys=True,separators=(',',':')).encode())
    for package in sorted(packages,key=lambda p:(p['name'],p['version'])):
        root=Path(package['manifest_path']).parent
        names=set(INPUTS)
        for target in package.get('targets',[]):
            source=Path(target['src_path'])
            if not source.is_relative_to(root):raise RuntimeError('target source outside package')
            names.add(source.relative_to(root).parts[0])
        for name in sorted(names):
            path=root/name
            files=sorted(p for p in path.rglob('*') if p.is_file()) if path.is_dir() else ([path] if path.is_file() else [])
            for file in files:
                relative=file.relative_to(root)
                if any(part in ('__pycache__','target','.git') for part in relative.parts):
                    continue
                add(package['name']+'@'+package['version']+'/'+relative.as_posix(),file.read_bytes())
    return digest.hexdigest()

def resolve(manifest, *, features=(), offline=False):
    args=['cargo','metadata','--locked','--format-version','1','--manifest-path',str(manifest)]
    if features: args+=['--features',','.join(features)]
    if offline:args+=['--offline']
    metadata=json.loads(subprocess.check_output(args))
    root=next(p for p in metadata['packages'] if Path(p['manifest_path']).resolve()==Path(manifest).resolve())
    nodes={n['id']:n for n in metadata['resolve']['nodes']};pending=[root['id']];seen=set()
    while pending:
        key=pending.pop()
        if key not in seen:
            seen.add(key);pending.extend(n['pkg'] for n in nodes[key]['deps'])
    packages=[p for p in metadata['packages'] if p['id'] in seen and (p.get('source') is None or p['name']=='gwz' or p['name'].startswith('gwz-'))]
    workers=[p for p in packages if p['name']=='gwz-sspi']
    if len(workers)!=1:raise RuntimeError('ambiguous or absent SSPI dependency')
    return metadata,packages,Path(workers[0]['manifest_path'])

def identify(manifest, *, target, profile, features=(), options=None):
    unsupported=('RUSTC','CARGO_BUILD_RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CARGO_BUILD_RUSTC_WRAPPER','CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER')
    if any(os.environ.get(key) for key in unsupported):raise RuntimeError('unsupported compiler override')
    offline=any(flag in (options or {}).get('maturin_args',[]) for flag in ('--offline','--frozen'))
    metadata,packages,worker=resolve(manifest,features=features,offline=offline)
    compiler=subprocess.check_output(['rustc','-vV'],text=True)
    lock=Path(metadata['workspace_root'])/'Cargo.lock'
    configuration={}
    directories=[Path(metadata['workspace_root'])/'.cargo',*[path/'.cargo' for path in [Path.cwd(),*Path.cwd().parents]],Path(os.environ.get('CARGO_HOME',Path.home()/'.cargo'))]
    paths=list(dict.fromkeys(path/name for path in directories for name in ('config','config.toml')))
    for path in paths:
        if path.is_file():
            import re
            # Conservative stdlib-only refusal, including quoted keys; no
            # Python 3.11 parser dependency in supported 3.10 build tooling.
            if re.search(r'^\s*[^#\n=]*\brustc(?:-wrapper|-workspace-wrapper)?[\"\'\s]*=',path.read_text(),re.MULTILINE):raise RuntimeError('unsupported configured compiler override')
            configuration['config-'+str(len(configuration))]=hashlib.sha256(path.read_bytes()).hexdigest()
    environment={key:value for key,value in os.environ.items() if key.startswith(('CARGO_PROFILE_','CARGO_TARGET_')) and key!='CARGO_TARGET_DIR'}
    for key in ('RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CC','CFLAGS','CXX','CXXFLAGS','AR','RANLIB','MACOSX_DEPLOYMENT_TARGET','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_TARGET'):
        if key in os.environ:environment[key]=os.environ[key]
    inputs={'target':target,'profile':profile,'features':sorted(features),'compiler':compiler,
            'rustflags':{key:os.environ[key] for key in ('CARGO_ENCODED_RUSTFLAGS','RUSTFLAGS','CARGO_BUILD_RUSTFLAGS') if key in os.environ},
            'lock':hashlib.sha256(lock.read_bytes()).hexdigest(),
            'workspace_manifest':hashlib.sha256((Path(metadata['workspace_root'])/'Cargo.toml').read_bytes()).hexdigest(),
            'cargo_configuration':configuration,'build_environment':environment,'options':options or {}}
    return fingerprint(packages,inputs),worker,inputs

def receipt(identifier,inputs):
    return (json.dumps({'format':'gwz-sspi-artifact-set-v1','build_fingerprint':identifier,'inputs':inputs},sort_keys=True,indent=2)+'\n').encode()

def bundle(wheel, worker, identifier, inputs):
    """Add executable+receipt beside extension, rebuilding standard wheel RECORD."""
    import base64,csv,io,stat,zipfile
    wheel=Path(wheel)
    with zipfile.ZipFile(wheel) as source:
        records={info.filename:(info,source.read(info)) for info in source.infolist()}
    modules=[name for name in records if name.startswith('gwz/_gwz_core.') and name.endswith(('.pyd','.so'))]
    if len(modules)!=1: raise RuntimeError('wheel must contain exactly one native extension')
    record=next(name for name in records if name.endswith('.dist-info/RECORD'))
    for name,data in [('gwz/'+Path(worker).name,Path(worker).read_bytes()),('gwz/sspi-artifact-set.json',receipt(identifier,inputs))]:
        if name in records: raise RuntimeError('worker already present in wheel')
        info=zipfile.ZipInfo(name);info.external_attr=(stat.S_IFREG|0o755)<<16;records[name]=(info,data)
    rows=[]
    for name,(_,data) in sorted(records.items()):
        if name!=record:
            encoded=base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b'=').decode()
            rows.append([name,'sha256='+encoded,str(len(data))])
    rows.append([record,'','']);stream=io.StringIO();csv.writer(stream).writerows(rows)
    records[record]=(records[record][0],stream.getvalue().encode())
    import tempfile
    descriptor,name=tempfile.mkstemp(prefix='.gwz-bundle-',suffix='.tmp',dir=wheel.parent);os.close(descriptor)
    temporary=Path(name)
    try:
        with zipfile.ZipFile(temporary,'w',compression=zipfile.ZIP_DEFLATED) as output:
            for _,(info,data) in sorted(records.items()):output.writestr(info,data)
        validate_wheel(temporary,identifier)
        temporary.replace(wheel)
    finally:
        temporary.unlink(missing_ok=True)


def validate_wheel(wheel,identifier):
    """Validate complete ZIP/RECORD and provisioned receipt before publication."""
    import base64,csv,io,zipfile
    with zipfile.ZipFile(wheel) as archive:
        names=archive.namelist()
        if len(names)!=len(set(names)) or any(Path(name).is_absolute() or '..' in Path(name).parts for name in names):raise RuntimeError('invalid wheel members')
        records=[name for name in names if name.endswith('.dist-info/RECORD')]
        if len(records)!=1:raise RuntimeError('invalid wheel RECORD')
        record=records[0];rows=list(csv.reader(io.StringIO(archive.read(record).decode())))
        if len(rows)!=len(names) or any(len(row)!=3 for row in rows) or {row[0] for row in rows}!=set(names):raise RuntimeError('incomplete wheel RECORD')
        for name,digest,size in rows:
            data=archive.read(name)
            if name==record:
                if digest or size:raise RuntimeError('invalid self RECORD')
            elif digest!='sha256='+base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b'=').decode() or size!=str(len(data)):raise RuntimeError('invalid wheel digest')
        if json.loads(archive.read('gwz/sspi-artifact-set.json'))['build_fingerprint']!=identifier:raise RuntimeError('wheel fingerprint mismatch')
        workers=[name for name in names if name in ('gwz/gwz-sspi-worker','gwz/gwz-sspi-worker.exe')]
        if len(workers)!=1:raise RuntimeError('missing packaged worker')
