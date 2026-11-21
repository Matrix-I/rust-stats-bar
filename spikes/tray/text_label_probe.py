# Spike S3: StatusNotifierItems with an XAyatanaLabel, so GNOME draws the text in its panel font.
# Registers six variants: two text-only labels, three icon + number items and one combined text
# item. Exits after 300 s or on SIGTERM.
from gi.repository import Gio, GLib

ITEM_XML = """
<node>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconPixmap" type="a(iiay)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <property name="WindowId" type="i" access="read"/>
    <property name="IconThemePath" type="s" access="read"/>
    <property name="OverlayIconName" type="s" access="read"/>
    <property name="OverlayIconPixmap" type="a(iiay)" access="read"/>
    <property name="AttentionIconName" type="s" access="read"/>
    <property name="AttentionIconPixmap" type="a(iiay)" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="XAyatanaLabel" type="s" access="read"/>
    <property name="XAyatanaLabelGuide" type="s" access="read"/>
  </interface>
</node>"""

def main():
    # (theme icon or '' for none, label, widest label it may take)
    items = [
        ("", "CPU 37%", "CPU 100%"),
        ("", "↑12K ↓1.5M", "↑999K ↓999M"),
        ("computer-chip-symbolic", "37%", "100%"),
        ("drive-harddisk-solidstate-symbolic", "36%", "100%"),
        ("network-transmit-receive-symbolic", "1.5M", "999M"),
        ("", "CPU 37  RAM 36  ↓1.5M", "CPU 100  RAM 100  ↓999M"),
    ]
    loop = GLib.MainLoop()
    conns = []
    for index, (icon, label, guide) in enumerate(items):
        # One private connection per item, so each has its own unique name and object path.
        address = Gio.dbus_address_get_for_bus_sync(Gio.BusType.SESSION, None)
        conn = Gio.DBusConnection.new_for_address_sync(
            address,
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
            None, None)
        node = Gio.DBusNodeInfo.new_for_xml(ITEM_XML)
        props = {
            'Category': GLib.Variant('s', 'Hardware'),
            'Id': GLib.Variant('s', f'statsbar-spike-text-{index}'),
            'Title': GLib.Variant('s', label),
            'Status': GLib.Variant('s', 'Active'),
            'IconName': GLib.Variant('s', icon),
            'IconPixmap': GLib.Variant('a(iiay)', [] if icon else [(1, 1, bytes(4))]),
            'ItemIsMenu': GLib.Variant('b', False),
            'Menu': GLib.Variant('o', '/NO_DBUSMENU'),
            'WindowId': GLib.Variant('i', 0),
            'IconThemePath': GLib.Variant('s', ''),
            'OverlayIconName': GLib.Variant('s', ''),
            'OverlayIconPixmap': GLib.Variant('a(iiay)', []),
            'AttentionIconName': GLib.Variant('s', ''),
            'AttentionIconPixmap': GLib.Variant('a(iiay)', []),
            'ToolTip': GLib.Variant('(sa(iiay)ss)', ('', [], label, '')),
            'XAyatanaLabel': GLib.Variant('s', label),
            'XAyatanaLabelGuide': GLib.Variant('s', guide),
        }
        conn.register_object_with_closures2(
            '/StatusNotifierItem', node.interfaces[0], None,
            lambda _c, _s, _p, _i, name, props=props: props[name], None)
        conn.call_sync('org.kde.StatusNotifierWatcher', '/StatusNotifierWatcher',
                       'org.kde.StatusNotifierWatcher', 'RegisterStatusNotifierItem',
                       GLib.Variant('(s)', (conn.get_unique_name(),)), None,
                       Gio.DBusCallFlags.NONE, 5000, None)
        conns.append(conn)
        print('registered', conn.get_unique_name(), label, flush=True)
    GLib.timeout_add_seconds(300, loop.quit)
    loop.run()

main()
