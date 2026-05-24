use std::time::Instant;

use synapse_config::config::QuakeConfig;
use winit::{dpi::LogicalPosition, window::Window};

const SLIDE_EASING: fn(f64) -> f64 = ease_out_quad;

fn ease_out_quad(t: f64) -> f64 {
    t * (2.0 - t)
}

pub struct QuakeMode {
    pub window_height: f64,
    pub target_y: f64,
    pub current_y: f64,
    anim_start_y: f64,
    anim_start: Instant,
    animation_ms: u64,
    pub hide_on_focus_lost: bool,
}

impl QuakeMode {
    pub fn new(
        config: &QuakeConfig,
        window_height: f64,
        _screen_height: f64,
        _screen_width: f64,
    ) -> Self {
        QuakeMode {
            window_height,
            target_y: 0.0,
            current_y: 0.0,
            anim_start_y: 0.0,
            anim_start: Instant::now(),
            animation_ms: config.animation_ms,
            hide_on_focus_lost: config.hide_on_focus_lost,
        }
    }

    pub fn toggle(&mut self) {
        self.anim_start = Instant::now();
        self.anim_start_y = self.current_y;
        if self.target_y.abs() < 1.0 {
            self.target_y = -(self.window_height);
        } else {
            self.target_y = 0.0;
        }
    }

    pub fn hide(&mut self) {
        if (self.target_y + self.window_height).abs() < 1.0 {
            return;
        }
        self.anim_start = Instant::now();
        self.anim_start_y = self.current_y;
        self.target_y = -(self.window_height);
    }

    fn is_animating(&self) -> bool {
        (self.target_y - self.current_y).abs() > 0.5
    }

    pub fn animate(&mut self, reduce_motion: bool) {
        if !self.is_animating() {
            self.current_y = self.target_y;
            return;
        }
        if reduce_motion {
            self.current_y = self.target_y;
            return;
        }
        let elapsed = self.anim_start.elapsed().as_secs_f64() * 1000.0;
        let duration = self.animation_ms as f64;
        let progress = if duration > 0.0 {
            (elapsed / duration).min(1.0)
        } else {
            1.0
        };
        let eased = SLIDE_EASING(progress);
        self.current_y = self.anim_start_y + (self.target_y - self.anim_start_y) * eased;
        if progress >= 1.0 {
            self.current_y = self.target_y;
        }
    }

    pub fn apply_position(&self, window: &Window) {
        let pos = LogicalPosition::new(0.0, self.current_y);
        window.set_outer_position(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> QuakeConfig {
        QuakeConfig {
            enabled: true,
            height_percent: 0.4,
            animation_ms: 200,
            hide_on_focus_lost: true,
            hotkey: "Ctrl+Space".into(),
        }
    }

    #[test]
    fn test_quake_new() {
        let cfg = test_config();
        let qm = QuakeMode::new(&cfg, 400.0, 1000.0, 1920.0);
        assert_eq!(qm.window_height, 400.0);
        assert_eq!(qm.target_y, 0.0);
        assert_eq!(qm.current_y, 0.0);
        assert!(qm.hide_on_focus_lost);
    }

    #[test]
    fn test_quake_toggle_hides_when_visible() {
        let cfg = test_config();
        let mut qm = QuakeMode::new(&cfg, 400.0, 1000.0, 1920.0);
        qm.toggle();
        assert!(qm.target_y < 0.0);
    }

    #[test]
    fn test_quake_toggle_shows_when_hidden() {
        let cfg = QuakeConfig {
            animation_ms: 0,
            ..test_config()
        };
        let mut qm = QuakeMode::new(&cfg, 400.0, 1000.0, 1920.0);
        qm.hide();
        qm.animate(false);
        assert!(qm.current_y < 0.0);
        qm.toggle();
        qm.animate(false);
        assert_eq!(qm.current_y, 0.0);
    }

    #[test]
    fn test_quake_hide_from_visible() {
        let cfg = QuakeConfig {
            animation_ms: 0,
            ..test_config()
        };
        let mut qm = QuakeMode::new(&cfg, 400.0, 1000.0, 1920.0);
        qm.hide();
        qm.animate(false);
        assert_eq!(qm.current_y, -400.0);
    }

    #[test]
    fn test_quake_double_hide_idempotent() {
        let cfg = QuakeConfig {
            animation_ms: 0,
            ..test_config()
        };
        let mut qm = QuakeMode::new(&cfg, 400.0, 1000.0, 1920.0);
        qm.hide();
        qm.animate(false);
        assert_eq!(qm.current_y, -400.0);
        qm.hide();
        qm.animate(false);
        assert_eq!(qm.current_y, -400.0);
    }

    #[test]
    fn test_quake_animate_interpolates() {
        let cfg = QuakeConfig {
            animation_ms: 200,
            ..test_config()
        };
        let mut qm = QuakeMode::new(&cfg, 400.0, 1000.0, 1920.0);
        qm.hide();
        qm.animate(false);
        assert!(qm.current_y <= 0.0);
        assert!(qm.current_y > -400.0);
    }
}
