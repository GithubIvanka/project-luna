#!/usr/bin/env python3
"""Seed manager-registered Alpha applications from a verified canonical snapshot."""
import argparse, hashlib, json, re, subprocess
from pathlib import Path

def digest(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream, 'sha256').hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--data', required=True, type=Path)
    parser.add_argument('--cli', required=True, type=Path)
    args=parser.parse_args()
    source=args.source.resolve(); data=args.data.resolve(); cli=args.cli.resolve()
    policy=data/'system/config/application-launch.toml'
    launcher=data/'system/apps/luna-desktop-launchers'
    if policy.exists() or policy.is_symlink() or launcher.exists() or launcher.is_symlink():
        raise ValueError('desktop seed requires a fresh build root')
    apps=json.loads((source/'applications.json').read_text())
    if [(a['name'],a['id'],a['version']) for a in apps] != [('luna-files','org.luna.files','0.1.0'),('terminal','org.luna.terminal','1.3.1')]:
        raise ValueError('unexpected Alpha application set')
    for app in apps:
        archive=source/(app['name']+'.lbp')
        if archive.is_symlink() or digest(archive)!=app['archive_sha256']: raise ValueError('application archive changed')
        inspection=subprocess.check_output([str(cli),'bundle','inspect',str(archive)],text=True)
        if f'content_identity={app["content_identity"]}' not in inspection: raise ValueError('application content identity changed')
        subprocess.run([str(cli),'app','--offline-data-root',str(data),'install',str(archive),'--trust-content',app['content_identity']],check=True)
    policy.write_bytes((source/'application-launch.toml').read_bytes()); policy.chmod(0o600)
    launcher.mkdir()
    for app in apps:
        file='luna-files' if app['name']=='luna-files' else 'luna-terminal'
        wrapper=launcher/file
        wrapper.write_text(f'#!/bin/sh\nexec /usr/bin/luna app run {app["id"]} {app["version"]} --expect-content {app["content_identity"]} -- "$@"\n')
        wrapper.chmod(0o755)
    print('Registered Luna Files + Ghostty/fish Alpha Bundles: PASS')

if __name__=='__main__': main()
