#!/usr/bin/env python3
"""Writes the Remora website (public/) in English and French. Standard library only.

    python3 site/build.py     then    cd site && npx wrangler deploy

The copy is in copy/en.json and copy/fr.json ($repo, $dmg, $tools… stand for the links and names below), the page
markup in templates/ (string.Template), and this file puts them together; the one-line fragments repeated in lists
(cards, rows, buttons) are the small helpers below. Screenshots come from macos/scripts/screenshots.sh and
desktop/app/scripts/screenshots.mjs (make screenshots). Same design system as Myna (site.css, tokens.css).
"""

from __future__ import annotations

import datetime
import functools
import html
import importlib.util
import json
import pathlib
import plistlib
import re
import string
import subprocess

SITE = pathlib.Path(__file__).parent
ROOT = SITE / "public"
ORIGIN = "https://triage.iscor.me"
REPO = "https://github.com/iGitScor/triage"
DMG = REPO + "/releases/latest/download/Remora.dmg"
EXE = REPO + "/releases/latest/download/Remora-Setup.exe"
APPIMAGE = REPO + "/releases/latest/download/Remora-x86_64.AppImage"
DEB = REPO + "/releases/latest/download/Remora-amd64.deb"
# The released version, from the source `make version` sets, so the page's structured data never goes stale.
VERSION = plistlib.loads((SITE.parent / "macos/Resources/Info.plist").read_bytes())["CFBundleShortVersionString"]
LICENSE_URL = "https://www.gnu.org/licenses/gpl-3.0.html"
DOCS = {"en": "/docs/", "fr": "/docs/fr/"}
# The MDM page's example policy, written by scripts/make-policy-files.py from its key list: the privacy page shows
# the same keys and values, so the sample can't drift from the files IT imports.
POLICY = json.loads((SITE.parent / "policy/policy.json").read_text(encoding="utf-8"))
# Every key IT can set, with its meaning in both languages: the privacy page's table comes from the same list as the
# policy files, so it can't fall behind.
_spec = importlib.util.spec_from_file_location("policy_files", SITE.parent / "scripts/make-policy-files.py")
_policy_files = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_policy_files)
POLICY_KEYS = _policy_files.KEYS
DOMAIN = "fr.igitscor.remora"
# Which capture illustrates which group of the features page.
SHOTS = {"time": "snoozed", "reviews": "waiting"}

ICONS = {
    "arrow": '<path d="M5 12h14M13 6l6 6-6 6"/>',
    "info": '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>',
    "reply": '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
    "review": '<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>',
    "fix": '<path d="M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.6 2.6-2.4-.6-.6-2.4z"/>',
    "merge": '<circle cx="6" cy="6" r="2.5"/><circle cx="6" cy="18" r="2.5"/><circle cx="18" cy="12" r="2.5"/><path d="M6 8.5v7M8.3 7.2C12 9 13 12 15.5 12"/>',
    "todo": '<rect x="4" y="4" width="16" height="16" rx="3"/><path d="m8.5 12 2.5 2.5 4.5-5"/>',
    "bell": '<path d="M6 16v-5a6 6 0 0 1 12 0v5l1.5 2h-15z"/><path d="M10 20a2 2 0 0 0 4 0"/>',
    "read": '<path d="M4 6h16M4 11h16M4 16h10"/>',
    "hourglass": '<path d="M6 3h12M6 21h12M7 3c0 5 10 5 10 9s-10 4-10 9M17 3c0 5-10 5-10 9s10 4 10 9"/>',
    "moon": '<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>',
    "done": '<circle cx="12" cy="12" r="9"/><path d="m8.5 12 2.5 2.5 4.5-5"/>',
    "pin": '<path d="M9 4h6l-1 6 3 3H7l3-3z"/><path d="M12 13v7"/>',
    "stopwatch": '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2.5M10 2h4"/>',
    "people": '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><path d="M16 4.5a3.5 3.5 0 0 1 0 7M18.5 20a6.5 6.5 0 0 0-2.5-5.1"/>',
    "star": '<path d="m12 3 2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1L3.2 9.5l6.1-.9z"/>',
    "lock": '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
    "shield": '<path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.3 7.5 9.5 4.3-1.2 7.5-4.9 7.5-9.5V6z"/><path d="m9 12 2 2 4-4"/>',
    "building": '<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M9 7h2M13 7h2M9 11h2M13 11h2M9 15h2M13 15h2"/>',
    "sparkle": '<path d="M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5 18 18M6 18l2.5-2.5M15.5 8.5 18 6"/>',
    "link": '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
    "globe": '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
    "swipe": '<path d="M5 12h14M15 8l4 4-4 4M9 8l-4 4 4 4"/>',
    "chip": '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>',
    "eyeoff": '<path d="M3 3l18 18"/><path d="M10.6 5.1A10 10 0 0 1 12 5c6.4 0 10 7 10 7a17 17 0 0 1-3.2 4M6.6 6.6A17 17 0 0 0 2 12s3.6 7 10 7a10 10 0 0 0 5.4-1.6"/>',
    "trash": '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
    "drag": '<path d="M12 3v14M7 12l5 5 5-5M5 21h14"/>',
    "menu": '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M15 4v5"/>',
    "key": '<circle cx="8" cy="15" r="4"/><path d="m11 12 9-9M17 6l3 3M14 9l2 2"/>',
    "wifi": '<path d="M2 8.8a15 15 0 0 1 20 0M5 12.9a10 10 0 0 1 14 0M8.5 16.4a5 5 0 0 1 7 0M12 20h.01"/>',
    "battery": '<rect x="2" y="7" width="17" height="10" rx="2.5"/><path d="M22 11v2M5 10h9v4H5z"/>',
}

FISH = (
    '<svg viewBox="5 35 88 32" fill="currentColor" aria-hidden="true">'
    '<path d="M27 51C18 44 10 36 6 36C10 45 10 57 6 66C10 66 18 58 27 51Z"/>'
    '<path fill-rule="evenodd" d="M35 38H79A13 13 0 0 1 79 64H35A13 13 0 0 1 35 38ZM77 51a3 3 0 1 0 6 0a3 3 0 1 0-6 0Z"/></svg>'
)

PAGES = {
    "home": {"en": "/en/", "fr": "/fr/"},
    "features": {"en": "/en/features", "fr": "/fr/fonctionnalites"},
    "privacy": {"en": "/en/privacy", "fr": "/fr/confidentialite"},
}


def icon(name: str, size: int = 22, cls: str = "") -> str:
    attrs = f' class="{cls}"' if cls else ""
    return (
        f'<svg{attrs} width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
        f'stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{ICONS[name]}</svg>'
    )


def e(text: str) -> str:
    return html.escape(text, quote=True)


# ---------------------------------------------------------------- copy and templates


def resolve(node, names: dict[str, str]):
    """The copy with its $placeholders replaced, at every depth. A placeholder with no value stops the build."""
    if isinstance(node, str):
        return string.Template(node).substitute(names)
    if isinstance(node, list):
        return [resolve(item, names) for item in node]
    if isinstance(node, dict):
        return {key: resolve(value, names) for key, value in node.items()}
    return node


def load_copy(lang: str) -> dict:
    data = json.loads((SITE / "copy" / f"{lang}.json").read_text(encoding="utf-8"))
    names = {"repo": REPO, "dmg": DMG, "exe": EXE, "appimage": APPIMAGE, "deb": DEB, "tools": data["tools"]}
    return resolve(data, names)


T = {lang: load_copy(lang) for lang in ("en", "fr")}


@functools.cache
def template(name: str) -> string.Template:
    # Without the file's final newline, so a template ends where its markup does whatever the editor saves.
    return string.Template((SITE / "templates" / name).read_text(encoding="utf-8").removesuffix("\n"))


def fill(file: str, /, **values: object) -> str:
    return template(file).substitute(values)


def href(lang: str, target: str) -> str:
    return PAGES[target][lang] if target in PAGES else target


# ---------------------------------------------------------------- layout


def ld(data: dict) -> str:
    text = json.dumps(data, ensure_ascii=False, indent=2).replace("</", "<\\/")
    return f'    <script type="application/ld+json">\n{text}\n    </script>\n'


def plain(markup: str) -> str:
    return html.unescape(re.sub(r"<[^>]+>", "", markup))


def head(lang: str, page: str, data: list[dict] | None = None) -> str:
    """The document head, the site header and the opening of <main>, with the page's structured data."""
    t = T[lang]
    seo = t["seo"][page]
    path = PAGES[page][lang]
    other = "fr" if lang == "en" else "en"
    data = list(data or [])
    if page != "home":
        data.append(
            {
                "@context": "https://schema.org",
                "@type": "BreadcrumbList",
                "itemListElement": [
                    {"@type": "ListItem", "position": 1, "name": t["crumb"], "item": ORIGIN + PAGES["home"][lang]},
                    {"@type": "ListItem", "position": 2, "name": t["nav"][page], "item": ORIGIN + path},
                ],
            }
        )
    nav = "".join(
        f'<li><a href="{PAGES[key][lang]}"{' aria-current="page"' if key == page else ""}>{label}</a></li>'
        for key, label in t["nav"].items()
    )
    nav += f'<li><a href="{DOCS[lang]}">Docs</a></li><li><a href="{REPO}">GitHub</a></li>'
    switch = "".join(
        f'<a href="{PAGES[page][code]}" hreflang="{code}" lang="{code}" data-lang="{code}" '
        f'aria-label="{T[code]["language_name"]}"{' aria-current="true"' if code == lang else ""}>{code.upper()}</a>'
        for code in ("fr", "en")
    )
    return (
        fill(
            "page-head.html",
            lang=lang,
            other=other,
            title=e(seo["title"]),
            description=e(seo["description"]),
            url=ORIGIN + path,
            other_url=ORIGIN + PAGES[page][other],
            origin=ORIGIN,
            locale=t["locale"],
            other_locale=T[other]["locale"],
            og_alt=e(t["og_alt"]),
            structured="".join(ld(d) for d in data),
            skip=t["skip"],
            brand=brand(lang),
            nav_label=t["nav_label"],
            nav=nav,
            lang_label=t["lang_label"],
            switch=switch,
            dmg=DMG,
            get_short=t["get_short"],
        )
        + "\n"
    )


def brand(lang: str) -> str:
    return (
        f'<a class="brand brand-remora" href="{PAGES["home"][lang]}" aria-label="{T[lang]["home_label"]}">'
        f'<span class="brand-disc">{FISH}</span><span class="brand-name">Remora</span></a>'
    )


def footer(lang: str) -> str:
    t = T[lang]
    columns = ""
    for key, column in t["footer"].items():
        lis = "".join(f'<li><a href="{href(lang, link["href"])}">{link["label"]}</a></li>' for link in column["links"])
        columns += f'<nav aria-labelledby="f-{key}"><h2 id="f-{key}">{column["title"]}</h2><ul>{lis}</ul></nav>'
    bottom = t["footer_bottom"]
    return (
        fill(
            "page-foot.html",
            brand=brand(lang),
            blurb=t["footer_blurb"],
            columns=columns,
            licence=bottom["licence"],
            credits=bottom["credits"],
        )
        + "\n"
    )


def screen(tab: str, lang: str, eager: bool = False, whole: bool = False, platform: str = "") -> str:
    """One capture. With a platform, it is tagged for site.js, which shows the Windows captures (from
    desktop/app/scripts/screenshots.mjs, hidden by default) to Windows and Linux visitors instead of the Mac ones."""
    base = f"/site/screens/{'windows/' if platform == 'windows' else ''}{tab}.{lang}"
    loading = 'fetchpriority="high"' if eager else 'loading="lazy"'
    cls = "screen screen-whole" if whole else "screen"
    tag = f' data-platform="{platform}"' if platform else ""
    if platform == "windows":
        tag += " hidden"
    return (
        f'<picture class="{cls}"{tag}>'
        f'<source media="(prefers-color-scheme: dark)" type="image/webp" srcset="{base}.dark.webp" />'
        f'<source media="(prefers-color-scheme: dark)" srcset="{base}.dark.png" />'
        f'<source type="image/webp" srcset="{base}.light.webp" />'
        f'<img src="{base}.light.png" alt="{e(T[lang]["screens"][tab])}" width="920" height="1400" {loading} decoding="async" />'
        f"</picture>"
    )


def screens(tab: str, lang: str) -> str:
    """The Mac capture, and the Windows one in its place for Windows and Linux visitors."""
    return screen(tab, lang, whole=True, platform="mac") + screen(tab, lang, whole=True, platform="windows")


def platform_note(lang: str, tag: str = "p") -> str:
    """Which app the captures above show, swapped by site.js like the captures."""
    note = T[lang]["platform_note"]
    return (
        f'<{tag} class="screen-platform"><span data-platform="mac">{note["mac"]}</span>'
        f'<span data-platform="windows" hidden>{note["windows"]}</span></{tag}>'
    )


def desk(lang: str, note: str) -> str:
    counters = "".join(
        f'<span class="counter"><span class="logo logo-{tool}"></span>{n}</span>'
        for tool, n in (("slack", 2), ("github", 3), ("gitlab", 1), ("notion", 1))
    )
    return fill(
        "desk.html",
        counters=counters,
        wifi=icon("wifi", 16, "glyph"),
        battery=icon("battery", 16, "glyph"),
        clock=T[lang]["clock"],
        screen=screen("myturn", lang, eager=True),
        note=note,
    )


def cards(items: list[dict], cls: str = "card card-field", accent: bool = False) -> str:
    icon_cls = "icon icon-accent" if accent else "icon"
    return "".join(
        f'<article class="{cls}"><span class="{icon_cls}">{icon(item["icon"])}</span>'
        f"<h3>{item['title']}</h3><p>{item['body']}</p></article>"
        for item in items
    )


def section_head(eyebrow: str, title: str, lead: str = "", tag: str = "h2", ident: str = "") -> str:
    lead_html = f'<p class="lead">{lead}</p>' if lead else ""
    return f'<div class="section-head"><span class="eyebrow">{eyebrow}</span><{tag} id="{ident}">{title}</{tag}>{lead_html}</div>'


def cta_button(lang: str, cls: str = "btn btn-primary btn-lg") -> str:
    return f'<a class="{cls}" href="{DMG}" data-download="mac">{T[lang]["get"]} {icon("drag", 20)}</a>'


def windows_button(lang: str, cls: str = "btn btn-secondary btn-lg") -> str:
    return f'<a class="{cls}" href="{EXE}" data-download="windows">{T[lang]["get_windows"]}</a>'


def linux_button(lang: str, cls: str = "btn btn-secondary btn-lg") -> str:
    """The AppImage, which updates itself; the .deb is in the FAQ and the guide."""
    return f'<a class="{cls}" href="{APPIMAGE}" data-download="linux">{T[lang]["get_linux"]}</a>'


def downloads(lang: str) -> str:
    return cta_button(lang) + windows_button(lang) + linux_button(lang)


def faq(items: list[dict]) -> str:
    return (
        '<div class="faq">'
        + "".join(
            f"<details><summary>{item['question']}</summary><div><p>{item['answer']}</p></div></details>"
            for item in items
        )
        + "</div>"
    )


def mdm_rows(lang: str) -> list[list[str]]:
    """The privacy page's MDM table: every policy key, its type and its meaning."""
    types = T[lang]["privacy"]["mdm_types"]
    rows = []
    for name, kind, _, en, _, fr in POLICY_KEYS:
        rows.append([f"<code>{name}</code>", e(types[kind]), e(fr if lang == "fr" else en)])
    return rows


def table(caption: str, columns: list[str], rows: list[list[str]]) -> str:
    head_html = "".join(f'<th scope="col">{c}</th>' for c in columns)
    body = "".join(
        "<tr>" + f'<th scope="row">{r[0]}</th>' + "".join(f"<td>{c}</td>" for c in r[1:]) + "</tr>" for r in rows
    )
    return (
        f'<div class="table-wrap"><table><caption class="visually-hidden">{caption}</caption>'
        f"<thead><tr>{head_html}</tr></thead><tbody>{body}</tbody></table></div>"
    )


def line_art(lang: str) -> str:
    art = T[lang]["line_art"]
    return fill("line-art.html", label=e(art["label"]), when=art["when"], at=e(art["at"]), hint=e(art["hint"]))


# ---------------------------------------------------------------- home


def home(lang: str) -> str:
    t = T[lang]
    c = t["home"]
    facts = "".join(
        f'<div class="fact"><strong>{f["value"]}</strong><span>{f["label"]}</span></div>' for f in c["facts"]
    )
    tabs = "".join(
        f"<li><figure>{screens(tab['tab'], lang)}<figcaption><strong>{tab['name']}</strong>{tab['text']}</figcaption></figure></li>"
        for tab in c["tabs"]
    )
    verbs = "".join(
        f'<li{' class="is-quiet"' if verb["icon"] in c["quiet"] else ""}><span class="dot">{icon(verb["icon"], 13)}</span>{verb["label"]}</li>'
        for verb in c["verbs"]
    )
    sources = "".join(
        f'<article class="source"><span class="logo logo-{s["tool"]}"></span><div><h3>{s["name"]}'
        f"{f'<span class="soon">{c["soon"][s["tool"]]}</span>' if s['tool'] in c['soon'] else ''}</h3><p>{s['text']}</p></div></article>"
        for s in c["sources"]
    )
    structured = [
        {
            "@context": "https://schema.org",
            "@type": "SoftwareApplication",
            "name": "Remora",
            "url": ORIGIN + PAGES["home"][lang],
            "description": t["description"],
            "applicationCategory": "BusinessApplication",
            "applicationSubCategory": "Productivity",
            "operatingSystem": "macOS 14 or later, Windows 10 or 11, Linux",
            "softwareVersion": VERSION,
            "inLanguage": ["en", "fr"],
            "isAccessibleForFree": True,
            "license": LICENSE_URL,
            "downloadUrl": DMG,
            "installUrl": DMG,
            "screenshot": [
                ORIGIN + f"/site/screens/{tab}.{lang}.light.png" for tab in ("myturn", "waiting", "snoozed")
            ],
            "image": ORIGIN + f"/site/og.{lang}.png",
            "codeRepository": REPO,
            "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"},
        },
        {
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "mainEntity": [
                {
                    "@type": "Question",
                    "name": item["question"],
                    "acceptedAnswer": {"@type": "Answer", "text": plain(item["answer"])},
                }
                for item in c["faq"]
            ],
        },
    ]
    body = fill(
        "home.html",
        eyebrow=c["eyebrow"],
        h1=c["h1"],
        lead=c["lead"],
        ctas=downloads(lang),
        note=c["note"],
        info=icon("info", 18),
        notice=c["notice"],
        desk=desk(lang, c["desk_note"]),
        facts=facts,
        tabs_head=section_head(c["tabs_eyebrow"], c["tabs_title"], c["tabs_lead"], ident="tabs-title"),
        tabs=tabs,
        platform_note=platform_note(lang),
        verbs_eyebrow=c["verbs_eyebrow"],
        verbs_title=c["verbs_title"],
        verbs_lead=c["verbs_lead"],
        verbs=verbs,
        verbs_more=c["verbs_more"],
        line_art=line_art(lang),
        line_eyebrow=c["line_eyebrow"],
        line_title=c["line_title"],
        line_lead=c["line_lead"],
        line_more=c["line_more"],
        features_head=section_head(
            c["features_eyebrow"], c["features_title"], c["features_lead"], ident="features-title"
        ),
        features=cards(c["features"]),
        features_href=PAGES["features"][lang],
        features_more=c["features_more"],
        sources_head=section_head(c["sources_eyebrow"], c["sources_title"], c["sources_lead"], ident="sources-title"),
        sources=sources,
        why_head=section_head(c["why_eyebrow"], c["why_title"], c["why_lead"], ident="why-title"),
        why=cards(c["why"]),
        privacy_eyebrow=c["privacy_eyebrow"],
        privacy_title=c["privacy_title"],
        privacy_lead=c["privacy_lead"],
        privacy=cards(c["privacy"], cls="card"),
        privacy_href=PAGES["privacy"][lang],
        privacy_more=c["privacy_more"],
        faq_eyebrow=c["faq_eyebrow"],
        faq_title=c["faq_title"],
        faq=faq(c["faq"]),
        final_title=c["final_title"],
        final_lead=c["final_lead"],
        final_ctas=cta_button(lang)
        + windows_button(lang, "btn btn-ink btn-lg")
        + linux_button(lang, "btn btn-ink btn-lg"),
    )
    return head(lang, "home", structured) + body + "\n" + footer(lang)


# ---------------------------------------------------------------- features


def features(lang: str) -> str:
    t = T[lang]
    c = t["features"]
    groups = ""
    for group in c["groups"]:
        key = group["key"]
        shot = ""
        if key in SHOTS:
            shot = f'<figure class="shot shot-narrow">{screens(SHOTS[key], lang)}{platform_note(lang, "figcaption")}</figure>'
        groups += "\n" + fill(
            "features-group.html",
            key=key,
            name=group["name"],
            intro=group["intro"],
            cards=cards(group["items"]),
            shot=shot,
        )
    body = fill(
        "features.html",
        eyebrow=t["nav"]["features"],
        title=c["title"],
        lead=c["lead"],
        ctas=downloads(lang),
        subnav="".join(f'<li><a href="#{group["key"]}">{group["name"]}</a></li>' for group in c["groups"]),
        groups=groups,
    )
    return head(lang, "features") + body + "\n" + footer(lang)


# ---------------------------------------------------------------- privacy


def mdm_sample() -> str:
    """The example policy as the plist keys of a configuration profile, escaped and highlighted for the code card."""
    lines = [f'<span class="tok-note">&lt;!-- Preference domain {DOMAIN} --&gt;</span>']
    for key, value in POLICY.items():
        name = f'&lt;key&gt;<span class="tok-key">{e(key)}</span>&lt;/key&gt;'
        if isinstance(value, bool):
            lines.append(name + ("&lt;true/&gt;" if value else "&lt;false/&gt;"))
        elif isinstance(value, list):
            lines += [
                name,
                "&lt;array&gt;",
                *(f"  &lt;string&gt;{e(v)}&lt;/string&gt;" for v in value),
                "&lt;/array&gt;",
            ]
        else:
            lines.append(f"{name}&lt;string&gt;{e(str(value))}&lt;/string&gt;")
    return "\n".join(lines)


def privacy(lang: str) -> str:
    c = T[lang]["privacy"]
    body = fill(
        "privacy.html",
        eyebrow=c["eyebrow"],
        h1=c["h1"],
        lead=c["lead"],
        out_head=section_head(c["out_eyebrow"], c["out_title"], c["out_lead"], ident="out-title"),
        out_table=table(c["out_title"], c["out_cols"], c["out_rows"]),
        stay_eyebrow=c["stay_eyebrow"],
        stay_title=c["stay_title"],
        stay_table=table(c["stay_title"], c["stay_cols"], c["stay_rows"]),
        enforce_eyebrow=c["enforce_eyebrow"],
        enforce_title=c["enforce_title"],
        enforce=cards(c["enforce"], cls="card"),
        mdm_eyebrow=c["mdm_eyebrow"],
        mdm_title=c["mdm_title"],
        mdm_lead=c["mdm_lead"],
        mdm_caption=c["mdm_caption"],
        mdm_sample=mdm_sample(),
        mdm_table=table(c["mdm_title"], c["mdm_cols"], mdm_rows(lang)),
        info=icon("info", 18),
        windows=c["windows"],
        more_href=c["more_href"],
        more=c["more"],
    )
    return head(lang, "privacy") + body + "\n" + footer(lang)


# ---------------------------------------------------------------- bilingual pages


def bilingual_head(title: str, description: str, extra: str = "", lang: str = "fr") -> str:
    return fill("bilingual-head.html", lang=lang, extra=extra, title=e(title), description=e(description))


def picker() -> str:
    # The x-default page every unmatched visitor gets: English first, both languages side by side, and a
    # description short enough to show whole in results.
    en, fr = T["en"], T["fr"]
    title = f"Remora — {en['picker']['choose']} · {fr['picker']['choose']}"
    description = f"{en['picker']['description']} {en['picker']['languages']} · {fr['picker']['languages']}."
    return (
        fill(
            "picker.html",
            head=bilingual_head(title, description, " data-lang-picker", lang="en"),
            origin=ORIGIN,
            brand=brand("en"),
            tagline_en=en["tagline"],
            tagline_fr=fr["tagline"],
            name_en=en["language_name"],
            name_fr=fr["language_name"],
        )
        + "\n"
    )


def not_found() -> str:
    en, fr = T["en"]["not_found"], T["fr"]["not_found"]
    return (
        fill(
            "404.html",
            head=bilingual_head(f"{fr['title']} · {en['title']} — Remora", f"{fr['heading']} {en['heading']}"),
            brand=brand("fr"),
            title_fr=fr["title"],
            title_en=en["title"],
            heading_fr=fr["heading"],
            text_fr=fr["text"],
            home_fr=fr["home"],
            heading_en=en["heading"],
            text_en=en["text"],
            home_en=en["home"],
        )
        + "\n"
    )


def copy_date() -> str:
    """When the pages' copy last changed (this file, the copy and the templates, in git), not the build date: a
    lastmod that changes at every build tells search engines nothing."""
    try:
        date = subprocess.run(
            ["git", "log", "-1", "--format=%cs", "--", __file__, SITE / "copy", SITE / "templates"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        date = ""
    return date or datetime.date.today().isoformat()


def sitemap() -> str:
    lastmod = copy_date()
    urls = "".join(
        f"  <url><loc>{ORIGIN}{paths[lang]}</loc><lastmod>{lastmod}</lastmod>"
        + "".join(
            f'<xhtml:link rel="alternate" hreflang="{code}" href="{ORIGIN}{paths[code]}"/>' for code in ("en", "fr")
        )
        + "</url>\n"
        for paths in PAGES.values()
        for lang in ("en", "fr")
    )
    return f'<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n{urls}</urlset>\n'


def llms() -> str:
    t = T["en"]
    pages = "".join(
        f"- [{t['seo'][key]['title']}]({ORIGIN}{PAGES[key]['en']}): {t['seo'][key]['description']}\n" for key in PAGES
    )
    return (
        fill(
            "llms.txt",
            description=t["description"],
            tools=t["tools"],
            origin=ORIGIN,
            pages=pages,
            dmg=DMG,
            exe=EXE,
            repo=REPO,
        )
        + "\n"
    )


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")
    print(f"✓ {path}")


if __name__ == "__main__":
    for lang in ("en", "fr"):
        write(f"{lang}/index.html", home(lang))
        write(PAGES["features"][lang].lstrip("/") + ".html", features(lang))
        write(PAGES["privacy"][lang].lstrip("/") + ".html", privacy(lang))
    write("index.html", picker())
    write("404.html", not_found())
    write("sitemap.xml", sitemap())
    write("llms.txt", llms())
    # The documentation has its own sitemap (VitePress): advertised too, so its pages aren't found by crawling only.
    write(
        "robots.txt", f"User-agent: *\nAllow: /\n\nSitemap: {ORIGIN}/sitemap.xml\nSitemap: {ORIGIN}/docs/sitemap.xml\n"
    )
