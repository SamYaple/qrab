#!/usr/bin/env python3
"""Inspect dashboard previews and the shared BMC using isolated browser fixtures.

Requires Playwright and Chromium or Firefox. No cluster, login, guest or production token.
Generate previews with `cargo run -p qrab-dashboard --example preview -- OUT_DIR`.
All BMC HTTP requests are intercepted; POSTs only mutate the in-memory fixture.
"""
import argparse
import base64
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import mimetypes
from pathlib import Path
import socket
from threading import Event, Thread
from urllib.parse import parse_qs, urlparse
from playwright.sync_api import expect, sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--engine', choices=['chromium', 'firefox'], default='chromium')
parser.add_argument('--browser', help='Optional browser executable; Chromium defaults to the installed Chrome.')
parser.add_argument('--previews', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path('target/ui-browser'))
parser.add_argument('--states-only', action='store_true', help='Run only BMC and dedicated-console state/lifecycle checks.')
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


def install_bmc(context, role='admin', power='On', legacy=False, origin='http://ui.test'):
    fixture = {'system': system(power), 'posts': [], 'expired': False,
               'hold_posts': False, 'pending_posts': [], 'post_error': None,
               'fail_system': None, 'fail_metrics': False, 'hold_session': False, 'pending_session': [], 'empty_events': False, 'extra_events': [], 'requests': []}
    fixture['system']['Oem']['Qrab']['Configuration']['redacted'] = role == 'viewer'

    def route(request_route):
        req = request_route.request
        path = urlparse(req.url).path
        if path.startswith('/assets/'):
            relative = path.removeprefix('/assets/')
            relative = {'bmc.js': 'console.js', 'bmc.css': 'style.css'}.get(relative, relative)
            asset_root = root / ('projects/dashboard/assets' if legacy else 'projects/bmc/assets')
            asset = asset_root / relative
            if asset.is_file() and asset.resolve().is_relative_to(asset_root):
                request_route.fulfill(path=asset, content_type=mimetypes.guess_type(asset.name)[0] or 'application/octet-stream')
            else:
                request_route.fulfill(status=404)
            return
        if legacy and path == base + '/console':
            # Chromium needs the document's real loopback address space for
            # the native socket, so let the local fixture serve this page.
            request_route.continue_()
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
                if legacy:
                    request_route.fulfill(status=403, body=fixture['post_error'], content_type='text/plain')
                else:
                    request_route.fulfill(status=403, json={'error': {'message': fixture['post_error']}})
                return
            request_route.fulfill(status=204)
            return
        elif legacy and path == base + '/power':
            if fixture['fail_system']:
                request_route.fulfill(status=503, **fixture['fail_system'])
                return
            data = {'running': True, 'phase': 'Running', 'pod': True, 'power_state': fixture['system']['PowerState'],
                    'health': 'OK', 'reset_types': resets, 'redfish_error': None}
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
            data['Members'].extend(fixture['extra_events'])
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

    context.route(origin + '/**', route)
    return fixture


@contextmanager
def native_console_socket():
    """A native WebSocket with no guest payload, held during RFB negotiation.

    noVNC checks native channel properties. Playwright's WebSocket proxy wraps
    their prototype, so use a loopback upgrade for the disconnect lifecycle.
    """
    opened, closed = Event(), Event()
    connections = []

    class Handler(BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'

        def log_message(self, *_):
            pass

        def do_GET(self):
            if self.path == base + '/console' and not self.headers.get('Upgrade'):
                html = (args.previews / 'console.html').read_text().replace(f'file://{root}/projects/dashboard/assets/style.css', '/assets/style.css').replace('src="about:blank"', 'src="/assets/console.js"').encode()
                self.send_response(200)
                self.send_header('Content-Type', 'text/html; charset=utf-8')
                self.send_header('Content-Length', str(len(html)))
                self.send_header('Connection', 'close')
                self.end_headers()
                self.wfile.write(html)
                self.close_connection = True
                return
            key = self.headers.get('Sec-WebSocket-Key')
            if not key or self.headers.get('Upgrade', '').lower() != 'websocket':
                self.send_error(400)
                return
            accept = base64.b64encode(hashlib.sha1((key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
            self.send_response(101)
            self.send_header('Upgrade', 'websocket')
            self.send_header('Connection', 'Upgrade')
            self.send_header('Sec-WebSocket-Accept', accept)
            self.end_headers()
            connections.append(self.connection)
            opened.set()
            self.connection.settimeout(10)
            try:
                # RFB waits for the server greeting, so the browser sends no
                # data before its close frame. Acknowledge the native close.
                while data := self.connection.recv(4096):
                    if data[0] & 15 == 8:
                        self.connection.sendall(b'\x88\x00')
                        break
            except OSError:
                pass
            finally:
                self.close_connection = True
                closed.set()

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f'http://127.0.0.1:{server.server_port}', opened, closed
    finally:
        for connection in connections:
            try:
                connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            connection.close()
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def capture(page, path):
    # Full-page captures preserve fixed/sticky elements at the current scroll
    # position unless reset, which can put a sticky header in mid-artifact.
    page.evaluate("window.scrollTo(0, 0)")
    page.screenshot(path=str(path), full_page=True)


def no_overflow(page, name, width):
    measured = page.evaluate('({viewport:innerWidth, content:document.documentElement.scrollWidth})')
    assert page.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--surface').trim()"), f'{name}: stylesheet missing'
    assert measured['content'] <= measured['viewport'], f'{name} at {width}: {measured}'


def wait_native(page, event, diagnostics=None):
    # Allow browser events to settle while waiting for the socket thread.
    for _ in range(100):
        if event.is_set():
            return
        page.wait_for_timeout(20)
    assert event.is_set(), (page.locator('#console-status').inner_text(), diagnostics)


def console_state_checks(browser, width, theme):
    with native_console_socket() as (origin, opened, closed):
        context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
        fixture = install_bmc(context, legacy=True, origin=origin)
        page = context.new_page()
        page.clock.install()
        errors = []
        console_errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.on('console', lambda message: console_errors.append(message.text) if message.type == 'error' else None)
        page.goto(origin + base + '/console')
        pause = page.locator('[data-power=Pause]')
        expect(pause).to_be_enabled()
        fixture['post_error'] = 'Permission denied by the VM.'
        pause.click()
        expect(page.locator('#console-error')).to_have_text('Permission denied by the VM.')
        fixture['post_error'] = None
        fixture['hold_posts'] = True
        previous_posts = len(fixture['posts'])
        pause.click()
        page.clock.fast_forward(3100)
        expect(pause).to_be_disabled()
        pause.dispatch_event('click')
        assert len(fixture['posts']) == previous_posts + 1
        assert fixture['posts'][-1]['body'] == {'action': 'Pause', 'confirm': False}
        fixture['system']['PowerState'] = 'Paused'
        fixture['hold_posts'] = False
        fixture['pending_posts'].pop().fulfill(status=204)
        expect(page.locator('[data-power=Resume]')).to_be_enabled()

        page.locator('#connect').click()
        page.clock.run_for(50)
        assert page.locator('#console-error').is_hidden(), page.locator('#console-error').inner_text()
        expect(page.locator('#screen canvas')).to_have_count(1)
        wait_native(page, opened, (fixture['requests'], console_errors))
        fixture['hold_posts'] = True
        reset = page.locator('[data-power=ForceRestart]')
        reset.click()
        reset.click()
        expect(reset).to_be_disabled()
        if width == 1440:
            page.evaluate("document.querySelector('#screen').requestFullscreen()")
            assert page.evaluate('document.fullscreenElement.id') == 'screen'
        fixture['expired'] = True
        page.clock.fast_forward(3100)
        expect(page.locator('#console-recovery')).to_be_visible()
        expect(page.locator('#screen canvas')).to_have_count(0)
        wait_native(page, closed)
        if width == 1440:
            page.wait_for_function('document.fullscreenElement === null')
        expect(page.locator('#screen')).to_contain_text('Your session has ended')
        expect(page.locator('#connect')).to_be_disabled()
        fixture['hold_posts'] = False
        fixture['pending_posts'].pop().fulfill(status=204)
        page.wait_for_timeout(50)
        expect(page.locator('#console-status')).to_contain_text('Session ended')
        expect(page.locator('#audio')).to_be_disabled()
        destination = parse_qs(urlparse(page.locator('#console-recovery a').get_attribute('href')).query)
        assert destination['next'] == [base + '/console']
        requests = len(fixture['requests'])
        page.clock.fast_forward(10000)
        assert len(fixture['requests']) == requests
        assert page.locator('#power button:enabled').count() == 0
        no_overflow(page, 'console-expired', width)
        capture(page, args.output / f'console-expired-{theme}-{width}.png')
        assert not errors, errors
        context.close()


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
    capture(page, args.output / f'bmc-loading-{theme}-{width}.png')
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
    capture(page, args.output / f'bmc-pending-{theme}-{width}.png')
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
    capture(page, args.output / f'bmc-stale-{theme}-{width}.png')
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
    fixture['post_error'] = None
    previous_posts = len(fixture['posts'])
    for target in ['/vms/qrab/other/bmc/redfish/v1/Systems/1/Actions/ComputerSystem.Reset',
                   '/redfish/../../vms/qrab/other/power', '//other.example/redfish/v1/Systems/1',
                   '/redfish/v1/Systems/1#unexpected-fragment']:
        fixture['system']['Actions']['#ComputerSystem.Reset']['target'] = target
        page.clock.fast_forward(3100)
        resume.click()
        expect(resume).to_be_enabled()
        assert len(fixture['posts']) == previous_posts, 'A VM action URL must stay in its own Redfish bridge'
        expect(page.locator('#error')).to_contain_text('Invalid management URL')
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
    capture(page, args.output / f'bmc-expired-{theme}-{width}.png')
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
    capture(page, args.output / f'bmc-off-empty-{theme}-{width}.png')
    fixture['system']['PowerState'] = 'On'
    page.clock.fast_forward(3100)
    expect(page.locator('#connect')).to_be_enabled()
    assert page.locator('#screen').get_attribute('data-state') is None
    assert fixture['posts'] == []
    context.close()

    context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
    fixture = install_bmc(context, role='viewer')
    fixture['extra_events'] = [
        {'Id': '4', 'Created': 'not-a-timestamp', 'Oem': {'Qrab': {'Sequence': 4, 'Event': 'INVALID_TIME',
            'Data': {'detail': "<img src=x onerror='window.payloadExecuted=true'>" + 'x' * 4000}}}},
        {'Id': '5', 'Oem': {'Qrab': {'Sequence': 5, 'Event': 'MISSING_TIME', 'Data': {}}}},
    ]
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.goto('http://ui.test' + base + '/bmc')
    page.locator('#dashboard').wait_for(state='visible')
    page.locator('#tab-events').click()
    for event_name in ['INVALID_TIME', 'MISSING_TIME']:
        row = page.locator(f'#events li[data-event={event_name}]')
        expect(row.locator('.event-time')).to_have_text('Time unavailable')
    details = page.locator('#events li[data-event=INVALID_TIME] .event-details')
    details.locator('summary').focus()
    page.keyboard.press('Enter')
    payload = details.locator('.event-payload')
    expect(payload).to_contain_text('<img src=x')
    assert payload.bounding_box()['height'] <= 280
    assert page.locator('#events img').count() == 0
    assert page.evaluate('window.payloadExecuted === undefined')
    no_overflow(page, 'bmc-event-payload', width)
    assert page.locator('#panel-events').evaluate('(e) => e.scrollWidth <= e.clientWidth + 1')
    capture(page, args.output / f'bmc-event-payload-{theme}-{width}.png')
    assert not errors, errors
    context.close()


with sync_playwright() as playwright:
    options = {'headless': True}
    if args.browser:
        options['executable_path'] = args.browser
    elif args.engine == 'chromium':
        options['executable_path'] = '/opt/google/chrome/chrome'
    if args.engine == 'chromium':
        options['args'] = ['--no-sandbox']
    browser = getattr(playwright, args.engine).launch(**options)
    for theme in ([] if args.states_only else ['dark', 'light']):
        for width in [1440, 768, 390, 320]:
            context = browser.new_context(viewport={'width': width, 'height': 1000}, color_scheme=theme)
            page = context.new_page()
            for name in ['list', 'detail', 'detail-viewer', 'images', 'launch', 'launch-os', 'launch-options', 'launched',
                         'error-session', 'error-denied', 'error-missing', 'error-service']:
                page.goto((args.previews / f'{name}.html').resolve().as_uri())
                no_overflow(page, name, width)
                if name in ['launch', 'launch-os']:
                    for radio in page.locator('input[type=radio]').all():
                        bounds = radio.bounding_box()
                        assert bounds['width'] <= 24 and bounds['height'] <= 24, bounds
                    page.locator('input[name=os]').nth(1).check()
                    assert page.locator('input[name=os]').nth(1).is_checked()
                capture(page, args.output / f'{name}-{theme}-{width}.png')
                if name.startswith('detail'):
                    for selector in ['#effective-configuration', '#pod-resources', '#recent-events']:
                        disclosure = page.locator(selector)
                        assert not disclosure.evaluate('(e) => e.open')
                        disclosure.locator('summary').focus()
                        page.keyboard.press('Enter')
                        assert disclosure.evaluate('(e) => e.open')
                    no_overflow(page, name + '-expanded', width)
                    capture(page, args.output / f'{name}-expanded-{theme}-{width}.png')
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
                page.wait_for_function("Math.abs(parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--topbar-height')) - document.querySelector('.topbar').getBoundingClientRect().height) <= 1")
                if role == 'admin' and width <= 1000:
                    page.locator('.panel-jump').click()
                    assert page.evaluate('document.activeElement.id') == 'vm-panels'
                    assert page.locator('#vm-panels').bounding_box()['y'] >= page.locator('.topbar').bounding_box()['height']
                for tab in ['overview', 'metrics', 'events', 'agent', 'config']:
                    page.locator('#tab-' + tab).click()
                    no_overflow(page, f'bmc-{role}-{tab}', width)
                    capture(page, args.output / f'bmc-{role}-{tab}-{theme}-{width}.png')
                page.locator('#tab-events').click()
                page.locator('#events-filter').fill('resume')
                assert page.locator('#events li:visible').count() == 1
                event = page.locator('#events li[data-event=RESUME] .event-details')
                event.locator('summary').focus()
                page.keyboard.press('Enter')
                expect(event.locator('.event-payload')).to_contain_text('requested by user')
                capture(page, args.output / f'bmc-event-expanded-{role}-{theme}-{width}.png')
                page.locator('#events-filter').fill('not-a-match')
                page.locator('#events-filter').clear()
                assert event.evaluate('(e) => e.open')
                page.locator('#tab-overview').focus()
                page.keyboard.press('ArrowRight')
                assert page.locator('#tab-metrics').get_attribute('aria-selected') == 'true'
                if role == 'viewer':
                    assert page.locator('#connect').count() == 0
                    assert page.locator('[data-agent]').count() == 0
                else:
                    advanced = page.locator('#advanced-power')
                    assert not advanced.evaluate('(e) => e.open')
                    advanced.locator('summary').focus()
                    page.keyboard.press('Enter')
                    assert advanced.evaluate('(e) => e.open')
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
    console_state_checks(browser, 1440, 'dark')
    console_state_checks(browser, 390, 'light')
    browser.close()
mode = 'state/lifecycle' if args.states_only else 'layout and state/lifecycle'
print(f'{args.engine}: Dashboard/BMC {mode} checks passed.')
