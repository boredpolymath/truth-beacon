#!/usr/bin/env python3
"""
Generate Orange Heart branded installer assets for macOS DMG, Windows NSIS, and WiX.
"""

from PIL import Image, ImageDraw, ImageFont
import os
import math

ICONS_DIR = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
os.makedirs(ICONS_DIR, exist_ok=True)

def draw_rounded_rect(draw, bbox, radius, fill, outline=None, width=1):
    x0, y0, x1, y1 = bbox
    draw.rounded_rectangle([x0, y0, x1, y1], radius=radius, fill=fill, outline=outline, width=width)

def draw_arrow(draw, start_x, start_y, end_x, end_y, color, width=3):
    draw.line([(start_x, start_y), (end_x, end_y)], fill=color, width=width)
    # Arrow head
    arrow_size = 14
    angle = math.atan2(end_y - start_y, end_x - start_x)
    p1 = (end_x - arrow_size * math.cos(angle - math.pi / 6),
          end_y - arrow_size * math.sin(angle - math.pi / 6))
    p2 = (end_x - arrow_size * math.cos(angle + math.pi / 6),
          end_y - arrow_size * math.sin(angle + math.pi / 6))
    draw.polygon([(end_x, end_y), p1, p2], fill=color)

def generate_dmg_background():
    w, h = 660, 400
    img = Image.new("RGBA", (w, h), (12, 17, 29, 255)) # Slate dark #0c111d
    draw = ImageDraw.Draw(img)

    # Subtle background gradient/vignette
    for y in range(h):
        ratio = y / h
        r = int(12 + ratio * 8)
        g = int(17 + ratio * 12)
        b = int(29 + ratio * 20)
        draw.line([(0, y), (w, y)], fill=(r, g, b, 255))

    # Decorative tech grid dots
    for gx in range(40, w, 40):
        for gy in range(40, h, 40):
            draw.ellipse([gx-1, gy-1, gx+1, gy+1], fill=(40, 55, 80, 80))

    # Outer border
    draw.rectangle([1, 1, w - 2, h - 2], outline=(30, 41, 59, 255), width=2)

    # Load brand icon if available
    icon_path = os.path.join(ICONS_DIR, "128x128.png")
    if os.path.exists(icon_path):
        icon = Image.open(icon_path).convert("RGBA").resize((48, 48), Image.Resampling.LANCZOS)
        img.paste(icon, (32, 28), icon)

    # Header text
    try:
        font_large = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 22)
        font_sub = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 12)
        font_small = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 11)
        font_bold = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 13)
    except Exception:
        font_large = ImageFont.load_default()
        font_sub = ImageFont.load_default()
        font_small = ImageFont.load_default()
        font_bold = ImageFont.load_default()

    draw.text((90, 28), "TruthBeacon", fill=(255, 255, 255, 255), font=font_large)
    draw.text((90, 56), "Community Impersonation Protection Console", fill=(249, 115, 22, 255), font=font_sub)

    # Target zones for App (180, 200) and Applications (480, 200)
    # In macOS DMG: coordinates are centers of 128x128 icon slots
    app_center = (180, 200)
    app_target_center = (480, 200)

    # Subtle target indicator rings
    for center, label in [(app_center, "TruthBeacon.app"), (app_target_center, "Applications")]:
        cx, cy = center
        draw.rounded_rectangle([cx - 56, cy - 56, cx + 56, cy + 56], radius=16,
                               fill=(20, 28, 45, 180), outline=(249, 115, 22, 100), width=1)
        # Sub-caption
        bbox = draw.textbbox((0, 0), label, font=font_small)
        text_w = bbox[2] - bbox[0]
        draw.text((cx - text_w // 2, cy + 64), label, fill=(148, 163, 184, 255), font=font_small)

    # Draw animated-style glowing drag arrow between the two slots
    arrow_start = (250, 200)
    arrow_end = (410, 200)
    draw_arrow(draw, arrow_start[0], arrow_start[1], arrow_end[0], arrow_end[1], (249, 115, 22, 230), width=3)

    # Instruction banner above arrow
    instr_text = "Drag to Applications to Install"
    bbox = draw.textbbox((0, 0), instr_text, font=font_bold)
    text_w = bbox[2] - bbox[0]
    draw.text((330 - text_w // 2, 165), instr_text, fill=(249, 115, 22, 255), font=font_bold)

    # Footer note
    footer = "Orange Heart Industries • High-Trust Desktop Ground-Truth Protection"
    bbox = draw.textbbox((0, 0), footer, font=font_small)
    text_w = bbox[2] - bbox[0]
    draw.text((w // 2 - text_w // 2, h - 30), footer, fill=(100, 116, 139, 255), font=font_small)

    out_file = os.path.join(ICONS_DIR, "dmg-background.png")
    img.save(out_file, "PNG")
    print(f"Generated DMG background: {out_file} ({w}x{h})")

def generate_nsis_assets():
    # 1. NSIS Header: 150x57 BMP
    hw, hh = 150, 57
    h_img = Image.new("RGB", (hw, hh), (15, 23, 42)) # Slate 900
    h_draw = ImageDraw.Draw(h_img)
    try:
        font_h = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 13)
        font_hs = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 9)
    except Exception:
        font_h = ImageFont.load_default()
        font_hs = ImageFont.load_default()

    h_draw.text((10, 12), "TruthBeacon", fill=(255, 255, 255), font=font_h)
    h_draw.text((10, 30), "Orange Heart", fill=(249, 115, 22), font=font_hs)

    # Small accent
    h_draw.rectangle([hw - 10, 0, hw, hh], fill=(249, 115, 22))

    h_out = os.path.join(ICONS_DIR, "nsis-header.bmp")
    h_img.save(h_out, "BMP")
    print(f"Generated NSIS Header: {h_out} ({hw}x{hh})")

    # 2. NSIS Sidebar: 164x314 BMP
    sw, sh = 164, 314
    s_img = Image.new("RGB", (sw, sh), (12, 17, 29))
    s_draw = ImageDraw.Draw(s_img)
    for y in range(sh):
        ratio = y / sh
        r = int(12 + ratio * 15)
        g = int(17 + ratio * 20)
        b = int(29 + ratio * 35)
        s_draw.line([(0, y), (sw, y)], fill=(r, g, b))

    # Orange accent bar
    s_draw.rectangle([0, 0, 5, sh], fill=(249, 115, 22))

    # Add brand icon
    icon_path = os.path.join(ICONS_DIR, "128x128.png")
    if os.path.exists(icon_path):
        icon = Image.open(icon_path).convert("RGBA").resize((64, 64), Image.Resampling.LANCZOS)
        s_img.paste(icon, (sw // 2 - 32, 40), icon)

    try:
        font_sb = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 15)
        font_ss = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 10)
    except Exception:
        font_sb = ImageFont.load_default()
        font_ss = ImageFont.load_default()

    s_draw.text((20, 120), "TruthBeacon", fill=(255, 255, 255), font=font_sb)
    s_draw.text((20, 145), "Impersonation\nDefense Console", fill=(249, 115, 22), font=font_ss)
    s_draw.text((20, 260), "Orange Heart\nIndustries", fill=(148, 163, 184), font=font_ss)

    s_out = os.path.join(ICONS_DIR, "nsis-sidebar.bmp")
    s_img.save(s_out, "BMP")
    print(f"Generated NSIS Sidebar: {s_out} ({sw}x{sh})")

def generate_wix_assets():
    # WiX Banner: 493x58 BMP
    bw, bh = 493, 58
    b_img = Image.new("RGB", (bw, bh), (15, 23, 42))
    b_draw = ImageDraw.Draw(b_img)
    try:
        font_b = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 14)
        font_bs = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 10)
    except Exception:
        font_b = ImageFont.load_default()
        font_bs = ImageFont.load_default()

    b_draw.text((20, 12), "TruthBeacon Setup", fill=(255, 255, 255), font=font_b)
    b_draw.text((20, 32), "Orange Heart Community Protection Installation", fill=(249, 115, 22), font=font_bs)
    b_draw.rectangle([bw - 12, 0, bw, bh], fill=(249, 115, 22))

    b_out = os.path.join(ICONS_DIR, "wix-banner.bmp")
    b_img.save(b_out, "BMP")
    print(f"Generated WiX Banner: {b_out} ({bw}x{bh})")

    # WiX Dialog: 493x312 BMP
    dw, dh = 493, 312
    d_img = Image.new("RGB", (dw, dh), (12, 17, 29))
    d_draw = ImageDraw.Draw(d_img)
    for y in range(dh):
        ratio = y / dh
        r = int(12 + ratio * 15)
        g = int(17 + ratio * 20)
        b = int(29 + ratio * 35)
        d_draw.line([(0, y), (dw, y)], fill=(r, g, b))

    d_draw.rectangle([0, 0, 6, dh], fill=(249, 115, 22))

    icon_path = os.path.join(ICONS_DIR, "128x128.png")
    if os.path.exists(icon_path):
        icon = Image.open(icon_path).convert("RGBA").resize((72, 72), Image.Resampling.LANCZOS)
        d_img.paste(icon, (30, 30), icon)

    try:
        font_db = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 18)
        font_ds = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", 11)
    except Exception:
        font_db = ImageFont.load_default()
        font_ds = ImageFont.load_default()

    d_draw.text((120, 35), "TruthBeacon Console", fill=(255, 255, 255), font=font_db)
    d_draw.text((120, 65), "Community Impersonation Protection System", fill=(249, 115, 22), font=font_ds)
    d_draw.text((120, 110), "Welcome to the TruthBeacon Setup Wizard.", fill=(226, 232, 240), font=font_ds)
    d_draw.text((120, 130), "TruthBeacon monitors and neutralizes adversarial impersonators.", fill=(148, 163, 184), font=font_ds)

    d_out = os.path.join(ICONS_DIR, "wix-dialog.bmp")
    d_img.save(d_out, "BMP")
    print(f"Generated WiX Dialog: {d_out} ({dw}x{dh})")

if __name__ == "__main__":
    generate_dmg_background()
    generate_nsis_assets()
    generate_wix_assets()
    print("All installer branding graphics generated successfully.")
