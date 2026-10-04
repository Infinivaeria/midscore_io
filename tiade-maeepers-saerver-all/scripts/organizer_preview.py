"""Serve the local preview through the running Rust server, including all writes."""
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ssl
from urllib.parse import urlsplit


class OrganizerProxy(BaseHTTPRequestHandler):
    def do_GET(self):
        self.forward()

    def do_POST(self):
        self.forward()

    def forward(self):
        path = urlsplit(self.path)
        if path.path == "/organizer.html" and self.command == "GET":
            self.send_response(307)
            self.send_header("Location", "/organizer" + ("?" + path.query if path.query else ""))
            self.end_headers()
            return
        if path.path != "/organizer":
            self.send_error(404)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.send_error(400)
            return
        if length < 0 or length > 16384:
            self.send_error(413)
            return
        # The upstream is fixed to loopback; its public certificate does not
        # cover 127.0.0.1. This exception applies only to this local dev proxy.
        upstream = http.client.HTTPSConnection(
            "127.0.0.1", 443, context=ssl._create_unverified_context(), timeout=30
        )
        try:
            upstream.request(self.command, self.path,
                             body=self.rfile.read(length),
                             headers={"Content-Type": self.headers.get("Content-Type", "")})
            response = upstream.getresponse()
            body = response.read()
            self.send_response(response.status)
            self.send_header("Content-Type", response.getheader("Content-Type", "application/json"))
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)
        except (OSError, http.client.HTTPException):
            self.send_error(502, "The Rust server on local port 443 is unavailable")
        finally:
            upstream.close()


if __name__ == "__main__":
    print("Organizer preview: http://127.0.0.1:8766/organizer", flush=True)
    print("Reads and saves use the running Rust server's persistent data.", flush=True)
    ThreadingHTTPServer(("127.0.0.1", 8766), OrganizerProxy).serve_forever()
