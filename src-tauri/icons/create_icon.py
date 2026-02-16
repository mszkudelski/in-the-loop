# Simple 1x1 PNG (valid but minimal)
import struct
import zlib

def create_simple_png(width, height, filename):
    # PNG signature
    png_sig = b'\x89PNG\r\n\x1a\n'
    
    # IHDR chunk
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    ihdr = b'IHDR' + ihdr_data
    ihdr_crc = struct.pack('>I', zlib.crc32(ihdr))
    ihdr_chunk = struct.pack('>I', len(ihdr_data)) + ihdr + ihdr_crc
    
    # IDAT chunk - simple blue image
    raw_data = b''
    for y in range(height):
        raw_data += b'\x00'  # Filter type
        for x in range(width):
            raw_data += b'\x42\x6c\xff'  # RGB: blue color
    
    compressed = zlib.compress(raw_data)
    idat = b'IDAT' + compressed
    idat_crc = struct.pack('>I', zlib.crc32(idat))
    idat_chunk = struct.pack('>I', len(compressed)) + idat + idat_crc
    
    # IEND chunk
    iend_chunk = struct.pack('>I', 0) + b'IEND' + struct.pack('>I', zlib.crc32(b'IEND'))
    
    # Write file
    with open(filename, 'wb') as f:
        f.write(png_sig + ihdr_chunk + idat_chunk + iend_chunk)

create_simple_png(32, 32, 'icon.png')
create_simple_png(128, 128, '128x128.png')
create_simple_png(128, 128, '128x128@2x.png')
create_simple_png(32, 32, '32x32.png')
print("Icons created")
