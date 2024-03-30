pub fn hhmmss(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:0>2}:{:0>2}:{:0>2}", hours, minutes, seconds)
}
