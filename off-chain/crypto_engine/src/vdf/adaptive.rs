use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct AdaptiveVdfConfig {
	pub t_base: u64,
	pub t_max: u64,
	pub alpha_num: u64,
	pub alpha_den: u64,
	pub decay_num: u64,
	pub decay_den: u64,
	pub decay_epoch_threshold: u32,
	pub expected_solve_ms: u64,
	pub tolerance_pct: u64,
}

impl Default for AdaptiveVdfConfig {
	fn default() -> Self {
		Self {
			t_base: 1 << 16,
			t_max: 1 << 22,
			alpha_num: 5,
			alpha_den: 4,
			decay_num: 9,
			decay_den: 10,
			decay_epoch_threshold: 3,
			expected_solve_ms: 2000,
			tolerance_pct: 15,
		}
	}
}

#[derive(Debug, Clone)]
pub struct AdaptiveVdfState {
	pub current_t: u64,
	pub healthy_epoch_streak: u32,
	pub last_actual_ms: u64,
}

impl AdaptiveVdfState {
	pub fn new(cfg: &AdaptiveVdfConfig) -> Self {
		Self {
			current_t: cfg.t_base,
			healthy_epoch_streak: 0,
			last_actual_ms: cfg.expected_solve_ms,
		}
	}

	pub fn update(&mut self, cfg: &AdaptiveVdfConfig, actual_ms: u64) -> Result<u64> {
		if cfg.alpha_den == 0 || cfg.decay_den == 0 {
			bail!("invalid adaptive config: denominator is zero");
		}
		if cfg.t_max < cfg.t_base {
			bail!("invalid adaptive config: t_max < t_base");
		}

		self.last_actual_ms = actual_ms;
		let lower = cfg.expected_solve_ms
			.saturating_mul(100u64.saturating_sub(cfg.tolerance_pct)) / 100;
		let upper = cfg.expected_solve_ms
			.saturating_mul(100 + cfg.tolerance_pct) / 100;

		if actual_ms < lower {
			self.healthy_epoch_streak = 0;
			let raised = self
				.current_t
				.saturating_mul(cfg.alpha_num)
				/ cfg.alpha_den;
			self.current_t = raised.min(cfg.t_max).max(cfg.t_base);
		} else if actual_ms > upper {
			self.healthy_epoch_streak = 0;
		} else {
			self.healthy_epoch_streak = self.healthy_epoch_streak.saturating_add(1);
			if self.healthy_epoch_streak >= cfg.decay_epoch_threshold {
				let decayed = self
					.current_t
					.saturating_mul(cfg.decay_num)
					/ cfg.decay_den;
				self.current_t = decayed.max(cfg.t_base);
				self.healthy_epoch_streak = 0;
			}
		}

		Ok(self.current_t)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn t_rises_when_solve_too_fast_but_capped() {
		let cfg = AdaptiveVdfConfig { t_base: 100, t_max: 400, ..Default::default() };
		let mut s = AdaptiveVdfState::new(&cfg);
		for _ in 0..20 {
			s.update(&cfg, 10).unwrap();
		}
		assert_eq!(s.current_t, 400);
	}

	#[test]
	fn t_decays_after_healthy_epochs() {
		let cfg = AdaptiveVdfConfig { t_base: 50, t_max: 1000, ..Default::default() };
		let mut s = AdaptiveVdfState::new(&cfg);
		s.current_t = 500;
		for _ in 0..10 {
			s.update(&cfg, cfg.expected_solve_ms).unwrap();
		}
		assert!(s.current_t < 500);
		assert!(s.current_t >= cfg.t_base);
	}

	#[test]
	fn liveness_recovery_returns_to_base() {
		let cfg = AdaptiveVdfConfig { t_base: 10, t_max: 1_000_000, ..Default::default() };
		let mut s = AdaptiveVdfState::new(&cfg);
		for _ in 0..50 {
			s.update(&cfg, 1).unwrap();
		}
		let peak = s.current_t;
		assert!(peak > cfg.t_base);
		for _ in 0..2000 {
			s.update(&cfg, cfg.expected_solve_ms).unwrap();
		}
		assert_eq!(s.current_t, cfg.t_base);
	}
}
