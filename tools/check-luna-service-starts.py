#!/usr/bin/env python3
"""Read sequential OpenRC start results despite interleaved kernel/elogind logs."""
import re
import sys
from pathlib import Path

SERVICES = ('seatd', 'System Message Bus', 'System login manager', 'Bluetooth',
            'NetworkManager', 'Polkit System Daemon', 'RealtimeKit', 'UPower')

def verify(text, services=SERVICES):
    for service in services:
        start = re.search(r'\* Starting ' + re.escape(service) + r' \.\.\.', text)
        if not start:
            raise ValueError(f'missing service start: {service}')
        remainder = text[start.end():]
        following = re.search(r'\* Starting ', remainder)
        segment = remainder[:following.start()] if following else remainder
        result = re.search(r'\[ (ok|!!) \]', segment[:4096])
        if not result or result.group(1) != 'ok':
            raise ValueError(f'service start did not succeed: {service}')

if __name__ == '__main__':
    verify(Path(sys.argv[1]).read_text(errors='replace'))
    print('Canonical OpenRC service starts: PASS')
