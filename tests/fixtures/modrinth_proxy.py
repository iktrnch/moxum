"""Serve local Modrinth fixtures through a TLS-verifying HTTPS proxy."""

import pathlib
import socketserver
import ssl
import subprocess
import sys


def create_certificates(root):
    """Generate a temporary test CA and a leaf certificate for api.modrinth.com."""
    commands = [
        [
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
            "-subj", "/CN=Minegr test CA", "-addext", "basicConstraints=critical,CA:TRUE",
            "-keyout", str(root / "ca.key"), "-out", str(root / "ca.pem"),
        ],
        [
            "req", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=api.modrinth.com",
            "-keyout", str(root / "server.key"), "-out", str(root / "server.csr"),
        ],
        [
            "x509", "-req", "-days", "1", "-in", str(root / "server.csr"),
            "-CA", str(root / "ca.pem"), "-CAkey", str(root / "ca.key"),
            "-CAcreateserial", "-extfile", str(root / "extensions.cnf"),
            "-out", str(root / "server.pem"),
        ],
    ]
    (root / "extensions.cnf").write_text(
        "subjectAltName=DNS:api.modrinth.com\n"
        "basicConstraints=critical,CA:FALSE\n"
        "keyUsage=critical,digitalSignature,keyEncipherment\n"
        "extendedKeyUsage=serverAuth\n"
    )
    for command in commands:
        subprocess.run(["openssl", *command], check=True, capture_output=True)


def read_headers(connection):
    """Read one bounded HTTP header block without consuming the following TLS bytes."""
    headers = bytearray()
    while not headers.endswith(b"\r\n\r\n"):
        byte = connection.recv(1)
        if not byte:
            raise ConnectionError("Connection closed before HTTP headers completed")
        headers.extend(byte)
        if len(headers) > 16384:
            raise ValueError("Fixture HTTP headers exceeded their limit")
    return headers.split(b"\r\n", 1)[0].decode("ascii")


class ModrinthProxy(socketserver.ThreadingTCPServer):
    """Bind only to loopback and serve requests from the CLI under test."""

    daemon_threads = True


class Handler(socketserver.BaseRequestHandler):
    """Terminate a CONNECT tunnel and serve only local metadata or artifact bytes."""

    def handle(self):
        self.request.settimeout(10)
        if read_headers(self.request) != "CONNECT api.modrinth.com:443 HTTP/1.1":
            self.request.sendall(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            return
        self.request.sendall(b"HTTP/1.1 200 Connection established\r\n\r\n")
        with self.server.tls.wrap_socket(self.request, server_side=True) as connection:
            method, path, _ = read_headers(connection).split()
            if method == "GET" and path.startswith("/v2/project/"):
                body = (self.server.root / "versions.json").read_bytes()
                status, content_type = "200 OK", "application/json"
            elif method == "GET" and path == "/artifact.jar":
                body = b"Minegr environment regression artifact"
                status, content_type = "200 OK", "application/java-archive"
            else:
                body = b"Unexpected fixture request"
                status, content_type = "404 Not Found", "text/plain"
            response = (
                f"HTTP/1.1 {status}\r\nContent-Length: {len(body)}\r\n"
                f"Content-Type: {content_type}\r\nConnection: close\r\n\r\n"
            ).encode("ascii")
            connection.sendall(response + body)


def main():
    """Print the bound proxy URL after certificate generation and listener setup."""
    root = pathlib.Path(sys.argv[1])
    create_certificates(root)
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(root / "server.pem", root / "server.key")
    with ModrinthProxy(("127.0.0.1", 0), Handler) as server:
        server.root = root
        server.tls = tls
        print(f"http://127.0.0.1:{server.server_address[1]}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
