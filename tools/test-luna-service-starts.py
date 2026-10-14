#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('services', Path(__file__).with_name('check-luna-service-starts.py'))
services = importlib.util.module_from_spec(spec)
spec.loader.exec_module(services)

class ServiceStarts(unittest.TestCase):
    def test_plain_success(self):
        services.verify('* Starting Bluetooth ... [ ok ]\n', ['Bluetooth'])
    def test_interleaved_kernel_output(self):
        services.verify('* Starting Bluetooth ...[4.1] elogind[293]: Watching buttons\n [ ok ]\n', ['Bluetooth'])
    def test_missing_success_cannot_borrow_the_next_result(self):
        with self.assertRaises(ValueError): services.verify('* Starting Bluetooth ...\n* Starting UPower ... [ ok ]', ['Bluetooth'])
    def test_failure_cannot_borrow_later_ok(self):
        with self.assertRaises(ValueError): services.verify('* Starting Bluetooth ... [ !! ]\n[ ok ]', ['Bluetooth'])
    def test_missing_start(self):
        with self.assertRaises(ValueError): services.verify('[ ok ]', ['Bluetooth'])

if __name__ == '__main__': unittest.main()
