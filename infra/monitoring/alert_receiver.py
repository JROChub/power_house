#!/usr/bin/env python3
import json
import subprocess
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/alerts":
            self.send_error(404)
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            payload = json.loads(self.rfile.read(length))
            alerts = payload.get("alerts", [])
            lines = []
            for alert in alerts:
                labels = alert.get("labels", {})
                summary = alert.get("annotations", {}).get("summary", "Power House alert")
                lines.append(
                    f"{alert.get('status', 'unknown').upper()}: "
                    f"{labels.get('severity', 'unknown')} - {summary}"
                )
            detail = "\n".join(lines) or "Alertmanager sent an empty alert group"
            subprocess.run(
                ["/usr/local/lib/powerhouse/alert.sh", "Monitoring alert", detail],
                check=False,
            )
        except Exception as exc:
            self.send_error(400, str(exc))
            return
        self.send_response(204)
        self.end_headers()

    def log_message(self, format, *args):
        return


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", 9193), Handler).serve_forever()
