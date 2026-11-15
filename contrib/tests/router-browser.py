#!/usr/bin/env python3
"""Exercise the actual test-only dashboard router; configuration arrives on stdin.

No HTTP interception, guest, cluster or production credentials. The Rust ignored
test seeds opaque fixture sessions directly and hosts fake VM Pod upstreams.
"""
import argparse
import json
from pathlib import Path
import sys
import time
from urllib.parse import urlparse
from playwright.sync_api import expect, sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--engine', choices=['chromium', 'firefox'], default='chromium')
parser.add_argument('--output', type=Path, default=Path('target/ui-router'))
args = parser.parse_args()
config = json.load(sys.stdin)
origin = config['origin']
args.output = args.output / args.engine
args.output.mkdir(parents=True, exist_ok=True)
errors, violations, requests = [], [], []


def context(browser, cookie):
    ctx = browser.new_context(viewport={'width': 1280, 'height': 900}, color_scheme='dark')
    ctx.add_cookies([{'name': 'qrab-dashboard-dev', 'value': cookie, 'url': origin,
                     'httpOnly': True, 'secure': False, 'sameSite': 'Lax'}])
    ctx.add_init_script("document.addEventListener('securitypolicyviolation', e => console.error('CSP:' + e.violatedDirective + ':' + e.blockedURI));")
    return ctx


def page_in(ctx):
    page = ctx.new_page()
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.on('console', lambda message: violations.append(message.text) if message.text.startswith('CSP:') else None)
    page.on('request', lambda request: requests.append(request.url))
    return page


def visit(page, path, status=200):
    response = page.goto(origin + path)
    assert response.status == status, (path, response.status)
    assert "default-src 'none'" in response.headers.get('content-security-policy', '')
    assert response.headers.get('cache-control') == 'no-store'
    assert not any(token in response.text() for token in ['id-token-of-alice', 'id-token-of-stale', 'client-secret-never-sent', 'service-account-token'])
    if path == '/':
        icon = page.request.get(origin + '/assets/favicon.svg')
        assert icon.status == 200 and icon.headers.get('content-type', '').startswith('image/svg+xml')
        assert '<svg' in icon.text()


def capture(page, name):
    page.evaluate('window.scrollTo(0, 0)')
    page.screenshot(path=str(args.output / f'{name}.png'), full_page=True)


with sync_playwright() as p:
    options = {'headless': True}
    if args.engine == 'chromium':
        options.update(executable_path='/opt/google/chrome/chrome', args=['--no-sandbox'])
    browser = getattr(p, args.engine).launch(**options)
    admin = context(browser, config['admin'])
    page = page_in(admin)
    page.clock.install()
    for path in ['/', '/vms/qrab/a', '/images', '/launch']:
        visit(page, path)
        assert page.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--surface').trim()")
    page.locator('#name').fill('validation-fixture')
    page.locator('#cpus').fill('2')
    page.locator('#launch-limits summary').click()
    page.locator('#max_cpus').fill('1')
    page.locator('#user_data').fill('DO_NOT_ECHO_PRIVATE_USER_DATA')
    page.get_by_role('button', name='Launch VM', exact=True).click()
    expect(page.get_by_role('heading', name='Check the form')).to_be_visible()
    expect(page.locator('#form-errors')).to_be_focused()
    expect(page.locator('#max_cpus')).to_have_attribute('aria-invalid', 'true')
    assert 'error-max_cpus' in page.locator('#max_cpus').get_attribute('aria-describedby')
    page.locator('#form-errors a[href="#max_cpus"]').click()
    expect(page.locator('#max_cpus')).to_be_focused()
    assert page.locator('#user_data').input_value() == ''
    assert 'DO_NOT_ECHO_PRIVATE_USER_DATA' not in page.content()
    visit(page, '/vms/qrab/a/bmc')
    expect(page.locator('#dashboard')).to_be_visible()
    for tab in ['overview', 'metrics', 'events', 'agent', 'config']:
        page.locator('#tab-' + tab).click()
    expect(page.locator('#config')).to_contain_text('browser-vm')
    assert page.locator('#agent script, #config script').count() == 0
    page.locator('#tab-metrics').click()
    expect(page.locator('svg.chart')).not_to_have_count(0)
    chart = page.locator('svg.chart').first
    chart.focus()
    page.keyboard.press('Home')
    readout = page.locator('.history-chart .readout').first
    selected = readout.inner_text()
    page.clock.fast_forward(31000)
    expect(chart).to_be_focused()
    expect(readout).to_have_text(selected)
    capture(page, 'actual-metrics')
    page.locator('#connect').click()
    expect(page.locator('#screen canvas')).to_have_count(1)
    peer = page_in(admin)
    visit(peer, '/')
    peer.get_by_role('button', name='Sign out', exact=True).click()
    expect(peer.get_by_role('link', name='Sign in with Kanidm →')).to_be_visible()
    page.bring_to_front()
    expect(page.locator('#login')).to_be_visible(timeout=10000)
    expect(page.locator('#screen canvas')).to_have_count(0)
    capture(page, 'peer-logout')
    admin.close()
    assert not errors, errors
    assert not violations, violations
    print(f'{args.engine}: actual-router admin routes, CSP and peer logout passed', flush=True)

    viewer = context(browser, config['viewer'])
    page = page_in(viewer)
    visit(page, '/vms/qrab/a/console', 403)
    expect(page.get_by_role('heading', name="Access isn't available")).to_be_visible()
    visit(page, '/vms/qrab/a/bmc')
    expect(page.locator('#dashboard')).to_be_visible()
    assert page.locator('[data-agent], #power, #connect').count() == 0
    page.locator('#tab-events').click()
    expect(page.locator('#events li')).not_to_have_count(0)
    capture(page, 'actual-viewer')
    viewer.close()
    assert not errors, errors
    assert not violations, violations
    print(f'{args.engine}: actual-router viewer role passed', flush=True)

    expiry = context(browser, config['expiry'])
    page = page_in(expiry)
    visit(page, '/vms/qrab/a/bmc')
    expect(page.locator('#dashboard')).to_be_visible()
    page.locator('#connect').click()
    expect(page.locator('#screen canvas')).to_have_count(1)
    # Real server time, not a mocked browser clock. For the optional 15-minute
    # run, rotate panels and sample DOM counts while the same session lives.
    nodes = []
    while time.time() < config['expires'] - 5:
        for tab in ['overview', 'events', 'agent', 'config', 'metrics']:
            page.locator('#tab-' + tab).click()
        expect(page.locator('.history-status')).to_be_visible()
        expect(page.locator('.history-status')).not_to_have_text('Loading…')
        nodes.append(page.locator('*').count())
        remaining = config['expires'] - time.time()
        print(f'{args.engine}: session live, {remaining:.0f}s to expiry, {nodes[-1]} DOM nodes', flush=True)
        page.wait_for_timeout(min(15000, max(100, remaining * 1000 - 5000)))
    expect(page.locator('#login')).to_be_visible(timeout=15000)
    expect(page.locator('#screen canvas')).to_have_count(0)
    before = len(requests)
    page.wait_for_timeout(4000)
    assert len(requests) == before, 'Expired UI kept making background requests'
    assert max(nodes, default=0) - min(nodes, default=0) < 100, nodes
    capture(page, 'actual-expiry')
    expiry.close()
    browser.close()
assert not errors, errors
assert not violations, violations
assert all(urlparse(url).netloc == urlparse(origin).netloc for url in requests), 'Browser left its dashboard origin'
print(f'{args.engine}: real-router/CSP/lifecycle checks passed', flush=True)
