#!/usr/bin/env python3
"""MCP adapter for the hosted md-docrs Worker."""

from __future__ import annotations

import argparse
import os
import urllib.error
import urllib.parse
import urllib.request

from mcp.server.fastmcp import FastMCP


DEFAULT_BASE_URL = "https://md-docrs.workedonmymachine.com"
USER_AGENT = "md-docrs-remote-mcp/0.1"

mcp = FastMCP("md-docrs-remote")


def base_url() -> str:
    return os.environ.get("MD_DOCRS_BASE_URL", DEFAULT_BASE_URL).rstrip("/")


def fetch_docs(spec: str, target: str | None = None) -> str:
    params = {"spec": spec}
    if target:
        params["target"] = target

    url = f"{base_url()}/?{urllib.parse.urlencode(params)}"
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "text/markdown",
            "User-Agent": USER_AGENT,
        },
    )

    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.read().decode("utf-8")
    except urllib.error.HTTPError as err:
        body = err.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"md-docrs returned HTTP {err.code}: {body}") from err


@mcp.tool()
def rust_docs(spec: str, target: str | None = None) -> str:
    """Fetch Rust docs.rs documentation as Markdown.

    Spec format: crate[@version][::path::to::item].
    Examples: anyhow, anyhow::Error, tokio@1.52.1::sync::Mutex.
    """
    return fetch_docs(spec, target)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run an MCP server backed by the remote md-docrs Worker."
    )
    parser.add_argument(
        "--smoke",
        metavar="SPEC",
        help="fetch one spec from the remote Worker and print Markdown",
    )
    parser.add_argument(
        "--target",
        help="optional docs.rs target triple for --smoke",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if args.smoke:
        print(fetch_docs(args.smoke, args.target), end="")
        return

    mcp.run()


if __name__ == "__main__":
    main()
