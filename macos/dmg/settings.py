# The DMG's window, for dmgbuild: `make dmg` (dmgbuild -s dmg/settings.py Remora build/Remora.dmg).
# The background is rendered from site/og/dmg.html (site/og/render.sh dmg); the positions below match it. Paths are
# from macos/, where make runs it.
import os.path

app = defines.get("app", "build/Remora.app")  # noqa: F821 (dmgbuild passes `defines`)

format = "UDZO"
filesystem = "HFS+"
files = [app, "build/Licences"]
symlinks = {"Applications": "/Applications"}
icon = os.path.join(app, "Contents/Resources/AppIcon.icns")
background = "dmg/background.tiff"

# 440 pt of background, plus the title bar.
window_rect = ((200, 120), (660, 468))
default_view = "icon-view"
show_status_bar = show_tab_view = show_toolbar = show_pathbar = show_sidebar = False
icon_size = 100
text_size = 13
arrange_by = None
icon_locations = {
    "Remora.app": (170, 200),
    "Applications": (490, 200),
    "Licences": (600, 352),
}
