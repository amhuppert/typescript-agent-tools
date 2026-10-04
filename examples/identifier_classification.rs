// Dump the unmodified dependency's classification for the exclusion generator.
fn main() -> std::io::Result<()> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".cache/unicode/rust.bin".into());
    let classification: Vec<u8> = (0..0x110000)
        .map(|cp| {
            char::from_u32(cp).map_or(0, |c| {
                let start = c == '$' || c == '_' || unicode_id_start::is_id_start(c);
                let part = c == '$'
                    || c == '_'
                    || matches!(c, '\u{200c}' | '\u{200d}')
                    || unicode_id_start::is_id_continue(c);
                u8::from(start) | (u8::from(part) << 1)
            })
        })
        .collect();
    std::fs::write(output, classification)
}
