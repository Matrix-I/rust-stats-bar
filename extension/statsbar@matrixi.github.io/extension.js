// Makes a single primary click on StatsBar's CPU or Memory icon open its page (ADR 0001).
// ubuntu-appindicators@ubuntu.com draws the icons and opens the menu on a single click; this
// connects a button-press-event handler, which runs before its vfunc, and stops the event for
// those two items only. No timers, no polling, no D-Bus of its own. The logic is in clicks.js
// and hooks.js; this file only hands them the Shell's objects.
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension, InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';

import {openOnPrimaryPress} from './clicks.js';
import {StatusAreaHooks, makePressHandler} from './hooks.js';

export default class StatsBarSingleClick extends Extension {
    enable() {
        this._hooks = new StatusAreaHooks({
            panel: Main.panel,
            injections: new InjectionManager(),
            onPress: makePressHandler({
                decide: openOnPrimaryPress,
                primaryButton: Clutter.BUTTON_PRIMARY,
                stop: Clutter.EVENT_STOP,
                propagate: Clutter.EVENT_PROPAGATE,
                logError,
                warn: (message) => console.warn(message),
            }),
        });
        this._hooks.enable();
    }

    disable() {
        this._hooks.disable();
        this._hooks = null;
    }
}
