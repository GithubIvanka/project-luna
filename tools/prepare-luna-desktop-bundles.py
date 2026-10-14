#!/usr/bin/env python3
"""Package explicit musl desktop resources; never use the host dynamic loader."""
import argparse
import hashlib
import json
import os
import shlex
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

REPO = Path(__file__).resolve().parent.parent

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def command(*args):
    return subprocess.check_output([str(x) for x in args], text=True)

def resolve(root, relative):
    """Resolve guest absolute/relative links inside the selected source root."""
    todo = list(Path(relative).parts)
    parts = []
    hops = 0
    while todo:
        item = todo.pop(0)
        if item in ('/', '.', ''): continue
        if item == '..':
            if not parts: raise ValueError('source link escapes its root')
            parts.pop(); continue
        path = root.joinpath(*parts, item)
        if path.is_symlink():
            hops += 1
            if hops > 40: raise ValueError('source link cycle')
            target = Path(os.readlink(path))
            if target.is_absolute(): parts = []
            todo = list(target.parts) + todo
        else: parts.append(item)
    path = root.joinpath(*parts)
    if not path.is_file(): raise ValueError(f'missing regular source: {path}')
    return path

def files_source_hashes():
    """Закрепляем собственный код Files и его локальные зависимости клиента runtime."""
    crates = set()
    pending = [REPO/'components/external/providers/luna-files']
    paths = {REPO/'Cargo.lock'}
    while pending:
        crate = pending.pop().resolve()
        if crate in crates: continue
        crates.add(crate)
        manifest = crate/'Cargo.toml'
        paths.add(manifest)
        paths.update((crate/'src').rglob('*.rs'))
        dependencies = tomllib.loads(manifest.read_text()).get('dependencies', {})
        for dependency in dependencies.values():
            if isinstance(dependency, dict) and 'path' in dependency:
                pending.append(crate/dependency['path'])
    return {str(path.relative_to(REPO)): digest(path) for path in sorted(paths)}

def verify_native_build(receipt, runtime, ghostty, files, yazi, patch):
    if receipt.get('format') != 1:
        raise ValueError('unsupported native build receipt')
    if receipt.get('ghostty_revision') != '332b2aefc6e72d363aa93ab6ecfc86eeeeb5ed28' or receipt.get('yazi_revision') != '8dd895c695a5950330c2623eb43debf323b60654':
        raise ValueError('unexpected native source revisions')
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', receipt.get('sdk_image_id', '')):
        raise ValueError('native SDK must be an immutable image ID')
    expected = {'ghostty': ghostty/'bin/ghostty', 'luna-files': files,
        'yazi': yazi/'yazi', 'ya': yazi/'ya'}
    if any(receipt['artifacts'].get(name) != digest(path) for name, path in expected.items()):
        raise ValueError('native binary differs from its build receipt')
    if receipt['ghostty_patch_sha256'] != digest(patch):
        raise ValueError('Ghostty patch differs from its build receipt')
    if receipt['sdk_package_database_sha256'] != digest(runtime/'lib/apk/db/installed'):
        raise ValueError('runtime SDK differs from the native build SDK')
    if receipt.get('luna_files_sources') != files_source_hashes():
        raise ValueError('Luna Files sources differ from the native build receipt')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', required=True, type=Path)
    parser.add_argument('--runtime-root', required=True, type=Path)
    parser.add_argument('--ghostty-prefix', required=True, type=Path)
    parser.add_argument('--luna-files', required=True, type=Path)
    parser.add_argument('--yazi-prefix', required=True, type=Path)
    parser.add_argument('--cli', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--keep-payload', action='store_true', help='retain temporary source payloads for diagnostics')
    args = parser.parse_args()
    inputs = args.inputs.resolve(); runtime = args.runtime_root.resolve()
    ghostty = args.ghostty_prefix.resolve(); cli = args.cli.resolve()
    receipt = json.loads((ghostty.parent/'build-provenance.json').read_text())
    verify_native_build(receipt, runtime, ghostty, args.luna_files.resolve(), args.yazi_prefix.resolve(),
        REPO/'tools/alpine-foundation/ghostty-wayland-app-id.patch')
    subprocess.run(['python3', str(REPO/'tools/canonical-luna-inputs.py'), 'verify', str(inputs)], check=True)
    output = args.output.absolute()
    if output.exists() or output.is_symlink(): raise ValueError('output already exists')
    output.mkdir(parents=True)
    mesa = inputs/'graphics/mesa/out'
    provenance = {'format': 1, 'base_input_lock_sha256': digest(inputs/'inputs.lock.json'),
        'yazi_revision': '8dd895c695a5950330c2623eb43debf323b60654',
        'ghostty_revision': '332b2aefc6e72d363aa93ab6ecfc86eeeeb5ed28',
        'native_build': receipt,
        'sdk_package_database_sha256': digest(runtime/'lib/apk/db/installed'), 'applications': {}}
    policies = []; applications = []; payloads = []
    try:
        for name, identity, version, binary in [
            ('luna-files', 'org.luna.files', '0.1.0', args.luna_files.resolve()),
            ('terminal', 'org.luna.terminal', '1.3.1', ghostty/'bin/ghostty')]:
            work = output/(name+'-source'); payload = work/'payload'; payload.mkdir(parents=True)
            payloads.append(payload)
            resources = {}; mappings = {}; queue = []; seen = set()
            def copy(source, destination, elf=False):
                target = payload/destination; target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source, target); target.chmod(0o755 if elf else 0o644)
                resources[destination] = {'source': str(source), 'source_sha256': digest(source), 'payload_sha256': digest(target)}
                if elf:
                    info = command('readelf', '-dW', target)
                    if re.search(r'\((?:RPATH|RUNPATH)\)', info):
                        subprocess.run([*shlex.split(os.environ.get('LUNA_PATCHELF_COMMAND', 'patchelf')), '--remove-rpath', str(target)], check=True)
                        resources[destination]['removed_search_path'] = True
                        resources[destination]['payload_sha256'] = digest(target)
                    queue.append(target)
            def mapping(logical, source, access): mappings[logical] = {'logical': logical, 'source': source, 'access': access}
            def tree(root, relative, destination, logical):
                source = root/relative
                if not source.is_dir(): raise ValueError(f'missing required resource tree: {relative}')
                for directory, dirs, files in os.walk(source, followlinks=False):
                    # Directory aliases are not silently traversed into another resource class.
                    dirs[:] = [d for d in dirs if not (Path(directory)/d).is_symlink()]
                    for file in sorted(files):
                        path = Path(directory)/file
                        suffix = path.relative_to(source)
                        copy(resolve(root, path.relative_to(root)), str(Path(destination)/suffix))
                mapping(logical, destination, ['read'])
            executable = 'bin/luna-files' if name == 'luna-files' else 'bin/ghostty'
            copy(binary, executable, True); mapping('/'+executable, executable, ['read', 'execute'])
            capabilities = ['desktop.wayland', 'user.home']
            if name == 'luna-files': capabilities.append('application.open')
            if name == 'terminal':
                capabilities.append('terminal.pty')
                for program in ['yazi', 'ya']:
                    copy(args.yazi_prefix.resolve()/program, 'bin/'+program, True)
                    mapping('/bin/'+program, 'bin/'+program, ['read','execute'])
                for logical, source in [('/bin/fish', 'usr/bin/fish'), ('/bin/bash', 'bin/bash')]:
                    copy(resolve(runtime, source), logical[1:], True); mapping(logical, logical[1:], ['read','execute'])
                # Regular copies satisfy Bundle's no-symlink payload contract.
                copy(resolve(runtime,'bin/busybox'), 'bin/busybox', True)
                mapping('/bin/busybox', 'bin/busybox', ['read','execute'])
                for applet in ['sh','cat','ls','mkdir','rmdir','pwd','echo','printf','touch','rm','cp','mv','env','head','tail','grep','sed','sleep','true','false','uname','id','wc','sort','find','date','test','seq','readlink','basename','dirname','stty','less']:
                    mapping('/bin/'+applet, 'bin/busybox', ['read','execute'])
                tree(runtime,'usr/share/fish','share/fish','/usr/share/fish')
                tree(ghostty,'share/ghostty','share/ghostty','/usr/share/ghostty')
                tree(ghostty,'share/terminfo','share/terminfo','/usr/share/terminfo')
                # The selected canonical Mesa was built with llvmpipe. No host GPU devices.
                driver = resolve(mesa, 'usr/local/lib/libgallium-26.1.6.so')
                copy(driver,'lib/libgallium-26.1.6.so',True)
                mapping('/lib/libgallium-26.1.6.so','lib/libgallium-26.1.6.so',['read','execute'])
                # EGL dlopens the DRI dispatch shim; it is not in DT_NEEDED.
                copy(resolve(mesa,'usr/lib/dri/libdril_dri.so'),'lib/dri/libdril_dri.so',True)
                for alias in ['libdril_dri.so','swrast_dri.so','kms_swrast_dri.so']:
                    mapping('/usr/lib/dri/'+alias,'lib/dri/libdril_dri.so',['read','execute'])
                for library in ['libEGL.so.1', 'libGL.so.1', 'libgbm.so.1']:
                    copy(resolve(mesa, 'usr/local/lib/'+library), 'lib/'+library, True)
                    mapping('/lib/'+library, 'lib/'+library, ['read','execute'])
                config = payload/'config/ghostty'; config.parent.mkdir(exist_ok=True)
                config.write_bytes((REPO/'resources/alpha/ghostty.conf').read_bytes())
                resources['config/ghostty']={'generated': True, 'payload_sha256': digest(config)}
                mapping('/etc/ghostty/config','config/ghostty',['read'])
            tree(runtime,'etc/fonts','config/fonts','/etc/fonts')
            # GDK creates a fallback XKB keymap before the compositor keymap arrives.
            # These data files (and Compose tables) are runtime resources, not ELF
            # DT_NEEDED entries; omitting them can leave GDK with a null keymap.
            tree(runtime,'usr/share/X11/xkb','share/X11/xkb','/usr/share/X11/xkb')
            # Current libxkbcommon searches this canonical resource path; Alpine
            # supplies it as a symlink. Map the same regular payload subtree twice.
            mapping('/usr/share/xkeyboard-config-2','share/X11/xkb',['read'])
            tree(runtime,'usr/share/X11/locale','share/X11/locale','/usr/share/X11/locale')
            font = 'usr/share/fonts/truetype/luna/JetBrainsMonoNerdFontMono-Regular.ttf'
            copy(resolve(inputs/'fonts', font), 'share/fonts/JetBrainsMonoNerdFontMono-Regular.ttf')
            # GUI использует пропорциональный шрифт из закреплённого поставщика;
            # Ghostty сохраняет явно выбранный Nerd Mono.
            copy(resolve(inputs/'provider-root', 'usr/share/fonts/Adwaita/AdwaitaSans-Regular.ttf'),
                 'share/fonts/AdwaitaSans-Regular.ttf')
            mapping('/usr/share/fonts','share/fonts',['read'])
            if (runtime/'usr/share/icons/Adwaita').is_dir(): tree(runtime,'usr/share/icons/Adwaita','share/icons/Adwaita','/usr/share/icons/Adwaita')
            if (runtime/'usr/share/mime').is_dir(): tree(runtime,'usr/share/mime','share/mime','/usr/share/mime')
            copy(resolve(runtime,'lib/ld-musl-x86_64.so.1'),'lib/ld-musl-x86_64.so.1',True)
            mapping('/lib/ld-musl-x86_64.so.1','lib/ld-musl-x86_64.so.1',['read','execute'])
            while queue:
                selected = queue.pop(0)
                for library in re.findall(r'\(NEEDED\).*?\[([^]]+)\]', command('readelf','-dW',selected)):
                    if library in seen: continue
                    seen.add(library)
                    if '/' in library: raise ValueError('noncanonical ELF library name')
                    source = None
                    sources = [(runtime,'lib'), (runtime,'usr/lib')]
                    if name == 'terminal': sources.insert(0, (mesa,'usr/local/lib'))
                    for root, directory in sources:
                        if (root/directory/library).exists() or (root/directory/library).is_symlink():
                            source=resolve(root, str(Path(directory)/library)); break
                    if source is None: raise ValueError(f'unresolved selected dependency: {library}')
                    copy(source,'lib/'+library,True); mapping('/lib/'+library,'lib/'+library,['read','execute'])
            manifest = f'''format = 1
    [bundle]
    id = "{identity}"
    name = "{'Luna Files' if name == 'luna-files' else 'Terminal'}"
    version = "{version}"
    type = "application"
    [platform]
    arch = "x86_64"
    [entry]
    exec = "{executable}"
    logical = "/{executable}"
    args = {json.dumps(['--config-file=/etc/ghostty/config'] if name == 'terminal' else [])}
    {('[entry.file_open]'+chr(10)+'mime_types = ["text/plain"]'+chr(10)+'args = ["--working-directory=/tmp", "-e", "/bin/less"]') if name == 'terminal' else ''}
    [capabilities]
    requested = {json.dumps(capabilities)}
    '''
            for item in mappings.values():
                manifest += '\n[[mapping]]\n'+''.join(f'{key} = {json.dumps(value)}\n' for key,value in item.items())
            (work/'manifest.toml').write_text(manifest)
            archive = output/(name+'.lbp'); command(cli,'bundle','pack',work/'manifest.toml',payload,archive)
            content = re.search(r'content_identity=([a-f0-9]{64})',command(cli,'bundle','inspect',archive)).group(1)
            grant = f'[[application]]\nuser = "luna"\nid = "{identity}"\nversion = "{version}"\ncontent_identity = "{content}"\nruntime = "musl"\nexecute = true\nprivate_data = true\ncapabilities = {json.dumps(capabilities + (['user.selected-file'] if name == 'terminal' else []))}\n'
            for item in [*mappings.values(), {'logical':'/home','access':['read','write','execute']}, {'logical':'/data','access':['read','write']}, {'logical':'/config','access':['read','write']}, {'logical':'/run/wayland-0','access':['read']},
                *([{'logical':'/run/luna/application.sock','access':['read']}] if name == 'luna-files' else [{'logical':'/open/selected','access':['read']}])]:
                grant += f'\n[[application.path]]\nlogical = {json.dumps(item["logical"])}\naccess = {json.dumps(item["access"])}\n'
            policies.append(grant)
            applications.append({'name':name,'id':identity,'version':version,'content_identity':content,'archive_sha256':digest(archive)})
            provenance['applications'][name] = resources
            print(f'{name}: {len(resources)} regular payload files, registered identity {content}',flush=True)
        (output/'applications.json').write_text(json.dumps(applications,indent=2)+'\n')
        (output/'application-launch.toml').write_text('format = 1\n'+'\n'.join(policies))
        (output/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
    finally:
        if not args.keep_payload:
            for payload in payloads:
                if payload.exists(): shutil.rmtree(payload)
            print("Temporary packaging payloads removed", flush=True)

if __name__ == '__main__': main()
