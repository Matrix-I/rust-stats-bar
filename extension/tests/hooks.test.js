// Unit tests for hooks.js, run by scripts/check-extension.sh with `gjs -m`. Same shape as
// clicks.test.js: tests run one after another, and the process exits once at the end with the
// failure count, so a failing or throwing test can never leave exit code 0.
import System from 'system';
import {StatusAreaHooks, makePressHandler} from '../statsbar@matrixi.github.io/hooks.js';

const APPINDICATOR_ROLE = 'appindicator-:1.341@/StatusNotifierItem';
const tests = [];
const test = (name, body) => tests.push([name, body]);

function equal(actual, expected, what) {
    const a = JSON.stringify(actual);
    const e = JSON.stringify(expected);
    if (a !== e)
        throw new Error(`${what}: expected ${e}, got ${a}`);
}

/**
 * A panel button with GObject-like signals: `connect` returns a fresh id, `disconnect` of an id
 * that is not connected throws (GObject warns), and `destroy` runs its handlers and then drops
 * them all, as dispose does.
 */
function fakeButton() {
    let nextId = 1;
    const handlers = new Map();
    return {
        connect(signal, handler) {
            const id = nextId++;
            handlers.set(id, [signal, handler]);
            return id;
        },
        disconnect(id) {
            if (!handlers.delete(id))
                throw new Error(`disconnect of unknown handler ${id}`);
        },
        emit(signal, ...args) {
            return [...handlers.values()].filter(([s]) => s === signal).map(([, h]) => h(this, ...args));
        },
        destroy() {
            this.emit('destroy');
            handlers.clear();
        },
        count(signal) {
            return [...handlers.values()].filter(([s]) => s === signal).length;
        },
    };
}

/** Main.panel's two members that hooks.js uses; addToStatusArea records its calls. */
function fakePanel(statusArea = {}) {
    const added = [];
    return {
        statusArea,
        added,
        addToStatusArea(role, indicator, position, box) {
            added.push([role, position, box]);
            this.statusArea[role] = indicator;
            return `added ${role}`;
        },
    };
}

/** The Shell's InjectionManager, reduced to overrideMethod and clear. */
function fakeInjections() {
    let saved = [];
    return {
        overrideMethod(object, name, createOverride) {
            const original = object[name];
            saved.push([object, name, original]);
            object[name] = createOverride(original);
        },
        clear() {
            for (const [object, name, original] of saved.reverse())
                object[name] = original;
            saved = [];
        },
    };
}

/** StatusAreaHooks over fakes; onPress records each call and returns 'handled'. */
function setUp(statusArea) {
    const panel = fakePanel(statusArea);
    const presses = [];
    const hooks = new StatusAreaHooks({
        panel,
        injections: fakeInjections(),
        onPress: (button, event) => {
            presses.push([button, event]);
            return 'handled';
        },
    });
    return {panel, presses, hooks};
}

test('enable hooks the appindicator buttons already in the status area', () => {
    const cpu = fakeButton();
    const memory = fakeButton();
    const {presses, hooks} = setUp({[APPINDICATOR_ROLE]: cpu, 'appindicator-:1.342@/StatusNotifierItem': memory});
    hooks.enable();
    equal([cpu.count('button-press-event'), memory.count('button-press-event')], [1, 1], 'press handlers');
    equal(cpu.emit('button-press-event', 'event-1'), ['handled'], 'handler returns onPress');
    equal(presses.length, 1, 'onPress calls');
    equal(presses[0][0] === cpu && presses[0][1] === 'event-1', true, 'onPress gets the button and the event');
});

test('other roles and empty entries are not hooked', () => {
    const quickSettings = fakeButton();
    const {hooks} = setUp({quickSettings, [APPINDICATOR_ROLE]: null});
    hooks.enable();
    equal(quickSettings.count('button-press-event'), 0, 'press handlers');
});

test('a button added later is hooked, and addToStatusArea still runs with its arguments', () => {
    const {panel, hooks} = setUp();
    hooks.enable();
    const cpu = fakeButton();
    const result = panel.addToStatusArea(APPINDICATOR_ROLE, cpu, 1, 'right');
    equal(result, `added ${APPINDICATOR_ROLE}`, 'return value');
    equal(panel.added, [[APPINDICATOR_ROLE, 1, 'right']], 'original calls');
    equal(cpu.count('button-press-event'), 1, 'press handlers');
});

test('a non-appindicator button added later is not hooked', () => {
    const {panel, hooks} = setUp();
    hooks.enable();
    const other = fakeButton();
    panel.addToStatusArea('tiling-assistant@leleat-on-github', other);
    equal(other.count('button-press-event'), 0, 'press handlers');
});

test('the same button added twice gets one handler', () => {
    const cpu = fakeButton();
    const {panel, hooks} = setUp({[APPINDICATOR_ROLE]: cpu});
    hooks.enable();
    panel.addToStatusArea(APPINDICATOR_ROLE, cpu, 1);
    panel.addToStatusArea(APPINDICATOR_ROLE, cpu, 2);
    equal(cpu.count('button-press-event'), 1, 'press handlers');
    equal(cpu.count('destroy'), 1, 'destroy handlers');
});

test('a destroyed button is dropped and not disconnected again on disable', () => {
    const cpu = fakeButton();
    const memory = fakeButton();
    const {hooks} = setUp({[APPINDICATOR_ROLE]: cpu, 'appindicator-:1.342@/StatusNotifierItem': memory});
    hooks.enable();
    cpu.destroy();
    hooks.disable();
    equal(memory.count('button-press-event') + memory.count('destroy'), 0, 'the live button is cleaned up');
});

test('disable disconnects every handler and restores addToStatusArea', () => {
    const cpu = fakeButton();
    const {panel, hooks} = setUp({[APPINDICATOR_ROLE]: cpu});
    const original = panel.addToStatusArea;
    hooks.enable();
    const later = fakeButton();
    panel.addToStatusArea('appindicator-:1.343@/StatusNotifierItem', later);
    hooks.disable();
    equal([cpu.count('button-press-event'), cpu.count('destroy')], [0, 0], 'existing button handlers');
    equal([later.count('button-press-event'), later.count('destroy')], [0, 0], 'later button handlers');
    equal(panel.addToStatusArea === original, true, 'addToStatusArea restored');
    const afterDisable = fakeButton();
    panel.addToStatusArea('appindicator-:1.344@/StatusNotifierItem', afterDisable);
    equal(afterDisable.count('button-press-event'), 0, 'no hook after disable');
});

test('enable after disable hooks again, once', () => {
    const cpu = fakeButton();
    const {panel, hooks} = setUp({[APPINDICATOR_ROLE]: cpu});
    hooks.enable();
    hooks.disable();
    hooks.enable();
    equal(cpu.count('button-press-event'), 1, 'existing button press handlers');
    const later = fakeButton();
    panel.addToStatusArea('appindicator-:1.343@/StatusNotifierItem', later);
    equal(later.count('button-press-event'), 1, 'later button press handlers');
});

/** A Clutter button event reduced to the three getters makePressHandler reads. */
const fakeEvent = {get_button: () => 1, get_coords: () => [120, 8], get_time: () => 4242};

/** makePressHandler over a fake decide() that returns `outcome` (or throws `error`). */
function pressHandler({outcome = 'opened', error = null} = {}) {
    const record = {decided: [], logged: [], warned: []};
    const handler = makePressHandler({
        decide: (button, press, primaryButton, onError) => {
            record.decided.push([button, press, primaryButton]);
            record.onError = onError;
            if (error)
                throw error;
            return outcome;
        },
        primaryButton: 1,
        stop: 'STOP',
        propagate: 'PROPAGATE',
        logError: (e, message) => record.logged.push([String(e), message]),
        warn: (message) => record.warned.push(message),
    });
    return {handler, record};
}

test('an opened page stops the event, with the press read from the event', () => {
    const {handler, record} = pressHandler({outcome: 'opened'});
    equal(handler('cpu', fakeEvent), 'STOP', 'return');
    equal(record.decided, [['cpu', {button: 1, x: 120, y: 8, time: 4242}, 1]], 'decide calls');
});

test('an ignored press propagates', () => {
    const {handler, record} = pressHandler({outcome: 'ignored'});
    equal(handler('cpu', fakeEvent), 'PROPAGATE', 'return');
    equal(record.warned, [], 'warnings');
});

test('an unsupported item propagates and warns once', () => {
    const {handler, record} = pressHandler({outcome: 'unsupported'});
    equal([handler('cpu', fakeEvent), handler('cpu', fakeEvent)], ['PROPAGATE', 'PROPAGATE'], 'returns');
    equal(record.warned.length, 1, 'warnings');
});

test('a throwing decide propagates and is logged', () => {
    const {handler, record} = pressHandler({error: new Error('boom')});
    equal(handler('cpu', fakeEvent), 'PROPAGATE', 'return');
    equal(record.logged, [['Error: boom', 'statsbar: opening the page failed']], 'logged');
});

test('a failed open reported through onError is logged', () => {
    const {handler, record} = pressHandler({outcome: 'opened'});
    handler('cpu', fakeEvent);
    record.onError(new Error('activate failed'));
    equal(record.logged, [['Error: activate failed', 'statsbar: opening the page failed']], 'logged');
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
print(failures === 0 ? `hooks: all ${tests.length} passed` : `hooks: ${failures} of ${tests.length} failed`);
System.exit(failures === 0 ? 0 : 1);
