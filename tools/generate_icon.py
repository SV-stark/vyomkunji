import math
import os
from PIL import Image, ImageDraw, ImageFilter

def create_vyomkunji_icon(size=1024):
    # Create master image with transparent background
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    
    # 1. Subtle drop shadow behind the main rounded squircle
    shadow_offset = int(size * 0.02)
    shadow_blur = int(size * 0.04)
    shadow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    s_draw = ImageDraw.Draw(shadow)
    
    pad = int(size * 0.065)
    radius = int(size * 0.20)
    
    s_draw.rounded_rectangle(
        [pad, pad + shadow_offset, size - pad, size - pad + shadow_offset],
        radius=radius,
        fill=(0, 0, 0, 110)
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(shadow_blur))
    img.paste(shadow, (0, 0), shadow)
    
    # 2. Main squircle mask & gradient background
    mask = Image.new("L", (size, size), 0)
    m_draw = ImageDraw.Draw(mask)
    m_draw.rounded_rectangle(
        [pad, pad, size - pad, size - pad],
        radius=radius,
        fill=255
    )
    
    # Create background gradient (Deep Space Indigo to Electric Azure)
    # Vyom (Sky/Cosmos) -> Deep dark sapphire to luminous cobalt
    bg = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    bg_draw = ImageDraw.Draw(bg)
    
    top_color = (11, 19, 43)      # #0B132B deep navy
    mid_color = (28, 48, 98)      # #1C3062 rich cosmic sapphire
    bot_color = (37, 99, 235)     # #2563EB electric azure
    
    for y in range(pad, size - pad):
        factor = (y - pad) / float(size - 2 * pad)
        if factor < 0.5:
            # Interpolate top to mid
            t = factor * 2.0
            r = int(top_color[0] + (mid_color[0] - top_color[0]) * t)
            g = int(top_color[1] + (mid_color[1] - top_color[1]) * t)
            b = int(top_color[2] + (mid_color[2] - top_color[2]) * t)
        else:
            # Interpolate mid to bot
            t = (factor - 0.5) * 2.0
            r = int(mid_color[0] + (bot_color[0] - mid_color[0]) * t)
            g = int(mid_color[1] + (bot_color[1] - mid_color[1]) * t)
            b = int(mid_color[2] + (bot_color[2] - mid_color[2]) * t)
        
        bg_draw.line([(pad, y), (size - pad, y)], fill=(r, g, b, 255))
        
    # Add ambient celestial glow (radial gradient at top center)
    glow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    g_draw = ImageDraw.Draw(glow)
    cx, cy = size * 0.5, size * 0.35
    max_r = size * 0.45
    for r_i in range(int(max_r), 0, -8):
        alpha = int(45 * (1.0 - r_i / max_r) ** 2)
        g_draw.ellipse([cx - r_i, cy - r_i, cx + r_i, cy + r_i], fill=(96, 165, 250, alpha))
        
    bg = Image.alpha_composite(bg, glow)
    
    # Composite background through squircle mask
    squircle = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    squircle.paste(bg, (0, 0), mask)
    
    # 3. Inner border rim highlight (subtle modern metallic/glass edge)
    border_layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    b_draw = ImageDraw.Draw(border_layer)
    b_draw.rounded_rectangle(
        [pad, pad, size - pad, size - pad],
        radius=radius,
        outline=(147, 197, 253, 90),
        width=int(size * 0.005)
    )
    b_draw.rounded_rectangle(
        [pad + 1, pad + 1, size - pad - 1, size - pad - 1],
        radius=radius - 1,
        outline=(255, 255, 255, 40),
        width=int(size * 0.003)
    )
    squircle = Image.alpha_composite(squircle, border_layer)
    img = Image.alpha_composite(img, squircle)
    
    # 4. Foreground: The "Kunji" (Celestial Key & Cryptographic Shield Motif)
    # We will draw a layer for the Key emblem with soft shadow
    key_shadow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    key_layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    
    ks_draw = ImageDraw.Draw(key_shadow)
    k_draw = ImageDraw.Draw(key_layer)
    
    center_x = size * 0.5
    center_y = size * 0.48
    
    # Dimensions
    head_cy = size * 0.38
    head_outer_r = size * 0.175
    head_inner_r = size * 0.088
    
    stem_w = size * 0.076
    stem_top = head_cy + head_inner_r * 0.4
    stem_bottom = size * 0.74
    
    # Function to draw the complete key shape onto a canvas
    def draw_key(draw, offset_x=0, offset_y=0, fill_color=(255, 255, 255, 255)):
        ox, oy = offset_x, offset_y
        
        # Bow Outer Circle
        draw.ellipse([
            center_x - head_outer_r + ox,
            head_cy - head_outer_r + oy,
            center_x + head_outer_r + ox,
            head_cy + head_outer_r + oy
        ], fill=fill_color)
        
        # Stem
        draw.rounded_rectangle([
            center_x - stem_w / 2.0 + ox,
            stem_top + oy,
            center_x + stem_w / 2.0 + ox,
            stem_bottom + oy
        ], radius=int(stem_w * 0.35), fill=fill_color)
        
        # Teeth / Bits (cryptographic modern geometric cutouts extending right)
        tooth_w = size * 0.085
        tooth_h1 = size * 0.052
        tooth_y1 = stem_bottom - tooth_h1
        
        tooth_h2 = size * 0.048
        tooth_y2 = stem_bottom - tooth_h1 - size * 0.042 - tooth_h2
        
        # Tooth 1 (bottom tooth)
        draw.rounded_rectangle([
            center_x + ox,
            tooth_y1 + oy,
            center_x + stem_w / 2.0 + tooth_w + ox,
            tooth_y1 + tooth_h1 + oy
        ], radius=int(size * 0.012), fill=fill_color)
        
        # Tooth 2 (upper tooth)
        draw.rounded_rectangle([
            center_x + ox,
            tooth_y2 + oy,
            center_x + stem_w / 2.0 + tooth_w * 0.75 + ox,
            tooth_y2 + tooth_h2 + oy
        ], radius=int(size * 0.012), fill=fill_color)

    # Draw key shadow
    draw_key(ks_draw, 0, int(size * 0.025), fill_color=(5, 12, 30, 160))
    key_shadow = key_shadow.filter(ImageFilter.GaussianBlur(int(size * 0.03)))
    img = Image.alpha_composite(img, key_shadow)
    
    # Draw key body
    draw_key(k_draw, 0, 0, fill_color=(255, 255, 255, 255))
    
    # Cut out the central keyhole / celestial aperture from the key head
    hole_mask = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    h_draw = ImageDraw.Draw(hole_mask)
    
    # Central hole: Circle with lower slot (classic keyhole) OR celestial diamond star
    hole_r = head_inner_r
    h_draw.ellipse([
        center_x - hole_r,
        head_cy - hole_r,
        center_x + hole_r,
        head_cy + hole_r
    ], fill=(0, 0, 0, 255))
    
    # Modern keyhole slot below circle
    slot_w = hole_r * 0.72
    slot_h = hole_r * 0.95
    h_draw.polygon([
        (center_x - slot_w * 0.5, head_cy),
        (center_x + slot_w * 0.5, head_cy),
        (center_x + slot_w * 0.65, head_cy + slot_h),
        (center_x - slot_w * 0.65, head_cy + slot_h)
    ], fill=(0, 0, 0, 255))
    
    # Subtract hole from key_layer:
    # Where hole_mask is black, make key_layer alpha 0
    key_pixels = key_layer.load()
    hole_pixels = hole_mask.load()
    for y in range(size):
        for x in range(size):
            if hole_pixels[x, y][3] > 0:
                key_pixels[x, y] = (0, 0, 0, 0)
                
    # Now let's colorize the key body with a subtle vertical gradient (Pure White -> Soft Cyan Ice)
    grad_key = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    gk_draw = ImageDraw.Draw(grad_key)
    for y in range(int(head_cy - head_outer_r), int(stem_bottom + size * 0.05)):
        f = (y - (head_cy - head_outer_r)) / float(stem_bottom + size * 0.05 - (head_cy - head_outer_r))
        f = max(0.0, min(1.0, f))
        # from (255, 255, 255) to (200, 230, 255)
        kr = int(255 - 45 * f)
        kg = int(255 - 25 * f)
        kb = 255
        gk_draw.line([(0, y), (size, y)], fill=(kr, kg, kb, 255))
        
    # Mask gradient by key_layer alpha
    key_final = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    key_final.paste(grad_key, (0, 0), key_layer)
    
    # Add a glowing celestial 4-point star in the center of the keyhole
    star_layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    st_draw = ImageDraw.Draw(star_layer)
    star_size = head_inner_r * 0.75
    # 4-point star coordinates
    star_pts = [
        (center_x, head_cy - star_size),
        (center_x + star_size * 0.22, head_cy - star_size * 0.22),
        (center_x + star_size, head_cy),
        (center_x + star_size * 0.22, head_cy + star_size * 0.22),
        (center_x, head_cy + star_size),
        (center_x - star_size * 0.22, head_cy + star_size * 0.22),
        (center_x - star_size, head_cy),
        (center_x - star_size * 0.22, head_cy - star_size * 0.22),
    ]
    st_draw.polygon(star_pts, fill=(56, 189, 248, 255))
    
    # Center white dot of star
    dot_r = star_size * 0.25
    st_draw.ellipse([
        center_x - dot_r,
        head_cy - dot_r,
        center_x + dot_r,
        head_cy + dot_r
    ], fill=(255, 255, 255, 255))
    
    # Star glow
    star_glow = star_layer.filter(ImageFilter.GaussianBlur(int(size * 0.015)))
    img = Image.alpha_composite(img, star_glow)
    img = Image.alpha_composite(img, star_layer)
    img = Image.alpha_composite(img, key_final)
    
    return img

if __name__ == "__main__":
    print("Generating VyomKunji modern square icon...")
    master = create_vyomkunji_icon(1024)
    
    os.makedirs("assets", exist_ok=True)
    os.makedirs("extension/icons", exist_ok=True)
    os.makedirs("extension-firefox/icons", exist_ok=True)
    
    # Save master & downsampled versions
    sizes = [512, 256, 128, 64, 48, 32, 16]
    images = {}
    for s in sizes:
        resized = master.resize((s, s), Image.Resampling.LANCZOS)
        images[s] = resized
        resized.save(f"assets/icon-{s}.png")
        print(f"Saved assets/icon-{s}.png")
        
    # Standard asset icons
    images[512].save("assets/icon.png")
    print("Saved assets/icon.png")
    
    # Multi-resolution Windows ICO
    # Pillow supports saving .ico with multiple sizes
    images[256].save(
        "assets/app.ico",
        format="ICO",
        sizes=[(256, 256), (48, 48), (32, 32), (16, 16)]
    )
    print("Saved assets/app.ico")
    
    # Extension icons
    for s in [16, 48, 128]:
        images[s].save(f"extension/icons/icon{s}.png")
        images[s].save(f"extension-firefox/icons/icon{s}.png")
        print(f"Updated extension icons for size {s}")
        
    # Dump 32x32 raw RGBA bytes for instant embedding in native-app
    rgba_bytes = images[32].tobytes("raw", "RGBA")
    with open("native-app/src/icon_32.rgba", "wb") as f:
        f.write(rgba_bytes)
    print(f"Saved native-app/src/icon_32.rgba ({len(rgba_bytes)} bytes)")
