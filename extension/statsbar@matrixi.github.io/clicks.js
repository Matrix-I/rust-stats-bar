// What a press on an AppIndicator panel button does. Pure: no gi:// or resource:// imports, so
// gjs can test it with fakes (extension/tests/clicks.test.js).
//
// The button fields read here (`_indicator`, its `id` and `open`) belong to
// ubuntu-appindicators@ubuntu.com (indicatorStatusIcon.js, appIndicator.js), not to a public API.
// When they are missing, every press is left to that extension, which opens the menu as before.

/** The StatusNotifierItem ids whose page a single primary click opens (statsbard's tray ids). */
export const PAGE_ITEM_IDS = new Set(['statsbar-cpu', 'statsbar-memory']);

const ROLE_PREFIX = 'appindicator-';

/** Whether a Main.panel.statusArea role is one ubuntu-appindicators added. */
export function isAppIndicatorRole(role) {
    return typeof role === 'string' && role.startsWith(ROLE_PREFIX);
}

/**
 * Opens the item's page for a primary press on a page item, as ubuntu-appindicators does on a
 * double click (`_indicator.open`, which sends Activate with the activation token).
 * Returns 'opened' (the caller stops the event), 'ignored' (let it through), or 'unsupported'
 * (a page item without `open`; let it through and log once).
 */
export function openOnPrimaryPress(button, press, primaryButton, onError) {
    if (press.button !== primaryButton)
        return 'ignored';
    const indicator = button?._indicator;
    if (!indicator || !PAGE_ITEM_IDS.has(indicator.id))
        return 'ignored';
    // false after Activate failed with UnknownMethod (appIndicator.js): ubuntu-appindicators then
    // opens the menu on every click, so leave the press to it.
    if (indicator.supportsActivation === false)
        return 'ignored';
    if (typeof indicator.open !== 'function')
        return 'unsupported';
    button.menu?.close?.();
    // `open` is async (appIndicator.js:843), so its failure arrives as a rejection.
    Promise.resolve(indicator.open(press.x, press.y, press.time)).catch(onError);
    return 'opened';
}
