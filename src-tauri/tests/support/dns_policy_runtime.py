"""Native-core DNS policy verification. All network endpoints are loopback fixtures."""
import base64
import copy
import http.server
import ipaddress
import json
import pathlib
import select
import socket
import socketserver
import ssl
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request


def read_exact(sock, count):
    data = b""
    while len(data) < count:
        chunk = sock.recv(count - len(data))
        if not chunk:
            raise EOFError("fixture socket closed")
        data += chunk
    return data


class ThreadedTCP(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


class Socks(socketserver.BaseRequestHandler):
    def handle(self):
        client = self.request
        client.settimeout(5)
        try:
            version, count = read_exact(client, 2)
            assert version == 5
            read_exact(client, count)
            client.sendall(b"\x05\x00")
            version, command, _, kind = read_exact(client, 4)
            assert version == 5 and command == 1
            if kind == 1:
                host = socket.inet_ntoa(read_exact(client, 4))
            elif kind == 3:
                host = read_exact(client, read_exact(client, 1)[0]).decode()
            else:
                host = socket.inet_ntop(socket.AF_INET6, read_exact(client, 16))
            port = struct.unpack("!H", read_exact(client, 2))[0]
            # Never forward fixture traffic to the Internet, even on regression.
            assert ipaddress.ip_address(host).is_loopback, host
            self.server.requests.append((host, port))
            if self.server.fail:
                client.sendall(b"\x05\x05\x00\x01" + b"\x00" * 6)
                return
            with socket.create_connection((host, port), timeout=5) as upstream:
                client.sendall(b"\x05\x00\x00\x01" + b"\x00" * 6)
                while True:
                    ready, _, _ = select.select([client, upstream], [], [], 5)
                    if not ready:
                        return
                    for source in ready:
                        data = source.recv(65536)
                        if not data:
                            return
                        (upstream if source is client else client).sendall(data)
        except (EOFError, OSError):
            return


class Doh(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_POST(self):
        request = self.rfile.read(int(self.headers["Content-Length"]))
        self.respond_dns(request)

    def do_GET(self):
        from urllib.parse import parse_qs, urlparse
        encoded = parse_qs(urlparse(self.path).query)["dns"][0]
        self.respond_dns(base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)))

    def respond_dns(self, query):
        self.server.queries += 1
        end = 12
        while query[end]:
            end += query[end] + 1
        question = query[12:end + 5]
        answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, 300, 4) + socket.inet_aton(self.server.answer)
        response = query[:2] + struct.pack("!HHHHH", 0x8180, 1, 1, 0, 0) + question + answer
        self.send_response(200)
        self.send_header("Content-Type", "application/dns-message")
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)


def serve(server):
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def api(port, path, method="GET", data=None):
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        data=None if data is None else json.dumps(data).encode(), method=method,
        headers={"Authorization": "Bearer SECRET", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=5) as response:
            body = response.read()
            return json.loads(body) if body else None
    except urllib.error.HTTPError as error:
        return {"error": error.read().decode(), "http_status": error.code}


def main():
    core, source = sys.argv[1:]
    configs = json.loads(pathlib.Path(source).read_text())
    with tempfile.TemporaryDirectory(prefix="dns-policy-tls-") as directory:
        root = pathlib.Path(directory)
        (root / "tls.cnf").write_text("[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=dns.test\n[ext]\nsubjectAltName=DNS:dns.test\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,digitalSignature,keyEncipherment,keyCertSign\nextendedKeyUsage=serverAuth\n")
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-config", str(root / "tls.cnf"), "-keyout", str(root / "key.pem"), "-out", str(root / "cert.pem")], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(root / "cert.pem", root / "key.pem")
        servers = []
        dns_servers = []
        for answer in ["203.0.113.11", "203.0.113.12"]:
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Doh)
            server.daemon_threads = True
            server.socket = context.wrap_socket(server.socket, server_side=True)
            server.answer, server.queries = answer, 0
            serve(server)
            servers.append(server)
            dns_servers.append(server)
        exits = []
        for _ in range(2):
            server = ThreadedTCP(("127.0.0.1", 0), Socks)
            server.requests, server.fail = [], False
            serve(server)
            servers.append(server)
            exits.append(server)
        try:
            for variant, original in configs.items():
                config = copy.deepcopy(original)
                chosen_tag = config["dns"]["final"]
                direct_tag = "__dns-direct-" + chosen_tag
                config.pop("services", None)
                config["inbounds"] = []
                config["route"].pop("auto_detect_interface", None)
                config["experimental"]["cache_file"] = {"enabled": False}
                port = free_port()
                config["experimental"]["clash_api"]["external_controller"] = f"127.0.0.1:{port}"
                config["experimental"]["clash_api"]["default_mode"] = "direct"
                config["dns"]["timeout"] = "1s"
                config["outbounds"] = [
                    {"type": "socks", "tag": f"fixture-{i}", "server": "127.0.0.1", "server_port": server.server_address[1], "version": "5"}
                    for i, server in enumerate(exits)
                ] + [{"type":"selector", "tag":"root-policy", "outbounds":["fixture-0","fixture-1"], "default":"fixture-0", "interrupt_exist_connections":True}, {"type":"direct","tag":"direct"}, {"type":"block","tag":"block"}]
                for server in config["dns"]["servers"]:
                    if server["tag"] not in [direct_tag, chosen_tag]:
                        continue
                    remote = server["tag"] == chosen_tag
                    fixture = dns_servers[1 if remote else 0]
                    # Endpoint and trust anchor are replaced only in this
                    # isolated test; production options are checked in Rust.
                    server.update(server="127.0.0.1", server_port=fixture.server_address[1], tls={"enabled":True, "server_name":"dns.test", "certificate_path":str(root / "cert.pem")})
                    if remote:
                        server["detour"] = "root-policy"
                path = root / f"{variant}.json"
                path.write_text(json.dumps(config))
                with (root / f"{variant}.log").open("w+") as log:
                    process = subprocess.Popen([core, "run", "-c", str(path)], cwd=root, stdout=log, stderr=log)
                    try:
                        for _ in range(100):
                            if process.poll() is not None:
                                log.seek(0)
                                raise AssertionError(log.read())
                            try:
                                api(port, "/configs")
                                break
                            except OSError:
                                time.sleep(.05)
                        def query(name="public.example"):
                            result = api(port, f"/dns/query?name={name}&type=A")
                            result["resolved_ips"] = [record["data"] for record in result.get("Answer", []) if record["type"] == 1]
                            result["trace"] = result.get("Trace", {})
                            return result
                        direct = query()
                        assert direct["resolved_ips"] == ["203.0.113.11"], direct
                        assert direct["trace"]["server"] == direct_tag, direct
                        direct_rule = query("routed.example")
                        assert direct_rule["trace"]["server"] == direct_tag, direct_rule
                        api(port, "/configs", "PATCH", {"mode":"global"})
                        routed = query("routed.example")
                        assert routed["trace"]["server"] == chosen_tag, routed
                        assert routed["resolved_ips"] == ["203.0.113.12"], routed
                        proxied = query()
                        expected = "fakeip" if variant == "fake" else chosen_tag
                        assert proxied["trace"]["server"] == expected, proxied
                        if variant == "fake":
                            assert ipaddress.ip_address(proxied["resolved_ips"][0]) in ipaddress.ip_network("198.18.0.0/15"), proxied
                        else:
                            assert proxied["resolved_ips"] == ["203.0.113.12"], proxied
                            assert exits[0].requests
                            api(port, "/proxies/root-policy", "PUT", {"name":"fixture-1"})
                            api(port, "/cache/dns/flush", "POST")
                            switched = query("switched.example")
                            assert switched["resolved_ips"] == ["203.0.113.12"], switched
                            assert exits[1].requests
                            exits[1].fail = True
                            # Changing selected exit closes old keepalives.
                            api(port, "/proxies/root-policy", "PUT", {"name":"fixture-0"})
                            api(port, "/proxies/root-policy", "PUT", {"name":"fixture-1"})
                            api(port, "/cache/dns/flush", "POST")
                            direct_count = dns_servers[0].queries
                            failed = query("failure.example")
                            assert not failed.get("resolved_ips"), failed
                            assert dns_servers[0].queries == direct_count, "silent Direct fallback"
                            exits[1].fail = False
                        local = query("localhost")
                        assert local["trace"]["server"] == "local-dns", local
                        assert "127.0.0.1" in local["resolved_ips"], local
                        api(port, "/configs", "PATCH", {"mode":"rule"})
                        assert query("rule.example")["trace"]["server"] == expected
                        print(f"{variant}: Direct -> Global -> Rule, local names, " + ("FakeIP OK" if variant == "fake" else "proxy selection and failure without fallback OK"))
                    finally:
                        process.terminate()
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait()
        finally:
            for server in servers:
                server.shutdown()
                server.server_close()


if __name__ == "__main__":
    main()
