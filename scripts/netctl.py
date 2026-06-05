#!/usr/bin/env python3
"""Operational control plane for JULIAN bootstrap nodes.

The script intentionally depends only on Python's standard library. It reads
`infra/ops_hosts.toml`, builds the `julian` binary, packages release artifacts,
and performs SSH/SCP based deployment and inspection tasks.
"""

from __future__ import annotations

import argparse
import os
import shlex
import subprocess
import sys
import tarfile
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_HOSTS_FILE = ROOT / "infra" / "ops_hosts.toml"
EXAMPLE_HOSTS_FILE = ROOT / "infra" / "ops_hosts.example.toml"


@dataclass(frozen=True)
class Node:
    name: str
    host: str
    ssh_user: str
    ssh_port: int
    service: str
    binary_path: str
    config_dir: str
    log_path: str
    work_dir: str
    bootstrap: tuple[str, ...]
    restart_command: str | None

    @property
    def ssh_target(self) -> str:
        return f"{self.ssh_user}@{self.host}" if self.ssh_user else self.host


def quote(value: str | Path) -> str:
    return shlex.quote(str(value))


def print_cmd(argv: Sequence[str]) -> None:
    print("+ " + " ".join(shlex.quote(str(part)) for part in argv))


def run(argv: Sequence[str], *, dry_run: bool = False, cwd: Path = ROOT) -> None:
    print_cmd(argv)
    if dry_run:
        return
    subprocess.run(list(argv), cwd=cwd, check=True)


def read_package_version() -> str:
    cargo_toml = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return str(cargo_toml["package"]["version"])


def field(config: dict, defaults: dict, key: str, fallback: str | None = None):
    if key in config:
        return config[key]
    if key in defaults:
        value = defaults[key]
        if isinstance(value, str):
            return value.format(name=config.get("name", "node"))
        return value
    if fallback is not None:
        return fallback.format(name=config.get("name", "node"))
    return None


def load_nodes(hosts_file: Path) -> list[Node]:
    if not hosts_file.exists():
        raise SystemExit(
            f"hosts file not found: {hosts_file}\n"
            f"copy {EXAMPLE_HOSTS_FILE.relative_to(ROOT)} to {DEFAULT_HOSTS_FILE.relative_to(ROOT)} first"
        )
    data = tomllib.loads(hosts_file.read_text(encoding="utf-8"))
    defaults = data.get("defaults", {})
    nodes = []
    for item in data.get("node", []):
        name = str(item.get("name", "")).strip()
        host = str(item.get("host", "")).strip()
        if not name or not host:
            raise SystemExit("each [[node]] entry requires non-empty name and host")
        bootstrap = field(item, defaults, "bootstrap", []) or []
        if isinstance(bootstrap, str):
            bootstrap = [bootstrap]
        nodes.append(
            Node(
                name=name,
                host=host,
                ssh_user=str(field(item, defaults, "ssh_user", "root") or "root"),
                ssh_port=int(field(item, defaults, "ssh_port", 22) or 22),
                service=str(field(item, defaults, "service", "powerhouse-{name}.service")),
                binary_path=str(field(item, defaults, "binary_path", "/usr/local/bin/julian")),
                config_dir=str(field(item, defaults, "config_dir", "/etc/jrocnet")),
                log_path=str(field(item, defaults, "log_path", "/var/log/jrocnet.log")),
                work_dir=str(field(item, defaults, "work_dir", "/var/lib/jrocnet/{name}")),
                bootstrap=tuple(str(entry) for entry in bootstrap),
                restart_command=field(item, defaults, "restart_command"),
            )
        )
    if not nodes:
        raise SystemExit(f"no [[node]] entries found in {hosts_file}")
    return nodes


def select_nodes(nodes: Sequence[Node], names: Sequence[str] | None) -> list[Node]:
    if not names:
        return list(nodes)
    by_name = {node.name: node for node in nodes}
    unknown = [name for name in names if name not in by_name]
    if unknown:
        known = ", ".join(sorted(by_name))
        raise SystemExit(f"unknown host(s): {', '.join(unknown)}; known hosts: {known}")
    return [by_name[name] for name in names]


def ssh_base(node: Node) -> list[str]:
    return ["ssh", "-p", str(node.ssh_port), node.ssh_target]


def scp_base(node: Node) -> list[str]:
    return ["scp", "-P", str(node.ssh_port)]


def remote(node: Node, command: str, *, dry_run: bool = False) -> None:
    run([*ssh_base(node), command], dry_run=dry_run)


def copy_to(node: Node, source: Path, dest: str, *, dry_run: bool = False) -> None:
    run([*scp_base(node), str(source), f"{node.ssh_target}:{dest}"], dry_run=dry_run)


def build_release(*, dry_run: bool = False) -> Path:
    run(
        ["cargo", "build", "--release", "--features", "net", "--bin", "julian"],
        dry_run=dry_run,
    )
    return ROOT / "target" / "release" / "julian"


def config_files() -> list[Path]:
    infra = ROOT / "infra"
    return sorted(path for path in infra.glob("*.json") if path.is_file())


def cmd_list_hosts(args: argparse.Namespace) -> None:
    nodes = select_nodes(load_nodes(args.hosts_file), args.hosts)
    headers = ("name", "ssh", "service", "work_dir", "bootstraps")
    rows = [
        (
            node.name,
            f"{node.ssh_target}:{node.ssh_port}",
            node.service,
            node.work_dir,
            str(len(node.bootstrap)),
        )
        for node in nodes
    ]
    widths = [len(header) for header in headers]
    for row in rows:
        widths = [max(width, len(value)) for width, value in zip(widths, row)]
    print("  ".join(header.ljust(width) for header, width in zip(headers, widths)))
    print("  ".join("-" * width for width in widths))
    for row in rows:
        print("  ".join(value.ljust(width) for value, width in zip(row, widths)))


def cmd_package(args: argparse.Namespace) -> None:
    binary = build_release(dry_run=args.dry_run)
    version = read_package_version()
    out = Path(args.out) if args.out else ROOT / "target" / f"power_house_ops-{version}.tar.gz"
    print(f"package target: {out}")
    if args.dry_run:
        return
    if not binary.exists():
        raise SystemExit(f"release binary missing after build: {binary}")
    out.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(out, "w:gz") as archive:
        archive.add(binary, arcname="bin/julian")
        for path in config_files():
            archive.add(path, arcname=f"infra/{path.name}")
        for path in sorted(ROOT.glob("powerhouse-boot*.service")):
            archive.add(path, arcname=f"systemd/{path.name}")
        archive.add(ROOT / "docs" / "ops.md", arcname="docs/ops.md")
        archive.add(ROOT / "infra" / "ops_hosts.example.toml", arcname="infra/ops_hosts.example.toml")
    print(f"wrote {out.relative_to(ROOT) if out.is_relative_to(ROOT) else out}")


def restart_command(node: Node) -> str:
    if node.restart_command:
        return str(node.restart_command).format(
            name=node.name,
            service=node.service,
            binary_path=node.binary_path,
            config_dir=node.config_dir,
            work_dir=node.work_dir,
        )
    return f"systemctl restart {quote(node.service)}"


def cmd_restart(args: argparse.Namespace) -> None:
    for node in select_nodes(load_nodes(args.hosts_file), args.hosts):
        remote(node, restart_command(node), dry_run=args.dry_run)


def cmd_status(args: argparse.Namespace) -> None:
    for node in select_nodes(load_nodes(args.hosts_file), args.hosts):
        remote(node, f"systemctl --no-pager --full status {quote(node.service)}", dry_run=args.dry_run)


def journal_or_tail(node: Node, *, lines: int, follow: bool) -> str:
    flags = f"-n {int(lines)}"
    if follow:
        flags += " -f"
    return (
        "if command -v journalctl >/dev/null 2>&1; then "
        f"journalctl -u {quote(node.service)} {flags} --no-pager; "
        "else "
        f"tail -n {int(lines)} {'-f ' if follow else ''}{quote(node.log_path)}; "
        "fi"
    )


def cmd_logs(args: argparse.Namespace) -> None:
    for node in select_nodes(load_nodes(args.hosts_file), args.hosts):
        remote(node, journal_or_tail(node, lines=args.lines, follow=False), dry_run=args.dry_run)


def cmd_follow(args: argparse.Namespace) -> None:
    nodes = select_nodes(load_nodes(args.hosts_file), args.hosts)
    if len(nodes) != 1:
        raise SystemExit("follow expects exactly one host; pass --hosts <name>")
    remote(nodes[0], journal_or_tail(nodes[0], lines=args.lines, follow=True), dry_run=args.dry_run)


def cmd_shell(args: argparse.Namespace) -> None:
    node = select_nodes(load_nodes(args.hosts_file), [args.host])[0]
    run(ssh_base(node), dry_run=args.dry_run)


def cmd_exec(args: argparse.Namespace) -> None:
    if not args.command:
        raise SystemExit("exec requires a command to run")
    node = select_nodes(load_nodes(args.hosts_file), [args.host])[0]
    remote(node, " ".join(args.command), dry_run=args.dry_run)


def cmd_deploy(args: argparse.Namespace) -> None:
    nodes = select_nodes(load_nodes(args.hosts_file), args.hosts)
    binary = build_release(dry_run=args.dry_run)
    release_stamp = str(int(time.time()))
    for node in nodes:
        binary_dir = os.path.dirname(node.binary_path) or "."
        remote_tmp = f"/tmp/julian-{release_stamp}-{node.name}.new"
        remote(
            node,
            "mkdir -p "
            f"{quote(binary_dir)} {quote(node.config_dir)} {quote(node.work_dir)}",
            dry_run=args.dry_run,
        )
        copy_to(node, binary, remote_tmp, dry_run=args.dry_run)
        remote(
            node,
            f"install -m 0755 {quote(remote_tmp)} {quote(node.binary_path)} && rm -f {quote(remote_tmp)}",
            dry_run=args.dry_run,
        )
        for path in config_files():
            copy_to(node, path, f"{node.config_dir}/{path.name}", dry_run=args.dry_run)
        if not args.no_restart:
            remote(node, restart_command(node), dry_run=args.dry_run)


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--hosts-file",
        type=Path,
        default=argparse.SUPPRESS,
        help="path to ops_hosts.toml (default: infra/ops_hosts.toml or $POWERHOUSE_HOSTS_FILE)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        default=argparse.SUPPRESS,
        help="print commands without executing them",
    )


def add_host_filter(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--hosts", nargs="+", help="limit command to named hosts")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description="Power-House JULIAN ops controller")
    add_common(root)
    sub = root.add_subparsers(dest="command", required=True)

    list_hosts = sub.add_parser("list-hosts", help="show configured hosts")
    add_common(list_hosts)
    add_host_filter(list_hosts)
    list_hosts.set_defaults(func=cmd_list_hosts)

    package = sub.add_parser("package", help="build and archive release artifacts")
    add_common(package)
    package.add_argument("--out", help="output .tar.gz path")
    package.set_defaults(func=cmd_package)

    deploy = sub.add_parser("deploy", help="build, upload, install, and restart nodes")
    add_common(deploy)
    add_host_filter(deploy)
    deploy.add_argument("--no-restart", action="store_true", help="install files without restart")
    deploy.set_defaults(func=cmd_deploy)

    restart = sub.add_parser("restart", help="restart node services")
    add_common(restart)
    add_host_filter(restart)
    restart.set_defaults(func=cmd_restart)

    status = sub.add_parser("status", help="show service status")
    add_common(status)
    add_host_filter(status)
    status.set_defaults(func=cmd_status)

    logs = sub.add_parser("logs", help="print recent service logs")
    add_common(logs)
    add_host_filter(logs)
    logs.add_argument("--lines", type=int, default=120)
    logs.set_defaults(func=cmd_logs)

    follow = sub.add_parser("follow", help="follow one service log stream")
    add_common(follow)
    add_host_filter(follow)
    follow.add_argument("--lines", type=int, default=120)
    follow.set_defaults(func=cmd_follow)

    shell = sub.add_parser("shell", help="open an SSH shell")
    add_common(shell)
    shell.add_argument("host")
    shell.set_defaults(func=cmd_shell)

    exec_cmd = sub.add_parser("exec", help="run an ad-hoc remote command")
    add_common(exec_cmd)
    exec_cmd.add_argument("host")
    exec_cmd.add_argument("command", nargs=argparse.REMAINDER)
    exec_cmd.set_defaults(func=cmd_exec)

    return root


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    args.hosts_file = Path(
        getattr(
            args,
            "hosts_file",
            os.environ.get("POWERHOUSE_HOSTS_FILE", DEFAULT_HOSTS_FILE),
        )
    )
    args.dry_run = bool(getattr(args, "dry_run", False))
    args.func(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
