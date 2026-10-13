export interface Plural {
  one?: string;
  few?: string;
  many?: string;
  other: string;
}

export const en = {
  "tab.home": "Connection",
  "tab.settings": "Settings",

  "titlebar.openConfig": "Open config folder",
  "titlebar.theme": "Toggle theme",
  "titlebar.minimize": "Minimize",
  "titlebar.close": "Close to tray",
  "titlebar.closeHint": "Close to tray — routing keeps running",

  "app.loading": "Loading…",
  "update.available": "Version {version} is available",
  "update.install": "Update",

  "status.connecting": "Connecting…",
  "status.disconnecting": "Disconnecting…",
  "status.on": "Running",
  "status.onTun": "Running · TUN",
  "status.off": "Off",
  "status.allDirect": "all traffic goes direct",
  "status.nothingRouted": "nothing is routed through the server — all traffic goes direct",
  "status.routed": "{items} via {server} · the rest direct",
  "status.join": "{a} and {b}",
  "status.server": "server",
  "status.toggle": "Turn routing on",
  "count.apps": { one: "{n} app", other: "{n} apps" } as Plural,
  "count.domains": { one: "{n} domain", other: "{n} domains" } as Plural,

  "warn.proxyNotSet": "The core is running, but the system proxy is not set — traffic goes direct.",
  "warn.proxyOff":
    "The system proxy is off: only what you point at 127.0.0.1:{port} manually reaches Routeway.",

  "server.none": "No server configured",
  "server.edit": "Edit",
  "server.setup": "Set up",
  "server.title": "Server",
  "server.cancel": "Cancel",
  "server.pasteLink": "Paste a link",
  "server.parse": "Parse",
  "server.name": "Name",
  "server.namePlaceholder": "Estonia",
  "server.address": "Address",
  "server.port": "Port",
  "server.save": "Save server",
  "server.needAddress": "Enter the server address",
  "server.needUuid": "Enter the UUID",

  "vless.empty": "The link is empty",
  "vless.scheme": "The link must start with vless://",
  "vless.parse": "Could not parse the link",
  "vless.noUuid": "The link has no UUID",
  "vless.noAddress": "The link has no server address",
  "vless.badPort": "Invalid port",

  "health.checking": "Checking…",
  "health.ok": "Reachable",
  "health.slow": "Slow",
  "health.down": "No response",
  "health.modeTcp": "TCP to server",
  "health.modeTunnel": "through the tunnel",
  "health.ms": "{ms} ms",
  "health.avg": "avg {ms} ms",
  "health.loss": "loss {pct}%",

  "log.title": "Core log",
  "log.empty": "No events yet",
  "log.raw": "Raw",
  "log.readable": "Readable",
  "log.clear": "Clear",
  "log.started": "Core started",
  "log.startedIn": "Core started in {time}",
  "log.listening": "Listening on {target}",
  "log.interface": "Network: {name}",
  "log.failedApp": "{app} couldn’t connect to {host}",
  "log.failed": "Couldn’t connect to {host}",
  "log.viaProxy": "via server",
  "log.direct": "direct",
  "log.ports": "ports {ports}",
  "log.hintBlocked": "Looks blocked on the direct route. Add {what} to the server list.",
  "log.reason.timeout": "no response (timeout)",
  "log.reason.refused": "connection refused",
  "log.reason.reset": "connection reset",
  "log.reason.unreachable": "network unreachable",
  "log.reason.notFound": "domain not found",
  "log.reason.eof": "connection closed early",

  "settings.general": "General",
  "settings.routing": "Routing",
  "settings.language": "Language",
  "settings.tun": "TUN mode",
  "settings.tunHint":
    "Captures traffic from every app, including UDP and apps that ignore the proxy. Needs administrator rights.",
  "settings.proxy": "System proxy",
  "settings.proxyTunHint": "Not needed in TUN mode.",
  "settings.proxyHint": "Points system traffic at 127.0.0.1:{port}.",
  "settings.port": "Port",
  "settings.portHint": "Apps connect to 127.0.0.1:{port}, the probe uses {probe}.",
  "settings.portLocked": "Turn routing off to change the port",
  "settings.autostart": "Launch at login",

  "check.title": "Check",
  "check.run": "Check IP",
  "check.direct": "Direct",
  "check.server": "Via server",

  "rules.edit": "Edit as JSON",
  "rules.title": "Rules file",
  "rules.hint": "Apps and domains that go through the server, both lists in one file. Paste as many entries as you need — they are checked and cleaned up on save.",
  "rules.invalid": "Invalid JSON: {error}",
  "rules.notObject": "The file must be an object: { \"apps\": [], \"domains\": [] }",
  "rules.save": "Save",
  "rules.reveal": "Show in folder",

  "list.empty": "list is empty",
  "list.add": "Add",
  "list.remove": "Remove {item}",

  "domains.title": "Domains via server",
  "domains.rejected": "Doesn't look like a domain",
  "domains.empty1": "No domain goes through the server yet.",
  "domains.empty2": "Add a domain and only it will go through the tunnel.",
  "domains.footer": "A domain includes all its subdomains: {a} also covers {b}.",

  "apps.title": "Apps via server",
  "apps.rejected": "Doesn't look like a process name",
  "apps.empty1": "No app is bound to the server yet.",
  "apps.empty2": "Add a process name, for example {example}.",
  "apps.footer":
    "Process name ({name}) or full path ({path}). Works for apps whose traffic reaches Routeway: with the system proxy on, or for any app in TUN mode.",
};

export type Key = keyof typeof en;

export type Dict = { [K in Key]: (typeof en)[K] extends string ? string : Plural };
