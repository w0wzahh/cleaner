// Throwaway: emit assets/icon.png (transparent) and assets/icon.ico
// (PNG-compressed ICO, 256px) from the processed icon data.
use cleaner::themes::generate_icon;
use std::io::Write;

fn png_chunk(t: &[u8; 4], d: &[u8]) -> Vec<u8> {
    let mut v = (d.len() as u32).to_be_bytes().to_vec();
    v.extend_from_slice(t);
    v.extend_from_slice(d);
    let mut crc = 0xFFFFFFFFu32;
    for &b in t.iter().chain(d.iter()) {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB88320 } else { crc >> 1 };
        }
    }
    v.extend_from_slice(&(!crc).to_be_bytes());
    v
}

fn encode_png_rgba(w: usize, h: usize, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 4 + 1) * h);
    for y in 0..h {
        raw.push(0u8);
        raw.extend_from_slice(&rgba[y * w * 4..(y + 1) * w * 4]);
    }
    let mut z = Vec::new();
    z.extend_from_slice(&[0x78, 0x01]);
    for chunk in raw.chunks(65535) {
        let last = chunk.len() < 65535;
        z.push(if last { 1 } else { 0 });
        z.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(chunk.len() as u16)).to_le_bytes());
        z.extend_from_slice(chunk);
    }
    let ad = raw.iter().fold((1u32, 0u32), |(a, b), &v| {
        ((a + v as u32) % 65521, (b + a + v as u32) % 65521)
    });
    z.extend_from_slice(&((ad.1 << 16) | ad.0).to_be_bytes());

    let mut ihdr = (w as u32).to_be_bytes().to_vec();
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(png_chunk(b"IHDR", &ihdr));
    png.extend(png_chunk(b"IDAT", &z));
    png.extend(png_chunk(b"IEND", &[]));
    png
}

fn main() {
    let icon = generate_icon();
    let (w, h) = (icon.width as usize, icon.height as usize);
    let png = encode_png_rgba(w, h, &icon.rgba);
    std::fs::write("assets/icon.png", &png).unwrap();

    // ICO: header + one PNG-compressed 256x256 entry.
    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    ico.extend_from_slice(&1u16.to_le_bytes()); // count
    ico.push(0); // width  = 256 (0 means 256)
    ico.push(0); // height = 256
    ico.push(0); // palette
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
    ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes()); // data offset
    ico.extend_from_slice(&png);
    std::fs::File::create("assets/icon.ico")
        .unwrap()
        .write_all(&ico)
        .unwrap();
    println!("wrote assets/icon.png + assets/icon.ico");
}
