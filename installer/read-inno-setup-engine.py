"""Read the temporary setup engine using Inno Setup 6.7.3 loader and compression formats.

See SetupLdr.dpr, Src/Shared.Struct.pas and Src/Compression.Base.pas at
https://github.com/jrsoftware/issrc/tree/is-6_7_3/Projects.
"""

import hashlib
import lzma
import struct
import sys
import zlib

MAX_ENGINE_SIZE = 32 * 1024 * 1024
MAX_DICT_SIZE = 64 * 1024 * 1024

def main():
    if len(sys.argv) != 3:
        raise ValueError("Usage: script.py INSTALLER EXTRACTED_ENGINE")
    installer_path, engine_path = sys.argv[1], sys.argv[2]
    marker = b'rDlPtS\xcd\xe6\xd7\x7b\x0b\x2a'
    with open(installer_path, 'rb') as f:
        head = f.read(1024 * 1024)
        idx = head.find(marker)
        if idx < 0:
            raise ValueError("table marker not found in first 1MiB")
        if head.find(marker, idx + 1) >= 0:
            raise ValueError("table marker not unique in first 1MiB")
        f.seek(idx)
        table_data = f.read(64)
        if len(table_data) != 64:
            raise ValueError("failed to read table struct")
        (m, version, totalSize, offsetExe, uncompressedSize, crcExe,
         offset0, offset1, reserved, tableCRC) = struct.unpack('<12sIQQIIQQII', table_data)
        if m != marker:
            raise ValueError("marker mismatch in table")
        if version != 2:
            raise ValueError(f"unsupported table version {version}")
        calc_crc = zlib.crc32(table_data[:-4]) & 0xffffffff
        if calc_crc != tableCRC:
            raise ValueError(f"table CRC mismatch: {calc_crc:08x} != {tableCRC:08x}")

        f.seek(offsetExe)
        hdr_crc_bytes = f.read(4)
        if len(hdr_crc_bytes) != 4:
            raise ValueError("failed to read header CRC")
        hdr_crc = struct.unpack('<I', hdr_crc_bytes)[0]
        hdr_data = f.read(9)
        if len(hdr_data) != 9:
            raise ValueError("failed to read header data")
        calc_hdr_crc = zlib.crc32(hdr_data) & 0xffffffff
        if calc_hdr_crc != hdr_crc:
            raise ValueError(f"header CRC mismatch: {calc_hdr_crc:08x} != {hdr_crc:08x}")
        storedSize, compressed = struct.unpack('<QB', hdr_data)
        if storedSize > MAX_ENGINE_SIZE or uncompressedSize > MAX_ENGINE_SIZE:
            raise ValueError("engine size limit exceeded")

        chunks = bytearray()
        remaining = storedSize
        while remaining > 0:
            if remaining <= 4:
                raise ValueError("failed to read chunk data")
            remaining -= 4
            chunk_size = min(4096, remaining)
            crc_bytes = f.read(4)
            if len(crc_bytes) != 4:
                raise ValueError("failed to read chunk data")
            expected_crc = struct.unpack("<I", crc_bytes)[0]
            chunk = f.read(chunk_size)
            if len(chunk) != chunk_size:
                raise ValueError("failed to read chunk data")
            actual_crc = zlib.crc32(chunk) & 0xffffffff
            if actual_crc != expected_crc:
                raise ValueError("CRC mismatch")
            chunks.extend(chunk)
            remaining -= chunk_size

    if compressed:
        if len(chunks) < 5:
            raise ValueError("compressed data too short for LZMA header")
        props = chunks[0]
        dict_size = int.from_bytes(chunks[1:5], 'little')
        if dict_size == 0 or dict_size > MAX_DICT_SIZE:
            raise ValueError("LZMA dictionary size out of range")
        lc = props % 9
        lp = (props // 9) % 5
        pb = props // 45
        filters = [{"id": lzma.FILTER_LZMA1, "dict_size": dict_size, "lc": lc, "lp": lp, "pb": pb}]
        dec = lzma.LZMADecompressor(format=lzma.FORMAT_RAW, filters=filters)
        try:
            decoded = dec.decompress(bytes(chunks[5:]), max_length=uncompressedSize + 1)
        except lzma.LZMAError as e:
            raise ValueError(f"LZMA decompression failed: {e}")
        if len(decoded) != uncompressedSize:
            raise ValueError(f"decompressed size mismatch: {len(decoded)} != {uncompressedSize}")
    else:
        decoded = bytes(chunks)
        if len(decoded) != uncompressedSize:
            raise ValueError(f"uncompressed size mismatch: {len(decoded)} != {uncompressedSize}")

    decoded = bytearray(decoded)
    size = len(decoded)
    i = 0
    while i < size - 4:
        if decoded[i] in (0xe8, 0xe9):
            i += 1
            if decoded[i + 3] in (0x00, 0xff):
                rel = (int.from_bytes(decoded[i:i + 3], 'little') - (i + 4)) & 0xffffff
                if rel & 0x800000:
                    decoded[i + 3] ^= 0xff
                decoded[i:i + 3] = rel.to_bytes(3, 'little')
            i += 4
        else:
            i += 1

    if len(decoded) != uncompressedSize:
        raise ValueError(f"decoded size mismatch: {len(decoded)} != {uncompressedSize}")
    calc_crc = zlib.crc32(decoded) & 0xffffffff
    if calc_crc != crcExe:
        raise ValueError(f"decoded CRC mismatch: {calc_crc:08x} != {crcExe:08x}")
    if decoded[0:2] != b'MZ':
        raise ValueError("decoded data does not start with MZ")

    with open(engine_path, 'xb') as out:
        out.write(decoded)
    print(hashlib.sha256(decoded).hexdigest())

if __name__ == '__main__':
    main()
