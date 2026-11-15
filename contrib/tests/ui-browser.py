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
from urllib.parse import parse_qs, urlparse
from playwright.sync_api import expect, sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--browser', default='/opt/google/chrome/chrome')
parser.add_argument('--previews', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path('target/ui-browser'))
parser.add_argument('--states-only', action='store_true', help='Run only BMC loading, failure, pending-request and expiry checks.')
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
    fixture = {'system': system(power), 'posts': [], 'expired': False,
               'hold_posts': False, 'pending_posts': [], 'post_error': None,
               'fail_system': None, 'fail_metrics': False, 'hold_session': False, 'pending_session': [], 'empty_events': False, 'requests': []}
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
        fixture['requests'].append((req.method, path))
        if fixture['expired']:
            request_route.fulfill(status=401, body='Sign in again.')
            return
        if fixture['fail_metrics'] and (path.endswith('/Resources') or path.endswith('/MetricReports/VM')):
            request_route.fulfill(status=503, json={'error': {'message': 'Measurements unavailable.'}})
            return
        if path == base + '/bmc/session':
            data = {'user': {'name': 'sam', 'subject': 'fixture-user', 'csrf': 'fixture-csrf'}, 'role': role, 'vm_name': 'web-1', 'admin_bucket': []}
            if fixture['hold_session']:
                fixture['pending_session'].append((request_route, data))
                return
        elif req.method == 'POST':
            fixture['posts'].append({'path': path, 'body': req.post_data_json if req.post_data else None, 'csrf': req.headers.get('x-csrf-token')})
            if fixture['hold_posts']:
                fixture['pending_posts'].append(request_route)
                return
            if fixture['post_error']:
                request_route.fulfill(status=403, json={'error': {'message': fixture['post_error']}})
                return
            request_route.fulfill(status=204)
            return
        elif path.endswith('/Systems/1'):
            if fixture['fail_system']:
                error = fixture['fail_system']
                request_route.fulfill(status=503, **error)
                return
            data = fixture['system']
        elif path.endswith('/ResetActionInfo'):
            data = {'Parameters': [{'Name': 'ResetType', 'AllowableValues': resets}]}
        elif path.endswith('/Events/Entries'):
            data = {'Members': [] if fixture['empty_events'] else [{'Id': str(i), 'Created': f'2026-11-15T04:00:{i:02d}Z', 'Oem': {'Qrab': {'Sequence': i, 'Event': event, 'Data': {'reason': 'requested by user'}}}} for i, event in [(1, 'STOP'), (2, 'RESUME'), (3, 'DEVICE_DELETED')]]}
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


def bmc_state_checks(browser, width, theme):
    context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
    fixture = install_bmc(context)
    fixture['hold_session'] = True
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.clock.install()
    page.goto('http://ui.test' + base + '/bmc', wait_until='domcontentloaded')
    expect(page.locator('#loading')).to_be_visible()
    page.screenshot(path=str(args.output / f'bmc-loading-{theme}-{width}.png'), full_page=True)
    page.wait_for_timeout(100)
    assert len(fixture['pending_session']) == 1
    request_route, data = fixture['pending_session'].pop()
    fixture['hold_session'] = False
    request_route.fulfill(json=data)
    pause = page.locator('#power button[data-type=Pause]')
    expect(pause).to_be_enabled()
    expect(page.locator('#loading')).to_be_hidden()

    # Polls must never unlock a pending action, and synthetic duplicate events
    # must not send another POST even if a caller bypasses the disabled DOM.
    fixture['hold_posts'] = True
    pause.click()
    expect(pause).to_have_attribute('aria-busy', 'true')
    page.clock.fast_forward(3100)
    assert page.locator('#power button:enabled, [data-agent]:enabled').count() == 0
    pause.dispatch_event('click')
    assert len(fixture['posts']) == 1
    page.screenshot(path=str(args.output / f'bmc-pending-{theme}-{width}.png'), full_page=True)
    fixture['system']['PowerState'] = 'Paused'
    fixture['hold_posts'] = False
    fixture['pending_posts'].pop().fulfill(status=204)
    expect(page.locator('#power button[data-type=Resume]')).to_be_enabled()
    expect(page.locator('#action-status')).to_contain_text('request accepted')

    fixture['fail_system'] = {'json': {'error': {'message': 'The VM has no running Pod.'}}}
    page.clock.fast_forward(3100)
    expect(page.locator('#read-status')).to_contain_text('The VM has no running Pod.')
    expect(page.locator('#read-status')).to_contain_text('last updated')
    assert page.locator('#power button:enabled, [data-agent]:enabled').count() == 0
    expect(page.locator('#connect')).to_be_disabled()
    no_overflow(page, 'bmc-stale', width)
    page.screenshot(path=str(args.output / f'bmc-stale-{theme}-{width}.png'), full_page=True)
    fixture['fail_system'] = {'body': '<!doctype html><html><body>Gateway unavailable</body></html>', 'content_type': 'text/html'}
    page.clock.fast_forward(3100)
    expect(page.locator('#read-status')).to_contain_text('VM access is unavailable (503)')
    assert '<!doctype' not in page.locator('#read-status').inner_text()
    fixture['fail_system'] = None
    page.clock.fast_forward(3100)
    expect(page.locator('#read-status')).to_be_hidden()
    resume = page.locator('#power button[data-type=Resume]')
    expect(resume).to_be_enabled()
    fixture['fail_metrics'] = True
    page.locator('#tab-metrics').click()
    expect(page.locator('#metrics-note')).to_have_text('Unavailable')
    fixture['fail_metrics'] = False
    page.clock.fast_forward(5100)
    expect(page.locator('#telemetry')).to_contain_text('No metrics have been reported yet.')
    expect(page.locator('#resources dd')).to_have_text(['unavailable', 'unavailable'])
    expect(page.locator('#metrics-note')).to_contain_text('Sampled')
    fixture['post_error'] = 'Permission denied by the VM.'
    resume.click()
    expect(page.locator('#error')).to_contain_text('Resume: Permission denied by the VM.')
    expect(page.locator('#action-status')).to_be_hidden()
    expect(resume).to_be_enabled()

    page.locator('#tab-events').click()
    page.locator('#events-filter').fill('not-a-recorded-event')
    expect(page.locator('#events-empty')).to_contain_text('No events match')
    fixture['expired'] = True
    page.clock.fast_forward(3100)
    expect(page.locator('#login')).to_be_visible()
    expect(page.locator('#dashboard')).to_be_hidden()
    expect(page.locator('#error')).to_be_hidden()
    destination = parse_qs(urlparse(page.locator('#sign-in').get_attribute('href')).query)
    assert destination['next'] == [base + '/bmc']
    requests = len(fixture['requests'])
    page.clock.fast_forward(10000)
    assert len(fixture['requests']) == requests, 'Logout/expiry must stop background requests'
    page.screenshot(path=str(args.output / f'bmc-expired-{theme}-{width}.png'), full_page=True)
    assert not errors, errors
    context.close()

    context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
    fixture = install_bmc(context, power='Off')
    fixture['empty_events'] = True
    page = context.new_page()
    page.clock.install()
    page.goto('http://ui.test' + base + '/bmc')
    page.locator('#dashboard').wait_for(state='visible')
    expect(page.locator('#connect')).to_be_disabled()
    expect(page.locator('#power button[data-type=On]')).to_be_enabled()
    page.locator('#tab-events').click()
    expect(page.locator('#events-empty')).to_have_text('No events recorded yet.')
    assert page.locator('#screen').bounding_box()['height'] <= 200
    no_overflow(page, 'bmc-off-empty', width)
    page.screenshot(path=str(args.output / f'bmc-off-empty-{theme}-{width}.png'), full_page=True)
    fixture['system']['PowerState'] = 'On'
    page.clock.fast_forward(3100)
    expect(page.locator('#connect')).to_be_enabled()
    assert page.locator('#screen').get_attribute('data-state') is None
    assert fixture['posts'] == []
    context.close()


with sync_playwright() as playwright:
    browser = playwright.chromium.launch(executable_path=args.browser, headless=True, args=['--no-sandbox'])
    for theme in ([] if args.states_only else ['dark', 'light']):
        for width in [1440, 768, 390]:
            context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
            page = context.new_page()
            for name in ['list', 'detail', 'detail-viewer', 'images', 'launch', 'launch-os', 'launch-options', 'launched']:
                page.goto((args.previews / f'{name}.html').resolve().as_uri())
                no_overflow(page, name, width)
                if name in ['launch', 'launch-os']:
                    for radio in page.locator('input[type=radio]').all():
                        bounds = radio.bounding_box()
                        assert bounds['width'] <= 24 and bounds['height'] <= 24, bounds
                    page.locator('input[name=os]').nth(1).check()
                    assert page.locator('input[name=os]').nth(1).is_checked()
                page.screenshot(path=str(args.output / f'{name}-{theme}-{width}.png'), full_page=True)
                if name.startswith('detail'):
                    for selector in ['#effective-configuration', '#pod-resources', '#recent-events']:
                        disclosure = page.locator(selector)
                        assert not disclosure.evaluate('(e) => e.open')
                        disclosure.locator('summary').focus()
                        page.keyboard.press('Enter')
                        assert disclosure.evaluate('(e) => e.open')
                    no_overflow(page, name + '-expanded', width)
                    page.screenshot(path=str(args.output / f'{name}-expanded-{theme}-{width}.png'), full_page=True)
                    anchors = page.locator('.section-nav a').evaluate_all('(links) => links.map(a => a.hash.slice(1))')
                    assert all(page.locator('[id="' + anchor + '"]').count() == 1 for anchor in anchors), anchors
                if name == 'launch-options':
                    assert page.locator('#launch-limits').evaluate('(e) => e.open')
                    assert page.locator('#launch-disks').evaluate('(e) => e.open')
                    assert page.locator('#max_cpus').input_value() == '1'
                    assert page.locator('#disk0_size_gib').input_value() == '0'
                if name == 'launch':
                    assert not page.locator('#launch-limits').evaluate('(e) => e.open')
                    assert not page.locator('#launch-disks').evaluate('(e) => e.open')
                    page.locator('#name').fill('Invalid VM')
                    assert page.locator('#name').evaluate('(e) => e.validity.patternMismatch')
                    page.locator('#name').fill('fixture-vm')
                    page.locator('#launch-disks summary').click()
                    page.locator('#disk0_id').fill('logs')
                    page.locator('#disk0_size_gib').fill('0')
                    page.locator('#launch-disks summary').click()
                    # Hidden optional values are server-validated and survive
                    # submission, instead of blocking on an unfocusable input.
                    assert page.locator('form.launch').evaluate('(e) => e.checkValidity()')
                    values = page.locator('form.launch').evaluate('(e) => Object.fromEntries(new FormData(e))')
                    assert values['disk0_size_gib'] == '0'
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
    bmc_state_checks(browser, 1440, 'dark')
    bmc_state_checks(browser, 390, 'light')
    browser.close()
print('Dashboard and BMC browser checks passed (desktop, tablet, mobile; dark and light; admin and viewer).')
