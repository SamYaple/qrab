#!/usr/bin/env python3
"""Inspect dashboard previews and the shared BMC using isolated browser fixtures.

Requires Playwright and Chromium, Firefox or WebKit. No cluster, login, guest or production token.
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
parser.add_argument('--engine', choices=['chromium', 'firefox', 'webkit'], default='chromium')
parser.add_argument('--browser', help='Optional browser executable; Chromium defaults to the installed Chrome.')
parser.add_argument('--previews', type=Path, required=True)
parser.add_argument('--output', type=Path, default=Path('target/ui-browser'))
parser.add_argument('--states-only', action='store_true', help='Run only BMC and dedicated-console state/lifecycle checks.')
parser.add_argument('--forced-colors', action='store_true', help='Check the browser forced-colors high-contrast mode.')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
args.output.mkdir(parents=True, exist_ok=True)
base = '/vms/qrab/web-1'
resets = ['On', 'Pause', 'Resume', 'ForceRestart', 'GracefulRestart', 'GracefulShutdown', 'ForceOff', 'PowerCycle', 'Nmi']


def browser_context(browser, **options):
    if args.forced_colors:
        options['forced_colors'] = 'active'
    return browser.new_context(**options)


def chart_keys(page, selector, lines):
    for figure in page.locator(selector).all():
        style = figure.evaluate('''(figure, selector) => {
            const styles = nodes => [...nodes].map(node => {
                const value = getComputedStyle(node);
                return [value.stroke, value.strokeDasharray];
            });
            return {plots: styles(figure.querySelectorAll(selector)), keys: styles(figure.querySelectorAll('.swatch line'))};
        }''', lines)
        assert style['plots'] == style['keys'], ('Legend lines must match their chart', style)
        assert all(stroke not in ['none', 'transparent', 'rgba(0, 0, 0, 0)'] for stroke, _ in style['keys'])
        assert len({dash for _, dash in style['keys']}) == len(style['keys']), 'Series must remain distinguishable without color'


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
               'hold_posts': False, 'pending_posts': [], 'post_received': Event(), 'post_error': None, 'hold_power': False, 'pending_power': [],
               'fail_system': None, 'fail_metrics': False, 'fail_history': False, 'empty_metrics': False, 'hold_session': False, 'pending_session': [], 'empty_events': False, 'extra_events': [], 'requests': [],
               'vm_name': 'web-1', 'user_name': 'sam'}
    fixture['system']['Oem']['Qrab']['Configuration']['redacted'] = role == 'viewer'

    def route(request_route):
        req = request_route.request
        path = urlparse(req.url).path
        if path.startswith('/assets/'):
            relative = path.removeprefix('/assets/')
            relative = {'bmc.js': 'console.js', 'bmc.css': 'style.css'}.get(relative, relative)
            asset_root = root / ('projects/dashboard/assets' if legacy else 'projects/bmc/assets')
            if relative == 'favicon.svg':
                asset_root = root / 'projects/bmc/assets'
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
            data = {'user': {'name': fixture['user_name'], 'subject': 'fixture-user', 'csrf': 'fixture-csrf'}, 'role': role, 'vm_name': fixture['vm_name'], 'admin_bucket': []}
            if fixture['hold_session']:
                fixture['pending_session'].append((request_route, data))
                return
        elif req.method == 'POST':
            fixture['posts'].append({'path': path, 'body': req.post_data_json if req.post_data else None, 'csrf': req.headers.get('x-csrf-token')})
            fixture['post_received'].set()
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
                    'health': 'OK', 'reset_types': fixture['system']['Actions']['#ComputerSystem.Reset']['ResetType@Redfish.AllowableValues'], 'redfish_error': None}
            agent = fixture['system']['Oem']['Qrab']['GuestAgent']
            data['guest_restart_unavailable'] = ('Guest restart needs a connected guest agent.' if not agent['enabled'] or not agent['connected']
                                                  else 'Thaw guest filesystems before restarting the guest.' if agent.get('FreezeState') == 'Frozen' else None)
            if fixture['hold_power']:
                fixture['pending_power'].append((request_route, data))
                return
        elif path.endswith('/Systems/1'):
            if fixture['fail_system']:
                error = fixture['fail_system']
                request_route.fulfill(status=503, **error)
                return
            data = fixture['system']
        elif path.endswith('/ResetActionInfo'):
            data = {'Parameters': [{'Name': 'ResetType', 'AllowableValues': fixture['system']['Actions']['#ComputerSystem.Reset']['ResetType@Redfish.AllowableValues']}]}
        elif path.endswith('/Events/Entries'):
            data = {'Members': [] if fixture['empty_events'] else [{'Id': str(i), 'Created': f'2026-11-15T04:00:{i:02d}Z', 'Oem': {'Qrab': {'Sequence': i, 'Event': event, 'Data': {'reason': 'requested by user'}}}} for i, event in [(1, 'STOP'), (2, 'RESUME'), (3, 'DEVICE_DELETED')]]}
            data['Members'].extend(fixture['extra_events'])
        elif path.endswith('/EventService/SSE'):
            request_route.fulfill(body=': fixture\n\n', content_type='text/event-stream')
            return
        elif path.endswith('/MetricHistory'):
            if fixture['fail_history']:
                request_route.fulfill(status=503, json={'error': {'message': 'History temporarily unavailable.'}})
                return
            query = parse_qs(urlparse(req.url).query)
            metric, interval = query['metric'][0], query['range'][0]
            duration = {'15m': 900, '1h': 3600, '6h': 21600, '24h': 86400, '7d': 604800}[interval]
            end, step = 1794715200, duration // 4
            unit = 'cores' if metric == 'container-cpu' else 'bytes' if metric == 'container-memory' else 'bytes/s'
            values = [0.125, 0.25, 0.5, 0.1875] if unit == 'cores' else [536870912, 805306368, 1073741824, 939524096] if unit == 'bytes' else [0, 16384, 65536, 8192]
            data = {'metric': metric, 'range': interval, 'start': end - duration, 'end': end, 'step': step, 'unit': unit, 'series': []}
            if not fixture['empty_metrics'] and metric != 'nic-rx':
                labels = ['manager', 'redfish', 'bmc'] if metric.startswith('container-') else ['root' if metric.startswith('block-') else 'eth0']
                data['series'] = [{'labels': {'name': label}, 'points': [[end - duration + i * step, value / (j + 1)] for i, value in enumerate(values)]} for j, label in enumerate(labels)]
        elif path.endswith('/MetricReports/VM'):
            data = {'MetricValues': [] if fixture['empty_metrics'] else [{'MetricId': key, 'MetricValue': str(value)} for key, value in [('LogicalProcessors', 2), ('MemoryBytes', 1073741824), ('BlockReadBytes', 83886080), ('BlockWriteBytes', 41943040)]], 'Timestamp': '2026-11-15T04:00:00Z'}
        elif path.endswith('/Resources'):
            data = {} if fixture['empty_metrics'] else {
                name: {'process': {'resident_memory_bytes': rss, 'cpu_time_seconds': 3.42},
                       'container': {'memory_current_bytes': rss + 8388608,
                                     'memory_limit_bytes': 268435456, 'cpu_usage_seconds': 5.15}}
                for name, rss in [('Redfish', 33554432), ('Manager', 67108864)]
            }
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
                html = (args.previews / 'console.html').read_text().replace(f'file://{root}/projects/dashboard/assets/style.css', '/assets/style.css').replace(f'file://{root}/projects/dashboard/../bmc/assets/favicon.svg', '/assets/favicon.svg').replace('src="about:blank"', 'src="/assets/console.js"').encode()
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
    # Pump browser callbacks while waiting for a fixture event.
    for _ in range(100):
        if event.is_set():
            return
        page.wait_for_timeout(20)
    assert event.is_set(), (page.locator('#console-status').inner_text(), diagnostics)


def long_content_checks(browser, width, theme, height=1000, scale=1):
    context = browser_context(browser, viewport={'width': width, 'height': height}, device_scale_factor=scale, color_scheme=theme)
    fixture = install_bmc(context)
    fixture['vm_name'] = 'vm-' + 'a' * 60
    fixture['user_name'] = 'sam.with-a-long-operator-name-for-production'
    fixture['system']['Oem']['Qrab']['Configuration']['name'] = fixture['vm_name']
    page = context.new_page()
    page.goto('http://ui.test' + base + '/bmc')
    expect(page.locator('#vm-name')).to_have_text(fixture['vm_name'])
    expect(page.locator('#overview')).to_contain_text('Health')
    page.wait_for_function("Math.abs(parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--topbar-height')) - document.querySelector('.topbar').getBoundingClientRect().height) <= 1")
    assert page.locator('#vm-name').evaluate('(e) => e.scrollWidth <= e.clientWidth + 1'), 'VM name is clipped'
    no_overflow(page, 'bmc-long-name', width)
    if width <= 1000:
        page.locator('.panel-jump').click()
        sticky = page.locator('.topbar').evaluate("(e) => getComputedStyle(e).position === 'sticky'")
        assert page.locator('#vm-panels').bounding_box()['y'] >= (page.locator('.topbar').bounding_box()['height'] if sticky else 0)
        if height < 400:
            health = page.locator('#overview dd').nth(1).bounding_box()
            assert health['y'] + health['height'] <= height, 'Header leaves too little room for VM health'
    capture(page, args.output / f'bmc-long-name-{theme}-{width}-{height}-{scale}x.png')
    context.close()


def console_state_checks(browser, width, theme):
    with native_console_socket() as (origin, opened, closed):
        context = browser_context(browser, viewport={'width': width, 'height': 320 if width == 320 else 1000}, color_scheme=theme)
        fixture = install_bmc(context, legacy=True, origin=origin)
        page = context.new_page()
        page.clock.install()
        def focused_in_view(target):
            expect(target).to_be_focused()
            box = target.bounding_box()
            assert box['y'] >= 0 and box['y'] + box['height'] <= page.viewport_size['height'], 'Focused recovery must be visible'
        errors = []
        console_errors = []
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.on('console', lambda message: console_errors.append(message.text) if message.type == 'error' else None)
        page.goto(origin + base + '/console')
        pause = page.locator('[data-power=Pause]')
        expect(pause).to_be_enabled()
        restart_guest = page.locator('[data-power=GracefulRestart]')
        fixture['system']['Oem']['Qrab']['GuestAgent']['connected'] = False
        page.clock.fast_forward(3100)
        expect(restart_guest).to_be_disabled()
        expect(page.locator('#restart-requirement')).to_contain_text('connected guest agent')
        fixture['system']['Oem']['Qrab']['GuestAgent']['connected'] = True
        fixture['system']['Oem']['Qrab']['GuestAgent']['FreezeState'] = 'Frozen'
        page.clock.fast_forward(3100)
        expect(restart_guest).to_be_disabled()
        expect(page.locator('#restart-requirement')).to_contain_text('Thaw guest filesystems')
        fixture['system']['Oem']['Qrab']['GuestAgent']['FreezeState'] = 'Thawed'
        page.clock.fast_forward(3100)
        expect(restart_guest).to_be_enabled()
        expect(page.locator('#restart-requirement')).to_be_hidden()
        more = page.locator('#console-power-more')
        expect(more).to_be_visible()
        assert not more.evaluate('(e) => e.open')
        more.locator('summary').focus()
        page.keyboard.press('Enter')
        reset = page.locator('[data-power=ForceRestart]')
        expect(reset).to_be_visible()
        no_overflow(page, 'console-more-actions', width)
        capture(page, args.output / f'console-more-actions-{theme}-{width}.png')
        reset.focus()
        page.keyboard.down('Enter')
        page.keyboard.down('Enter')
        page.keyboard.up('Enter')
        assert fixture['posts'] == [], 'First click only arms the action'
        more.locator('summary').click()
        expect(reset).to_have_text('Reset')
        more.locator('summary').click()
        reset.click()
        action_info = fixture['system']['Actions']['#ComputerSystem.Reset']
        original_types = action_info['ResetType@Redfish.AllowableValues']
        action_info['ResetType@Redfish.AllowableValues'] = []
        more.locator('summary').focus()
        page.clock.fast_forward(3100)
        expect(more).to_be_hidden()
        focused_in_view(page.locator('#power-chip'))
        expect(reset).to_be_disabled()
        expect(reset).to_have_text('Reset')
        reset.dispatch_event('click')
        assert fixture['posts'] == [], 'An unoffered action must not be sent'
        action_info['ResetType@Redfish.AllowableValues'] = original_types
        page.clock.fast_forward(3100)
        expect(more).to_be_visible()
        assert not more.evaluate('(e) => e.open')
        more.locator('summary').click()
        fixture['post_error'] = 'Permission denied by the VM.'
        pause.click()
        expect(page.locator('#power-action-error')).to_have_text('Pause: Permission denied by the VM.')
        fixture['post_error'] = None
        fixture['hold_posts'] = True
        previous_posts = len(fixture['posts'])
        pause.click()
        page.clock.fast_forward(3100)
        expect(page.locator('#console-error')).to_be_hidden()
        expect(page.locator('#power-action-error')).to_be_hidden()
        expect(pause).to_be_disabled()
        expect(pause).to_have_text('Sending pause…')
        expect(pause).to_be_focused()
        expect(pause).to_have_attribute('aria-busy', 'true')
        expect(pause).to_have_attribute('aria-disabled', 'true')
        expect(page.locator('#power-action-status')).to_have_text('Sending pause…')
        pause.dispatch_event('click')
        assert len(fixture['posts']) == previous_posts + 1
        assert fixture['posts'][-1]['body'] == {'action': 'Pause', 'confirm': False}
        fixture['system']['PowerState'] = 'Paused'
        fixture['hold_posts'] = False
        fixture['hold_power'] = True
        fixture['pending_posts'].pop().fulfill(status=204)
        expect(pause).to_have_text('Updating state…')
        expect(pause).to_be_focused()
        expect(page.locator('#power-action-status')).to_contain_text('request accepted. Updating state')
        page.clock.fast_forward(3100)
        expect(pause).to_be_disabled()
        assert len(fixture['pending_power']) == 1, 'Follow-up state reads must coalesce'
        fixture['hold_power'] = False
        pending_read, data = fixture['pending_power'].pop()
        if width == 1440:
            # A key pressed while blocked must not activate on release after
            # the request settles, even when the first read still says On.
            data['power_state'] = 'On'
            page.keyboard.down('Space')
        if width == 390:
            page.locator('.back').focus()
        pending_read.fulfill(json=data)
        if width == 1440:
            expect(pause).to_be_enabled()
            page.keyboard.up('Space')
            page.clock.fast_forward(3100)
            assert len(fixture['posts']) == previous_posts + 1, 'A blocked held key must not send after settlement'
        expect(page.locator('[data-power=Resume]')).to_be_enabled()
        expect(page.locator('[data-power=GracefulShutdown]')).to_be_enabled()
        focused_in_view(page.locator('.back' if width == 390 else '#power-chip'))
        expect(pause).to_have_text('Pause')
        expect(pause).to_have_attribute('aria-busy', 'false')
        expect(page.locator('#power-action-status')).to_have_text('Pause request accepted.')
        expect(page.locator('#console-status')).to_have_text('Console disconnected')

        # A failed follow-up read must not erase the action's failure reason.
        fixture['hold_posts'] = True
        fixture['post_received'].clear()
        page.locator('[data-power=Resume]').click()
        wait_native(page, fixture['post_received'])
        fixture['fail_system'] = {'json': {'error': {'message': 'State read unavailable.'}}}
        fixture['pending_posts'].pop().fulfill(status=403, body='Permission denied by the VM.', content_type='text/plain')
        expect(page.locator('#power-chip')).to_have_text('Unknown')
        expect(page.locator('#power-action-error')).to_have_text('Resume: Permission denied by the VM.')
        expect(page.locator('#console-error')).to_contain_text('State read unavailable.')
        page.clock.fast_forward(3100)
        expect(page.locator('#power-action-error')).to_have_text('Resume: Permission denied by the VM.')
        fixture['hold_posts'] = False
        fixture['fail_system'] = None
        page.clock.fast_forward(3100)
        expect(page.locator('[data-power=Resume]')).to_be_enabled()
        expect(page.locator('#power-action-error')).to_have_text('Resume: Permission denied by the VM.')
        expect(page.locator('#console-error')).to_be_hidden()

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
        expect(page.locator('#power-action-status')).to_have_text('Sending reset…')
        more.locator('summary').click()
        expect(page.locator('#power-action-status')).to_be_visible()
        if width == 1440:
            page.evaluate("document.querySelector('#screen').requestFullscreen()")
            assert page.evaluate('document.fullscreenElement.id') == 'screen'
        elif width == 320:
            more.locator('summary').focus()
            more.locator('summary').scroll_into_view_if_needed()
            assert page.evaluate('scrollY > 0')
        else:
            page.locator('#connect').focus()
        fixture['expired'] = True
        page.clock.fast_forward(3100)
        expect(page.locator('#console-recovery')).to_be_visible()
        expect(page.locator('#power-action-status')).to_be_hidden()
        expect(reset).to_have_attribute('aria-busy', 'false')
        if width != 1440:
            focused_in_view(page.locator('#console-recovery a'))
        expect(page.locator('#screen canvas')).to_have_count(0)
        wait_native(page, closed)
        if width == 1440:
            page.wait_for_function('document.fullscreenElement === null')
        expect(page.locator('#screen')).to_contain_text('Your session has ended')
        expect(page.locator('#connect')).to_be_disabled()
        fixture['hold_posts'] = False
        late = fixture['pending_posts'].pop()
        if width == 1440:
            late.fulfill(status=403, body='Late refusal.', content_type='text/plain')
        else:
            late.fulfill(status=204)
        page.wait_for_timeout(50)
        expect(page.locator('#console-status')).to_contain_text('Session ended')
        expect(page.locator('#power-action-status')).to_be_hidden()
        expect(page.locator('#power-action-error')).to_be_hidden()
        expect(page.locator('#audio')).to_be_disabled()
        destination = parse_qs(urlparse(page.locator('#console-recovery a').get_attribute('href')).query)
        assert destination['next'] == [base + '/console']
        requests = len(fixture['requests'])
        page.clock.fast_forward(10000)
        assert len(fixture['requests']) == requests
        assert page.locator('#power button:enabled').count() == 0
        no_overflow(page, 'console-expired', width)
        capture(page, args.output / f'console-expired-{theme}-{width}.png')
        # Expiry while navigating elsewhere must preserve that focus.
        fixture['expired'] = False
        page.goto(origin + base + '/console')
        expect(page.locator('[data-power=Resume]')).to_be_enabled()
        page.locator('.back').focus()
        fixture['expired'] = True
        page.clock.fast_forward(3100)
        expect(page.locator('#console-recovery')).to_be_visible()
        focused_in_view(page.locator('.back'))
        assert not errors, errors
        context.close()


def bmc_state_checks(browser, width, theme):
    context = browser_context(browser, viewport={'width': width, 'height': 320 if width == 320 else 1000}, color_scheme=theme)
    fixture = install_bmc(context)
    full_advertised = fixture['system']['Actions']['#ComputerSystem.Reset']['ResetType@Redfish.AllowableValues']
    fixture['system']['Actions']['#ComputerSystem.Reset']['ResetType@Redfish.AllowableValues'] = ['Pause']
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
    advertised = fixture['system']['Actions']['#ComputerSystem.Reset']
    advertised['ResetType@Redfish.AllowableValues'] = full_advertised
    page.clock.fast_forward(3100)
    expect(page.locator('#advanced-power')).to_be_visible()
    original_advertised = advertised['ResetType@Redfish.AllowableValues']
    page.locator('#advanced-power summary').click()
    advertised['ResetType@Redfish.AllowableValues'] = ['Pause']
    page.clock.fast_forward(3100)
    expect(page.locator('#power button[data-type=ForceRestart]')).to_be_hidden()
    expect(page.locator('#power button[data-type=ForceRestart]')).to_be_disabled()
    expect(page.locator('#advanced-power')).to_be_hidden()
    expect(page.locator('#vm-status')).to_be_focused()
    expect(pause).to_be_enabled()
    page.locator('#power button[data-type=ForceRestart]').dispatch_event('click')
    assert fixture['posts'] == []
    advertised['ResetType@Redfish.AllowableValues'] = []
    page.clock.fast_forward(3100)
    expect(page.locator('#power')).to_be_hidden()
    expect(page.locator('#power-unavailable')).to_be_visible()
    advertised['ResetType@Redfish.AllowableValues'] = original_advertised
    page.clock.fast_forward(3100)
    expect(page.locator('#power')).to_be_visible()
    expect(page.locator('#power-unavailable')).to_be_hidden()
    assert not page.locator('#advanced-power').evaluate('(e) => e.open')
    advertised.pop('ResetType@Redfish.AllowableValues')
    page.clock.fast_forward(3100)
    expect(pause).to_be_enabled()
    expect(page.locator('#advanced-power')).to_be_hidden()
    assert sum(path.endswith('/ResetActionInfo') for _, path in fixture['requests']) == 1, 'Availability refresh must use the existing System poll'
    advertised['ResetType@Redfish.AllowableValues'] = original_advertised
    page.clock.fast_forward(3100)
    expect(page.locator('#advanced-power')).to_be_visible()
    restart_guest = page.locator('#power button[data-type=GracefulRestart]')
    fixture['system']['Oem']['Qrab']['GuestAgent']['connected'] = False
    page.clock.fast_forward(3100)
    expect(restart_guest).to_be_disabled()
    expect(page.locator('#restart-requirement')).to_contain_text('connected guest agent')
    fixture['system']['Oem']['Qrab']['GuestAgent']['connected'] = True
    fixture['system']['Oem']['Qrab']['GuestAgent']['FreezeState'] = 'Frozen'
    page.clock.fast_forward(3100)
    expect(restart_guest).to_be_disabled()
    expect(page.locator('#restart-requirement')).to_contain_text('Thaw guest filesystems')
    fixture['system']['Oem']['Qrab']['GuestAgent']['FreezeState'] = 'Thawed'
    page.clock.fast_forward(3100)
    expect(restart_guest).to_be_enabled()

    advanced = page.locator('#advanced-power')
    advanced.locator('summary').click()
    reset = page.locator('#power button[data-type=ForceRestart]')
    reset.focus()
    page.keyboard.down('Enter')
    page.keyboard.down('Enter')
    page.keyboard.up('Enter')
    assert fixture['posts'] == [], 'Enter repeat must not confirm a reset'
    advanced.locator('summary').click()
    expect(reset).to_have_text('Reset')

    # Polls must never unlock a pending action, and synthetic duplicate events
    # must not send another POST even if a caller bypasses the disabled DOM.
    fixture['hold_posts'] = True
    pause.click()
    expect(pause).to_have_attribute('aria-busy', 'true')
    expect(pause).to_be_focused()
    page.clock.fast_forward(3100)
    expect(pause).to_be_focused()
    assert page.locator('#power button:enabled:not([aria-disabled=true]), [data-agent]:enabled:not([aria-disabled=true])').count() == 0
    pause.dispatch_event('click')
    assert len(fixture['posts']) == 1
    capture(page, args.output / f'bmc-pending-{theme}-{width}.png')
    page.keyboard.down('Space')
    fixture['hold_posts'] = False
    fixture['pending_posts'].pop().fulfill(status=204)
    expect(pause).to_be_enabled()
    page.keyboard.up('Space')
    page.clock.fast_forward(3100)
    assert len(fixture['posts']) == 1, 'A blocked held key must not send after settlement'
    fixture['system']['PowerState'] = 'Paused'
    page.clock.fast_forward(3100)
    expect(page.locator('#power button[data-type=Resume]')).to_be_enabled()
    expect(page.locator('#vm-status')).to_be_focused()
    expect(page.locator('#action-status')).to_contain_text('request accepted')

    page.locator('#tab-agent').click()
    fixture['hold_posts'] = True
    fixture['post_received'].clear()
    thaw = page.locator('[data-agent=thaw]')
    thaw.click()
    expect(thaw).to_have_attribute('aria-busy', 'true')
    expect(thaw).to_be_focused()
    fixture['system']['Oem']['Qrab']['GuestAgent']['connected'] = False
    wait_native(page, fixture['post_received'])
    fixture['hold_posts'] = False
    fixture['pending_posts'].pop().fulfill(json={'ThawedFilesystems': 2})
    expect(thaw).to_be_disabled()
    expect(page.locator('#tab-agent')).to_be_focused()
    fixture['system']['Oem']['Qrab']['GuestAgent']['connected'] = True
    page.clock.fast_forward(3100)
    expect(thaw).to_be_enabled()
    page.locator('#tab-overview').click()

    config = fixture['system']['Oem']['Qrab'].pop('Configuration')
    agent = fixture['system']['Oem']['Qrab'].pop('GuestAgent')
    page.clock.fast_forward(3100)
    for label in ['Disks', 'NICs', 'Guest agent']:
        expect(page.locator('#overview dt').filter(has_text=label).locator('xpath=following-sibling::dd[1]')).to_have_text('Unavailable')
    page.locator('#tab-config').click()
    expect(page.locator('#config')).to_have_text('ConfigurationUnavailable')
    expect(page.locator('#config-note')).to_have_text('Unavailable')
    page.locator('#tab-agent').click()
    expect(page.locator('#agent dt').filter(has_text='Channel').locator('xpath=following-sibling::dd[1]')).to_have_text('Unavailable')
    expect(page.locator('#agent dt').filter(has_text='Connected').locator('xpath=following-sibling::dd[1]')).to_have_text('Unavailable')
    expect(page.locator('#agent-note')).to_have_text('Unavailable')
    fixture['system']['Oem']['Qrab']['Configuration'] = {**config, 'disks': [], 'nics': [], 'guest_network': False}
    fixture['system']['Oem']['Qrab']['GuestAgent'] = {**agent, 'enabled': False, 'connected': False}
    page.clock.fast_forward(3100)
    page.locator('#tab-overview').click()
    for label in ['Disks', 'NICs']:
        expect(page.locator('#overview dt').filter(has_text=label).locator('xpath=following-sibling::dd[1]')).to_have_text('none')
    for label in ['Guest network', 'Guest agent']:
        expect(page.locator('#overview dt').filter(has_text=label).locator('xpath=following-sibling::dd[1]')).to_have_text('disabled')
    fixture['system']['Oem']['Qrab']['Configuration'] = config
    fixture['system']['Oem']['Qrab']['GuestAgent'] = agent
    page.clock.fast_forward(3100)
    page.locator('#tab-agent').click()
    expect(page.locator('#agent-note')).to_have_text('Connected')
    page.locator('#tab-config').click()
    expect(page.locator('#config-note')).to_have_text('Full view')
    expect(page.locator('#config')).to_contain_text('web-1')
    page.locator('#tab-agent').click()
    update = agent['LastUpdate']
    agent['LastUpdate'] = {**update, 'Output': '\n'.join(f'output line {n:03d} <script>inert</script>' for n in range(300))}
    page.clock.fast_forward(3100)
    output = page.locator('#agent dt').filter(has_text='Update output').locator('xpath=following-sibling::dd[1]')
    output.evaluate('''dd => {
        window.updateOutputNode = dd;
        const text = document.createTreeWalker(dd, NodeFilter.SHOW_TEXT).nextNode();
        const range = document.createRange(); range.setStart(text, 0); range.setEnd(text, 6);
        getSelection().removeAllRanges(); getSelection().addRange(range);
    }''')
    agent['last_seen_unix_ms'] = 1794715200000
    page.clock.fast_forward(3100)
    last_seen = page.locator('#agent dt').filter(has_text='Last seen').locator('xpath=following-sibling::dd[1]')
    expect(last_seen).not_to_have_text('—')
    assert output.evaluate('dd => dd === window.updateOutputNode'), 'A changing timestamp must not replace unchanged update output'
    assert page.evaluate('getSelection().toString()') == 'output'
    expect(page.locator('#agent dt').filter(has_text='Connected').locator('xpath=following-sibling::dd[1]')).to_have_text('yes')
    output_area = output.locator('pre')
    expect(output_area).to_have_attribute('role', 'region')
    expect(output_area).to_have_attribute('aria-label', 'Update output')
    output_area.focus()
    output_area.evaluate('e => e.scrollTop = 150')
    previous_seen = last_seen.inner_text()
    agent['last_seen_unix_ms'] += 1000
    page.clock.fast_forward(3100)
    expect(last_seen).not_to_have_text(previous_seen)
    expect(output_area).to_be_focused()
    assert output_area.evaluate('e => e.scrollTop') == 150
    agent['last_error'] = 'Fixture agent warning.'
    page.clock.fast_forward(3100)
    expect(page.locator('#agent')).to_contain_text('Fixture agent warning.')
    assert output.evaluate('dd => dd === window.updateOutputNode'), 'A neighboring error must not replace unchanged output'
    expect(output_area).to_be_focused()
    assert output_area.evaluate('e => e.scrollTop') == 150
    agent.pop('last_error')
    page.clock.fast_forward(3100)
    expect(page.locator('#agent')).not_to_contain_text('Fixture agent warning.')
    expect(output_area).to_be_focused()
    assert output_area.evaluate('e => e.scrollTop') == 150
    assert output_area.bounding_box()['height'] <= 260
    assert output.locator('script').count() == 0
    capture(page, args.output / f'bmc-agent-output-{theme}-{width}.png')
    agent['connected'] = False
    page.clock.fast_forward(3100)
    expect(page.locator('#panel-agent')).to_be_focused()
    expect(output_area).to_have_count(0)
    agent['connected'] = True
    agent['LastUpdate'] = update
    page.clock.fast_forward(3100)
    page.locator('#tab-overview').click()

    fixture['fail_system'] = {'json': {'error': {'message': 'The VM has no running Pod.'}}}
    page.clock.fast_forward(3100)
    expect(page.locator('#read-status')).to_contain_text('The VM has no running Pod.')
    expect(page.locator('#read-status')).to_contain_text('last updated')
    assert page.locator('#power button:enabled, [data-agent]:enabled').count() == 0
    expect(page.locator('#connect')).to_be_disabled()
    no_overflow(page, 'bmc-stale', width)
    capture(page, args.output / f'bmc-stale-{theme}-{width}.png')
    page.evaluate('''() => {
        window.staleAnnouncements = 0;
        new MutationObserver(() => window.staleAnnouncements++)
            .observe(document.getElementById('read-status'), {childList: true, subtree: true, characterData: true});
    }''')
    page.clock.fast_forward(3100)
    assert page.evaluate('window.staleAnnouncements') == 0, 'Repeated read failures must not repeat the same warning'
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
    fixture['empty_metrics'] = True
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
    fixture['system']['Actions']['#ComputerSystem.Reset']['target'] = '/redfish/v1/Systems/1/Actions/ComputerSystem.Reset'
    page.clock.fast_forward(3100)
    fixture['hold_posts'] = True
    fixture['post_received'].clear()
    resume.click()
    wait_native(page, fixture['post_received'])
    page.locator('#tab-overview').focus()
    fixture['expired'] = True
    page.clock.fast_forward(3100)
    expect(page.locator('#login')).to_be_visible()
    expect(page.locator('#sign-in')).to_be_focused()
    box = page.locator('#sign-in').bounding_box()
    assert box['y'] >= 0 and box['y'] + box['height'] <= page.viewport_size['height']
    expect(page.locator('#dashboard')).to_be_hidden()
    expect(page.locator('#error')).to_be_hidden()
    expect(resume).not_to_have_attribute('aria-busy', 'true')
    fixture['hold_posts'] = False
    fixture['pending_posts'].pop().fulfill(status=403, body='Late refusal.', content_type='text/plain')
    page.wait_for_timeout(50)
    expect(page.locator('#error')).to_be_hidden()
    expect(page.locator('#action-status')).to_be_hidden()
    destination = parse_qs(urlparse(page.locator('#sign-in').get_attribute('href')).query)
    assert destination['next'] == [base + '/bmc']
    requests = len(fixture['requests'])
    page.clock.fast_forward(10000)
    assert len(fixture['requests']) == requests, 'Logout/expiry must stop background requests'
    capture(page, args.output / f'bmc-expired-{theme}-{width}.png')
    fixture['expired'] = False
    page.goto('http://ui.test' + base + '/bmc')
    expect(page.locator('#dashboard')).to_be_visible()
    page.locator('#vm-detail-link').focus()
    fixture['expired'] = True
    page.clock.fast_forward(3100)
    expect(page.locator('#login')).to_be_visible()
    expect(page.locator('#vm-detail-link')).to_be_focused()
    assert not errors, errors
    context.close()

    context = browser_context(browser, viewport={'width': width, 'height': 1000}, color_scheme=theme)
    fixture = install_bmc(context, power='Off')
    fixture['empty_events'] = True
    page = context.new_page()
    page.clock.install()
    page.goto('http://ui.test' + base + '/bmc')
    page.locator('#dashboard').wait_for(state='visible')
    expect(page.locator('#connect')).to_be_disabled()
    expect(page.locator('#power button[data-type=On]')).to_be_enabled()
    page.locator('#advanced-power summary').click()
    expect(page.locator('#power button[data-type=PowerCycle]')).to_be_enabled()
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

    context = browser_context(browser, viewport={'width': width, 'height': 1000}, color_scheme=theme)
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


def timezone_checks(browser):
    cases = [
        ('America/Los_Angeles', ['America/Los_Angeles'], '8:00:01', '9:00:01', '8:00:00', '7:00:00', 'PM'),
        ('Asia/Kolkata', ['Asia/Kolkata', 'Asia/Calcutta'], '9:30:01', '9:30:01', '9:30:00', '8:30:00', 'AM'),
    ]
    for zone, aliases, winter, summer, sampled, first_sample, period in cases:
        context = browser_context(browser, viewport={'width': 390, 'height': 1000}, locale='en-US', timezone_id=zone)
        fixture = install_bmc(context)
        fixture['system']['Oem']['Qrab']['GuestAgent']['connected_since_unix_ms'] = 1794715200000
        fixture['system']['Oem']['Qrab']['GuestAgent']['LastUpdate']['EndTime'] = 'not-a-date'
        fixture['extra_events'] = [{'Id': '100', 'Created': '2026-07-15T04:00:01Z',
                                    'Oem': {'Qrab': {'Sequence': 100, 'Event': 'SUMMER', 'Data': {}}}}]
        page = context.new_page()
        page.goto('http://ui.test' + base + '/bmc')
        page.locator('#tab-events').click()
        note = page.locator('#panel-events [data-local-time]').inner_text()
        assert any(alias in note for alias in aliases), note
        for event, clock in [('STOP', winter), ('SUMMER', summer)]:
            at = page.locator(f'#events li[data-event={event}] time')
            expect(at).to_contain_text(clock)
            expect(at).to_contain_text(period)
        row = page.locator('#events li[data-event=STOP]')
        row.locator('summary').click()
        expect(row.locator('.event-context')).to_contain_text('2026-11-15T04:00:01.000Z')
        page.locator('#tab-metrics').click()
        expect(page.locator('#metrics-note')).to_contain_text(sampled)
        expect(page.locator('#metrics-note')).to_contain_text(period)
        chart = page.locator('svg.chart').first
        expect(chart).to_be_visible()
        chart.focus()
        page.keyboard.press('Home')
        expect(page.locator('.history-chart .readout').first).to_contain_text(first_sample)
        page.locator('#tab-agent').click()
        connected = page.locator('#agent').get_by_text('Connected', exact=True).locator('xpath=following-sibling::dd[1]')
        expect(connected).to_contain_text(sampled)
        expect(page.locator('#agent')).to_contain_text('Time unavailable')
        no_overflow(page, 'timezone-' + zone, 390)
        capture(page, args.output / ('timezone-' + zone.replace('/', '-') + '.png'))
        context.close()


def metric_focus_checks(browser):
    context = browser_context(browser, viewport={'width': 390, 'height': 1000})
    fixture = install_bmc(context)
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.clock.install()
    page.goto('http://ui.test' + base + '/bmc')
    page.locator('#tab-metrics').click()
    chart = page.locator('[data-metric="container-cpu"]')

    def poll():
        status = page.locator('.history-status')
        previous = status.inner_text()
        page.clock.fast_forward(31000)
        expect(status).not_to_have_text(previous)
        expect(status).not_to_have_text('Loading…')

    expect(chart.locator('svg.chart')).to_be_visible()
    chart.locator('svg.chart').focus()
    expect(page.locator('#history-availability')).to_have_text('Metric history available.')
    page.evaluate('''() => {
        window.metricAnnouncements = [];
        window.sampleAnnouncements = [];
        const sample = document.querySelector('[data-metric="container-cpu"] .readout');
        new MutationObserver(() => window.sampleAnnouncements.push(sample.textContent))
            .observe(sample, {childList: true, subtree: true, characterData: true});
        for (const id of ['metrics-availability', 'history-availability']) {
            const region = document.getElementById(id);
            new MutationObserver(() => window.metricAnnouncements.push(region.textContent))
                .observe(region, {childList: true, subtree: true, characterData: true});
        }
    }''')
    poll()
    expect(chart.locator('svg.chart')).to_be_focused()
    assert page.evaluate('window.metricAnnouncements') == [], 'Ordinary polls must not repeat live announcements'
    sample_announcements = page.evaluate('window.sampleAnnouncements')
    assert sample_announcements == [], ('An unchanged selected sample must remain quiet', sample_announcements)
    selected = chart.locator('.readout').inner_text()
    for width in [768, 390, 320]:
        page.set_viewport_size({'width': width, 'height': 1000})
        plot_width = max(240, chart.locator('.chart-body').evaluate('(e) => e.clientWidth'))
        expect(chart.locator('svg.chart')).to_have_attribute('width', str(plot_width))
        expect(chart.locator('svg.chart')).to_be_focused()
        expect(chart.locator('.readout')).to_have_text(selected)
    hit = chart.locator('rect.hit').bounding_box()
    page.mouse.move(hit['x'] + 1, hit['y'] + hit['height'] / 2)
    expect(chart.locator('.readout')).to_contain_text('manager 125 mcores')
    page.mouse.move(hit['x'] + hit['width'] - 1, hit['y'] + hit['height'] / 2)
    expect(chart.locator('.readout')).to_contain_text('manager 188 mcores')
    fixture['empty_metrics'] = True
    poll()
    expect(chart.locator('.chart-empty')).to_be_focused()
    message = chart.locator('.chart-empty').element_handle()
    poll()
    assert message.evaluate('(e) => e.isConnected'), 'An unchanged empty message should keep its DOM and focus'
    expect(chart.locator('.chart-empty')).to_be_focused()
    fixture['empty_metrics'] = False
    poll()
    expect(chart.locator('svg.chart')).to_be_focused()
    fixture['fail_history'] = True
    poll()
    expect(chart.locator('.chart-error')).to_be_focused()
    expect(page.locator('#history-availability')).to_have_text('Metric history unavailable.')
    fixture['fail_history'] = False
    poll()
    expect(chart.locator('svg.chart')).to_be_focused()
    assert page.evaluate('window.metricAnnouncements') == ['Metric history unavailable.', 'Metric history available.']
    # Background updates must not move the user's focus from another control.
    range_button = page.locator('.ranges button').first
    range_button.focus()
    fixture['empty_metrics'] = True
    poll()
    expect(range_button).to_be_focused()
    fixture['fail_metrics'] = True
    poll()
    expect(page.locator('#metrics-availability')).to_have_text('VM metrics unavailable.')
    poll()
    assert page.evaluate("window.metricAnnouncements.filter(v => v === 'VM metrics unavailable.').length") == 1
    fixture['fail_metrics'] = False
    poll()
    expect(page.locator('#metrics-availability')).to_have_text('VM metrics available.')
    expect(range_button).to_be_focused()
    assert page.evaluate("window.metricAnnouncements.filter(v => v === 'VM metrics available.').length") == 1
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
            context = browser_context(browser, viewport={'width': width, 'height': 1000}, color_scheme=theme)
            page = context.new_page()
            for name in ['list', 'list-long', 'list-storage', 'detail', 'detail-long', 'detail-viewer', 'detail-redfish-only', 'detail-no-power-actions', 'detail-needs-agent', 'detail-frozen-agent', 'detail-paused', 'detail-powered-off', 'detail-powering-on', 'detail-unknown-power', 'images', 'launch', 'launch-os', 'launch-options', 'launch-unavailable', 'launched',
                         'error-session', 'error-denied', 'error-missing', 'error-service']:
                page.goto((args.previews / f'{name}.html').resolve().as_uri())
                if args.forced_colors:
                    assert page.evaluate("matchMedia('(forced-colors: active)').matches")
                    current = page.locator('.site-nav [aria-current=page]')
                    if current.count():
                        assert 'underline' in current.evaluate('(e) => getComputedStyle(e).textDecorationLine')
                no_overflow(page, name, width)
                if name in ['launch', 'launch-os']:
                    assert page.locator('#name').get_attribute('maxlength') == '43'
                    for radio in page.locator('input[type=radio]').all():
                        bounds = radio.bounding_box()
                        assert bounds['width'] <= 24 and bounds['height'] <= 24, bounds
                    page.locator('input[name=os]').nth(1).check()
                    assert page.locator('input[name=os]').nth(1).is_checked()
                if name == 'launch-unavailable':
                    assert page.locator('form.launch').count() == 0
                    expect(page.locator('h1')).to_have_text('Launching is unavailable')
                if name == 'images':
                    assert page.locator('td[data-label=Boot]').evaluate_all('''cells => cells.every(cell => {
                        const text = document.createRange();
                        text.selectNodeContents(cell);
                        return text.getClientRects().length === 1;
                    })'''), 'The UEFI boot label must stay on one line'
                    staging = page.locator('#image-staging')
                    assert not staging.evaluate('(e) => e.open')
                    capture(page, args.output / f'images-collapsed-{theme}-{width}.png')
                    staging.locator('summary').focus()
                    page.keyboard.press('Enter')
                    assert staging.evaluate('(e) => e.open')
                    expect(staging).to_contain_text('stage-base-image.sh')
                if name == 'list-long':
                    assert page.locator('.table-wrap').evaluate('(e) => e.scrollWidth <= e.clientWidth + 1')
                    expect(page.locator('td[data-label=Compute]')).to_contain_text('16 GiB (max 1 TiB)')
                    expect(page.locator('td[data-label="Usage snapshot"]')).to_contain_text('8.125 cores')
                if name == 'list-storage':
                    for vm in ['pending-storage', 'static-media']:
                        row = page.get_by_role('row').filter(has=page.get_by_role('link', name=vm, exact=True))
                        expect(row.locator('td[data-label=Storage]')).to_contain_text('Capacity not reported')
                        expect(row.locator('td[data-label=Storage]')).not_to_contain_text('static')
                    row = page.get_by_role('row').filter(has=page.get_by_role('link', name='without-disks', exact=True))
                    expect(row.locator('td[data-label=Storage]')).to_have_text('No disks')
                    row = page.get_by_role('row').filter(has=page.get_by_role('link', name='reported-storage', exact=True))
                    expect(row.locator('td[data-label=Storage]')).to_contain_text('8.0 GiB reported')
                capture(page, args.output / f'{name}-{theme}-{width}.png')
                if name.startswith('detail'):
                    if name == 'detail':
                        expect(page.get_by_role('button', name='Resume', exact=True)).to_be_disabled()
                    if name in ['detail-paused', 'detail-powered-off', 'detail-powering-on', 'detail-unknown-power']:
                        expect(page.get_by_role('button', name='Pause', exact=True)).to_be_disabled()
                        if name == 'detail-paused':
                            expect(page.get_by_role('button', name='Resume', exact=True)).to_be_enabled()
                        else:
                            expect(page.get_by_role('button', name='Resume', exact=True)).to_be_disabled()
                        if name == 'detail-powered-off':
                            expect(page.get_by_role('button', name='Power on', exact=True)).to_be_enabled()
                        else:
                            expect(page.get_by_role('button', name='Power on', exact=True)).to_be_disabled()
                        expect(page.get_by_role('button', name='Power off', exact=True)).to_be_enabled()
                        expect(page.locator('#power-state-help')).to_contain_text('snapshot')
                    chart_keys(page, '.spark:has(polyline)', 'polyline')
                    more_power = page.locator('#more-power-actions')
                    if more_power.count():
                        assert not more_power.evaluate('(e) => e.open')
                        expect(more_power.get_by_role('button')).to_have_count(0)
                        more_power.locator('summary').focus()
                        page.keyboard.press('Enter')
                        assert more_power.evaluate('(e) => e.open')
                        assert more_power.get_by_role('button').count() > 0
                    if name == 'detail-no-power-actions':
                        expect(page.get_by_role('heading', name='Power', exact=True).locator('..')).to_contain_text('No guest power actions are currently offered')
                        assert more_power.count() == 0
                    confirmations = page.get_by_role('checkbox')
                    if confirmations.count():
                        names = [box.get_attribute('aria-label') for box in confirmations.all()]
                        assert all(label and label.startswith('Confirm ') and ' for ' in label for label in names)
                        assert len(set(names)) == len(names), 'Power confirmations must identify their action'
                        box = page.get_by_role('checkbox', disabled=False).first
                        label = box.locator('..')
                        assert label.bounding_box()['height'] >= 44
                        form = box.locator('xpath=ancestor::form')
                        assert not form.evaluate('(e) => e.checkValidity()')
                        box.focus()
                        page.keyboard.press('Space')
                        expect(box).to_be_checked()
                        assert form.evaluate('(e) => e.checkValidity()')
                        page.keyboard.press('Space')
                    if page.locator('#confirm-name').count():
                        expect(page.locator('label[for=confirm-name] code')).to_have_text(page.locator('h1').inner_text())
                        expect(page.locator('#confirm-name')).to_have_attribute('autocapitalize', 'none')
                        expect(page.locator('.danger-zone')).to_contain_text('Access groups are kept')
                    if name in ['detail-needs-agent', 'detail-frozen-agent']:
                        expect(page.get_by_role('button', name='Restart guest', exact=True)).to_be_disabled()
                        expect(page.locator('#restart-requirement')).to_contain_text('connected guest agent' if name == 'detail-needs-agent' else 'Thaw guest filesystems')
                        expect(page.get_by_role('button', name='Shut down', exact=True)).to_be_enabled()
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
                    expect(page.locator('#form-errors')).to_be_focused()
                    for field in ['max_cpus', 'disk0_size_gib']:
                        expect(page.locator('#' + field)).to_have_attribute('aria-invalid', 'true')
                        assert 'error-' + field in page.locator('#' + field).get_attribute('aria-describedby')
                        page.locator('#form-errors a[href="#' + field + '"]').click()
                        expect(page.locator('#' + field)).to_be_focused()
                    assert page.locator('#name').get_attribute('aria-invalid') is None
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
                context = browser_context(browser, viewport={'width': width, 'height': 1000}, color_scheme=theme)
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
                    if tab == 'metrics':
                        expect(page.locator('svg.chart')).to_have_count(5)
                        expect(page.locator('[data-metric="nic-rx"] .chart-empty')).to_have_text('No samples over the last 1h.')
                        assert 'undefined' not in page.locator('#metric-history').inner_text()
                        chart_keys(page, '.history-chart:has(svg.chart)', 'svg.chart .line')
                        if args.forced_colors:
                            selected_range = page.locator('.ranges [aria-pressed=true]')
                            assert selected_range.evaluate('(e) => parseFloat(getComputedStyle(e).outlineWidth) >= 2')
                        chart = page.locator('svg.chart').first
                        chart.focus()
                        page.keyboard.press('Home')
                        expect(page.locator('.history-chart .readout').first).to_contain_text('125 mcores')
                        page.keyboard.press('End')
                        expect(page.locator('.history-chart .readout').first).to_contain_text('188 mcores')
                        for svg in page.locator('svg.chart').all():
                            assert svg.evaluate('''svg => {
                                const box = svg.getBoundingClientRect();
                                const labels = [...svg.querySelectorAll('.tick')].map(t => t.getBoundingClientRect());
                                return labels.every(a => a.left >= box.left && a.right <= box.right + 1)
                                    && labels.every((a, i) => labels.slice(i + 1).every(b =>
                                        a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top));
                            }'''), 'Chart labels must fit without overlapping'
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
            long_content_checks(browser, width, theme)
    timezone_checks(browser)
    metric_focus_checks(browser)
    bmc_state_checks(browser, 1440, 'dark')
    bmc_state_checks(browser, 390, 'light')
    bmc_state_checks(browser, 320, 'dark')
    console_state_checks(browser, 1440, 'dark')
    console_state_checks(browser, 390, 'light')
    console_state_checks(browser, 320, 'dark')
    if not args.states_only:
        long_content_checks(browser, 320, 'light', height=320, scale=2)
    browser.close()
mode = 'state/lifecycle' if args.states_only else 'layout and state/lifecycle'
print(f'{args.engine}: Dashboard/BMC {mode} checks passed.')
