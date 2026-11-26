// Which panel buttons get the press handler, and how it comes off again. Pure: the panel, the
// method injector and the press decision are passed in, so gjs can test it with fakes
// (extension/tests/hooks.test.js); extension.js passes the Shell's own.

import {isAppIndicatorRole} from './clicks.js';

/**
 * Connects `onPress` to every AppIndicator button on the panel: the ones there at `enable()` and
 * the ones `addToStatusArea` adds later. `disable()` takes it all off again.
 *
 * - `panel`: `statusArea` ({role: button}) and `addToStatusArea(role, indicator, ...)`, as
 *   Main.panel.
 * - `injections`: `overrideMethod(object, name, (original) => replacement)` and `clear()`, as
 *   the Shell's InjectionManager.
 * - `onPress(button, event)`: what the button-press-event handler returns.
 */
export class StatusAreaHooks {
    constructor({panel, injections, onPress}) {
        this._panel = panel;
        this._injections = injections;
        this._onPress = onPress;
        /** Each hooked button → its [press, destroy] handler ids; null while disabled. */
        this._handlers = null;
    }

    enable() {
        this._handlers = new Map();
        const hook = (button) => this._hook(button);
        this._injections.overrideMethod(this._panel, 'addToStatusArea', (original) =>
            function (role, indicator, ...rest) {
                const result = original.call(this, role, indicator, ...rest);
                if (isAppIndicatorRole(role) && indicator)
                    hook(indicator);
                return result;
            });
        for (const [role, button] of Object.entries(this._panel.statusArea)) {
            if (isAppIndicatorRole(role) && button)
                this._hook(button);
        }
    }

    disable() {
        this._injections.clear();
        for (const [button, ids] of this._handlers) {
            for (const id of ids)
                button.disconnect(id);
        }
        this._handlers = null;
    }

    _hook(button) {
        // addIconToPanel re-adds the same button when the tray position changes.
        if (!this._handlers || this._handlers.has(button))
            return;
        const press = button.connect('button-press-event', (_actor, event) => this._onPress(button, event));
        const destroy = button.connect('destroy', () => this._handlers?.delete(button));
        this._handlers.set(button, [press, destroy]);
    }
}

/**
 * The button-press-event handler: `decide` (clicks.js openOnPrimaryPress) picks the outcome, the
 * event stops only when the page opened, and no error escapes into the Shell. An item without
 * `open()` is warned about once per handler.
 */
export function makePressHandler({decide, primaryButton, stop, propagate, logError, warn}) {
    let warned = false;
    const onError = (error) => logError(error, 'statsbar: opening the page failed');
    return (button, event) => {
        const [x, y] = event.get_coords();
        let outcome;
        try {
            outcome = decide(button, {button: event.get_button(), x, y, time: event.get_time()},
                primaryButton, onError);
        } catch (error) {
            onError(error);
            return propagate;
        }
        if (outcome === 'unsupported' && !warned) {
            warned = true;
            warn('statsbar: the AppIndicator item has no open(); a single click opens the menu');
        }
        return outcome === 'opened' ? stop : propagate;
    };
}
