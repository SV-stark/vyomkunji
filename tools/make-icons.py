import struct, zlib, os

def png(path, size, rgb):
    w = h = size
    raw = b""
    row = bytes([0]) + bytes(rgb) * w  # filter 0 + RGB
    raw = row * h
    def chunk(typ, data):
        c = struct.pack(">I", len(data)) + typ + data
        c += struct.pack(">I", zlib.crc32(typ + data) & 0xffffffff)
        return c
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    data = (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(raw, 9))
            + chunk(b"IEND", b""))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)

rgb = (0x1a, 0x73, 0xe8)  # blue
for s in (16, 48, 128):
    png(rf"E:\Signer\extension\icons\icon{s}.png", s, rgb)
    png(rf"E:\Signer\extension-firefox\icons\icon{s}.png", s, rgb)
print("icons written")
