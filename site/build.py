#!/usr/bin/env python3
"""Writes the Remora website (public/) in English and French from the copy below. Standard library only.

    python3 site/build.py     then    cd site && npx wrangler deploy

Screenshots come from macos/scripts/screenshots.sh. Same design system as Myna (site.css, tokens.css).
"""
from __future__ import annotations

import datetime
import html
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).parent / "public"
ORIGIN = "https://triage.iscor.me"
REPO = "https://github.com/iGitScor/triage"
DMG = REPO + "/releases/latest/download/Remora.dmg"
EXE = REPO + "/releases/latest/download/Remora-Setup.exe"
VERSION = "0.2.0"
LICENSE_URL = "https://www.gnu.org/licenses/gpl-3.0.html"
DOCS = {"en": "/docs/", "fr": "/docs/fr/"}

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


def icon(name: str, size: int = 22, cls: str = "") -> str:
    attrs = f' class="{cls}"' if cls else ""
    return (
        f'<svg{attrs} width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
        f'stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{ICONS[name]}</svg>'
    )


def e(text: str) -> str:
    return html.escape(text, quote=True)


# ---------------------------------------------------------------- copy

PAGES = {
    "home": {"en": "/en/", "fr": "/fr/"},
    "features": {"en": "/en/features", "fr": "/fr/fonctionnalites"},
    "privacy": {"en": "/en/privacy", "fr": "/fr/confidentialite"},
}

T = {
    "en": {
        "locale": "en_US",
        "tagline": "Everything that needs you. Nothing that doesn’t.",
        "description": "Remora is a menu bar inbox for your Mac: code reviews, merge requests, Slack mentions and Linear issues, sorted by what you have to do. On-device, no account, only the tools you allow.",
        "skip": "Skip to content",
        "nav": {"features": "Features", "privacy": "Privacy & compliance"},
        "nav_label": "Main navigation",
        "lang_label": "Language",
        "home_label": "Remora, home",
        "get": "Download for Mac",
        "get_windows": "Download for Windows",
        "get_short": "Download",
        "source": "Source on GitHub",
        "footer_blurb": "A menu bar inbox for everything that needs you. It runs on your Mac and talks only to the tools you allow.",
        "footer": {
            "product": ("Product", [("features", "Features"), ("privacy", "Privacy & compliance"), (DMG, "Download for Mac"), (EXE, "Download for Windows (preview)"), (REPO + "/releases", "Release notes")]),
            "resources": ("Resources", [("/docs/", "Documentation"), ("/docs/guide/connecting-tools", "Connecting your tools"), ("/docs/admin/", "For IT and compliance"), ("/docs/develop/plugins", "Writing a plugin"), (REPO, "Source code on GitHub")]),
            "trust": ("Trust", [(REPO + "/blob/main/LICENSE", "GPL-3.0-or-later licence"), (REPO + "/security", "Report a vulnerability"), ("/docs/admin/code-signing", "Code signing policy"), (REPO + "/issues", "Questions and feedback")]),
        },
        "footer_bottom": ("Remora is free software under the GPL-3.0-or-later licence, built with the help of Claude.", "Tool logos: Simple Icons (CC0) · Outfit font (OFL)"),
        "clock": "Thu 9:41",
        "og_alt": "Remora: everything that needs you, in your Mac’s menu bar. The inbox with replies, reviews and tasks sorted by what to do.",
        "seo": {
            "home": ("Remora — Menu bar inbox for GitHub, GitLab, Slack and Linear", "Code reviews, merge requests, Slack mentions and Linear issues in one Mac menu bar inbox, sorted by what to do. Free, open source, on-device, no account."),
            "features": ("Features — Remora, the menu bar inbox for developers", "Verbs instead of apps, Done and Pin, snooze with a reason, reminders dragged from the menu bar, review prep and sessions, a waiting assistant. All of Remora."),
            "privacy": ("Privacy & compliance — Remora sends nothing but to your tools", "No server, no account, no telemetry: Remora talks only to the tools you allow, with AI off by default and a policy IT can lock with MDM. Every data flow."),
        },
        "crumb": "Home",
        "screens": {
            "myturn": "The My turn tab: replies, reviews with their estimate, your merge requests and tasks, grouped by what to do",
            "waiting": "The Waiting tab: merge requests waiting on reviewers, with a suggested nudge",
            "snoozed": "The Snoozed tab: items coming back on Monday and an insight about the ones you keep putting off",
        },
    },
    "fr": {
        "locale": "fr_FR",
        "tagline": "Tout ce qui a besoin de vous. Rien de plus.",
        "description": "Remora est une boîte de réception dans la barre des menus du Mac : relectures de code, merge requests, mentions Slack et tickets Linear, rangés par ce que vous avez à faire. Sur l’appareil, sans compte, seulement les outils autorisés.",
        "skip": "Aller au contenu",
        "nav": {"features": "Fonctionnalités", "privacy": "Confidentialité et conformité"},
        "nav_label": "Navigation principale",
        "lang_label": "Langue",
        "home_label": "Remora, accueil",
        "get": "Télécharger pour Mac",
        "get_windows": "Télécharger pour Windows",
        "get_short": "Télécharger",
        "source": "Code source sur GitHub",
        "footer_blurb": "Une boîte de réception dans la barre des menus pour tout ce qui a besoin de vous. Elle tourne sur votre Mac et ne parle qu’aux outils autorisés.",
        "footer": {
            "product": ("Produit", [("features", "Fonctionnalités"), ("privacy", "Confidentialité et conformité"), (DMG, "Télécharger pour Mac"), (EXE, "Télécharger pour Windows (aperçu)"), (REPO + "/releases", "Notes de version")]),
            "resources": ("Ressources", [("/docs/fr/", "Documentation"), ("/docs/fr/guide/connecter-vos-outils", "Connecter vos outils"), ("/docs/fr/admin/", "DSI et conformité"), ("/docs/fr/developper/extensions", "Écrire une extension"), (REPO, "Code source sur GitHub")]),
            "trust": ("Confiance", [(REPO + "/blob/main/LICENSE", "Licence GPL-3.0-or-later"), (REPO + "/security", "Signaler une vulnérabilité"), ("/docs/fr/admin/signature-du-code", "Politique de signature du code"), (REPO + "/issues", "Questions et retours")]),
        },
        "footer_bottom": ("Remora est un logiciel libre sous licence GPL-3.0-or-later, conçu avec l’aide de Claude.", "Logos des outils : Simple Icons (CC0) · police Outfit (OFL)"),
        "clock": "jeu. 9:41",
        "og_alt": "Remora : tout ce qui a besoin de vous, dans la barre des menus du Mac. La boîte avec réponses, relectures et tâches rangées par ce qu’il faut faire.",
        "seo": {
            "home": ("Remora — Boîte de réception GitHub, GitLab, Slack et Linear pour Mac", "Relectures de code, merge requests, mentions Slack et tickets Linear dans une seule boîte, dans la barre des menus du Mac. Gratuit, open source, sans compte."),
            "features": ("Fonctionnalités — Remora, la boîte de réception des développeurs", "Des verbes plutôt que des applis, Terminé et Épingler, reporter avec une raison, des rappels tirés de la barre des menus, préparer ses relectures. Tout Remora."),
            "privacy": ("Confidentialité et conformité — Remora n’envoie rien ailleurs", "Ni serveur, ni compte, ni télémétrie : Remora ne parle qu’aux outils autorisés, IA désactivée par défaut, politique verrouillable par MDM. Chaque flux."),
        },
        "crumb": "Accueil",
        "screens": {
            "myturn": "L’onglet À moi : réponses, relectures avec leur estimation, vos merge requests et tâches, rangées par ce qu’il y a à faire",
            "waiting": "L’onglet En attente : les merge requests qui attendent des relecteurs, avec une relance suggérée",
            "snoozed": "L’onglet Reportés : ce qui revient lundi, et un conseil sur ce que vous repoussez sans cesse",
        },
    },
}


def href(lang: str, target: str) -> str:
    return PAGES[target][lang] if target in PAGES else target


# ---------------------------------------------------------------- layout


def ld(data: dict) -> str:
    text = json.dumps(data, ensure_ascii=False, indent=2).replace("</", "<\\/")
    return f'    <script type="application/ld+json">\n{text}\n    </script>\n'


def plain(markup: str) -> str:
    return html.unescape(re.sub(r"<[^>]+>", "", markup))


def head(lang: str, page: str, title: str, description: str, data: list[dict] | None = None) -> str:
    t = T[lang]
    title, description = t["seo"][page]
    path = PAGES[page][lang]
    other = "fr" if lang == "en" else "en"
    data = list(data or [])
    if page != "home":
        data.append({
            "@context": "https://schema.org",
            "@type": "BreadcrumbList",
            "itemListElement": [
                {"@type": "ListItem", "position": 1, "name": t["crumb"], "item": ORIGIN + PAGES["home"][lang]},
                {"@type": "ListItem", "position": 2, "name": t["nav"][page], "item": ORIGIN + path},
            ],
        })
    structured = "".join(ld(d) for d in data)
    return f"""<!doctype html>
<html lang="{lang}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{e(title)}</title>
    <meta name="description" content="{e(description)}" />
    <link rel="canonical" href="{ORIGIN}{path}" />
    <link rel="alternate" hreflang="{lang}" href="{ORIGIN}{path}" />
    <link rel="alternate" hreflang="{other}" href="{ORIGIN}{PAGES[page][other]}" />
    <link rel="alternate" hreflang="x-default" href="{ORIGIN}/" />
    <meta name="color-scheme" content="light dark" />
    <meta name="theme-color" media="(prefers-color-scheme: light)" content="#f0f0e8" />
    <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0e0e14" />
    <link rel="icon" type="image/svg+xml" href="/site/favicon.svg" />
    <link rel="apple-touch-icon" href="/site/apple-touch-icon.png" />
    <link rel="preload" href="/site/outfit.woff2" as="font" type="font/woff2" crossorigin />
    <link rel="stylesheet" href="/site/tokens.css" />
    <link rel="stylesheet" href="/site/site.css" />
    <link rel="stylesheet" href="/site/remora.css" />
    <meta property="og:type" content="website" />
    <meta property="og:site_name" content="Remora" />
    <meta property="og:locale" content="{t['locale']}" />
    <meta property="og:locale:alternate" content="{T[other]['locale']}" />
    <meta property="og:url" content="{ORIGIN}{path}" />
    <meta property="og:title" content="{e(title)}" />
    <meta property="og:description" content="{e(description)}" />
    <meta property="og:image" content="{ORIGIN}/site/og.{lang}.png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:image:alt" content="{e(t['og_alt'])}" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content="{e(title)}" />
    <meta name="twitter:description" content="{e(description)}" />
    <meta name="twitter:image" content="{ORIGIN}/site/og.{lang}.png" />
    <script src="/site/site.js" defer></script>
{structured}  </head>
  <body>
    <a class="skip-link" href="#main">{t['skip']}</a>
{header(lang, page)}
    <main id="main">
"""


def brand(lang: str) -> str:
    return (
        f'<a class="brand brand-remora" href="{PAGES["home"][lang]}" aria-label="{T[lang]["home_label"]}">'
        f'<span class="brand-disc">{FISH}</span><span class="brand-name">Remora</span></a>'
    )


def header(lang: str, page: str) -> str:
    t = T[lang]
    items = "".join(
        f'<li><a href="{PAGES[key][lang]}"{" aria-current=\"page\"" if key == page else ""}>{label}</a></li>'
        for key, label in t["nav"].items()
    )
    items += f'<li><a href="{DOCS[lang]}">Docs</a></li><li><a href="{REPO}">GitHub</a></li>'
    switch = "".join(
        f'<a href="{PAGES[page][code]}" hreflang="{code}" lang="{code}" data-lang="{code}" aria-label="{name}"'
        f'{" aria-current=\"true\"" if code == lang else ""}>{code.upper()}</a>'
        for code, name in (("fr", "Français"), ("en", "English"))
    )
    return f"""    <header class="site-header">
      <div class="wrap header-bar">
        {brand(lang)}
        <nav class="site-nav" aria-label="{t['nav_label']}"><ul>{items}</ul></nav>
        <div class="header-actions">
          <nav class="lang-switch" aria-label="{t['lang_label']}">{switch}</nav>
          <a class="btn btn-primary btn-sm" href="{DMG}">{t['get_short']}</a>
        </div>
      </div>
    </header>"""


def footer(lang: str) -> str:
    t = T[lang]
    columns = ""
    for key, (title, links) in t["footer"].items():
        lis = "".join(f'<li><a href="{href(lang, target)}">{label}</a></li>' for target, label in links)
        columns += f'<nav aria-labelledby="f-{key}"><h2 id="f-{key}">{title}</h2><ul>{lis}</ul></nav>'
    left, right = t["footer_bottom"]
    return f"""    </main>
    <footer class="site-footer">
      <div class="wrap">
        <div class="footer-grid">
          <div class="footer-brand">{brand(lang)}<p>{t['footer_blurb']}</p></div>
          {columns}
        </div>
        <div class="footer-bottom"><span>{left}</span><span>{right}</span></div>
      </div>
    </footer>
  </body>
</html>
"""


def screen(tab: str, lang: str, eager: bool = False, whole: bool = False) -> str:
    base = f"/site/screens/{tab}.{lang}"
    loading = 'fetchpriority="high"' if eager else 'loading="lazy"'
    cls = "screen screen-whole" if whole else "screen"
    return (
        f'<picture class="{cls}">'
        f'<source media="(prefers-color-scheme: dark)" type="image/webp" srcset="{base}.dark.webp" />'
        f'<source media="(prefers-color-scheme: dark)" srcset="{base}.dark.png" />'
        f'<source type="image/webp" srcset="{base}.light.webp" />'
        f'<img src="{base}.light.png" alt="{e(T[lang]["screens"][tab])}" width="800" height="1200" {loading} decoding="async" />'
        f"</picture>"
    )


def desk(lang: str, note: str) -> str:
    counters = "".join(
        f'<span class="counter"><span class="logo logo-{tool}"></span>{n}</span>'
        for tool, n in (("slack", 2), ("github", 3), ("gitlab", 1), ("notion", 1))
    )
    return f"""<div class="desk">
            <div class="menubar" aria-hidden="true">
              <span class="status-item"><span class="logo logo-fish"></span>{counters}</span>
              {icon("wifi", 16, "glyph")}{icon("battery", 16, "glyph")}<span>{T[lang]['clock']}</span>
            </div>
            <div class="desk-body"><div class="popover">{screen("myturn", lang, eager=True)}</div></div>
            <p class="desk-note">{note}</p>
          </div>"""


def cards(items: list[tuple[str, str, str]], cls: str = "card card-field", accent: bool = False) -> str:
    icon_cls = "icon icon-accent" if accent else "icon"
    return "".join(
        f'<article class="{cls}"><span class="{icon_cls}">{icon(name)}</span><h3>{title}</h3><p>{body}</p></article>'
        for name, title, body in items
    )


def section_head(eyebrow: str, title: str, lead: str = "", tag: str = "h2", ident: str = "") -> str:
    lead_html = f'<p class="lead">{lead}</p>' if lead else ""
    return f'<div class="section-head"><span class="eyebrow">{eyebrow}</span><{tag} id="{ident}">{title}</{tag}>{lead_html}</div>'


def cta_button(lang: str, cls: str = "btn btn-primary btn-lg") -> str:
    return f'<a class="{cls}" href="{DMG}">{T[lang]["get"]} {icon("drag", 20)}</a>'


def windows_button(lang: str, cls: str = "btn btn-secondary btn-lg") -> str:
    return f'<a class="{cls}" href="{EXE}">{T[lang]["get_windows"]}</a>'


def faq(items: list[tuple[str, str]]) -> str:
    return '<div class="faq">' + "".join(f"<details><summary>{q}</summary><div><p>{a}</p></div></details>" for q, a in items) + "</div>"


def table(caption: str, columns: tuple[str, ...], rows: list[tuple[str, ...]]) -> str:
    head_html = "".join(f'<th scope="col">{c}</th>' for c in columns)
    body = "".join("<tr>" + f'<th scope="row">{r[0]}</th>' + "".join(f"<td>{c}</td>" for c in r[1:]) + "</tr>" for r in rows)
    return (
        f'<div class="table-wrap"><table><caption class="visually-hidden">{caption}</caption>'
        f"<thead><tr>{head_html}</tr></thead><tbody>{body}</tbody></table></div>"
    )


def line_art(lang: str) -> str:
    when, at, hint = ("in 25 min", "10:06 · release to name it", "drag down: later") if lang == "en" else (
        "dans 25 min", "10:06 · relâchez pour le nommer", "plus bas : plus tard")
    label = (
        "Dragging the Remora fish down from the menu bar: a fishing line follows the pointer and a bubble reads in 25 minutes."
        if lang == "en"
        else "Le poisson Remora tiré vers le bas depuis la barre des menus : une ligne de pêche suit le pointeur et une bulle indique dans 25 minutes."
    )
    return f"""<figure class="line-art" role="img" aria-label="{e(label)}">
            <svg viewBox="0 0 520 340" xmlns="http://www.w3.org/2000/svg">
              <rect class="bar" x="-1" y="-1" width="522" height="38" />
              <rect class="highlight" x="292" y="6" width="66" height="24" rx="6" />
              <g class="ink" transform="translate(300 10) scale(0.56)">
                <path d="M27 51C18 44 10 36 6 36C10 45 10 57 6 66C10 66 18 58 27 51Z" transform="translate(-5 -35)" />
                <path fill-rule="evenodd" transform="translate(-5 -35)" d="M35 38H79A13 13 0 0 1 79 64H35A13 13 0 0 1 35 38ZM77 51a3 3 0 1 0 6 0a3 3 0 1 0-6 0Z" />
              </g>
              <rect class="soft" x="380" y="13" width="44" height="10" rx="5" />
              <rect class="soft" x="436" y="13" width="64" height="10" rx="5" />
              <path class="rod" d="M326 30 C 326 110, 250 150, 236 222" />
              <g class="catch">
                <path class="rod" d="M236 222 q -2 12 8 12" />
                <rect class="bubble" x="150" y="244" width="196" height="62" rx="16" />
                <text class="bubble-text" x="248" y="270" text-anchor="middle">{when}</text>
                <text class="bubble-sub" x="248" y="291" text-anchor="middle">{e(at)}</text>
                <text class="hint" x="372" y="282">↓ {e(hint)}</text>
              </g>
            </svg>
          </figure>"""


# ---------------------------------------------------------------- home


def home(lang: str) -> str:
    t = T[lang]
    if lang == "en":
        c = dict(
            eyebrow="Menu bar inbox · macOS · Windows",
            h1='Everything that<br /><span class="mark">needs you.</span>',
            lead="Nothing that doesn’t. Code reviews, your merge requests, Slack mentions and Linear issues, in one inbox in your menu bar. Sorted by what you have to do, not by where it came from.",
            secondary="Privacy & compliance",
            note="Free and open source, no account. Mac: macOS 14 or later; the first time, right-click Remora → Open. Windows 10 or 11, in preview: if SmartScreen warns, More info → Run anyway.",
            notice="Remora speaks English and French, and so does on-device sorting.",
            desk_note="The real app, with sample data. One fish in the menu bar, one count per tool.",
            facts=[("0", "servers, accounts or trackers of ours"), ("5", "tools: GitHub, GitLab, Slack, Linear and Notion"), ("5", "items per verb, the rest folded away"), ("100%", "of sorting and ranking done on your computer")],
            tabs_eyebrow="See it",
            tabs_title='Your turn, their turn, <span class="mark">later.</span>',
            tabs_lead="Three tabs instead of four apps. What waits on you, what you wait on, and what you chose to see again later.",
            tabs=[("myturn", "My turn", "Someone is waiting on you: a reply, a review, a fix, a task, a reminder."), ("waiting", "Waiting", "You’re waiting on others. Remora suggests reviewers and drafts the nudge."), ("snoozed", "Snoozed", "Out of sight until it’s time, or until there’s news. With a reason.")],
            verbs_eyebrow="Verbs, not apps",
            verbs_title='Sorted by <span class="mark">what to do.</span>',
            verbs_lead="A Slack question and a GitLab comment both land in To reply. Messages are sorted on your Mac with keyword rules and Apple’s on-device language model. No generative AI, nothing sent.",
            verbs=[("reply", "To reply"), ("review", "To review"), ("fix", "To fix"), ("merge", "Ready to merge"), ("todo", "To do"), ("bell", "Reminders"), ("read", "To read"), ("hourglass", "Waiting on others")],
            quiet={"read", "hourglass"},
            verbs_more="To read and Waiting are shown, never counted: the badge only counts what needs you.",
            line_eyebrow="Reminders",
            line_title='Cast a reminder <span class="mark">from the menu bar.</span>',
            line_lead="Grab the fish and pull down. The further you drag, the later it comes back: minutes, then hours, then days. Release, type what to remember, press Enter. Esc cancels.",
            line_more="Or press + in the inbox. Due reminders arrive as notifications with Snooze buttons.",
            features_eyebrow="Features",
            features_title='Less noise, <span class="mark">more done.</span>',
            features_lead="Every feature has one job: fewer things on your mind at once.",
            features=[
                ("done", "Done until it changes", "Mark it done and it stays gone, until a new commit, an approval or a reply brings it back."),
                ("moon", "Snooze with a reason", "Waiting for someone, no time now, needs focus. Each reason picks a sensible return time, and “until there’s news” is one of them."),
                ("stopwatch", "Review prep", "“~6 min · 4 files · tests ✓ · Auth” under each review request, from file paths and line counts only. Then a session, quick wins first."),
                ("people", "Waiting assistant", "No reviewer on your merge request? Suggestions from who usually reviews there. Silent for a day? A drafted nudge, which you copy. Remora never sends."),
                ("bell", "Notifications that act", "Approved, changes requested, checks failed, a reminder due. Mark it done or snooze it right from the notification."),
                ("star", "Learns your habits", "What you handle fast and what you push back, learned on your Mac. It breaks ties, and a small star says why."),
            ],
            features_more="All features",
            sources_eyebrow="Sources",
            sources_title='Your tools, <span class="mark">your accounts.</span>',
            sources_lead="Connect as many accounts as you need and name them: “Work GitLab”, “Client Slack”. Each one opens in its own app when it’s installed.",
            sources=[
                ("github", "GitHub", "Pull requests you opened and review requests. GitHub Enterprise too."),
                ("gitlab", "GitLab", "Your merge requests, the ones you review, approvals and pipelines. Self-hosted too."),
                ("slack", "Slack", "Mentions and direct messages, opened in the Slack app."),
                ("linear", "Linear", "Issues assigned to you with their priority and due date, plus unread mentions."),
                ("claude", "Claude (optional)", "A short brief on what to handle first. Off by default, and only if your organization allows it."),
                ("notion", "Notion", "Tasks assigned to you in the databases you pick."),
            ],
            soon={},
            privacy_eyebrow="Privacy & compliance",
            privacy_title="Built for teams with rules.",
            privacy_lead="No data leaves your Mac except to the tools you allow. Enforced in code, checked by tests, lockable by your IT.",
            privacy=[
                ("shield", "Only allowed destinations", "Each tool declares the hosts it may reach. Any other address is blocked, look-alike domains included."),
                ("sparkle", "External AI off by default", "Claude is opt-in. Your organization can forbid it, and then every AI feature disappears."),
                ("building", "Managed by your IT", "Allowed tools, AI and avatars can be locked with a configuration profile (MDM). Group Policy on Windows."),
            ],
            privacy_more="Every data flow, and how it is enforced",
            faq_title="Good to know",
            faq=[
                ("Is my data sent to a Remora server?", "There is no Remora server. The app talks directly to the tools you connect, keeps its data in your user folder and your tokens in the macOS Keychain. No telemetry, no analytics, no crash reports."),
                ("Do I need Claude?", "No. Sorting, ranking, snooze advice and review prep all run on your Mac without generative AI. Claude only adds optional briefs and summaries, and it is off until you, and your organization, allow it."),
                ("Does it work with GitHub Enterprise and self-hosted GitLab?", f'Yes. Enter your host when you connect the account; Remora only talks to that host. Setup for each tool: <a href="/docs/guide/connecting-tools">connecting your tools</a>.'),
                ("Is there a Windows version?", f'Yes, in preview: <a href="{EXE}">Remora-Setup.exe</a> for Windows 10 or 11, installed for your user only (no admin rights). Same rules, the same tools and the Claude assistant, same compliance model, with the policy in the registry for Group Policy or Intune. It sorts messages with keywords only. The installer isn’t signed yet: if SmartScreen warns, click More info → Run anyway.'),
                ("Why does macOS ask for my Keychain password?", "Remora keeps all your tokens in one Keychain item. macOS asks once per new build of an app that isn’t signed by a registered developer. Choose Always Allow."),
                ("macOS says it can’t check the app?", "Remora isn’t notarized yet. Right-click it and choose Open; on macOS 15, use System Settings → Privacy & Security → Open Anyway. macOS only asks once."),
                ("How much does it cost?", f'Nothing. Remora is free software under the <a href="{REPO}/blob/main/LICENSE">GPL-3.0-or-later</a> licence, and its source code is on <a href="{REPO}">GitHub</a>, so your security team can read exactly what it does.'),
            ],
            final_title='Your inbox,<br />without the <span class="mark">noise.</span>',
            final_lead="Connect one tool, see what needs you, close the other tabs.",
        )
    else:
        c = dict(
            eyebrow="Barre des menus · macOS · Windows",
            h1='Tout ce qui a<br /><span class="mark">besoin de vous.</span>',
            lead="Rien de plus. Relectures de code, vos merge requests, mentions Slack et tickets Linear, dans une seule boîte de réception, dans la barre des menus. Rangés par ce que vous avez à faire, pas par leur provenance.",
            secondary="Confidentialité et conformité",
            note="Gratuit et open source, sans compte. Mac : macOS 14 ou ultérieur ; la première fois, clic droit sur Remora → Ouvrir. Windows 10 ou 11, en aperçu : si SmartScreen prévient, Informations complémentaires → Exécuter quand même.",
            notice="Remora parle français et anglais, tout comme le tri sur l’appareil.",
            desk_note="La vraie app, avec des données d’exemple. Un poisson dans la barre des menus, un compteur par outil.",
            facts=[("0", "serveur, compte ou traceur de notre part"), ("5", "outils : GitHub, GitLab, Slack, Linear et Notion"), ("5", "éléments par verbe, le reste replié"), ("100 %", "du tri et du classement faits sur votre ordinateur")],
            tabs_eyebrow="En images",
            tabs_title='À vous, aux autres, <span class="mark">plus tard.</span>',
            tabs_lead="Trois onglets au lieu de quatre applis. Ce qui vous attend, ce que vous attendez, et ce que vous avez choisi de revoir plus tard.",
            tabs=[("myturn", "À moi", "Quelqu’un attend après vous : une réponse, une relecture, une correction, une tâche, un rappel."), ("waiting", "En attente", "Vous attendez les autres. Remora suggère des relecteurs et rédige la relance."), ("snoozed", "Reportés", "Hors de vue jusqu’au bon moment, ou jusqu’à du nouveau. Avec une raison.")],
            verbs_eyebrow="Des verbes, pas des applis",
            verbs_title='Rangé par <span class="mark">ce qu’il faut faire.</span>',
            verbs_lead="Une question sur Slack et un commentaire GitLab finissent tous deux dans À répondre. Les messages sont triés sur votre Mac par des règles de mots-clés et le modèle de langue d’Apple embarqué. Pas d’IA générative, rien n’est envoyé.",
            verbs=[("reply", "À répondre"), ("review", "À relire"), ("fix", "À corriger"), ("merge", "Prêtes à fusionner"), ("todo", "À faire"), ("bell", "Rappels"), ("read", "À lire"), ("hourglass", "En attente des autres")],
            quiet={"read", "hourglass"},
            verbs_more="À lire et En attente sont affichés, jamais comptés : le badge ne compte que ce qui a besoin de vous.",
            line_eyebrow="Rappels",
            line_title='Lancez un rappel <span class="mark">depuis la barre des menus.</span>',
            line_lead="Attrapez le poisson et tirez vers le bas. Plus vous descendez, plus il revient tard : des minutes, puis des heures, puis des jours. Relâchez, tapez ce qu’il faut retenir, Entrée. Échap annule.",
            line_more="Ou appuyez sur + dans la boîte. À l’heure dite, une notification arrive, avec des boutons pour reporter.",
            features_eyebrow="Fonctionnalités",
            features_title='Moins de bruit, <span class="mark">plus de fait.</span>',
            features_lead="Chaque fonctionnalité a un seul but : moins de choses en tête à la fois.",
            features=[
                ("done", "Terminé tant que rien ne change", "Marquez-le terminé et il disparaît, jusqu’à ce qu’un commit, une approbation ou une réponse le fasse revenir."),
                ("moon", "Reporter avec une raison", "En attente de quelqu’un, pas le temps, demande de la concentration. Chaque raison propose un retour adapté, dont « jusqu’à du nouveau »."),
                ("stopwatch", "Préparer la relecture", "« ~6 min · 4 fichiers · tests ✓ · Auth » sous chaque demande de relecture, à partir des chemins et du nombre de lignes seulement. Puis une session, les plus rapides d’abord."),
                ("people", "Assistant d’attente", "Pas de relecteur sur votre merge request ? Des suggestions parmi ceux qui relisent d’habitude. Silence depuis un jour ? Une relance rédigée, que vous copiez. Remora n’envoie jamais rien."),
                ("bell", "Des notifications qui agissent", "Approuvée, modifications demandées, tests en échec, un rappel à l’heure. Terminez ou reportez directement depuis la notification."),
                ("star", "Apprend vos habitudes", "Ce que vous traitez vite et ce que vous repoussez, appris sur votre Mac. Ça départage, et une petite étoile dit pourquoi."),
            ],
            features_more="Toutes les fonctionnalités",
            sources_eyebrow="Sources",
            sources_title='Vos outils, <span class="mark">vos comptes.</span>',
            sources_lead="Connectez autant de comptes que nécessaire et nommez-les : « GitLab boulot », « Slack client ». Chacun s’ouvre dans son appli quand elle est installée.",
            sources=[
                ("github", "GitHub", "Les pull requests que vous avez ouvertes et les demandes de relecture. GitHub Enterprise aussi."),
                ("gitlab", "GitLab", "Vos merge requests, celles que vous relisez, approbations et pipelines. Auto-hébergé aussi."),
                ("slack", "Slack", "Mentions et messages directs, ouverts dans l’appli Slack."),
                ("linear", "Linear", "Les tickets qui vous sont assignés, avec priorité et échéance, plus les mentions non lues."),
                ("claude", "Claude (facultatif)", "Un court brief sur quoi traiter d’abord. Désactivé par défaut, et seulement si votre organisation l’autorise."),
                ("notion", "Notion", "Les tâches qui vous sont assignées dans les bases choisies."),
            ],
            soon={},
            privacy_eyebrow="Confidentialité et conformité",
            privacy_title="Pensé pour les équipes qui ont des règles.",
            privacy_lead="Aucune donnée ne quitte votre Mac, sauf vers les outils autorisés. Appliqué dans le code, vérifié par des tests, verrouillable par votre DSI.",
            privacy=[
                ("shield", "Seulement les destinations autorisées", "Chaque outil déclare les adresses qu’il peut joindre. Toute autre adresse est bloquée, y compris les domaines qui imitent."),
                ("sparkle", "IA externe désactivée par défaut", "Claude est facultatif. Votre organisation peut l’interdire, et toutes les fonctions d’IA disparaissent."),
                ("building", "Géré par votre DSI", "Outils autorisés, IA et avatars se verrouillent par profil de configuration (MDM). Par stratégie de groupe sous Windows."),
            ],
            privacy_more="Chaque flux de données, et comment il est contrôlé",
            faq_title="Bon à savoir",
            faq=[
                ("Mes données passent-elles par un serveur Remora ?", "Il n’y a pas de serveur Remora. L’app parle directement aux outils connectés, garde ses données dans votre dossier utilisateur et vos jetons dans le trousseau de macOS. Pas de télémétrie, pas d’analytics, pas de rapports de plantage."),
                ("Faut-il Claude ?", "Non. Le tri, le classement, les conseils de report et la préparation des relectures tournent sur votre Mac, sans IA générative. Claude n’ajoute que des briefs et résumés facultatifs, désactivés tant que vous, et votre organisation, ne les autorisez pas."),
                ("Ça marche avec GitHub Enterprise et un GitLab auto-hébergé ?", f'Oui. Indiquez votre adresse en connectant le compte : Remora ne parle qu’à celle-ci. Configuration de chaque outil : <a href="/docs/fr/guide/connecter-vos-outils">connecter vos outils</a>.'),
                ("Existe-t-il une version Windows ?", f'Oui, en aperçu : <a href="{EXE}">Remora-Setup.exe</a> pour Windows 10 ou 11, installée pour votre utilisateur seulement (sans droits d’administrateur). Mêmes règles, les mêmes outils et l’assistant Claude, même modèle de conformité, avec la politique dans le registre pour la stratégie de groupe ou Intune. Elle trie les messages par mots-clés seulement. L’installeur n’est pas encore signé : si SmartScreen prévient, cliquez sur Informations complémentaires → Exécuter quand même.'),
                ("Pourquoi macOS demande-t-il le mot de passe du trousseau ?", "Remora garde tous vos jetons dans un seul élément du trousseau. macOS le demande une fois par nouvelle version d’une app qui n’est pas signée par un développeur enregistré. Choisissez Toujours autoriser."),
                ("macOS dit qu’il ne peut pas vérifier l’app ?", "Remora n’est pas encore notarisée. Clic droit, puis Ouvrir ; sous macOS 15, Réglages Système → Confidentialité et sécurité → Ouvrir quand même. macOS ne demande qu’une fois."),
                ("Combien ça coûte ?", f'Rien. Remora est un logiciel libre sous licence <a href="{REPO}/blob/main/LICENSE">GPL-3.0-or-later</a>, et son code source est sur <a href="{REPO}">GitHub</a> : votre équipe sécurité peut lire exactement ce qu’elle fait.'),
            ],
            final_title='Votre boîte,<br />sans le <span class="mark">bruit.</span>',
            final_lead="Connectez un outil, voyez ce qui a besoin de vous, fermez les autres onglets.",
        )

    facts = "".join(f'<div class="fact"><strong>{n}</strong><span>{label}</span></div>' for n, label in c["facts"])
    tabs = "".join(
        f'<li><figure>{screen(tab, lang, whole=True)}<figcaption><strong>{name}</strong>{text}</figcaption></figure></li>'
        for tab, name, text in c["tabs"]
    )
    verbs = "".join(
        f'<li{" class=\"is-quiet\"" if name in c["quiet"] else ""}><span class="dot">{icon(name, 13)}</span>{label}</li>'
        for name, label in c["verbs"]
    )
    sources = "".join(
        f'<article class="source"><span class="logo logo-{tool}"></span><div><h3>{name}'
        f'{f"<span class=\"soon\">{c["soon"][tool]}</span>" if tool in c["soon"] else ""}</h3><p>{text}</p></div></article>'
        for tool, name, text in c["sources"]
    )
    return (
        head(lang, "home", "", "", [
            {
                "@context": "https://schema.org",
                "@type": "SoftwareApplication",
                "name": "Remora",
                "url": ORIGIN + PAGES["home"][lang],
                "description": t["description"],
                "applicationCategory": "BusinessApplication",
                "applicationSubCategory": "Productivity",
                "operatingSystem": "macOS 14 or later, Windows 10 or 11",
                "softwareVersion": VERSION,
                "inLanguage": ["en", "fr"],
                "isAccessibleForFree": True,
                "license": LICENSE_URL,
                "downloadUrl": DMG,
                "installUrl": DMG,
                "screenshot": [ORIGIN + f"/site/screens/{tab}.{lang}.light.png" for tab in ("myturn", "waiting", "snoozed")],
                "image": ORIGIN + f"/site/og.{lang}.png",
                "codeRepository": REPO,
                "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"},
            },
            {
                "@context": "https://schema.org",
                "@type": "FAQPage",
                "mainEntity": [
                    {"@type": "Question", "name": q, "acceptedAnswer": {"@type": "Answer", "text": plain(a)}} for q, a in c["faq"]
                ],
            },
        ])
        + f"""      <section class="hero" aria-labelledby="hero-title">
        <div class="wrap hero-grid">
          <div>
            <span class="eyebrow">{c['eyebrow']}</span>
            <h1 id="hero-title">{c['h1']}</h1>
            <p class="lead">{c['lead']}</p>
            <div class="ctas">{cta_button(lang)}{windows_button(lang)}</div>
            <p class="cta-note">{c['note']}</p>
            <p class="notice">{icon("info", 18)}<span>{c['notice']}</span></p>
          </div>
          {desk(lang, c['desk_note'])}
        </div>
      </section>

      <section class="section-tight" aria-label="Remora">
        <div class="wrap"><div class="facts">{facts}</div></div>
      </section>

      <section class="section" aria-labelledby="tabs-title">
        <div class="wrap">
          {section_head(c['tabs_eyebrow'], c['tabs_title'], c['tabs_lead'], ident="tabs-title")}
          <ul class="tabs-show">{tabs}</ul>
        </div>
      </section>

      <section class="section" aria-labelledby="verbs-title">
        <div class="wrap feature-row">
          <div>
            <span class="eyebrow">{c['verbs_eyebrow']}</span>
            <h2 id="verbs-title">{c['verbs_title']}</h2>
            <p class="lead">{c['verbs_lead']}</p>
          </div>
          <div><ul class="verbs">{verbs}</ul><p class="cta-note">{c['verbs_more']}</p></div>
        </div>
      </section>

      <section class="section" aria-labelledby="line-title">
        <div class="wrap feature-row">
          {line_art(lang)}
          <div>
            <span class="eyebrow">{c['line_eyebrow']}</span>
            <h2 id="line-title">{c['line_title']}</h2>
            <p class="lead">{c['line_lead']}</p>
            <p class="cta-note">{c['line_more']}</p>
          </div>
        </div>
      </section>

      <section class="section" aria-labelledby="features-title">
        <div class="wrap">
          {section_head(c['features_eyebrow'], c['features_title'], c['features_lead'], ident="features-title")}
          <div class="grid grid-3">{cards(c['features'])}</div>
          <p class="cta-note"><a class="link-arrow" href="{PAGES['features'][lang]}">{c['features_more']}</a></p>
        </div>
      </section>

      <section class="section" aria-labelledby="sources-title">
        <div class="wrap">
          {section_head(c['sources_eyebrow'], c['sources_title'], c['sources_lead'], ident="sources-title")}
          <div class="sources">{sources}</div>
        </div>
      </section>

      <section class="band" aria-labelledby="privacy-title">
        <div class="wrap">
          <div class="section-head"><span class="eyebrow">{c['privacy_eyebrow']}</span><h2 id="privacy-title">{c['privacy_title']}</h2><p>{c['privacy_lead']}</p></div>
          <div class="grid grid-3">{cards(c['privacy'], cls="card")}</div>
          <p class="cta-note"><a class="link-arrow" href="{PAGES['privacy'][lang]}">{c['privacy_more']}</a></p>
        </div>
      </section>

      <section class="section" aria-labelledby="faq-title">
        <div class="wrap">
          <div class="section-head"><span class="eyebrow">FAQ</span><h2 id="faq-title">{c['faq_title']}</h2></div>
          {faq(c['faq'])}
        </div>
      </section>

      <section class="section final" aria-labelledby="final-title">
        <div class="wrap">
          <h2 id="final-title">{c['final_title']}</h2>
          <p class="lead">{c['final_lead']}</p>
          <div class="ctas">{cta_button(lang)}{windows_button(lang, "btn btn-ink btn-lg")}</div>
        </div>
      </section>
"""
        + footer(lang)
    )


# ---------------------------------------------------------------- features


def features(lang: str) -> str:
    t = T[lang]
    if lang == "en":
        title, lead = 'One inbox, <span class="mark">built to calm down.</span>', "Everything Remora does, grouped by the moment you need it."
        groups = [
            ("inbox", "The inbox", "Sorted by what to do, never more than you can take in.", [
                ("todo", "My turn and Waiting", "My turn holds what someone waits on you for. Waiting holds what you wait on others for. Snoozed and Done are one click away."),
                ("reply", "Verbs", "To reply, To review, To fix, Ready to merge, To do, Reminders, To read, Waiting on others. Messages are classified on your Mac, in English and French; when unsure, To reply."),
                ("done", "Done", "Hides an item until something changes on it. Clear the Done list in one click; cleared items still come back on new activity."),
                ("pin", "Pin", "Keeps an item at the top, whatever arrives next."),
                ("star", "Priority, then habits", "Overdue and due today first, then the tool’s priority (Linear Urgent, High…), the nearest due date, recent activity. Low priority is shown, never counted."),
                ("read", "Five at a time", "Long bundles show their top five and “Show N more”. A smart summary of the rest when Claude is allowed."),
            ]),
            ("menubar", "The menu bar", "Remora lives in the menu bar: no window to keep open.", [
                ("menu", "One count per tool", "The fish and a single number, or one count per tool, ordered by the most pressing verb. Start a task and it shows alone, with how long you’ve been on it."),
                ("drag", "Drag to remind", "Pull the fish down to cast a reminder; a bubble shows when it comes back. Further down means later."),
                ("swipe", "Gestures", "Two fingers on the trackpad: right to mark done, left to snooze."),
                ("link", "Opens in the right app", "Slack and Linear items open in their desktop app when it’s installed (⌥-click for the web), with the browser as fallback."),
            ]),
            ("time", "Time", "Snooze is a decision, so Remora helps you make a good one.", [
                ("moon", "Snooze or remind", "Hide an item until later, or keep it visible and get a reminder. Presets (later today, this evening, tomorrow, next week) or a scrubber: minutes, then hours, then days."),
                ("hourglass", "With a reason", "Waiting for someone, no time now, needs focus, not urgent, not feeling it. Each picks a return time; waiting can come back “until there’s news”."),
                ("star", "Insights", "Loops (snoozed three times), pile-ups on the same morning, items on the same topic, items gone quiet. One insight at a time, never a report."),
                ("bell", "Notifications with buttons", "New review requests, mentions, approvals, failed checks, due reminders. Done or Snooze without opening Remora. Hide the message text per tool, for screen shares."),
            ]),
            ("reviews", "Reviews", "Less context switching between your tools.", [
                ("stopwatch", "Review prep", "An estimate and what the change touches (migrations, auth, infrastructure, dependencies), from file paths and line counts only. Code is never stored."),
                ("review", "Review sessions", "Start session goes through your review requests one by one, quick wins first: ⏎ open, D done, S snooze, → skip."),
                ("link", "Links between tools", "A pull request and a Linear issue with a similar title are shown together, even when nobody linked them."),
                ("people", "Waiting assistant", "Suggested reviewers when there are none, a drafted nudge when they stay silent. Written locally; you copy it, Remora never sends."),
            ]),
            ("assistant", "Assistant (optional)", "Off by default. Only where your organization allows external AI.", [
                ("sparkle", "Brief", "Three sentences and the items to handle first, with Claude Code on your plan or the Claude API. Cached, so it doesn’t spend tokens on every open."),
                ("read", "Bundle summaries", "Two sentences on a busy bundle, with a lighter model. Reused while the bundle holds the same items."),
                ("moon", "Triage", "For a crowded Snoozed tab: keep, reschedule, let go or do now, item by item. You confirm every change."),
            ]),
        ]
        shots = {"time": "snoozed", "reviews": "waiting"}
        page_title = "Features — Remora"
        description = "Everything Remora does: verbs, Done and Pin, snooze with a reason, reminders from the menu bar, review prep and sessions, the waiting assistant, and an optional Claude brief."
    else:
        title, lead = 'Une boîte de réception <span class="mark">qui apaise.</span>', "Tout ce que fait Remora, rangé selon le moment où vous en avez besoin."
        groups = [
            ("inbox", "La boîte de réception", "Rangée par ce qu’il faut faire, jamais plus que ce que vous pouvez absorber.", [
                ("todo", "À moi et En attente", "À moi contient ce que l’on attend de vous. En attente, ce que vous attendez des autres. Reportés et Terminés sont à un clic."),
                ("reply", "Des verbes", "À répondre, À relire, À corriger, Prêtes à fusionner, À faire, Rappels, À lire, En attente des autres. Les messages sont classés sur votre Mac, en français et en anglais ; dans le doute, À répondre."),
                ("done", "Terminé", "Masque un élément jusqu’à ce qu’il change. Videz la liste en un clic ; les éléments effacés reviennent quand même s’il y a du nouveau."),
                ("pin", "Épingler", "Garde un élément en haut, quoi qu’il arrive ensuite."),
                ("star", "La priorité, puis vos habitudes", "En retard et à faire aujourd’hui d’abord, puis la priorité de l’outil (Linear Urgent, Haute…), l’échéance la plus proche, l’activité récente. La priorité basse est affichée, jamais comptée."),
                ("read", "Cinq à la fois", "Les longues listes montrent leurs cinq premiers et « Afficher N de plus ». Un résumé du reste quand Claude est autorisé."),
            ]),
            ("menubar", "La barre des menus", "Remora vit dans la barre des menus : aucune fenêtre à garder ouverte.", [
                ("menu", "Un compteur par outil", "Le poisson et un seul nombre, ou un compteur par outil, dans l’ordre du verbe le plus pressant. Commencez une tâche : elle s’affiche seule, avec le temps passé dessus."),
                ("drag", "Tirer pour un rappel", "Tirez le poisson vers le bas pour lancer un rappel ; une bulle indique quand il revient. Plus bas, c’est plus tard."),
                ("swipe", "Gestes", "Deux doigts sur le trackpad : à droite pour terminer, à gauche pour reporter."),
                ("link", "S’ouvre dans la bonne appli", "Les éléments Slack et Linear s’ouvrent dans leur appli quand elle est installée (⌥-clic pour le web), sinon dans le navigateur."),
            ]),
            ("time", "Le temps", "Reporter est une décision : Remora vous aide à bien la prendre.", [
                ("moon", "Reporter ou rappeler", "Masquez un élément jusqu’à plus tard, ou gardez-le visible avec un rappel. Des raccourcis (plus tard, ce soir, demain, semaine prochaine) ou un curseur : minutes, puis heures, puis jours."),
                ("hourglass", "Avec une raison", "En attente de quelqu’un, pas le temps, demande de la concentration, pas urgent, pas motivé. Chacune propose un retour ; l’attente peut durer « jusqu’à du nouveau »."),
                ("star", "Des conseils", "Les boucles (reporté trois fois), les embouteillages du même matin, les éléments sur un même sujet, ceux devenus silencieux. Un conseil à la fois, jamais un rapport."),
                ("bell", "Des notifications avec boutons", "Nouvelles relectures, mentions, approbations, tests en échec, rappels. Terminer ou Reporter sans ouvrir Remora. Masquez le texte des messages par outil, pour les partages d’écran."),
            ]),
            ("reviews", "Les relectures", "Moins d’allers-retours entre vos outils.", [
                ("stopwatch", "Préparer la relecture", "Une estimation et ce que touche la modification (migrations, authentification, infrastructure, dépendances), à partir des chemins et du nombre de lignes seulement. Le code n’est jamais stocké."),
                ("review", "Sessions de relecture", "Démarrer une session parcourt vos relectures une par une, les plus rapides d’abord : ⏎ ouvrir, D terminer, S reporter, → passer."),
                ("link", "Des liens entre outils", "Une pull request et un ticket Linear au titre proche sont présentés ensemble, même si personne ne les a liés."),
                ("people", "Assistant d’attente", "Des relecteurs suggérés quand il n’y en a pas, une relance rédigée quand ils restent silencieux. Écrite sur place ; vous la copiez, Remora n’envoie jamais rien."),
            ]),
            ("assistant", "L’assistant (facultatif)", "Désactivé par défaut. Seulement si votre organisation autorise l’IA externe.", [
                ("sparkle", "Brief", "Trois phrases et les éléments à traiter d’abord, avec Claude Code sur votre abonnement ou l’API Claude. Mis en cache, pour ne pas dépenser de jetons à chaque ouverture."),
                ("read", "Résumés de liste", "Deux phrases sur une liste chargée, avec un modèle plus léger. Réutilisé tant que la liste ne change pas."),
                ("moon", "Tri", "Pour un onglet Reportés encombré : garder, reprogrammer, laisser tomber ou faire maintenant, élément par élément. Vous validez chaque changement."),
            ]),
        ]
        shots = {"time": "snoozed", "reviews": "waiting"}
        page_title = "Fonctionnalités — Remora"
        description = "Tout ce que fait Remora : verbes, Terminé et Épingler, reporter avec une raison, rappels depuis la barre des menus, préparation et sessions de relecture, assistant d’attente, et un brief Claude facultatif."

    subnav = "".join(f'<li><a href="#{key}">{name}</a></li>' for key, name, _, _ in groups)
    body = ""
    for key, name, intro, items in groups:
        shot = ""
        if key in shots:
            shot = f'<figure class="shot shot-narrow">{screen(shots[key], lang, whole=True)}</figure>'
        body += f"""
      <section class="section-tight" id="{key}" aria-labelledby="{key}-title">
        <div class="wrap">
          <div class="section-head"><h2 id="{key}-title">{name}</h2><p>{intro}</p></div>
          <div class="grid grid-2">{cards(items)}</div>
          {shot}
        </div>
      </section>"""
    return (
        head(lang, "features", page_title, description)
        + f"""      <section class="page-hero" aria-labelledby="page-title">
        <div class="wrap">
          <span class="eyebrow">{t['nav']['features']}</span>
          <h1 id="page-title">{title}</h1>
          <p class="lead">{lead}</p>
          <div class="ctas">{cta_button(lang)}{windows_button(lang)}</div>
        </div>
      </section>
      <nav class="subnav" aria-label="{t['nav']['features']}"><div class="wrap"><ul>{subnav}</ul></div></nav>
{body}
"""
        + footer(lang)
    )


# ---------------------------------------------------------------- privacy


MDM = """<span class="tok-note">&lt;!-- Preference domain fr.igitscor.remora --&gt;</span>
&lt;key&gt;<span class="tok-key">AllowedPlugins</span>&lt;/key&gt;
&lt;array&gt;
  &lt;string&gt;github&lt;/string&gt;
  &lt;string&gt;gitlab&lt;/string&gt;
  &lt;string&gt;linear&lt;/string&gt;
&lt;/array&gt;
&lt;key&gt;<span class="tok-key">AllowExternalAI</span>&lt;/key&gt;&lt;false/&gt;
&lt;key&gt;<span class="tok-key">AllowRemoteImages</span>&lt;/key&gt;&lt;true/&gt;"""


def privacy(lang: str) -> str:
    t = T[lang]
    if lang == "en":
        c = dict(
            title="Privacy & compliance — Remora",
            description="Remora sends nothing anywhere except to the tools you allow. Every data flow, what stays on your Mac, how it is enforced, and how IT can lock the policy with MDM.",
            eyebrow="Privacy & compliance",
            h1='Only the tools <span class="mark">you allow.</span>',
            lead="Remora has no server, no account and no telemetry. It talks directly to the tools you connect, and to nothing else. Built for organizations with strict data rules.",
            out_eyebrow="On the network", out_title="What leaves your Mac",
            out_lead="Read requests to your own tools, with your own tokens. Data comes back and stays on your Mac.",
            out_cols=("Flow", "Destination", "What is sent", "Default"),
            out_rows=[
                ("GitHub", "<code>api.github.com</code>, <code>github.com</code>, avatars, or your GitHub Enterprise host", "Your token; read requests for your pull requests, review requests and changed file paths", "Allowed"),
                ("GitLab", "Your GitLab host", "Your token; read requests for your merge requests, approvals, pipelines and changed file paths", "Allowed"),
                ("Slack", "<code>slack.com</code>", "Your user token; searches for your mentions and direct messages", "Allowed"),
                ("Linear", "<code>api.linear.app</code>", "Your API key; read requests for assigned issues and notifications", "Allowed"),
                ("Claude", "Anthropic, through Claude Code or <code>api.anthropic.com</code>", "Titles, contexts, authors and statuses of inbox items", "<strong>Off</strong>"),
            ],
            stay_eyebrow="On your Mac", stay_title="What stays where",
            stay_cols=("Data", "Where"),
            stay_rows=[
                ("Inbox cache, states, reminders, snooze history, preferences", "<code>~/Library/Application Support/Remora</code>, protected by FileVault"),
                ("Tokens", "One item in your login Keychain"),
                ("What you handle fast or snooze", "<code>learning.json</code>, used for ranking on this Mac only"),
                ("Review prep", "File paths and line counts only; code is never stored"),
                ("Drafted messages", "Written from templates, copied to your clipboard, never sent"),
            ],
            enforce_eyebrow="Enforced, not promised", enforce_title='Checked in code, <span class="mark">covered by tests.</span>',
            enforce=[
                ("shield", "Declared destinations", "Every plugin declares the hosts it may reach. A test fails if one doesn’t."),
                ("lock", "One gate", "Plugins only get a network client limited to their own hosts. Anything else fails with “Blocked: not an allowed destination”, look-alike domains included."),
                ("sparkle", "External AI off by default", "Claude plugins are refused until external AI is allowed; then the brief, summaries and triage appear."),
                ("chip", "On-device intelligence", "Sorting messages, topic similarity, ranking and snooze advice use Apple’s on-device language models. Nothing is sent."),
                ("eyeoff", "No telemetry", "No analytics, no crash reporting, no update check unless you turn updates on. Fonts and logos are bundled, never downloaded."),
                ("trash", "Erase local data", "Settings → Privacy lists every connected account’s destinations, and erases everything Remora stored in one click."),
            ],
            mdm_eyebrow="For IT", mdm_title='Lock the policy <span class="mark">with MDM.</span>',
            mdm_lead="Deploy a configuration profile for the preference domain <code>fr.igitscor.remora</code>. Settings → Privacy then shows “Managed by your organization”, and users can’t change it.",
            mdm_cols=("Key", "Type", "Meaning"),
            mdm_rows=[
                ("<code>AllowedPlugins</code>", "Array of strings", "Allowed tools: <code>github</code>, <code>gitlab</code>, <code>slack</code>, <code>linear</code>, <code>notion</code>, <code>claude-code</code>, <code>claude</code>"),
                ("<code>AllowExternalAI</code>", "Boolean", "Allow Claude to receive inbox content"),
                ("<code>AllowRemoteImages</code>", "Boolean", "Load avatars from allowed tools only"),
                ("<code>AIExcludedSources</code>", "Array of strings", "Tools whose items never reach the assistant, such as <code>slack</code> (Mac)"),
                ("<code>AllowedAIModels</code>", "Array of strings", "The Claude models the assistant may use (Mac)"),
            ],
            mdm_caption="Configuration profile payload",
            windows="The Windows app follows the same model: Credential Manager for tokens, <code>%LOCALAPPDATA%\\fr.igitscor.remora</code> for data, and the policy under <code>HKLM\\SOFTWARE\\Policies\\Remora</code> (Group Policy or Intune).",
            more="Data flows, MDM profile and deployment, for IT and compliance teams",
            more_href="/docs/admin/",
        )
    else:
        c = dict(
            title="Confidentialité et conformité — Remora",
            description="Remora n’envoie rien ailleurs que vers les outils autorisés. Chaque flux de données, ce qui reste sur votre Mac, comment c’est appliqué, et comment la DSI verrouille la politique par MDM.",
            eyebrow="Confidentialité et conformité",
            h1='Seulement les outils <span class="mark">que vous autorisez.</span>',
            lead="Remora n’a ni serveur, ni compte, ni télémétrie. Elle parle directement aux outils connectés, et à rien d’autre. Pensée pour les organisations aux règles strictes sur les données.",
            out_eyebrow="Sur le réseau", out_title="Ce qui quitte votre Mac",
            out_lead="Des requêtes de lecture vers vos propres outils, avec vos propres jetons. Les données reviennent et restent sur votre Mac.",
            out_cols=("Flux", "Destination", "Ce qui est envoyé", "Par défaut"),
            out_rows=[
                ("GitHub", "<code>api.github.com</code>, <code>github.com</code>, avatars, ou votre GitHub Enterprise", "Votre jeton ; des lectures de vos pull requests, demandes de relecture et chemins des fichiers modifiés", "Autorisé"),
                ("GitLab", "Votre serveur GitLab", "Votre jeton ; des lectures de vos merge requests, approbations, pipelines et chemins des fichiers modifiés", "Autorisé"),
                ("Slack", "<code>slack.com</code>", "Votre jeton utilisateur ; des recherches de vos mentions et messages directs", "Autorisé"),
                ("Linear", "<code>api.linear.app</code>", "Votre clé d’API ; des lectures des tickets assignés et des notifications", "Autorisé"),
                ("Claude", "Anthropic, via Claude Code ou <code>api.anthropic.com</code>", "Titres, contextes, auteurs et statuts des éléments", "<strong>Désactivé</strong>"),
            ],
            stay_eyebrow="Sur votre Mac", stay_title="Ce qui reste, et où",
            stay_cols=("Données", "Emplacement"),
            stay_rows=[
                ("Cache, états, rappels, historique des reports, préférences", "<code>~/Library/Application Support/Remora</code>, protégé par FileVault"),
                ("Jetons", "Un seul élément de votre trousseau de session"),
                ("Ce que vous traitez vite ou reportez", "<code>learning.json</code>, pour le classement sur ce Mac seulement"),
                ("Préparation des relectures", "Chemins et nombre de lignes seulement ; le code n’est jamais stocké"),
                ("Messages rédigés", "Écrits à partir de modèles, copiés dans le presse-papiers, jamais envoyés"),
            ],
            enforce_eyebrow="Appliqué, pas promis", enforce_title='Contrôlé dans le code, <span class="mark">couvert par des tests.</span>',
            enforce=[
                ("shield", "Destinations déclarées", "Chaque extension déclare les adresses qu’elle peut joindre. Un test échoue sinon."),
                ("lock", "Un seul passage", "Les extensions ne reçoivent qu’un client réseau limité à leurs adresses. Tout le reste échoue avec « Bloqué : destination non autorisée », domaines imitateurs compris."),
                ("sparkle", "IA externe désactivée par défaut", "Les extensions Claude sont refusées tant que l’IA externe n’est pas autorisée ; ensuite seulement apparaissent brief, résumés et tri."),
                ("chip", "Intelligence sur l’appareil", "Le tri des messages, la similarité des sujets, le classement et les conseils de report utilisent les modèles de langue embarqués d’Apple. Rien n’est envoyé."),
                ("eyeoff", "Pas de télémétrie", "Pas d’analytics, pas de rapports de plantage, pas de vérification de mise à jour sauf si vous l’activez. Polices et logos sont intégrés, jamais téléchargés."),
                ("trash", "Effacer les données locales", "Réglages → Confidentialité liste les destinations de chaque compte connecté, et efface en un clic tout ce que Remora a stocké."),
            ],
            mdm_eyebrow="Pour la DSI", mdm_title='Verrouillez la politique <span class="mark">par MDM.</span>',
            mdm_lead="Déployez un profil de configuration pour le domaine de préférences <code>fr.igitscor.remora</code>. Réglages → Confidentialité affiche alors « Géré par votre organisation », et l’utilisateur ne peut plus la modifier.",
            mdm_cols=("Clé", "Type", "Signification"),
            mdm_rows=[
                ("<code>AllowedPlugins</code>", "Tableau de chaînes", "Outils autorisés : <code>github</code>, <code>gitlab</code>, <code>slack</code>, <code>linear</code>, <code>notion</code>, <code>claude-code</code>, <code>claude</code>"),
                ("<code>AllowExternalAI</code>", "Booléen", "Autoriser Claude à recevoir le contenu de la boîte"),
                ("<code>AllowRemoteImages</code>", "Booléen", "Charger les avatars depuis les outils autorisés seulement"),
                ("<code>AIExcludedSources</code>", "Tableau de chaînes", "Les outils dont les éléments ne parviennent jamais à l’assistant, comme <code>slack</code> (Mac)"),
                ("<code>AllowedAIModels</code>", "Tableau de chaînes", "Les modèles Claude que l’assistant peut utiliser (Mac)"),
            ],
            mdm_caption="Contenu du profil de configuration",
            windows="L’app Windows suit le même modèle : le Gestionnaire d’identification pour les jetons, <code>%LOCALAPPDATA%\\fr.igitscor.remora</code> pour les données, et la politique sous <code>HKLM\\SOFTWARE\\Policies\\Remora</code> (stratégie de groupe ou Intune).",
            more="Flux de données, profil MDM et déploiement, pour la DSI et la conformité",
            more_href="/docs/fr/admin/",
        )
    return (
        head(lang, "privacy", c["title"], c["description"])
        + f"""      <section class="page-hero" aria-labelledby="page-title">
        <div class="wrap">
          <span class="eyebrow">{c['eyebrow']}</span>
          <h1 id="page-title">{c['h1']}</h1>
          <p class="lead">{c['lead']}</p>
        </div>
      </section>

      <section class="section-tight" aria-labelledby="out-title">
        <div class="wrap">
          {section_head(c['out_eyebrow'], c['out_title'], c['out_lead'], ident="out-title")}
          {table(c['out_title'], c['out_cols'], c['out_rows'])}
        </div>
      </section>

      <section class="section" aria-labelledby="stay-title">
        <div class="wrap feature-row">
          <div><span class="eyebrow">{c['stay_eyebrow']}</span><h2 id="stay-title">{c['stay_title']}</h2></div>
          {table(c['stay_title'], c['stay_cols'], c['stay_rows'])}
        </div>
      </section>

      <section class="band" aria-labelledby="enforce-title">
        <div class="wrap">
          <div class="section-head"><span class="eyebrow">{c['enforce_eyebrow']}</span><h2 id="enforce-title">{c['enforce_title']}</h2></div>
          <div class="grid grid-3">{cards(c['enforce'], cls="card")}</div>
        </div>
      </section>

      <section class="section" aria-labelledby="mdm-title">
        <div class="wrap feature-row">
          <div>
            <span class="eyebrow">{c['mdm_eyebrow']}</span>
            <h2 id="mdm-title">{c['mdm_title']}</h2>
            <p class="lead">{c['mdm_lead']}</p>
          </div>
          <figure class="code-card">
            <figcaption><span class="dots" aria-hidden="true"><i></i><i></i><i></i></span><span>{c['mdm_caption']}</span></figcaption>
            <pre><code>{MDM}</code></pre>
          </figure>
        </div>
        <div class="wrap">
          {table(c['mdm_title'], c['mdm_cols'], c['mdm_rows'])}
          <p class="notice">{icon("info", 18)}<span>{c['windows']}</span></p>
          <p class="cta-note"><a class="link-arrow" href="{c['more_href']}">{c['more']}</a></p>
        </div>
      </section>
"""
        + footer(lang)
    )


# ---------------------------------------------------------------- bilingual pages


def bilingual_head(title: str, description: str, extra: str = "") -> str:
    return f"""<!doctype html>
<html lang="fr"{extra}>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{e(title)}</title>
    <meta name="description" content="{e(description)}" />
    <meta name="color-scheme" content="light dark" />
    <meta name="theme-color" media="(prefers-color-scheme: light)" content="#f0f0e8" />
    <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0e0e14" />
    <link rel="icon" type="image/svg+xml" href="/site/favicon.svg" />
    <link rel="stylesheet" href="/site/tokens.css" />
    <link rel="stylesheet" href="/site/site.css" />
    <link rel="stylesheet" href="/site/remora.css" />
"""


def picker() -> str:
    return (
        bilingual_head("Remora — Choisissez votre langue · Choose your language", T["fr"]["description"] + " " + T["en"]["description"], " data-lang-picker")
        + f"""    <link rel="canonical" href="{ORIGIN}/" />
    <link rel="alternate" hreflang="fr" href="{ORIGIN}/fr/" />
    <link rel="alternate" hreflang="en" href="{ORIGIN}/en/" />
    <link rel="alternate" hreflang="x-default" href="{ORIGIN}/" />
    <script src="/site/site.js"></script>
  </head>
  <body>
    <main id="main" class="center-page">
      <div class="stack">
        {brand("fr")}
        <h1>{T['fr']['tagline']}</h1>
        <p class="lead" lang="en">{T['en']['tagline']}</p>
        <div class="ctas">
          <a class="btn btn-primary btn-lg" href="/fr/" hreflang="fr" data-lang="fr">Français</a>
          <a class="btn btn-secondary btn-lg" href="/en/" hreflang="en" lang="en" data-lang="en">English</a>
        </div>
      </div>
    </main>
  </body>
</html>
"""
    )


def not_found() -> str:
    return (
        bilingual_head("Page introuvable · Page not found — Remora", "Cette page n’existe pas. This page doesn’t exist.")
        + """    <meta name="robots" content="noindex" />
  </head>
  <body>
    <main id="main" class="center-page">
      <div class="stack">
        """
        + brand("fr")
        + """
        <p class="big-code" aria-hidden="true">4<span class="mark">0</span>4</p>
        <h1><span lang="fr">Page introuvable</span> · <span lang="en">Page not found</span></h1>
        <div class="bilingual">
          <section class="card card-field" lang="fr" aria-labelledby="nf-fr">
            <h2 id="nf-fr">Cette page n’existe pas.</h2>
            <p>Le lien est peut-être ancien, ou l’adresse mal saisie.</p>
            <div class="ctas"><a class="btn btn-primary btn-sm" href="/fr/" data-lang="fr">Accueil</a></div>
          </section>
          <section class="card card-field" lang="en" aria-labelledby="nf-en">
            <h2 id="nf-en">This page doesn’t exist.</h2>
            <p>The link may be old, or the address mistyped.</p>
            <div class="ctas"><a class="btn btn-primary btn-sm" href="/en/" data-lang="en">Home</a></div>
          </section>
        </div>
      </div>
    </main>
  </body>
</html>
"""
    )


def sitemap() -> str:
    urls = "".join(
        f"  <url><loc>{ORIGIN}{paths[lang]}</loc><lastmod>{datetime.date.today().isoformat()}</lastmod>"
        + "".join(f'<xhtml:link rel="alternate" hreflang="{code}" href="{ORIGIN}{paths[code]}"/>' for code in ("en", "fr"))
        + "</url>\n"
        for paths in PAGES.values()
        for lang in ("en", "fr")
    )
    return f'<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n{urls}</urlset>\n'


def llms() -> str:
    t = T["en"]
    pages = "".join(f"- [{t['seo'][key][0]}]({ORIGIN}{PAGES[key]['en']}): {t['seo'][key][1]}\n" for key in PAGES)
    return f"""# Remora

> {t['description']}

Remora is a free, open-source (GPL-3.0-or-later) macOS menu bar app. It groups items by what you have to do
(To reply, To review, To fix, Ready to merge, To do, Reminders, To read, Waiting on others), runs its sorting and
ranking on the device, and only talks to the tools the user or their organization allows. A Windows tray app is in
preview. French pages: {ORIGIN}/fr/.

## Pages

{pages}
## Links

- [Download for Mac]({DMG})
- [Download for Windows (preview)]({EXE})
- [Source code]({REPO})
- [Documentation]({ORIGIN}/docs/): the user guide, IT and compliance, and writing a plugin
- [Connecting your tools]({ORIGIN}/docs/guide/connecting-tools)
- [Data flows]({ORIGIN}/docs/admin/data-flows) and [MDM policy]({ORIGIN}/docs/admin/mdm)
"""


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
    write("robots.txt", f"User-agent: *\nAllow: /\n\nSitemap: {ORIGIN}/sitemap.xml\n")
