//! Equalizer-style loading animation: vertical bars with varying heights and grayscale.
//! Animated using time-based phase for smooth motion during loading.
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

const BAR_WIDTH: u16 = 1;
const GAP: u16 = 1;

pub struct LoadingWidget {
    /// Time in milliseconds for animation phase (use elapsed time)
    tick_ms: u64,
}

impl LoadingWidget {
    pub fn new(tick_ms: u64) -> Self {
        Self { tick_ms }
    }
}

impl Widget for LoadingWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height < 3 {
            return;
        }

        let cols_per_bar = BAR_WIDTH + GAP;
        let bar_count = (area.width / cols_per_bar).max(1) as usize;
        let max_height = (area.height - 2).max(1) as f64;
        let t = self.tick_ms as f64 * 0.008;

        for bar_idx in 0..bar_count {
            let x = area.x + (bar_idx as u16) * cols_per_bar;
            if x >= area.x + area.width {
                break;
            }

            // Animated height: wave that moves left-to-right over time (phase scales with bar count)
            let phase_scale = 28.0 / bar_count.max(1) as f64;
            let phase = t + (bar_idx as f64 * 0.35 * phase_scale);
            let wave = phase.sin() * 0.45 + 0.55;
            let h = (wave * max_height).max(1.0) as u16;

            // Grayscale intensity: animated gradient (darker to lighter)
            let intensity_phase = (t * 0.7) + (bar_idx as f64 * 0.25 * phase_scale);
            let intensity = (intensity_phase.sin() * 0.35 + 0.65).clamp(0.25, 1.0);
            let color = if intensity > 0.88 {
                Color::White
            } else if intensity > 0.7 {
                Color::Rgb(210, 210, 210)
            } else if intensity > 0.5 {
                Color::Rgb(150, 150, 150)
            } else if intensity > 0.35 {
                Color::Rgb(100, 100, 100)
            } else {
                Color::Rgb(70, 70, 70)
            };

            let style = Style::default().fg(color);
            let y_start = area.y + area.height - 1 - h;
            for dy in 0..h {
                let y = y_start + dy;
                if x < area.x + area.width && y < area.y + area.height {
                    buf.set_string(x, y, "│", style);
                }
            }
        }
    }
}
