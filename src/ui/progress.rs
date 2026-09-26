use colored::Colorize;

pub fn render_progress_bar(current: u64, total: u64, bar_width: usize) -> String {
    if total == 0 {
        return format!("[{}]", " ".repeat(bar_width));
    }

    let progress_ratio = (current as f64 / total as f64).clamp(0.0, 1.0);
    let filled_len = ((bar_width as f64) * progress_ratio).round() as usize;
    let unfilled_len = bar_width.saturating_sub(filled_len);

    let filled = "━".repeat(filled_len).green();
    let unfilled = "─".repeat(unfilled_len).bright_black();

    format!("[{}{}]", filled, unfilled)
}
