#!/usr/bin/env python3
"""
Render pixel-accurate screenshots of TruthBeacon's Installer Windows:
1. macOS DMG Finder Installation Window (with live app & Applications folder icons)
2. Windows NSIS Installer (Welcome Wizard Page)
3. Windows NSIS Installer (Header & Installation Progress Page)
4. Windows WiX MSI Installer Wizard Dialog
"""

import os
import tempfile
from PIL import Image, ImageDraw, ImageFont, ImageFilter

ARTIFACTS_DIR = "/Users/wardagentic/.gemini/antigravity-ide/brain/f7c1a54b-6d96-4928-9d6c-7a561a70cdf8"
os.makedirs(ARTIFACTS_DIR, exist_ok=True)

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
ICONS_DIR = os.path.join(PROJECT_ROOT, "src-tauri", "icons")

def get_font(size, bold=False):
    paths = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFCompact.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        "/System/Library/Fonts/Supplemental/Arial.ttf"
    ]
    for p in paths:
        if os.path.exists(p):
            try:
                return ImageFont.truetype(p, int(size))
            except (OSError, ValueError):
                continue
    return ImageFont.load_default()

# =============================================================================
# 1. macOS DMG Finder Window Screenshot
# =============================================================================
def render_macos_dmg_window():
    # DMG window interior is 660x400
    # Add native macOS titlebar: height 38px
    # Total window size: 660 x 438
    # Wrap on dark desktop backdrop with drop shadow: canvas 780 x 540
    cw, ch = 780, 540
    canvas = Image.new("RGBA", (cw, ch), (22, 24, 29, 255)) # Dark wallpaper
    
    # Desktop subtle ambient mesh
    draw_bg = ImageDraw.Draw(canvas)
    for y in range(0, ch, 20):
        draw_bg.line([(0, y), (cw, y)], fill=(28, 30, 36, 255), width=1)
    for x in range(0, cw, 20):
        draw_bg.line([(x, 0), (x, ch)], fill=(28, 30, 36, 255), width=1)

    win_w, win_h = 660, 438
    win_x = (cw - win_w) // 2
    win_y = (ch - win_h) // 2

    # Drop shadow
    shadow = Image.new("RGBA", (win_w + 60, win_h + 60), (0, 0, 0, 0))
    s_draw = ImageDraw.Draw(shadow)
    s_draw.rounded_rectangle([30, 30, win_w + 30, win_h + 30], radius=14, fill=(0, 0, 0, 180))
    shadow = shadow.filter(ImageFilter.GaussianBlur(16))
    canvas.paste(shadow, (win_x - 30, win_y - 20), shadow)

    # Window surface
    win = Image.new("RGBA", (win_w, win_h), (0, 0, 0, 0))
    w_draw = ImageDraw.Draw(win)

    # macOS Titlebar (height 38)
    title_h = 38
    w_draw.rounded_rectangle([0, 0, win_w - 1, win_h - 1], radius=12, fill=(30, 31, 34, 255), outline=(255, 255, 255, 30), width=1)
    w_draw.rounded_rectangle([0, 0, win_w - 1, title_h + 10], radius=12, fill=(38, 40, 44, 255))
    w_draw.rectangle([0, title_h, win_w - 1, title_h + 10], fill=(30, 31, 34, 255))
    w_draw.line([(0, title_h), (win_w - 1, title_h)], fill=(255, 255, 255, 20), width=1)

    # Window controls (traffic lights)
    w_draw.ellipse([14, 13, 26, 25], fill=(255, 95, 86, 255), outline=(224, 68, 62, 255), width=1)
    w_draw.ellipse([34, 13, 46, 25], fill=(255, 189, 46, 255), outline=(214, 158, 32, 255), width=1)
    w_draw.ellipse([54, 13, 66, 25], fill=(39, 201, 63, 255), outline=(27, 168, 48, 255), width=1)

    # Window Title
    font_wtitle = get_font(12, bold=True)
    w_title = "TruthBeacon"
    bbox = w_draw.textbbox((0, 0), w_title, font=font_wtitle)
    tw = bbox[2] - bbox[0]
    w_draw.text((win_w // 2 - tw // 2, 11), w_title, fill=(219, 222, 225, 255), font=font_wtitle)

    # Paste the 660x400 DMG Background canvas inside the window body
    dmg_bg_path = os.path.join(ICONS_DIR, "dmg-background.png")
    if os.path.exists(dmg_bg_path):
        dmg_bg = Image.open(dmg_bg_path).convert("RGBA")
        win.paste(dmg_bg, (0, title_h), dmg_bg)

    # Place live icons at appPosition (180, 200) and applicationFolderPosition (480, 200)
    # Note: on DMG, coordinates are relative to the 660x400 content area
    app_center = (180, title_h + 190)
    apps_folder_center = (480, title_h + 190)

    # 1. TruthBeacon.app Icon (96x96 with macOS drop shadow)
    app_icon_path = os.path.join(ICONS_DIR, "128x128@2x.png")
    if os.path.exists(app_icon_path):
        app_icon = Image.open(app_icon_path).convert("RGBA").resize((92, 92), Image.Resampling.LANCZOS)
        # Drop shadow for app icon
        ashadow = Image.new("RGBA", (104, 104), (0, 0, 0, 0))
        adraw = ImageDraw.Draw(ashadow)
        adraw.ellipse([6, 6, 98, 98], fill=(0, 0, 0, 140))
        ashadow = ashadow.filter(ImageFilter.GaussianBlur(6))
        win.paste(ashadow, (app_center[0] - 52, app_center[1] - 46), ashadow)
        win.paste(app_icon, (app_center[0] - 46, app_center[1] - 46), app_icon)

    # 2. Applications Folder Icon (92x92)
    folder_path = os.path.join(tempfile.gettempdir(), "apps_folder.png")
    if os.path.exists(folder_path):
        folder_icon = Image.open(folder_path).convert("RGBA").resize((92, 92), Image.Resampling.LANCZOS)
        fshadow = Image.new("RGBA", (104, 104), (0, 0, 0, 0))
        fdraw = ImageDraw.Draw(fshadow)
        fdraw.ellipse([6, 6, 98, 98], fill=(0, 0, 0, 140))
        fshadow = fshadow.filter(ImageFilter.GaussianBlur(6))
        win.paste(fshadow, (apps_folder_center[0] - 52, apps_folder_center[1] - 46), fshadow)
        win.paste(folder_icon, (apps_folder_center[0] - 46, apps_folder_center[1] - 46), folder_icon)

    # 3. Finder's native text labels (rendered cleanly below icons without duplicates)
    font_finder = get_font(12)
    label_y = app_center[1] + 52

    lbl1 = "TruthBeacon"
    bbox1 = w_draw.textbbox((0, 0), lbl1, font=font_finder)
    lw1 = bbox1[2] - bbox1[0]
    w_draw.text((app_center[0] - lw1 // 2, label_y), lbl1, fill=(242, 243, 245, 255), font=font_finder)

    lbl2 = "Applications"
    bbox2 = w_draw.textbbox((0, 0), lbl2, font=font_finder)
    lw2 = bbox2[2] - bbox2[0]
    w_draw.text((apps_folder_center[0] - lw2 // 2, label_y), lbl2, fill=(242, 243, 245, 255), font=font_finder)

    canvas.paste(win, (win_x, win_y), win)
    out_path = os.path.join(ARTIFACTS_DIR, "macos_dmg_installer_window.png")
    canvas.save(out_path, "PNG")
    print(f"Generated macOS DMG window screenshot: {out_path}")

# =============================================================================
# 2. Windows NSIS Installer (Welcome Wizard Page) Screenshot
# =============================================================================
def render_windows_nsis_welcome_window():
    # Standard NSIS Modern UI window: 503 x 390
    # Canvas with Windows 11 drop shadow: 620 x 500
    cw, ch = 620, 500
    canvas = Image.new("RGBA", (cw, ch), (24, 26, 32, 255))
    
    # Subtle ambient backdrop
    draw_bg = ImageDraw.Draw(canvas)
    for y in range(0, ch, 25):
        draw_bg.line([(0, y), (cw, y)], fill=(30, 32, 40, 255), width=1)

    win_w, win_h = 503, 390
    win_x = (cw - win_w) // 2
    win_y = (ch - win_h) // 2

    # Drop shadow
    shadow = Image.new("RGBA", (win_w + 40, win_h + 40), (0, 0, 0, 0))
    s_draw = ImageDraw.Draw(shadow)
    s_draw.rounded_rectangle([20, 20, win_w + 20, win_h + 20], radius=8, fill=(0, 0, 0, 160))
    shadow = shadow.filter(ImageFilter.GaussianBlur(12))
    canvas.paste(shadow, (win_x - 20, win_y - 12), shadow)

    # Window Surface (Windows 11 dark mica style #202020)
    win = Image.new("RGBA", (win_w, win_h), (0, 0, 0, 0))
    w_draw = ImageDraw.Draw(win)

    title_h = 32
    # Outer frame
    w_draw.rounded_rectangle([0, 0, win_w - 1, win_h - 1], radius=8, fill=(32, 33, 36, 255), outline=(255, 255, 255, 30), width=1)
    
    # Titlebar
    w_draw.rounded_rectangle([0, 0, win_w - 1, title_h + 6], radius=8, fill=(30, 31, 34, 255))
    w_draw.rectangle([0, title_h, win_w - 1, title_h + 6], fill=(30, 31, 34, 255))
    w_draw.line([(0, title_h), (win_w - 1, title_h)], fill=(255, 255, 255, 18), width=1)

    # Title icon & text
    icon_16_path = os.path.join(ICONS_DIR, "32x32.png")
    if os.path.exists(icon_16_path):
        icon_16 = Image.open(icon_16_path).convert("RGBA").resize((16, 16), Image.Resampling.LANCZOS)
        win.paste(icon_16, (10, 8), icon_16)

    font_wtitle = get_font(11)
    w_draw.text((32, 8), "TruthBeacon Setup", fill=(219, 222, 225, 255), font=font_wtitle)

    # Windows 11 Min, Max, Close buttons (Right)
    bx = win_w - 46
    w_draw.text((bx + 16, 6), "✕", fill=(180, 180, 180, 255), font=font_wtitle)
    w_draw.text((bx - 26, 6), "🗖", fill=(180, 180, 180, 255), font=font_wtitle)
    w_draw.text((bx - 68, 6), "🗕", fill=(180, 180, 180, 255), font=font_wtitle)

    # -------------------------------------------------------------------------
    # Content Area: Left Sidebar (164x314) + Right Wizard Text
    # -------------------------------------------------------------------------
    content_y = title_h
    sidebar_path = os.path.join(ICONS_DIR, "nsis-sidebar.bmp")
    if os.path.exists(sidebar_path):
        sidebar = Image.open(sidebar_path).convert("RGBA")
        win.paste(sidebar, (0, content_y), sidebar)

    # Right Content Pane
    rx0 = 164
    rw = win_w - rx0
    w_draw.rectangle([rx0, content_y, win_w - 1, content_y + 314], fill=(38, 40, 44, 255))
    w_draw.line([(rx0, content_y), (rx0, content_y + 314)], fill=(255, 255, 255, 20), width=1)

    font_wh1 = get_font(15, bold=True)
    font_body = get_font(10.5)
    font_sub = get_font(9.5)

    w_draw.text((rx0 + 24, content_y + 24), "Welcome to TruthBeacon Setup", fill=(242, 243, 245, 255), font=font_wh1)
    w_draw.line([(rx0 + 24, content_y + 48), (rx0 + rw - 24, content_y + 48)], fill=(249, 115, 22, 180), width=2)

    lines = [
        "Setup will guide you through the installation of TruthBeacon,",
        "the identity ground-truth and impersonation defense console.",
        "",
        "This software operates strictly on your local machine with zero",
        "cloud telemetry, safeguarding your community leaders and church staff.",
        "",
        "It is recommended that you close all other applications before",
        "starting Setup. This will make it possible to update relevant",
        "system files without rebooting your computer.",
        "",
        "Click Next to continue."
    ]
    for idx, l in enumerate(lines):
        w_draw.text((rx0 + 24, content_y + 60 + idx * 16), l, fill=(219, 222, 225, 255), font=font_body)

    # -------------------------------------------------------------------------
    # Bottom Action Bar (y: content_y + 314 to win_h)
    # -------------------------------------------------------------------------
    bar_y = content_y + 314
    w_draw.rectangle([0, bar_y, win_w - 1, win_h - 1], fill=(30, 31, 34, 255))
    w_draw.line([(0, bar_y), (win_w - 1, bar_y)], fill=(255, 255, 255, 20), width=1)

    # Buttons: "< Back" (disabled), "Next >" (primary), "Cancel"
    font_btn = get_font(10.5)
    # "< Back"
    w_draw.rounded_rectangle([win_w - 240, bar_y + 9, win_w - 170, bar_y + 35], radius=4, fill=(45, 47, 52, 255), outline=(255, 255, 255, 15), width=1)
    w_draw.text((win_w - 222, bar_y + 14), "< Back", fill=(120, 125, 135, 255), font=font_btn)

    # "Next >" (Orange Heart primary accent button)
    w_draw.rounded_rectangle([win_w - 162, bar_y + 9, win_w - 92, bar_y + 35], radius=4, fill=(249, 115, 22, 255), outline=(234, 88, 12, 255), width=1)
    w_draw.text((win_w - 144, bar_y + 14), "Next >", fill=(255, 255, 255, 255), font=font_btn)

    # "Cancel"
    w_draw.rounded_rectangle([win_w - 82, bar_y + 9, win_w - 12, bar_y + 35], radius=4, fill=(45, 47, 52, 255), outline=(255, 255, 255, 25), width=1)
    w_draw.text((win_w - 63, bar_y + 14), "Cancel", fill=(219, 222, 225, 255), font=font_btn)

    canvas.paste(win, (win_x, win_y), win)
    out_path = os.path.join(ARTIFACTS_DIR, "windows_nsis_installer_window.png")
    canvas.save(out_path, "PNG")
    print(f"Generated Windows NSIS window screenshot: {out_path}")

# =============================================================================
# 3. Windows NSIS Installer (Progress & Header Page) Screenshot
# =============================================================================
def render_windows_nsis_progress_window():
    cw, ch = 620, 500
    canvas = Image.new("RGBA", (cw, ch), (24, 26, 32, 255))
    draw_bg = ImageDraw.Draw(canvas)
    for y in range(0, ch, 25):
        draw_bg.line([(0, y), (cw, y)], fill=(30, 32, 40, 255), width=1)

    win_w, win_h = 503, 390
    win_x = (cw - win_w) // 2
    win_y = (ch - win_h) // 2

    # Drop shadow
    shadow = Image.new("RGBA", (win_w + 40, win_h + 40), (0, 0, 0, 0))
    s_draw = ImageDraw.Draw(shadow)
    s_draw.rounded_rectangle([20, 20, win_w + 20, win_h + 20], radius=8, fill=(0, 0, 0, 160))
    shadow = shadow.filter(ImageFilter.GaussianBlur(12))
    canvas.paste(shadow, (win_x - 20, win_y - 12), shadow)

    win = Image.new("RGBA", (win_w, win_h), (0, 0, 0, 0))
    w_draw = ImageDraw.Draw(win)

    title_h = 32
    w_draw.rounded_rectangle([0, 0, win_w - 1, win_h - 1], radius=8, fill=(32, 33, 36, 255), outline=(255, 255, 255, 30), width=1)
    w_draw.rounded_rectangle([0, 0, win_w - 1, title_h + 6], radius=8, fill=(30, 31, 34, 255))
    w_draw.rectangle([0, title_h, win_w - 1, title_h + 6], fill=(30, 31, 34, 255))
    w_draw.line([(0, title_h), (win_w - 1, title_h)], fill=(255, 255, 255, 18), width=1)

    icon_16_path = os.path.join(ICONS_DIR, "32x32.png")
    if os.path.exists(icon_16_path):
        icon_16 = Image.open(icon_16_path).convert("RGBA").resize((16, 16), Image.Resampling.LANCZOS)
        win.paste(icon_16, (10, 8), icon_16)

    font_wtitle = get_font(11)
    w_draw.text((32, 8), "TruthBeacon Setup: Installing Files", fill=(219, 222, 225, 255), font=font_wtitle)

    bx = win_w - 46
    w_draw.text((bx + 16, 6), "✕", fill=(180, 180, 180, 255), font=font_wtitle)
    w_draw.text((bx - 26, 6), "🗖", fill=(180, 180, 180, 255), font=font_wtitle)
    w_draw.text((bx - 68, 6), "🗕", fill=(180, 180, 180, 255), font=font_wtitle)

    # -------------------------------------------------------------------------
    # Header Banner: 57px high, with nsis-header.bmp (150x57) aligned to the right
    # -------------------------------------------------------------------------
    header_y = title_h
    w_draw.rectangle([0, header_y, win_w - 1, header_y + 57], fill=(30, 31, 34, 255))
    w_draw.line([(0, header_y + 57), (win_w - 1, header_y + 57)], fill=(249, 115, 22, 200), width=2)

    font_hh1 = get_font(12, bold=True)
    font_sub = get_font(9.5)
    w_draw.text((20, header_y + 12), "Installing TruthBeacon Core & Enclave Bridge", fill=(242, 243, 245, 255), font=font_hh1)
    w_draw.text((20, header_y + 32), "Please wait while TruthBeacon is being installed on your computer.", fill=(148, 155, 164, 255), font=font_sub)

    header_bmp_path = os.path.join(ICONS_DIR, "nsis-header.bmp")
    if os.path.exists(header_bmp_path):
        header_bmp = Image.open(header_bmp_path).convert("RGBA")
        win.paste(header_bmp, (win_w - 150, header_y), header_bmp)

    # -------------------------------------------------------------------------
    # Middle Body: Progress Bar & File Activity
    # -------------------------------------------------------------------------
    body_y = header_y + 58
    w_draw.rectangle([0, body_y, win_w - 1, body_y + 256], fill=(38, 40, 44, 255))

    font_body = get_font(10)
    w_draw.text((32, body_y + 28), "Extracting: truth-beacon.exe... 100%", fill=(219, 222, 225, 255), font=font_body)
    w_draw.text((32, body_y + 46), "Configuring SQLite WAL embedded storage...", fill=(148, 155, 164, 255), font=font_body)

    # Modern Progress Bar
    pbar_x0, pbar_y0 = 32, body_y + 72
    pbar_w, pbar_h = win_w - 64, 20
    w_draw.rounded_rectangle([pbar_x0, pbar_y0, pbar_x0 + pbar_w, pbar_y0 + pbar_h], radius=4, fill=(26, 27, 30, 255), outline=(255, 255, 255, 20), width=1)
    
    # Progress fill (74% complete, Orange Heart primary accent)
    fill_w = int(pbar_w * 0.74)
    w_draw.rounded_rectangle([pbar_x0 + 1, pbar_y0 + 1, pbar_x0 + fill_w, pbar_y0 + pbar_h - 1], radius=3, fill=(249, 115, 22, 255))

    # Details Box (Card #2b2d31)
    box_y = body_y + 110
    w_draw.rounded_rectangle([32, box_y, win_w - 32, box_y + 120], radius=6, fill=(43, 45, 49, 255), outline=(255, 255, 255, 20), width=1)
    
    status_lines = [
        "✓ Output folder: C:\\Users\\Administrator\\AppData\\Local\\TruthBeacon",
        "✓ Extract: truth-beacon.exe (14.2 MB)",
        "✓ Extract: webview2_loader.dll",
        "✓ Initializing Windows Credential Manager enclave bridge...",
        "✓ Registering system tray status agent..."
    ]
    for idx, sl in enumerate(status_lines):
        w_draw.text((44, box_y + 12 + idx * 21), sl, fill=(180, 185, 195, 255), font=font_sub)

    # -------------------------------------------------------------------------
    # Bottom Action Bar
    # -------------------------------------------------------------------------
    bar_y = body_y + 256
    w_draw.rectangle([0, bar_y, win_w - 1, win_h - 1], fill=(30, 31, 34, 255))
    w_draw.line([(0, bar_y), (win_w - 1, bar_y)], fill=(255, 255, 255, 20), width=1)

    font_btn = get_font(10.5)
    # Cancel (active during install)
    w_draw.rounded_rectangle([win_w - 82, bar_y + 9, win_w - 12, bar_y + 35], radius=4, fill=(45, 47, 52, 255), outline=(255, 255, 255, 25), width=1)
    w_draw.text((win_w - 63, bar_y + 14), "Cancel", fill=(219, 222, 225, 255), font=font_btn)

    canvas.paste(win, (win_x, win_y), win)
    out_path = os.path.join(ARTIFACTS_DIR, "windows_nsis_progress_window.png")
    canvas.save(out_path, "PNG")
    print(f"Generated Windows NSIS Progress screenshot: {out_path}")

# =============================================================================
# 4. Windows WiX MSI Installer Wizard Dialog Screenshot
# =============================================================================
def render_windows_wix_msi_window():
    # WiX dialog standard size: 493 x 312
    # Canvas with Windows 11 drop shadow: 610 x 420
    cw, ch = 610, 420
    canvas = Image.new("RGBA", (cw, ch), (24, 26, 32, 255))
    draw_bg = ImageDraw.Draw(canvas)
    for y in range(0, ch, 25):
        draw_bg.line([(0, y), (cw, y)], fill=(30, 32, 40, 255), width=1)

    win_w, win_h = 493, 312 + 32 # dialog + titlebar
    win_x = (cw - win_w) // 2
    win_y = (ch - win_h) // 2

    # Drop shadow
    shadow = Image.new("RGBA", (win_w + 40, win_h + 40), (0, 0, 0, 0))
    s_draw = ImageDraw.Draw(shadow)
    s_draw.rounded_rectangle([20, 20, win_w + 20, win_h + 20], radius=8, fill=(0, 0, 0, 160))
    shadow = shadow.filter(ImageFilter.GaussianBlur(12))
    canvas.paste(shadow, (win_x - 20, win_y - 12), shadow)

    win = Image.new("RGBA", (win_w, win_h), (0, 0, 0, 0))
    w_draw = ImageDraw.Draw(win)

    title_h = 32
    w_draw.rounded_rectangle([0, 0, win_w - 1, win_h - 1], radius=8, fill=(30, 31, 34, 255), outline=(255, 255, 255, 30), width=1)
    w_draw.rounded_rectangle([0, 0, win_w - 1, title_h + 6], radius=8, fill=(30, 31, 34, 255))
    w_draw.rectangle([0, title_h, win_w - 1, title_h + 6], fill=(30, 31, 34, 255))
    w_draw.line([(0, title_h), (win_w - 1, title_h)], fill=(255, 255, 255, 18), width=1)

    icon_16_path = os.path.join(ICONS_DIR, "32x32.png")
    if os.path.exists(icon_16_path):
        icon_16 = Image.open(icon_16_path).convert("RGBA").resize((16, 16), Image.Resampling.LANCZOS)
        win.paste(icon_16, (10, 8), icon_16)

    font_wtitle = get_font(11)
    w_draw.text((32, 8), "TruthBeacon Enterprise Installer", fill=(219, 222, 225, 255), font=font_wtitle)

    bx = win_w - 46
    w_draw.text((bx + 16, 6), "✕", fill=(180, 180, 180, 255), font=font_wtitle)
    w_draw.text((bx - 26, 6), "🗖", fill=(180, 180, 180, 255), font=font_wtitle)
    w_draw.text((bx - 68, 6), "🗕", fill=(180, 180, 180, 255), font=font_wtitle)

    # Paste wix-dialog.bmp (493x312) directly inside
    dialog_bmp_path = os.path.join(ICONS_DIR, "wix-dialog.bmp")
    if os.path.exists(dialog_bmp_path):
        d_bmp = Image.open(dialog_bmp_path).convert("RGBA")
        win.paste(d_bmp, (0, title_h), d_bmp)

    canvas.paste(win, (win_x, win_y), win)
    out_path = os.path.join(ARTIFACTS_DIR, "windows_wix_msi_dialog_window.png")
    canvas.save(out_path, "PNG")
    print(f"Generated WiX MSI window screenshot: {out_path}")

if __name__ == "__main__":
    render_macos_dmg_window()
    render_windows_nsis_welcome_window()
    render_windows_nsis_progress_window()
    render_windows_wix_msi_window()
    print("All installer window screenshots rendered successfully.")
