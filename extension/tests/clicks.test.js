// Unit tests for clicks.js, run by scripts/check-extension.sh with `gjs -m`. Tests run one after
// another (top-level await), and the process exits once at the end with the failure count, so a
// failing or throwing test can never leave exit code 0.
import System from 'system';
import {PAGE_ITEM_IDS, isAppIndicatorRole, openOnPrimaryPress} from '../statsbar@matrixi.github.io/clicks.js';

const PRIMARY = 1;
const MIDDLE = 2;
const SECONDARY = 3;
const tests = [];
const test = (name, body) => tests.push([name, body]);

function equal(actual, expected, what) {
    const a = JSON.stringify(actual);
    const e = JSON.stringify(expected);
    if (a !== e)
        throw new Error(`${what}: expected ${e}, got ${a}`);
}

/** A button as ubuntu-appindicators builds it: `_indicator` with `id` and `open`, and a menu. */
function fakeButton(id, {withOpen = true, openError = null, supportsActivation} = {}) {
    const calls = {open: [], menuClosed: 0};
    const indicator = {id, supportsActivation};
    if (withOpen) {
        indicator.open = async (x, y, time) => {
            calls.open.push([x, y, time]);
            if (openError)
                throw openError;
        };
    }
    const button = {_indicator: indicator, menu: {close: () => { calls.menuClosed += 1; }}};
    return {button, calls};
}

const press = (button) => ({button, x: 120, y: 8, time: 4242});
/** onError for presses that must not fail. It records the calls, which noErrors() then checks. */
const errors = [];
const recordError = (error) => errors.push(String(error));
const mustNotClose = () => { throw new Error('menu closed'); };

async function noErrors() {
    // open() settles asynchronously: a macrotask lets its .catch(onError) run first.
    await new Promise((resolve) => setTimeout(resolve, 0));
    equal(errors.splice(0), [], 'onError calls');
}

test('the page items are CPU and Memory only', () => {
    equal([...PAGE_ITEM_IDS].sort(), ['statsbar-cpu', 'statsbar-memory'], 'ids');
});

test('appindicator roles are recognised and others are not', () => {
    equal(isAppIndicatorRole('appindicator-:1.341@/StatusNotifierItem'), true, 'appindicator role');
    equal(isAppIndicatorRole('quickSettings'), false, 'shell role');
    equal(isAppIndicatorRole(undefined), false, 'no role');
});

for (const id of ['statsbar-cpu', 'statsbar-memory']) {
    test(`a primary press on ${id} closes the menu and opens the page at the press`, async () => {
        const {button, calls} = fakeButton(id);
        equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, recordError), 'opened', 'outcome');
        equal(calls.open, [[120, 8, 4242]], 'open calls');
        equal(calls.menuClosed, 1, 'menu closes');
        await noErrors();
    });
}

for (const id of ['statsbar-hub', 'statsbar-battery', 'statsbar-fan', 'statsbar-network', 'org.other.App']) {
    test(`a primary press on ${id} is left to the menu`, async () => {
        const {button, calls} = fakeButton(id);
        equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, recordError), 'ignored', 'outcome');
        equal(calls.open, [], 'open calls');
        equal(calls.menuClosed, 0, 'menu untouched');
        await noErrors();
    });
}

for (const [name, which] of [['secondary', SECONDARY], ['middle', MIDDLE]]) {
    test(`a ${name} press on CPU is left alone`, async () => {
        const {button, calls} = fakeButton('statsbar-cpu');
        equal(openOnPrimaryPress(button, press(which), PRIMARY, recordError), 'ignored', 'outcome');
        equal(calls.open, [], 'open calls');
        equal(calls.menuClosed, 0, 'menu untouched');
        await noErrors();
    });
}

test('a button without _indicator (legacy tray icon, or a renamed field) is ignored', async () => {
    equal(openOnPrimaryPress({menu: {close: mustNotClose}}, press(PRIMARY), PRIMARY, recordError), 'ignored', 'outcome');
    await noErrors();
});

test('our item without open() is unsupported and the menu is untouched', async () => {
    const {button, calls} = fakeButton('statsbar-cpu', {withOpen: false});
    equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, recordError), 'unsupported', 'outcome');
    equal(calls.menuClosed, 0, 'menu untouched');
    await noErrors();
});

test('our item that answered Activate with UnknownMethod is left to the menu', async () => {
    // ubuntu-appindicators sets supportsActivation = false then, and opens the menu instead.
    const {button, calls} = fakeButton('statsbar-cpu', {supportsActivation: false});
    equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, recordError), 'ignored', 'outcome');
    equal(calls.open, [], 'open calls');
    equal(calls.menuClosed, 0, 'menu untouched');
    await noErrors();
});

test('our item that supports activation opens the page', async () => {
    const {button, calls} = fakeButton('statsbar-memory', {supportsActivation: true});
    equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, recordError), 'opened', 'outcome');
    equal(calls.open, [[120, 8, 4242]], 'open calls');
    await noErrors();
});

test('a failed open is handed to onError', async () => {
    const {button} = fakeButton('statsbar-memory', {openError: new Error('activate failed')});
    let reported;
    const reportedOnce = new Promise((resolve) => { reported = resolve; });
    equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, reported), 'opened', 'outcome');
    const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error('onError never called')), 2000));
    const error = await Promise.race([reportedOnce, timeout]);
    equal(error.message, 'activate failed', 'error');
});

let failures = 0;
for (const [name, body] of tests) {
    try {
        await body();
        print(`ok - ${name}`);
    } catch (error) {
        failures += 1;
        print(`FAIL - ${name}: ${error.message}`);
    }
}
print(failures === 0 ? `clicks: all ${tests.length} passed` : `clicks: ${failures} of ${tests.length} failed`);
System.exit(failures === 0 ? 0 : 1);
