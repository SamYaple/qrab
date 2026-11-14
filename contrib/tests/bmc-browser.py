#!/usr/bin/env python3
"""Real OIDC/noVNC acceptance against an isolated IdP and the RAM-only guest.

Requires Playwright and a private JSON credentials file: {username, password}.
Never use a production administrator account or save browser session storage.
"""
import argparse
import json
from pathlib import Path
import sys
import time

from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--origin', required=True)
parser.add_argument('--credentials', type=Path, required=True)
parser.add_argument('--serial-log', type=Path, required=True)
parser.add_argument('--browser', required=True)
parser.add_argument('--output', type=Path, default=Path('target/bmc-browser'))
a = parser.parse_args()
credentials = json.loads(a.credentials.read_text())
serial_offset = a.serial_log.stat().st_size

# VNC forwards hardware scancodes. Synthetic keyboard.type('A') omits a real
# Shift press, so send physical key combinations as an actual keyboard does.
shifted = dict(zip('!@#$%^&*()', ['Digit1','Digit2','Digit3','Digit4','Digit5','Digit6','Digit7','Digit8','Digit9','Digit0']))
shifted.update({'_':'Minus','+':'Equal',':':'Semicolon','"':'Quote','>':'Period','<':'Comma','?':'Slash','|':'Backslash','{':'BracketLeft','}':'BracketRight','~':'Backquote'})

def type_keys(page, text):
    for char in text:
        key = 'Shift+Key' + char if char.isascii() and char.isupper() else 'Shift+' + shifted[char] if char in shifted else char
        page.keyboard.press(key)

def command(page, text):
    type_keys(page, text)
    page.keyboard.press('Enter')

def serial_marker(marker):
    for _ in range(50):
        if marker in a.serial_log.read_bytes()[serial_offset:].decode(errors='replace'):
            return
        time.sleep(.1)
    raise AssertionError('Guest did not report ' + marker)

try:
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=a.browser, headless=True, args=['--no-sandbox'])
        try:
            context = browser.new_context(ignore_https_errors=True, viewport={'width':1280,'height':900})
            page = context.new_page()
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.goto(a.origin)
            page.get_by_role('link', name='Sign in with Kanidm').click()
            page.locator('input[name="username"]').fill(credentials['username'])
            page.locator('form#login button[type="submit"]').click()
            page.locator('input[name="password"][type="password"]').fill(credentials['password'])
            page.locator('form#login button[type="submit"]').click()
            try:
                page.get_by_role('button', name='Proceed', exact=True).click(timeout=5000)
            except Exception:
                pass  # Existing consent may redirect directly.
            page.wait_for_url(a.origin.rstrip('/') + '/', timeout=20000)
            page.get_by_role('toolbar', name='Console controls').get_by_role('button', name='Connect console', exact=True).click()
            page.locator('#console-status', has_text='Connected · Keyboard and mouse enabled').wait_for(timeout=15000)
            a.output.mkdir(parents=True, exist_ok=True)
            page.screenshot(path=str(a.output / 'connected.png'), full_page=True)
            canvas = page.locator('#screen canvas')
            canvas.click(position={'x':100,'y':100})
            command(page, 'echo QRAB_BROWSER_KEYBOARD_OK > /dev/ttyS0')
            serial_marker('QRAB_BROWSER_KEYBOARD_OK')
            command(page, 'timeout 6 cat /dev/input/event1 > /tmp/mouse &')
            command(page, 'echo QRAB_MOUSE_CAPTURE_STARTED > /dev/ttyS0')
            serial_marker('QRAB_MOUSE_CAPTURE_STARTED')
            box = canvas.bounding_box()
            page.mouse.move(box['x']+80, box['y']+80)
            page.mouse.move(box['x']+240, box['y']+180, steps=20)
            page.mouse.move(box['x']+120, box['y']+240, steps=20)
            time.sleep(.5)
            command(page, 'test -s /tmp/mouse && echo QRAB_BROWSER_MOUSE_OK > /dev/ttyS0')
            serial_marker('QRAB_BROWSER_MOUSE_OK')
            command(page, 'test "$(ls /sys/class/net)" = lo && echo QRAB_GUEST_NO_NIC_OK > /dev/ttyS0')
            serial_marker('QRAB_GUEST_NO_NIC_OK')
            print('PASS real Kanidm OIDC, video, physical keyboard, USB mouse and no guest NIC')
            # Power state comes from Redfish Systems/1 through the BMC's /redfish proxy.
            page.get_by_role('group', name='Power').get_by_role('button', name='Pause', exact=True).click()
            page.wait_for_function("async () => (await (await fetch('/redfish/v1/Systems/1')).json()).PowerState === 'Paused'")
            assert page.request.get(a.origin + '/readyz').status == 200
            page.get_by_role('group', name='Power').get_by_role('button', name='Resume', exact=True).click()
            page.wait_for_function("async () => (await (await fetch('/redfish/v1/Systems/1')).json()).PowerState === 'On'")
            page.screenshot(path=str(a.output / 'input-verified.png'), full_page=True)
            print('PASS authenticated pause/resume; paused VM stays reachable')
            # Another tab signs out while this tab holds an active VNC connection.
            other = context.new_page()
            other.goto(a.origin)
            other.get_by_role('toolbar', name='Console controls').get_by_role('button', name='Connect console', exact=True).click()
            other.locator('#console-status', has_text='Connected · Keyboard and mouse enabled').wait_for()
            page.get_by_role('button', name='Sign out', exact=True).click()
            page.get_by_role('link', name='Sign in with Kanidm').wait_for()
            other.get_by_role('link', name='Sign in with Kanidm').wait_for(timeout=5000)
            assert other.request.get(a.origin + '/api/session').status == 401
            assert not errors, errors
            print('PASS logout revokes the session and active consoles in other tabs')
        finally:
            browser.close()
except Exception as error:
    # Playwright diagnostics can include input values. Suppress known secrets.
    message = str(error)
    for value in credentials.values():
        message = message.replace(str(value), '[redacted]')
    print(message[-1800:], file=sys.stderr)
    sys.exit(1)
