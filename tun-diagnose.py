#!/usr/bin/env python3
"""Read-only TUN snapshot; run while the failure is happening."""
import datetime
import json
import pathlib
import subprocess
import urllib.request

BASE = pathlib.Path.home() / "Library/Application Support/com.zeytun.app/core/runtime/config/zeytun-core/Default.json"
OUT = pathlib.Path.home() / "Desktop" / ("zeytun-tun-" + datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + ".txt")


def command(*args):
    try:
        p = subprocess.run(args, capture_output=True, text=True, timeout=8)
        return f"exit={p.returncode}\n{p.stdout}{p.stderr}"
    except (OSError, subprocess.TimeoutExpired) as e:
        return str(e)


def api(path):
    try:
        req = urllib.request.Request("http://127.0.0.1:9090" + path, headers={"Authorization": "Bearer SECRET"})
        return json.load(urllib.request.urlopen(req, timeout=4))
    except Exception as e:
        return {"error": str(e)}


try:
    config = json.loads(BASE.read_text())
    inbounds = [{k: i.get(k) for k in ("tag", "type", "interface_name", "auto_route", "address", "mtu")} for i in config.get("inbounds", [])]
except (OSError, ValueError) as e:
    inbounds = {"error": str(e)}

snapshot = api("/connections")
connections = snapshot
if isinstance(snapshot, dict) and isinstance(snapshot.get("connections"), list):
    connections = []
    for c in snapshot["connections"]:
        if not isinstance(c, dict):
            continue
        m = c.get("metadata") or {}
        connections.append({"network": m.get("network"), "inbound": m.get("type"), "host": m.get("host"), "destinationIP": m.get("destinationIP"), "destinationPort": m.get("destinationPort"), "rule": c.get("rule"), "chains": c.get("chains")})
sections = {
    "time": datetime.datetime.now().astimezone().isoformat(),
    "config inbounds": inbounds,
    "core /configs": api("/configs"),
    "core /connections (process/source omitted)": connections,
    "default routes": command("netstat", "-rn", "-f", "inet"),
    "interfaces": command("ifconfig"),
    "DNS": command("scutil", "--dns"),
    "route to test IP": command("route", "-n", "get", "1.1.1.1"),
    "core process": command("pgrep", "-fl", "zeytun-core run"),
    "direct HTTPS": command("curl", "--noproxy", "*", "-sS", "-m", "8", "-o", "/dev/null", "-w", "status=%{http_code} remote=%{remote_ip}\n", "https://www.gstatic.com/generate_204"),
    "mixed 6060 HTTPS": command("curl", "--proxy", "http://127.0.0.1:6060", "-sS", "-m", "8", "-o", "/dev/null", "-w", "status=%{http_code} remote=%{remote_ip}\n", "https://www.gstatic.com/generate_204"),
}
OUT.write_text("\n\n".join(f"=== {k} ===\n{json.dumps(v, ensure_ascii=False, indent=2) if isinstance(v, (dict, list)) else v}" for k, v in sections.items()))
print(OUT)
