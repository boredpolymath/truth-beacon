#!/usr/bin/env python3
"""
Generate Orange Heart branded installer assets for macOS DMG, Windows NSIS, and WiX.
Matches the exact Discord-familiar charcoal & Orange Heart stewardship aesthetic of the main application.
"""

from PIL import Image, ImageDraw, ImageFont
import os
import math

ICONS_DIR = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
ASSETS_DIR = os.path.join(os.path.dirname(__file__), "..", "ui", "assets")
os.makedirs(ICONS_DIR, exist_ok=True)

# -----------------------------------------------------------------------------
# Color Palette Constants (from ui/css/styles.css)
# -----------------------------------------------------------------------------
CLR_BG_DARKEST = (17, 18, 20, 255)       # #111214 - Deepest backdrop / window frame
CLR_BG_SIDEBAR = (30, 31, 34, 255)       # #1e1f22 - Header, titlebar
CLR_BG_BASE = (43, 45, 49, 255)          # #2b2d31 - Container panels, card surfaces
CLR_BG_CHAT = (49, 51, 56, 255)          # #313338 - Main viewport canvas
CLR_BG_SECONDARY = (35, 36, 40, 255)     # #232428 - Sub-surfaces, input fields
CLR_BORDER_SUBTLE = (255, 255, 255, 22)  # rgba(255, 255, 255, 0.08)
CLR_BORDER_MEDIUM = (255, 255, 255, 35)  # rgba(255, 255, 255, 0.14)

CLR_TEXT_HEADER = (242, 243, 245, 255)   # #f2f3f5 - Discord high-contrast white
CLR_TEXT_PRIMARY = (219, 222, 225, 255)  # #dbdee1 - Readable body text
CLR_TEXT_SECONDARY = (148, 155, 164, 255)# #949ba4 - Muted metadata
CLR_TEXT_MUTED = (128, 132, 142, 255)    # #80848e - Helper hints

CLR_OH_ORANGE = (249, 115, 22, 255)      # #f97316 - Primary Orange Heart accent
CLR_OH_ORANGE_DARK = (234, 88, 12, 255)  # #ea580c - Deep orange gradient
CLR_OH_ORANGE_LIGHT = (251, 146, 60, 255)# #fb923c - Light amber
CLR_BLURPLE = (88, 101, 242, 255)        # #5865f2 - Discord signature Blurple
CLR_GREEN = (35, 165, 90, 255)           # #23a55a - Discord success green

def get_font(size, bold=False):
    """Load SFNS font if available on macOS, falling back gracefully."""
    font_paths = [
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/SFCompact.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        "/System/Library/Fonts/Supplemental/Arial.ttf"
    ]
    for path in font_paths:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, size)
            except (OSError, ValueError):
                continue
    return ImageFont.load_default()

def get_brand_icon(size):
    """Load high-res brand emblem, resized smoothly."""
    candidates = [
        os.path.join(ASSETS_DIR, "truthbeacon_emblem.png"),
        os.path.join(ICONS_DIR, "128x128@2x.png"),
        os.path.join(ICONS_DIR, "128x128.png")
    ]
    for c in candidates:
        if os.path.exists(c):
            try:
                img = Image.open(c).convert("RGBA")
                return img.resize((size, size), Image.Resampling.LANCZOS)
            except (OSError, ValueError):
                continue
    # Fallback circle if icon missing
    fb = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(fb)
    d.ellipse([2, 2, size - 3, size - 3], fill=CLR_OH_ORANGE)
    return fb

def draw_arrow(draw, start_x, start_y, end_x, end_y, color, width=3):
    """Draw a smooth modern directional indicator with chevron arrow head."""
    draw.line([(start_x, start_y), (end_x, end_y)], fill=color, width=width)
    arrow_size = 11
    angle = math.atan2(end_y - start_y, end_x - start_x)
    p1 = (end_x - arrow_size * math.cos(angle - math.pi / 5),
          end_y - arrow_size * math.sin(angle - math.pi / 5))
    p2 = (end_x - arrow_size * math.cos(angle + math.pi / 5),
          end_y - arrow_size * math.sin(angle + math.pi / 5))
    draw.polygon([(end_x, end_y), p1, p2], fill=color)

# =============================================================================
# 1. macOS DMG Background Canvas (660 x 400)
# =============================================================================
def generate_dmg_background():
    w, h = 660, 400
    base = Image.new("RGBA", (w, h), CLR_BG_DARKEST)
    
    # Layer 1: Window frame & background canvas
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)

    # Main window interior (rounded container)
    draw.rounded_rectangle([0, 0, w - 1, h - 1], radius=14, fill=CLR_BG_CHAT, outline=CLR_BORDER_MEDIUM, width=1)

    # Header Bar (#1e1f22)
    header_h = 48
    draw.rounded_rectangle([0, 0, w - 1, header_h], radius=14, fill=CLR_BG_SIDEBAR)
    # Square off bottom of header
    draw.rectangle([0, header_h - 10, w - 1, header_h], fill=CLR_BG_SIDEBAR)
    draw.line([(0, header_h), (w - 1, header_h)], fill=CLR_BORDER_SUBTLE, width=1)

    # Header Brand Logo & Text (Left-aligned, clean & minimal)
    emblem_32 = get_brand_icon(28)
    canvas.paste(emblem_32, (18, 10), emblem_32)

    font_title = get_font(15, bold=True)
    font_badge = get_font(10, bold=True)
    font_body = get_font(12)
    font_small = get_font(10)
    font_pill = get_font(11, bold=True)

    draw.text((54, 15), "TruthBeacon", fill=CLR_TEXT_HEADER, font=font_title)

    # -------------------------------------------------------------------------
    # Drop Target Pods (Inspection Card Styling from TruthBeacon UI)
    # Slot 1 Center: (180, 200) -> Pod Bounds: x0=104, y0=95, x1=256, y1=315 (W=152, H=220)
    # Slot 2 Center: (480, 200) -> Pod Bounds: x0=404, y0=95, x1=556, y1=315 (W=152, H=220)
    # Note: Bottom text is deliberately omitted so Finder's native icon labels
    # ('TruthBeacon' and 'Applications') render cleanly without duplicate text.
    # -------------------------------------------------------------------------
    for (cx, cy, is_target) in [
        (180, 200, False),
        (480, 200, True)
    ]:
        card_w, card_h = 152, 220
        x0 = cx - card_w // 2
        y0 = cy - 105
        x1 = x0 + card_w
        y1 = y0 + card_h

        # Card container with drop shadow simulation
        shadow_box = [x0 + 2, y0 + 4, x1 - 2, y1 + 4]
        draw.rounded_rectangle(shadow_box, radius=12, fill=(0, 0, 0, 60))
        
        card_border = (35, 165, 90, 160) if is_target else (88, 101, 242, 140)
        draw.rounded_rectangle([x0, y0, x1, y1], radius=12, fill=CLR_BG_BASE, outline=card_border, width=1)

        # Card Top Accent Strip
        top_strip_color = (35, 165, 90, 40) if is_target else (88, 101, 242, 40)
        draw.rounded_rectangle([x0, y0, x1, y0 + 26], radius=12, fill=top_strip_color)
        draw.rectangle([x0, y0 + 16, x1, y0 + 26], fill=top_strip_color)
        draw.line([(x0, y0 + 26), (x1, y0 + 26)], fill=CLR_BORDER_SUBTLE, width=1)

        badge_text = "DESTINATION" if is_target else "APPLICATION"
        badge_color = CLR_GREEN if is_target else CLR_BLURPLE
        bbox = draw.textbbox((0, 0), badge_text, font=font_badge)
        bw = bbox[2] - bbox[0]
        draw.text((cx - bw // 2, y0 + 7), badge_text, fill=badge_color, font=font_badge)

        # Subtle pedestal drop ring around the 128x128 icon area
        pedestal_radius = 54
        draw.ellipse([cx - pedestal_radius, cy - pedestal_radius, cx + pedestal_radius, cy + pedestal_radius],
                     outline=(255, 255, 255, 25), width=1)
        draw.ellipse([cx - pedestal_radius + 4, cy - pedestal_radius + 4, cx + pedestal_radius - 4, cy + pedestal_radius - 4],
                     outline=(255, 255, 255, 12), width=1)

    # -------------------------------------------------------------------------
    # Center Action Bridge (Between the pods, center x=330, y=200)
    # Modeled directly after the Threat Metric Pillar in TruthBeacon
    # -------------------------------------------------------------------------
    center_x = 330
    center_y = 195

    # Center connecting box / pillar
    pillar_w, pillar_h = 120, 100
    px0 = center_x - pillar_w // 2
    py0 = center_y - pillar_h // 2
    px1 = px0 + pillar_w
    py1 = py0 + pillar_h
    draw.rounded_rectangle([px0, py0, px1, py1], radius=10, fill=CLR_BG_SIDEBAR, outline=CLR_BORDER_SUBTLE, width=1)

    # Action Pill: "DRAG TO INSTALL"
    btn_w, btn_h = 104, 26
    bx0 = center_x - btn_w // 2
    by0 = center_y - 36
    bx1 = bx0 + btn_w
    by1 = by0 + btn_h
    draw.rounded_rectangle([bx0, by0, bx1, by1], radius=6, fill=CLR_OH_ORANGE)
    btn_text = "DRAG TO INSTALL"
    bbox_btn = draw.textbbox((0, 0), btn_text, font=font_badge)
    btw = bbox_btn[2] - bbox_btn[0]
    draw.text((center_x - btw // 2, by0 + 7), btn_text, fill=(255, 255, 255, 255), font=font_badge)

    # Directional Glowing Arrow
    arrow_y = center_y + 4
    draw_arrow(draw, center_x - 38, arrow_y, center_x + 38, arrow_y, CLR_OH_ORANGE, width=3)

    # Secondary guide caption
    guide_text = "Drop into Applications"
    bbox_g = draw.textbbox((0, 0), guide_text, font=font_small)
    gw = bbox_g[2] - bbox_g[0]
    draw.text((center_x - gw // 2, center_y + 24), guide_text, fill=CLR_TEXT_SECONDARY, font=font_small)

    # -------------------------------------------------------------------------
    # Bottom Footer Strip (y: 355 to 400)
    # -------------------------------------------------------------------------
    footer_y = 355
    draw.rounded_rectangle([0, footer_y, w - 1, h - 1], radius=14, fill=CLR_BG_SIDEBAR)
    draw.rectangle([0, footer_y, w - 1, footer_y + 14], fill=CLR_BG_SIDEBAR)
    draw.line([(0, footer_y), (w - 1, footer_y)], fill=CLR_BORDER_SUBTLE, width=1)

    # Footer Left
    draw.text((20, footer_y + 13), "Orange Heart Industries", fill=CLR_TEXT_PRIMARY, font=font_small)
    draw.text((150, footer_y + 13), "•", fill=CLR_TEXT_MUTED, font=font_small)
    draw.text((160, footer_y + 13), "Community Impersonation Prevention Console", fill=CLR_TEXT_SECONDARY, font=font_small)

    # Footer Right Badges
    badge_right = "Zero Cloud Telemetry • Universal Binary"
    bbox_br = draw.textbbox((0, 0), badge_right, font=font_small)
    brw = bbox_br[2] - bbox_br[0]
    draw.text((w - brw - 20, footer_y + 13), badge_right, fill=CLR_TEXT_MUTED, font=font_small)

    # Composite layers
    final_img = Image.alpha_composite(base, canvas)
    out_file = os.path.join(ICONS_DIR, "dmg-background.png")
    final_img.save(out_file, "PNG")
    print(f"Generated DMG background: {out_file} ({w}x{h})")

# =============================================================================
# 2. Windows NSIS Installer Images
# =============================================================================
def generate_nsis_assets():
    # -------------------------------------------------------------------------
    # NSIS Header: 150x57 BMP (Top-right header banner during install wizard)
    # -------------------------------------------------------------------------
    hw, hh = 150, 57
    h_img = Image.new("RGBA", (hw, hh), (30, 31, 34, 255)) # #1e1f22 (Sidebar surface)
    h_draw = ImageDraw.Draw(h_img)

    # Bottom Orange Accent Line
    h_draw.rectangle([0, hh - 2, hw, hh], fill=CLR_OH_ORANGE)

    # Small TruthBeacon Emblem (32x32) at top right
    emblem_32 = get_brand_icon(32)
    h_img.paste(emblem_32, (hw - 42, 12), emblem_32)

    font_nh = get_font(12, bold=True)
    font_nhs = get_font(9)

    h_draw.text((10, 12), "TruthBeacon", fill=CLR_TEXT_HEADER, font=font_nh)
    h_draw.text((10, 30), "Identity Protection", fill=CLR_TEXT_SECONDARY, font=font_nhs)

    h_out = os.path.join(ICONS_DIR, "nsis-header.bmp")
    h_img.convert("RGB").save(h_out, "BMP")
    print(f"Generated NSIS Header: {h_out} ({hw}x{hh})")

    # -------------------------------------------------------------------------
    # NSIS Sidebar: 164x314 BMP (Left column on Welcome & Finished wizard pages)
    # -------------------------------------------------------------------------
    sw, sh = 164, 314
    s_img = Image.new("RGBA", (sw, sh), (17, 18, 20, 255)) # #111214
    s_draw = ImageDraw.Draw(s_img)

    # Vertical gradient from #111214 to #1e1f22
    for y in range(sh):
        ratio = y / sh
        r = int(17 + ratio * 13)
        g = int(18 + ratio * 13)
        b = int(20 + ratio * 14)
        s_draw.line([(0, y), (sw, y)], fill=(r, g, b, 255))

    # Left vertical Orange Heart accent bar
    s_draw.rectangle([0, 0, 4, sh], fill=CLR_OH_ORANGE)

    # Brand emblem
    icon_56 = get_brand_icon(56)
    s_img.paste(icon_56, (sw // 2 - 28, 26), icon_56)

    font_sb = get_font(15, bold=True)
    font_sub = get_font(9.5, bold=True)
    font_body = get_font(8.5)
    font_muted = get_font(8)

    # Title & Subtitle
    s_draw.text((16, 96), "TruthBeacon", fill=CLR_TEXT_HEADER, font=font_sb)
    s_draw.text((16, 118), "Impersonation Defense", fill=CLR_OH_ORANGE, font=font_sub)

    # Feature List Card (#2b2d31)
    card_y = 146
    s_draw.rounded_rectangle([12, card_y, sw - 12, card_y + 112], radius=8, fill=(43, 45, 49, 255), outline=CLR_BORDER_SUBTLE, width=1)
    
    features = [
        ("• Gateway Guard", (219, 222, 225, 255)),
        ("• Homoglyph Spoofing", (219, 222, 225, 255)),
        ("• Perceptual Avatar DCT", (219, 222, 225, 255)),
        ("• Zero Cloud Telemetry", (35, 165, 90, 255))
    ]
    for idx, (feat, clr) in enumerate(features):
        s_draw.text((20, card_y + 12 + idx * 24), feat, fill=clr, font=font_body)

    # Bottom Branding
    s_draw.text((16, sh - 30), "Orange Heart Industries", fill=CLR_TEXT_SECONDARY, font=font_muted)
    s_draw.text((16, sh - 16), "Local Data Sovereignty", fill=CLR_TEXT_MUTED, font=font_muted)

    s_out = os.path.join(ICONS_DIR, "nsis-sidebar.bmp")
    s_img.convert("RGB").save(s_out, "BMP")
    print(f"Generated NSIS Sidebar: {s_out} ({sw}x{sh})")

# =============================================================================
# 3. Windows WiX MSI Installer Images
# =============================================================================
def generate_wix_assets():
    # -------------------------------------------------------------------------
    # WiX Banner: 493x58 BMP (Top wizard banner)
    # -------------------------------------------------------------------------
    bw, bh = 493, 58
    b_img = Image.new("RGBA", (bw, bh), (30, 31, 34, 255)) # #1e1f22
    b_draw = ImageDraw.Draw(b_img)

    # Bottom accent line
    b_draw.rectangle([0, bh - 2, bw, bh], fill=CLR_OH_ORANGE)

    emblem_36 = get_brand_icon(36)
    b_img.paste(emblem_36, (bw - 48, 11), emblem_36)

    font_wb = get_font(13, bold=True)
    font_ws = get_font(9.5)

    b_draw.text((20, 12), "TruthBeacon Setup Wizard", fill=CLR_TEXT_HEADER, font=font_wb)
    b_draw.text((20, 32), "Orange Heart Community Impersonation Defense Installation", fill=CLR_TEXT_SECONDARY, font=font_ws)

    b_out = os.path.join(ICONS_DIR, "wix-banner.bmp")
    b_img.convert("RGB").save(b_out, "BMP")
    print(f"Generated WiX Banner: {b_out} ({bw}x{bh})")

    # -------------------------------------------------------------------------
    # WiX Dialog: 493x312 BMP (Welcome & Finished Dialog Background)
    # -------------------------------------------------------------------------
    dw, dh = 493, 312
    d_img = Image.new("RGBA", (dw, dh), (17, 18, 20, 255)) # #111214
    d_draw = ImageDraw.Draw(d_img)

    # Gradient background
    for y in range(dh):
        ratio = y / dh
        r = int(17 + ratio * 15)
        g = int(18 + ratio * 15)
        b = int(20 + ratio * 18)
        d_draw.line([(0, y), (dw, y)], fill=(r, g, b, 255))

    # Left vertical accent line
    d_draw.rectangle([0, 0, 5, dh], fill=CLR_OH_ORANGE)

    # Brand emblem
    emblem_64 = get_brand_icon(64)
    d_img.paste(emblem_64, (32, 28), emblem_64)

    font_dt = get_font(17, bold=True)
    font_dsub = get_font(10.5, bold=True)
    font_db = get_font(9.5)
    font_badge = get_font(9, bold=True)
    font_dfoot = get_font(8.5)

    d_draw.text((114, 32), "TruthBeacon Console", fill=CLR_TEXT_HEADER, font=font_dt)
    d_draw.text((114, 60), "Community Ground-Truth & Kinship Stewardship", fill=CLR_OH_ORANGE, font=font_dsub)

    # Info card
    cy0 = 106
    d_draw.rounded_rectangle([32, cy0, dw - 32, cy0 + 138], radius=8, fill=(43, 45, 49, 255), outline=CLR_BORDER_SUBTLE, width=1)
    
    d_draw.text((48, cy0 + 16), "Welcome to the TruthBeacon Setup Wizard.", fill=CLR_TEXT_HEADER, font=font_db)
    d_draw.text((48, cy0 + 38), "This wizard installs TruthBeacon with local SQLite WAL storage and native", fill=CLR_TEXT_PRIMARY, font=font_db)
    d_draw.text((48, cy0 + 56), "OS Enclave (Windows DPAPI / Credential Manager) protection.", fill=CLR_TEXT_PRIMARY, font=font_db)
    
    # Feature Badges (Discord-style translucent badges)
    d_draw.rounded_rectangle([48, cy0 + 88, 220, cy0 + 116], radius=6, fill=(41, 63, 54, 255), outline=(35, 165, 90, 255), width=1)
    d_draw.text((62, cy0 + 95), "✓  Zero Cloud Telemetry", fill=(120, 240, 160, 255), font=font_badge)

    d_draw.rounded_rectangle([232, cy0 + 88, 412, cy0 + 116], radius=6, fill=(49, 53, 77, 255), outline=(88, 101, 242, 255), width=1)
    d_draw.text((246, cy0 + 95), "✓  Local Data Sovereignty", fill=(170, 185, 255, 255), font=font_badge)

    # Footer
    d_draw.text((32, dh - 26), "Orange Heart Industries • Built with memory-safe Rust & Tauri v2", fill=CLR_TEXT_MUTED, font=font_dfoot)

    d_out = os.path.join(ICONS_DIR, "wix-dialog.bmp")
    d_img.convert("RGB").save(d_out, "BMP")
    print(f"Generated WiX Dialog: {d_out} ({dw}x{dh})")

if __name__ == "__main__":
    generate_dmg_background()
    generate_nsis_assets()
    generate_wix_assets()
    print("All installer branding graphics generated successfully.")
