"""Checks that every internal link of the built website and docs (site/public) leads to a page or a file.

Run after `python3 site/build.py` and `npm --prefix docs run build`. Links are resolved the way Cloudflare serves
the folder: `/a/b` finds `a/b.html`, `a/b/index.html` or the file itself. Anchors and query strings are ignored, and
so are other origins. Exits with 1 and lists each broken link with the pages that have it.
"""

from __future__ import annotations

import html.parser
import pathlib
import sys
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parent.parent / "site" / "public"
ORIGIN = "https://triage.iscor.me"


class Links(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.found = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        # Alternates and canonicals name the public URL, which a page of the build must also answer.
        linked = tag == "a" or (tag == "link" and attrs.get("rel") in ("alternate", "canonical"))
        if linked and attrs.get("href"):
            self.found.append(attrs["href"])


def target(page: pathlib.Path, href: str) -> str | None:
    """The path a link asks this site for, or None when it goes elsewhere."""
    if href.startswith(ORIGIN):
        href = href[len(ORIGIN) :] or "/"
    url = urllib.parse.urlsplit(href)
    if url.scheme or url.netloc or not url.path:
        return None
    if url.path.startswith("/"):
        return urllib.parse.unquote(url.path)
    base = "/" + page.relative_to(ROOT).as_posix()
    return urllib.parse.unquote(urllib.parse.urljoin(base, url.path))


def exists(path: str) -> bool:
    file = ROOT / path.lstrip("/")
    if path.endswith("/"):
        return (file / "index.html").is_file()
    return file.is_file() or file.with_name(file.name + ".html").is_file() or (file / "index.html").is_file()


def main() -> int:
    if not (ROOT / "docs" / "index.html").is_file():
        sys.exit("No built docs in site/public/docs: run `npm --prefix docs run build` first.")
    broken: dict[str, set[str]] = {}
    pages = sorted(ROOT.rglob("*.html"))
    for page in pages:
        parser = Links()
        parser.feed(page.read_text(encoding="utf-8"))
        for href in parser.found:
            path = target(page, href)
            if path is not None and not exists(path):
                broken.setdefault(path, set()).add(page.relative_to(ROOT).as_posix())
    for path, where in sorted(broken.items()):
        shown = sorted(where)
        more = f" and {len(shown) - 3} more" if len(shown) > 3 else ""
        print(f"{path}  (in {', '.join(shown[:3])}{more})")
    print(f"{len(pages)} pages, {len(broken)} broken link targets.")
    return 1 if broken else 0


if __name__ == "__main__":
    sys.exit(main())
