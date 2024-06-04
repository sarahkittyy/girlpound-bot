pub fn hhmmss(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:0>2}:{:0>2}:{:0>2}", hours, minutes, seconds)
}

pub fn get_bit(value: u16, bit: u8) -> bool {
    value & (1 << bit) > 0
}

pub fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
