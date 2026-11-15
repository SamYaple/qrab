#!/usr/bin/env python3
"""Inspect dashboard previews and the shared BMC using isolated browser fixtures.

Requires Playwright and Chromium. No cluster, login, guest or production token.
Generate previews with `cargo run -p qrab-dashboard --example preview -- OUT_DIR`.
All BMC HTTP requests are intercepted; POSTs only mutate the in-memory fixture.
"""
import argparse
import json
import mimetypes
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--browser', default='/opt/google/chrome/chrome')
parser.add_argument('--previews', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path('target/ui-browser'))
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
args.output.mkdir(parents=True, exist_ok=True)
base = '/vms/qrab/web-1'
resets = ['On', 'Pause', 'Resume', 'ForceRestart', 'GracefulRestart', 'GracefulShutdown', 'ForceOff', 'PowerCycle', 'Nmi']


def system(power='On'):
    return {'Name': 'web-1', 'PowerState': power, 'Status': {'State': 'Enabled', 'Health': 'OK'},
            'MemorySummary': {'TotalSystemMemoryGiB': 1}, 'ProcessorSummary': {'LogicalProcessorCount': 2},
            'Actions': {'#ComputerSystem.Reset': {'target': '/redfish/v1/Systems/1/Actions/ComputerSystem.Reset', 'ResetType@Redfish.AllowableValues': resets}},
            'Oem': {'Qrab': {'GuestAgent': {'enabled': True, 'connected': True, 'version': 'qrab-guest-agent 0.1.0',
                                         'FreezeState': 'Thawed', 'LastUpdate': {'State': 'Succeeded', 'RunId': 7, 'ExitCode': 0, 'HookInstalled': True}},
                             'Configuration': {'name': 'web-1', 'memory_mib': 1024, 'cpus': 2, 'accelerator': 'kvm', 'boot': 'uefi',
                                               'tpm': True, 'console': True, 'guest_network': True, 'startup_timeout_secs': 60, 'shutdown_grace_secs': 30,
                                               'disks': [{'id': 'root', 'kind': 'iscsi', 'bus': 'virtio', 'read_only': False}],
                                               'nics': [{'id': 'eth0', 'mac': '02:5e:a1:3c:77:0b'}]}}}}


def install_bmc(context, role='admin', power='On'):
    fixture = {'system': system(power), 'posts': [], 'expired': False}
    fixture['system']['Oem']['Qrab']['Configuration']['redacted'] = role == 'viewer'

    def route(request_route):
        req = request_route.request
        path = urlparse(req.url).path
        if path.startswith('/assets/'):
            relative = path.removeprefix('/assets/')
            relative = {'bmc.js': 'console.js', 'bmc.css': 'style.css'}.get(relative, relative)
            asset = root / 'projects/bmc/assets' / relative
            if asset.is_file() and asset.resolve().is_relative_to(root / 'projects/bmc/assets'):
                request_route.fulfill(path=asset, content_type=mimetypes.guess_type(asset.name)[0] or 'application/octet-stream')
            else:
                request_route.fulfill(status=404)
            return
        if path == base + '/bmc':
            html = (root / 'projects/bmc/assets/index.html').read_text().replace('/assets/style.css', '/assets/bmc.css').replace('/assets/console.js', '/assets/bmc.js').replace('<body>', f'<body data-vm-base="{base}">')
            request_route.fulfill(body=html, content_type='text/html')
            return
        if fixture['expired']:
            request_route.fulfill(status=401, body='Sign in again.')
            return
        if path == base + '/bmc/session':
            data = {'user': {'name': 'sam', 'subject': 'fixture-user', 'csrf': 'fixture-csrf'}, 'role': role, 'vm_name': 'web-1', 'admin_bucket': []}
        elif req.method == 'POST':
            fixture['posts'].append({'path': path, 'body': req.post_data_json, 'csrf': req.headers.get('x-csrf-token')})
            request_route.fulfill(status=204)
            return
        elif path.endswith('/Systems/1'):
            data = fixture['system']
        elif path.endswith('/ResetActionInfo'):
            data = {'Parameters': [{'Name': 'ResetType', 'AllowableValues': resets}]}
        elif path.endswith('/Events/Entries'):
            data = {'Members': [{'Id': str(i), 'Created': f'2026-11-15T04:00:{i:02d}Z', 'Oem': {'Qrab': {'Sequence': i, 'Event': event, 'Data': {'reason': 'requested by user'}}}} for i, event in [(1, 'STOP'), (2, 'RESUME'), (3, 'DEVICE_DELETED')]]}
        elif path.endswith('/EventService/SSE'):
            request_route.fulfill(body=': fixture\n\n', content_type='text/event-stream')
            return
        elif path.endswith('/MetricHistory'):
            data = {'start': 1794711600, 'end': 1794715200, 'step': 60, 'unit': 'bytes/s', 'series': []}
        elif path.endswith('/MetricReports/VM'):
            data = {'MetricValues': [], 'Timestamp': '2026-11-15T04:00:00Z'}
        elif path.endswith('/Resources'):
            data = {}
        else:
            request_route.fulfill(status=404, body='Unknown fixture route: ' + path)
            return
        request_route.fulfill(json=data)

    context.route('http://ui.test/**', route)
    return fixture


def no_overflow(page, name, width):
    measured = page.evaluate('({viewport:innerWidth, content:document.documentElement.scrollWidth})')
    assert measured['content'] <= measured['viewport'], f'{name} at {width}: {measured}'


with sync_playwright() as playwright:
    browser = playwright.chromium.launch(executable_path=args.browser, headless=True, args=['--no-sandbox'])
    for theme in ['dark', 'light']:
        for width in [1440, 768, 390]:
            context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
            page = context.new_page()
            for name in ['list', 'detail', 'detail-viewer', 'images', 'launch', 'launch-os', 'launched']:
                page.goto((args.previews / f'{name}.html').resolve().as_uri())
                no_overflow(page, name, width)
                page.screenshot(path=str(args.output / f'{name}-{theme}-{width}.png'), full_page=True)
            context.close()
            for role in ['admin', 'viewer']:
                context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
                fixture = install_bmc(context, role)
                page = context.new_page()
                errors = []
                page.on('pageerror', lambda error: errors.append(str(error)))
                page.goto('http://ui.test' + base + '/bmc')
                page.locator('#dashboard').wait_for(state='visible')
                assert page.locator('#vm-detail-link').get_attribute('href') == base
                for tab in ['overview', 'metrics', 'events', 'agent', 'config']:
                    page.locator('#tab-' + tab).click()
                    no_overflow(page, f'bmc-{role}-{tab}', width)
                    page.screenshot(path=str(args.output / f'bmc-{role}-{tab}-{theme}-{width}.png'), full_page=True)
                page.locator('#tab-events').click()
                page.locator('#events-filter').fill('resume')
                assert page.locator('#events li:visible').count() == 1
                page.locator('#tab-overview').focus()
                page.keyboard.press('ArrowRight')
                assert page.locator('#tab-metrics').get_attribute('aria-selected') == 'true'
                if role == 'viewer':
                    assert page.locator('#connect').count() == 0
                    assert page.locator('[data-agent]').count() == 0
                else:
                    reset = page.locator('#power button[data-type=ForceRestart]')
                    reset.click()
                    assert len(fixture['posts']) == 0, 'First click must only arm a disruptive action'
                    reset.click()
                    page.wait_for_timeout(100)
                    assert fixture['posts'][-1]['csrf'] == 'fixture-csrf'
                    assert fixture['posts'][-1]['body'] == {'ResetType': 'ForceRestart'}
                assert not errors, errors
                context.close()
    browser.close()
print('Dashboard and BMC browser checks passed (desktop, tablet, mobile; dark and light; admin and viewer).')
